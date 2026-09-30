//! R0 capture and replay (CR-2; REQ-TST-011).
use std::path::PathBuf;

use fake_llama_server::record::{CaptureOptions, Liveness, Shape, capture};
use fake_llama_server::{ModelsShape, Scenario, spawn};

fn tmpdir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("fake-rec-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn opts(upstream: String, out: PathBuf) -> CaptureOptions {
    CaptureOptions {
        upstream,
        out,
        baseline: None,
        bearer: None,
        slow_ms: 2000,
    }
}

fn liveness(m: &fake_llama_server::record::Manifest, path: &str) -> Liveness {
    m.endpoints
        .iter()
        .find(|e| e.path == path)
        .unwrap()
        .liveness
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn capture_saves_bodies_manifest_and_liveness() {
    let f = spawn(Scenario::default().with_model("A", true)).await;
    let out = tmpdir("basic");
    let m = capture(&opts(f.base_url(), out.clone())).await.unwrap();
    for p in ["/health", "/models", "/v1/models", "/slots", "/metrics"] {
        assert_eq!(liveness(&m, p), Liveness::Live, "{p}");
    }
    assert_eq!(liveness(&m, "/props"), Liveness::Absent);
    for file in [
        "health.json",
        "models.json",
        "v1_models.json",
        "slots.json",
        "metrics.txt",
        "manifest.json",
    ] {
        assert!(out.join(file).exists(), "{file}");
    }
    assert!(!out.join("props.json").exists());
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn capture_issues_only_get_requests() {
    let f = spawn(Scenario::default().with_model("A", true)).await;
    capture(&opts(f.base_url(), tmpdir("get"))).await.unwrap();
    let methods: Vec<String> = f
        .recorder
        .of_kind("request")
        .iter()
        .map(|r| r["method"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(methods.len(), 6);
    assert!(methods.iter().all(|m| m == "GET"), "{methods:?}");
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn capture_redacts_home_prompts_and_bearer() {
    let f = spawn(Scenario::default().with_model("/home/alice/models/q.gguf", true)).await;
    let out = tmpdir("redact");
    let mut o = opts(f.base_url(), out.clone());
    o.bearer = Some("sk-live-token".into());
    capture(&o).await.unwrap();
    assert_eq!(f.recorder.of_kind("request")[0]["has_authorization"], true);
    let mut all = String::new();
    for e in std::fs::read_dir(&out).unwrap() {
        all.push_str(&std::fs::read_to_string(e.unwrap().path()).unwrap());
    }
    assert!(all.contains("${HOME}/models/q.gguf"));
    assert!(!all.contains("/home/alice"));
    assert!(!all.contains("sk-live-token"));
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn drift_against_a_baseline_names_the_changed_key_paths() {
    let base = tmpdir("drift-base");
    let now = tmpdir("drift-now");
    let a = spawn(Scenario::default().with_model("A", true)).await;
    capture(&opts(a.base_url(), base.clone())).await.unwrap();
    let b = spawn(
        Scenario::default()
            .with_model("A", true)
            .with_shape(ModelsShape::Legacy),
    )
    .await;
    let mut o = opts(b.base_url(), now);
    o.baseline = Some(base);
    let m = capture(&o).await.unwrap();
    let models = m.endpoints.iter().find(|e| e.path == "/models").unwrap();
    assert_eq!(
        models.shape,
        Shape::Drift {
            added: vec![],
            removed: vec!["data[].status.value".into()]
        }
    );
    let health = m.endpoints.iter().find(|e| e.path == "/health").unwrap();
    assert_eq!(health.shape, Shape::Ok);
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn down_upstream_is_reported_not_raised() {
    let m = capture(&opts("http://127.0.0.1:1".into(), tmpdir("down")))
        .await
        .unwrap();
    assert!(m.endpoints.iter().all(|e| e.liveness == Liveness::Down));
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn replay_serves_captured_bytes_and_falls_back_for_the_rest() {
    let src = spawn(
        Scenario::default()
            .with_model("A", true)
            .with_shape(ModelsShape::Legacy),
    )
    .await;
    let cap = tmpdir("replay");
    capture(&opts(src.base_url(), cap.clone())).await.unwrap();
    drop(src);
    let f = spawn(Scenario::default().with_replay(cap.clone())).await;
    let body = reqwest::get(format!("{}/models", f.base_url()))
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    assert_eq!(
        body.as_ref(),
        std::fs::read(cap.join("models.json")).unwrap().as_slice()
    );
    assert_eq!(
        reqwest::get(format!("{}/props", f.base_url()))
            .await
            .unwrap()
            .status(),
        404
    );
    let empty = tmpdir("replay-empty");
    let g = spawn(Scenario::default().with_replay(empty)).await;
    assert_eq!(
        reqwest::get(format!("{}/health", g.base_url()))
            .await
            .unwrap()
            .status(),
        200
    );
}
