mod app;
mod events;
mod ui;

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
    /// The port the AI daemon is running on
    #[arg(short, long, default_value_t = 8080)]
    port: u16,

    /// The host address of the AI daemon
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// The systemd service name for the AI daemon
    #[arg(short, long, default_value = "llama-router")]
    service_name: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 0. Parse Command Line Arguments
    let cli = Cli::parse();

    // 1. Pre-Flight Hardware Scan
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

    // 2. Terminal Initialization
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 3. Application State & Channels
    let mut app = App::new(
        cpu_name, cpu_core_count, ram_total, gpu_name, vram_total, has_nvidia,
        cli.host.clone(), cli.port, cli.service_name.clone()
    );
    let (tx, mut rx) = mpsc::channel::<Event>(100);

    // 4. Start Event Producers (Background Tasks)
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
    let service_c = cli.service_name.clone();
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
    let host_d = cli.host.clone();
    let port_d = cli.port;
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
    let port_e = cli.port;
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
                    format!("Port {}: ZOMBIE THREAD DETECTED!", port_e)
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
                                    }
                                    app.show_model_selector = false;
                                }
                                _ => {}
                            }
                        } else if app.show_tuner {
                            // --- TUNER MENU CONTROLS ---
                            match key.code {
                                KeyCode::Esc | KeyCode::Char('t') => app.show_tuner = false,
                                KeyCode::Up => app.tuner_selected = 0,
                                KeyCode::Down => app.tuner_selected = 1,
                                KeyCode::Left => {
                                    if app.tuner_selected == 0 && app.current_ngl > 0 { app.current_ngl -= 1; }
                                    if app.tuner_selected == 1 && app.current_ctx > 1024 { app.current_ctx -= 1024; }
                                }
                                KeyCode::Right => {
                                    if app.tuner_selected == 0 && app.current_ngl < 99 { app.current_ngl += 1; }
                                    if app.tuner_selected == 1 && app.current_ctx < 131072 { app.current_ctx += 1024; }
                                }
                                KeyCode::Enter => {
                                    let ini_content = format!("[model]\nngl = {}\nctx-size = {}\n", app.current_ngl, app.current_ctx);
                                    app.add_log(format!(">>> ROUTER CONFIG UPDATED: ngl={}, ctx={}", app.current_ngl, app.current_ctx));
                                    app.show_tuner = false;
                                    tokio::spawn(async move { let _ = tokio::fs::write("router.ini", ini_content).await; });
                                }
                                _ => {}
                            }
                        } else if app.console_focused {
                            // --- API INTERROGATOR CONTROLS ---
                            match key.code {
                                KeyCode::Esc => app.console_focused = false,
                                KeyCode::Char(c) => app.console_input.push(c),
                                KeyCode::Backspace => { app.console_input.pop(); }
                                KeyCode::Enter => {
                                    app.last_api_result = "Sending payload...".to_string();
                                    app.last_ttft = 0;
                                    
                                    let payload = app.console_input.clone();
                                    let tx_api = tx.clone();
                                    let host_api = app.host.clone();
                                    let port_api = app.port;
                                    
                                    tokio::spawn(async move {
                                        let client = Client::new();
                                        let start = Instant::now();
                                        let url = format!("http://{}:{}/v1/chat/completions", host_api, port_api);
                                        
                                        let response = client.post(&url)
                                            .header("Content-Type", "application/json")
                                            .body(payload)
                                            .send()
                                            .await;
                                            
                                        let ttft_ms = start.elapsed().as_millis();
                                        
                                        match response {
                                            Ok(res) => {
                                                let status = res.status().to_string();
                                                let text = res.text().await.unwrap_or_else(|_| "Failed to parse body".to_string());
                                                
                                                // --- NEW: Calculate Total Time and TPS ---
                                                let total_time_ms = start.elapsed().as_millis();
                                                let gen_time_s = (total_time_ms.saturating_sub(ttft_ms)) as f64 / 1000.0;
                                                
                                                let mut est_tokens = 0.0;
                                                // Try to grab exact token count from OpenAI JSON spec
                                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                                                    if let Some(usage) = json.get("usage") {
                                                        if let Some(tokens) = usage.get("completion_tokens").and_then(|t| t.as_f64()) {
                                                            est_tokens = tokens;
                                                        }
                                                    }
                                                }
                                                // Fallback heuristic if JSON parsing fails
                                                if est_tokens == 0.0 {
                                                    let word_count = text.split_whitespace().count() as f64;
                                                    est_tokens = word_count * 1.3;
                                                }
                                                
                                                let tps = if gen_time_s > 0.0 { est_tokens / gen_time_s } else { 0.0 };
                                                
                                                let snippet = text.chars().take(80).collect::<String>();
                                                
                                                let _ = tx_api.send(Event::ApiResponse { ttft_ms, tps, status, message: snippet }).await;
                                            }
                                            Err(e) => {
                                                let _ = tx_api.send(Event::ApiResponse { ttft_ms: 0, tps: 0.0, status: "ERROR".to_string(), message: e.to_string() }).await;
                                            }
                                        }
                                    });
                                }
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
                                _ => {}
                            }
                        }
                    }
                    Event::ApiResponse { ttft_ms, tps, status, message } => {
                        app.last_ttft = ttft_ms;
                        app.last_tps = tps;
                        app.last_api_result = format!("[{}] {}", status, message);
                        app.add_log(format!("API Strike Completed: {}ms", ttft_ms));
                    }
                    Event::ModelsFetched(models) => {
                        app.available_models = models;
                        
                        // --- NEW: Auto-select the first model if we don't have one ---
                        if app.active_model == "None" && !app.available_models.is_empty() {
                            let first_model = app.available_models[0].clone();
                            app.active_model = first_model.clone();
                            
                            // Dynamically rewrite the console input with the first discovered model
                            app.console_input = format!(r#"{{"model": "{}", "messages": [{{"role": "user", "content": "ping"}}]}}"#, first_model);
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

    // 6. Clean Teardown
    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}