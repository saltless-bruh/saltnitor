//! G1 row 2 / D1: the proxy streams. BD-02 (the baseline buffers the whole upstream body).

use std::time::{Duration, Instant};

use crate::harness::{Stack, chat_scenario, concat, read_arrivals};

/// The fake waits this long before every SSE piece, including `data: [DONE]`.
const GAP: Duration = Duration::from_millis(2000);

/// Verifies: REQ-PRX-002/AC1, REQ-PRX-002/AC2
///
/// Upstream sends `A`, waits 2 000 ms, sends `B`, waits 2 000 ms, sends `[DONE]`. The client
/// must hold `A` before `B` is even sent (t < 2 × gap), and the body must arrive in separate
/// reads rather than as one buffered blob at the end.
#[tokio::test]
async fn first_sse_chunk_reaches_the_client_before_upstream_sends_the_last() {
    let stack = Stack::with_token(chat_scenario(&["A", "B"], GAP.as_millis() as u64)).await;

    let sent_at = Instant::now();
    let resp = stack.chat(true).send().await.expect("proxy answered");
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::OK,
        "screen:\n{}",
        stack.daemon.screen()
    );
    let arrivals = read_arrivals(resp).await;
    assert!(!arrivals.is_empty(), "empty body");

    let first = &arrivals[0];
    let first_text = String::from_utf8_lossy(&first.bytes);
    assert!(
        first_text.contains("\"content\":\"A\""),
        "first chunk is not delta A: {first_text:?}"
    );
    assert!(
        !first_text.contains("[DONE]"),
        "first read already contains the terminal frame — the body was buffered: {first_text:?}"
    );

    let first_after = first.at - sent_at;
    assert!(
        first_after < 2 * GAP,
        "first chunk arrived {first_after:?} after the request; upstream sent B at ~{:?}, so the \
         proxy forwarded nothing until the body was complete",
        2 * GAP
    );

    let last_after = arrivals.last().expect("last").at - sent_at;
    assert!(
        last_after - first_after >= GAP,
        "first and last chunk arrived {:?} apart although upstream spaced them by ≥ {GAP:?}",
        last_after - first_after
    );
    let body = String::from_utf8_lossy(&concat(&arrivals)).into_owned();
    assert!(
        body.ends_with("data: [DONE]\n\n"),
        "body did not end with [DONE]: {body:?}"
    );
}
