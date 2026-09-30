//! Event recorder: argv, env, requests, disconnects, crashes — in memory and optionally JSONL.
//! Never stores the Authorization header value.
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use axum::http::{HeaderMap, header};
use serde_json::{Map, Value, json};

#[derive(Clone)]
pub struct Recorder {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    t0: Instant,
    events: Vec<Value>,
    file: Option<std::fs::File>,
}

impl Default for Recorder {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                t0: Instant::now(),
                events: Vec::new(),
                file: None,
            })),
        }
    }
}

impl Recorder {
    /// Also append every event as one JSON line to `path`.
    pub fn to_file(path: &Path) -> std::io::Result<Self> {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        let r = Self::default();
        r.inner.lock().expect("recorder lock").file = Some(file);
        Ok(r)
    }

    /// Record `data` (an object) with `kind` and milliseconds since start (`t_ms`).
    pub fn record(&self, kind: &str, mut data: Value) {
        let mut g = self.inner.lock().expect("recorder lock");
        let t_ms = g.t0.elapsed().as_millis() as u64;
        if let Value::Object(m) = &mut data {
            m.insert("kind".into(), json!(kind));
            m.insert("t_ms".into(), json!(t_ms));
        }
        if let Some(f) = g.file.as_mut() {
            let _ = writeln!(f, "{data}");
            let _ = f.flush();
        }
        g.events.push(data);
    }

    pub(crate) fn request(
        &self,
        method: &str,
        path: &str,
        query: &str,
        headers: &HeaderMap,
        body: &[u8],
    ) {
        let kept: Map<String, Value> = headers
            .iter()
            .filter(|(k, _)| *k != header::AUTHORIZATION)
            .map(|(k, v)| (k.to_string(), json!(v.to_str().unwrap_or(""))))
            .collect();
        self.record(
            "request",
            json!({
                "method": method, "path": path, "query": query, "headers": kept,
                "has_authorization": headers.contains_key(header::AUTHORIZATION),
                "body": String::from_utf8_lossy(body),
            }),
        );
    }

    pub(crate) fn disconnect(&self, path: &str) {
        self.record("disconnect", json!({ "path": path }));
    }

    pub fn events(&self) -> Vec<Value> {
        self.inner.lock().expect("recorder lock").events.clone()
    }

    pub fn of_kind(&self, kind: &str) -> Vec<Value> {
        self.events()
            .into_iter()
            .filter(|e| e["kind"] == kind)
            .collect()
    }
}
