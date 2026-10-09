#![allow(clippy::unwrap_used, clippy::expect_used)] // tests may unwrap: a panic is the failure signal
//! Cancellation, timeouts, upstream failures and request limits of the streaming proxy
//! (REQ-PRX-006, -008, -010, -011, -017, -018), through Saltnitor's real router and the fake runtime.
mod common;
use common::*;
use fake_llama_server::Fault;
use saltnitor::control_api::{ControlApi, serve};
use saltnitor::events::Event;
use saltnitor::proxy_stream::ProxyLimits;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn short() -> ProxyLimits {
    ProxyLimits {
        connect: Duration::from_millis(500),
        first_byte: Duration::from_millis(500),
        idle: Duration::from_millis(300),
        max_body_bytes: 33_554_432,
    }
}

/// Verifies: REQ-PRX-006/AC1, REQ-TST-002/AC1 (cancellation)
#[tokio::test]
async fn client_disconnect_reaches_the_fake_within_one_second() {
    let r = rig(scenario(raw(&["data: 1\n\n"; 200], 50)), None, limits()).await;
    let mut resp = r
        .http
        .post(format!("{}/v1/chat/completions", r.base))
        .body(r#"{"model":"A","stream":true}"#)
        .send()
        .await
        .unwrap();
    let _ = resp.chunk().await.unwrap();
    assert_eq!(r.fake.in_flight(), 1);
    drop(resp);
    let t = Instant::now();
    loop {
        if !r.fake.recorder.of_kind("disconnect").is_empty() && r.fake.in_flight() == 0 {
            break;
        }
        assert!(
            t.elapsed() < Duration::from_secs(1),
            "fake did not see the close within cancel_propagation_ms"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// Verifies: REQ-PRX-008/AC1, REQ-TST-002/AC1 (4xx/5xx passthrough)
#[tokio::test]
async fn upstream_errors_pass_through_byte_exact() {
    for (code, body) in [
        (400u16, r#"{"error":{"message":"bad"}}"#),
        (503, "slot busy"),
        (500, ""),
    ] {
        let r = rig(
            scenario(Fault::Status {
                code,
                body: body.into(),
            }),
            None,
            limits(),
        )
        .await;
        let resp = r
            .http
            .post(format!("{}/v1/chat/completions", r.base))
            .body(r#"{"model":"A"}"#)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), code);
        assert_eq!(resp.text().await.unwrap(), body);
    }
}

/// A bare-listener runtime: it reports model A as loaded, then answers the chat request with
/// `chat_reply` (`None` closes the socket without writing a byte). The fake cannot do either of
/// these: its faults always write a well-formed status line and headers. Returns the proxy's
/// response to a chat request.
async fn chat_against_bare_runtime(chat_reply: Option<&'static [u8]>) -> reqwest::Response {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (mut sock, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                let n = sock.read(&mut buf).await.unwrap_or(0);
                if buf[..n].starts_with(b"GET /v1/models") {
                    let body = r#"{"object":"list","data":[{"id":"A","loaded":true}]}"#;
                    let head = format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = sock.write_all(head.as_bytes()).await;
                } else if let Some(reply) = chat_reply {
                    let _ = sock.write_all(reply).await;
                }
                // Otherwise (the chat request, `None`): close without writing a byte.
            });
        }
    });
    let (tx, _rx) = tokio::sync::mpsc::channel(8);
    let api = Arc::new(
        ControlApi::new(
            HashMap::from([("A".to_string(), profile())]),
            format!("http://{upstream}"),
            None,
            None,
            0.0,
            0.0,
            tx,
        )
        .unwrap()
        .limits(short())
        .unwrap(),
    );
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
            .is_ok_and(|r| r.status() == 200)
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    http.post(format!("http://{addr}/v1/chat/completions"))
        .body(r#"{"model":"A"}"#)
        .send()
        .await
        .unwrap()
}

/// Verifies: REQ-PRX-008/AC2 — the connection dies before any response header was written
#[tokio::test]
async fn a_connection_closed_before_headers_is_502_runtime_unhealthy() {
    let resp = chat_against_bare_runtime(None).await;
    assert_eq!(resp.status(), 502);
    let v: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(v["error"]["code"], "RUNTIME_UNHEALTHY");
}

/// Verifies: REQ-PRX-008/AC2 — a malformed response (no valid status line) gets the same envelope
#[tokio::test]
async fn a_malformed_status_line_is_502_runtime_unhealthy() {
    let resp = chat_against_bare_runtime(Some(b"garbage\r\n\r\n")).await;
    assert_eq!(resp.status(), 502);
    let v: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(v["error"]["code"], "RUNTIME_UNHEALTHY");
}

/// Verifies: REQ-PRX-010/AC2, REQ-TST-002/AC1 (timeouts)
#[tokio::test]
async fn hang_before_headers_is_504() {
    let r = rig(scenario(Fault::HangBeforeHeaders), None, short()).await;
    let resp = r
        .http
        .post(format!("{}/v1/chat/completions", r.base))
        .body(r#"{"model":"A"}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 504);
    let v: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(v["error"]["code"], "UPSTREAM_TIMEOUT");
}

/// Verifies: REQ-PRX-011/AC1 — a runtime crash mid-stream truncates the body (an error, never a clean end)
#[tokio::test]
async fn mid_stream_crash_truncates_without_done_and_logs_the_abort() {
    let mut r = rig(scenario(Fault::CrashAfter { chunks: 2 }), None, limits()).await;
    let mut rx = r.take_events();
    let resp = r
        .http
        .post(format!("{}/v1/chat/completions", r.base))
        .body(r#"{"model":"A","stream":true}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    // The truncation must reach the client as a body error; a clean Ok would hide the crash.
    let err = resp
        .bytes()
        .await
        .expect_err("a crashed stream must not end cleanly");
    assert!(
        err.is_body() || err.is_decode() || err.is_request(),
        "{err:?}"
    );
    tokio::time::sleep(Duration::from_millis(50)).await;
    let mut logged = false;
    while let Ok(ev) = rx.try_recv() {
        if let Event::LogLine(l) = ev
            && l.starts_with("UPSTREAM_STREAM_ABORTED request_id=")
        {
            logged = true;
        }
    }
    assert!(logged, "abort must be recorded with the request id");
}

/// Verifies: REQ-PRX-010/AC1 — a stream that stalls longer than idle_ms is aborted, not completed
#[tokio::test]
async fn an_idle_stall_aborts_the_stream_before_the_next_chunk() {
    let r = rig(
        scenario(raw(&["data: a\n\n", "data: b\n\n"], 2_000)),
        None,
        short(),
    )
    .await;
    let resp = r
        .http
        .post(format!("{}/v1/chat/completions", r.base))
        .body(r#"{"model":"A","stream":true}"#)
        .send()
        .await
        .unwrap();
    let t = Instant::now();
    let got = resp.bytes().await;
    assert!(
        t.elapsed() < Duration::from_millis(1_500),
        "idle_ms=300 must abort before the 2 s chunk"
    );
    assert!(got.is_err(), "the stalled stream must end with an error");
}

/// Verifies: REQ-PRX-017/AC1
#[tokio::test]
async fn thirty_three_mib_is_413_and_thirty_one_mib_passes() {
    let r = rig(scenario(raw(&["ok"], 0)), None, limits()).await;
    let big = |mib: usize| {
        let pad = "x".repeat(mib * 1024 * 1024);
        format!(r#"{{"model":"A","pad":"{pad}"}}"#)
    };
    let resp = r
        .http
        .post(format!("{}/v1/chat/completions", r.base))
        .body(big(33))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 413);
    let v: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(v["error"]["code"], "PAYLOAD_TOO_LARGE");
    let resp = r
        .http
        .post(format!("{}/v1/chat/completions", r.base))
        .body(big(31))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

/// Verifies: REQ-PRX-018/AC1
#[tokio::test]
async fn missing_model_is_400() {
    let r = rig(scenario(raw(&["ok"], 0)), None, limits()).await;
    let resp = r
        .http
        .post(format!("{}/v1/chat/completions", r.base))
        .body(r#"{"messages":[]}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}
