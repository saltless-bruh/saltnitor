#![allow(clippy::unwrap_used, clippy::expect_used)] // tests may unwrap: a panic is the failure signal
use ErrorCode::*;
use axum::response::IntoResponse;
use saltnitor::error::{ApiError, ErrorCode};

/// Appendix B, transcribed. Changing a row here is a spec change (REQ-ERR-002/AC1).
const APPENDIX_B: [(ErrorCode, &str, Option<u16>, &str); 22] = [
    (
        AuthRequired,
        "AUTH_REQUIRED",
        Some(401),
        "authentication_error",
    ),
    (
        AuthForbidden,
        "AUTH_FORBIDDEN",
        Some(403),
        "permission_error",
    ),
    (
        RequestInvalid,
        "REQUEST_INVALID",
        Some(400),
        "invalid_request_error",
    ),
    (
        PayloadTooLarge,
        "PAYLOAD_TOO_LARGE",
        Some(413),
        "invalid_request_error",
    ),
    (
        EndpointNotSupported,
        "ENDPOINT_NOT_SUPPORTED",
        Some(404),
        "invalid_request_error",
    ),
    (
        ModelNotFound,
        "MODEL_NOT_FOUND",
        Some(404),
        "invalid_request_error",
    ),
    (ModelBusy, "MODEL_BUSY", Some(503), "server_error"),
    (OracleRejected, "ORACLE_REJECTED", Some(507), "server_error"),
    (
        RuntimeNotFound,
        "RUNTIME_NOT_FOUND",
        Some(503),
        "server_error",
    ),
    (
        RuntimeCapabilityMissing,
        "RUNTIME_CAPABILITY_MISSING",
        Some(422),
        "invalid_request_error",
    ),
    (
        RuntimeStartFailed,
        "RUNTIME_START_FAILED",
        Some(502),
        "server_error",
    ),
    (
        RuntimeUnhealthy,
        "RUNTIME_UNHEALTHY",
        Some(502),
        "server_error",
    ),
    (
        UpstreamTimeout,
        "UPSTREAM_TIMEOUT",
        Some(504),
        "server_error",
    ),
    (
        UpstreamStreamAborted,
        "UPSTREAM_STREAM_ABORTED",
        None,
        "server_error",
    ),
    (GpuOom, "GPU_OOM", Some(503), "server_error"),
    (
        ProcessChanged,
        "PROCESS_CHANGED",
        Some(409),
        "invalid_request_error",
    ),
    (
        ProcessProtected,
        "PROCESS_PROTECTED",
        Some(403),
        "permission_error",
    ),
    (
        ValidationRequired,
        "VALIDATION_REQUIRED",
        Some(409),
        "invalid_request_error",
    ),
    (
        BenchmarkFailed,
        "BENCHMARK_FAILED",
        Some(500),
        "server_error",
    ),
    (ShuttingDown, "SHUTTING_DOWN", Some(503), "server_error"),
    (
        ConfigInvalid,
        "CONFIG_INVALID",
        Some(422),
        "invalid_request_error",
    ),
    (Internal, "INTERNAL", Some(500), "server_error"),
];

/// Verifies: REQ-ERR-002/AC1, REQ-ERR-003/AC1
#[test]
fn every_appendix_b_code_maps_to_its_status_and_type() {
    assert_eq!(ErrorCode::ALL.len(), APPENDIX_B.len());
    for (code, name, status, kind) in APPENDIX_B {
        assert!(ErrorCode::ALL.contains(&code), "{name} missing from ALL");
        assert_eq!(code.as_str(), name);
        assert_eq!(code.status().map(|s| s.as_u16()), status, "{name}");
        assert_eq!(code.kind(), kind, "{name}");
    }
}

/// Verifies: REQ-ERR-001/AC1
#[tokio::test]
async fn envelope_has_exactly_the_five_fields_and_json_content_type() {
    let resp = ApiError::new(ModelBusy, "A is loading")
        .request_id("req-1")
        .into_response();
    assert_eq!(resp.status().as_u16(), 503);
    assert_eq!(resp.headers()["content-type"], "application/json");
    assert_eq!(resp.headers()["retry-after"], "1");
    let body = axum::body::to_bytes(resp.into_body(), 1 << 16)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let e = v["error"].as_object().unwrap();
    let mut keys: Vec<_> = e.keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, ["code", "details", "message", "request_id", "type"]);
    assert_eq!(e["code"], "MODEL_BUSY");
    assert_eq!(e["type"], "server_error");
    assert_eq!(e["request_id"], "req-1");
    assert!(e["details"].is_null());
}
