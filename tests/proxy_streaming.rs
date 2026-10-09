#![allow(clippy::unwrap_used, clippy::expect_used)] // tests may unwrap: a panic is the failure signal
//! The proxy streams byte-exact through Saltnitor's real router (REQ-PRX-001…005, -009; REQ-SEC-012).
use fake_llama_server::{Fault, Handle, Scenario};
use proptest::prelude::*;
use saltnitor::control_api::{ControlApi, ProfileMeta, serve};
use saltnitor::proxy_stream::{ProxyLimits, is_hop_by_hop, request_id_for};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

fn scenario(fault: Fault) -> Scenario {
    let mut s = Scenario {
        models_shape: fake_llama_server::ModelsShape::Legacy,
        ..Scenario::default()
    };
    s.models.push(fake_llama_server::ModelSpec {
        id: "A".into(),
        loaded: true,
    });
    s.routes.insert("POST /v1/chat/completions".into(), fault);
    s
}
fn raw(items: &[&str], delay_ms: u64) -> Fault {
    Fault::RawChunks {
        items: items.iter().map(|s| s.to_string()).collect(),
        delay_ms,
        code: 200,
        content_type: "text/event-stream".into(),
        headers: vec![],
    }
}
fn profile() -> ProfileMeta {
    ProfileMeta {
        model: "a.gguf".into(),
        offload: false,
        est_vram_gb: Some(0.1),
        est_ram_gb: Some(0.1),
    }
}

struct Rig {
    base: String,
    fake: Handle,
    _rx: mpsc::Receiver<saltnitor::events::Event>,
    http: reqwest::Client,
}
async fn rig(s: Scenario, upstream_bearer: Option<&str>, limits: ProxyLimits) -> Rig {
    let fake = fake_llama_server::spawn(s).await;
    let (tx, rx) = mpsc::channel(256);
    let api = Arc::new(
        ControlApi::new(
            HashMap::from([("A".to_string(), profile())]),
            fake.base_url(),
            upstream_bearer.map(str::to_string),
            None,
            0.0,
            0.0,
            tx,
        )
        .limits(limits),
    );
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    tokio::spawn(serve(api, addr));
    let base = format!("http://{addr}");
    let http = reqwest::Client::new();
    for _ in 0..100 {
        if http.get(format!("{base}/healthz")).send().await.is_ok() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    Rig {
        base,
        fake,
        _rx: rx,
        http,
    }
}
fn limits() -> ProxyLimits {
    ProxyLimits {
        connect: Duration::from_secs(5),
        first_byte: Duration::from_secs(600),
        idle: Duration::from_secs(120),
        max_body_bytes: 33_554_432,
    }
}
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

/// Verifies: REQ-PRX-002/AC1, REQ-PRX-002/AC2
#[tokio::test]
async fn first_chunk_is_forwarded_before_the_second_is_sent() {
    let r = rig(
        scenario(raw(&["data: A\n\n", "data: B\n\n"], 2_000)),
        None,
        limits(),
    )
    .await;
    let mut resp = r
        .http
        .post(format!("{}/v1/chat/completions", r.base))
        .body(r#"{"model":"A","stream":true}"#)
        .send()
        .await
        .unwrap();
    let first = resp.chunk().await.unwrap().unwrap();
    let t_recv = now_ms();
    assert_eq!(&first[..], b"data: A\n\n");
    let sent = r.fake.recorder.of_kind("chunk_sent");
    let t_sent_a = sent[0]["t_unix_ms"].as_u64().unwrap();
    assert!(
        t_recv.saturating_sub(t_sent_a) <= 200,
        "A took {} ms after the flush (budget 200)",
        t_recv.saturating_sub(t_sent_a)
    );
    assert_eq!(sent.len(), 1, "B must not have been sent yet");
    let rest = resp.bytes().await.unwrap();
    assert_eq!(&rest[..], b"data: B\n\n");
}

/// Verifies: REQ-PRX-004/AC1, REQ-PRX-004/AC2 — [RF-5] hop-by-hop headers from upstream
#[tokio::test]
async fn status_and_end_to_end_headers_pass_hop_by_hop_dropped_request_id_added() {
    let fault = Fault::RawChunks {
        items: vec!["x".into()],
        delay_ms: 0,
        code: 201,
        content_type: "text/plain".into(),
        headers: vec![
            ("cache-control".into(), "no-cache".into()),
            ("x-fake-upstream".into(), "1".into()),
            ("keep-alive".into(), "timeout=5".into()),
            ("proxy-connection".into(), "keep-alive".into()),
        ],
    };
    let r = rig(scenario(fault), None, limits()).await;
    let resp = r
        .http
        .post(format!("{}/v1/chat/completions", r.base))
        .body(r#"{"model":"A"}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    assert_eq!(resp.headers()["content-type"], "text/plain");
    assert_eq!(resp.headers()["cache-control"], "no-cache");
    assert_eq!(resp.headers()["x-fake-upstream"], "1");
    assert!(
        resp.headers().get("keep-alive").is_none()
            && resp.headers().get("proxy-connection").is_none()
    );
    assert!(resp.headers().get("x-request-id").is_some());
    assert_eq!(resp.text().await.unwrap(), "x");
}

/// Verifies: REQ-SEC-012/AC1, REQ-PRX-005/AC1
#[tokio::test]
async fn client_credentials_are_stripped_and_the_body_is_forwarded_verbatim() {
    let r = rig(scenario(raw(&["ok"], 0)), Some("router-key"), limits()).await;
    let body = r#"{"model":"A",  "messages":[{"role":"user","content":"hi"}] ,"extra":1}"#;
    r.http
        .post(format!("{}/v1/chat/completions", r.base))
        .header("authorization", "Bearer client-secret")
        .header("cookie", "a=b")
        .header("proxy-authorization", "Basic x")
        .body(body)
        .send()
        .await
        .unwrap();
    let reqs = r.fake.recorder.of_kind("request");
    let chat = reqs
        .iter()
        .rfind(|e| e["path"] == "/v1/chat/completions")
        .unwrap();
    assert_eq!(chat["body"], body, "bytes must be forwarded unmodified");
    assert_eq!(
        chat["has_authorization"], true,
        "the upstream credential, not the client's"
    );
    assert!(
        chat["headers"].get("cookie").is_none()
            && chat["headers"].get("proxy-authorization").is_none()
    );
}

/// Verifies: REQ-PRX-001/AC1
#[tokio::test]
async fn the_six_documented_routes_are_served() {
    let r = rig(scenario(raw(&["ok"], 0)), None, limits()).await;
    let routes = [
        ("GET", "/healthz"),
        ("GET", "/v1/models"),
        ("GET", "/v1/status"),
        ("POST", "/v1/chat/completions"),
        ("POST", "/v1/ensure"),
        ("GET", "/v1/ensure/stream"),
    ];
    let mut served: Vec<&str> = saltnitor::control_api::HANDLER_PATHS.to_vec();
    served.sort_unstable();
    let mut documented: Vec<&str> = routes.iter().map(|(_, p)| *p).collect();
    documented.sort_unstable();
    assert_eq!(
        served, documented,
        "the route table is exactly REQ-PRX-001/AC1"
    );
    for (method, path) in routes {
        let url = format!("{}{path}?profile=A", r.base);
        let req = if method == "GET" {
            r.http.get(url)
        } else {
            r.http.post(url).body(r#"{"model":"A","profile":"A"}"#)
        };
        let resp = req.send().await.unwrap();
        assert_ne!(resp.status(), 404, "{method} {path} is not served");
        assert_ne!(resp.status(), 405, "{method} {path} has the wrong method");
    }
}

/// Verifies: REQ-PRX-005/AC2 — the client's `model` is the profile id and goes upstream untouched
#[tokio::test]
async fn the_model_value_is_the_profile_id_and_reaches_the_runtime_unchanged() {
    let r = rig(scenario(raw(&["ok"], 0)), None, limits()).await;
    let body = r#"{"model":"A","messages":[]}"#;
    let resp = r
        .http
        .post(format!("{}/v1/chat/completions", r.base))
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "profile id A resolved");
    let chat = r
        .fake
        .recorder
        .of_kind("request")
        .into_iter()
        .rfind(|e| e["path"] == "/v1/chat/completions")
        .unwrap();
    assert_eq!(chat["body"], body);
    let sent: serde_json::Value = serde_json::from_str(chat["body"].as_str().unwrap()).unwrap();
    assert_eq!(
        sent["model"], "A",
        "no body rewrite: the id is the preset section"
    );
    let unknown = r
        .http
        .post(format!("{}/v1/chat/completions", r.base))
        .body(r#"{"model":"not-a-profile"}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(
        unknown.status(),
        404,
        "an id that is no profile is MODEL_NOT_FOUND"
    );
}

/// Verifies: REQ-PRX-009/AC1, REQ-PRX-009/AC2
#[tokio::test]
async fn request_ids_are_kept_when_valid_generated_otherwise_and_forwarded() {
    let r = rig(scenario(raw(&["ok"], 0)), None, limits()).await;
    let resp = r
        .http
        .post(format!("{}/v1/chat/completions", r.base))
        .header("x-request-id", "abc.DEF-123_x")
        .body(r#"{"model":"A"}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.headers()["x-request-id"], "abc.DEF-123_x");
    let resp = r
        .http
        .post(format!("{}/v1/chat/completions", r.base))
        .header("x-request-id", "bad id!")
        .body(r#"{"model":"A"}"#)
        .send()
        .await
        .unwrap();
    let generated = resp.headers()["x-request-id"].to_str().unwrap().to_string();
    assert_ne!(generated, "bad id!");
    assert!(uuid::Uuid::parse_str(&generated).is_ok_and(|u| u.get_version_num() == 7));
    let reqs = r.fake.recorder.of_kind("request");
    assert!(
        reqs.iter()
            .any(|e| e["headers"]["x-request-id"] == "abc.DEF-123_x")
    );
    let err = r
        .http
        .post(format!("{}/v1/chat/completions", r.base))
        .header("x-request-id", "err-1")
        .body(r#"{"nomodel":true}"#)
        .send()
        .await
        .unwrap();
    let v: serde_json::Value = err.json().await.unwrap();
    assert_eq!(
        v["error"]["request_id"], "err-1",
        "error bodies carry the id"
    );
}

/// Verifies: REQ-PRX-018/AC1 — [RF-2] model present but not a string; top-level array
#[tokio::test]
async fn non_string_model_or_non_object_body_is_400_and_never_forwarded() {
    let r = rig(scenario(raw(&["ok"], 0)), None, limits()).await;
    for body in [r#"{"model":3}"#, r#"[{"model":"A"}]"#, "not json"] {
        let resp = r
            .http
            .post(format!("{}/v1/chat/completions", r.base))
            .body(body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 400, "{body}");
        let v: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(v["error"]["code"], "REQUEST_INVALID");
    }
    assert!(
        r.fake
            .recorder
            .of_kind("request")
            .iter()
            .all(|e| e["path"] != "/v1/chat/completions")
    );
}

/// Verifies: REQ-PRX-002/AC3 (inspection, mechanised)
#[test]
fn proxy_stream_never_buffers_an_upstream_body() {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/proxy_stream.rs"))
        .unwrap();
    let re = regex::Regex::new(r"\.(bytes|text|json)\(\)").unwrap();
    assert!(
        !re.is_match(&src),
        "proxy_stream.rs must stream, not buffer"
    );
}

#[test]
fn request_id_validation_examples() {
    assert_eq!(request_id_for(Some("ok-1")), "ok-1");
    assert_ne!(request_id_for(Some("")), "");
    assert_ne!(request_id_for(Some(&"x".repeat(129))), "x".repeat(129));
    assert!(
        is_hop_by_hop("Transfer-Encoding", None)
            && is_hop_by_hop("x-custom", Some("x-custom, close"))
            && !is_hop_by_hop("content-type", None)
    );
}

fn sse_piece() -> impl Strategy<Value = String> {
    prop_oneof![
        Just(": keep-alive\n\n".to_string()),
        Just("data: [DONE]\n\n".to_string()),
        Just("data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"a\\\":1}\"}}]}}]}\n\n".to_string()),
        Just("data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"hmm\"}}]}\n\n".to_string()),
        "[a-z0-9 ,:{}\\[\\]\"]{0,40}\n?".prop_map(|s| s),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 24, .. ProptestConfig::default() })]
    /// Verifies: REQ-PRX-003/AC1 [PROP], REQ-TST-008/AC1
    #[test]
    fn downstream_bytes_equal_upstream_bytes_for_any_chunking(pieces in prop::collection::vec(sse_piece(), 1..12), code in prop_oneof![Just(200u16), Just(404u16), Just(500u16)], stream in any::<bool>()) {
        let expected: String = pieces.concat();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let got = rt.block_on(async {
            let fault = Fault::RawChunks { items: pieces.clone(), delay_ms: 0, code, content_type: "text/event-stream".into(), headers: vec![] };
            let r = rig(scenario(fault), None, limits()).await;
            let body = if stream { r#"{"model":"A","stream":true}"# } else { r#"{"model":"A"}"# };
            let resp = r.http.post(format!("{}/v1/chat/completions", r.base)).body(body).send().await.unwrap();
            (resp.status().as_u16(), resp.bytes().await.unwrap())
        });
        prop_assert_eq!(got.0, code);
        prop_assert_eq!(&got.1[..], expected.as_bytes());
    }
}

/// Verifies: REQ-PRX-009/AC2 — the auth layer's 401 envelope carries the request id too
#[tokio::test]
async fn auth_rejections_carry_the_request_id() {
    let fake = fake_llama_server::spawn(scenario(raw(&["ok"], 0))).await;
    let (tx, _rx) = mpsc::channel(16);
    let api = Arc::new(ControlApi::new(
        HashMap::from([("A".to_string(), profile())]),
        fake.base_url(),
        None,
        Some("secret-token".to_string()),
        0.0,
        0.0,
        tx,
    ));
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    tokio::spawn(serve(api, addr));
    let http = reqwest::Client::new();
    for _ in 0..100 {
        if http
            .get(format!("http://{addr}/healthz"))
            .send()
            .await
            .is_ok()
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let resp = http
        .post(format!("http://{addr}/v1/chat/completions"))
        .header("x-request-id", "auth-1")
        .body(r#"{"model":"A"}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
    assert_eq!(resp.headers()["x-request-id"], "auth-1");
    let v: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(v["error"]["code"], "AUTH_REQUIRED");
    assert_eq!(v["error"]["request_id"], "auth-1");
    assert!(fake.recorder.of_kind("request").is_empty());
}
