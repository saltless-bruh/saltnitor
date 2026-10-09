//! PID-based process control (REQ-PROC-001…007, REQ-TUI-010; INV-09). Every action targets one
//! PID, is identity-checked against the snapshot the operator acted on, and is signalled through
//! a pidfd taken at selection time where the kernel supports it.
use crate::error::ErrorCode;
use rustix::process::{Pid, PidfdFlags, Signal};
use std::os::fd::OwnedFd;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub memory_bytes: u64,
    pub gpu_memory_bytes: Option<u64>,
    /// Why `gpu_memory_bytes` is `None` (INV-18: never a silent 0).
    pub gpu_memory_reason: Option<String>,
    pub command: Option<String>,
    pub start_time_ticks: u64,
    pub uid: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GpuApp {
    pub pid: u32,
    pub name: String,
    pub used_memory_bytes: Option<u64>,
    pub reason: Option<String>,
}

/// `nvidia-smi --query-compute-apps=pid,process_name,used_memory --format=csv,noheader,nounits`
/// (`used_memory` is MiB; `[N/A]` and non-numbers become `None` with the raw text as the reason).
pub fn parse_compute_apps(csv: &str) -> Vec<GpuApp> {
    csv.lines()
        .filter_map(|line| {
            let mut f = line.split(',').map(str::trim);
            let pid = f.next()?.parse().ok()?;
            let name = f.next()?.to_string();
            let raw = f.next().unwrap_or("");
            let (used_memory_bytes, reason) = match raw.parse::<u64>() {
                Ok(mib) => (Some(mib * 1024 * 1024), None),
                Err(_) => (
                    None,
                    Some(format!("nvidia-smi reported {raw:?} for used_memory")),
                ),
            };
            Some(GpuApp {
                pid,
                name,
                used_memory_bytes,
                reason,
            })
        })
        .collect()
}

/// One entry per PID (REQ-PROC-002/AC1); GPU rows merge by PID. Rows keep input order.
pub fn table(raw: Vec<ProcessInfo>, gpu: &[GpuApp]) -> Vec<ProcessInfo> {
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<ProcessInfo> = raw.into_iter().filter(|p| seen.insert(p.pid)).collect();
    for g in gpu {
        if let Some(p) = out.iter_mut().find(|p| p.pid == g.pid) {
            p.gpu_memory_bytes = g.used_memory_bytes;
            p.gpu_memory_reason = g.reason.clone();
        } else if let Some(id) = identity(g.pid) {
            out.push(ProcessInfo {
                pid: g.pid,
                name: g.name.clone(),
                memory_bytes: 0,
                gpu_memory_bytes: g.used_memory_bytes,
                gpu_memory_reason: g.reason.clone(),
                command: None,
                start_time_ticks: id.start_time_ticks,
                uid: id.uid,
            });
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identity {
    pub start_time_ticks: u64,
    pub uid: u32,
}

/// `/proc/<pid>/stat` field 22 and `/proc/<pid>/status` `Uid:`. `None` when the PID is gone or a
/// zombie (a reaped-but-not-waited child counts as exited).
pub fn identity(pid: u32) -> Option<Identity> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after = &stat[stat.rfind(')')? + 1..];
    let fields: Vec<&str> = after.split_whitespace().collect(); // fields[0] is field 3 (state)
    if fields.first() == Some(&"Z") {
        return None;
    }
    let start_time_ticks = fields.get(19)?.parse().ok()?; // field 22
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let uid = status
        .lines()
        .find_map(|l| l.strip_prefix("Uid:"))?
        .split_whitespace()
        .next()?
        .parse()
        .ok()?;
    Some(Identity {
        start_time_ticks,
        uid,
    })
}

/// `Tgid:` from `/proc/<pid>/status`: the thread-group leader (the process) `pid` belongs to.
/// `Tgid != pid` means `pid` is a thread (TID), never a process.
pub fn tgid(pid: u32) -> Option<u32> {
    std::fs::read_to_string(format!("/proc/{pid}/status"))
        .ok()?
        .lines()
        .find_map(|l| l.strip_prefix("Tgid:"))?
        .trim()
        .parse()
        .ok()
}

/// Build the raw table from sysinfo (name, memory, command) plus `/proc` identity.
pub fn snapshot_from_sysinfo(sys: &sysinfo::System) -> Vec<ProcessInfo> {
    sys.processes()
        .values()
        // sysinfo lists every thread as its own entry; a TID is not a process (REQ-PROC-002/AC1).
        .filter(|p| p.thread_kind().is_none())
        .filter_map(|p| {
            let pid = p.pid().as_u32();
            let id = identity(pid)?;
            let command = p
                .cmd()
                .iter()
                .map(|s| s.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ");
            Some(ProcessInfo {
                pid,
                name: p.name().to_string_lossy().into_owned(),
                memory_bytes: p.memory(),
                gpu_memory_bytes: None,
                gpu_memory_reason: Some(NOT_A_GPU_PROCESS.into()),
                command: (!command.is_empty()).then_some(command),
                start_time_ticks: id.start_time_ticks,
                uid: id.uid,
            })
        })
        .collect()
}

/// Map a `pidfd_open(2)` failure (RF-4): only `ENOSYS` (kernel without pidfd) falls back to
/// `kill(2)`; `ESRCH` means the process is gone (`Changed`); anything else (`EMFILE`, `EPERM`, …)
/// is surfaced rather than silently reopening the PID-recycle window.
pub fn classify_pidfd_errno(e: rustix::io::Errno) -> Result<Option<OwnedFd>, ProcessError> {
    use rustix::io::Errno;
    match e {
        Errno::NOSYS => Ok(None),
        Errno::SRCH => Err(ProcessError::Changed(
            "the process no longer exists (pidfd_open: ESRCH)".into(),
        )),
        other => Err(ProcessError::Signal(format!(
            "pidfd_open failed: {}",
            std::io::Error::from(other)
        ))),
    }
}

/// Open a pidfd for `pid`; `Ok(None)` only where the kernel has no pidfd support.
pub fn open_pidfd(pid: u32) -> Result<Option<OwnedFd>, ProcessError> {
    let Some(p) = i32::try_from(pid).ok().and_then(Pid::from_raw) else {
        return Err(ProcessError::Changed(format!("{pid} is not a valid PID")));
    };
    match rustix::process::pidfd_open(p, PidfdFlags::empty()) {
        Ok(fd) => Ok(Some(fd)),
        Err(e) => classify_pidfd_errno(e),
    }
}

/// What the operator selected: PID + identity snapshot + a pidfd when the kernel gives one.
#[derive(Debug)]
pub struct Target {
    pub pid: u32,
    pub name: String,
    pub identity: Identity,
    pidfd: Option<OwnedFd>,
    /// A `pidfd_open` failure other than ENOSYS, reported by [`check`] before anything is signalled.
    open_error: Option<ProcessError>,
}

impl Target {
    pub fn select(info: &ProcessInfo) -> Self {
        let (pidfd, open_error) = match open_pidfd(info.pid) {
            Ok(fd) => (fd, None),
            Err(e) => (None, Some(e)),
        };
        Self {
            pid: info.pid,
            name: info.name.clone(),
            identity: Identity {
                start_time_ticks: info.start_time_ticks,
                uid: info.uid,
            },
            pidfd,
            open_error,
        }
    }
    /// The `kill(2)` fallback path (kernels without pidfd), also used by tests.
    pub fn without_pidfd(info: &ProcessInfo) -> Self {
        Self {
            pid: info.pid,
            name: info.name.clone(),
            identity: Identity {
                start_time_ticks: info.start_time_ticks,
                uid: info.uid,
            },
            pidfd: None,
            open_error: None,
        }
    }
    /// Snapshot `pid` now: identity from `/proc` plus a pidfd (REQ-PROC-004). `Changed` when the
    /// PID does not exist, is a zombie, or cannot be read.
    pub fn snapshot(pid: u32) -> Result<Self, ProcessError> {
        let identity = identity(pid)
            .ok_or_else(|| ProcessError::Changed(format!("PID {pid} does not exist")))?;
        if let Some(leader) = tgid(pid).filter(|l| *l != pid) {
            return Err(ProcessError::Protected(format!(
                "PID {pid} is a thread of PID {leader}; only whole processes can be signalled"
            )));
        }
        let name = std::fs::read_to_string(format!("/proc/{pid}/comm"))
            .map(|c| c.trim().to_string())
            .unwrap_or_default();
        let info = ProcessInfo {
            pid,
            name,
            memory_bytes: 0,
            gpu_memory_bytes: None,
            gpu_memory_reason: None,
            command: None,
            start_time_ticks: identity.start_time_ticks,
            uid: identity.uid,
        };
        Ok(Self::select(&info))
    }
    pub fn has_pidfd(&self) -> bool {
        self.pidfd.is_some()
    }
}

/// Who may be signalled (REQ-PROC-005). `own_uids` = {euid, ruid, SUDO_UID} (design P1-D8).
#[derive(Debug, Clone)]
pub struct Guard {
    pub own_uids: Vec<u32>,
    pub self_pid: u32,
    pub runtime_pids: Vec<u32>,
}
impl Guard {
    pub fn current(runtime_pids: Vec<u32>) -> Self {
        let mut own_uids = vec![
            rustix::process::geteuid().as_raw(),
            rustix::process::getuid().as_raw(),
        ];
        if let Some(s) = std::env::var("SUDO_UID").ok().and_then(|v| v.parse().ok()) {
            own_uids.push(s);
        }
        own_uids.sort_unstable();
        own_uids.dedup();
        Self {
            own_uids,
            self_pid: std::process::id(),
            runtime_pids,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessError {
    /// The PID no longer is what the operator selected (or no longer exists).
    Changed(String),
    /// Structurally or by ownership off limits.
    Protected(String),
    /// The kernel refused or failed the signal itself.
    Signal(String),
}
impl ProcessError {
    pub fn code(&self) -> ErrorCode {
        match self {
            ProcessError::Changed(_) | ProcessError::Signal(_) => ErrorCode::ProcessChanged,
            ProcessError::Protected(_) => ErrorCode::ProcessProtected,
        }
    }
    pub fn reason(&self) -> &str {
        match self {
            ProcessError::Changed(r) | ProcessError::Protected(r) | ProcessError::Signal(r) => r,
        }
    }
}
impl std::fmt::Display for ProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.reason())
    }
}
impl std::error::Error for ProcessError {}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Outcome {
    Exited,
    StillRunning,
}

/// Structural protection, then identity, then ownership. Nothing is signalled on `Err`.
pub fn check(t: &Target, g: &Guard) -> Result<(), ProcessError> {
    // A TID signals its whole thread group, so judge the leader the TID belongs to (REQ-PROC-005).
    let leader = tgid(t.pid).unwrap_or(t.pid);
    if leader == 1 {
        return Err(ProcessError::Protected("PID 1 is never a target".into()));
    }
    if leader == g.self_pid {
        return Err(ProcessError::Protected(
            "saltnitor will not signal itself".into(),
        ));
    }
    if g.runtime_pids.contains(&leader) {
        return Err(ProcessError::Protected(format!(
            "PID {} belongs to the runtime's process tree; stop the runtime through its lifecycle (Ctrl+K)",
            t.pid
        )));
    }
    if leader != t.pid {
        return Err(ProcessError::Protected(format!(
            "PID {} is a thread of PID {leader}; only whole processes can be signalled",
            t.pid
        )));
    }
    if let Some(e) = &t.open_error {
        return Err(e.clone());
    }
    match identity(t.pid) {
        Some(now) if now == t.identity => {}
        Some(now) => {
            return Err(ProcessError::Changed(format!(
                "PID {} changed since selection (start {} → {}, uid {} → {})",
                t.pid, t.identity.start_time_ticks, now.start_time_ticks, t.identity.uid, now.uid
            )));
        }
        None => {
            return Err(ProcessError::Changed(format!(
                "PID {} no longer exists",
                t.pid
            )));
        }
    }
    if !g.own_uids.contains(&t.identity.uid) {
        return Err(ProcessError::Protected(format!(
            "PID {} is owned by uid {}, not the operator",
            t.pid, t.identity.uid
        )));
    }
    Ok(())
}

fn send(t: &Target, sig: Signal) -> std::io::Result<()> {
    match &t.pidfd {
        Some(fd) => rustix::process::pidfd_send_signal(fd, sig).map_err(std::io::Error::from),
        None => {
            let pid = Pid::from_raw(i32::try_from(t.pid).unwrap_or(0))
                .ok_or_else(|| std::io::Error::other("invalid pid"))?;
            rustix::process::kill_process(pid, sig).map_err(std::io::Error::from)
        }
    }
}

/// SIGTERM the exact PID, wait up to `grace`, report (REQ-PROC-003/AC1). Blocking: call from
/// `spawn_blocking`.
pub fn terminate_guarded(t: &Target, g: &Guard, grace: Duration) -> Result<Outcome, ProcessError> {
    check(t, g)?;
    if let Err(e) = send(t, Signal::TERM) {
        return Err(ProcessError::Signal(format!(
            "SIGTERM to {} failed: {e}",
            t.pid
        )));
    }
    let deadline = Instant::now() + grace;
    loop {
        if identity(t.pid) != Some(t.identity) {
            return Ok(Outcome::Exited);
        }
        if Instant::now() >= deadline {
            return Ok(Outcome::StillRunning);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// SIGKILL, only after a separate confirmation (REQ-PROC-003/AC2).
pub fn kill_guarded(t: &Target, g: &Guard) -> Result<(), ProcessError> {
    check(t, g)?;
    send(t, Signal::KILL)
        .map_err(|e| ProcessError::Signal(format!("SIGKILL to {} failed: {e}", t.pid)))
}

/// [`terminate_guarded`] with only the structural guard (no runtime tree).
pub fn terminate(t: &Target, grace: Duration) -> Result<Outcome, ProcessError> {
    terminate_guarded(t, &Guard::current(vec![]), grace)
}

/// [`kill_guarded`] with only the structural guard (no runtime tree).
pub fn kill(t: &Target) -> Result<(), ProcessError> {
    kill_guarded(t, &Guard::current(vec![]))
}

/// The external unit's main PID and its descendants (`systemctl show -p MainPID --value`).
pub fn runtime_tree(service_name: &str) -> Vec<u32> {
    let out = std::process::Command::new("systemctl")
        .args(["show", "-p", "MainPID", "--value", service_name])
        .output();
    let Some(root) = out
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .parse::<u32>()
                .ok()
        })
        .filter(|p| *p > 0)
    else {
        return vec![];
    };
    let mut tree = vec![root];
    let mut i = 0;
    while i < tree.len() {
        let parent = tree[i];
        if let Ok(rd) = std::fs::read_dir("/proc") {
            for e in rd.flatten() {
                let Ok(pid) = e.file_name().to_string_lossy().parse::<u32>() else {
                    continue;
                };
                let ppid = std::fs::read_to_string(format!("/proc/{pid}/status"))
                    .ok()
                    .and_then(|s| {
                        s.lines()
                            .find_map(|l| l.strip_prefix("PPid:"))
                            .and_then(|v| v.trim().parse::<u32>().ok())
                    });
                if ppid == Some(parent) && !tree.contains(&pid) {
                    tree.push(pid);
                }
            }
        }
        i += 1;
    }
    tree
}

/// Operator-facing log line for a finished SIGTERM request (`grace_ms` is what was waited).
pub fn describe_terminate(
    t: &Target,
    result: &Result<Outcome, ProcessError>,
    grace_ms: u64,
) -> String {
    match result {
        Ok(Outcome::Exited) => format!(">>> PROCESS: SIGTERM {} ({}) → exited", t.pid, t.name),
        Ok(Outcome::StillRunning) => format!(
            ">>> PROCESS: SIGTERM {} ({}) → still_running after {grace_ms} ms; press X to SIGKILL",
            t.pid, t.name
        ),
        Err(e) => describe_refusal(e),
    }
}

/// Operator-facing log line for a finished SIGKILL request.
pub fn describe_kill(t: &Target, result: &Result<(), ProcessError>) -> String {
    match result {
        Ok(()) => format!(">>> PROCESS: SIGKILL {} ({}) sent", t.pid, t.name),
        Err(e) => describe_refusal(e),
    }
}

fn describe_refusal(e: &ProcessError) -> String {
    format!(">>> PROCESS: refused {}: {}", e.code().as_str(), e.reason())
}

/// Rows for the GPU inspector: those nvidia-smi reported memory for, or gave a reason it could not.
pub fn gpu_rows(all: &[ProcessInfo]) -> Vec<ProcessInfo> {
    all.iter()
        .filter(|p| {
            p.gpu_memory_bytes.is_some()
                || p.gpu_memory_reason
                    .as_deref()
                    .is_some_and(|r| r != NOT_A_GPU_PROCESS)
        })
        .cloned()
        .collect()
}

/// Rows for the CPU/RAM inspector: more than 1 MiB resident, largest first (one row per PID).
pub fn ram_rows(all: &[ProcessInfo]) -> Vec<ProcessInfo> {
    let mut rows: Vec<ProcessInfo> = all
        .iter()
        .filter(|p| p.memory_bytes > 1_048_576)
        .cloned()
        .collect();
    rows.sort_by_key(|p| std::cmp::Reverse(p.memory_bytes));
    rows
}

/// `gpu_memory_reason` for processes with no GPU involvement (not an error, not shown as GPU rows).
pub const NOT_A_GPU_PROCESS: &str = "not a GPU process";
