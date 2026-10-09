#![allow(clippy::unwrap_used, clippy::expect_used)] // tests may unwrap: a panic is the failure signal
//! Runs the real binary: a malformed config must exit 2 before any port is bound (REQ-CFG-003).
use std::io::Write;
use std::net::TcpListener;
use std::process::Command;

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn run(config: &str) -> (i32, String, u16, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let port = free_port();
    let path = dir.path().join("config.toml");
    std::fs::File::create(&path)
        .unwrap()
        .write_all(config.replace("{PORT}", &port.to_string()).as_bytes())
        .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_saltnitor"))
        .arg("--config")
        .arg(&path)
        .env("HOME", dir.path())
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        port,
        dir,
    )
}

/// Verifies: REQ-CFG-003/AC1, REQ-CFG-003/AC2, REQ-CFG-003/AC3
#[test]
fn type_error_exits_2_with_the_diagnostic_and_leaves_the_port_free() {
    let (code, err, port, _d) = run(
        "control_port = {PORT}\n[profiles.qwen36]\nmodel = \"m.gguf\"\nest_vram_gb = \"9gb\"\n",
    );
    assert_eq!(code, 2, "stderr: {err}");
    for needle in [
        "config.toml:4:",
        "profiles.qwen36.est_vram_gb",
        "expected float",
        "found string \"9gb\"",
    ] {
        assert!(err.contains(needle), "missing {needle:?} in {err}");
    }
    assert!(
        TcpListener::bind(("127.0.0.1", port)).is_ok(),
        "control port must be free after exit 2"
    );
}

/// Verifies: REQ-CFG-004/AC1, REQ-CFG-004/AC2
#[test]
fn unknown_key_exits_2_with_a_hint() {
    let (code, err, _p, _d) = run("control_port = {PORT}\ncontrol_prot = 1\n");
    assert_eq!(code, 2, "stderr: {err}");
    assert!(
        err.contains("unknown key `control_prot`") && err.contains("did you mean `control_port`?"),
        "{err}"
    );
}

/// Verifies: REQ-CFG-002/AC2
#[test]
fn missing_config_flag_file_exits_2() {
    let dir = tempfile::tempdir().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_saltnitor"))
        .arg("--config")
        .arg(dir.path().join("absent.toml"))
        .env("HOME", dir.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("absent.toml"));
}
