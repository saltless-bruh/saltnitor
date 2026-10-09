#![allow(clippy::unwrap_used, clippy::expect_used)] // tests may unwrap: a panic is the failure signal
//! The TUI hot-swap talks to the raw router with the router's own key, never the daemon client
//! key, and never sends a bearer off-host (REQ-SEC-012/AC1, INV-06, INV-16).
use axum::{Router, extract::State, http::HeaderMap, routing::post};
use saltnitor::hotswap::{direct_router_bearer, warm_load};
use std::sync::{Arc, Mutex};

/// Verifies: REQ-SEC-012/AC1 — the router's key goes only to a loopback router, only when enabled
#[test]
fn the_router_key_is_sent_only_to_loopback_and_only_when_enabled() {
    for host in ["127.0.0.1", "localhost", "::1", "[::1]", "127.0.0.9"] {
        assert_eq!(
            direct_router_bearer(host, true, Some("router-key")).as_deref(),
            Some("router-key"),
            "{host}"
        );
    }
    assert_eq!(
        direct_router_bearer("127.0.0.1", false, Some("router-key")),
        None
    );
    assert_eq!(direct_router_bearer("127.0.0.1", true, None), None);
    for host in [
        "100.64.1.2",
        "example.com",
        "0.0.0.0",
        "192.168.1.5",
        "localhost.evil.com",
    ] {
        assert_eq!(
            direct_router_bearer(host, true, Some("router-key")),
            None,
            "{host} must never receive a bearer"
        );
    }
}

type Seen = Arc<Mutex<Vec<(Option<String>, String)>>>;

async fn spawn_router() -> (u16, Seen) {
    let seen: Seen = Arc::default();
    async fn chat(State(seen): State<Seen>, h: HeaderMap, body: String) -> &'static str {
        let auth = h
            .get("authorization")
            .map(|v| v.to_str().unwrap().to_string());
        seen.lock().unwrap().push((auth, body));
        "{}"
    }
    let app = Router::new()
        .route("/v1/chat/completions", post(chat))
        .with_state(seen.clone());
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    (port, seen)
}

/// Verifies: REQ-SEC-012/AC1 — the warm-up request carries the router key, and a model id with a quote stays valid JSON
#[tokio::test]
async fn warm_load_sends_the_router_key_not_the_client_key() {
    let (port, seen) = spawn_router().await;
    warm_load("127.0.0.1", port, Some("router-key".into()), "we\"ird")
        .await
        .unwrap();
    let seen = seen.lock().unwrap();
    assert_eq!(seen[0].0.as_deref(), Some("Bearer router-key"));
    let body: serde_json::Value = serde_json::from_str(&seen[0].1).unwrap();
    assert_eq!(body["model"], "we\"ird");
    assert_eq!(body["max_tokens"], 1);
}

/// Verifies: REQ-SEC-012/AC1 — without a router key no Authorization header is sent
#[tokio::test]
async fn warm_load_without_a_key_sends_no_authorization() {
    let (port, seen) = spawn_router().await;
    warm_load("127.0.0.1", port, None, "m").await.unwrap();
    assert_eq!(seen.lock().unwrap()[0].0, None);
}

/// Verifies: REQ-ERR-005/AC1 — a non-2xx router answer is an error, not a silent success
#[tokio::test]
async fn warm_load_reports_a_refusal() {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    let app = Router::new().route(
        "/v1/chat/completions",
        post(|| async { (axum::http::StatusCode::UNAUTHORIZED, "no") }),
    );
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    let e = warm_load("127.0.0.1", port, None, "m").await.unwrap_err();
    assert!(e.contains("401"), "{e}");
}
