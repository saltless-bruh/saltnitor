//! R0 record/replay (CR-2): read-only capture of a live llama-server router, shape drift
//! against a baseline, and replay lookup for scenarios with `replay_from`.
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Endpoints R0 reads, and the file each is saved as. GET only — never load/unload/chat.
pub const ENDPOINTS: &[(&str, &str)] = &[
    ("/health", "health.json"),
    ("/models", "models.json"),
    ("/v1/models", "v1_models.json"),
    ("/props", "props.json"),
    ("/slots", "slots.json"),
    ("/metrics", "metrics.txt"),
];
/// String fields that may carry user text; their values are replaced before saving.
const REDACT_KEYS: &[&str] = &["prompt", "content", "text", "generated", "generated_text"];
/// Command-line flags whose value is a secret (router children echo their argv in `/models`).
const SECRET_FLAGS: &[&str] = &[
    "--api-key",
    "--api-key-file",
    "--hf-token",
    "--ssl-key-file",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Liveness {
    Live,
    Degraded,
    Down,
    Absent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Shape {
    Ok,
    Drift {
        added: Vec<String>,
        removed: Vec<String>,
    },
    NoBaseline,
    NotJson,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EndpointRecord {
    pub path: String,
    pub file: Option<String>,
    pub status: Option<u16>,
    pub latency_ms: u64,
    pub content_type: Option<String>,
    pub liveness: Liveness,
    pub shape: Shape,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub captured_unix: u64,
    pub upstream: String,
    pub endpoints: Vec<EndpointRecord>,
}

pub struct CaptureOptions {
    pub upstream: String,
    pub out: PathBuf,
    pub baseline: Option<PathBuf>,
    /// Sent as `Authorization: Bearer`; never written anywhere.
    pub bearer: Option<String>,
    pub slow_ms: u64,
}

pub async fn capture(opts: &CaptureOptions) -> Result<Manifest, String> {
    std::fs::create_dir_all(&opts.out)
        .map_err(|e| format!("create {}: {e}", opts.out.display()))?;
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let mut endpoints = Vec::new();
    for (path, file) in ENDPOINTS {
        let mut rb = http.get(format!("{}{}", opts.upstream.trim_end_matches('/'), path));
        if let Some(t) = &opts.bearer {
            rb = rb.bearer_auth(t);
        }
        let t0 = Instant::now();
        let rec = match rb.send().await {
            Err(_) => EndpointRecord {
                path: (*path).into(),
                file: None,
                status: None,
                latency_ms: t0.elapsed().as_millis() as u64,
                content_type: None,
                liveness: Liveness::Down,
                shape: Shape::NotJson,
            },
            Ok(resp) => {
                let status = resp.status().as_u16();
                let content_type = resp
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_string);
                let body = resp.bytes().await.map_err(|e| format!("{path}: {e}"))?;
                let latency_ms = t0.elapsed().as_millis() as u64;
                if status == 404 {
                    EndpointRecord {
                        path: (*path).into(),
                        file: None,
                        status: Some(404),
                        latency_ms,
                        content_type,
                        liveness: Liveness::Absent,
                        shape: Shape::NotJson,
                    }
                } else {
                    let text = sanitize_home(&String::from_utf8_lossy(&body));
                    let saved = match serde_json::from_str::<Value>(&text) {
                        Ok(mut v) => {
                            redact(&mut v);
                            serde_json::to_string_pretty(&v).map_err(|e| e.to_string())? + "\n"
                        }
                        Err(_) => text,
                    };
                    std::fs::write(opts.out.join(file), &saved)
                        .map_err(|e| format!("write {file}: {e}"))?;
                    let liveness = if (200..300).contains(&status) && latency_ms <= opts.slow_ms {
                        Liveness::Live
                    } else {
                        Liveness::Degraded
                    };
                    let shape = shape_vs_baseline(&saved, opts.baseline.as_deref(), file);
                    EndpointRecord {
                        path: (*path).into(),
                        file: Some((*file).into()),
                        status: Some(status),
                        latency_ms,
                        content_type,
                        liveness,
                        shape,
                    }
                }
            }
        };
        endpoints.push(rec);
    }
    let captured_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let m = Manifest {
        captured_unix,
        upstream: opts.upstream.clone(),
        endpoints,
    };
    let text = serde_json::to_string_pretty(&m).map_err(|e| e.to_string())? + "\n";
    std::fs::write(opts.out.join("manifest.json"), text)
        .map_err(|e| format!("write manifest: {e}"))?;
    Ok(m)
}

fn shape_vs_baseline(text: &str, baseline: Option<&Path>, file: &str) -> Shape {
    let Ok(cur) = serde_json::from_str::<Value>(text) else {
        return Shape::NotJson;
    };
    let Some(dir) = baseline else {
        return Shape::NoBaseline;
    };
    let Some(prev) = std::fs::read_to_string(dir.join(file))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
    else {
        return Shape::NoBaseline;
    };
    let (now, before) = (key_paths(&cur), key_paths(&prev));
    let added: Vec<String> = now.difference(&before).cloned().collect();
    let removed: Vec<String> = before.difference(&now).cloned().collect();
    if added.is_empty() && removed.is_empty() {
        Shape::Ok
    } else {
        Shape::Drift { added, removed }
    }
}

/// Every key path in `v`; array elements are merged under `name[]`.
pub fn key_paths(v: &Value) -> BTreeSet<String> {
    fn walk(v: &Value, at: String, out: &mut BTreeSet<String>) {
        match v {
            Value::Object(m) => {
                if !at.is_empty() {
                    out.insert(at.clone());
                }
                for (k, x) in m {
                    walk(
                        x,
                        if at.is_empty() {
                            k.clone()
                        } else {
                            format!("{at}.{k}")
                        },
                        out,
                    );
                }
            }
            Value::Array(a) => {
                let p = format!("{at}[]");
                out.insert(p.clone());
                for x in a {
                    walk(x, p.clone(), out);
                }
            }
            _ => {
                out.insert(at);
            }
        }
    }
    let mut out = BTreeSet::new();
    walk(v, String::new(), &mut out);
    out
}

/// Replace `/home/<user>/` with `${HOME}/`.
pub fn sanitize_home(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("/home/") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 6..];
        match after.find('/') {
            Some(j) if j > 0 && !after[..j].contains(|c: char| c == '"' || c.is_whitespace()) => {
                out.push_str("${HOME}/");
                rest = &after[j + 1..];
            }
            _ => {
                out.push_str("/home/");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Replace string values of user-text keys (prompts, generated text) with `<redacted>`.
pub fn redact(v: &mut Value) {
    match v {
        Value::Object(m) => {
            for (k, x) in m.iter_mut() {
                if x.is_string() && REDACT_KEYS.contains(&k.as_str()) {
                    *x = Value::String("<redacted>".into());
                } else {
                    redact(x);
                }
            }
        }
        Value::Array(a) => {
            let mut mask_next = false;
            for x in a.iter_mut() {
                if let Value::String(s) = x {
                    if mask_next {
                        *s = "<redacted>".into();
                        mask_next = false;
                        continue;
                    }
                    if SECRET_FLAGS.contains(&s.as_str()) {
                        mask_next = true;
                    } else if let Some(flag) = SECRET_FLAGS
                        .iter()
                        .find(|f| s.starts_with(&format!("{f}=")))
                    {
                        *s = format!("{flag}=<redacted>");
                    }
                } else {
                    mask_next = false;
                    redact(x);
                }
            }
        }
        _ => {}
    }
}

pub fn render_table(m: &Manifest) -> String {
    let mut s = format!(
        "{:<12} {:>6} {:>7}  {:<9} {}\n",
        "ENDPOINT", "STATUS", "MS", "LIVENESS", "SHAPE"
    );
    for e in &m.endpoints {
        let status = e
            .status
            .map(|c| c.to_string())
            .unwrap_or_else(|| "-".into());
        let live = match e.liveness {
            Liveness::Live => "LIVE",
            Liveness::Degraded => "DEGRADED",
            Liveness::Down => "DOWN",
            Liveness::Absent => "absent",
        };
        let shape = match &e.shape {
            Shape::Ok => "SHAPE-OK".to_string(),
            Shape::Drift { added, removed } => format!("SHAPE-DRIFT +{added:?} -{removed:?}"),
            Shape::NoBaseline => "no-baseline".into(),
            Shape::NotJson => "-".into(),
        };
        s.push_str(&format!(
            "{:<12} {:>6} {:>7}  {:<9} {}\n",
            e.path, status, e.latency_ms, live, shape
        ));
    }
    s
}

/// Recorded (status, content type, body) for `path` in the capture at `dir`, if any.
pub fn replay_file(dir: &Path, path: &str) -> Option<(u16, String, Vec<u8>)> {
    let text = std::fs::read_to_string(dir.join("manifest.json")).ok()?;
    let m: Manifest = serde_json::from_str(&text).ok()?;
    let e = m.endpoints.into_iter().find(|e| e.path == path)?;
    let file = e.file?;
    let bytes = std::fs::read(dir.join(&file)).ok()?;
    let ct = e.content_type.unwrap_or_else(|| {
        if file.ends_with(".txt") {
            "text/plain".into()
        } else {
            "application/json".into()
        }
    });
    Some((e.status.unwrap_or(200), ct, bytes))
}

const USAGE: &str = "usage: fake-llama-server record --out DIR [--upstream URL] [--baseline DIR] [--bearer-env VAR] [--slow-ms N]";

/// `fake-llama-server record …`; exit 0 when /health is LIVE, 1 otherwise, 2 on usage/IO errors.
pub async fn cli(args: &[String]) -> i32 {
    let mut upstream = "http://127.0.0.1:8080".to_string();
    let (mut out, mut baseline, mut bearer_env, mut slow_ms) = (None, None, None, 2000u64);
    let mut i = 0;
    while i < args.len() {
        match (args[i].as_str(), args.get(i + 1).cloned()) {
            ("--upstream", Some(v)) => upstream = v,
            ("--out", Some(v)) => out = Some(PathBuf::from(v)),
            ("--baseline", Some(v)) => baseline = Some(PathBuf::from(v)),
            ("--bearer-env", Some(v)) => bearer_env = Some(v),
            ("--slow-ms", Some(v)) => match v.parse() {
                Ok(n) => slow_ms = n,
                Err(_) => {
                    eprintln!("record: --slow-ms needs a number\n{USAGE}");
                    return 2;
                }
            },
            (other, _) => {
                eprintln!("record: unknown or incomplete argument '{other}'\n{USAGE}");
                return 2;
            }
        }
        i += 2;
    }
    let Some(out) = out else {
        eprintln!("record: --out is required\n{USAGE}");
        return 2;
    };
    let bearer = match bearer_env {
        Some(name) => match std::env::var(&name) {
            Ok(t) if !t.is_empty() => Some(t),
            Ok(_) => None,
            Err(_) => {
                eprintln!("record: environment variable {name} is not set");
                return 2;
            }
        },
        None => None,
    };
    match capture(&CaptureOptions {
        upstream,
        out,
        baseline,
        bearer,
        slow_ms,
    })
    .await
    {
        Ok(m) => {
            print!("{}", render_table(&m));
            let live = m
                .endpoints
                .iter()
                .any(|e| e.path == "/health" && e.liveness == Liveness::Live);
            if live { 0 } else { 1 }
        }
        Err(e) => {
            eprintln!("record: {e}");
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Verifies: REQ-TST-011/AC1
    #[test]
    fn key_paths_merge_array_elements() {
        let got: Vec<String> = key_paths(&json!({"a": {"b": [{"c": 1}, {"c": 2, "d": null}]}}))
            .into_iter()
            .collect();
        assert_eq!(got, vec!["a", "a.b[]", "a.b[].c", "a.b[].d"]);
    }

    /// Verifies: REQ-TST-011/AC1
    #[test]
    fn sanitize_home_rewrites_only_real_home_paths() {
        assert_eq!(
            sanitize_home(r#"path /home/laz/ai-models/x and /home/ alone and "/home/bob/q""#),
            r#"path ${HOME}/ai-models/x and /home/ alone and "${HOME}/q""#
        );
    }

    /// Verifies: REQ-TST-011/AC1
    #[test]
    fn redact_masks_secret_values_in_argv_arrays() {
        let mut v = json!({"status": {"args": [
            "llama-server", "--api-key", "sk-live-1", "--port", "8080",
            "--hf-token=hf_abc", "--ssl-key-file", "/k.pem", "--api-key-file", "/tok"
        ]}});
        redact(&mut v);
        assert_eq!(
            v,
            json!({"status": {"args": [
                "llama-server", "--api-key", "<redacted>", "--port", "8080",
                "--hf-token=<redacted>", "--ssl-key-file", "<redacted>", "--api-key-file", "<redacted>"
            ]}})
        );
    }

    /// Verifies: REQ-TST-011/AC1
    #[test]
    fn redact_replaces_user_text_but_keeps_shape() {
        let mut v =
            json!({"slots": [{"id": 0, "prompt": "secret plan", "n_ctx": 4096, "content": ["x"]}]});
        redact(&mut v);
        assert_eq!(
            v,
            json!({"slots": [{"id": 0, "prompt": "<redacted>", "n_ctx": 4096, "content": ["x"]}]})
        );
    }
}
