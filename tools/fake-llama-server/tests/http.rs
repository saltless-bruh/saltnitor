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
