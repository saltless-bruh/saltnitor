//! The fake llama-server HTTP surface: one fallback handler dispatching on (method, path).
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::response::Response;
use serde_json::{Value, json};

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
        _ => not_found(),
    }
}

async fn apply_fault(
    st: &Arc<Shared>,
    fault: Fault,
    _method: &str,
    _path: &str,
    _json: &Value,
) -> Response {
    match fault {
        Fault::Status { code, body } => {
            let ct = if body.trim_start().starts_with('{') {
                "application/json"
            } else {
                "text/plain"
            };
            text_resp(code, ct, body)
        }
        other => {
            // Streaming faults are implemented in Task 5; until then they are an explicit 501,
            // and no test in this task depends on them.
            let _ = st;
            text_resp(
                501,
                "text/plain",
                format!("fault not implemented yet: {other:?}"),
            )
        }
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
