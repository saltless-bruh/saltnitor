use std::collections::VecDeque;
use ratatui::widgets::ListState;

/// The central state of the application.
pub struct App {
    pub should_quit: bool,

    // Hardware info
    pub cpu_name: String,
    pub cpu_core_count: usize,
    pub gpu_name: String,
    pub has_nvidia: bool,

    // CLI Configurations
    pub host: String,
    pub port: u16,
    pub service_name: String,
    
    // Telemetry State
    pub vram_used: f64,
    pub vram_total: f64,
    pub ram_used: f64,
    pub ram_total: f64,
    
    // CPU Sparkline history (storing the last 100 data points)
    pub cpu_history: Vec<u64>,

    // Deep-Dive Telemetry
    pub gpu_temp: i32,
    pub gpu_power: String,
    pub gpu_processes: Vec<(String, f64)>,
    
    pub cpu_cores: Vec<f32>,
    pub swap_used: f64,
    pub swap_total: f64,
    pub sys_processes: Vec<(String, f64)>,

    pub show_gpu_inspector: bool,
    pub show_sys_inspector: bool,

    pub gpu_util: String,
    pub vram_util: String,
    pub gpu_fan: String,
    pub gpu_clocks: String,
    pub sys_uptime: u64,

    // Config Tuner State
    pub show_tuner: bool,
    pub tuner_page: usize,     // <-- NEW: Tracks [Page 1], [Page 2], or [Page 3]
    pub tuner_selected: usize, 
    
    // Page 1: Compute & Memory
    pub current_ngl: i32,
    pub current_ctx: i32,
    pub current_threads: usize,
    pub current_batch: i32,
    pub current_parallel: i32,
    pub flash_attn: bool,
    pub mlock: bool,
    pub no_mmap: bool,
    pub cache_k_idx: usize,
    pub cache_v_idx: usize,

    // Page 2: Context & Speculation
    pub rope_base: i32,
    pub rope_scale: f32,
    pub defrag_thold: f32,
    pub draft_max: i32,
    pub draft_min: i32,

    // Page 3: Sampling Defaults
    pub temp: f32,
    pub top_k: i32,
    pub top_p: f32,
    pub min_p: f32,
    pub rep_pen: f32,

    // Help Menu State
    pub show_help: bool,

    // API Interrogator State
    pub console_focused: bool,
    pub console_input: String,
    pub console_cursor: usize,
    pub console_history: Vec<String>,
    pub history_index: usize,
    pub last_api_result: String,
    pub last_ttft: u128,
    pub last_eval_tps: f64,
    pub last_gen_tps: f64,

    // Model Selector State
    pub available_models: Vec<String>,
    pub active_model: String,
    pub show_model_selector: bool,
    pub model_selector_index: usize,

    // Auditor State
    pub port_status: String,

    // Log State
    pub logs: VecDeque<String>,
    pub log_state: ListState,
    pub auto_scroll: bool,
}

impl App {
    pub fn new(
        cpu_name: String, 
        cpu_core_count: usize, 
        ram_total: f64, 
        gpu_name: String, 
        vram_total: f64, 
        has_nvidia: bool,
        host: String,
        port: u16,
        service_name: String,
        default_ngl: i32,
        default_ctx: i32
    ) -> Self {
        let mut log_state = ListState::default();
        log_state.select(Some(0));
        
        let port_status = format!("Port {}: SCANNING...", port);

        // --- NEW: Load API History from Disk ---
        let mut console_history = Vec::new();
        if let Ok(content) = std::fs::read_to_string(".saltnitor_history") {
            for line in content.lines() {
                if !line.trim().is_empty() {
                    console_history.push(line.to_string());
                }
            }
        }
        let history_index = console_history.len();
        // ---------------------------------------
        
        Self {
            should_quit: false,
            // GPU/CPU Info
            cpu_name,
            cpu_core_count,
            gpu_name,
            has_nvidia,
            vram_total,
            ram_total,
            // CLI Configs
            host,
            port,
            service_name,
            // VRAM
            vram_used: 0.0,
            vram_util: "0%".to_string(),
            // RAM
            ram_used: 0.0,
            // CPU
            cpu_history: vec![0; 100],
            cpu_cores: vec![0.0; 16],
            // GPU
            gpu_temp: 0,
            gpu_power: "0W".to_string(),
            gpu_processes: Vec::new(),
            gpu_util: "0%".to_string(),
            gpu_fan: "0%".to_string(),
            gpu_clocks: "0 MHz".to_string(),
            // CONF
            logs: VecDeque::with_capacity(100),
            show_tuner: false,
            tuner_page: 0,
            tuner_selected: 0,
            
            // P1
            current_ngl: default_ngl,
            current_ctx: default_ctx,
            current_threads: cpu_core_count.saturating_sub(1).max(1),
            current_batch: 512,
            current_parallel: 1,
            flash_attn: false,
            mlock: false,
            no_mmap: false,
            cache_k_idx: 0,
            cache_v_idx: 0,

            // P2
            rope_base: 10000,
            rope_scale: 1.0,
            defrag_thold: -1.0, // -1 means disabled
            draft_max: 16,
            draft_min: 5,

            // P3
            temp: 0.8,
            top_k: 40,
            top_p: 0.95,
            min_p: 0.05,
            rep_pen: 1.1,

            console_focused: false,
            console_input: r#"{"model": "None", "messages": [{"role": "user", "content": "ping"}]}"#.to_string(),
            console_cursor: 69,
            console_history, 
            history_index,
            last_api_result: "Ready. Press 'i' to focus console, Enter to fire.".to_string(),
            last_ttft: 0,
            last_eval_tps: 0.0,
            last_gen_tps: 0.0,
            available_models: Vec::new(),
            active_model: "None".to_string(),
            show_model_selector: false,
            model_selector_index: 0,
            port_status,
            log_state,
            auto_scroll: true,
            show_help: false,
            swap_used: 0.0,
            swap_total: 1.0,
            sys_processes: Vec::new(),
            sys_uptime: 0,
            show_gpu_inspector: false,
            show_sys_inspector: false,
        }
    }

    /// Safely pushes a new log and pops the oldest if capacity is reached.
    pub fn add_log(&mut self, log: String) {
        if self.logs.len() == 100 {
            self.logs.pop_front();
        }
        self.logs.push_back(log);
        if self.auto_scroll {
            self.log_state.select(Some(self.logs.len().saturating_sub(1)));
        }
    }

    pub fn scroll_logs_up(&mut self) {
        self.auto_scroll = false; // Break out of auto-scroll
        let i = match self.log_state.selected() {
            Some(i) => i.saturating_sub(1),
            None => 0,
        };
        self.log_state.select(Some(i));
    }

    pub fn scroll_logs_down(&mut self) {
        let i = match self.log_state.selected() {
            Some(i) => {
                if i >= self.logs.len().saturating_sub(1) {
                    self.auto_scroll = true; // Snap back to auto-scroll if we hit the bottom
                    self.logs.len().saturating_sub(1)
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.log_state.select(Some(i));
    }
}