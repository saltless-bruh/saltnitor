//! Strict v1 config loader (REQ-CFG-001…005, REQ-CFG-007; INV-10). Parses with
//! `deny_unknown_fields`, reports `file:line:col`, the dotted key path, what was expected and
//! what was found, and never falls back to defaults on a malformed file.
#![allow(
    clippy::result_large_err,
    reason = "ConfigError is a one-shot startup diagnostic; boxing it would only obscure the public fields"
)]

use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: u32 = 1;
/// §6 default for `max_body_bytes` (32 MiB).
#[allow(
    dead_code,
    reason = "consumed by the proxy body limit in T1.12 (REQ-PRX-017)"
)]
pub const DEFAULT_MAX_BODY_BYTES: u64 = 33_554_432;

#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ConfigV1 {
    pub schema_version: Option<u32>,
    pub port: Option<u16>,
    pub host: Option<String>,
    pub service_name: Option<String>,
    pub default_ngl: Option<i32>,
    pub default_ctx: Option<i32>,
    pub control_port: Option<u16>,
    pub control_token: Option<String>,
    pub control_token_env: Option<String>,
    pub control_token_file: Option<String>,
    pub router_base: Option<String>,
    pub infer_bearer: Option<String>,
    pub reserve_vram_gb: Option<f64>,
    pub reserve_ram_gb: Option<f64>,
    pub router_ini: Option<String>,
    pub client_key_env: Option<String>,
    pub allow_query_token: Option<bool>,
    pub max_body_bytes: Option<u64>,
    #[serde(default)]
    pub timeouts: Timeouts,
    #[serde(default)]
    pub process: ProcessCfg,
    #[serde(default)]
    pub profiles: BTreeMap<String, ProfileV1>,
}

/// §6 defaults: connect 5 000 ms, first byte 600 000 ms, idle between chunks 120 000 ms.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct Timeouts {
    pub connect_ms: u64,
    pub first_byte_ms: u64,
    pub idle_ms: u64,
}
impl Default for Timeouts {
    fn default() -> Self {
        Self {
            connect_ms: 5_000,
            first_byte_ms: 600_000,
            idle_ms: 120_000,
        }
    }
}

/// §6 default: `process.term_grace_ms` 5 000.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct ProcessCfg {
    pub term_grace_ms: u64,
}
impl Default for ProcessCfg {
    fn default() -> Self {
        Self {
            term_grace_ms: 5_000,
        }
    }
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProfileV1 {
    pub model: String,
    #[serde(default)]
    pub offload: bool,
    pub est_vram_gb: Option<f64>,
    pub est_ram_gb: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigError {
    pub file: PathBuf,
    pub line: Option<usize>,
    pub col: Option<usize>,
    pub key: String,
    pub expected: String,
    pub found: String,
    pub hint: Option<String>,
}
impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.file.display())?;
        if let (Some(l), Some(c)) = (self.line, self.col) {
            write!(f, ":{l}:{c}")?;
        }
        write!(f, ": ")?;
        if !self.key.is_empty() {
            write!(f, "{}: ", self.key)?;
        }
        write!(f, "expected {}, found {}", self.expected, self.found)?;
        if let Some(h) = &self.hint {
            write!(f, "\n  hint: {h}")?;
        }
        Ok(())
    }
}
impl std::error::Error for ConfigError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Flag,
    Xdg,
    Home,
    Defaults,
}
impl Source {
    fn label(self) -> &'static str {
        match self {
            Source::Flag => "--config",
            Source::Xdg => "$XDG_CONFIG_HOME",
            Source::Home => "~/.config",
            Source::Defaults => "defaults",
        }
    }
}

#[derive(Debug, Clone)]
#[allow(
    dead_code,
    reason = "path/source are read by tests and by the daemon status work in later tasks; main logs `notes`"
)]
pub struct Loaded {
    pub config: ConfigV1,
    pub path: PathBuf,
    pub source: Source,
    /// Lines the caller logs at startup (resolved path, defaults notice, deprecations).
    pub notes: Vec<String>,
}

/// Environment lookup, injected so tests never touch the process environment.
pub type Env<'a> = &'a dyn Fn(&str) -> Option<String>;

/// `--config` wins; else `$XDG_CONFIG_HOME/saltnitor/config.toml`; else `~/.config/…`.
/// `SUDO_USER` is deliberately ignored (REQ-CFG-001/AC3).
pub fn resolve_path(flag: Option<&Path>, env: Env) -> (PathBuf, Source) {
    if let Some(p) = flag {
        return (p.to_path_buf(), Source::Flag);
    }
    if let Some(x) = env("XDG_CONFIG_HOME").filter(|s| !s.is_empty()) {
        return (PathBuf::from(x).join("saltnitor/config.toml"), Source::Xdg);
    }
    let home = env("HOME").unwrap_or_default();
    (
        PathBuf::from(home).join(".config/saltnitor/config.toml"),
        Source::Home,
    )
}

pub fn load(flag: Option<&Path>, env: Env) -> Result<Loaded, ConfigError> {
    let (path, source) = resolve_path(flag, env);
    let mut notes = vec![format!("config: {} ({})", path.display(), source.label())];
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && source != Source::Flag => {
            notes.push("config: no file found; starting with documented defaults".to_string());
            return Ok(Loaded {
                config: ConfigV1::default(),
                path,
                source: Source::Defaults,
                notes,
            });
        }
        Err(e) => {
            return Err(ConfigError {
                file: path,
                line: None,
                col: None,
                key: String::new(),
                expected: "a readable config file".into(),
                found: e.to_string(),
                hint: None,
            });
        }
    };
    let config = parse(&text, &path, env)?;
    if config.schema_version.is_none() {
        notes.push("config: schema_version absent; treated as v1".to_string());
    }
    Ok(Loaded {
        config,
        path,
        source,
        notes,
    })
}

pub fn parse(text: &str, file: &Path, env: Env) -> Result<ConfigV1, ConfigError> {
    let de = toml::de::Deserializer::parse(text).map_err(|e| toml_error(file, text, "", &e))?;
    let mut cfg: ConfigV1 = serde_path_to_error::deserialize(de).map_err(|e| {
        let key = match e.path().to_string() {
            p if p == "." => String::new(),
            p => p,
        };
        toml_error(file, text, &key, e.inner())
    })?;
    validate(&mut cfg, file, env)?;
    Ok(cfg)
}

fn validate(cfg: &mut ConfigV1, file: &Path, env: Env) -> Result<(), ConfigError> {
    let err = |key: &str, expected: &str, found: String| ConfigError {
        file: file.to_path_buf(),
        line: None,
        col: None,
        key: key.into(),
        expected: expected.into(),
        found,
        hint: None,
    };
    if let Some(v) = cfg.schema_version
        && v != SCHEMA_VERSION
    {
        return Err(err("schema_version", "1", v.to_string()));
    }
    if let Some(rb) = &cfg.router_base
        && !(rb.starts_with("http://") || rb.starts_with("https://"))
    {
        return Err(err("router_base", "an http(s) URL", format!("{rb:?}")));
    }
    let base = file.parent().unwrap_or_else(|| Path::new("."));
    if let Some(p) = cfg.router_ini.take() {
        cfg.router_ini = Some(
            expand_path(&p, base, env).map_err(|e| err("router_ini", "an expandable path", e))?,
        );
    }
    if let Some(p) = cfg.control_token_file.take() {
        cfg.control_token_file = Some(
            expand_path(&p, base, env)
                .map_err(|e| err("control_token_file", "an expandable path", e))?,
        );
    }
    Ok(())
}

/// Expand `~`, `$VAR`, `${VAR}`; resolve a relative result against `base` (REQ-CFG-007).
pub fn expand_path(raw: &str, base: &Path, env: Env) -> Result<String, String> {
    let mut s = raw.to_string();
    if s == "~" || s.starts_with("~/") {
        let home = env("HOME").ok_or_else(|| "undefined variable HOME".to_string())?;
        s.replace_range(0..1, &home);
    }
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while let Some(off) = s[i..].find('$') {
        out.push_str(&s[i..i + off]);
        let after = &s[i + off + 1..];
        let (name, consumed) = if let Some(inner) = after.strip_prefix('{') {
            let close = inner
                .find('}')
                .ok_or_else(|| "unterminated `${`".to_string())?;
            (&inner[..close], close + 2)
        } else {
            let n = after
                .bytes()
                .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
                .count();
            (&after[..n], n)
        };
        if name.is_empty() {
            return Err("`$` must be followed by a variable name".to_string());
        }
        out.push_str(&env(name).ok_or_else(|| format!("undefined variable {name}"))?);
        i += off + 1 + consumed;
    }
    out.push_str(&s[i..]);
    let p = Path::new(&out);
    Ok(if p.is_absolute() {
        out
    } else {
        base.join(p).to_string_lossy().into_owned()
    })
}

fn toml_error(file: &Path, text: &str, key: &str, e: &toml::de::Error) -> ConfigError {
    let (line, col) = match e.span() {
        Some(s) => {
            let (l, c) = line_col(text, s.start);
            (Some(l), Some(c))
        }
        None => (None, None),
    };
    let (key, expected, mut found, hint) = humanize(key, e.message());
    if is_secret_key(&key) && !found.starts_with("unknown key") {
        // INV-16: serde echoes the offending value ("integer `12345`"); for a secret keep the type only.
        found = found
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_string();
    }
    ConfigError {
        file: file.to_path_buf(),
        line,
        col,
        key,
        expected,
        found,
        hint,
    }
}

fn line_col(text: &str, offset: usize) -> (usize, usize) {
    let before = &text[..offset.min(text.len())];
    let line = before.matches('\n').count() + 1;
    let col = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
    (line, col)
}

/// Keys whose values are credentials (or name where one lives): diagnostics never print their values.
fn is_secret_key(key: &str) -> bool {
    let leaf = key.rsplit('.').next().unwrap_or(key);
    matches!(leaf, "control_token" | "infer_bearer" | "control_token_env")
        || leaf.ends_with("_token")
        || leaf.ends_with("_bearer")
        || leaf.ends_with("_key")
}

/// Turn serde's wording into the REQ-CFG-003/AC2 shape: (key, expected, found, hint).
fn humanize(key: &str, msg: &str) -> (String, String, String, Option<String>) {
    if let Some(rest) = msg.strip_prefix("unknown field `") {
        let (field, tail) = rest.split_once('`').unwrap_or((rest, ""));
        let known: Vec<String> = tail
            .split('`')
            .skip(1)
            .step_by(2)
            .map(str::to_string)
            .collect();
        let hint = closest(field, &known).map(|k| format!("did you mean `{k}`?"));
        let expected = if known.is_empty() {
            "no keys here".to_string()
        } else {
            format!("one of {}", known.join(", "))
        };
        // toml reports the unknown key after entering it, so the path may already end in it.
        let already = key == field || key.ends_with(&format!(".{field}"));
        let full = if already {
            key.to_string()
        } else {
            join(key, field)
        };
        return (full, expected, format!("unknown key `{field}`"), hint);
    }
    if let Some(rest) = msg.strip_prefix("missing field `") {
        let field = rest.trim_end_matches('`');
        return (
            join(key, field),
            "a value".to_string(),
            "nothing".to_string(),
            None,
        );
    }
    if let Some(rest) = msg.strip_prefix("invalid type: ")
        && let Some((found, expected)) = rest.rsplit_once(", expected ")
    {
        return (
            key.to_string(),
            type_name(expected),
            found.to_string(),
            None,
        );
    }
    if let Some(rest) = msg.strip_prefix("invalid value: ")
        && let Some((found, expected)) = rest.rsplit_once(", expected ")
    {
        return (
            key.to_string(),
            expected.to_string(),
            found.to_string(),
            None,
        );
    }
    (
        key.to_string(),
        "valid TOML".to_string(),
        msg.to_string(),
        None,
    )
}

fn type_name(serde_expected: &str) -> String {
    match serde_expected {
        "u8" | "u16" | "u32" | "u64" | "i8" | "i16" | "i32" | "i64" | "usize" | "isize" => {
            "integer".into()
        }
        "f32" | "f64" => "float".into(),
        "a string" | "string" => "string".into(),
        "a boolean" | "boolean" => "boolean".into(),
        "a map" | "a table" | "map" => "table".into(),
        other => other.trim_start_matches("a ").to_string(),
    }
}

fn join(path: &str, field: &str) -> String {
    if path.is_empty() {
        field.to_string()
    } else {
        format!("{path}.{field}")
    }
}

fn closest(field: &str, known: &[String]) -> Option<String> {
    known
        .iter()
        .map(|k| (edit_distance(field, k), k))
        .filter(|(d, _)| *d <= 3)
        .min_by_key(|(d, _)| *d)
        .map(|(_, k)| k.clone())
}

fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur.push((prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Pure check behind REQ-SEC-003/AC2: regular file, no group/other bits, owned by us.
pub fn validate_key_file(
    is_file: bool,
    mode: u32,
    owner_uid: u32,
    my_uid: u32,
) -> Result<(), String> {
    if !is_file {
        return Err("not a regular file".to_string());
    }
    if mode & 0o077 != 0 {
        return Err(format!(
            "mode {:04o} is readable by group or others; chmod 600",
            mode & 0o7777
        ));
    }
    if owner_uid != my_uid {
        return Err(format!(
            "owned by uid {owner_uid}, not the daemon user {my_uid}"
        ));
    }
    Ok(())
}

/// Exactly one of `control_token` (deprecated literal), `control_token_env`, `control_token_file`.
pub fn resolve_control_token(
    cfg: &ConfigV1,
    config_file: &Path,
    env: Env,
    notes: &mut Vec<String>,
) -> Result<Option<String>, ConfigError> {
    let err = |key: &str, expected: &str, found: String| ConfigError {
        file: config_file.to_path_buf(),
        line: None,
        col: None,
        key: key.into(),
        expected: expected.into(),
        found,
        hint: None,
    };
    let set = [
        cfg.control_token.is_some(),
        cfg.control_token_env.is_some(),
        cfg.control_token_file.is_some(),
    ]
    .iter()
    .filter(|b| **b)
    .count();
    if set > 1 {
        return Err(err(
            "control_token",
            "exactly one of control_token, control_token_env, control_token_file",
            format!("{set} set"),
        ));
    }
    if let Some(t) = &cfg.control_token {
        notes.push("control_token: a literal secret in config.toml is deprecated; use control_token_env or control_token_file (REQ-SEC-003)".to_string());
        return Ok(Some(t.clone()));
    }
    if let Some(name) = &cfg.control_token_env {
        let v = env(name).filter(|v| !v.is_empty()).ok_or_else(|| {
            err(
                "control_token_env",
                "an environment variable with a value",
                format!("{name} is unset or empty"),
            )
        })?;
        return Ok(Some(v));
    }
    if let Some(path) = &cfg.control_token_file {
        use std::os::unix::fs::MetadataExt;
        let meta = std::fs::metadata(path)
            .map_err(|e| err("control_token_file", "a readable file", e.to_string()))?;
        let my_uid = rustix::process::geteuid().as_raw();
        validate_key_file(meta.is_file(), meta.mode(), meta.uid(), my_uid)
            .map_err(|m| err("control_token_file", "a private key file", m))?;
        let v = std::fs::read_to_string(path)
            .map_err(|e| err("control_token_file", "a readable file", e.to_string()))?;
        let v = v.trim_end_matches(['\n', '\r']).to_string();
        if v.is_empty() {
            return Err(err(
                "control_token_file",
                "a non-empty token",
                "empty file".to_string(),
            ));
        }
        return Ok(Some(v));
    }
    Ok(None)
}

/// The bearer the TUI sends on its own HTTP calls, from the env var named by `client_key_env`
/// (REQ-SEC-013). Never a literal, never logged. (Moved here from `main.rs`, T1.6.)
pub fn client_bearer(env_name: Option<&str>) -> Option<String> {
    env_name
        .and_then(|n| std::env::var(n).ok())
        .filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)] // tests may unwrap: a panic is the failure signal
    use super::*;
    use std::collections::HashMap;

    fn env<'a>(map: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |k| {
            map.iter()
                .find(|(n, _)| *n == k)
                .map(|(_, v)| v.to_string())
        }
    }

    /// Verifies: REQ-CFG-003/AC2
    #[test]
    fn type_error_reports_file_line_col_key_expected_found() {
        let text = "port = 8080\n[profiles.qwen36]\nmodel = \"m.gguf\"\nest_vram_gb = \"9gb\"\n";
        let e = parse(text, Path::new("config.toml"), &env(&[])).unwrap_err();
        let s = e.to_string();
        assert!(s.starts_with("config.toml:4:"), "line is required: {s}");
        assert!(
            s.ends_with(": profiles.qwen36.est_vram_gb: expected float, found string \"9gb\""),
            "{s}"
        );
        assert!(e.col.is_some(), "column is required (REQ-CFG-003/AC2)");
    }

    /// Verifies: REQ-CFG-004/AC1, REQ-CFG-004/AC2
    #[test]
    fn unknown_key_is_rejected_with_the_closest_known_key() {
        let text = "[profiles.a]\nmodel = \"m\"\nest_vram_bg = 1.0\n";
        let e = parse(text, Path::new("c.toml"), &env(&[])).unwrap_err();
        assert_eq!(e.key, "profiles.a.est_vram_bg");
        assert_eq!(e.found, "unknown key `est_vram_bg`");
        assert_eq!(e.hint.as_deref(), Some("did you mean `est_vram_gb`?"));
        assert!(e.line.is_some(), "unknown keys carry a span: {e}");
    }

    /// Verifies: REQ-CFG-005/AC2
    #[test]
    fn unsupported_schema_version_is_config_invalid() {
        let e = parse("schema_version = 2\n", Path::new("c.toml"), &env(&[])).unwrap_err();
        assert_eq!(
            (e.key.as_str(), e.expected.as_str(), e.found.as_str()),
            ("schema_version", "1", "2")
        );
    }

    /// Verifies: REQ-CFG-007/AC1
    #[test]
    fn paths_expand_tilde_vars_braces_and_resolve_relative_to_the_config_dir() {
        let e = env(&[("HOME", "/h"), ("X", "ex")]);
        assert_eq!(expand_path("~/a", Path::new("/cfg"), &e).unwrap(), "/h/a");
        assert_eq!(
            expand_path("$X/b", Path::new("/cfg"), &e).unwrap(),
            "/cfg/ex/b"
        );
        assert_eq!(
            expand_path("/p/${X}_q", Path::new("/cfg"), &e).unwrap(),
            "/p/ex_q"
        );
        assert_eq!(
            expand_path("rel/r.ini", Path::new("/cfg"), &e).unwrap(),
            "/cfg/rel/r.ini"
        );
    }

    /// Verifies: REQ-CFG-007/AC2
    #[test]
    fn undefined_variable_in_a_path_is_config_invalid() {
        let text = "router_ini = \"$NOPE/router.ini\"\n";
        let e = parse(text, Path::new("c.toml"), &env(&[])).unwrap_err();
        assert_eq!(e.key, "router_ini");
        assert_eq!(e.found, "undefined variable NOPE");
    }

    /// Verifies: REQ-CFG-001/AC1, REQ-CFG-001/AC3
    #[test]
    fn resolution_prefers_flag_then_xdg_then_home_and_ignores_sudo_user() {
        let e = env(&[
            ("HOME", "/me"),
            ("XDG_CONFIG_HOME", "/xdg"),
            ("SUDO_USER", "root"),
        ]);
        assert_eq!(
            resolve_path(Some(Path::new("/f.toml")), &e),
            (PathBuf::from("/f.toml"), Source::Flag)
        );
        assert_eq!(
            resolve_path(None, &e),
            (PathBuf::from("/xdg/saltnitor/config.toml"), Source::Xdg)
        );
        let e2 = env(&[("HOME", "/me"), ("SUDO_USER", "root")]);
        assert_eq!(
            resolve_path(None, &e2),
            (
                PathBuf::from("/me/.config/saltnitor/config.toml"),
                Source::Home
            )
        );
    }

    /// Verifies: REQ-CFG-002/AC1, REQ-CFG-001/AC2
    #[test]
    fn missing_default_file_starts_with_defaults_and_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().to_string_lossy().into_owned();
        let l = load(None, &env(&[("HOME", &home)])).unwrap();
        assert_eq!(l.source, Source::Defaults);
        assert_eq!(l.config, ConfigV1::default());
        assert!(
            l.notes[0].contains(&home),
            "resolved path is logged: {:?}",
            l.notes
        );
        assert!(l.notes.iter().any(|n| n.contains("documented defaults")));
    }

    /// Verifies: REQ-CFG-002/AC2
    #[test]
    fn flag_pointing_at_a_missing_file_is_an_error() {
        let e = load(Some(Path::new("/nonexistent/x.toml")), &env(&[])).unwrap_err();
        assert_eq!(e.file, PathBuf::from("/nonexistent/x.toml"));
        assert_eq!(e.expected, "a readable config file");
    }

    /// Verifies: REQ-TST-001/AC1 (config validation unit coverage)
    #[test]
    fn live_shape_fixture_loads_with_every_profile() {
        let text = include_str!("../tests/data/live-shape.toml");
        let cfg = parse(text, Path::new("live-shape.toml"), &env(&[("HOME", "/h")])).unwrap();
        assert_eq!(cfg.control_port, Some(8765));
        assert_eq!(cfg.profiles.len(), 3);
        assert_eq!(cfg.timeouts, Timeouts::default());
        let _: HashMap<String, ProfileV1> = cfg.profiles.clone().into_iter().collect();
    }

    /// Verifies: REQ-SEC-013/AC1
    #[test]
    fn client_bearer_comes_only_from_the_named_env_var() {
        // SAFETY (test): single-threaded access to this unique variable name.
        unsafe { std::env::set_var("SALTNITOR_T16_KEY", "k-from-env") };
        assert_eq!(
            client_bearer(Some("SALTNITOR_T16_KEY")).as_deref(),
            Some("k-from-env")
        );
        assert_eq!(client_bearer(Some("SALTNITOR_T16_MISSING")), None);
        assert_eq!(client_bearer(None), None);
    }

    /// Verifies: REQ-CFG-003/AC2
    #[test]
    fn missing_required_key_and_top_level_unknown_key_name_the_full_path() {
        let e = parse(
            "[profiles.a]\noffload = true\n",
            Path::new("c.toml"),
            &env(&[]),
        )
        .unwrap_err();
        assert_eq!(
            (e.key.as_str(), e.found.as_str()),
            ("profiles.a.model", "nothing")
        );
        let e = parse("control_prot = 1\n", Path::new("c.toml"), &env(&[])).unwrap_err();
        assert_eq!(e.key, "control_prot");
    }
}
