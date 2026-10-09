//! One authentication layer for the whole router, driven by the Appendix A policy table
//! (REQ-SEC-001/002/004/005/011/015, REQ-PRX-016). Handlers contain no auth logic.
use crate::error::{ApiError, ErrorCode};
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{Method, Uri, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use subtle::ConstantTimeEq;

/// Removes known secret values and credential-shaped substrings from a log line (REQ-SEC-006/AC1).
#[derive(Debug, Clone, Default)]
pub struct Redactor {
    secrets: Vec<String>,
    bearer: Option<regex::Regex>,
    query: Option<regex::Regex>,
}
impl Redactor {
    pub fn new(secrets: Vec<String>) -> Self {
        Self {
            secrets: secrets.into_iter().filter(|s| !s.is_empty()).collect(),
            bearer: regex::Regex::new(r"(?i)(bearer\s+)[A-Za-z0-9._~+/=-]+").ok(),
            query: regex::Regex::new(r"([?&]token=)[^&\s]+").ok(),
        }
    }
    pub fn redact(&self, line: &str) -> String {
        let mut out = line.to_string();
        for s in &self.secrets {
            out = out.replace(s, "<redacted>");
        }
        if let Some(re) = &self.bearer {
            out = re.replace_all(&out, "${1}<redacted>").into_owned();
        }
        if let Some(re) = &self.query {
            out = re.replace_all(&out, "${1}<redacted>").into_owned();
        }
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Auth {
    Public,
    Required,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    None,
    Inference,
    Admin,
}

#[derive(Debug, Clone, Copy)]
pub struct RoutePolicy {
    pub path: &'static str,
    pub methods: &'static [Method],
    pub auth: Auth,
    pub scope: Scope,
    /// `?token=` accepted only here, only on GET, only when `allow_query_token` (REQ-SEC-005/AC3).
    pub query_token_compat: bool,
}

/// Appendix A, the P1 subset. A route exists only if it is listed here AND in
/// `control_api::HANDLER_PATHS`; `tests/auth_policy.rs` fails otherwise (REQ-SEC-001/AC3).
pub const POLICY: &[RoutePolicy] = &[
    RoutePolicy {
        path: "/healthz",
        methods: &[Method::GET],
        auth: Auth::Public,
        scope: Scope::None,
        query_token_compat: false,
    },
    RoutePolicy {
        path: "/v1/models",
        methods: &[Method::GET],
        auth: Auth::Required,
        scope: Scope::Inference,
        query_token_compat: false,
    },
    RoutePolicy {
        path: "/v1/status",
        methods: &[Method::GET],
        auth: Auth::Required,
        scope: Scope::Inference,
        query_token_compat: false,
    },
    RoutePolicy {
        path: "/v1/chat/completions",
        methods: &[Method::POST],
        auth: Auth::Required,
        scope: Scope::Inference,
        query_token_compat: false,
    },
    RoutePolicy {
        path: "/v1/ensure",
        methods: &[Method::POST],
        auth: Auth::Required,
        scope: Scope::Inference,
        query_token_compat: false,
    },
    RoutePolicy {
        path: "/v1/ensure/stream",
        methods: &[Method::GET],
        auth: Auth::Required,
        scope: Scope::Inference,
        query_token_compat: true,
    },
];

pub fn policy_for(path: &str) -> Option<&'static RoutePolicy> {
    POLICY.iter().find(|p| p.path == path)
}

/// What the request presented, reduced to booleans so the decision is a pure function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Presented {
    pub token_configured: bool,
    pub bearer_valid: bool,
    pub query_valid: bool,
    pub allow_query: bool,
    pub is_get: bool,
}

/// The policy decision (REQ-SEC-001/AC2): `Ok` = pass, else the error code to render.
pub fn decide(policy: &RoutePolicy, method: &Method, p: Presented) -> Result<(), ErrorCode> {
    if !policy.methods.contains(method) {
        return Err(ErrorCode::EndpointNotSupported);
    }
    if policy.auth == Auth::Public || !p.token_configured {
        return Ok(());
    }
    let via_query = policy.query_token_compat && p.allow_query && p.is_get && p.query_valid;
    if !(p.bearer_valid || via_query) {
        return Err(ErrorCode::AuthRequired);
    }
    match policy.scope {
        Scope::None | Scope::Inference => Ok(()),
        Scope::Admin => Err(ErrorCode::AuthForbidden), // inference keys never grant admin (P3 adds admin tokens)
    }
}

/// `Authorization: Bearer <token>` — scheme case-insensitive, one or more spaces (RFC 9110 §11.4).
pub fn bearer_from_header(value: &str) -> Option<&str> {
    let (scheme, rest) = value.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let t = rest.trim_start_matches(' ');
    (!t.is_empty()).then_some(t)
}

/// Constant-time comparison (REQ-SEC-004). Length mismatch short-circuits, as `subtle` documents.
pub fn token_matches(expected: Option<&str>, presented: &str) -> bool {
    match expected {
        Some(e) => e.as_bytes().ct_eq(presented.as_bytes()).into(),
        None => false,
    }
}

fn query_token(uri: &Uri) -> Option<String> {
    uri.query()?
        .split('&')
        .find_map(|kv| kv.strip_prefix("token=").map(|v| v.to_string()))
}

/// The same URI without `token=` (REQ-SEC-005/AC3: redacted everywhere downstream).
fn strip_query_token(uri: &Uri) -> Uri {
    let Some(q) = uri.query() else {
        return uri.clone();
    };
    let kept: Vec<&str> = q
        .split('&')
        .filter(|kv| !kv.starts_with("token="))
        .collect();
    let pq = if kept.is_empty() {
        uri.path().to_string()
    } else {
        format!("{}?{}", uri.path(), kept.join("&"))
    };
    pq.parse().unwrap_or_else(|_| uri.clone())
}

pub struct AuthState {
    token: Option<String>,
    allow_query_token: bool,
    log: Box<dyn Fn(String) + Send + Sync>,
    last_failure: Mutex<HashMap<IpAddr, Instant>>,
}

impl AuthState {
    pub fn new(
        token: Option<String>,
        allow_query_token: bool,
        log: impl Fn(String) + Send + Sync + 'static,
    ) -> Arc<Self> {
        Arc::new(Self {
            token,
            allow_query_token,
            log: Box::new(log),
            last_failure: Mutex::new(HashMap::new()),
        })
    }

    /// At most one line per peer per second (REQ-SEC-011/AC1); never the credential.
    fn log_failure(&self, peer: Option<SocketAddr>, method: &Method, path: &str, reason: &str) {
        let ip = peer.map(|p| p.ip());
        if let Some(ip) = ip
            && let Ok(mut m) = self.last_failure.lock()
        {
            let now = Instant::now();
            if m.get(&ip)
                .is_some_and(|t| now.duration_since(*t) < Duration::from_secs(1))
            {
                return;
            }
            m.insert(ip, now);
        }
        let peer = peer.map_or_else(|| "unknown".to_string(), |p| p.to_string());
        (self.log)(format!(
            "auth: 401 listener=loopback peer={peer} {method} {path} ({reason})"
        ));
    }
}

pub async fn middleware(
    State(auth): State<Arc<AuthState>>,
    mut req: Request,
    next: Next,
) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let Some(policy) = policy_for(&path) else {
        return ApiError::new(
            ErrorCode::EndpointNotSupported,
            format!("{path} is not supported"),
        )
        .into_response();
    };
    let bearer_valid = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(bearer_from_header)
        .is_some_and(|t| token_matches(auth.token.as_deref(), t));
    let query_valid =
        query_token(req.uri()).is_some_and(|t| token_matches(auth.token.as_deref(), &t));
    let presented = Presented {
        token_configured: auth.token.is_some(),
        bearer_valid,
        query_valid,
        allow_query: auth.allow_query_token,
        is_get: method == Method::GET,
    };
    match decide(policy, &method, presented) {
        Ok(()) => {
            if query_valid {
                *req.uri_mut() = strip_query_token(req.uri());
            }
            next.run(req).await
        }
        Err(ErrorCode::AuthRequired) => {
            let peer = req
                .extensions()
                .get::<ConnectInfo<SocketAddr>>()
                .map(|c| c.0);
            let reason = if req.headers().contains_key(header::AUTHORIZATION) {
                "invalid credentials"
            } else {
                "no credentials"
            };
            auth.log_failure(peer, &method, &path, reason);
            ApiError::new(ErrorCode::AuthRequired, "missing or invalid credentials").into_response()
        }
        Err(ErrorCode::AuthForbidden) => ApiError::new(
            ErrorCode::AuthForbidden,
            "this credential lacks the required scope",
        )
        .into_response(),
        Err(code) => {
            ApiError::new(code, format!("{method} {path} is not supported")).into_response()
        }
    }
}
