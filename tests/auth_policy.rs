#![allow(clippy::unwrap_used, clippy::expect_used)] // tests may unwrap: a panic is the failure signal
//! Enumerates the route policy against the real router (REQ-SEC-001, REQ-SEC-005, REQ-PRX-016).
use axum::http::Method;
use fake_llama_server::Scenario;
use proptest::prelude::*;
use saltnitor::auth::{Auth, POLICY, Presented, RoutePolicy, Scope, decide};
use saltnitor::control_api::{ControlApi, HANDLER_PATHS, serve};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

const TOKEN: &str = "test-token-0123456789";

async fn rig(
    allow_query: bool,
) -> (
    String,
    mpsc::Receiver<saltnitor::events::Event>,
    fake_llama_server::Handle,
) {
    let fake = fake_llama_server::spawn(
        toml::from_str::<Scenario>(include_str!("fixtures/scenarios/control-api-legacy.toml"))
            .unwrap(),
    )
    .await;
    let (tx, rx) = mpsc::channel(256);
    let api = Arc::new(
        ControlApi::new(
            Default::default(),
            fake.base_url(),
            None,
            Some(TOKEN.into()),
            0.0,
            0.0,
            tx,
        )
        .allow_query_token(allow_query),
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
            return (base, rx, fake);
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("control API did not come up");
}

fn body_for(path: &str) -> &'static str {
    match path {
        "/v1/chat/completions" => r#"{"model":"A","messages":[]}"#,
        "/v1/ensure" => r#"{"profile":"A"}"#,
        _ => "",
    }
}

/// Verifies: REQ-SEC-001/AC3
#[test]
fn every_handler_has_a_policy_and_every_policy_a_handler() {
    let handlers: BTreeSet<&str> = HANDLER_PATHS.iter().copied().collect();
    let policies: BTreeSet<&str> = POLICY.iter().map(|p| p.path).collect();
    assert_eq!(
        handlers, policies,
        "route without a policy entry, or policy without a route"
    );
}

/// Verifies: REQ-SEC-001/AC1, REQ-SEC-001/AC2, REQ-SEC-005/AC1, REQ-PRX-016/AC1
#[tokio::test]
async fn every_route_gets_the_policy_outcome() {
    let (base, _rx, _fake) = rig(false).await;
    let http = reqwest::Client::new();
    for p in POLICY {
        for m in p.methods {
            let url = format!("{base}{}", p.path);
            let send = |bearer: Option<&str>| {
                let mut r = http
                    .request(m.clone(), &url)
                    .body(body_for(p.path))
                    .header("content-type", "application/json");
                if let Some(b) = bearer {
                    r = r.header("authorization", format!("Bearer {b}"));
                }
                r.send()
            };
            let none = send(None).await.unwrap();
            let wrong = send(Some("nope")).await.unwrap();
            let right = send(Some(TOKEN)).await.unwrap();
            match p.auth {
                Auth::Public => assert_ne!(none.status(), 401, "{} {}", m, p.path),
                Auth::Required => {
                    assert_eq!(none.status(), 401, "{} {} without credentials", m, p.path);
                    assert_eq!(wrong.status(), 401, "{} {} wrong token", m, p.path);
                    let v: serde_json::Value = none.json().await.unwrap();
                    assert_eq!(v["error"]["code"], "AUTH_REQUIRED");
                    assert_ne!(right.status(), 401, "{} {} with the token", m, p.path);
                }
            }
        }
    }
    let h: serde_json::Value = http
        .get(format!("{base}/healthz"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(h, serde_json::json!({"status": "ok"}));
}

/// Verifies: REQ-SEC-005/AC2
#[tokio::test]
async fn query_token_is_ignored_by_default() {
    let (base, _rx, _fake) = rig(false).await;
    let r = reqwest::get(format!("{base}/v1/ensure/stream?profile=A&token={TOKEN}"))
        .await
        .unwrap();
    assert_eq!(r.status(), 401);
}

/// Verifies: REQ-SEC-005/AC3
#[tokio::test]
async fn query_token_in_compat_mode_works_only_on_get_ensure_stream_and_is_redacted() {
    let (base, mut rx, _fake) = rig(true).await;
    let ok = reqwest::get(format!("{base}/v1/ensure/stream?profile=A&token={TOKEN}"))
        .await
        .unwrap();
    assert_ne!(ok.status(), 401);
    let other = reqwest::get(format!("{base}/v1/models?token={TOKEN}"))
        .await
        .unwrap();
    assert_eq!(other.status(), 401, "?token= is never accepted elsewhere");
    while let Ok(ev) = rx.try_recv() {
        if let saltnitor::events::Event::LogLine(l) = ev {
            assert!(!l.contains(TOKEN), "token leaked into a log line: {l}");
        }
    }
}

/// Verifies: REQ-SEC-011/AC1
#[tokio::test]
async fn failed_auth_is_logged_once_per_peer_per_second_without_the_token() {
    let (base, mut rx, _fake) = rig(false).await;
    let http = reqwest::Client::new();
    for _ in 0..3 {
        http.get(format!("{base}/v1/models"))
            .header("authorization", "Bearer wrong-secret")
            .send()
            .await
            .unwrap();
    }
    tokio::time::sleep(Duration::from_millis(50)).await;
    let mut lines = vec![];
    while let Ok(ev) = rx.try_recv() {
        if let saltnitor::events::Event::LogLine(l) = ev
            && l.starts_with("auth: 401")
        {
            lines.push(l);
        }
    }
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].contains("127.0.0.1")
            && lines[0].contains("/v1/models")
            && !lines[0].contains("wrong-secret")
    );
}

/// Verifies: REQ-SEC-015/AC1
#[tokio::test]
async fn no_cors_headers_by_default() {
    let (base, _rx, _fake) = rig(false).await;
    let r = reqwest::Client::new()
        .get(format!("{base}/healthz"))
        .header("origin", "http://evil.example")
        .send()
        .await
        .unwrap();
    assert!(r.headers().get("access-control-allow-origin").is_none());
}

/// Verifies: REQ-SEC-001/AC2 (wrong scope → 403; admin routes arrive in P3, so the decision is unit-tested)
#[test]
fn admin_scope_with_an_inference_token_is_forbidden() {
    let admin = RoutePolicy {
        path: "/admin/x",
        methods: &[Method::GET],
        auth: Auth::Required,
        scope: Scope::Admin,
        query_token_compat: false,
    };
    let p = Presented {
        token_configured: true,
        bearer_valid: true,
        query_valid: false,
        allow_query: false,
        is_get: true,
    };
    assert_eq!(
        decide(&admin, &Method::GET, p),
        Err(saltnitor::error::ErrorCode::AuthForbidden)
    );
}

proptest! {
    /// Verifies: REQ-SEC-001/AC2 [PROP], REQ-SEC-005/AC1 — [RF-1] scheme case and extra spaces
    #[test]
    fn bearer_header_parsing_is_tolerant_and_exact(scheme in "[Bb][Ee][Aa][Rr][Ee][Rr]", spaces in " {1,3}", junk in "[A-Za-z0-9._-]{1,40}") {
        let header = format!("{scheme}{spaces}{TOKEN}");
        prop_assert_eq!(saltnitor::auth::bearer_from_header(&header), Some(TOKEN));
        prop_assume!(junk != TOKEN);
        let junk_header = format!("Bearer {junk}");
        prop_assert_eq!(saltnitor::auth::bearer_from_header(&junk_header), Some(junk.as_str()));
        prop_assert!(!saltnitor::auth::token_matches(Some(TOKEN), &junk));
        prop_assert!(saltnitor::auth::token_matches(Some(TOKEN), TOKEN));
    }
}
