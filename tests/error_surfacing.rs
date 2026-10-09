#![allow(clippy::unwrap_used, clippy::expect_used)] // tests may unwrap: a panic is the failure signal
use saltnitor::control_api::{ControlApi, serve};
use saltnitor::events::Event;
use std::sync::Arc;

/// Verifies: REQ-ERR-005/AC1 (bind failure is a visible event, not an eprintln behind the TUI)
#[tokio::test]
async fn occupied_control_port_produces_an_error_event() {
    let holder = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = holder.local_addr().unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::channel(8);
    let api = Arc::new(ControlApi::new(
        Default::default(),
        "http://127.0.0.1:1".into(),
        None,
        None,
        0.0,
        0.0,
        tx,
    ));
    serve(api, addr).await; // returns once binding failed
    let ev = rx.recv().await.unwrap();
    match ev {
        Event::Error { source, message } => {
            assert_eq!(source, "control_api");
            assert!(message.contains(&addr.to_string()) && message.contains("bind"));
        }
        other => panic!("expected Event::Error, got {other:?}"),
    }
}
