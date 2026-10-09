#![allow(clippy::unwrap_used, clippy::expect_used)] // tests may unwrap: a panic is the failure signal
//! The Processes screen acts on the PID the operator selected, not on a list index
//! (REQ-TUI-010/AC1, REQ-PROC-004/AC1). The rows re-sort on every poll.
use saltnitor::app::{App, ProcPane};
use saltnitor::process::ProcessInfo;
use std::collections::HashMap;

fn row(pid: u32, name: &str, mem_mib: u64) -> ProcessInfo {
    ProcessInfo {
        pid,
        name: name.into(),
        memory_bytes: mem_mib * 1024 * 1024,
        gpu_memory_bytes: None,
        gpu_memory_reason: None,
        command: None,
        start_time_ticks: 1,
        uid: 1000,
    }
}

fn gpu_row(pid: u32, name: &str, mib: u64) -> ProcessInfo {
    ProcessInfo {
        gpu_memory_bytes: Some(mib * 1024 * 1024),
        ..row(pid, name, 0)
    }
}

fn app_with(rows: &[ProcessInfo]) -> App {
    let mut app = App::new(
        "cpu".into(),
        1,
        1.0,
        "gpu".into(),
        1.0,
        false,
        "127.0.0.1".into(),
        8080,
        "svc".into(),
        1,
        1,
    );
    app.set_processes(rows, HashMap::new());
    app
}

/// Verifies: REQ-TUI-010/AC1 — SIGTERM lands on the PID that was highlighted, even after a re-sort
#[test]
fn the_selected_pid_survives_a_resort_and_x_acts_on_it() {
    // browser 300 MiB, editor 200 MiB, wanted 100 MiB: `wanted` is highlighted (index 2).
    let mut app = app_with(&[
        row(10, "browser", 300),
        row(20, "editor", 200),
        row(30, "wanted", 100),
    ]);
    for _ in 0..3 {
        app.move_proc_cursor(ProcPane::Sys, true);
    }
    assert_eq!(app.selected_proc(ProcPane::Sys).unwrap().pid, 30);

    // The next poll: `wanted` now uses the most memory, the list is re-sorted.
    app.set_processes(
        &[
            row(10, "browser", 300),
            row(20, "editor", 200),
            row(30, "wanted", 900),
        ],
        HashMap::new(),
    );
    assert_eq!(
        app.sys_proc_state.selected(),
        Some(0),
        "the cursor follows the PID"
    );
    assert_eq!(
        app.selected_proc(ProcPane::Sys).unwrap().pid,
        30,
        "x/X must act on the PID the operator selected, not on the row now at the old index"
    );
}

/// Verifies: REQ-TUI-010/AC1 — a selected PID that vanished is cleared, never replaced by a neighbour
#[test]
fn a_vanished_pid_clears_the_selection() {
    let mut app = app_with(&[row(10, "a", 300), row(20, "b", 200), row(30, "c", 100)]);
    app.move_proc_cursor(ProcPane::Sys, true);
    app.move_proc_cursor(ProcPane::Sys, true);
    assert_eq!(app.selected_proc(ProcPane::Sys).unwrap().pid, 20);

    app.set_processes(&[row(10, "a", 300), row(30, "c", 100)], HashMap::new());
    assert_eq!(app.sys_selected_pid, None);
    assert_eq!(app.sys_proc_state.selected(), None);
    assert!(
        app.selected_proc(ProcPane::Sys).is_none(),
        "x on a vanished row does nothing"
    );
}

/// Verifies: REQ-TUI-010/AC1 — the GPU pane keeps its own PID-stable selection
#[test]
fn the_gpu_pane_selection_is_pid_stable_too() {
    let mut app = app_with(&[gpu_row(1, "x", 100), gpu_row(2, "y", 200)]);
    app.move_proc_cursor(ProcPane::Gpu, true);
    let pid = app.selected_proc(ProcPane::Gpu).unwrap().pid;
    app.set_processes(
        &[gpu_row(2, "y", 200), gpu_row(1, "x", 100)],
        HashMap::new(),
    );
    assert_eq!(app.selected_proc(ProcPane::Gpu).unwrap().pid, pid);
    assert_eq!(app.sys_selected_pid, None, "the other pane is untouched");
}

/// Verifies: REQ-TUI-010/AC1 — cursor movement wraps and updates the selected PID
#[test]
fn moving_the_cursor_wraps_and_tracks_the_pid() {
    let mut app = app_with(&[row(10, "a", 300), row(20, "b", 200)]);
    app.move_proc_cursor(ProcPane::Sys, false); // nothing selected: Up selects the first row
    assert_eq!(app.sys_selected_pid, Some(10));
    app.move_proc_cursor(ProcPane::Sys, false); // wraps to the last row
    assert_eq!(app.sys_selected_pid, Some(20));
    app.move_proc_cursor(ProcPane::Sys, true); // Down wraps to the first
    assert_eq!(app.sys_selected_pid, Some(10));
}
