//! G1 row 2: authentication is enforced centrally (Appendix A policy, Appendix B envelope).
//! BD-03 (per-handler auth, some routes open) and BD-04 (query token accepted by default).

use crate::harness::{MODEL, Stack, TOKEN, chat_body, chat_scenario};

/// Every protected route of Appendix A that exists in P1, with the request that reaches it.
/// `/v1/ensure/stream` is GET-only until P6, so only GET is listed.
fn protected_routes(stack: &Stack) -> Vec<(&'static str, reqwest::RequestBuilder)> {
    let c = &stack.client;
    vec![
        ("GET /v1/models", c.get(stack.url("/v1/models"))),
        ("GET /v1/status", c.get(stack.url("/v1/status"))),
        (
            "POST /v1/chat/completions",
            c.post(stack.url("/v1/chat/completions"))
                .json(&chat_body(false)),
        ),
        (
            "POST /v1/ensure",
            c.post(stack.url("/v1/ensure"))
                .json(&serde_json::json!({ "profile": MODEL })),
        ),
        (
            "GET /v1/ensure/stream",
            c.get(stack.url(&format!("/v1/ensure/stream?profile={MODEL}"))),
        ),
    ]
}

/// Checks one response against the 401 contract: status, JSON body, and the Appendix B
/// envelope `{"error":{"code":"AUTH_REQUIRED","type":"authentication_error",…}}`.
async fn auth_required_problems(route: &str, resp: reqwest::Response) -> Vec<String> {
    let mut problems = Vec::new();
    let status = resp.status();
    let ctype = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let text = resp.text().await.unwrap_or_default();
    if status != reqwest::StatusCode::UNAUTHORIZED {
        problems.push(format!(
            "{route}: status {status}, expected 401 (body {text:?})"
        ));
    }
    if !ctype.starts_with("application/json") {
        problems.push(format!(
            "{route}: content-type {ctype:?}, expected application/json"
        ));
    }
    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(v) => {
            if v["error"]["code"] != "AUTH_REQUIRED" {
                problems.push(format!(
                    "{route}: error.code is not AUTH_REQUIRED in {text:?}"
                ));
            }
            if v["error"]["type"] != "authentication_error" {
                problems.push(format!(
                    "{route}: error.type is not authentication_error in {text:?}"
                ));
            }
        }
        Err(_) => problems.push(format!("{route}: body is not JSON: {text:?}")),
    }
    problems
}

/// Verifies: REQ-SEC-001/AC2
///
/// `/healthz` has policy `none`: it answers 200 without credentials. This test also proves
/// the harness itself (binary in a pty, throw-away `$HOME`, free port) against today's code.
#[tokio::test]
async fn healthz_answers_200_without_credentials() {
    let stack = Stack::with_token(chat_scenario(&["x"], 0)).await;
    let resp = stack
        .client
        .get(stack.url("/healthz"))
        .send()
        .await
        .expect("healthz");
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
}

/// Verifies: REQ-SEC-001/AC2, REQ-SEC-002/AC3
///
/// With `control_token` configured, every protected route of Appendix A refuses a request
/// that carries no bearer with 401 and the `AUTH_REQUIRED` envelope. All routes are checked
/// before the test fails so the report lists every offender.
#[tokio::test]
async fn every_protected_route_returns_401_envelope_without_a_bearer() {
    let stack = Stack::with_token(chat_scenario(&["x"], 0)).await;
    let mut problems = Vec::new();
    for (route, req) in protected_routes(&stack) {
        let resp = req.send().await.unwrap_or_else(|e| panic!("{route}: {e}"));
        problems.extend(auth_required_problems(route, resp).await);
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    assert!(
        stack.fake.recorder.of_kind("request").is_empty(),
        "an unauthenticated request reached the upstream: {:?}",
        stack.fake.recorder.of_kind("request")
    );
}

/// Verifies: REQ-SEC-001/AC2
///
/// Invalid credentials are refused the same way as missing ones (Appendix B: "missing or
/// invalid credentials").
#[tokio::test]
async fn every_protected_route_returns_401_envelope_with_a_wrong_bearer() {
    let stack = Stack::with_token(chat_scenario(&["x"], 0)).await;
    let mut problems = Vec::new();
    for (route, req) in protected_routes(&stack) {
        let resp = req
            .bearer_auth("not-the-token")
            .send()
            .await
            .unwrap_or_else(|e| panic!("{route}: {e}"));
        problems.extend(auth_required_problems(route, resp).await);
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Verifies: REQ-SEC-005/AC1
///
/// `Authorization: Bearer <control_token>` is accepted on a protected route.
#[tokio::test]
async fn a_valid_bearer_is_accepted() {
    let stack = Stack::with_token(chat_scenario(&["x"], 0)).await;
    let resp = stack
        .client
        .get(stack.url("/v1/models"))
        .bearer_auth(TOKEN)
        .send()
        .await
        .expect("models");
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let v: serde_json::Value = resp.json().await.expect("json");
    assert_eq!(v["data"][0]["id"], MODEL, "model list: {v}");
}

/// Verifies: REQ-SEC-005/AC2
///
/// `?token=<the token>` on `GET /v1/ensure/stream` with no header is ignored by default
/// (`security.allow_query_token` unset), so the request gets the 401 envelope.
#[tokio::test]
async fn query_token_on_ensure_stream_is_refused_by_default() {
    let stack = Stack::with_token(chat_scenario(&["x"], 0)).await;
    let resp = stack
        .client
        .get(stack.url(&format!("/v1/ensure/stream?profile={MODEL}&token={TOKEN}")))
        .send()
        .await
        .expect("ensure/stream");
    let problems = auth_required_problems("GET /v1/ensure/stream?token=", resp).await;
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
