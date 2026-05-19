mod app;
mod events;
mod ui;

use serde::Deserialize;
use app::App;
use events::Event;
use crossterm::{
    event::{self, Event as CEvent, KeyCode, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use reqwest::Client;
use std::time::Instant;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, time::Duration};
use tokio::sync::mpsc;
use sysinfo::{System, CpuRefreshKind, RefreshKind, MemoryRefreshKind};
use tokio::process::Command;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use clap::Parser;

/// Saltnitor: High-performance hybrid hardware monitor and LLM orchestrator.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short, long)]
    port: Option<u16>, // Changed to Option so we know if the user explicitly typed it

    #[arg(long)]
    host: Option<String>,

    #[arg(short, long)]
    service_name: Option<String>,
}

// --- TOML Configuration Struct ---
#[derive(Deserialize, Default, Debug)]
struct TomlConfig {
    port: Option<u16>,
    host: Option<String>,
    service_name: Option<String>,
    default_ngl: Option<i32>,
    default_ctx: Option<i32>,
}

// --- Sudo-Aware Config Loader ---
fn load_config() -> TomlConfig {
    let mut config_path = std::path::PathBuf::new();
    
    // Intelligently bypass the Sudo Trap
    if let Ok(sudo_user) = std::env::var("SUDO_USER") {
        config_path.push(format!("/home/{}/.config/saltnitor/config.toml", sudo_user));
    } else if let Some(home) = std::env::var_os("HOME") {
        config_path.push(home);
        config_path.push(".config/saltnitor/config.toml");
    } else {
        return TomlConfig::default();
    }

    if let Ok(content) = std::fs::read_to_string(config_path) {
        toml::from_str(&content).unwrap_or_default()
    } else {
        TomlConfig::default()
    }
}

// --- Pre-Flight Dependency Checker ---
fn check_dependencies() -> Result<(), String> {
    let required_cmds = ["journalctl", "ss", "systemctl", "killall"];
    let mut missing = Vec::new();

    for cmd in required_cmds {
        // Use the POSIX standard 'command -v' to safely check if a binary exists in the system PATH
        let is_installed = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("command -v {}", cmd))
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !is_installed {
            missing.push(cmd);
        }
    }

    if !missing.is_empty() {
        return Err(format!("Missing critical Linux dependencies: {}", missing.join(", ")));
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 0. Parse Command Line Arguments
    let cli = Cli::parse();
    let toml_conf = load_config();

    // 1. Load Configuration
    let final_port = cli.port.or(toml_conf.port).unwrap_or(8080);
    let final_host = cli
        .host
        .or(toml_conf.host)
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let final_svc = cli
        .service_name
        .or(toml_conf.service_name)
        .unwrap_or_else(|| "llama-router".to_string());
    let final_ngl = toml_conf.default_ngl.unwrap_or(33);
    let final_ctx = toml_conf.default_ctx.unwrap_or(8192);

    // --- Enforce Pre-Flight Checks ---
    if let Err(e) = check_dependencies() {
        eprintln!("\n[!] SALTNITOR BOOT SEQUENCE HALTED");
        eprintln!("[!] {}", e);
        eprintln!("[!] Please install the required packages (e.g., 'iproute2', 'psmisc', 'systemd') and try again.\n");
        std::process::exit(1);
    }

    // 2. Pre-Flight Hardware Scan
    let mut sys = System::new_all();
    sys.refresh_cpu_specifics(CpuRefreshKind::everything());
    sys.refresh_memory();
    
    // Get Dynamic CPU & RAM
    let cpu_name = sys.cpus().first().map(|c| c.brand().to_string()).unwrap_or_else(|| "Unknown CPU".to_string());
    let cpu_core_count = sys.cpus().len();
    let ram_total = sys.total_memory() as f64 / 1_073_741_824.0;

    // Probing for NVIDIA GPU
    let mut gpu_name = "NO NVIDIA GPU DETECTED".to_string();
    let mut vram_total = 1.0; // Fallback to prevent divide-by-zero
    let mut has_nvidia = false;

    if let Ok(output) = std::process::Command::new("nvidia-smi")
        .args(["--query-gpu=name,memory.total", "--format=csv,noheader,nounits"])
        .output()
    {
        if output.status.success() {
            let out = String::from_utf8_lossy(&output.stdout);
            let parts: Vec<&str> = out.trim().split(", ").collect();
            if parts.len() == 2 {
                gpu_name = parts[0].to_string();
                vram_total = parts[1].parse::<f64>().unwrap_or(1.0) / 1024.0;
                has_nvidia = true;
            }
        }
    }

    // 3. Terminal Initialization
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 4. Application State & Channels
    let mut app = App::new(
        cpu_name, cpu_core_count, ram_total, gpu_name, vram_total, has_nvidia,
        final_host.clone(), final_port, final_svc.clone(), final_ngl, final_ctx
    );
    let (tx, mut rx) = mpsc::channel::<Event>(100);

    // 5. Start Event Producers (Background Tasks)
    let tx_keys = tx.clone();
    let tx_logs = tx.clone();

    // Task A: Keyboard Input Stream
    tokio::spawn(async move {
        loop {
            if event::poll(Duration::from_millis(250)).unwrap() {
                if let CEvent::Key(key) = event::read().unwrap() {
                    if tx_keys.send(Event::Key(key)).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    // Task B: Hardware Poller (CPU, RAM, VRAM)
    let has_nvidia = app.has_nvidia; // Capture flag for the worker
    let tx_hw = tx.clone();
    tokio::spawn(async move {
        // Note: sysinfo 0.30+ requires ProcessRefreshKind to get process memory
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate};
        let mut sys = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything())
                .with_processes(ProcessRefreshKind::everything()),
        );

        loop {
            tokio::time::sleep(Duration::from_millis(1000)).await;
            sys.refresh_cpu_specifics(CpuRefreshKind::everything());
            sys.refresh_memory();
            sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::everything());

            // System Metrics
            let ram_used = sys.used_memory() as f64 / 1_073_741_824.0; 
            let swap_used = sys.used_swap() as f64 / 1_073_741_824.0;
            let swap_total = sys.total_swap() as f64 / 1_073_741_824.0;
            let sys_uptime = System::uptime(); 
            
            let cpu_cores: Vec<f32> = sys.cpus().iter().map(|c| c.cpu_usage()).collect();
            let cpu_load = cpu_cores.iter().sum::<f32>() / cpu_cores.len() as f32;

            // Top 8 System RAM Processes
            let mut procs: Vec<_> = sys.processes().values().collect();
            procs.sort_by(|a, b| b.memory().cmp(&a.memory()));
            let sys_processes: Vec<(String, f64)> = procs.iter().take(8).map(|p| {
                (p.name().to_string_lossy().to_string(), p.memory() as f64 / 1_073_741_824.0)
            }).collect();

            // NVIDIA Metrics (General)
            let mut vram_used = 0.0;
            let mut gpu_temp = 0;
            let mut gpu_power = String::from("N/A");
            let mut gpu_util = String::from("0");
            let mut vram_util = String::from("0");
            let mut gpu_fan = String::from("N/A");
            let mut gpu_clocks = String::from("N/A");
            
            if has_nvidia {
                // Expanded query to grab 9 specific data points at once
                if let Ok(output) = std::process::Command::new("nvidia-smi")
                    .args(["--query-gpu=memory.used,temperature.gpu,power.draw,power.limit,utilization.gpu,utilization.memory,fan.speed,clocks.gr,clocks.mem", "--format=csv,noheader,nounits"])
                    .output()
                {
                    let out = String::from_utf8_lossy(&output.stdout);
                    let parts: Vec<&str> = out.trim().split(", ").collect();
                    if parts.len() >= 9 {
                        vram_used = parts[0].parse::<f64>().unwrap_or(0.0) / 1024.0;
                        gpu_temp = parts[1].parse::<i32>().unwrap_or(0);
                        gpu_power = format!("{}W / {}W", parts[2], parts[3]);
                        gpu_util = parts[4].to_string();
                        vram_util = parts[5].to_string();
                        gpu_fan = parts[6].to_string();
                        gpu_clocks = format!("{} MHz / {} MHz", parts[7], parts[8]); // Core / Mem
                    }
                }
            }

            // NVIDIA Metrics (Processes)
            let mut gpu_processes: Vec<(String, f64)> = Vec::new();
            if has_nvidia {
                if let Ok(output) = std::process::Command::new("nvidia-smi")
                    .args(["--query-compute-apps=process_name,used_memory", "--format=csv,noheader,nounits"])
                    .output()
                {
                    let out = String::from_utf8_lossy(&output.stdout);
                    for line in out.lines() {
                        let parts: Vec<&str> = line.split(", ").collect();
                        if parts.len() == 2 {
                            let name = parts[0].split('/').last().unwrap_or(parts[0]).to_string(); // Get just the exe name
                            let mem = parts[1].parse::<f64>().unwrap_or(0.0) / 1024.0;
                            gpu_processes.push((name, mem));
                        }
                    }
                }
            }

            let _ = tx_hw.send(Event::HardwareUpdate {
                vram_used, ram_used, cpu_load: cpu_load as u64,
                gpu_temp, gpu_power, gpu_processes,
                cpu_cores, swap_used, swap_total, sys_processes,
                gpu_util, vram_util, gpu_fan, gpu_clocks, sys_uptime,
            }).await;

            let _ = tx_hw.send(Event::Tick).await;
        }
    });

    // Task C: Journalctl Log Streamer
    let service_c = final_svc.clone();
    tokio::spawn(async move {
        // Stream the dynamic service logs asynchronously
        let mut child = Command::new("journalctl")
            .args(["-u", &service_c, "-f", "-n", "30"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to spawn journalctl");

        let stdout = child.stdout.take().expect("Failed to capture stdout");
        let mut reader = BufReader::new(stdout).lines();

        while let Ok(Some(line)) = reader.next_line().await {
            if tx_logs.send(Event::LogLine(line)).await.is_err() {
                break;
            }
        }
    });

    // Task D: Background Model Discovery
    let tx_models = tx.clone();
    let host_d = final_host.clone();
    let port_d = final_port;
    tokio::spawn(async move {
        let client = reqwest::Client::new();
        loop {
            // Ping the router's model manifest endpoint dynamically
            let url = format!("http://{}:{}/v1/models", host_d, port_d);
            if let Ok(res) = client.get(&url).send().await {
                if let Ok(json) = res.json::<serde_json::Value>().await {
                    if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                        let models: Vec<String> = data.iter()
                            .filter_map(|m| m.get("id").and_then(|id| id.as_str()).map(|s| s.to_string()))
                            .collect();
                        
                        let _ = tx_models.send(Event::ModelsFetched(models)).await;
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });

    // Task E: Background Session & Port Auditor
    let tx_port = tx.clone();
    let port_e = final_port;
    tokio::spawn(async move {
        loop {
            // Use 'ss' to dynamically look for processes holding our target port
            let cmd = format!("ss -lptn 'sport = :{}'", port_e);
            let output = tokio::process::Command::new("sh")
                .arg("-c")
                .arg(&cmd)
                .output()
                .await;

            let status_msg = if let Ok(out) = output {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let port_str = port_e.to_string();

                if stdout.trim().is_empty() || !stdout.contains(&port_str) {
                    format!("Port {}: OFFLINE (Daemon Down)", port_e)
                } else if stdout.contains("llama-server") || stdout.contains("llama-se") {
                    format!("Port {}: SECURE (llama-server bound)", port_e)
                } else {
                    // --- UPGRADED LOGIC: Extract the actual process name ---
                    let mut proc_name = "UNKNOWN (Requires Sudo?)".to_string();
                    if let Some(users_idx) = stdout.find("users:((\"") {
                        let start = users_idx + 9;
                        if let Some(end) = stdout[start..].find('\"') {
                            proc_name = stdout[start..start+end].to_string();
                        }
                    }
                    format!("Port {}: BLOCKED BY [{}]", port_e, proc_name)
                }
            } else {
                format!("Port {}: AUDIT ERROR", port_e)
            };

            let _ = tx_port.send(Event::PortAudit(status_msg)).await;
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    });

    // 5. The Render & Control Loop
    loop {
        terminal.draw(|f| ui::draw(f, &mut app))?;

        tokio::select! {
            Some(event) = rx.recv() => {
                match event {
                    Event::Key(key) => {
                        if app.show_model_selector {
                            // --- MODEL SELECTOR CONTROLS ---
                            match key.code {
                                KeyCode::Esc | KeyCode::Char('m') => app.show_model_selector = false,
                                KeyCode::Up => {
                                    if app.model_selector_index > 0 { app.model_selector_index -= 1; }
                                }
                                KeyCode::Down => {
                                    if app.model_selector_index < app.available_models.len().saturating_sub(1) { app.model_selector_index += 1; }
                                }
                                KeyCode::Enter => {
                                    if !app.available_models.is_empty() {
                                        let selected_model = &app.available_models[app.model_selector_index];
                                        
                                        // --- NEW: Update the active model state ---
                                        app.active_model = selected_model.clone();
                                        
                                        // Auto-inject the selected model into the JSON payload
                                        app.console_input = format!(r#"{{"model": "{}", "messages": [{{"role": "user", "content": "ping"}}]}}"#, selected_model);
                                        app.add_log(format!(">>> API Target locked to: {}", selected_model));
                                        app.console_cursor = app.console_input.chars().count();
                                    }
                                    app.show_model_selector = false;
                                }
                                _ => {}
                            }
                        } else if app.show_tuner {
                            // --- DEEP TUNER MENU CONTROLS ---
                            match key.code {
                                KeyCode::Esc | KeyCode::Char('t') => app.show_tuner = false,
                                KeyCode::Tab => {
                                    app.tuner_page = (app.tuner_page + 1) % 3; // Cycle 0, 1, 2
                                    app.tuner_selected = 0; // Reset cursor to top
                                }
                                KeyCode::Up => { if app.tuner_selected > 0 { app.tuner_selected -= 1; } }
                                KeyCode::Down => {
                                    // Limit bounds based on the active page
                                    let max_idx = match app.tuner_page { 0 => 9, 1 => 6, 2 => 4, _ => 0 };
                                    if app.tuner_selected < max_idx { app.tuner_selected += 1; }
                                }
                                KeyCode::Left | KeyCode::Right => {
                                    let is_right = key.code == KeyCode::Right;
                                    match app.tuner_page {
                                        0 => match app.tuner_selected {
                                            0 => if is_right && app.current_ngl < 99 { app.current_ngl += 1; } else if !is_right && app.current_ngl > 0 { app.current_ngl -= 1; },
                                            1 => if is_right && app.current_ctx < 131072 { app.current_ctx += 1024; } else if !is_right && app.current_ctx > 1024 { app.current_ctx -= 1024; },
                                            2 => if is_right && app.current_threads < app.cpu_core_count { app.current_threads += 1; } else if !is_right && app.current_threads > 1 { app.current_threads -= 1; },
                                            3 => if is_right && app.current_batch < 8192 { app.current_batch *= 2; } else if !is_right && app.current_batch > 128 { app.current_batch /= 2; },
                                            4 => if is_right && app.current_parallel < 16 { app.current_parallel += 1; } else if !is_right && app.current_parallel > 1 { app.current_parallel -= 1; },
                                            5 => { app.flash_attn = !app.flash_attn; },
                                            6 => { app.mlock = !app.mlock; },
                                            7 => { app.no_mmap = !app.no_mmap; },
                                            8 => if is_right && app.cache_k_idx < 3 { app.cache_k_idx += 1; } else if !is_right && app.cache_k_idx > 0 { app.cache_k_idx -= 1; },
                                            9 => if is_right && app.cache_v_idx < 3 { app.cache_v_idx += 1; } else if !is_right && app.cache_v_idx > 0 { app.cache_v_idx -= 1; },
                                            _ => {}
                                        },
                                        1 => match app.tuner_selected {
                                            0 => if is_right { app.rope_base += 10000; } else if !is_right && app.rope_base > 10000 { app.rope_base -= 10000; },
                                            1 => if is_right { app.rope_scale += 0.5; } else if !is_right && app.rope_scale > 1.0 { app.rope_scale -= 0.5; },
                                            2 => if is_right && app.defrag_thold < 1.0 { app.defrag_thold += 0.1; } else if !is_right && app.defrag_thold > -1.0 { app.defrag_thold -= 0.1; },
                                            3 => if is_right { app.draft_max += 1; } else if !is_right && app.draft_max > 1 { app.draft_max -= 1; },
                                            4 => if is_right { app.draft_min += 1; } else if !is_right && app.draft_min > 1 { app.draft_min -= 1; },
                                            5 => { app.prompt_cache = !app.prompt_cache; },
                                            6 => { app.prompt_cache_all = !app.prompt_cache_all; },
                                            _ => {}
                                        },
                                        2 => match app.tuner_selected {
                                            0 => if is_right && app.temp < 2.0 { app.temp += 0.1; } else if !is_right && app.temp > 0.0 { app.temp -= 0.1; },
                                            1 => if is_right { app.top_k += 5; } else if !is_right && app.top_k > 0 { app.top_k -= 5; },
                                            2 => if is_right && app.top_p < 1.0 { app.top_p += 0.05; } else if !is_right && app.top_p > 0.0 { app.top_p -= 0.05; },
                                            3 => if is_right && app.min_p < 1.0 { app.min_p += 0.05; } else if !is_right && app.min_p > 0.0 { app.min_p -= 0.05; },
                                            4 => if is_right && app.rep_pen < 2.0 { app.rep_pen += 0.05; } else if !is_right && app.rep_pen > 1.0 { app.rep_pen -= 0.05; },
                                            _ => {}
                                        },
                                        _ => {}
                                    }
                                }
                                KeyCode::Enter => {
                                    let cache_types = ["f16", "q8_0", "q4_0", "q4_1"];
                                    let p_cache_val = if app.prompt_cache { "prompt_cache.bin" } else { "" };
                                    let p_cache_all_val = if app.prompt_cache_all { "true" } else { "false" };
                                    let ini_content = format!(
                                        "[model]\nngl = {}\nctx-size = {}\nthreads = {}\nn-batch = {}\nparallel = {}\nflash-attn = {}\nmlock = {}\nno-mmap = {}\ncache-type-k = {}\ncache-type-v = {}\nrope-freq-base = {}\nrope-scale = {}\ndefrag-thold = {}\ndraft-max = {}\nprompt-cache = {}\nprompt-cache-all = {}\ntemperature = {}\ntop-k = {}\ntop-p = {}\n", 
                                        app.current_ngl, app.current_ctx, app.current_threads, app.current_batch, app.current_parallel,
                                        app.flash_attn, app.mlock, app.no_mmap, cache_types[app.cache_k_idx], cache_types[app.cache_v_idx],
                                        app.rope_base, app.rope_scale, app.defrag_thold, app.draft_max, p_cache_val, p_cache_all_val, app.temp, app.top_k, app.top_p
                                    );
                                    app.add_log(format!(">>> DEEP CONFIG APPLIED: Page 1-3 Saved to router.ini"));
                                    app.show_tuner = false;
                                    tokio::spawn(async move { let _ = tokio::fs::write("router.ini", ini_content).await; });
                                }
                                _ => {}
                            }
                        } else if app.console_focused {
                            // --- API INTERROGATOR CONTROLS ---
                            match key.code {
                                KeyCode::Esc => app.console_focused = false, // Exit insert mode
                                KeyCode::Left => {
                                    if app.console_cursor > 0 { app.console_cursor -= 1; }
                                }
                                KeyCode::Right => {
                                    if app.console_cursor < app.console_input.chars().count() { app.console_cursor += 1; }
                                }
                                KeyCode::Up => {
                                    if !app.console_history.is_empty() && app.history_index > 0 {
                                        app.history_index -= 1;
                                        app.console_input = app.console_history[app.history_index].clone();
                                        app.console_cursor = app.console_input.chars().count(); // Snap cursor to end
                                    }
                                }
                                KeyCode::Down => {
                                    if app.history_index < app.console_history.len() {
                                        app.history_index += 1;
                                        if app.history_index == app.console_history.len() {
                                            app.console_input = String::new(); // Clear when cycling past the newest
                                        } else {
                                            app.console_input = app.console_history[app.history_index].clone();
                                        }
                                        app.console_cursor = app.console_input.chars().count();
                                    }
                                }
                                KeyCode::Char(c) => {
                                    // Insert character exactly at the cursor position
                                    let mut chars: Vec<char> = app.console_input.chars().collect();
                                    chars.insert(app.console_cursor, c);
                                    app.console_input = chars.into_iter().collect();
                                    app.console_cursor += 1;
                                }
                                KeyCode::Backspace => {
                                    if app.console_cursor > 0 {
                                        let mut chars: Vec<char> = app.console_input.chars().collect();
                                        chars.remove(app.console_cursor - 1);
                                        app.console_input = chars.into_iter().collect();
                                        app.console_cursor -= 1;
                                    }
                                }
                                KeyCode::Delete => {
                                    let mut chars: Vec<char> = app.console_input.chars().collect();
                                    if app.console_cursor < chars.len() {
                                        chars.remove(app.console_cursor);
                                        app.console_input = chars.into_iter().collect();
                                    }
                                }
                                KeyCode::Enter => {
                                    // --- Add to History Buffer ---
                                    if !app.console_input.trim().is_empty() {
                                        if app.console_history.is_empty() || app.console_history.last() != Some(&app.console_input) {
                                            app.console_history.push(app.console_input.clone());
                                            if app.console_history.len() > 10 {
                                                app.console_history.remove(0); // Keep max 10 entries
                                            }
                                        }
                                        app.history_index = app.console_history.len();
                                    }

                                    // Lock the UI briefly to show loading state
                                    app.last_api_result = "Sending payload...".to_string();
                                    app.last_ttft = 0;
                                    
                                    // Clone data to move into the async worker
                                    let payload = app.console_input.clone();
                                    let tx_api = tx.clone();

                                    let host_api = app.host.clone();
                                    let port_api = app.port;

                                    tokio::spawn(async move {
                                        let client = Client::new();
                                        let start = Instant::now();
                                        let url = format!("http://{}:{}/v1/chat/completions", host_api, port_api);
                                        
                                        // --- Secretly inject "stream": true into the user's payload ---
                                        let mut payload_json: serde_json::Value = serde_json::from_str(&payload).unwrap_or_else(|_| serde_json::json!({}));
                                        if let Some(obj) = payload_json.as_object_mut() {
                                            obj.insert("stream".to_string(), serde_json::json!(true));
                                        }
                                        let stream_payload = payload_json.to_string();

                                        let response = client.post(&url)
                                            .header("Content-Type", "application/json")
                                            .body(stream_payload)
                                            .send()
                                            .await;
                                            
                                        match response {
                                            Ok(mut res) => {
                                                let status = res.status().to_string();
                                                let mut first_token = true;
                                                let mut total_tokens = 0.0;
                                                let mut buffer = String::new();

                                                // --- Read the SSE stream chunk by chunk in real-time ---
                                                while let Ok(Some(chunk)) = res.chunk().await {
                                                    if first_token {
                                                        let ttft = start.elapsed().as_millis();
                                                        let _ = tx_api.send(Event::ApiStreamStart { ttft_ms: ttft }).await;
                                                        first_token = false;
                                                    }

                                                    // Accumulate chunks and split by newlines safely
                                                    buffer.push_str(&String::from_utf8_lossy(&chunk));
                                                    while let Some(idx) = buffer.find('\n') {
                                                        let line = buffer[..idx].trim().to_string();
                                                        buffer = buffer[idx+1..].to_string();

                                                        if line.starts_with("data: ") {
                                                            let data = &line[6..];
                                                            if data == "[DONE]" { continue; }
                                                            
                                                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                                                                if let Some(content) = json.get("choices")
                                                                    .and_then(|c| c.get(0))
                                                                    .and_then(|c| c.get("delta"))
                                                                    .and_then(|d| d.get("content"))
                                                                    .and_then(|c| c.as_str()) 
                                                                {
                                                                    let clean_token = content.replace('\n', " ⏎ ");
                                                                    let _ = tx_api.send(Event::ApiStreamChunk(clean_token)).await;
                                                                    total_tokens += 1.0;
                                                                }
                                                            }
                                                        }
                                                    }
                                                }

                                                // Calculate generation speed
                                                let total_time_s = start.elapsed().as_millis() as f64 / 1000.0;
                                                let gen_tps = if total_time_s > 0.0 { total_tokens / total_time_s } else { 0.0 };
                                                let eval_tps = 0.0; // Streaming obscures prompt eval times, so we omit it to avoid faking data

                                                let _ = tx_api.send(Event::ApiStreamEnd { eval_tps, gen_tps, status }).await;
                                            }
                                            Err(e) => {
                                                let _ = tx_api.send(Event::ApiStreamChunk(format!("ERROR: {}", e))).await;
                                                let _ = tx_api.send(Event::ApiStreamEnd { eval_tps: 0.0, gen_tps: 0.0, status: "500".to_string() }).await;
                                            }
                                        }
                                    });
                                }
                                _ => {}
                            }
                        } else if app.show_help {
                            // --- HELP OVERLAY CONTROLS ---
                            match key.code {
                                KeyCode::Esc | KeyCode::Char('h') | KeyCode::Char('q') => app.show_help = false,
                                _ => {}
                            }
                        } else if app.is_searching {
                            // --- NEW: LOG SEARCH CONTROLS ---
                            match key.code {
                                KeyCode::Esc | KeyCode::Enter => app.is_searching = false,
                                KeyCode::Backspace => { app.search_query.pop(); }
                                KeyCode::Char(c) => { app.search_query.push(c); }
                                _ => {}
                            }
                        } else {
                            // --- MAIN DASHBOARD CONTROLS ---
                            match key.code {
                                KeyCode::Char('q') => app.should_quit = true,
                                KeyCode::Char('t') => app.show_tuner = true,
                                KeyCode::Char('i') => app.console_focused = true,
                                KeyCode::Char('m') => app.show_model_selector = true,
                                KeyCode::Char('g') => { app.show_gpu_inspector = !app.show_gpu_inspector; app.show_sys_inspector = false; },
                                KeyCode::Char('c') => { app.show_sys_inspector = !app.show_sys_inspector; app.show_gpu_inspector = false; },
                                KeyCode::Char('/') => { app.is_searching = true; app.search_query.clear(); },
                                KeyCode::Char('h') => app.show_help = true,
                                KeyCode::PageUp => app.scroll_logs_up(),
                                KeyCode::PageDown => app.scroll_logs_down(),
                                
                                // Dynamic Systemctl Bindings
                                KeyCode::Char('S') => { // Capital S
                                    let svc = app.service_name.clone();
                                    app.add_log(format!(">>> SYSTEMCTL: Starting {}...", svc));
                                    tokio::spawn(async move { let _ = tokio::process::Command::new("sudo").args(["-n", "systemctl", "start", &svc]).output().await; });
                                }
                                KeyCode::Char('X') => { // Capital X
                                    let svc = app.service_name.clone();
                                    app.add_log(format!(">>> SYSTEMCTL: Stopping {}...", svc));
                                    tokio::spawn(async move { let _ = tokio::process::Command::new("sudo").args(["-n", "systemctl", "stop", &svc]).output().await; });
                                }
                                KeyCode::Char('R') => { // Capital R
                                    let svc = app.service_name.clone();
                                    app.add_log(format!(">>> SYSTEMCTL: Restarting {}...", svc));
                                    tokio::spawn(async move { let _ = tokio::process::Command::new("sudo").args(["-n", "systemctl", "restart", &svc]).output().await; });
                                }
                                
                                KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                    app.add_log(">>> TACTICAL KILL-SWITCH ENGAGED...".to_string());
                                    tokio::spawn(async { let _ = tokio::process::Command::new("sudo").args(["-n", "killall", "-9", "llama-server"]).output().await; });
                                }

                                KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
                                    let filename = format!("crash_dump_{}.txt", timestamp);
                                    
                                    // Gather state data
                                    let active_model = app.active_model.clone();
                                    let vram_used = app.vram_used;
                                    let vram_total = app.vram_total;
                                    let ram_used = app.ram_used;
                                    let ram_total = app.ram_total;
                                    let gpu_temp = app.gpu_temp;
                                    let gpu_power = app.gpu_power.clone();
                                    let cpu_load = app.cpu_history.last().copied().unwrap_or(0);
                                    let logs: Vec<String> = app.logs.iter().cloned().collect();

                                    app.add_log(format!(">>> INITIATING CRASH DUMP -> {}", filename));

                                    // Spawn async task to write the file
                                    tokio::spawn(async move {
                                        use tokio::io::AsyncWriteExt;
                                        let mut content = String::new();
                                        content.push_str(&format!("--- SALTNITOR CRASH DUMP [{}] ---\n\n", timestamp));
                                        content.push_str(&format!("TARGET MODEL: {}\n", active_model));
                                        content.push_str(&format!("VRAM USAGE:   {:.2} / {:.2} GB\n", vram_used, vram_total));
                                        content.push_str(&format!("RAM USAGE:    {:.2} / {:.2} GB\n", ram_used, ram_total));
                                        content.push_str(&format!("GPU TEMP:     {} C\n", gpu_temp));
                                        content.push_str(&format!("GPU POWER:    {}\n", gpu_power));
                                        content.push_str(&format!("CPU LOAD:     {}%\n\n", cpu_load));
                                        
                                        content.push_str("--- RECENT LOGS (100 LINES) ---\n");
                                        for log in logs {
                                            content.push_str(&format!("{}\n", log));
                                        }

                                        if let Ok(mut file) = tokio::fs::File::create(&filename).await {
                                            let _ = file.write_all(content.as_bytes()).await;
                                        }
                                    });
                                }
                                _ => {}
                            }
                        }
                    }
                    // --- Live Streaming Event Handlers ---
                    Event::ApiStreamStart { ttft_ms } => {
                        app.last_ttft = ttft_ms;
                        app.last_api_result.clear(); // Clear the "Sending payload..." message
                    }
                    Event::ApiStreamChunk(token) => {
                        app.last_api_result.push_str(&token); // Paint it to the screen instantly
                    }
                    Event::ApiStreamEnd { eval_tps, gen_tps, status } => {
                        app.last_eval_tps = eval_tps;
                        app.last_gen_tps = gen_tps;
                        let final_msg = format!("[{}] {}", status, app.last_api_result.chars().take(30).collect::<String>());
                        app.add_log(format!("API Strike: {}ms | Gen: {:.1} t/s | {}", app.last_ttft, gen_tps, final_msg));
                    }
                    Event::ModelsFetched(models) => {
                        app.available_models = models;
                        
                        // --- Auto-select the first model if we don't have one ---
                        if app.active_model == "None" && !app.available_models.is_empty() {
                            let first_model = app.available_models[0].clone();
                            app.active_model = first_model.clone();
                            
                            // Dynamically rewrite the console input with the first discovered model
                            app.console_input = format!(r#"{{"model": "{}", "messages": [{{"role": "user", "content": "ping"}}]}}"#, first_model);
                            app.console_cursor = app.console_input.chars().count();
                        }
                        
                        // Ensure index doesn't go out of bounds if a model is removed
                        if app.model_selector_index >= app.available_models.len() && !app.available_models.is_empty() {
                            app.model_selector_index = app.available_models.len() - 1;
                        }
                    } 
                    Event::PortAudit(status) => {
                        app.port_status = status;
                    }
                    Event::HardwareUpdate { vram_used, ram_used, cpu_load, gpu_temp, gpu_power, gpu_processes, cpu_cores, swap_used, swap_total, sys_processes, gpu_util, vram_util, gpu_fan, gpu_clocks, sys_uptime } => {
                        app.vram_used = vram_used;
                        app.ram_used = ram_used;
                        app.gpu_temp = gpu_temp;
                        app.gpu_power = gpu_power;
                        app.gpu_processes = gpu_processes;
                        app.cpu_cores = cpu_cores;
                        app.swap_used = swap_used;
                        app.swap_total = swap_total;
                        app.sys_processes = sys_processes;
                        app.gpu_util = gpu_util;
                        app.vram_util = vram_util;
                        app.gpu_fan = gpu_fan;
                        app.gpu_clocks = gpu_clocks;
                        app.sys_uptime = sys_uptime;

                        if app.cpu_history.len() >= 100 {
                            app.cpu_history.remove(0);
                        }
                        app.cpu_history.push(cpu_load);
                    }
                    Event::LogLine(line) => {
                        app.add_log(line);
                    }
                    Event::Tick => {}
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    // --- NEW: Save History Buffer to Disk before exiting ---
    if !app.console_history.is_empty() {
        let history_content = app.console_history.join("\n");
        let _ = std::fs::write(".saltnitor_history", history_content);
    }

    // 6. Clean Teardown
    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}