#![allow(clippy::unwrap_used, clippy::expect_used)] // tests may unwrap: a panic is the failure signal
//! `sudo -n systemctl <verb> <unit>` never fails silently (REQ-ERR-005/AC1, BD-15).
use saltnitor::events::Event;
use saltnitor::systemctl::{run, run_and_report};
use std::os::unix::fs::PermissionsExt;
use tokio::sync::mpsc;

/// A freshly written script must not be exec'd while another test's fork still holds its write
/// fd (ETXTBSY), so these tests run one at a time.
static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A stand-in for `sudo`: `$@` is `-n systemctl <verb> <unit>`; it runs `body` instead.
fn fake_sudo(dir: &tempfile::TempDir, body: &str) -> String {
    let p = dir.path().join("sudo");
    std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p.to_string_lossy().into_owned()
}

/// Verifies: REQ-ERR-005/AC1 — a successful verb is Ok and the arguments are `-n systemctl <verb> <unit>`
#[tokio::test]
async fn success_passes_the_exact_arguments() {
    let _serial = SERIAL.lock().await;
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("argv");
    let sudo = fake_sudo(&d, &format!("printf '%s\\n' \"$*\" > {}", out.display()));
    run(&sudo, "restart", "llama-router").await.unwrap();
    assert_eq!(
        std::fs::read_to_string(out).unwrap().trim(),
        "-n systemctl restart llama-router"
    );
}

/// Verifies: REQ-ERR-005/AC1 — a non-zero exit is an error carrying the verb, the exit status and stderr
#[tokio::test]
async fn a_failing_verb_reports_the_exit_status_and_stderr() {
    let _serial = SERIAL.lock().await;
    let d = tempfile::tempdir().unwrap();
    let sudo = fake_sudo(&d, "echo 'sudo: a password is required' >&2; exit 1");
    let e = run(&sudo, "start", "llama-router").await.unwrap_err();
    assert!(e.contains("start"), "{e}");
    assert!(e.contains("llama-router"), "{e}");
    assert!(e.contains("a password is required"), "{e}");
}

/// Verifies: REQ-ERR-005/AC1 — a spawn failure is an error, not silence
#[tokio::test]
async fn a_missing_sudo_is_a_spawn_error() {
    let _serial = SERIAL.lock().await;
    let e = run("/nonexistent/sudo", "stop", "u").await.unwrap_err();
    assert!(e.contains("cannot run"), "{e}");
}

/// Verifies: REQ-ERR-005/AC1 — failures surface as `Event::Error` (status line + log), success as a log line
#[tokio::test]
async fn failures_become_error_events_and_success_a_log_line() {
    let _serial = SERIAL.lock().await;
    let d = tempfile::tempdir().unwrap();
    let bad = fake_sudo(&d, "echo nope >&2; exit 1");
    let (tx, mut rx) = mpsc::channel(4);
    run_and_report(&bad, "stop", "u", &tx, "stopped").await;
    match rx.recv().await.unwrap() {
        Event::Error { source, message } => {
            assert_eq!(source, "systemctl");
            assert!(message.contains("nope"), "{message}");
        }
        other => panic!("expected Event::Error, got {other:?}"),
    }
    let good = fake_sudo(&d, "exit 0");
    run_and_report(&good, "stop", "u", &tx, "stopped").await;
    assert!(matches!(rx.recv().await.unwrap(), Event::LogLine(l) if l.contains("stopped")));
}
