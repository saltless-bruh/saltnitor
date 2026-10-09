//! G1 row 2 / D3: terminating by PID hits exactly one process (REQ-PROC-003, BD-05, INV-09).
//!
//! This is the one module that calls a Phase 1 library API instead of the binary. The API it
//! expects is specified in `tests/acceptance/README.md` ("Process-control API contract"). It
//! fails to compile until T1.8 (lib.rs, CR-7) and T1.13 (process module) land.

use std::os::unix::process::ExitStatusExt;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use saltnitor::process::{Outcome, Target, terminate};

use crate::harness::POLL;

/// A long `sleep` that is killed on drop whatever the test decides.
struct Sleeper(Child);

impl Sleeper {
    fn spawn() -> Self {
        let child = Command::new("sleep")
            .arg("300")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sleep");
        Self(child)
    }

    fn pid(&self) -> u32 {
        self.0.id()
    }

    /// Poll until the child has exited, up to `budget`.
    fn exited_within(&mut self, budget: Duration) -> Option<std::process::ExitStatus> {
        let deadline = Instant::now() + budget;
        loop {
            if let Some(status) = self.0.try_wait().expect("try_wait") {
                return Some(status);
            }
            if Instant::now() >= deadline {
                return None;
            }
            std::thread::sleep(POLL);
        }
    }
}

impl Drop for Sleeper {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Verifies: REQ-PROC-003/AC1, REQ-PROC-003/AC3
///
/// Two processes named `sleep`. Terminating a `Target` snapshotted from the first PID sends
/// SIGTERM to that PID only and reports `Exited`; the second `sleep` keeps running.
#[test]
fn terminating_one_of_two_same_named_processes_by_pid_leaves_the_other_running() {
    let mut victim = Sleeper::spawn();
    let mut bystander = Sleeper::spawn();
    assert_ne!(victim.pid(), bystander.pid());

    let target = Target::snapshot(victim.pid()).expect("snapshot the victim PID");
    let outcome = terminate(&target, Duration::from_secs(5)).expect("terminate by PID");
    assert!(
        matches!(outcome, Outcome::Exited),
        "expected Outcome::Exited for a plain `sleep`, got {outcome:?}"
    );

    let status = victim
        .exited_within(Duration::from_secs(2))
        .expect("the targeted sleep did not exit");
    assert_eq!(
        status.signal(),
        Some(libc_sigterm()),
        "the target was not ended by SIGTERM: {status:?}"
    );

    assert!(
        bystander
            .exited_within(Duration::from_millis(500))
            .is_none(),
        "the same-named bystander was killed too — name-based kill (INV-09)"
    );
}

/// SIGTERM without pulling in `libc` or `nix`: POSIX fixes it at 15 on Linux.
const fn libc_sigterm() -> i32 {
    15
}
