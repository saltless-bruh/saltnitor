//! `sudo -n systemctl <verb> <unit>` for the TUI keys. Every outcome is reported: a spawn
//! failure or non-zero exit becomes an `Event::Error` carrying stderr (REQ-ERR-005/AC1, BD-15).
use crate::events::Event;
use tokio::process::Command;
use tokio::sync::mpsc;

/// Run the verb through `sudo` (`sudo` is a parameter so tests can substitute a stand-in).
pub async fn run(sudo: &str, verb: &str, unit: &str) -> Result<(), String> {
    let out = Command::new(sudo)
        .args(["-n", "systemctl", verb, unit])
        .output()
        .await
        .map_err(|e| format!("cannot run `{sudo} systemctl {verb} {unit}`: {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stderr = stderr.trim();
    Err(format!(
        "systemctl {verb} {unit} failed ({}){}{stderr}",
        out.status,
        if stderr.is_empty() { "" } else { ": " }
    ))
}

/// [`run`], then tell the operator: `ok` as a log line on success, `Event::Error` otherwise.
pub async fn run_and_report(
    sudo: &str,
    verb: &str,
    unit: &str,
    tx: &mpsc::Sender<Event>,
    ok: &str,
) {
    let event = match run(sudo, verb, unit).await {
        Ok(()) => Event::LogLine(ok.to_string()),
        Err(message) => Event::Error {
            source: "systemctl".into(),
            message,
        },
    };
    let _ = tx.send(event).await;
}
