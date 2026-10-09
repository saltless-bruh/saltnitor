//! Interrogator metrics from an SSE transcript (REQ-TUI-006, REQ-TUI-007). Measured numbers come
//! only from the runtime's `timings`; otherwise the value is an estimate labeled `est.` or `n/a`.
use crate::events::Event;
use std::path::PathBuf;
use std::time::Instant;
use tokio::sync::mpsc::Sender;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Source {
    #[default]
    Unavailable,
    Estimated,
    Measured,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Metrics {
    pub ttft_ms: Option<u128>,
    pub pp_tps: Option<f64>,
    pub tg_tps: Option<f64>,
    pub source: Source,
    pub content_chunks: usize,
}

/// `timings.{prompt_n, prompt_ms, predicted_n, predicted_ms}` → (PP t/s, TG t/s).
pub fn parse_timings(chunk: &serde_json::Value) -> Option<(f64, f64)> {
    let t = chunk.get("timings")?;
    let rate = |n: &str, ms: &str| -> Option<f64> {
        let n = t.get(n)?.as_f64()?;
        let ms = t.get(ms)?.as_f64()?;
        (ms > 0.0).then(|| n / ms * 1000.0)
    };
    Some((
        rate("prompt_n", "prompt_ms")?,
        rate("predicted_n", "predicted_ms")?,
    ))
}

pub fn metrics_from_transcript(
    transcript: &str,
    ttft_ms: Option<u128>,
    gen_elapsed_ms: Option<u128>,
) -> Metrics {
    let mut m = Metrics {
        ttft_ms,
        ..Default::default()
    };
    let mut measured = None;
    for data in transcript
        .lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter(|d| *d != "[DONE]")
    {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(data) else {
            continue;
        };
        if v["choices"][0]["delta"]
            .get("content")
            .and_then(|c| c.as_str())
            .is_some()
        {
            m.content_chunks += 1;
        }
        if let Some(t) = parse_timings(&v) {
            measured = Some(t);
        }
    }
    match (measured, gen_elapsed_ms) {
        (Some((pp, tg)), _) => {
            m.pp_tps = Some(pp);
            m.tg_tps = Some(tg);
            m.source = Source::Measured;
        }
        (None, Some(ms)) if ms > 0 && m.content_chunks > 0 => {
            m.tg_tps = Some(m.content_chunks as f64 / ms as f64 * 1000.0);
            m.source = Source::Estimated;
        }
        _ => {}
    }
    m
}

/// Display strings for the deck: never `0` for an unmeasured value (INV-18).
pub fn render(m: &Metrics) -> (String, String, String) {
    let ttft = m
        .ttft_ms
        .map_or("TTFT: n/a".to_string(), |t| format!("TTFT: {t}ms"));
    let suffix = if m.source == Source::Estimated {
        " (est.)"
    } else {
        ""
    };
    let pp = m
        .pp_tps
        .map_or("PP: n/a".to_string(), |v| format!("PP: {v:.1} t/s{suffix}"));
    let tg = m
        .tg_tps
        .map_or("TG: n/a".to_string(), |v| format!("TG: {v:.1} t/s{suffix}"));
    (ttft, pp, tg)
}

/// `$XDG_STATE_HOME/saltnitor/history`, else `~/.local/state/saltnitor/history` (REQ-TUI-007/AC3).
pub fn history_path(env: &dyn Fn(&str) -> Option<String>) -> PathBuf {
    match env("XDG_STATE_HOME").filter(|s| !s.is_empty()) {
        Some(s) => PathBuf::from(s).join("saltnitor/history"),
        None => {
            PathBuf::from(env("HOME").unwrap_or_default()).join(".local/state/saltnitor/history")
        }
    }
}

/// Write the console history, creating its directory. `Err` is the operator-facing reason
/// (REQ-ERR-005/AC1): a history that cannot be saved must not vanish silently.
pub fn save_history(path: &std::path::Path, content: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("cannot create history directory {}: {e}", dir.display()))?;
    }
    std::fs::write(path, content)
        .map_err(|e| format!("cannot write history {}: {e}", path.display()))
}

/// The strike failed before or instead of a normal answer: tell the operator why (an
/// `Event::Error` for the status line and log) and close the deck with `status` (INV-18).
async fn fail(tx: &Sender<Event>, message: String, status: &str) {
    let _ = tx
        .send(Event::Error {
            source: "interrogator".into(),
            message,
        })
        .await;
    let _ = tx
        .send(Event::ApiStreamEnd {
            metrics: Metrics::default(),
            status: status.to_string(),
        })
        .await;
}

/// `error.code: error.message` from an error envelope, else the first part of the raw body.
fn describe_error_body(status: reqwest::StatusCode, body: &[u8]) -> String {
    let envelope = serde_json::from_slice::<serde_json::Value>(body).ok();
    let field = |k: &str| {
        envelope
            .as_ref()
            .and_then(|v| v["error"][k].as_str())
            .map(str::to_string)
    };
    match (field("code"), field("message")) {
        (Some(code), Some(message)) => format!("{status}: {code}: {message}"),
        (None, Some(message)) => format!("{status}: {message}"),
        _ => {
            let text = String::from_utf8_lossy(body);
            let text = text.trim();
            if text.is_empty() {
                format!("{status} with an empty body")
            } else {
                format!("{status}: {}", text.chars().take(200).collect::<String>())
            }
        }
    }
}

/// Fire one streamed request at Saltnitor's own endpoint with the client key (REQ-TUI-007/AC1),
/// forwarding tokens as events. TTFT is measured client-side at the first body chunk.
pub async fn strike(control_port: u16, bearer: Option<String>, payload: String, tx: Sender<Event>) {
    let start = Instant::now();
    let url = format!("http://127.0.0.1:{control_port}/v1/chat/completions");
    let mut body: serde_json::Value = match serde_json::from_str(&payload) {
        Ok(v) => v,
        Err(e) => {
            fail(&tx, format!("payload is not valid JSON: {e}"), "n/a").await;
            return;
        }
    };
    if let Some(obj) = body.as_object_mut() {
        obj.insert("stream".to_string(), serde_json::json!(true));
        obj.insert("timings_per_token".to_string(), serde_json::json!(true));
    }
    let mut req = reqwest::Client::new()
        .post(&url)
        .header("Content-Type", "application/json");
    if let Some(b) = bearer {
        req = req.header("Authorization", format!("Bearer {b}"));
    }
    let mut res = match req.body(body.to_string()).send().await {
        Ok(r) => r,
        Err(e) => {
            fail(
                &tx,
                format!("cannot reach saltnitor on port {control_port}: {e}"),
                "n/a",
            )
            .await;
            return;
        }
    };
    let http_status = res.status();
    let status = http_status.to_string();
    if !http_status.is_success() {
        let body = res.bytes().await.unwrap_or_default();
        fail(&tx, describe_error_body(http_status, &body), &status).await;
        return;
    }
    let (mut ttft, mut first_at) = (None, None);
    let mut transcript = String::new();
    let mut buffer = String::new();
    let mut aborted = None;
    loop {
        let chunk = match res.chunk().await {
            Ok(Some(chunk)) => chunk,
            Ok(None) => break,
            Err(e) => {
                aborted = Some(e);
                break;
            }
        };
        if first_at.is_none() {
            let t = start.elapsed().as_millis();
            ttft = Some(t);
            first_at = Some(Instant::now());
            let _ = tx.send(Event::ApiStreamStart { ttft_ms: t }).await;
        }
        let text = String::from_utf8_lossy(&chunk);
        transcript.push_str(&text);
        buffer.push_str(&text);
        while let Some(idx) = buffer.find('\n') {
            let line: String = buffer.drain(..=idx).collect();
            let Some(data) = line.trim().strip_prefix("data: ") else {
                continue;
            };
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(data)
                && let Some(content) = json["choices"][0]["delta"]["content"].as_str()
            {
                let _ = tx
                    .send(Event::ApiStreamChunk(content.replace('\n', " ⏎ ")))
                    .await;
            }
        }
    }
    let elapsed = first_at.map(|t| t.elapsed().as_millis());
    let metrics = metrics_from_transcript(&transcript, ttft, elapsed);
    let status = if let Some(e) = aborted {
        let _ = tx
            .send(Event::Error {
                source: "interrogator".into(),
                message: format!("stream aborted before the end: {e}"),
            })
            .await;
        format!("{status} aborted")
    } else {
        status
    };
    let _ = tx.send(Event::ApiStreamEnd { metrics, status }).await;
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)] // tests may unwrap: a panic is the failure signal
    use super::*;

    /// Verifies: REQ-TUI-007/AC2
    #[test]
    fn timings_in_the_final_chunk_give_measured_pp_and_tg() {
        let m = metrics_from_transcript(
            include_str!("../tests/data/sse/with_timings.txt"),
            Some(530),
            Some(1_000),
        );
        assert_eq!(m.source, Source::Measured);
        assert!((m.pp_tps.unwrap() - 500.0).abs() < 0.01, "{m:?}"); // prompt_n 25 / prompt_ms 50
        assert!((m.tg_tps.unwrap() - 40.0).abs() < 0.01, "{m:?}"); // predicted_n 20 / predicted_ms 500
        assert_eq!(
            render(&m),
            (
                "TTFT: 530ms".to_string(),
                "PP: 500.0 t/s".to_string(),
                "TG: 40.0 t/s".to_string()
            )
        );
    }

    /// Verifies: REQ-TUI-007/AC2, REQ-TUI-006/AC1
    #[test]
    fn no_timings_means_estimated_or_not_available_never_zero() {
        let m = metrics_from_transcript(
            include_str!("../tests/data/sse/without_timings.txt"),
            Some(530),
            Some(1_000),
        );
        assert_eq!(m.source, Source::Estimated);
        assert_eq!(m.pp_tps, None);
        assert_eq!(m.content_chunks, 3);
        assert_eq!(
            render(&m),
            (
                "TTFT: 530ms".to_string(),
                "PP: n/a".to_string(),
                "TG: 3.0 t/s (est.)".to_string()
            )
        );
        let none = metrics_from_transcript("", None, None);
        assert_eq!(
            render(&none),
            (
                "TTFT: n/a".to_string(),
                "PP: n/a".to_string(),
                "TG: n/a".to_string()
            )
        );
    }

    /// Verifies: REQ-TUI-007/AC3
    #[test]
    fn history_lives_under_xdg_state_home() {
        let e = |k: &str| match k {
            "XDG_STATE_HOME" => Some("/st".to_string()),
            "HOME" => Some("/h".to_string()),
            _ => None,
        };
        assert_eq!(
            history_path(&e),
            std::path::PathBuf::from("/st/saltnitor/history")
        );
        let e2 = |k: &str| (k == "HOME").then(|| "/h".to_string());
        assert_eq!(
            history_path(&e2),
            std::path::PathBuf::from("/h/.local/state/saltnitor/history")
        );
    }

    /// Verifies: REQ-TUI-007/AC1
    #[tokio::test]
    async fn strike_posts_to_the_given_port_with_bearer_and_timings_flag() {
        use axum::{Router, extract::State, http::HeaderMap, routing::post};
        use std::sync::{Arc, Mutex};
        type Seen = Arc<Mutex<Option<(String, serde_json::Value)>>>;
        let seen: Seen = Arc::default();
        async fn handler(State(seen): State<Seen>, h: HeaderMap, body: String) -> String {
            let auth = h
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            let json = serde_json::from_str(&body).unwrap_or_default();
            *seen.lock().unwrap() = Some((auth, json));
            include_str!("../tests/data/sse/with_timings.txt").to_string()
        }
        let app = Router::new()
            .route("/v1/chat/completions", post(handler))
            .with_state(seen.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move { axum::serve(listener, app).await });

        let (tx, mut rx) = tokio::sync::mpsc::channel(64);
        strike(
            port,
            Some("client-key".into()),
            r#"{"messages":[]}"#.into(),
            tx,
        )
        .await;
        let mut end = None;
        while let Some(ev) = rx.recv().await {
            if let Event::ApiStreamEnd { metrics, status } = ev {
                end = Some((metrics, status));
            }
        }
        let (metrics, status) = end.unwrap();
        assert_eq!(status, "200 OK");
        assert_eq!(metrics.source, Source::Measured);
        assert!(metrics.ttft_ms.is_some(), "TTFT is measured client-side");
        let (auth, body) = seen.lock().unwrap().clone().unwrap();
        assert_eq!(auth, "Bearer client-key");
        assert_eq!(body["stream"], true);
        assert_eq!(body["timings_per_token"], true);
    }
}
