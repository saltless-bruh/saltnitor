//! HTTP behavior of the in-process fake (REQ-TST-011).
use fake_llama_server::{ModelsShape, Scenario, spawn};
use serde_json::{Value, json};

fn ab() -> Scenario {
    Scenario::default()
        .with_model("A", true)
        .with_model("B", false)
}

async fn get(url: String) -> (u16, Value) {
    let r = reqwest::get(url).await.expect("request");
    let s = r.status().as_u16();
    (s, r.json().await.unwrap_or(Value::Null))
}

async fn post(url: String, body: Value) -> (u16, Value) {
    let r = reqwest::Client::new()
        .post(url)
        .json(&body)
        .send()
        .await
        .expect("request");
    let s = r.status().as_u16();
    (s, r.json().await.unwrap_or(Value::Null))
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn health_reports_ok() {
    let f = spawn(Scenario::default()).await;
    assert_eq!(
        get(format!("{}/health", f.base_url())).await,
        (200, json!({"status": "ok"}))
    );
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn models_use_status_objects_by_default_on_both_paths() {
    let f = spawn(ab()).await;
    let want = json!({"object": "list", "data": [
        {"id": "A", "object": "model", "owned_by": "llamacpp", "status": {"value": "loaded"}},
        {"id": "B", "object": "model", "owned_by": "llamacpp", "status": {"value": "unloaded"}}
    ]});
    assert_eq!(
        get(format!("{}/models", f.base_url())).await,
        (200, want.clone())
    );
    assert_eq!(
        get(format!("{}/v1/models", f.base_url())).await,
        (200, want)
    );
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn legacy_shape_uses_a_status_string() {
    let f = spawn(ab().with_shape(ModelsShape::Legacy)).await;
    let (_, v) = get(format!("{}/v1/models", f.base_url())).await;
    assert_eq!(v["data"][0]["status"], json!("loaded"));
    assert_eq!(v["data"][1]["status"], json!("unloaded"));
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn load_and_unload_change_what_models_reports() {
    let f = spawn(ab()).await;
    assert_eq!(
        post(
            format!("{}/models/load", f.base_url()),
            json!({"model": "B"})
        )
        .await,
        (200, json!({"success": true}))
    );
    assert_eq!(f.loaded(), vec!["A", "B"]);
    assert_eq!(
        post(
            format!("{}/models/unload", f.base_url()),
            json!({"model": "A"})
        )
        .await
        .0,
        200
    );
    assert_eq!(f.loaded(), vec!["B"]);
    let (_, v) = get(format!("{}/models", f.base_url())).await;
    assert_eq!(v["data"][0]["status"]["value"], json!("unloaded"));
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn max_loaded_evicts_the_oldest_model() {
    let f = spawn(ab().with_max_loaded(1)).await;
    post(
        format!("{}/models/load", f.base_url()),
        json!({"model": "B"}),
    )
    .await;
    assert_eq!(f.loaded(), vec!["B"]);
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn load_of_unknown_model_is_404_and_changes_nothing() {
    let f = spawn(ab()).await;
    assert_eq!(
        post(
            format!("{}/models/load", f.base_url()),
            json!({"model": "Z"})
        )
        .await
        .0,
        404
    );
    assert_eq!(
        post(format!("{}/models/load", f.base_url()), json!({}))
            .await
            .0,
        400
    );
    assert_eq!(f.loaded(), vec!["A"]);
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn metrics_and_slots_are_served() {
    let f = spawn(ab()).await;
    let r = reqwest::get(format!("{}/metrics", f.base_url()))
        .await
        .unwrap();
    assert!(
        r.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/plain")
    );
    let text = r.text().await.unwrap();
    assert!(text.contains("llamacpp:requests_processing 0"), "{text}");
    assert!(text.contains("llamacpp:models_loaded 1"), "{text}");
    assert_eq!(
        get(format!("{}/slots", f.base_url())).await,
        (200, json!([{"id": 0, "is_processing": false}]))
    );
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn unknown_route_is_404() {
    let f = spawn(ab()).await;
    assert_eq!(get(format!("{}/nope", f.base_url())).await.0, 404);
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn recorder_strips_authorization() {
    let f = spawn(ab()).await;
    reqwest::Client::new()
        .post(format!("{}/models/load", f.base_url()))
        .bearer_auth("sk-secret")
        .body(r#"{"model":"B"}"#)
        .send()
        .await
        .unwrap();
    let reqs = f.recorder.of_kind("request");
    let last = reqs.last().unwrap();
    assert_eq!(last["method"], "POST");
    assert_eq!(last["path"], "/models/load");
    assert_eq!(last["body"], r#"{"model":"B"}"#);
    assert_eq!(last["has_authorization"], true);
    assert!(last["headers"].get("authorization").is_none());
    assert!(
        !serde_json::to_string(&f.recorder.events())
            .unwrap()
            .contains("sk-secret")
    );
}

use fake_llama_server::Fault;
use std::time::{Duration, Instant};

fn chat_url(f: &fake_llama_server::Handle) -> String {
    format!("{}/v1/chat/completions", f.base_url())
}

async fn read_all(resp: &mut reqwest::Response) -> (String, Option<reqwest::Error>) {
    let mut text = String::new();
    loop {
        match resp.chunk().await {
            Ok(Some(c)) => text.push_str(&String::from_utf8_lossy(&c)),
            Ok(None) => return (text, None),
            Err(e) => return (text, Some(e)),
        }
    }
}

fn deltas(sse: &str) -> String {
    sse.lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter(|d| *d != "[DONE]")
        .map(|d| {
            serde_json::from_str::<Value>(d).unwrap()["choices"][0]["delta"]["content"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect()
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn chat_non_stream_autoloads_and_evicts() {
    let f = spawn(ab().with_max_loaded(1)).await;
    let (s, v) = post(
        chat_url(&f),
        json!({"model": "B", "messages": [{"role": "user", "content": "hi"}]}),
    )
    .await;
    assert_eq!(s, 200);
    assert_eq!(v["object"], "chat.completion");
    assert_eq!(v["model"], "B");
    assert_eq!(v["choices"][0]["message"]["content"], "Hello from fake");
    assert_eq!(f.loaded(), vec!["B"]);
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn chat_rejects_missing_or_unknown_model() {
    let f = spawn(ab()).await;
    assert_eq!(post(chat_url(&f), json!({"messages": []})).await.0, 400);
    let (s, v) = post(chat_url(&f), json!({"model": "Z", "messages": []})).await;
    assert_eq!(s, 400);
    assert_eq!(v["error"]["message"], "model 'Z' not found");
    assert_eq!(f.loaded(), vec!["A"]);
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn chat_stream_sends_sse_chunks_then_done() {
    let f = spawn(ab()).await;
    let mut r = reqwest::Client::new()
        .post(chat_url(&f))
        .json(&json!({"model": "A", "stream": true, "messages": []}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.headers()["content-type"], "text/event-stream");
    let (text, err) = read_all(&mut r).await;
    assert!(err.is_none());
    assert_eq!(text.matches("chat.completion.chunk").count(), 3);
    assert_eq!(deltas(&text), "Hello from fake");
    assert!(text.ends_with("data: [DONE]\n\n"), "{text}");
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn chunks_fault_streams_with_scripted_timing() {
    let f = spawn(ab().with_fault(
        "POST /v1/chat/completions",
        Fault::Chunks {
            items: vec!["a".into(), "b".into(), "c".into()],
            delay_ms: 150,
        },
    ))
    .await;
    let t0 = Instant::now();
    let mut r = reqwest::Client::new()
        .post(chat_url(&f))
        .json(&json!({"model": "A", "stream": true, "messages": []}))
        .send()
        .await
        .unwrap();
    let mut first = None;
    let mut text = String::new();
    while let Some(c) = r.chunk().await.unwrap() {
        first.get_or_insert(t0.elapsed());
        text.push_str(&String::from_utf8_lossy(&c));
    }
    let (first, total) = (first.unwrap(), t0.elapsed());
    assert_eq!(deltas(&text), "abc");
    assert!(first >= Duration::from_millis(140), "first={first:?}");
    assert!(total >= Duration::from_millis(430), "total={total:?}");
    assert!(
        total - first >= Duration::from_millis(250),
        "streamed, not buffered: first={first:?} total={total:?}"
    );
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn chunks_fault_on_non_stream_chat_waits_then_answers() {
    let f = spawn(ab().with_fault(
        "POST /v1/chat/completions",
        Fault::Chunks {
            items: vec!["x".into(), "y".into()],
            delay_ms: 100,
        },
    ))
    .await;
    let t0 = Instant::now();
    let (s, v) = post(chat_url(&f), json!({"model": "A", "messages": []})).await;
    assert_eq!(
        (s, v["choices"][0]["message"]["content"].as_str()),
        (200, Some("xy"))
    );
    assert!(t0.elapsed() >= Duration::from_millis(190));
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn status_fault_overrides_a_route() {
    let f = spawn(ab().with_fault(
        "GET /health",
        Fault::Status {
            code: 503,
            body: r#"{"error":"busy"}"#.into(),
        },
    ))
    .await;
    let r = reqwest::get(format!("{}/health", f.base_url()))
        .await
        .unwrap();
    assert_eq!(r.status(), 503);
    assert_eq!(r.headers()["content-type"], "application/json");
    assert_eq!(r.text().await.unwrap(), r#"{"error":"busy"}"#);
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn hang_before_headers_times_the_client_out() {
    let f = spawn(ab().with_fault("GET /health", Fault::HangBeforeHeaders)).await;
    let err = reqwest::Client::new()
        .get(format!("{}/health", f.base_url()))
        .timeout(Duration::from_millis(300))
        .send()
        .await
        .unwrap_err();
    assert!(err.is_timeout(), "{err}");
    assert_eq!(
        f.recorder.of_kind("request").last().unwrap()["path"],
        "/health"
    );
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn crash_after_drops_the_connection_mid_stream() {
    let f =
        spawn(ab().with_fault("POST /v1/chat/completions", Fault::CrashAfter { chunks: 1 })).await;
    let mut r = reqwest::Client::new()
        .post(chat_url(&f))
        .json(&json!({"model": "A", "stream": true, "messages": []}))
        .send()
        .await
        .unwrap();
    let (text, err) = read_all(&mut r).await;
    assert!(
        err.is_some(),
        "stream must end in an error, got clean end: {text}"
    );
    assert_eq!(text.matches("chat.completion.chunk").count(), 1, "{text}");
    assert!(!text.contains("[DONE]"));
    assert_eq!(f.recorder.of_kind("crash").len(), 1);
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn malformed_fault_breaks_json_and_sse() {
    let f = spawn(ab().with_fault("POST /v1/chat/completions", Fault::Malformed)).await;
    let r = reqwest::Client::new()
        .post(chat_url(&f))
        .json(&json!({"model": "A", "messages": []}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    assert!(serde_json::from_str::<Value>(&r.text().await.unwrap()).is_err());
    let r = reqwest::Client::new()
        .post(chat_url(&f))
        .json(&json!({"model": "A", "stream": true, "messages": []}))
        .send()
        .await
        .unwrap();
    let text = r.text().await.unwrap();
    let data = text.strip_prefix("data: ").unwrap().trim_end();
    assert!(serde_json::from_str::<Value>(data).is_err());
    assert!(!text.contains("[DONE]"));
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn client_disconnect_is_recorded() {
    let items = (0..10).map(|i| i.to_string()).collect();
    let f = spawn(ab().with_fault(
        "POST /v1/chat/completions",
        Fault::Chunks {
            items,
            delay_ms: 100,
        },
    ))
    .await;
    let mut r = reqwest::Client::new()
        .post(chat_url(&f))
        .json(&json!({"model": "A", "stream": true, "messages": []}))
        .send()
        .await
        .unwrap();
    r.chunk().await.unwrap();
    drop(r);
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let d = f.recorder.of_kind("disconnect");
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0]["path"], "/v1/chat/completions");
}
