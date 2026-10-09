//! Streaming proxy core (REQ-PRX-002…005, REQ-PRX-008…011, REQ-PRX-017/018, REQ-SEC-012; INV-01,
//! INV-08). Bytes in, bytes out: the body is never parsed into a value, never re-serialized, and
//! never buffered (no whole-body read helper of the HTTP client is called in this file —
//! REQ-PRX-002/AC3, mechanised by `tests/proxy_streaming.rs`).
use crate::config_v1::{DEFAULT_MAX_BODY_BYTES, Timeouts};
use crate::error::{ApiError, ErrorCode};
use axum::body::{Body, Bytes};
use axum::extract::Request;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use std::future::Future;
use std::pin::Pin;
use std::sync::LazyLock;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio_stream::Stream;

#[derive(Debug, Clone, Copy)]
pub struct ProxyLimits {
    pub connect: Duration,
    pub first_byte: Duration,
    pub idle: Duration,
    pub max_body_bytes: u64,
}
impl ProxyLimits {
    pub fn from_config(t: &Timeouts, max_body_bytes: Option<u64>) -> Self {
        Self {
            connect: Duration::from_millis(t.connect_ms),
            first_byte: Duration::from_millis(t.first_byte_ms),
            idle: Duration::from_millis(t.idle_ms),
            max_body_bytes: max_body_bytes.unwrap_or(DEFAULT_MAX_BODY_BYTES),
        }
    }
}
impl Default for ProxyLimits {
    fn default() -> Self {
        Self::from_config(&Timeouts::default(), None)
    }
}

/// The upstream client: only the connect timeout is client-wide. A client-level read timeout would
/// also bound the wait for response headers, making `first_byte_ms` unreachable; the idle gap
/// between body chunks is enforced per chunk by `AbortOnError` instead (REQ-PRX-010/AC1).
///
/// The runtime is a loopback service (INV-06): `no_proxy()` keeps a stray `HTTP_PROXY` in the
/// daemon's environment from diverting inference traffic. A build failure is an error, never a
/// silently different client without the connect timeout.
pub fn upstream_client(l: &ProxyLimits) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .no_proxy()
        .connect_timeout(l.connect)
        .build()
        .map_err(|e| format!("cannot build the upstream HTTP client: {e}"))
}

/// RFC 9110 §7.6.1 (REQ-PRX-004/AC2).
pub const HOP_BY_HOP: [&str; 7] = [
    "connection",
    "keep-alive",
    "proxy-connection",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
];
/// Never forwarded upstream (REQ-SEC-012/AC1).
pub const CLIENT_CREDENTIALS: [&str; 3] = ["authorization", "cookie", "proxy-authorization"];

pub fn is_hop_by_hop(name: &str, connection_header: Option<&str>) -> bool {
    let n = name.to_ascii_lowercase();
    HOP_BY_HOP.contains(&n.as_str())
        || connection_header
            .is_some_and(|c| c.split(',').any(|t| t.trim().eq_ignore_ascii_case(&n)))
}

fn connection_value(h: &HeaderMap) -> Option<&str> {
    h.get(header::CONNECTION).and_then(|v| v.to_str().ok())
}

/// Request headers that go upstream: end-to-end only, minus credentials and the ones we own.
pub fn forwardable_request_headers(h: &HeaderMap) -> Vec<(HeaderName, HeaderValue)> {
    let conn = connection_value(h);
    h.iter()
        .filter(|(n, _)| {
            let s = n.as_str();
            !is_hop_by_hop(s, conn)
                && !CLIENT_CREDENTIALS.contains(&s)
                && s != "host"
                && s != "content-length"
                && s != "x-request-id"
        })
        .map(|(n, v)| (n.clone(), v.clone()))
        .collect()
}

/// Response headers that go downstream: end-to-end only (status is copied separately). The
/// proxy owns `X-Request-Id` (REQ-PRX-009), so an upstream one is dropped, never duplicated.
pub fn forwardable_response_headers(h: &HeaderMap) -> Vec<(HeaderName, HeaderValue)> {
    let conn = connection_value(h);
    h.iter()
        .filter(|(n, _)| !is_hop_by_hop(n.as_str(), conn) && n.as_str() != "x-request-id")
        .map(|(n, v)| (n.clone(), v.clone()))
        .collect()
}

/// Only the top-level `model` of a JSON *object*. Deserialized through `deserialize_map` so a
/// sequence (`["A"]`) or scalar is an error, which a derived `Deserialize` would not give.
struct ModelProbe {
    model: Option<serde_json::Value>,
}
impl<'de> Deserialize<'de> for ModelProbe {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = ModelProbe;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a JSON object")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut m: A,
            ) -> Result<Self::Value, A::Error> {
                let mut model = None;
                while let Some(key) = m.next_key::<String>()? {
                    if key == "model" {
                        model = Some(m.next_value::<serde_json::Value>()?);
                    } else {
                        m.next_value::<serde::de::IgnoredAny>()?;
                    }
                }
                // `"model": null` is present-but-not-a-string, same as any other non-string.
                Ok(ModelProbe { model })
            }
        }
        d.deserialize_map(V)
    }
}

pub struct Validated {
    pub model: String,
}

/// A JSON object with a string `model` (REQ-PRX-018/AC1). The bytes are only inspected, never
/// re-serialized (AC2): the caller forwards the original buffer.
pub fn validate_chat_body(body: &[u8]) -> Result<Validated, ApiError> {
    let probe: ModelProbe = serde_json::from_slice(body).map_err(|e| {
        ApiError::new(
            ErrorCode::RequestInvalid,
            format!("body must be a JSON object: {e}"),
        )
    })?;
    match probe.model {
        Some(serde_json::Value::String(m)) if !m.is_empty() => Ok(Validated { model: m }),
        Some(_) => Err(ApiError::new(
            ErrorCode::RequestInvalid,
            "'model' must be a non-empty string",
        )),
        None => Err(ApiError::new(
            ErrorCode::RequestInvalid,
            "missing 'model' in request body",
        )),
    }
}

/// Read at most `max` bytes (REQ-PRX-017/AC1). A declared `Content-Length` above the limit is
/// refused before reading anything.
pub async fn read_body_limited(
    body: Body,
    content_length: Option<u64>,
    max: u64,
) -> Result<Bytes, ApiError> {
    let too_large = || {
        ApiError::new(
            ErrorCode::PayloadTooLarge,
            format!("request body exceeds max_body_bytes ({max})"),
        )
    };
    if content_length.is_some_and(|n| n > max) {
        return Err(too_large());
    }
    let limit = usize::try_from(max).unwrap_or(usize::MAX);
    axum::body::to_bytes(body, limit)
        .await
        .map_err(|_| too_large())
}

pub fn content_length(h: &HeaderMap) -> Option<u64> {
    h.get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
}

/// Ends the downstream body with an error (hyper aborts the connection) the moment the upstream
/// body fails or stalls longer than `idle`: no synthetic `[DONE]`, nothing fabricated
/// (REQ-PRX-011/AC1). The idle deadline is re-armed on every chunk (REQ-PRX-010/AC1).
struct AbortOnError {
    inner: Pin<Box<dyn Stream<Item = reqwest::Result<Bytes>> + Send>>,
    request_id: String,
    log: Box<dyn Fn(String) + Send + Sync>,
    idle: Duration,
    deadline: Pin<Box<tokio::time::Sleep>>,
    failed: bool,
}
impl AbortOnError {
    fn new(
        inner: Pin<Box<dyn Stream<Item = reqwest::Result<Bytes>> + Send>>,
        request_id: &str,
        log: Box<dyn Fn(String) + Send + Sync>,
        idle: Duration,
    ) -> Self {
        Self {
            inner,
            request_id: request_id.to_string(),
            log,
            idle,
            deadline: Box::pin(tokio::time::sleep(idle)),
            failed: false,
        }
    }
    fn rearm(&mut self) {
        let now = tokio::time::Instant::now();
        // An absurdly large configured idle must not overflow the clock: treat it as "never".
        let at = now
            .checked_add(self.idle)
            .unwrap_or_else(|| now + Duration::from_secs(86_400 * 365));
        self.deadline.as_mut().reset(at);
    }
    fn abort(&mut self, kind: &str) -> Poll<Option<Result<Bytes, std::io::Error>>> {
        self.failed = true;
        (self.log)(format!(
            "UPSTREAM_STREAM_ABORTED request_id={} {kind}",
            self.request_id
        ));
        Poll::Ready(Some(Err(std::io::Error::other("upstream stream aborted"))))
    }
}
impl Stream for AbortOnError {
    type Item = Result<Bytes, std::io::Error>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.failed {
            return Poll::Ready(None);
        }
        match self.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(b))) => {
                self.rearm();
                Poll::Ready(Some(Ok(b)))
            }
            Poll::Ready(Some(Err(_))) => self.abort("upstream failed mid-stream"),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => {
                if self.deadline.as_mut().poll(cx).is_ready() {
                    self.abort("idle timeout between chunks")
                } else {
                    Poll::Pending
                }
            }
        }
    }
}

/// Forward `body` to `url` and stream the reply back (REQ-PRX-002/003/004/005/008/010).
#[expect(
    clippy::too_many_arguments,
    reason = "one call site; every argument is a distinct input of the forward step"
)]
pub async fn forward(
    client: &reqwest::Client,
    url: &str,
    upstream_bearer: Option<&str>,
    req_headers: &HeaderMap,
    body: Bytes,
    request_id: &str,
    limits: &ProxyLimits,
    log: impl Fn(String) + Send + Sync + 'static,
) -> Response {
    let mut rb = client
        .post(url)
        .body(body)
        .header("x-request-id", request_id);
    for (n, v) in forwardable_request_headers(req_headers) {
        rb = rb.header(n, v);
    }
    if let Some(b) = upstream_bearer {
        rb = rb.header(header::AUTHORIZATION, format!("Bearer {b}"));
    }
    let upstream = match tokio::time::timeout(limits.first_byte, rb.send()).await {
        Err(_) => {
            return ApiError::new(
                ErrorCode::UpstreamTimeout,
                "no response headers from the runtime within first_byte_ms",
            )
            .request_id(request_id)
            .into_response();
        }
        Ok(Err(e)) if e.is_timeout() => {
            return ApiError::new(
                ErrorCode::UpstreamTimeout,
                "the runtime did not answer within the configured timeout",
            )
            .request_id(request_id)
            .into_response();
        }
        Ok(Err(e)) => {
            return ApiError::new(
                ErrorCode::RuntimeUnhealthy,
                format!(
                    "runtime connection failed before response headers: {}",
                    e.without_url()
                ),
            )
            .request_id(request_id)
            .into_response();
        }
        Ok(Ok(r)) => r,
    };
    let status =
        StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut b = Response::builder().status(status);
    for (n, v) in forwardable_response_headers(upstream.headers()) {
        b = b.header(n, v);
    }
    b = b.header("x-request-id", request_id);
    let stream = AbortOnError::new(
        Box::pin(upstream.bytes_stream()),
        request_id,
        Box::new(log),
        limits.idle,
    );
    b.body(Body::from_stream(stream)).unwrap_or_else(|e| {
        ApiError::new(
            ErrorCode::RuntimeUnhealthy,
            format!("could not relay the runtime response: {e}"),
        )
        .request_id(request_id)
        .into_response()
    })
}

// ── Request IDs (REQ-PRX-009) ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RequestId(pub String);

static ID_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"^[A-Za-z0-9._-]{1,128}$").unwrap_or_else(|_| unreachable!("static regex"))
});

/// Keep a well-formed incoming id; otherwise mint a UUIDv7.
pub fn request_id_for(incoming: Option<&str>) -> String {
    match incoming {
        Some(s) if ID_RE.is_match(s) => s.to_string(),
        _ => uuid::Uuid::now_v7().to_string(),
    }
}

/// Outermost layer: assigns the id, exposes it as an extension, echoes it on every response.
pub async fn request_id_middleware(mut req: Request, next: Next) -> Response {
    let id = request_id_for(
        req.headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok()),
    );
    if let Ok(v) = HeaderValue::from_str(&id) {
        req.headers_mut().insert("x-request-id", v.clone());
        req.extensions_mut().insert(RequestId(id));
        let mut resp = next.run(req).await;
        resp.headers_mut().entry("x-request-id").or_insert(v);
        return resp;
    }
    next.run(req).await
}
