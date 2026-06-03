mod app;
mod events;
mod ui;
mod control_api;

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
use std::collections::HashMap;
use std::sync::Arc;
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
    // --- Control API (native-router edition; Saltcode headless hot-swap bridge) ---
    control_port: Option<u16>,
    control_token: Option<String>,
    router_base: Option<String>,
    infer_bearer: Option<String>,
    reserve_vram_gb: Option<f64>,
    reserve_ram_gb: Option<f64>,
    #[serde(default)]
    profiles: HashMap<String, control_api::ProfileMeta>,
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

/// Upsert `kv` into the `[section]` block of an INI string, preserving every other
/// line in that section (the `model = ` path, comments, `load-on-startup`,
/// `override-tensor`, ...) and all other sections untouched. Returns the new file
/// content, or None if the section header was not found.
fn upsert_ini_section(content: &str, section: &str, kv: &[(String, String)]) -> Option<String> {
    let header = format!("[{}]", section);
    let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();

    let start = lines.iter().position(|l| l.trim() == header)?;
    let mut end = lines.len();
    for i in (start + 1)..lines.len() {
        let t = lines[i].trim();
        if t.starts_with('[') && t.ends_with(']') { end = i; break; }
    }

    let mut out: Vec<String> = lines[..=start].to_vec();
    let mut remaining: Vec<(String, String)> = kv.to_vec();
    for line in &lines[(start + 1)..end] {
        let trimmed = line.trim_start();
        let is_comment = trimmed.starts_with(';') || trimmed.starts_with('#');
        let key = trimmed.split('=').next().map(str::trim).unwrap_or("");
        if !is_comment && !key.is_empty() {
            if let Some(pos) = remaining.iter().position(|(k, _)| k == key) {
                let (k, v) = remaining.remove(pos);
                out.push(format!("{} = {}", k, v));
                continue;
            }
        }
        out.push(line.clone());
    }
    for (k, v) in remaining {
        out.push(format!("{} = {}", k, v));
    }
    out.extend_from_slice(&lines[end..]);
    Some(out.join("\n") + "\n")
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

    // --- Headless Control API (Saltcode native-router bridge) ---
    // Sits in front of llama.cpp's native router (--models-preset). Provides the
    // VRAM oracle + /v1/ensure that the router itself lacks. Binds 127.0.0.1 only.
    {
        let controller = Arc::new(control_api::ControlApi::new(
            toml_conf.profiles.clone(),
            toml_conf
                .router_base
                .clone()
                .unwrap_or_else(|| format!("http://{}:{}", final_host, final_port)),
            toml_conf.infer_bearer.clone(),
            toml_conf.control_token.clone(),
            toml_conf.reserve_vram_gb.unwrap_or(0.8),
            toml_conf.reserve_ram_gb.unwrap_or(1.0),
            tx.clone(),
        ));
        let control_port = toml_conf.control_port.unwrap_or(8765);
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], control_port));
        tokio::spawn(async move { control_api::serve(controller, addr).await; });
    }

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

            // Top System RAM Culprits (Showing ALL processes > 1MB, Deduplicated)
            let mut procs: Vec<_> = sys.processes().values().collect();
            procs.sort_by(|a, b| b.memory().cmp(&a.memory())); // Sort by memory descending first
            
            let mut seen_names = std::collections::HashSet::new();
            let sys_processes: Vec<(String, f64)> = procs.iter()
                .filter(|p| p.memory() > 1_048_576) // Filter out tiny < 1MB threads
                .filter_map(|p| {
                    let name = p.name().to_string_lossy().to_string();
                    // HashSet.insert() returns true only if the name has never been seen before
                    if seen_names.insert(name.clone()) {
                        Some((name, p.memory() as f64 / 1_073_741_824.0))
                    } else {
                        None // Silently drop the ghost thread
                    }
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
                        if app.show_gpu_inspector {
                            // --- PROCESS SNIPER (GPU) ---
                            match key.code {
                                KeyCode::Esc | KeyCode::Char('g') | KeyCode::Char('q') => app.show_gpu_inspector = false,
                                KeyCode::Up => {
                                    let i = match app.gpu_proc_state.selected() { Some(i) => if i == 0 { app.gpu_processes.len().saturating_sub(1) } else { i - 1 }, None => 0 };
                                    app.gpu_proc_state.select(Some(i));
                                }
                                KeyCode::Down => {
                                    let i = match app.gpu_proc_state.selected() { Some(i) => if i >= app.gpu_processes.len().saturating_sub(1) { 0 } else { i + 1 }, None => 0 };
                                    app.gpu_proc_state.select(Some(i));
                                }
                                KeyCode::Char('x') | KeyCode::Delete => {
                                    if let Some(i) = app.gpu_proc_state.selected() {
                                        if let Some((name, _)) = app.gpu_processes.get(i) {
                                            let proc_name = name.clone();
                                            if proc_name == "saltnitor" || proc_name.contains("llama-server") { app.add_log(">>> PROCESS SNIPER: Access Denied.".to_string()); } 
                                            else {
                                                app.add_log(format!(">>> PROCESS SNIPER: Executing SIGKILL (-9) on {}", proc_name));
                                                tokio::spawn(async move { let _ = tokio::process::Command::new("killall").arg("-9").arg(proc_name).output().await; });
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        } else if app.show_sys_inspector {
                            // --- PROCESS SNIPER (CPU/RAM) ---
                            match key.code {
                                KeyCode::Esc | KeyCode::Char('c') | KeyCode::Char('q') => app.show_sys_inspector = false,
                                KeyCode::Up => {
                                    let i = match app.sys_proc_state.selected() { Some(i) => if i == 0 { app.sys_processes.len().saturating_sub(1) } else { i - 1 }, None => 0 };
                                    app.sys_proc_state.select(Some(i));
                                }
                                KeyCode::Down => {
                                    let i = match app.sys_proc_state.selected() { Some(i) => if i >= app.sys_processes.len().saturating_sub(1) { 0 } else { i + 1 }, None => 0 };
                                    app.sys_proc_state.select(Some(i));
                                }
                                KeyCode::Char('x') | KeyCode::Delete => {
                                    if let Some(i) = app.sys_proc_state.selected() {
                                        if let Some((name, _)) = app.sys_processes.get(i) {
                                            let proc_name = name.clone();
                                            if proc_name == "saltnitor" || proc_name.contains("llama-server") { app.add_log(">>> PROCESS SNIPER: Access Denied.".to_string()); } 
                                            else {
                                                app.add_log(format!(">>> PROCESS SNIPER: Executing SIGKILL (-9) on {}", proc_name));
                                                tokio::spawn(async move { let _ = tokio::process::Command::new("killall").arg("-9").arg(proc_name).output().await; });
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        } else if app.show_tuner {
                            // --- DEEP TUNER MENU CONTROLS (Keep exact same as before) ---
                            match key.code {
                                KeyCode::Esc | KeyCode::Char('t') => app.show_tuner = false,
                                KeyCode::Tab => { app.tuner_page = (app.tuner_page + 1) % 3; app.tuner_selected = 0; }
                                KeyCode::Up => { if app.tuner_selected > 0 { app.tuner_selected -= 1; } }
                                KeyCode::Down => { let max_idx = match app.tuner_page { 0 => 9, 1 => 5, 2 => 5, _ => 0 }; if app.tuner_selected < max_idx { app.tuner_selected += 1; } }
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
                                            8 => if is_right && app.cache_k_idx < 8 { app.cache_k_idx += 1; } else if !is_right && app.cache_k_idx > 0 { app.cache_k_idx -= 1; },
                                            9 => if is_right && app.cache_v_idx < 8 { app.cache_v_idx += 1; } else if !is_right && app.cache_v_idx > 0 { app.cache_v_idx -= 1; },
                                            _ => {}
                                        },
                                        1 => match app.tuner_selected {
                                            0 => if is_right { app.rope_base += 10000; } else if !is_right && app.rope_base > 10000 { app.rope_base -= 10000; },
                                            1 => if is_right { app.rope_scale += 0.5; } else if !is_right && app.rope_scale > 1.0 { app.rope_scale -= 0.5; },
                                            2 => if is_right && app.defrag_thold < 1.0 { app.defrag_thold += 0.1; } else if !is_right && app.defrag_thold > -1.0 { app.defrag_thold -= 0.1; },
                                            3 => if is_right { app.draft_max += 1; } else if !is_right && app.draft_max > 1 { app.draft_max -= 1; },
                                            4 => if is_right { app.draft_min += 1; } else if !is_right && app.draft_min > 1 { app.draft_min -= 1; },
                                            5 => if is_right && app.draft_model_idx < app.available_models.len() { app.draft_model_idx += 1; } else if !is_right && app.draft_model_idx > 0 { app.draft_model_idx -= 1; },
                                            _ => {}
                                        },
                                        2 => match app.tuner_selected {
                                            0 => if is_right && app.threads_batch < app.cpu_core_count { app.threads_batch += 1; } else if !is_right && app.threads_batch > 1 { app.threads_batch -= 1; },
                                            1 => if is_right && app.ubatch_size < app.current_batch { app.ubatch_size *= 2; } else if !is_right && app.ubatch_size > 32 { app.ubatch_size /= 2; },
                                            2 => { app.cont_batching = !app.cont_batching; },
                                            3 => { app.ctx_shift = !app.ctx_shift; },
                                            4 => { app.metrics = !app.metrics; },
                                            5 => { app.api_key = !app.api_key; },
                                            _ => {}
                                        }, 
                                        _ => {}
                                    }
                                }
                                KeyCode::Enter => {
                                    // Deep Tuner -> router.ini SECTION EDITOR. Writes the tuned flags into
                                    // the active model's [section] in router.ini (preserving its model path
                                    // and the other sections), then restarts the router to apply them.
                                    let section = app.active_model.clone();
                                    if section.is_empty() || section == "None" {
                                        app.add_log(">>> TUNER: no active model - select one in the Hot-Swap deck (Tab) before applying.".to_string());
                                    } else {
                                        let cache_types = ["f16", "f32", "bf16", "q8_0", "q4_0", "q4_1", "iq4_nl", "q5_0", "q5_1"];
                                        let mut kv: Vec<(String, String)> = vec![
                                            ("ngl".to_string(),             app.current_ngl.to_string()),
                                            ("ctx-size".to_string(),        app.current_ctx.to_string()),
                                            ("batch-size".to_string(),      app.current_batch.to_string()),
                                            ("ubatch-size".to_string(),     app.ubatch_size.to_string()),
                                            ("threads".to_string(),         app.current_threads.to_string()),
                                            ("threads-batch".to_string(),   app.threads_batch.to_string()),
                                            ("parallel".to_string(),        app.current_parallel.to_string()),
                                            ("flash-attn".to_string(),      if app.flash_attn { "on".to_string() } else { "off".to_string() }),
                                            ("cache-type-k".to_string(),    cache_types[app.cache_k_idx].to_string()),
                                            ("cache-type-v".to_string(),    cache_types[app.cache_v_idx].to_string()),
                                            ("cont-batching".to_string(),   app.cont_batching.to_string()),
                                            ("rope-freq-base".to_string(),  app.rope_base.to_string()),
                                            ("rope-freq-scale".to_string(), format!("{}", app.rope_scale)),
                                            ("defrag-thold".to_string(),    format!("{}", app.defrag_thold)),
                                        ];
                                        if app.mlock   { kv.push(("mlock".to_string(),   "true".to_string())); }
                                        if app.no_mmap { kv.push(("no-mmap".to_string(), "true".to_string())); }

                                        app.add_log(format!(">>> TUNER: writing {} keys to [{}] in router.ini...", kv.len(), section));
                                        app.show_tuner = false;
                                        let svc_name = app.service_name.clone();
                                        let tx_t = tx.clone();
                                        tokio::spawn(async move {
                                            const ROUTER_INI: &str = "/home/laz/ai-models/llama.cpp/router.ini";
                                            match tokio::fs::read_to_string(ROUTER_INI).await {
                                                Ok(content) => match upsert_ini_section(&content, &section, &kv) {
                                                    Some(updated) => {
                                                        if tokio::fs::write(ROUTER_INI, updated).await.is_ok() {
                                                            let _ = tx_t.send(Event::LogLine(format!(">>> TUNER: [{}] updated. Restarting router...", section))).await;
                                                            let out = tokio::process::Command::new("sudo").args(["-n", "systemctl", "restart", &svc_name]).output().await;
                                                            match out {
                                                                Ok(o) if o.status.success() => { let _ = tx_t.send(Event::LogLine(">>> TUNER: router restarted. If it does not come back, a key may be unsupported - check: journalctl -u llama-router -e".to_string())).await; }
                                                                _ => { let _ = tx_t.send(Event::LogLine(">>> TUNER ERROR: restart failed (sudoers for 'systemctl restart'? check journalctl).".to_string())).await; }
                                                            }
                                                        } else {
                                                            let _ = tx_t.send(Event::LogLine(format!(">>> TUNER ERROR: cannot write {}", ROUTER_INI))).await;
                                                        }
                                                    }
                                                    None => { let _ = tx_t.send(Event::LogLine(format!(">>> TUNER ERROR: section [{}] not found in router.ini", section))).await; }
                                                },
                                                Err(_) => { let _ = tx_t.send(Event::LogLine(format!(">>> TUNER ERROR: cannot read {}", ROUTER_INI))).await; }
                                            }
                                        });
                                    }
                                }

                                _ => {}
                            }
                        } else if app.console_focused {
                            // --- NEW: DUAL-MODE BOTTOM DECK CONTROLS ---
                            if app.bottom_tab_mode == 1 { // HOT-SWAP MODE
                                match key.code {
                                    KeyCode::Esc => app.console_focused = false,
                                    KeyCode::Up => {
                                        let i = match app.hot_swap_state.selected() {
                                            Some(i) => if i == 0 { app.available_models.len().saturating_sub(1) } else { i - 1 },
                                            None => 0,
                                        };
                                        app.hot_swap_state.select(Some(i));
                                    }
                                    KeyCode::Down => {
                                        let i = match app.hot_swap_state.selected() {
                                            Some(i) => if i >= app.available_models.len().saturating_sub(1) { 0 } else { i + 1 },
                                            None => 0,
                                        };
                                        app.hot_swap_state.select(Some(i));
                                    }
                                    KeyCode::Enter => {
                                        if let Some(i) = app.hot_swap_state.selected() {
                                            if let Some(chosen_model) = app.available_models.get(i).cloned() {
                                                app.console_focused = false;
                                                
                                                // 1. Calculate NGL
                                                let mut auto_ngl = 99;
                                                let model_upper = chosen_model.to_uppercase();
                                                for word in model_upper.replace("-", " ").replace("_", " ").split_whitespace() {
                                                    if word.ends_with("B") {
                                                        if let Ok(p) = word.trim_end_matches('B').parse::<f64>() {
                                                            if p > 14.0 { auto_ngl = 24; } 
                                                        }
                                                    }
                                                }
                                                
                                                // 2. Lock State
                                                app.current_ngl = auto_ngl;
                                                app.active_model = chosen_model.clone();
                                                app.console_input = format!(r#"{{"model": "{}", "messages": [{{"role": "user", "content": "ping"}}]}}"#, chosen_model);
                                                app.console_cursor = app.console_input.chars().count();
                                                
                                                // Native-router hot-swap: warm-load the chosen model BY NAME.
                                                // The router (--models-preset --models-max 1) autoloads it and
                                                // evicts the incumbent. No router.env, no systemctl, no sudo.
                                                app.add_log(format!(">>> HOT-SWAP: Requesting [{}] from router...", chosen_model));
                                                let host_api = app.host.clone();
                                                let port_api = app.port;
                                                let warmup_model = chosen_model.clone();
                                                let use_api_key = app.api_key;
                                                let tx_warmup = tx.clone();
                                                tokio::spawn(async move {
                                                    let client = reqwest::Client::new();
                                                    let url = format!("http://{}:{}/v1/chat/completions", host_api, port_api);
                                                    let payload = format!(r#"{{"model": "{}", "messages": [{{"role": "user", "content": "warmup"}}], "max_tokens": 1}}"#, warmup_model);
                                                    let mut req = client.post(&url)
                                                        .header("Content-Type", "application/json")
                                                        .timeout(std::time::Duration::from_secs(120));
                                                    if use_api_key { req = req.header("Authorization", "Bearer sk-saltnitor-2026"); }
                                                    match req.body(payload).send().await {
                                                        Ok(res) if res.status().is_success() => {
                                                            let _ = tx_warmup.send(Event::ActiveModelSet(warmup_model.clone())).await;
                                                            let _ = tx_warmup.send(Event::LogLine(format!(">>> HOT-SWAP: [{}] resident & warm.", warmup_model))).await;
                                                        }
                                                        Ok(res) => { let _ = tx_warmup.send(Event::LogLine(format!(">>> HOT-SWAP ERROR: router returned {}", res.status()))).await; }
                                                        Err(_) => { let _ = tx_warmup.send(Event::LogLine(">>> HOT-SWAP ERROR: router did not respond.".to_string())).await; }
                                                    }
                                                });
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            } else { // INTERROGATOR MODE
                                match key.code {
                                    KeyCode::Esc => app.console_focused = false, // Exit insert mode
                                    KeyCode::Left => { if app.console_cursor > 0 { app.console_cursor -= 1; } }
                                    KeyCode::Right => { if app.console_cursor < app.console_input.chars().count() { app.console_cursor += 1; } }
                                    KeyCode::Up => {
                                        if !app.console_history.is_empty() && app.history_index > 0 {
                                            app.history_index -= 1;
                                            app.console_input = app.console_history[app.history_index].clone();
                                            app.console_cursor = app.console_input.chars().count();
                                        }
                                    }
                                    KeyCode::Down => {
                                        if app.history_index < app.console_history.len() {
                                            app.history_index += 1;
                                            if app.history_index == app.console_history.len() {
                                                app.console_input = String::new();
                                            } else {
                                                app.console_input = app.console_history[app.history_index].clone();
                                            }
                                            app.console_cursor = app.console_input.chars().count();
                                        }
                                    }
                                    KeyCode::Char(c) => {
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
                                        if !app.console_input.trim().is_empty() {
                                            if app.console_history.is_empty() || app.console_history.last() != Some(&app.console_input) {
                                                app.console_history.push(app.console_input.clone());
                                                if app.console_history.len() > 10 { app.console_history.remove(0); }
                                            }
                                            app.history_index = app.console_history.len();
                                        }

                                        app.last_api_result = "Sending payload...".to_string();
                                        app.last_ttft = 0;
                                        let payload = app.console_input.clone();
                                        let tx_api = tx.clone();
                                        let host_api = app.host.clone();
                                        let port_api = app.port;

                                        let use_api_key = app.api_key;
                                        tokio::spawn(async move {
                                            let client = Client::new();
                                            let start = Instant::now();
                                            let url = format!("http://{}:{}/v1/chat/completions", host_api, port_api);
                                            
                                            let mut payload_json: serde_json::Value = serde_json::from_str(&payload).unwrap_or_else(|_| serde_json::json!({}));
                                            if let Some(obj) = payload_json.as_object_mut() {
                                                obj.insert("stream".to_string(), serde_json::json!(true));
                                            }
                                            let stream_payload = payload_json.to_string();

                                            let mut req = client.post(&url).header("Content-Type", "application/json");
                                            if use_api_key { req = req.header("Authorization", "Bearer sk-saltnitor-2026"); }
                                            let response = req.body(stream_payload).send().await;
                                                
                                            match response {
                                                Ok(mut res) => {
                                                    let status = res.status().to_string();
                                                    let mut first_token = true;
                                                    let mut total_tokens = 0.0;
                                                    let mut buffer = String::new();

                                                    while let Ok(Some(chunk)) = res.chunk().await {
                                                        if first_token {
                                                            let ttft = start.elapsed().as_millis();
                                                            let _ = tx_api.send(Event::ApiStreamStart { ttft_ms: ttft }).await;
                                                            first_token = false;
                                                        }
                                                        buffer.push_str(&String::from_utf8_lossy(&chunk));
                                                        while let Some(idx) = buffer.find('\n') {
                                                            let line = buffer[..idx].trim().to_string();
                                                            buffer = buffer[idx+1..].to_string();

                                                            if line.starts_with("data: ") {
                                                                let data = &line[6..];
                                                                if data == "[DONE]" { continue; }
                                                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                                                                    if let Some(content) = json.get("choices").and_then(|c| c.get(0)).and_then(|c| c.get("delta")).and_then(|d| d.get("content")).and_then(|c| c.as_str()) {
                                                                        let clean_token = content.replace('\n', " ⏎ ");
                                                                        let _ = tx_api.send(Event::ApiStreamChunk(clean_token)).await;
                                                                        total_tokens += 1.0;
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                    let total_time_s = start.elapsed().as_millis() as f64 / 1000.0;
                                                    let gen_tps = if total_time_s > 0.0 { total_tokens / total_time_s } else { 0.0 };
                                                    let _ = tx_api.send(Event::ApiStreamEnd { eval_tps: 0.0, gen_tps, status }).await;
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
                            }
                        } else if app.show_help {
                            match key.code {
                                KeyCode::Esc | KeyCode::Char('h') | KeyCode::Char('q') => app.show_help = false,
                                _ => {}
                            }
                        } else if app.is_searching {
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
                                KeyCode::Tab => app.bottom_tab_mode = (app.bottom_tab_mode + 1) % 2,
                                KeyCode::Char('g') => { app.show_gpu_inspector = !app.show_gpu_inspector; app.show_sys_inspector = false; },
                                KeyCode::Char('c') => { app.show_sys_inspector = !app.show_sys_inspector; app.show_gpu_inspector = false; },
                                KeyCode::Char('/') => { app.is_searching = true; app.search_query.clear(); },
                                KeyCode::Char('h') => app.show_help = true,
                                KeyCode::PageUp => app.scroll_logs_up(),
                                KeyCode::PageDown => app.scroll_logs_down(),
                                KeyCode::Esc => { app.show_gpu_inspector = false; app.show_sys_inspector = false; },
                                
                                KeyCode::Char('S') => { 
                                    let svc = app.service_name.clone();
                                    app.add_log(format!(">>> SYSTEMCTL: Starting {}...", svc));
                                    tokio::spawn(async move { let _ = tokio::process::Command::new("sudo").args(["-n", "systemctl", "start", &svc]).output().await; });
                                }
                                KeyCode::Char('X') => { 
                                    let svc = app.service_name.clone();
                                    app.add_log(format!(">>> SYSTEMCTL: Stopping {}...", svc));
                                    tokio::spawn(async move { let _ = tokio::process::Command::new("sudo").args(["-n", "systemctl", "stop", &svc]).output().await; });
                                }
                                KeyCode::Char('R') => {
                                    let svc = app.service_name.clone();
                                    app.add_log(format!(">>> SYSTEMCTL: Restarting {}...", svc));
                                    tokio::spawn(async move { let _ = tokio::process::Command::new("sudo").args(["-n", "systemctl", "restart", &svc]).output().await; });
                                }
                                KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                    // Kill-switch must STOP the unit. With Restart=always a bare killall is
                                    // respawned in ~2s and VRAM never frees; stopping the unit wins and stays down.
                                    let svc = app.service_name.clone();
                                    app.add_log(">>> TACTICAL KILL-SWITCH: stopping unit (frees VRAM, stays down)...".to_string());
                                    let tx_k = tx.clone();
                                    tokio::spawn(async move {
                                        let out = tokio::process::Command::new("sudo").args(["-n", "systemctl", "stop", &svc]).output().await;
                                        match out {
                                            Ok(o) if o.status.success() => { let _ = tx_k.send(Event::LogLine(">>> KILL-SWITCH: unit stopped, VRAM freed. (Shift+S to restart.)".to_string())).await; }
                                            _ => { let _ = tx_k.send(Event::LogLine(">>> KILL-SWITCH ERROR: stop failed - sudoers must allow 'systemctl stop'; see journalctl.".to_string())).await; }
                                        }
                                    });
                                }
                                KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                    // Crash dump -> an ABSOLUTE path under $HOME, and report where it went (or
                                    // if it failed) instead of writing to an unknown CWD and swallowing errors.
                                    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
                                    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                                    let path = format!("{}/saltnitor_crash_{}.txt", home, timestamp);
                                    let active_model = app.active_model.clone();
                                    let vram_used = app.vram_used;
                                    let vram_total = app.vram_total;
                                    let ram_used = app.ram_used;
                                    let ram_total = app.ram_total;
                                    let gpu_temp = app.gpu_temp;
                                    let gpu_power = app.gpu_power.clone();
                                    let cpu_load = app.cpu_history.last().copied().unwrap_or(0);
                                    let logs: Vec<String> = app.logs.iter().cloned().collect();
                                    app.add_log(format!(">>> CRASH DUMP -> {}", path));
                                    let tx_d = tx.clone();
                                    tokio::spawn(async move {
                                        use tokio::io::AsyncWriteExt;
                                        let mut content = String::new();
                                        content.push_str(&format!("--- SALTNITOR CRASH DUMP [{}] ---\n\nTARGET MODEL: {}\nVRAM USAGE:   {:.2} / {:.2} GB\nRAM USAGE:    {:.2} / {:.2} GB\nGPU TEMP:     {} C\nGPU POWER:    {}\nCPU LOAD:     {}%\n\n--- RECENT LOGS ---\n", timestamp, active_model, vram_used, vram_total, ram_used, ram_total, gpu_temp, gpu_power, cpu_load));
                                        for log in logs { content.push_str(&format!("{}\n", log)); }
                                        match tokio::fs::File::create(&path).await {
                                            Ok(mut file) => {
                                                if file.write_all(content.as_bytes()).await.is_ok() {
                                                    let _ = tx_d.send(Event::LogLine(format!(">>> CRASH DUMP: saved to {}", path))).await;
                                                } else {
                                                    let _ = tx_d.send(Event::LogLine(format!(">>> CRASH DUMP ERROR: write failed -> {}", path))).await;
                                                }
                                            }
                                            Err(e) => { let _ = tx_d.send(Event::LogLine(format!(">>> CRASH DUMP ERROR: cannot create {} ({})", path, e))).await; }
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
                        
                        // --- FIXED: Prevent out-of-bounds using the new Hot-Swap ListState ---
                        if let Some(selected) = app.hot_swap_state.selected() {
                            if selected >= app.available_models.len() && !app.available_models.is_empty() {
                                // If the list shrank, snap the cursor to the bottom
                                app.hot_swap_state.select(Some(app.available_models.len() - 1));
                            } else if app.available_models.is_empty() {
                                // If all models were deleted, clear the cursor
                                app.hot_swap_state.select(None);
                            }
                        } else if !app.available_models.is_empty() {
                            // If we just booted and have models, initialize the cursor at index 0
                            app.hot_swap_state.select(Some(0));
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

                        // Clamp GPU State
                        if let Some(selected) = app.gpu_proc_state.selected() {
                            if selected >= app.gpu_processes.len() && !app.gpu_processes.is_empty() { app.gpu_proc_state.select(Some(app.gpu_processes.len() - 1)); } 
                            else if app.gpu_processes.is_empty() { app.gpu_proc_state.select(None); }
                        } else if !app.gpu_processes.is_empty() { app.gpu_proc_state.select(Some(0)); }

                        // Clamp Sys State
                        if let Some(selected) = app.sys_proc_state.selected() {
                            if selected >= app.sys_processes.len() && !app.sys_processes.is_empty() { app.sys_proc_state.select(Some(app.sys_processes.len() - 1)); } 
                            else if app.sys_processes.is_empty() { app.sys_proc_state.select(None); }
                        } else if !app.sys_processes.is_empty() { app.sys_proc_state.select(Some(0)); }

                        if app.cpu_history.len() >= 100 { app.cpu_history.remove(0); }
                        app.cpu_history.push(cpu_load);
                    }
                    Event::LogLine(line) => {
                        app.add_log(line);
                    }
                    Event::ActiveModelSet(m) => {
                        app.active_model = m.clone();
                        app.add_log(format!(">>> EXTERNAL: resident model is now {}", m));
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