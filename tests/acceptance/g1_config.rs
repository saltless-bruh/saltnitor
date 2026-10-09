//! G1 row 2 / D2: a malformed config never starts the app (REQ-CFG-003, BD-01).
//!
//! These tests pass `--config <file>`. Trap (brief §6): clap exits 2 on an unknown flag, so
//! the exit code alone proves nothing — the diagnostic text on stderr is what is asserted.

use std::path::Path;
use std::time::Duration;

use crate::harness::{TempDir, free_port, port_is_free, run_to_exit};

const EXIT_TIMEOUT: Duration = Duration::from_secs(20);

/// Run the binary with `--config` and return (exit code, stderr) once it has exited.
fn run_with_config(dir: &TempDir, config: &Path) -> (Option<i32>, String) {
    let cfg = config.to_str().expect("utf-8 path");
    let (status, _stdout, stderr) = run_to_exit(&["--config", cfg], dir.path(), EXIT_TIMEOUT);
    (status.code(), stderr)
}

/// 1-based line of the first line starting with `key` in `text`.
fn line_of(text: &str, key: &str) -> usize {
    text.lines()
        .position(|l| l.starts_with(key))
        .map(|i| i + 1)
        .expect("key present in config")
}

/// `<path>:<line>:<col>` where `<col>` is any column on that line.
fn location_regex(path: &Path, line: usize) -> regex::Regex {
    regex::Regex::new(&format!(
        "{}:{line}:[0-9]+",
        regex::escape(path.to_str().expect("utf-8 path"))
    ))
    .expect("regex")
}

/// Verifies: REQ-CFG-003/AC1, REQ-CFG-003/AC2, REQ-CFG-003/AC3
///
/// `est_vram_gb = "9gb"` under `[profiles.x]` is a type error: exit code 2, stderr names the
/// file, line and column, the dotted key, the expected type and the value found, and the
/// configured `control_port` is still free afterwards.
#[tokio::test]
async fn type_error_in_a_profile_exits_2_with_the_full_diagnostic_and_a_free_port() {
    let dir = TempDir::new("badcfg-type");
    let port = free_port();
    let text = format!(
        "control_port = {port}\ncontrol_token = \"t\"\nrouter_base = \"http://127.0.0.1:1\"\n\n\
         [profiles.x]\nmodel = \"x.gguf\"\nest_vram_gb = \"9gb\"\n"
    );
    let config = dir.write("config.toml", &text);
    let line = line_of(&text, "est_vram_gb");

    let (code, stderr) = run_with_config(&dir, &config);

    let mut problems = Vec::new();
    if code != Some(2) {
        problems.push(format!("exit code {code:?}, expected 2"));
    }
    if !location_regex(&config, line).is_match(&stderr) {
        problems.push(format!("stderr lacks `{}:{line}:<col>`", config.display()));
    }
    for needle in [
        "profiles.x.est_vram_gb",
        "expected float",
        "found string \"9gb\"",
    ] {
        if !stderr.contains(needle) {
            problems.push(format!("stderr lacks {needle:?}"));
        }
    }
    if !port_is_free(port) {
        problems.push(format!(
            "control_port {port} is not free after the failed start"
        ));
    }
    assert!(
        problems.is_empty(),
        "{}\n--- stderr ---\n{stderr}",
        problems.join("\n")
    );
}

/// Verifies: REQ-CFG-003/AC1, REQ-CFG-003/AC2, REQ-CFG-004/AC1
///
/// `ctx_size = 32768` under `[profiles.x]` is an unknown key for a v1 profile (only `model`,
/// `offload`, `est_vram_gb`, `est_ram_gb` exist): exit code 2, stderr names the dotted key and
/// says `unknown key`, and the configured port stays free.
#[tokio::test]
async fn unknown_profile_key_exits_2_with_the_diagnostic_and_a_free_port() {
    let dir = TempDir::new("badcfg-unknown");
    let port = free_port();
    let text = format!(
        "control_port = {port}\ncontrol_token = \"t\"\nrouter_base = \"http://127.0.0.1:1\"\n\n\
         [profiles.x]\nmodel = \"x.gguf\"\nctx_size = 32768\n"
    );
    let config = dir.write("config.toml", &text);
    let line = line_of(&text, "ctx_size");

    let (code, stderr) = run_with_config(&dir, &config);

    let mut problems = Vec::new();
    if code != Some(2) {
        problems.push(format!("exit code {code:?}, expected 2"));
    }
    if !location_regex(&config, line).is_match(&stderr) {
        problems.push(format!("stderr lacks `{}:{line}:<col>`", config.display()));
    }
    for needle in ["profiles.x.ctx_size", "unknown key"] {
        if !stderr.contains(needle) {
            problems.push(format!("stderr lacks {needle:?}"));
        }
    }
    if !port_is_free(port) {
        problems.push(format!(
            "control_port {port} is not free after the failed start"
        ));
    }
    assert!(
        problems.is_empty(),
        "{}\n--- stderr ---\n{stderr}",
        problems.join("\n")
    );
}
