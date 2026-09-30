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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn meta(model: &str, offload: bool, v: Option<f64>, r: Option<f64>) -> ProfileMeta {
        ProfileMeta {
            model: model.into(),
            offload,
            est_vram_gb: v,
            est_ram_gb: r,
        }
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[test]
    fn parse_params_b_reads_the_first_size_token() {
        for (name, want) in [
            ("Qwen3-30B-A3B-Q4_K_M.gguf", Some(30.0)),
            ("llama-2-7b-chat.Q8_0.gguf", Some(7.0)),
            ("gemma-3-270M.gguf", None),
            ("mistral.gguf", None),
            ("model-9000B.gguf", None),
        ] {
            assert_eq!(parse_params_b(name), want, "{name}");
        }
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[test]
    fn parse_bpw_maps_quant_names() {
        for (name, want) in [
            ("m-Q2_K.gguf", 2.6),
            ("m-Q3_K_M.gguf", 3.5),
            ("m-IQ3_XXS.gguf", 3.5),
            ("m-IQ4_XS.gguf", 4.3),
            ("m-Q4_K_M.gguf", 4.85),
            ("m-IQ4_NL.gguf", 4.85),
            ("m-Q5_K_M.gguf", 5.5),
            ("m-Q5_0.gguf", 5.5),
            ("m-Q6_K.gguf", 6.6),
            ("m-Q8_0.gguf", 8.5),
            ("m-F16.gguf", 16.0),
            ("m-BF16.gguf", 16.0),
            ("m.gguf", 5.0),
        ] {
            assert_eq!(parse_bpw(name), want, "{name}");
        }
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[test]
    fn estimate_footprint_overrides_and_heuristics() {
        assert_eq!(
            estimate_footprint(&meta("x-30B-Q4_K_M.gguf", true, Some(3.0), Some(4.0))),
            (3.0, 4.0)
        );
        assert_eq!(
            estimate_footprint(&meta("llama-7b-Q8_0.gguf", false, None, None)),
            (8.9, 0.0)
        );
        assert_eq!(
            estimate_footprint(&meta("Qwen3-30B-A3B-Q4_K_M.gguf", true, None, None)),
            (4.7, 15.5)
        );
        assert_eq!(
            estimate_footprint(&meta("Qwen3-30B-A3B-Q4_K_M.gguf", true, Some(3.0), None)),
            (5.7, 15.5)
        );
        assert_eq!(
            estimate_footprint(&meta("mystery.gguf", false, None, None)),
            (6.5, 0.0)
        );
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[test]
    fn stage_from_outcome_builds_the_terminal_frame() {
        let v = |o| serde_json::to_value(Stage::from_outcome(o)).unwrap();
        assert_eq!(
            v(EnsureOutcome::Loaded {
                model: "A".into(),
                endpoint: "http://x/v1".into(),
                load_ms: 1234,
                vram_estimate_gb: Some(4.7)
            }),
            json!({"stage": "done", "status": "loaded", "model": "A", "endpoint": "http://x/v1", "load_ms": 1234, "vram_estimate_gb": 4.7})
        );
        assert_eq!(
            v(EnsureOutcome::AlreadyResident {
                model: "A".into(),
                endpoint: "http://x/v1".into()
            }),
            json!({"stage": "done", "status": "already_resident", "model": "A", "endpoint": "http://x/v1", "load_ms": null, "vram_estimate_gb": null})
        );
        assert_eq!(
            v(EnsureOutcome::Oom(OomInfo {
                need_vram_gb: 1.0,
                total_vram_gb: 2.0,
                need_ram_gb: 3.0,
                total_ram_gb: 4.0
            })),
            json!({"stage": "oom", "detail": "need ~1.0GB VRAM (have 2.0) / ~3.0GB RAM (have 4.0); pass force=true"})
        );
        assert_eq!(
            v(EnsureOutcome::Bad("x".into())),
            json!({"stage": "error", "detail": "x"})
        );
        assert_eq!(
            v(EnsureOutcome::Err("y".into())),
            json!({"stage": "error", "detail": "y"})
        );
    }

    use fake_llama_server::{Fault, Scenario};
    use serde_json::Value;

    const SCENARIOS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/scenarios");

    fn scenario(name: &str) -> Scenario {
        Scenario::from_file(std::path::Path::new(&format!("{SCENARIOS}/{name}")))
            .expect("scenario fixture")
    }
    /// Zero estimates + zero reserves: the oracle passes on any host, GPU or not (design D1).
    fn fits() -> ProfileMeta {
        meta("m-7b-Q4_K_M.gguf", false, Some(0.0), Some(0.0))
    }
    /// An estimate no machine has: the oracle rejects on any host (design D1).
    fn never_fits() -> ProfileMeta {
        meta("m-7b-Q4_K_M.gguf", false, Some(1.0e6), Some(0.0))
    }

    struct Rig {
        base: String,
        fake: fake_llama_server::Handle,
        events: mpsc::Receiver<Event>,
        http: reqwest::Client,
    }

    impl Rig {
        fn url(&self, path: &str) -> String {
            format!("{}{}", self.base, path)
        }
        fn endpoint(&self) -> String {
            format!("{}/v1", self.fake.base_url())
        }
        fn upstream_chats(&self) -> Vec<Value> {
            self.fake
                .recorder
                .of_kind("request")
                .into_iter()
                .filter(|r| r["path"] == "/v1/chat/completions")
                .collect()
        }
        async fn post_json(&self, path: &str, body: Value) -> (u16, Value) {
            let r = self
                .http
                .post(self.url(path))
                .json(&body)
                .send()
                .await
                .unwrap();
            let s = r.status().as_u16();
            (s, r.json().await.unwrap_or(Value::Null))
        }
        async fn post_raw(&self, path: &str, body: &'static str) -> (u16, String, String) {
            let r = self
                .http
                .post(self.url(path))
                .header("Content-Type", "application/json")
                .body(body)
                .send()
                .await
                .unwrap();
            let s = r.status().as_u16();
            let ct = r
                .headers()
                .get("content-type")
                .map(|v| v.to_str().unwrap().to_string())
                .unwrap_or_default();
            (s, ct, r.text().await.unwrap())
        }
    }

    /// Real ControlApi + real `serve()` route table on a free loopback port, in front of the fake.
    async fn rig(scenario: Scenario, profiles: &[(&str, ProfileMeta)], token: Option<&str>) -> Rig {
        let fake = fake_llama_server::spawn(scenario).await;
        let (tx, events) = mpsc::channel(256);
        let profiles = profiles
            .iter()
            .map(|(k, p)| (k.to_string(), p.clone()))
            .collect();
        let api = Arc::new(ControlApi::new(
            profiles,
            fake.base_url(),
            None,
            token.map(str::to_string),
            0.0,
            0.0,
            tx,
        ));
        let http = reqwest::Client::new();
        for _ in 0..20 {
            let port = std::net::TcpListener::bind("127.0.0.1:0")
                .unwrap()
                .local_addr()
                .unwrap()
                .port();
            let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
            let task = tokio::spawn(serve(api.clone(), addr));
            let base = format!("http://{addr}");
            for _ in 0..50 {
                if let Ok(r) = http.get(format!("{base}/healthz")).send().await {
                    if r.status() == 200 {
                        return Rig {
                            base,
                            fake,
                            events,
                            http,
                        };
                    }
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            task.abort(); // port was taken between bind and serve; try another
        }
        panic!("control API did not come up");
    }

    fn stages(sse: &str) -> Vec<Value> {
        sse.lines()
            .filter_map(|l| l.strip_prefix("data: "))
            .map(|d| serde_json::from_str(d).unwrap())
            .collect()
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2, REQ-TST-002/AC1
    #[tokio::test]
    async fn healthz_is_200() {
        let r = rig(scenario("control-api-legacy.toml"), &[("A", fits())], None).await;
        assert_eq!(
            r.http.get(r.url("/healthz")).send().await.unwrap().status(),
            200
        );
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2, REQ-TST-002/AC1
    #[tokio::test]
    async fn pins_bd28_models_lists_every_configured_profile() {
        // Known defect BD-28 (baseline/DEFECTS.md): pinned so a fix shows up as a deliberate
        // snapshot change, not endorsed as correct (REQ-MIG-002/AC3).

        let r = rig(
            scenario("control-api-legacy.toml"),
            &[("A", fits()), ("B", fits())],
            None,
        )
        .await;
        let v: Value = r
            .http
            .get(r.url("/v1/models"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(v["object"], "list");
        let mut ids: Vec<&str> = v["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["id"].as_str().unwrap())
            .collect();
        ids.sort();
        assert_eq!(ids, ["A", "B"]);
        assert!(
            v["data"]
                .as_array()
                .unwrap()
                .iter()
                .all(|m| m["object"] == "model" && m["owned_by"] == "saltnitor")
        );
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[tokio::test]
    async fn status_reports_resident_models_and_numeric_telemetry() {
        let r = rig(
            scenario("control-api-legacy.toml"),
            &[("A", fits()), ("B", fits())],
            None,
        )
        .await;
        let v: Value = r
            .http
            .get(r.url("/v1/status"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(v["resident_models"], json!(["A"]));
        assert_eq!(v["endpoint"], json!(r.endpoint()));
        let mut profiles: Vec<&str> = v["profiles"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_str().unwrap())
            .collect();
        profiles.sort();
        assert_eq!(profiles, ["A", "B"]);
        for k in ["vram_used_gb", "vram_total_gb", "ram_free_gb"] {
            assert!(v[k].is_f64(), "{k} = {}", v[k]);
        }
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2, REQ-MIG-007/AC2
    #[tokio::test]
    async fn ensure_already_resident_skips_the_warm_load() {
        let r = rig(scenario("control-api-legacy.toml"), &[("A", fits())], None).await;
        let (s, v) = r.post_json("/v1/ensure", json!({"profile": "A"})).await;
        assert_eq!(
            (s, v),
            (
                200,
                json!({"status": "already_resident", "model": "A", "endpoint": r.endpoint()})
            )
        );
        assert!(r.upstream_chats().is_empty());
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2, REQ-MIG-007/AC2
    #[tokio::test]
    async fn ensure_loads_with_a_one_token_warm_request() {
        let mut r = rig(
            scenario("control-api-legacy.toml"),
            &[("A", fits()), ("B", fits())],
            None,
        )
        .await;
        let (s, v) = r.post_json("/v1/ensure", json!({"profile": "B"})).await;
        assert_eq!(s, 200);
        assert_eq!(
            (v["status"].as_str(), v["model"].as_str()),
            (Some("loaded"), Some("B"))
        );
        assert_eq!(v["endpoint"], json!(r.endpoint()));
        assert!(v["load_ms"].is_u64());
        assert_eq!(v["vram_estimate_gb"], json!(0.0));
        assert!(v.get("detail").is_none());
        let chats = r.upstream_chats();
        assert_eq!(chats.len(), 1);
        let warm: Value = serde_json::from_str(chats[0]["body"].as_str().unwrap()).unwrap();
        assert_eq!(
            warm,
            json!({"model": "B", "messages": [{"role": "user", "content": "warmup"}], "max_tokens": 1})
        );
        assert_eq!(r.fake.loaded(), vec!["B"]);
        let mut active = vec![];
        while let Ok(e) = r.events.try_recv() {
            if let Event::ActiveModelSet(m) = e {
                active.push(m);
            }
        }
        assert_eq!(active, ["B"]);
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[tokio::test]
    async fn ensure_unknown_profile_is_400() {
        let r = rig(scenario("control-api-legacy.toml"), &[("A", fits())], None).await;
        let (s, v) = r.post_json("/v1/ensure", json!({"profile": "Z"})).await;
        assert_eq!(
            (s, v),
            (
                400,
                json!({"status": "bad_request", "model": "", "endpoint": r.endpoint(), "detail": "unknown profile 'Z'"})
            )
        );
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2, REQ-MIG-007/AC2
    #[tokio::test]
    async fn pins_bd29_ensure_oracle_reject_is_507_json() {
        // Known defect BD-29 (507 here vs 503 on chat) (baseline/DEFECTS.md): pinned so a fix shows up as a deliberate
        // snapshot change, not endorsed as correct (REQ-MIG-002/AC3).

        let r = rig(
            scenario("control-api-legacy.toml"),
            &[("B", never_fits())],
            None,
        )
        .await;
        let (s, v) = r.post_json("/v1/ensure", json!({"profile": "B"})).await;
        assert_eq!(s, 507);
        assert_eq!(
            (v["status"].as_str(), v["model"].as_str()),
            (Some("oom_rejected"), Some(""))
        );
        assert_eq!(v["vram_estimate_gb"], json!(1.0e6));
        assert!(
            v["detail"]
                .as_str()
                .unwrap()
                .starts_with("need ~1000000.0GB VRAM (have "),
            "{v}"
        );
        assert!(r.upstream_chats().is_empty());
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[tokio::test]
    async fn ensure_force_bypasses_the_oracle() {
        let r = rig(
            scenario("control-api-legacy.toml"),
            &[("B", never_fits())],
            None,
        )
        .await;
        let (s, v) = r
            .post_json("/v1/ensure", json!({"profile": "B", "force": true}))
            .await;
        assert_eq!((s, v["status"].as_str()), (200, Some("loaded")));
        assert_eq!(v["vram_estimate_gb"], json!(1.0e6));
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2, REQ-TST-002/AC1
    #[tokio::test]
    async fn ensure_reports_router_failure_as_503() {
        let sc = scenario("control-api-legacy.toml").with_fault(
            "POST /v1/chat/completions",
            Fault::Status {
                code: 500,
                body: "boom".into(),
            },
        );
        let r = rig(sc, &[("B", fits())], None).await;
        let (s, v) = r.post_json("/v1/ensure", json!({"profile": "B"})).await;
        assert_eq!(s, 503);
        assert_eq!(v["status"], "error");
        assert_eq!(
            v["detail"],
            "router load failed: router returned 500 Internal Server Error"
        );
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2, REQ-TST-002/AC1
    #[tokio::test]
    async fn ensure_requires_the_bearer_when_a_token_is_set() {
        let r = rig(
            scenario("control-api-legacy.toml"),
            &[("A", fits())],
            Some("t0k"),
        )
        .await;
        let (s, v) = r.post_json("/v1/ensure", json!({"profile": "A"})).await;
        assert_eq!(
            (s, v),
            (
                401,
                json!({"status": "error", "model": "", "endpoint": r.endpoint(), "detail": "bad token"})
            )
        );
        let ok = r
            .http
            .post(r.url("/v1/ensure"))
            .bearer_auth("t0k")
            .json(&json!({"profile": "A"}))
            .send()
            .await
            .unwrap();
        assert_eq!(ok.status(), 200);
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[tokio::test]
    async fn ensure_stream_emits_stages_in_order() {
        let r = rig(
            scenario("control-api-legacy.toml"),
            &[("A", fits()), ("B", fits())],
            None,
        )
        .await;
        let text = r
            .http
            .get(r.url("/v1/ensure/stream?profile=B"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        let st = stages(&text);
        let names: Vec<&str> = st.iter().map(|s| s["stage"].as_str().unwrap()).collect();
        assert_eq!(names, ["received", "oracle_ok", "loading", "done"]);
        assert_eq!(
            st[0],
            json!({"stage": "received", "profile": "B", "model": "B"})
        );
        assert_eq!(st[3]["status"], "loaded");
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[tokio::test]
    async fn ensure_stream_rejects_a_wrong_query_token() {
        let r = rig(
            scenario("control-api-legacy.toml"),
            &[("A", fits())],
            Some("t0k"),
        )
        .await;
        let bad = r
            .http
            .get(r.url("/v1/ensure/stream?profile=A&token=nope"))
            .send()
            .await
            .unwrap();
        assert_eq!(
            (bad.status().as_u16(), bad.text().await.unwrap()),
            (401, "bad token".to_string())
        );
    }

    /// BD-04 evidence probe — the query token is always accepted; measured, not asserted (REQ-MIG-002/AC3).
    #[tokio::test]
    #[ignore = "BD-04 evidence probe: cargo test bd04_query_token_is_always_accepted -- --ignored --nocapture"]
    async fn bd04_query_token_is_always_accepted() {
        let r = rig(
            scenario("control-api-legacy.toml"),
            &[("A", fits())],
            Some("t0k"),
        )
        .await;
        let ok = r
            .http
            .get(r.url("/v1/ensure/stream?profile=A&token=t0k"))
            .send()
            .await
            .unwrap();
        let status = ok.status().as_u16();
        let last = stages(&ok.text().await.unwrap()).last().cloned();
        println!("BD-04: ?token= on /v1/ensure/stream → HTTP {status}, last stage {last:?}");
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2, REQ-MIG-007/AC1, REQ-TST-002/AC1
    #[tokio::test]
    async fn chat_forwards_the_body_unchanged_after_ensure() {
        let r = rig(scenario("control-api-legacy.toml"), &[("A", fits())], None).await;
        const BODY: &str = r#"{"model":"A","messages":[{"role":"user","content":"hi"}]}"#;
        let (s, _, text) = r.post_raw("/v1/chat/completions", BODY).await;
        assert_eq!(s, 200);
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["choices"][0]["message"]["content"], "Hello from fake");
        let chats = r.upstream_chats();
        assert_eq!(chats.len(), 1);
        assert_eq!(chats[0]["body"], BODY);
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2, REQ-MIG-007/AC1
    #[tokio::test]
    async fn chat_hot_swaps_a_non_resident_model_first() {
        let r = rig(
            scenario("control-api-legacy.toml"),
            &[("A", fits()), ("B", fits())],
            None,
        )
        .await;
        const BODY: &str = r#"{"model":"B","messages":[{"role":"user","content":"hi"}]}"#;
        assert_eq!(r.post_raw("/v1/chat/completions", BODY).await.0, 200);
        let chats = r.upstream_chats();
        assert_eq!(chats.len(), 2);
        let warm: Value = serde_json::from_str(chats[0]["body"].as_str().unwrap()).unwrap();
        assert_eq!(warm["max_tokens"], 1);
        assert_eq!(chats[1]["body"], BODY);
        assert_eq!(r.fake.loaded(), vec!["B"]);
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[tokio::test]
    async fn pins_bd29_chat_errors_are_plain_text() {
        // Known defect BD-29 (plain-text chat errors) (baseline/DEFECTS.md): pinned so a fix shows up as a deliberate
        // snapshot change, not endorsed as correct (REQ-MIG-002/AC3).

        let r = rig(scenario("control-api-legacy.toml"), &[("A", fits())], None).await;
        let (s, _, text) = r
            .post_raw("/v1/chat/completions", r#"{"messages":[]}"#)
            .await;
        assert_eq!((s, text.as_str()), (400, "missing 'model' in request body"));
        let (s, _, text) = r
            .post_raw("/v1/chat/completions", r#"{"model":"Z","messages":[]}"#)
            .await;
        assert_eq!(
            (s, text.as_str()),
            (404, "unknown model 'Z': unknown profile 'Z'")
        );
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2, REQ-TST-002/AC1
    #[tokio::test]
    async fn chat_passes_upstream_5xx_through() {
        let sc = scenario("control-api-legacy.toml").with_fault(
            "POST /v1/chat/completions",
            Fault::Status {
                code: 503,
                body: r#"{"error":"busy"}"#.into(),
            },
        );
        let r = rig(sc, &[("A", fits())], None).await;
        let (s, ct, text) = r
            .post_raw("/v1/chat/completions", r#"{"model":"A","messages":[]}"#)
            .await;
        assert_eq!(
            (s, ct.as_str(), text.as_str()),
            (503, "application/json", r#"{"error":"busy"}"#)
        );
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[tokio::test]
    async fn pins_bd29_chat_oracle_reject_is_503_plain_text() {
        // Known defect BD-29 (503 here vs 507 on ensure) (baseline/DEFECTS.md): pinned so a fix shows up as a deliberate
        // snapshot change, not endorsed as correct (REQ-MIG-002/AC3).

        let r = rig(
            scenario("control-api-legacy.toml"),
            &[("B", never_fits())],
            None,
        )
        .await;
        let (s, _, text) = r
            .post_raw("/v1/chat/completions", r#"{"model":"B","messages":[]}"#)
            .await;
        assert_eq!(s, 503);
        assert!(
            text.starts_with("saltnitor oracle: 'B' needs ~1000000.0GB VRAM"),
            "{text}"
        );
    }

    /// BD-02 evidence probe — measures, asserts nothing about buffering (REQ-MIG-002/AC3).
    #[tokio::test]
    #[ignore = "BD-02 evidence probe: cargo test bd02_first_byte_timing -- --ignored --nocapture"]
    async fn bd02_first_byte_timing() {
        async fn first_byte(http: &reqwest::Client, url: String) -> (u16, u128, u128) {
            let t0 = std::time::Instant::now();
            let mut resp = http
                .post(url)
                .header("Content-Type", "application/json")
                .body(r#"{"model":"A","stream":true,"messages":[]}"#)
                .send()
                .await
                .unwrap();
            let status = resp.status().as_u16();
            let mut first = None;
            while let Some(_c) = resp.chunk().await.unwrap() {
                first.get_or_insert(t0.elapsed().as_millis());
            }
            (status, first.unwrap_or(0), t0.elapsed().as_millis())
        }
        let r = rig(scenario("bd02-slow-stream.toml"), &[("A", fits())], None).await;
        let direct = first_byte(
            &r.http,
            format!("{}/v1/chat/completions", r.fake.base_url()),
        )
        .await;
        let via = first_byte(&r.http, r.url("/v1/chat/completions")).await;
        println!(
            "BD-02 direct: status={} first_byte_ms={} total_ms={}",
            direct.0, direct.1, direct.2
        );
        println!(
            "BD-02 via saltnitor: status={} first_byte_ms={} total_ms={}",
            via.0, via.1, via.2
        );
        assert_eq!((direct.0, via.0), (200, 200));
    }

    /// BD-32 evidence probe — shows what status reports for the current upstream shape.
    #[tokio::test]
    #[ignore = "BD-32 evidence probe: cargo test bd32_status_object_not_recognised -- --ignored --nocapture"]
    async fn bd32_status_object_not_recognised() {
        let sc = Scenario::default().with_model("A", true); // default = status objects
        let r = rig(sc, &[("A", fits())], None).await;
        let v: Value = r
            .http
            .get(r.url("/v1/status"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        println!(
            "BD-32: fake reports A loaded (status object); saltnitor resident_models = {}",
            v["resident_models"]
        );
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2, REQ-TST-002/AC1
    #[tokio::test]
    async fn pins_bd29_chat_load_failure_is_502_plain_text() {
        // Known defect BD-29 (plain-text chat errors): pinned so a fix shows up as a deliberate
        // change, not endorsed as correct (REQ-MIG-002/AC3).
        let sc = scenario("control-api-legacy.toml").with_fault(
            "POST /v1/chat/completions",
            Fault::Status {
                code: 500,
                body: "boom".into(),
            },
        );
        let r = rig(sc, &[("A", fits()), ("B", fits())], None).await;
        let (s, ct, text) = r
            .post_raw("/v1/chat/completions", r#"{"model":"B","messages":[]}"#)
            .await;
        assert_eq!(
            (s, ct.as_str(), text.as_str()),
            (
                502,
                "text/plain; charset=utf-8",
                "saltnitor: load failed: router load failed: router returned 500 Internal Server Error"
            )
        );
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2, REQ-TST-002/AC1
    #[tokio::test]
    async fn pins_bd29_chat_upstream_body_failure_is_502_plain_text() {
        // Known defect BD-29 (plain-text chat errors): pinned, not endorsed (REQ-MIG-002/AC3).
        let sc = scenario("control-api-legacy.toml")
            .with_fault("POST /v1/chat/completions", Fault::CrashAfter { chunks: 0 });
        let r = rig(sc, &[("A", fits())], None).await;
        let (s, _, text) = r
            .post_raw("/v1/chat/completions", r#"{"model":"A","messages":[]}"#)
            .await;
        assert_eq!(s, 502);
        assert!(text.starts_with("router read failed: "), "{text}");
    }
}
