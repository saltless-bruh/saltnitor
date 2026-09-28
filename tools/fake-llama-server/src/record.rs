//! R0 record/replay (CR-2). This task provides the replay lookup the engine calls;
//! Task 7 adds capture, drift, sanitizing and the `record` CLI.
use std::path::Path;

/// Recorded (status, content type, body) for `path` in the capture at `dir`, if any.
pub fn replay_file(_dir: &Path, _path: &str) -> Option<(u16, String, Vec<u8>)> {
    None
}
