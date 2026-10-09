//! G1 row 2: client cancellation propagates upstream (REQ-PRX-006, BD-23's P1 half).

use std::time::Duration;

use crate::harness::{CANCEL_PROPAGATION, Stack, chat_scenario};

/// Gap between upstream pieces. The fake only notices a dropped downstream when its next
/// send fails, so this is the observation granularity added to `cancel_propagation_ms`.
const PIECE_GAP: Duration = Duration::from_millis(100);

/// Verifies: REQ-PRX-006/AC1
///
/// Sixty pieces, 100 ms apart (6 s of upstream streaming). The client takes the first chunk
/// (or gives up waiting after 1.5 s if the proxy buffers), then drops the connection. The
/// fake must record a `disconnect` within `cancel_propagation_ms` plus one piece gap.
#[tokio::test]
async fn dropping_the_client_mid_stream_closes_the_upstream_request_within_budget() {
    let items: Vec<&str> = std::iter::repeat_n("x", 60).collect();
    let stack = Stack::with_token(chat_scenario(&items, PIECE_GAP.as_millis() as u64)).await;

    let mut resp = stack.chat(true).send().await.expect("proxy answered");
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::OK,
        "screen:\n{}",
        stack.daemon.screen()
    );
    // Wait for the first chunk, but never longer than 1.5 s: a buffering proxy never
    // delivers one, and this test is about cancellation, not streaming.
    let _ = tokio::time::timeout(Duration::from_millis(1500), resp.chunk()).await;
    assert!(
        stack.fake.recorder.of_kind("disconnect").is_empty(),
        "upstream saw a disconnect before the client dropped"
    );

    drop(resp);

    let saw = stack
        .wait_for_disconnect(CANCEL_PROPAGATION + PIECE_GAP)
        .await;
    assert!(
        saw,
        "no `disconnect` recorded by the fake within {:?} of the client dropping; events: {:?}",
        CANCEL_PROPAGATION + PIECE_GAP,
        stack.fake.recorder.events()
    );
}
