//! Error envelope and code catalog (REQ-ERR-001/002/003; Appendix B; DEC-08).
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    AuthRequired,
    AuthForbidden,
    RequestInvalid,
    PayloadTooLarge,
    EndpointNotSupported,
    ModelNotFound,
    ModelBusy,
    OracleRejected,
    RuntimeNotFound,
    RuntimeCapabilityMissing,
    RuntimeStartFailed,
    RuntimeUnhealthy,
    UpstreamTimeout,
    UpstreamStreamAborted,
    GpuOom,
    ProcessChanged,
    ProcessProtected,
    ValidationRequired,
    BenchmarkFailed,
    ShuttingDown,
    ConfigInvalid,
    Internal,
}

impl ErrorCode {
    pub const ALL: [ErrorCode; 22] = [
        Self::AuthRequired,
        Self::AuthForbidden,
        Self::RequestInvalid,
        Self::PayloadTooLarge,
        Self::EndpointNotSupported,
        Self::ModelNotFound,
        Self::ModelBusy,
        Self::OracleRejected,
        Self::RuntimeNotFound,
        Self::RuntimeCapabilityMissing,
        Self::RuntimeStartFailed,
        Self::RuntimeUnhealthy,
        Self::UpstreamTimeout,
        Self::UpstreamStreamAborted,
        Self::GpuOom,
        Self::ProcessChanged,
        Self::ProcessProtected,
        Self::ValidationRequired,
        Self::BenchmarkFailed,
        Self::ShuttingDown,
        Self::ConfigInvalid,
        Self::Internal,
    ];

    /// Appendix B status. `None` for `UPSTREAM_STREAM_ABORTED`: headers are already sent, it is
    /// only logged. `CONFIG_INVALID` is exit code 2 at startup and 422 on an admin reload.
    pub fn status(self) -> Option<StatusCode> {
        use StatusCode as S;
        Some(match self {
            Self::AuthRequired => S::UNAUTHORIZED,
            Self::AuthForbidden | Self::ProcessProtected => S::FORBIDDEN,
            Self::RequestInvalid => S::BAD_REQUEST,
            Self::PayloadTooLarge => S::PAYLOAD_TOO_LARGE,
            Self::EndpointNotSupported | Self::ModelNotFound => S::NOT_FOUND,
            Self::ModelBusy | Self::RuntimeNotFound | Self::GpuOom | Self::ShuttingDown => {
                S::SERVICE_UNAVAILABLE
            }
            Self::OracleRejected => S::INSUFFICIENT_STORAGE,
            Self::RuntimeCapabilityMissing | Self::ConfigInvalid => S::UNPROCESSABLE_ENTITY,
            Self::RuntimeStartFailed | Self::RuntimeUnhealthy => S::BAD_GATEWAY,
            Self::UpstreamTimeout => S::GATEWAY_TIMEOUT,
            Self::UpstreamStreamAborted => return None,
            Self::ProcessChanged | Self::ValidationRequired => S::CONFLICT,
            Self::BenchmarkFailed | Self::Internal => S::INTERNAL_SERVER_ERROR,
        })
    }

    pub fn kind(self) -> &'static str {
        match self {
            Self::AuthRequired => "authentication_error",
            Self::AuthForbidden | Self::ProcessProtected => "permission_error",
            Self::RequestInvalid
            | Self::PayloadTooLarge
            | Self::EndpointNotSupported
            | Self::ModelNotFound
            | Self::RuntimeCapabilityMissing
            | Self::ProcessChanged
            | Self::ValidationRequired
            | Self::ConfigInvalid => "invalid_request_error",
            Self::ModelBusy
            | Self::OracleRejected
            | Self::RuntimeNotFound
            | Self::RuntimeStartFailed
            | Self::RuntimeUnhealthy
            | Self::UpstreamTimeout
            | Self::UpstreamStreamAborted
            | Self::GpuOom
            | Self::BenchmarkFailed
            | Self::ShuttingDown
            | Self::Internal => "server_error",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::AuthRequired => "AUTH_REQUIRED",
            Self::AuthForbidden => "AUTH_FORBIDDEN",
            Self::RequestInvalid => "REQUEST_INVALID",
            Self::PayloadTooLarge => "PAYLOAD_TOO_LARGE",
            Self::EndpointNotSupported => "ENDPOINT_NOT_SUPPORTED",
            Self::ModelNotFound => "MODEL_NOT_FOUND",
            Self::ModelBusy => "MODEL_BUSY",
            Self::OracleRejected => "ORACLE_REJECTED",
            Self::RuntimeNotFound => "RUNTIME_NOT_FOUND",
            Self::RuntimeCapabilityMissing => "RUNTIME_CAPABILITY_MISSING",
            Self::RuntimeStartFailed => "RUNTIME_START_FAILED",
            Self::RuntimeUnhealthy => "RUNTIME_UNHEALTHY",
            Self::UpstreamTimeout => "UPSTREAM_TIMEOUT",
            Self::UpstreamStreamAborted => "UPSTREAM_STREAM_ABORTED",
            Self::GpuOom => "GPU_OOM",
            Self::ProcessChanged => "PROCESS_CHANGED",
            Self::ProcessProtected => "PROCESS_PROTECTED",
            Self::ValidationRequired => "VALIDATION_REQUIRED",
            Self::BenchmarkFailed => "BENCHMARK_FAILED",
            Self::ShuttingDown => "SHUTTING_DOWN",
            Self::ConfigInvalid => "CONFIG_INVALID",
            Self::Internal => "INTERNAL",
        }
    }
}

/// One daemon-generated HTTP error (REQ-ERR-001). Rendered as
/// `{"error":{"code","message","type","details","request_id"}}` with `application/json`.
#[derive(Debug, Clone)]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: String,
    pub details: Option<Value>,
    pub request_id: Option<String>,
}

impl ApiError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
            request_id: None,
        }
    }
    pub fn details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }
    pub fn request_id(mut self, id: impl Into<String>) -> Self {
        self.request_id = Some(id.into());
        self
    }
    pub fn body(&self) -> Value {
        json!({ "error": {
            "code": self.code.as_str(), "message": self.message, "type": self.code.kind(),
            "details": self.details, "request_id": self.request_id,
        }})
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.message)
    }
}
impl std::error::Error for ApiError {}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        // UPSTREAM_STREAM_ABORTED is never rendered (headers already sent); 500 is the safe fallback.
        let status = self
            .code
            .status()
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let mut resp = (
            status,
            [(header::CONTENT_TYPE, "application/json")],
            self.body().to_string(),
        )
            .into_response();
        if self.code == ErrorCode::ModelBusy {
            resp.headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from_static("1"));
        }
        resp
    }
}
