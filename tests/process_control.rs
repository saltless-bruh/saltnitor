#![allow(clippy::unwrap_used, clippy::expect_used)] // tests may unwrap: a panic is the failure signal
//! Real processes, exact PIDs (REQ-PROC-001…007).
use proptest::prelude::*;
use saltnitor::error::ErrorCode;
use saltnitor::process::{
    Guard, Outcome, ProcessError, ProcessInfo, Target, check, classify_pidfd_errno, identity, kill,
    kill_guarded, open_pidfd, parse_compute_apps, snapshot_from_sysinfo, table, terminate,
    terminate_guarded,
};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
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

/// Keeps `n` extra threads alive in this (multi-threaded) test process and returns exactly their
/// TIDs (each thread reads its own id from `/proc/thread-self`); dropping the sender releases them.
fn spawn_threads(n: usize) -> (Vec<u32>, mpsc::Sender<()>) {
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let stop_rx = std::sync::Arc::new(std::sync::Mutex::new(stop_rx));
    let (id_tx, id_rx) = mpsc::channel::<u32>();
    for _ in 0..n {
        let (rx, id_tx) = (stop_rx.clone(), id_tx.clone());
        std::thread::spawn(move || {
            let link = std::fs::read_link("/proc/thread-self").unwrap();
            let tid = link.file_name().unwrap().to_string_lossy().parse().unwrap();
            id_tx.send(tid).unwrap();
            let _ = rx.lock().unwrap().recv();
        });
    }
    let tids = (0..n).map(|_| id_rx.recv().unwrap()).collect();
    (tids, stop_tx)
}

fn info_for(pid: u32) -> ProcessInfo {
    let id = identity(pid).expect("pid is alive");
    ProcessInfo {
        pid,
        name: "thread".into(),
        memory_bytes: 0,
        gpu_memory_bytes: None,
        gpu_memory_reason: None,
        command: None,
        start_time_ticks: id.start_time_ticks,
        uid: id.uid,
    }
}

/// Verifies: REQ-PROC-002/AC1 — sysinfo lists every thread as its own entry; the table must not.
#[test]
fn process_table_has_one_row_per_pid_and_no_thread_rows() {
    let (tids, stop) = spawn_threads(3);
    assert!(!tids.is_empty(), "the test process has extra threads");
    let mut sys = sysinfo::System::new();
    sys.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::All,
        true,
        sysinfo::ProcessRefreshKind::everything(),
    );
    let rows = snapshot_from_sysinfo(&sys);
    let pids: Vec<u32> = rows.iter().map(|r| r.pid).collect();
    for tid in &tids {
        assert!(
            !pids.contains(tid),
            "thread {tid} must not be a process row"
        );
    }
    assert!(
        pids.contains(&std::process::id()),
        "the process itself is listed"
    );
    let mut dedup = pids.clone();
    dedup.sort_unstable();
    dedup.dedup();
    assert_eq!(dedup.len(), pids.len(), "exactly one row per PID");
    let _ = stop.send(());
}

/// Verifies: REQ-PROC-005/AC1 — a thread of saltnitor itself is not a target (no self-guard bypass)
#[test]
fn a_thread_of_saltnitor_is_refused_not_signalled() {
    let (tids, stop) = spawn_threads(1);
    let tid = tids[0];
    let e = Target::snapshot(tid).unwrap_err();
    assert!(matches!(e, ProcessError::Protected(_)), "{e:?}");
    assert_eq!(e.code(), ErrorCode::ProcessProtected);

    let t = Target::without_pidfd(&info_for(tid));
    let e = terminate_guarded(&t, &own_guard(), Duration::from_millis(10)).unwrap_err();
    assert_eq!(e.code(), ErrorCode::ProcessProtected, "{e:?}");
    assert!(matches!(
        kill_guarded(&t, &own_guard()),
        Err(ProcessError::Protected(_))
    ));
    let _ = stop.send(());
}

/// Verifies: REQ-PROC-005/AC1 — a thread of a runtime-tree process is protected as the runtime
#[test]
fn a_thread_of_a_runtime_process_is_protected_as_the_runtime() {
    let (tids, stop) = spawn_threads(1);
    let g = Guard {
        own_uids: own_guard().own_uids,
        self_pid: 1, // pretend we are not this process: only the runtime list names it
        runtime_pids: vec![std::process::id()],
    };
    let t = Target::without_pidfd(&info_for(tids[0]));
    let e = check(&t, &g).unwrap_err();
    assert!(
        matches!(&e, ProcessError::Protected(r) if r.contains("runtime")),
        "{e:?}"
    );
    let _ = stop.send(());
}

/// Verifies: REQ-PROC-004/AC1 — UID drift (not only start time) is `PROCESS_CHANGED`
#[test]
fn changed_uid_is_refused_and_nothing_is_signalled() {
    let mut c = sleeper();
    let mut stale = info(&c);
    stale.uid += 1;
    let e = terminate_guarded(
        &Target::without_pidfd(&stale),
        &own_guard(),
        Duration::from_millis(100),
    )
    .unwrap_err();
    assert!(matches!(e, ProcessError::Changed(_)), "{e:?}");
    assert!(alive(c.id()));
    c.kill().unwrap();
    let _ = c.wait();
}

/// Verifies: REQ-PROC-004/AC2 — only ENOSYS falls back to kill(2); ESRCH is `Changed`, the rest surface
#[test]
fn pidfd_open_errors_are_classified_not_swallowed() {
    use rustix::io::Errno;
    assert!(matches!(
        classify_pidfd_errno(Errno::SRCH),
        Err(ProcessError::Changed(_))
    ));
    assert!(matches!(classify_pidfd_errno(Errno::NOSYS), Ok(None)));
    assert!(matches!(
        classify_pidfd_errno(Errno::MFILE),
        Err(ProcessError::Signal(_))
    ));
    assert!(matches!(
        classify_pidfd_errno(Errno::PERM),
        Err(ProcessError::Signal(_))
    ));

    let mut c = sleeper();
    let pid = c.id();
    assert!(
        open_pidfd(pid).unwrap().is_some(),
        "a live PID gets a pidfd"
    );
    c.kill().unwrap();
    let _ = c.wait();
    assert!(
        matches!(open_pidfd(pid), Err(ProcessError::Changed(_))),
        "a reaped PID is Changed"
    );
}

/// Verifies: REQ-PROC-004/AC1 — a selection whose pidfd could not be opened (PID gone) is refused as changed
#[test]
fn selecting_a_pid_that_vanished_is_refused_as_changed() {
    let mut c = sleeper();
    let stale = info(&c);
    c.kill().unwrap();
    let _ = c.wait();
    let t = Target::select(&stale);
    assert!(!t.has_pidfd());
    let e = terminate_guarded(&t, &own_guard(), Duration::from_millis(10)).unwrap_err();
    assert!(matches!(e, ProcessError::Changed(_)), "{e:?}");
}
