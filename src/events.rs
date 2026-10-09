use crate::process::ProcessInfo;
use crossterm::event::KeyEvent;
use std::collections::HashMap;

/// Defines all possible events that can trigger a state change or render.
#[derive(Debug)]
pub enum Event {
    /// A hardware telemetry update from sysinfo/nvidia-smi
    HardwareUpdate {
        vram_used: f64,
        ram_used: f64,
        cpu_load: u64,
        // --- GPU Telemetry ---
        gpu_temp: i32,
        gpu_power: String,
        cpu_cores: Vec<f32>,
        swap_used: f64,
        swap_total: f64,
        /// One row per PID, GPU memory merged in (REQ-PROC-001/002).
        processes: Vec<ProcessInfo>,
        /// uid → user name; empty on polls that did not refresh it.
        users: HashMap<u32, String>,
        gpu_util: String,
        vram_util: String,
        gpu_fan: String,
        gpu_clocks: String,
        sys_uptime: u64,
    },
    /// A new line intercepted from journalctl
    LogLine(String),
    /// A keyboard input from the user
    Key(KeyEvent),
    /// A scheduled tick to force a UI refresh
    Tick,
    // --- Live Streaming Events ---
    ApiStreamStart {
        ttft_ms: u128,
    },
    ApiStreamChunk(String),
    ApiStreamEnd {
        metrics: crate::interrogate::Metrics,
        status: String,
    },

    ModelsFetched(Vec<String>),
    PortAudit(String),
    /// Control API → the currently-resident model changed (reflects headless swaps in the TUI)
    ActiveModelSet(String),
    /// A background failure the operator must see (REQ-ERR-005/AC1)
    Error {
        source: String,
        message: String,
    },
}
