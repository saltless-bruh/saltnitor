//! src/control_api.rs  (v2 — native-router edition)
//!
//! Saltnitor's headless control API, reworked to sit in FRONT of llama.cpp's
//! native router (`llama-server --models-preset presets.ini --models-max 1`).
//!
//! The router already does the hot-swap: an IDE/agent sets `"model": "B"` in
//! /v1/chat/completions and the router auto-loads B (evicting the incumbent,
//! since --models-max 1). So Saltnitor no longer writes router.env or restarts
//! systemd. Its job shrinks to the ONE thing the router lacks:
//!
//!   * VRAM/RAM ORACLE — refuse a load that won't fit BEFORE it OOMs the box.
//!   * deterministic "ensure resident" (oracle -> trigger autoload -> wait ready).
//!   * a load-progress SSE stream + status/telemetry for human-facing tools.
//!
//! Agents that don't want the oracle can skip this entirely and hit the router
//! directly (just set the model field). This layer is the *guarded* path —
//! worth it for the offload-heavy Tier B, optional for the always-fits Tier A.
//!
//! Cargo.toml:  axum = "0.7"   tokio-stream = "0.1"   (tokio/serde/reqwest already present)

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{
    body::{Body, Bytes},
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::sse::{Event as SseEvent, KeepAlive, Sse},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex};
use tokio_stream::{wrappers::ReceiverStream, StreamExt};

use crate::events::Event;

// ───────────────────────── profile metadata (oracle only) ─────────────────────────
// The router flags live in presets.ini. Here we only need what the ORACLE needs:
// the model file name (to match the router's loaded list) and footprint hints.
#[derive(Clone, Debug, Deserialize)]
pub struct ProfileMeta {
    pub model: String,                       // gguf filename — must match presets.ini
    #[serde(default)] pub offload: bool,     // true if the preset uses -ot exps=CPU
    #[serde(default)] pub est_vram_gb: Option<f64>,
    #[serde(default)] pub est_ram_gb: Option<f64>,
}

pub struct ControlApi {
    profiles: HashMap<String, ProfileMeta>,
    router_base: String,                     // e.g. http://127.0.0.1:8080
    infer_bearer: Option<String>,            // bearer the router expects, if any
    control_token: Option<String>,           // bearer required on THIS API (None = open on localhost)
    reserve_vram_gb: f64,                     // GPU headroom kept free (compute buffers)
    reserve_ram_gb: f64,
    tx: mpsc::Sender<Event>,
    http: reqwest::Client,
    ensure_lock: Mutex<()>,                  // serialize ensures (don't fire two loads at once)
}

impl ControlApi {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        profiles: HashMap<String, ProfileMeta>,
        router_base: String,
        infer_bearer: Option<String>,
        control_token: Option<String>,
        reserve_vram_gb: f64,
        reserve_ram_gb: f64,
        tx: mpsc::Sender<Event>,
    ) -> Self {
        Self {
            profiles, router_base, infer_bearer, control_token,
            reserve_vram_gb, reserve_ram_gb, tx,
            http: reqwest::Client::new(),
            ensure_lock: Mutex::new(()),
        }
    }

    fn infer_endpoint(&self) -> String { format!("{}/v1", self.router_base) }

    async fn log(&self, line: impl Into<String>) {
        let _ = self.tx.send(Event::LogLine(line.into())).await;
    }

    /// Which preset names the router currently reports as LOADED.
    /// Tolerant parse: looks for a truthy loaded/state/status per entry.
    /// (Verify the exact field name against your llama.cpp /v1/models output.)
    async fn router_loaded(&self) -> Vec<String> {
        let url = format!("{}/v1/models", self.router_base);
        let Ok(r) = self.http.get(&url).timeout(Duration::from_millis(1500)).send().await else {
            return vec![];
        };
        let Ok(v) = r.json::<serde_json::Value>().await else { return vec![]; };
        let mut out = vec![];
        if let Some(arr) = v["data"].as_array() {
            for m in arr {
                let id = m["id"].as_str().unwrap_or("").to_string();
                let loaded = m["loaded"].as_bool().unwrap_or(false)
                    || m["state"].as_str() == Some("loaded")
                    || m["status"].as_str() == Some("loaded");
                if loaded && !id.is_empty() { out.push(id); }
            }
        }
        out
    }

    /// Trigger the router to autoload+warm a preset by issuing a 1-token request.
    /// The request blocks until the model is loaded and serving, so its return
    /// == "resident and warm". (If your build exposes POST /models/load, swap it
    /// in; this warm-request path is runtime-agnostic.)
    async fn warm_load(&self, preset: &str) -> Result<(), String> {
        let url = format!("{}/v1/chat/completions", self.router_base);
        let body = format!(
            r#"{{"model":"{}","messages":[{{"role":"user","content":"warmup"}}],"max_tokens":1}}"#,
            preset
        );
        let mut req = self.http.post(&url)
            .header("Content-Type", "application/json")
            .timeout(Duration::from_secs(120));
        if let Some(tok) = &self.infer_bearer {
            req = req.header("Authorization", format!("Bearer {}", tok));
        }
        match req.body(body).send().await {
            Ok(r) if r.status().is_success() => Ok(()),
            Ok(r) => Err(format!("router returned {}", r.status())),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Oracle: does `prof` fit once the incumbent is evicted? With --models-max 1
    /// the router evicts the resident model on load, so we check against TOTAL
    /// capacity minus a reserve — not current free.
    async fn oracle(&self, prof: &ProfileMeta) -> Result<(f64, f64), OomInfo> {
        let (need_v, need_r) = estimate_footprint(prof);
        let total_v = vram_total_gb().await;
        let total_r = total_ram_gb();
        let vram_ok = need_v + self.reserve_vram_gb <= total_v;
        let ram_ok  = need_r + self.reserve_ram_gb  <= total_r;
        if vram_ok && ram_ok { Ok((need_v, need_r)) }
        else { Err(OomInfo { need_vram_gb: need_v, total_vram_gb: total_v,
                             need_ram_gb: need_r, total_ram_gb: total_r }) }
    }

    /// Ensure `profile` is the resident model. Idempotent. The core entry point
    /// behind Saltcode's `ensure_resident()`.
    pub async fn ensure(&self, req: EnsureRequest, sink: &ProgressSink) -> EnsureOutcome {
        let prof = match self.profiles.get(&req.profile) {
            Some(p) => p.clone(),
            None => return EnsureOutcome::Bad(format!("unknown profile '{}'", req.profile)),
        };
        let gguf = basename(&prof.model);   // used for the oracle footprint + log; router id is the section name
        sink.emit(Stage::Received { profile: req.profile.clone(), model: req.profile.clone() }).await;

        let _serialize = self.ensure_lock.lock().await;     // one ensure at a time

        // idempotency — is this preset already loaded? The router reports SECTION NAMES
        // (A_STD / A_FOCUS / B) as model ids, so compare against the requested profile.
        if self.router_loaded().await.iter().any(|m| m == &req.profile) {
            return EnsureOutcome::AlreadyResident { model: req.profile.clone(), endpoint: self.infer_endpoint() };
        }

        // oracle gate (skippable with force) — footprint derived from the gguf in [profiles]
        let est_vram = match self.oracle(&prof).await {
            Ok((v, _)) => { sink.emit(Stage::OracleOk { vram_estimate_gb: v }).await; Some(v) }
            Err(info) => {
                if !req.force.unwrap_or(false) { return EnsureOutcome::Oom(info); }
                self.log(">>> ENSURE: oracle predicted tight fit, FORCED by request").await;
                Some(info.need_vram_gb)
            }
        };

        // trigger autoload + warm (blocks until resident); router auto-evicts incumbent
        sink.emit(Stage::Loading { model: req.profile.clone() }).await;
        self.log(format!(">>> ENSURE: loading preset [{}] ({})", req.profile, gguf)).await;
        let start = Instant::now();
        if let Err(e) = self.warm_load(&req.profile).await {
            return EnsureOutcome::Err(format!("router load failed: {}", e));
        }
        let load_ms = start.elapsed().as_millis();

        let _ = self.tx.send(Event::ActiveModelSet(req.profile.clone())).await;
        self.log(format!(">>> ENSURE: [{}] resident & warm in {} ms", req.profile, load_ms)).await;
        EnsureOutcome::Loaded { model: req.profile.clone(), endpoint: self.infer_endpoint(),
                                load_ms, vram_estimate_gb: est_vram }
    }

    pub async fn status(&self) -> StatusOut {
        StatusOut {
            resident_models: self.router_loaded().await,
            endpoint: self.infer_endpoint(),
            vram_used_gb: round1(vram_used_gb().await),
            vram_total_gb: round1(vram_total_gb().await),
            ram_free_gb: round1(free_ram_gb()),
            profiles: self.profiles.keys().cloned().collect(),
        }
    }
}

// ───────────────────────── progress (shared by JSON + SSE paths) ─────────────────────────
pub enum ProgressSink { None, Chan(mpsc::Sender<Stage>) }
impl ProgressSink {
    async fn emit(&self, s: Stage) { if let ProgressSink::Chan(tx) = self { let _ = tx.send(s).await; } }
}

#[derive(Serialize, Clone)]
#[serde(tag = "stage", rename_all = "snake_case")]
pub enum Stage {
    Received { profile: String, model: String },
    OracleOk { vram_estimate_gb: f64 },
    Loading  { model: String },
    Done { status: String, model: String, endpoint: String,
           load_ms: Option<u128>, vram_estimate_gb: Option<f64> },
    Oom   { detail: String },
    Error { detail: String },
}
impl Stage {
    fn from_outcome(o: EnsureOutcome) -> Self {
        match o {
            EnsureOutcome::Loaded { model, endpoint, load_ms, vram_estimate_gb } =>
                Stage::Done { status: "loaded".into(), model, endpoint,
                              load_ms: Some(load_ms), vram_estimate_gb },
            EnsureOutcome::AlreadyResident { model, endpoint } =>
                Stage::Done { status: "already_resident".into(), model, endpoint,
                              load_ms: None, vram_estimate_gb: None },
            EnsureOutcome::Oom(i) => Stage::Oom { detail: format!(
                "need ~{:.1}GB VRAM (have {:.1}) / ~{:.1}GB RAM (have {:.1}); pass force=true",
                i.need_vram_gb, i.total_vram_gb, i.need_ram_gb, i.total_ram_gb) },
            EnsureOutcome::Bad(e) | EnsureOutcome::Err(e) => Stage::Error { detail: e },
        }
    }
}

// ───────────────────────── HTTP types ─────────────────────────
#[derive(Deserialize, Default)]
pub struct EnsureRequest {
    pub profile: String,
    pub force: Option<bool>,
    pub token: Option<String>,    // SSE query-param auth (EventSource can't set headers)
}

#[derive(Serialize)]
pub struct EnsureResponse {
    pub status: String,           // loaded | already_resident | oom_rejected | bad_request | error
    pub model: String,
    pub endpoint: String,
    #[serde(skip_serializing_if = "Option::is_none")] pub load_ms: Option<u128>,
    #[serde(skip_serializing_if = "Option::is_none")] pub vram_estimate_gb: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")] pub detail: Option<String>,
}

#[derive(Serialize)]
pub struct StatusOut {
    pub resident_models: Vec<String>,
    pub endpoint: String,
    pub vram_used_gb: f64,
    pub vram_total_gb: f64,
    pub ram_free_gb: f64,
    pub profiles: Vec<String>,
}

pub enum EnsureOutcome {
    Loaded { model: String, endpoint: String, load_ms: u128, vram_estimate_gb: Option<f64> },
    AlreadyResident { model: String, endpoint: String },
    Oom(OomInfo),
    Bad(String),
    Err(String),
}
#[derive(Serialize)]
pub struct OomInfo { pub need_vram_gb: f64, pub total_vram_gb: f64,
                     pub need_ram_gb: f64,  pub total_ram_gb: f64 }

fn auth_ok(api: &ControlApi, headers: &HeaderMap) -> bool {
    match &api.control_token {
        None => true,
        Some(t) => headers.get("authorization").and_then(|h| h.to_str().ok())
            == Some(&format!("Bearer {}", t)),
    }
}

async fn h_ensure(
    State(api): State<Arc<ControlApi>>, headers: HeaderMap, Json(req): Json<EnsureRequest>,
) -> (StatusCode, Json<EnsureResponse>) {
    if !auth_ok(&api, &headers) {
        return (StatusCode::UNAUTHORIZED, Json(EnsureResponse {
            status: "error".into(), model: String::new(), endpoint: api.infer_endpoint(),
            load_ms: None, vram_estimate_gb: None, detail: Some("bad token".into()) }));
    }
    match api.ensure(req, &ProgressSink::None).await {
        EnsureOutcome::Loaded { model, endpoint, load_ms, vram_estimate_gb } =>
            (StatusCode::OK, Json(EnsureResponse { status: "loaded".into(), model, endpoint,
                load_ms: Some(load_ms), vram_estimate_gb, detail: None })),
        EnsureOutcome::AlreadyResident { model, endpoint } =>
            (StatusCode::OK, Json(EnsureResponse { status: "already_resident".into(), model,
                endpoint, load_ms: None, vram_estimate_gb: None, detail: None })),
        EnsureOutcome::Oom(i) =>
            (StatusCode::INSUFFICIENT_STORAGE, Json(EnsureResponse { status: "oom_rejected".into(),
                model: String::new(), endpoint: api.infer_endpoint(), load_ms: None,
                vram_estimate_gb: Some(i.need_vram_gb), detail: Some(format!(
                    "need ~{:.1}GB VRAM (have {:.1}) / ~{:.1}GB RAM (have {:.1})",
                    i.need_vram_gb, i.total_vram_gb, i.need_ram_gb, i.total_ram_gb)) })),
        EnsureOutcome::Bad(e) =>
            (StatusCode::BAD_REQUEST, Json(EnsureResponse { status: "bad_request".into(),
                model: String::new(), endpoint: api.infer_endpoint(), load_ms: None,
                vram_estimate_gb: None, detail: Some(e) })),
        EnsureOutcome::Err(e) =>
            (StatusCode::SERVICE_UNAVAILABLE, Json(EnsureResponse { status: "error".into(),
                model: String::new(), endpoint: api.infer_endpoint(), load_ms: None,
                vram_estimate_gb: None, detail: Some(e) })),
    }
}

async fn h_ensure_stream(
    State(api): State<Arc<ControlApi>>, headers: HeaderMap, Query(req): Query<EnsureRequest>,
) -> axum::response::Response {
    let token_ok = match &api.control_token { None => true,
        Some(t) => req.token.as_deref() == Some(t.as_str()) };
    if !auth_ok(&api, &headers) && !token_ok {
        return (StatusCode::UNAUTHORIZED, "bad token").into_response();
    }
    let (ptx, prx) = mpsc::channel::<Stage>(16);
    let a = api.clone();
    tokio::spawn(async move {
        let outcome = a.ensure(req, &ProgressSink::Chan(ptx.clone())).await;
        let _ = ptx.send(Stage::from_outcome(outcome)).await;     // terminal frame
    });
    let stream = ReceiverStream::new(prx).map(|s| SseEvent::default().json_data(&s));
    Sse::new(stream).keep_alive(KeepAlive::default()).into_response()
}

async fn h_status(State(api): State<Arc<ControlApi>>) -> Json<StatusOut> {
    Json(api.status().await)
}
async fn h_models(State(api): State<Arc<ControlApi>>) -> Json<serde_json::Value> {
    // OpenAI-compatible model list, so any agent (Pi, OpenCode, ...) pointed at THIS
    // control API as its baseUrl can enumerate models. The ids are the router section
    // / profile names. Richer status (resident set, router url) lives at /v1/status.
    let data: Vec<serde_json::Value> = api.profiles.keys()
        .map(|id| serde_json::json!({ "id": id, "object": "model", "owned_by": "saltnitor" }))
        .collect();
    Json(serde_json::json!({ "object": "list", "data": data }))
}
async fn h_health() -> StatusCode { StatusCode::OK }

/// OpenAI-compatible POST /v1/chat/completions PROXY with an ensure-then-forward step.
/// Point an IDE/agent (Pi, OpenCode, Cline, ...) at THIS control API as its baseUrl,
/// and every request first makes the requested model resident (idempotent, oracle-
/// gated, evicting the incumbent) and is THEN forwarded to the router, with the reply
/// streamed straight back. This is the path where the agent literally "calls Saltnitor
/// to switch models" — one swap-orchestrating endpoint that works for any agent that
/// speaks the OpenAI chat API. (Agents that don't want the oracle can still point at
/// the router :8080 directly; the router's --models-max 1 auto-swaps on the model id.)
async fn h_chat(State(api): State<Arc<ControlApi>>, body: Bytes) -> axum::response::Response {
    // pull the model id (= a router section / profile name) out of the request body
    let model = serde_json::from_slice::<serde_json::Value>(&body).ok()
        .and_then(|v| v.get("model").and_then(|m| m.as_str()).map(str::to_string));
    let Some(model) = model else {
        return (StatusCode::BAD_REQUEST, "missing 'model' in request body").into_response();
    };

    // make it resident before forwarding. Idempotent: AlreadyResident is the fast path.
    match api.ensure(EnsureRequest { profile: model.clone(), ..Default::default() },
                     &ProgressSink::None).await {
        EnsureOutcome::Bad(e) =>
            return (StatusCode::NOT_FOUND, format!("unknown model '{}': {}", model, e)).into_response(),
        EnsureOutcome::Oom(i) =>
            return (StatusCode::SERVICE_UNAVAILABLE, format!(
                "saltnitor oracle: '{}' needs ~{:.1}GB VRAM (have {:.1}) / ~{:.1}GB RAM (have {:.1}); refusing to load (would OOM)",
                model, i.need_vram_gb, i.total_vram_gb, i.need_ram_gb, i.total_ram_gb)).into_response(),
        EnsureOutcome::Err(e) =>
            return (StatusCode::BAD_GATEWAY, format!("saltnitor: load failed: {}", e)).into_response(),
        _ => {} // Loaded | AlreadyResident -> proceed
    }

    // forward verbatim to the router; stream the (possibly SSE) response straight back
    let url = format!("{}/v1/chat/completions", api.router_base);
    let mut rb = api.http.post(&url).header("Content-Type", "application/json").body(body);
    if let Some(tok) = &api.infer_bearer {
        rb = rb.header("Authorization", format!("Bearer {}", tok));
    }
    match rb.send().await {
        Ok(resp) => {
            let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            let ctype = resp.headers().get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()).unwrap_or("application/json").to_string();
            match resp.bytes().await {
                Ok(bytes) => axum::response::Response::builder()
                    .status(status)
                    .header("Content-Type", ctype)
                    .body(Body::from(bytes))
                    .unwrap_or_else(|_| (StatusCode::BAD_GATEWAY, "proxy build error").into_response()),
                Err(e) => (StatusCode::BAD_GATEWAY, format!("router read failed: {}", e)).into_response(),
            }
        }
        Err(e) => (StatusCode::BAD_GATEWAY, format!("router unreachable: {}", e)).into_response(),
    }
}

/// Spawn from main.rs: `tokio::spawn(control_api::serve(api, addr));`
pub async fn serve(api: Arc<ControlApi>, addr: std::net::SocketAddr) {
    let app = Router::new()
        .route("/v1/ensure", post(h_ensure))
        .route("/v1/ensure/stream", get(h_ensure_stream))
        .route("/v1/status", get(h_status))
        .route("/v1/models", get(h_models))
        .route("/v1/chat/completions", post(h_chat))
        .route("/healthz", get(h_health))
        .with_state(api);
    match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => { let _ = axum::serve(l, app).await; }
        Err(e) => eprintln!("[control_api] bind {} failed: {}", addr, e),
    }
}

// ───────────────────────── helpers (footprint + telemetry) ─────────────────────────
fn basename(s: &str) -> String {
    std::path::Path::new(s).file_name().map(|x| x.to_string_lossy().into_owned()).unwrap_or_else(|| s.into())
}
fn round1(x: f64) -> f64 { (x * 10.0).round() / 10.0 }

fn estimate_footprint(p: &ProfileMeta) -> (f64, f64) {
    if let (Some(v), Some(r)) = (p.est_vram_gb, p.est_ram_gb) { return (v, r); }
    let params_b = parse_params_b(&p.model).unwrap_or(8.0);
    let bpw = parse_bpw(&p.model);
    let weights = params_b * bpw / 8.0;
    if p.offload {           // experts -> RAM; ~15% (attention/shared) stays in VRAM
        (round1(weights * 0.15 + p.est_vram_gb.unwrap_or(0.0).max(2.0)), round1(weights * 0.85))
    } else {
        (round1(weights + 1.5), 0.0)   // +~1.5 GB KV/compute slack for a fully-resident model
    }
}
fn parse_params_b(name: &str) -> Option<f64> {
    let up = name.to_uppercase().replace(['-', '_', '.'], " ");
    for w in up.split_whitespace() {
        if let Some(s) = w.strip_suffix('B') {
            if let Ok(n) = s.parse::<f64>() { if (0.3..2000.0).contains(&n) { return Some(n); } }
        }
    }
    None
}
fn parse_bpw(name: &str) -> f64 {
    let up = name.to_uppercase();
    if up.contains("Q2_K") { 2.6 } else if up.contains("Q3_K") || up.contains("IQ3") { 3.5 }
    else if up.contains("IQ4_XS") { 4.3 } else if up.contains("Q4_K") || up.contains("IQ4") { 4.85 }
    else if up.contains("Q5_K") || up.contains("Q5_") { 5.5 } else if up.contains("Q6_K") { 6.6 }
    else if up.contains("Q8_0") { 8.5 } else if up.contains("F16") || up.contains("BF16") { 16.0 }
    else { 5.0 }
}
async fn nvidia_query(field: &str) -> Option<f64> {
    let out = tokio::process::Command::new("nvidia-smi")
        .args([&format!("--query-gpu={}", field), "--format=csv,noheader,nounits"])
        .output().await.ok()?;
    if !out.status.success() { return None; }
    String::from_utf8_lossy(&out.stdout).lines().next()?.trim().parse::<f64>().ok()
}
async fn vram_total_gb() -> f64 { nvidia_query("memory.total").await.unwrap_or(1.0) / 1024.0 }
async fn vram_used_gb()  -> f64 { nvidia_query("memory.used").await.unwrap_or(0.0) / 1024.0 }
fn total_ram_gb() -> f64 { meminfo_kb("MemTotal:").map(|kb| kb / 1_048_576.0).unwrap_or(1.0) }
fn free_ram_gb()  -> f64 { meminfo_kb("MemAvailable:").map(|kb| kb / 1_048_576.0).unwrap_or(0.0) }
fn meminfo_kb(key: &str) -> Option<f64> {
    std::fs::read_to_string("/proc/meminfo").ok()?
        .lines().find(|l| l.starts_with(key))?
        .split_whitespace().nth(1)?.parse::<f64>().ok()
}
