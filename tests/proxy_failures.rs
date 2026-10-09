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

/// Verifies: REQ-PRX-008/AC2 — the connection dies before any response header was written.
/// The fake cannot do this (its faults always write headers), so the runtime here is a bare
/// listener: it reports model A as loaded, then closes the socket on the chat request.
#[tokio::test]
async fn a_connection_closed_before_headers_is_502_runtime_unhealthy() {
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
                }
                // Anything else (the chat request): close without writing a byte.
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
        .limits(short()),
    );
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    tokio::spawn(serve(api, addr));
    tokio::time::sleep(Duration::from_millis(50)).await;
    let resp = reqwest::Client::new()
        .post(format!("http://{addr}/v1/chat/completions"))
        .body(r#"{"model":"A"}"#)
        .send()
        .await
        .unwrap();
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

/// Verifies: REQ-PRX-011/AC1, REQ-PRX-010/AC1 (idle timeout)
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
    let got = resp.bytes().await; // reqwest reports the truncation as an error
    let text = match got {
        Ok(b) => String::from_utf8_lossy(&b).into_owned(),
        Err(_) => String::new(),
    };
    assert!(!text.contains("[DONE]"), "no synthetic [DONE]: {text}");
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
    // idle timeout: a stream that stalls longer than idle_ms is aborted the same way
    let r2 = rig(
        scenario(raw(&["data: a\n\n", "data: b\n\n"], 2_000)),
        None,
        short(),
    )
    .await;
    let resp = r2
        .http
        .post(format!("{}/v1/chat/completions", r2.base))
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
    assert!(got.map(|b| !b.ends_with(b"data: b\n\n")).unwrap_or(true));
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
