#![allow(clippy::unwrap_used, clippy::expect_used)] // tests may unwrap: a panic is the failure signal
//! Every failure on `/v1/ensure*` and the fallback is an error envelope that carries the request
//! id, including the ones axum's extractors would otherwise answer in plain text
//! (REQ-ERR-001/AC1, REQ-PRX-009/AC2).
mod common;
use common::*;
use serde_json::Value;

async fn envelope(resp: reqwest::Response) -> (u16, Value, String) {
    let status = resp.status().as_u16();
    assert_eq!(
        resp.headers()["content-type"],
        "application/json",
        "status {status} must be a JSON envelope"
    );
    let header_id = resp.headers()["x-request-id"].to_str().unwrap().to_string();
    (status, resp.json().await.unwrap(), header_id)
}

/// Verifies: REQ-ERR-001/AC1, REQ-PRX-009/AC2 — malformed `/v1/ensure` requests are REQUEST_INVALID envelopes
#[tokio::test]
async fn malformed_ensure_posts_are_json_envelopes_with_the_request_id() {
    let r = rig(scenario(raw(&["ok"], 0)), None, limits()).await;
    let url = format!("{}/v1/ensure", r.base);
    let cases = [
        (
            "invalid JSON",
            r.http
                .post(&url)
                .header("content-type", "application/json")
                .body("{nope"),
        ),
        (
            "missing content-type",
            r.http.post(&url).body(r#"{"profile":"A"}"#),
        ),
        (
            "missing profile",
            r.http
                .post(&url)
                .header("content-type", "application/json")
                .body("{}"),
        ),
        (
            "wrong field type",
            r.http
                .post(&url)
                .header("content-type", "application/json")
                .body(r#"{"profile":3}"#),
        ),
    ];
    for (what, req) in cases {
        let (status, v, header_id) = envelope(req.send().await.unwrap()).await;
        assert_eq!(status, 400, "{what}");
        assert_eq!(v["error"]["code"], "REQUEST_INVALID", "{what}");
        assert_eq!(
            v["error"]["request_id"],
            header_id.as_str(),
            "{what}: body id == header id"
        );
    }
}

/// Verifies: REQ-ERR-001/AC1, REQ-PRX-017/AC1 — an over-limit `/v1/ensure` body is a PAYLOAD_TOO_LARGE envelope
#[tokio::test]
async fn an_oversized_ensure_body_is_payload_too_large() {
    let r = rig(scenario(raw(&["ok"], 0)), None, limits()).await;
    let pad = "x".repeat(3 * 1024 * 1024);
    let resp = r
        .http
        .post(format!("{}/v1/ensure", r.base))
        .header("content-type", "application/json")
        .body(format!(r#"{{"profile":"A","pad":"{pad}"}}"#))
        .send()
        .await
        .unwrap();
    let (status, v, header_id) = envelope(resp).await;
    assert_eq!(status, 413);
    assert_eq!(v["error"]["code"], "PAYLOAD_TOO_LARGE");
    assert_eq!(v["error"]["request_id"], header_id.as_str());
}

/// Verifies: REQ-ERR-001/AC1, REQ-PRX-009/AC2 — a stream request without `profile` is an envelope, not text
#[tokio::test]
async fn ensure_stream_without_a_profile_is_a_json_envelope() {
    let r = rig(scenario(raw(&["ok"], 0)), None, limits()).await;
    let resp = r
        .http
        .get(format!("{}/v1/ensure/stream", r.base))
        .send()
        .await
        .unwrap();
    let (status, v, header_id) = envelope(resp).await;
    assert_eq!(status, 400);
    assert_eq!(v["error"]["code"], "REQUEST_INVALID");
    assert_eq!(v["error"]["request_id"], header_id.as_str());
}

/// Verifies: REQ-PRX-009/AC2 — envelopes built by the ensure handlers and the fallback carry the id in the body
#[tokio::test]
async fn ensure_and_fallback_envelopes_carry_the_request_id_in_the_body() {
    let r = rig(scenario(raw(&["ok"], 0)), None, limits()).await;
    let resp = r
        .http
        .post(format!("{}/v1/ensure", r.base))
        .json(&serde_json::json!({"profile": "no-such-profile"}))
        .send()
        .await
        .unwrap();
    let (status, v, header_id) = envelope(resp).await;
    assert_eq!(status, 404);
    assert_eq!(v["error"]["code"], "MODEL_NOT_FOUND");
    assert_eq!(v["error"]["request_id"], header_id.as_str());

    let resp = r
        .http
        .get(format!("{}/v1/embeddings", r.base))
        .send()
        .await
        .unwrap();
    let (status, v, header_id) = envelope(resp).await;
    assert_eq!(status, 404);
    assert_eq!(v["error"]["code"], "ENDPOINT_NOT_SUPPORTED");
    assert_eq!(v["error"]["request_id"], header_id.as_str());
}
