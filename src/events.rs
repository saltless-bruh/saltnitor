use crossterm::event::KeyEvent;

/// Defines all possible events that can trigger a state change or render.
#[derive(Debug)]
pub enum Event {
    /// A hardware telemetry update from sysinfo/nvidia-smi
    HardwareUpdate {
        vram_used: f64,
        ram_used: f64,
        cpu_load: u64,
        // --- NEW FIELDS ---
        gpu_temp: i32,
        gpu_power: String,
        gpu_processes: Vec<(String, f64)>, // Name, VRAM (GB)
        cpu_cores: Vec<f32>,
        swap_used: f64,
        swap_total: f64,
        sys_processes: Vec<(String, f64)>, // Name, RAM (GB)
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
    // Add this new event:
    ApiResponse {
        ttft_ms: u128,
        tps: f64,
        status: String,
        message: String,
    },
    ModelsFetched(Vec<String>),
    PortAudit(String),
}