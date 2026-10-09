//! G1 row 3: one end-to-end flow through auth, request IDs, incremental streaming and
//! cancellation (REQ-TST-014). Each step depends on the previous one succeeding.

use std::time::{Duration, Instant};

use crate::harness::{CANCEL_PROPAGATION, Stack, chat_scenario, read_arrivals};

const PIECE_GAP: Duration = Duration::from_millis(300);
const REQUEST_ID: &str = "acc.flow-0001_Z";
const ID_SHAPE: &str = "^[A-Za-z0-9._-]{1,128}$";

fn header(resp: &reqwest::Response, name: &str) -> Option<String> {
    resp.headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
}

/// Verifies: REQ-TST-014/AC1, REQ-SEC-001/AC2, REQ-PRX-009/AC1, REQ-PRX-009/AC2, REQ-PRX-002/AC1, REQ-PRX-006/AC1
///
/// 1. Without a bearer the chat route answers 401 `AUTH_REQUIRED` and nothing reaches the
///    upstream.
/// 2. With the bearer and `X-Request-Id: acc.flow-0001_Z`, the response echoes that ID, the
///    upstream saw it, and the first SSE chunk arrives before the stream ends.
/// 3. Without `X-Request-Id`, the response carries a generated ID of the required shape that
///    differs from the one we sent.
/// 4. Dropping that second stream mid-way makes the fake record a `disconnect` within
///    `cancel_propagation_ms` plus one piece gap.
#[tokio::test]
async fn auth_then_request_id_then_streaming_then_cancellation_in_one_flow() {
    let items: Vec<&str> = std::iter::repeat_n("tok", 20).collect();
    let stack = Stack::with_token(chat_scenario(&items, PIECE_GAP.as_millis() as u64)).await;

    // 1. auth
    let resp = stack
        .client
        .post(stack.url("/v1/chat/completions"))
        .json(&crate::harness::chat_body(true))
        .send()
        .await
        .expect("unauthenticated request answered");
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::UNAUTHORIZED,
        "step 1: status"
    );
    let v: serde_json::Value = resp.json().await.expect("step 1: JSON envelope");
    assert_eq!(v["error"]["code"], "AUTH_REQUIRED", "step 1: envelope {v}");
    assert!(
        stack.fake.recorder.of_kind("request").is_empty(),
        "step 1: the unauthenticated request reached the upstream"
    );

    // 2. request ID echoed + incremental streaming
    let sent_at = Instant::now();
    let resp = stack
        .chat(true)
        .header("X-Request-Id", REQUEST_ID)
        .send()
        .await
        .expect("authenticated request answered");
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::OK,
        "step 2: status\n{}",
        stack.daemon.screen()
    );
    assert_eq!(
        header(&resp, "x-request-id").as_deref(),
        Some(REQUEST_ID),
        "step 2: X-Request-Id was not echoed"
    );
    let arrivals = read_arrivals(resp).await;
    let first_after = arrivals.first().expect("step 2: body").at - sent_at;
    let last_after = arrivals.last().expect("step 2: body").at - sent_at;
    assert!(
        last_after - first_after >= PIECE_GAP,
        "step 2: first and last chunk arrived {:?} apart; the body was buffered",
        last_after - first_after
    );
    let upstream_requests = stack.fake.recorder.of_kind("request");
    assert_eq!(
        upstream_requests.len(),
        1,
        "step 2: upstream requests {upstream_requests:?}"
    );
    assert_eq!(
        upstream_requests[0]["headers"]["x-request-id"], REQUEST_ID,
        "step 2: the request ID was not forwarded upstream: {upstream_requests:?}"
    );

    // 3. generated request ID
    let mut resp = stack
        .chat(true)
        .send()
        .await
        .expect("second authenticated request");
    assert_eq!(resp.status(), reqwest::StatusCode::OK, "step 3: status");
    let generated = header(&resp, "x-request-id").expect("step 3: no X-Request-Id generated");
    assert!(
        regex::Regex::new(ID_SHAPE)
            .expect("regex")
            .is_match(&generated),
        "step 3: generated ID {generated:?} does not match {ID_SHAPE}"
    );
    assert_ne!(
        generated, REQUEST_ID,
        "step 3: generated ID reused the previous one"
    );

    // 4. cancellation
    let _ = tokio::time::timeout(Duration::from_millis(1500), resp.chunk()).await;
    drop(resp);
    assert!(
        stack
            .wait_for_disconnect(CANCEL_PROPAGATION + PIECE_GAP)
            .await,
        "step 4: no `disconnect` recorded within {:?}; events: {:?}",
        CANCEL_PROPAGATION + PIECE_GAP,
        stack.fake.recorder.events()
    );
}
