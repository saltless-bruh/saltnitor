//! The fake llama-server HTTP surface: one fallback handler dispatching on (method, path).
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::response::Response;
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::recorder::Recorder;
use crate::scenario::{Fault, ModelsShape, Scenario};

/// What a `CrashAfter` fault does once its chunks are sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrashMode {
    /// In-process: abort the HTTP connection.
    DropConnection,
    /// Binary: exit the process with status 101.
    ExitProcess,
}

pub(crate) struct Shared {
    pub(crate) scenario: Scenario,
    pub(crate) loaded: Mutex<Vec<String>>,
    pub(crate) recorder: Recorder,
    pub(crate) crash: CrashMode,
}

impl Shared {
    pub(crate) fn new(scenario: Scenario, recorder: Recorder, crash: CrashMode) -> Arc<Self> {
        let loaded = scenario
            .models
            .iter()
            .filter(|m| m.loaded)
            .map(|m| m.id.clone())
            .collect();
        Arc::new(Self {
            scenario,
            loaded: Mutex::new(loaded),
            recorder,
            crash,
        })
    }

    pub(crate) fn load(&self, id: &str) {
        let mut l = self.loaded.lock().expect("loaded lock");
        if l.iter().any(|m| m == id) {
            return;
        }
        l.push(id.to_string());
        if let Some(max) = self.scenario.max_loaded {
            while l.len() > max {
                l.remove(0);
            }
        }
    }

    fn unload(&self, id: &str) {
        self.loaded.lock().expect("loaded lock").retain(|m| m != id);
    }

    pub(crate) fn known(&self, id: &str) -> bool {
        self.scenario.models.is_empty() || self.scenario.models.iter().any(|m| m.id == id)
    }
}

pub(crate) fn router(shared: Arc<Shared>) -> Router {
    Router::new().fallback(handle).with_state(shared)
}

async fn handle(State(st): State<Arc<Shared>>, req: Request) -> Response {
    let method = req.method().as_str().to_string();
    let path = req.uri().path().to_string();
    let query = req.uri().query().unwrap_or("").to_string();
    let headers = req.headers().clone();
    let body = axum::body::to_bytes(req.into_body(), usize::MAX)
        .await
        .unwrap_or_default();
    st.recorder.request(&method, &path, &query, &headers, &body);
    let json: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);

    if let Some(fault) = st.scenario.routes.get(&format!("{method} {path}")).cloned() {
        return apply_fault(&st, fault, &method, &path, &json).await;
    }
    if method == "GET"
        && let Some(resp) = replay(&st, &path)
    {
        return resp;
    }
    match (method.as_str(), path.as_str()) {
        ("GET", "/health") => json_resp(200, json!({"status": "ok"})),
        ("GET", "/models") | ("GET", "/v1/models") => json_resp(200, models_body(&st)),
        ("POST", "/models/load") => load_unload(&st, &json, true),
        ("POST", "/models/unload") => load_unload(&st, &json, false),
        ("GET", "/metrics") => text_resp(200, "text/plain; version=0.0.4", metrics_body(&st)),
        ("GET", "/slots") => json_resp(200, json!([{"id": 0, "is_processing": false}])),
        ("POST", "/v1/chat/completions") => chat(&st, &json),
        _ => not_found(),
    }
}

async fn apply_fault(
    st: &Arc<Shared>,
    fault: Fault,
    method: &str,
    path: &str,
    json: &Value,
) -> Response {
    let is_chat = method == "POST" && path == "/v1/chat/completions";
    let stream = json["stream"].as_bool().unwrap_or(false);
    let model = json["model"].as_str().unwrap_or("fake").to_string();
    match fault {
        Fault::Status { code, body } => {
            let ct = if body.trim_start().starts_with('{') {
                "application/json"
            } else {
                "text/plain"
            };
            text_resp(code, ct, body)
        }
        Fault::HangBeforeHeaders => std::future::pending::<Response>().await,
        Fault::Chunks { items, delay_ms } if is_chat && stream => {
            st.load(&model);
            stream_body(
                st.clone(),
                path,
                sse_pieces(&model, &items, true),
                delay_ms,
                None,
                "text/event-stream",
            )
        }
        Fault::Chunks { items, delay_ms } if is_chat => {
            st.load(&model);
            tokio::time::sleep(Duration::from_millis(delay_ms * items.len() as u64)).await;
            json_resp(200, completion(&model, &items.concat()))
        }
        Fault::Chunks { items, delay_ms } => stream_body(
            st.clone(),
            path,
            items,
            delay_ms,
            None,
            "application/octet-stream",
        ),
        Fault::CrashAfter { chunks } => {
            let items = chat_items(st);
            let (pieces, ct) = if is_chat {
                (sse_pieces(&model, &items, false), "text/event-stream")
            } else {
                (items, "application/octet-stream")
            };
            stream_body(st.clone(), path, pieces, 0, Some(chunks), ct)
        }
        Fault::Malformed if is_chat && stream => text_resp(
            200,
            "text/event-stream",
            "data: {\"choices\":[{\"delta\":{\"content\":\"x\"\n\n".into(),
        ),
        Fault::Malformed => text_resp(
            200,
            "application/json",
            "{\"object\":\"list\",\"data\":[".into(),
        ),
    }
}

fn chat(st: &Arc<Shared>, body: &Value) -> Response {
    let Some(model) = body["model"].as_str() else {
        return json_resp(
            400,
            error_body(400, "missing 'model'", "invalid_request_error"),
        );
    };
    if !st.known(model) {
        return json_resp(
            400,
            error_body(
                400,
                &format!("model '{model}' not found"),
                "invalid_request_error",
            ),
        );
    }
    st.load(model);
    let items = chat_items(st);
    if body["stream"].as_bool().unwrap_or(false) {
        stream_body(
            st.clone(),
            "/v1/chat/completions",
            sse_pieces(model, &items, true),
            0,
            None,
            "text/event-stream",
        )
    } else {
        json_resp(200, completion(model, &items.concat()))
    }
}

fn chat_items(st: &Shared) -> Vec<String> {
    st.scenario
        .chat_chunks
        .clone()
        .unwrap_or_else(|| vec!["Hello".into(), " from".into(), " fake".into()])
}

fn completion(model: &str, content: &str) -> Value {
    json!({
        "id": "chatcmpl-fake", "object": "chat.completion", "model": model,
        "choices": [{"index": 0, "message": {"role": "assistant", "content": content}, "finish_reason": "stop"}],
        "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
    })
}

fn sse_pieces(model: &str, items: &[String], done: bool) -> Vec<String> {
    let mut out: Vec<String> = items
        .iter()
        .map(|c| {
            let ev = json!({
                "id": "chatcmpl-fake", "object": "chat.completion.chunk", "model": model,
                "choices": [{"index": 0, "delta": {"content": c}, "finish_reason": null}]
            });
            format!("data: {ev}\n\n")
        })
        .collect();
    if done {
        out.push("data: [DONE]\n\n".into());
    }
    out
}

/// Stream `pieces` (sleeping `delay_ms` before each); crash after `crash_after` pieces.
/// A failed send means the client went away: record a disconnect and stop.
fn stream_body(
    st: Arc<Shared>,
    path: &str,
    pieces: Vec<String>,
    delay_ms: u64,
    crash_after: Option<usize>,
    content_type: &str,
) -> Response {
    let (tx, rx) = mpsc::channel::<Result<Bytes, std::io::Error>>(1);
    let path = path.to_string();
    tokio::spawn(async move {
        for (i, piece) in pieces.into_iter().enumerate() {
            if crash_after == Some(i) {
                return crash(&st, &tx).await;
            }
            if delay_ms > 0 {
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }
            if tx.send(Ok(Bytes::from(piece))).await.is_err() {
                st.recorder.disconnect(&path);
                return;
            }
        }
        if crash_after.is_some() {
            crash(&st, &tx).await;
        }
    });
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .body(Body::from_stream(ReceiverStream::new(rx)))
        .expect("valid streaming response")
}

async fn crash(st: &Shared, tx: &mpsc::Sender<Result<Bytes, std::io::Error>>) {
    st.recorder.record("crash", json!({}));
    tokio::time::sleep(Duration::from_millis(50)).await; // let already-sent pieces flush
    match st.crash {
        CrashMode::DropConnection => {
            let _ = tx
                .send(Err(std::io::Error::other(
                    "fake-llama-server: simulated crash",
                )))
                .await;
        }
        CrashMode::ExitProcess => std::process::exit(101),
    }
}

fn replay(st: &Shared, path: &str) -> Option<Response> {
    let dir = st.scenario.replay_from.as_ref()?;
    let (status, ct, bytes) = crate::record::replay_file(dir, path)?;
    Some(
        Response::builder()
            .status(StatusCode::from_u16(status).unwrap_or(StatusCode::OK))
            .header(header::CONTENT_TYPE, ct)
            .body(Body::from(bytes))
            .expect("valid replay response"),
    )
}

fn models_body(st: &Shared) -> Value {
    let loaded = st.loaded.lock().expect("loaded lock").clone();
    let data: Vec<Value> = st
        .scenario
        .models
        .iter()
        .map(|m| {
            let state = if loaded.contains(&m.id) {
                "loaded"
            } else {
                "unloaded"
            };
            let status = match st.scenario.models_shape {
                ModelsShape::StatusObject => json!({ "value": state }),
                ModelsShape::Legacy => json!(state),
            };
            json!({"id": m.id, "object": "model", "owned_by": "llamacpp", "status": status})
        })
        .collect();
    json!({"object": "list", "data": data})
}

fn load_unload(st: &Shared, body: &Value, load: bool) -> Response {
    let Some(id) = body["model"].as_str() else {
        return json_resp(
            400,
            error_body(400, "missing 'model'", "invalid_request_error"),
        );
    };
    if !st.known(id) {
        return json_resp(
            404,
            error_body(404, &format!("model '{id}' not found"), "not_found_error"),
        );
    }
    if load {
        st.load(id)
    } else {
        st.unload(id)
    }
    json_resp(200, json!({"success": true}))
}

fn metrics_body(st: &Shared) -> String {
    let n = st.loaded.lock().expect("loaded lock").len();
    format!(
        "# HELP llamacpp:requests_processing Number of requests processing.\n\
         # TYPE llamacpp:requests_processing gauge\n\
         llamacpp:requests_processing 0\n\
         llamacpp:models_loaded {n}\n"
    )
}

pub(crate) fn error_body(code: u16, message: &str, kind: &str) -> Value {
    json!({"error": {"code": code, "message": message, "type": kind}})
}

fn not_found() -> Response {
    json_resp(404, error_body(404, "File Not Found", "not_found_error"))
}

pub(crate) fn json_resp(code: u16, v: Value) -> Response {
    text_resp(code, "application/json", v.to_string())
}

pub(crate) fn text_resp(code: u16, content_type: &str, body: String) -> Response {
    Response::builder()
        .status(StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        .header(header::CONTENT_TYPE, content_type)
        .body(Body::from(body))
        .expect("valid response parts")
}
