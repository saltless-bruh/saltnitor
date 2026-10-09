#![allow(clippy::unwrap_used, clippy::expect_used)] // tests may unwrap: a panic is the failure signal
//! The upstream client talks to a loopback runtime directly: a stray `HTTP_PROXY` in the
//! daemon's environment must not divert inference traffic (INV-06, REQ-PRX-002).
//! One test per binary: it edits the process environment.
use axum::{Router, routing::get};
use saltnitor::proxy_stream::{ProxyLimits, upstream_client};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Verifies: REQ-PRX-002/AC1 — loopback upstream traffic ignores the system proxy variables
#[tokio::test]
async fn http_proxy_env_does_not_divert_upstream_traffic() {
    let decoy = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let decoy_port = decoy.local_addr().unwrap().port();
    let hits = Arc::new(AtomicUsize::new(0));
    let h = hits.clone();
    tokio::spawn(async move {
        while decoy.accept().await.is_ok() {
            h.fetch_add(1, Ordering::SeqCst);
        }
    });
    // SAFETY: this is the only test in this binary, so no other thread reads the environment
    // while it is edited, and the variables are set before the client is built.
    unsafe {
        std::env::set_var("HTTP_PROXY", format!("http://127.0.0.1:{decoy_port}"));
        std::env::set_var("http_proxy", format!("http://127.0.0.1:{decoy_port}"));
        std::env::remove_var("NO_PROXY");
        std::env::remove_var("no_proxy");
    }

    let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = upstream.local_addr().unwrap().port();
    let app = Router::new().route("/ping", get(|| async { "pong" }));
    tokio::spawn(async move { axum::serve(upstream, app).await });

    let client = upstream_client(&ProxyLimits::default()).unwrap();
    let body = client
        .get(format!("http://127.0.0.1:{port}/ping"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert_eq!(body, "pong");
    assert_eq!(
        hits.load(Ordering::SeqCst),
        0,
        "the proxy decoy must see no connection"
    );
}
