//! The TUI hot-swap's direct call to the raw router. It is a different hop from the interrogator
//! (which goes through Saltnitor with the *client* key): the router wants its own `--api-key`
//! (`infer_bearer`), and no credential ever leaves the machine (INV-06, REQ-SEC-012/AC1).
use std::net::IpAddr;
use std::time::Duration;

/// How long a warm-up load may take before the TUI gives up on it.
const WARM_LOAD_TIMEOUT: Duration = Duration::from_secs(120);

fn is_loopback_host(host: &str) -> bool {
    let h = host.trim().trim_start_matches('[').trim_end_matches(']');
    h.eq_ignore_ascii_case("localhost") || h.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

/// The bearer for the direct router call: the router's own key (`infer_bearer`), only when the
/// operator enabled it and only for a loopback router.
pub fn direct_router_bearer(
    host: &str,
    enabled: bool,
    infer_bearer: Option<&str>,
) -> Option<String> {
    infer_bearer
        .filter(|_| enabled && is_loopback_host(host))
        .map(str::to_string)
}

/// Warm-load `model` by name through the raw router (`max_tokens = 1`).
pub async fn warm_load(
    host: &str,
    port: u16,
    bearer: Option<String>,
    model: &str,
) -> Result<(), String> {
    let payload = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": "warmup"}],
        "max_tokens": 1,
    });
    let url = format!("http://{host}:{port}/v1/chat/completions");
    let mut req = reqwest::Client::new()
        .post(url)
        .timeout(WARM_LOAD_TIMEOUT)
        .json(&payload);
    if let Some(b) = bearer {
        req = req.bearer_auth(b);
    }
    match req.send().await {
        Ok(res) if res.status().is_success() => Ok(()),
        Ok(res) => Err(format!("router returned {}", res.status())),
        Err(e) if e.is_timeout() => Err("router did not answer in time".to_string()),
        Err(_) => Err("router did not respond".to_string()),
    }
}
