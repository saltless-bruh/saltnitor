//! R0 record/replay (CR-2). This task provides the replay lookup the engine calls;
//! Task 7 adds capture, drift, sanitizing and the `record` CLI.
use std::path::Path;

/// Recorded (status, content type, body) for `path` in the capture at `dir`, if any.
pub fn replay_file(_dir: &Path, _path: &str) -> Option<(u16, String, Vec<u8>)> {
    None
}

/// `fake-llama-server record …` entry point; returns the process exit code.
pub async fn cli(args: &[String]) -> i32 {
    eprintln!("record: capture is added in the next commit (args: {args:?})");
    2
}
