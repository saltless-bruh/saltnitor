//! Process-level behavior of the fake binary (REQ-TST-011).
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use serde_json::Value;

const BIN: &str = env!("CARGO_BIN_EXE_fake-llama-server");

fn tmpdir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("fake-llama-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Start the binary on a free port; returns the child and its base URL.
fn start(cmd: &mut Command) -> (Child, String) {
    let mut child = cmd
        .args(["--port", "0"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.as_mut().unwrap())
        .read_line(&mut line)
        .unwrap();
    let addr = line
        .trim()
        .strip_prefix("FAKE_LISTENING ")
        .unwrap_or_else(|| panic!("unexpected: {line:?}"))
        .to_string();
    (child, format!("http://{addr}"))
}

fn jsonl(path: &PathBuf) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

/// Verifies: REQ-TST-011/AC1
#[test]
fn help_and_version_print_default_fixtures() {
    let out = Command::new(BIN).arg("--help").output().unwrap();
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        include_str!("../fixtures/help-default.txt")
    );
    let out = Command::new(BIN).arg("--version").output().unwrap();
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        include_str!("../fixtures/version-default.txt")
    );
}

/// Verifies: REQ-TST-011/AC1
#[test]
fn help_fixture_is_selected_by_env() {
    let d = tmpdir("help");
    std::fs::write(
        d.join("h.txt"),
        "usage: llama-server [build 1234]\n  --n-cpu-moe N\n",
    )
    .unwrap();
    let out = Command::new(BIN)
        .arg("--help")
        .env("FAKE_HELP_FIXTURE", d.join("h.txt"))
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "usage: llama-server [build 1234]\n  --n-cpu-moe N\n"
    );
    let out = Command::new(BIN)
        .arg("--help")
        .env("FAKE_HELP_FIXTURE", d.join("missing.txt"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn argv_and_allowlisted_env_are_recorded() {
    let d = tmpdir("argv");
    let rec = d.join("rec.jsonl");
    let (mut child, base) = start(
        Command::new(BIN)
            .args(["--models-preset", "x.ini", "-ngl", "99"])
            .env("FAKE_RECORD", &rec)
            .env("LLAMA_ARG_THREADS", "7")
            .env("SECRET_TOKEN", "hunter2"),
    );
    assert_eq!(
        reqwest::get(format!("{base}/health"))
            .await
            .unwrap()
            .status(),
        200
    );
    child.kill().unwrap();
    child.wait().unwrap();
    let ev = jsonl(&rec);
    let argv = &ev.iter().find(|e| e["kind"] == "argv").unwrap()["argv"];
    assert!(argv.as_array().unwrap().iter().any(|a| a == "-ngl"));
    let env = &ev.iter().find(|e| e["kind"] == "env").unwrap()["env"];
    assert_eq!(env["LLAMA_ARG_THREADS"], "7");
    assert!(env.get("SECRET_TOKEN").is_none());
    assert!(!std::fs::read_to_string(&rec).unwrap().contains("hunter2"));
    assert!(
        ev.iter()
            .any(|e| e["kind"] == "request" && e["path"] == "/health")
    );
}

/// Verifies: REQ-TST-011/AC1
#[test]
fn oom_rule_exits_when_the_argument_is_too_large() {
    let d = tmpdir("oom");
    std::fs::write(
        d.join("s.toml"),
        "[oom]\narg = \"--ctx-size\"\ngt = 65536\n",
    )
    .unwrap();
    let out = Command::new(BIN)
        .args(["--ctx-size", "131072", "--port", "0"])
        .env("FAKE_SCENARIO", d.join("s.toml"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("cudaMalloc failed: out of memory"));
    let (mut child, _) = start(
        Command::new(BIN)
            .args(["--ctx-size", "4096"])
            .env("FAKE_SCENARIO", d.join("s.toml")),
    );
    child.kill().unwrap();
    child.wait().unwrap();
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn crash_after_exits_the_process() {
    let d = tmpdir("crash");
    std::fs::write(
        d.join("s.toml"),
        "[routes.\"POST /v1/chat/completions\"]\nkind = \"crash_after\"\nchunks = 1\n",
    )
    .unwrap();
    let (mut child, base) = start(Command::new(BIN).env("FAKE_SCENARIO", d.join("s.toml")));
    let mut r = reqwest::Client::new()
        .post(format!("{base}/v1/chat/completions"))
        .body(r#"{"model":"A","stream":true,"messages":[]}"#)
        .send()
        .await
        .unwrap();
    while let Ok(Some(_)) = r.chunk().await {}
    assert_eq!(child.wait().unwrap().code(), Some(101));
}

/// Verifies: REQ-TST-011/AC1
#[test]
fn bad_scenario_exits_2_with_a_diagnostic() {
    let d = tmpdir("bad");
    std::fs::write(d.join("s.toml"), "models = [").unwrap();
    let out = Command::new(BIN)
        .args(["--port", "0"])
        .env("FAKE_SCENARIO", d.join("s.toml"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("bad scenario"));
}

/// Verifies: REQ-TST-011/AC1
#[test]
fn record_cli_prints_a_liveness_table_and_exit_code() {
    let d = tmpdir("reccli");
    let (mut child, base) = start(&mut Command::new(BIN));
    let out = Command::new(BIN)
        .args(["record", "--upstream", &base, "--out"])
        .arg(d.join("cap"))
        .output()
        .unwrap();
    child.kill().unwrap();
    child.wait().unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let table = String::from_utf8(out.stdout).unwrap();
    assert!(
        table
            .lines()
            .any(|l| l.starts_with("/health") && l.contains("LIVE")),
        "{table}"
    );
    let out = Command::new(BIN)
        .args(["record", "--upstream", "http://127.0.0.1:1", "--out"])
        .arg(d.join("down"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let out = Command::new(BIN).args(["record"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}
