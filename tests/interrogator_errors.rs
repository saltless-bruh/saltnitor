#![allow(clippy::unwrap_used, clippy::expect_used)] // tests may unwrap: a panic is the failure signal
//! The interrogator reports why a strike failed instead of fabricating a status (INV-18,
//! REQ-ERR-005/AC1, REQ-TUI-006/AC1).
use axum::{Router, http::StatusCode, routing::post};
use saltnitor::events::Event;
use saltnitor::interrogate::strike;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

async fn run(port: u16, payload: &str) -> Vec<Event> {
    let (tx, mut rx) = mpsc::channel(64);
    strike(port, Some("k".into()), payload.to_string(), tx).await;
    let mut out = vec![];
    while let Some(e) = rx.recv().await {
        out.push(e);
    }
    out
}
fn error_of(events: &[Event]) -> Option<(&str, &str)> {
    events.iter().find_map(|e| match e {
        Event::Error { source, message } => Some((source.as_str(), message.as_str())),
        _ => None,
    })
}
fn end_status(events: &[Event]) -> Option<&str> {
    events.iter().find_map(|e| match e {
        Event::ApiStreamEnd { status, .. } => Some(status.as_str()),
        _ => None,
    })
}
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Verifies: REQ-TUI-006/AC1, REQ-ERR-005/AC1 — a transport failure is an error event and status n/a, never "500"
#[tokio::test]
async fn a_connect_failure_is_reported_with_status_na() {
    let events = run(free_port(), r#"{"messages":[]}"#).await;
    let (source, message) = error_of(&events).expect("an Event::Error");
    assert_eq!(source, "interrogator");
    assert!(message.contains("cannot reach"), "{message}");
    assert_eq!(end_status(&events), Some("n/a"));
}

/// Verifies: REQ-TUI-006/AC1, REQ-ERR-005/AC1 — the error envelope of a non-2xx answer reaches the operator
#[tokio::test]
async fn a_401_envelope_is_surfaced_as_code_and_message() {
    let app = Router::new().route(
        "/v1/chat/completions",
        post(|| async {
            (
                StatusCode::UNAUTHORIZED,
                [("content-type", "application/json")],
                r#"{"error":{"message":"missing bearer token","type":"authentication_error","code":"UNAUTHENTICATED","request_id":"r1"}}"#,
            )
        }),
    );
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(l, app).await });
    let events = run(port, r#"{"messages":[]}"#).await;
    let (_, message) = error_of(&events).expect("an Event::Error");
    assert!(
        message.contains("UNAUTHENTICATED: missing bearer token"),
        "{message}"
    );
    assert!(end_status(&events).unwrap().starts_with("401"));
}

/// Verifies: REQ-TUI-006/AC1, REQ-ERR-005/AC1 — a stream that dies mid-body is reported, not shown as a normal end
#[tokio::test]
async fn a_mid_stream_abort_is_reported() {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(async move {
        let (mut s, _) = l.accept().await.unwrap();
        let mut buf = [0u8; 4096];
        let _ = tokio::io::AsyncReadExt::read(&mut s, &mut buf).await;
        let head = "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\n\r\n";
        s.write_all(head.as_bytes()).await.unwrap();
        s.write_all(b"6\r\ndata: \r\n").await.unwrap();
        s.flush().await.unwrap();
        // Close without the terminating 0-chunk: the body is truncated.
        drop(s);
    });
    let events = run(port, r#"{"messages":[]}"#).await;
    let (_, message) = error_of(&events).expect("an Event::Error for the aborted stream");
    assert!(message.contains("stream"), "{message}");
    assert!(
        end_status(&events).unwrap().contains("aborted"),
        "{:?}",
        end_status(&events)
    );
}

/// Verifies: REQ-ERR-005/AC1 — invalid JSON typed in the console is reported locally and never sent as `{}`
#[tokio::test]
async fn invalid_json_is_rejected_locally_without_a_request() {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    let hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let h = hits.clone();
    tokio::spawn(async move {
        if l.accept().await.is_ok() {
            h.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    });
    let events = run(port, "{not json").await;
    let (_, message) = error_of(&events).expect("an Event::Error");
    assert!(message.contains("payload is not valid JSON"), "{message}");
    assert_eq!(end_status(&events), Some("n/a"));
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert_eq!(
        hits.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "nothing was sent"
    );
}

/// Verifies: REQ-ERR-005/AC1 — a history file that cannot be written is an error, not a silent loss
#[test]
fn a_history_write_failure_is_reported_and_a_success_creates_the_directory() {
    use saltnitor::interrogate::save_history;
    let d = tempfile::tempdir().unwrap();
    let ok = d.path().join("state/saltnitor/history");
    save_history(&ok, "a\nb").unwrap();
    assert_eq!(std::fs::read_to_string(&ok).unwrap(), "a\nb");

    // The parent "directory" is a regular file: neither create_dir_all nor write can succeed.
    let blocker = d.path().join("blocker");
    std::fs::write(&blocker, "x").unwrap();
    let e = save_history(&blocker.join("history"), "a").unwrap_err();
    assert!(e.contains("history"), "{e}");
}
