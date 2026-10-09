#![allow(clippy::unwrap_used, clippy::expect_used)] // tests may unwrap: a panic is the failure signal
//! Real processes, exact PIDs (REQ-PROC-001…007).
use proptest::prelude::*;
use saltnitor::error::ErrorCode;
use saltnitor::process::{
    Guard, Outcome, ProcessError, ProcessInfo, Target, check, identity, kill, kill_guarded,
    parse_compute_apps, table, terminate, terminate_guarded,
};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

fn sleeper() -> Child {
    Command::new("sleep")
        .arg("300")
        .stdout(Stdio::null())
        .spawn()
        .unwrap()
}
fn info(child: &Child) -> ProcessInfo {
    let id = identity(child.id()).expect("child is alive");
    ProcessInfo {
        pid: child.id(),
        name: "sleep".into(),
        memory_bytes: 0,
        gpu_memory_bytes: None,
        gpu_memory_reason: None,
        command: None,
        start_time_ticks: id.start_time_ticks,
        uid: id.uid,
    }
}
fn own_guard() -> Guard {
    Guard::current(vec![])
}
fn alive(pid: u32) -> bool {
    identity(pid).is_some()
}

/// Verifies: REQ-PROC-003/AC1, REQ-PROC-003/AC3, REQ-PROC-006/AC1
#[test]
fn terminating_one_of_two_same_named_processes_leaves_the_other_alive() {
    let (mut a, mut b) = (sleeper(), sleeper());
    let t = Target::select(&info(&a));
    assert_eq!(
        terminate_guarded(&t, &own_guard(), Duration::from_secs(5)).unwrap(),
        Outcome::Exited
    );
    assert!(alive(b.id()), "the same-named sibling must keep running");
    let _ = a.wait();
    b.kill().unwrap();
    let _ = b.wait();
}

/// Verifies: REQ-PROC-003/AC1, REQ-PROC-003/AC2
#[test]
fn sigterm_ignorer_is_still_running_then_kill_ends_it() {
    let mut c = Command::new("sh")
        .args(["-c", "trap '' TERM; exec sleep 300"])
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    std::thread::sleep(Duration::from_millis(200)); // let exec happen
    let t = Target::select(&info(&c));
    assert_eq!(
        terminate_guarded(&t, &own_guard(), Duration::from_millis(300)).unwrap(),
        Outcome::StillRunning
    );
    assert!(alive(c.id()));
    kill_guarded(&t, &own_guard()).unwrap();
    let _ = c.wait();
    assert!(!alive(c.id()));
}

/// Verifies: REQ-PROC-004/AC1 — [RF-4] identity drift between snapshot and action
#[test]
fn changed_identity_is_refused_and_nothing_is_signalled() {
    let mut c = sleeper();
    let mut stale = info(&c);
    stale.start_time_ticks += 1; // a recycled PID would differ here
    let t = Target::without_pidfd(&stale);
    let e = terminate_guarded(&t, &own_guard(), Duration::from_millis(100)).unwrap_err();
    assert!(matches!(e, ProcessError::Changed(_)));
    assert_eq!(e.code(), ErrorCode::ProcessChanged);
    assert!(alive(c.id()));
    c.kill().unwrap();
    let _ = c.wait();
}

/// Verifies: REQ-PROC-005/AC1
#[test]
fn protected_targets_are_refused_with_a_reason() {
    let one = Target::without_pidfd(&ProcessInfo {
        pid: 1,
        name: "init".into(),
        memory_bytes: 0,
        gpu_memory_bytes: None,
        gpu_memory_reason: None,
        command: None,
        start_time_ticks: 0,
        uid: 0,
    });
    assert!(
        matches!(check(&one, &own_guard()), Err(ProcessError::Protected(r)) if r.contains("PID 1"))
    );
    let my = identity(std::process::id()).unwrap();
    let me = ProcessInfo {
        pid: std::process::id(),
        name: "saltnitor".into(),
        memory_bytes: 0,
        gpu_memory_bytes: None,
        gpu_memory_reason: None,
        command: None,
        start_time_ticks: my.start_time_ticks,
        uid: my.uid,
    };
    assert!(
        matches!(check(&Target::without_pidfd(&me), &own_guard()), Err(ProcessError::Protected(r)) if r.contains("itself"))
    );
    let mut c = sleeper();
    let g = Guard {
        own_uids: vec![],
        self_pid: std::process::id(),
        runtime_pids: vec![],
    };
    assert!(
        matches!(check(&Target::select(&info(&c)), &g), Err(ProcessError::Protected(r)) if r.contains("owned by uid"))
    );
    let g = Guard {
        own_uids: own_guard().own_uids,
        self_pid: std::process::id(),
        runtime_pids: vec![c.id()],
    };
    assert!(
        matches!(check(&Target::select(&info(&c)), &g), Err(ProcessError::Protected(r)) if r.contains("runtime"))
    );
    c.kill().unwrap();
    let _ = c.wait();
}

/// Verifies: REQ-PROC-004/AC2 (pidfd where available; kill(2) fallback is exercised explicitly)
#[test]
fn pidfd_is_taken_at_selection_and_the_fallback_also_works() {
    let mut c = sleeper();
    let t = Target::select(&info(&c));
    assert!(t.has_pidfd(), "Linux ≥ 5.3 provides pidfd_open");
    let fallback = Target::without_pidfd(&info(&c));
    assert_eq!(
        terminate_guarded(&fallback, &own_guard(), Duration::from_secs(5)).unwrap(),
        Outcome::Exited
    );
    let _ = c.wait();
}

/// Verifies: REQ-PROC-003/AC1, REQ-PROC-004/AC1 — the contract entry points (`snapshot`, `terminate`, `kill`)
#[test]
fn snapshot_then_terminate_and_kill_use_the_default_guard() {
    let mut a = sleeper();
    let t = Target::snapshot(a.id()).unwrap();
    assert!(t.has_pidfd());
    assert_eq!(
        terminate(&t, Duration::from_secs(5)).unwrap(),
        Outcome::Exited
    );
    let _ = a.wait();

    let mut b = Command::new("sh")
        .args(["-c", "trap '' TERM; exec sleep 300"])
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    std::thread::sleep(Duration::from_millis(200)); // let exec happen
    let t = Target::snapshot(b.id()).unwrap();
    assert_eq!(
        terminate(&t, Duration::from_millis(300)).unwrap(),
        Outcome::StillRunning
    );
    kill(&t).unwrap();
    let _ = b.wait();
    assert!(!alive(b.id()));
}

/// Verifies: REQ-PROC-004/AC1 — a PID that does not exist is `Changed`, never signalled
#[test]
fn snapshot_of_a_missing_pid_is_a_changed_error() {
    let mut c = sleeper();
    let pid = c.id();
    c.kill().unwrap();
    let _ = c.wait();
    let e = Target::snapshot(pid).unwrap_err();
    assert!(matches!(e, ProcessError::Changed(_)));
    assert!(e.to_string().contains("does not exist"), "{e}");
}

/// Verifies: REQ-PROC-005/AC1 — the unguarded entry points still refuse PID 1 and saltnitor itself
#[test]
fn default_guard_refuses_pid_1_and_self() {
    let me = Target::snapshot(std::process::id()).unwrap();
    let e = terminate(&me, Duration::from_millis(10)).unwrap_err();
    assert_eq!(e.code(), ErrorCode::ProcessProtected);
    assert!(matches!(kill(&me), Err(ProcessError::Protected(_))));
}

/// Verifies: REQ-PROC-007/AC1
#[test]
fn nvidia_smi_compute_apps_parse_with_null_memory_and_a_reason() {
    let csv = "4242, /usr/bin/llama-server, 6144\n4243, python3, [N/A]\n";
    let apps = parse_compute_apps(csv);
    assert_eq!(apps[0].pid, 4242);
    assert_eq!(apps[0].used_memory_bytes, Some(6144 * 1024 * 1024));
    assert_eq!(apps[1].used_memory_bytes, None);
    assert!(apps[1].reason.as_deref().unwrap().contains("[N/A]"));
}

fn raw_row() -> impl Strategy<Value = ProcessInfo> {
    (
        1u32..50_000,
        prop_oneof![Just("sleep"), Just("python3"), Just("llama-server")],
        0u64..1 << 34,
    )
        .prop_map(|(pid, name, mem)| ProcessInfo {
            pid,
            name: name.into(),
            memory_bytes: mem,
            gpu_memory_bytes: None,
            gpu_memory_reason: None,
            command: None,
            start_time_ticks: 1,
            uid: 1000,
        })
}

proptest! {
    /// Verifies: REQ-PROC-001/AC1, REQ-PROC-002/AC1 [PROP]
    #[test]
    fn listing_has_exactly_one_entry_per_pid_whatever_the_names(rows in prop::collection::vec(raw_row(), 0..40)) {
        let out = table(rows.clone(), &[]);
        let mut pids: Vec<u32> = rows.iter().map(|r| r.pid).collect();
        pids.sort(); pids.dedup();
        let mut got: Vec<u32> = out.iter().map(|r| r.pid).collect();
        got.sort();
        prop_assert_eq!(got.clone(), pids);
        prop_assert_eq!(got.len(), out.len(), "no duplicate PIDs");
    }
}
