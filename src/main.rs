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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 0. Pre-Flight Hardware Scan
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

    // 1. Terminal Initialization
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 2. Application State & Channels
    let mut app = App::new(cpu_name, cpu_core_count, ram_total, gpu_name, vram_total, has_nvidia);
    let (tx, mut rx) = mpsc::channel::<Event>(100);

    // 3. Start Event Producers (Background Tasks)
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

            // NVIDIA Metrics (Processes)
            let mut gpu_processes: Vec<(String, f64)> = Vec::new();
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

            if !has_nvidia {
                gpu_temp = 0;
                gpu_power = "N/A".to_string();
                gpu_util = "0".to_string();
                vram_util = "0".to_string();
                gpu_fan = "N/A".to_string();
                gpu_clocks = "N/A".to_string();
            }

            let _ = tx_hw.send(Event::HardwareUpdate {
                vram_used, ram_used, cpu_load: cpu_load as u64,
                gpu_temp, gpu_power, gpu_processes,
                cpu_cores, swap_used, swap_total, sys_processes,
                // Add this new line right here:
                gpu_util, vram_util, gpu_fan, gpu_clocks, sys_uptime,
            }).await;

            let _ = tx_hw.send(Event::Tick).await;
        }
    });

    // Task C: Journalctl Log Streamer
    tokio::spawn(async move {
        // Stream the llama-router logs asynchronously
        let mut child = Command::new("journalctl")
            .args(["-u", "llama-router", "-f", "-n", "30"])
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
    tokio::spawn(async move {
        let client = reqwest::Client::new();
        loop {
            // Ping the router's model manifest endpoint
            if let Ok(res) = client.get("http://127.0.0.1:8080/v1/models").send().await {
                if let Ok(json) = res.json::<serde_json::Value>().await {
                    // Parse the JSON array to extract the "id" of each model
                    if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                        let models: Vec<String> = data.iter()
                            .filter_map(|m| m.get("id").and_then(|id| id.as_str()).map(|s| s.to_string()))
                            .collect();
                        
                        let _ = tx_models.send(Event::ModelsFetched(models)).await;
                    }
                }
            }
            // Sleep for 5 seconds before checking again
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });

    // Task E: Background Session & Port Auditor
    let tx_port = tx.clone();
    tokio::spawn(async move {
        loop {
            // Use 'ss' to look for processes holding port 8080
            let output = tokio::process::Command::new("sh")
                .arg("-c")
                // -lptn: listening, processes, tcp, numeric
                .arg("ss -lptn 'sport = :8080'")
                .output()
                .await;

            let status_msg = if let Ok(out) = output {
                let stdout = String::from_utf8_lossy(&out.stdout);
                
                if stdout.trim().is_empty() || !stdout.contains("8080") {
                    "Port 8080: OFFLINE (Daemon Down)".to_string()
                } else if stdout.contains("llama-server") || stdout.contains("llama-se") {
                    "Port 8080: SECURE (llama-server bound)".to_string()
                } else {
                    // If it's not empty, and not llama-server, a rogue Python script or zombie has it!
                    "Port 8080: ZOMBIE THREAD DETECTED!".to_string()
                }
            } else {
                "Port 8080: AUDIT ERROR".to_string()
            };

            let _ = tx_port.send(Event::PortAudit(status_msg)).await;
            
            // Sweep the port every 3 seconds
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    });

    // 4. The Render & Control Loop
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
                                        // Auto-inject the selected model into the JSON payload
                                        app.console_input = format!(r#"{{"model": "{}", "messages": [{{"role": "user", "content": "ping"}}]}}"#, selected_model);
                                        app.add_log(format!(">>> API Target locked to: {}", selected_model));
                                    }
                                    app.show_model_selector = false;
                                }
                                _ => {}
                            }
                        } else if app.show_tuner {
                            // --- TUNER MENU CONTROLS --- (Leave your existing tuner logic here)
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
                                KeyCode::Esc => app.console_focused = false, // Exit insert mode
                                KeyCode::Char(c) => app.console_input.push(c), // Type into buffer
                                KeyCode::Backspace => { app.console_input.pop(); } // Delete chars
                                KeyCode::Enter => {
                                    // Lock the UI briefly to show loading state
                                    app.last_api_result = "Sending payload...".to_string();
                                    app.last_ttft = 0;
                                    
                                    // Clone data to move into the async worker
                                    let payload = app.console_input.clone();
                                    let tx_api = tx.clone();
                                    
                                    tokio::spawn(async move {
                                        let client = Client::new();
                                        let start = Instant::now();
                                        
                                        // Fire the raw POST request to the local router
                                        let response = client.post("http://127.0.0.1:8080/v1/chat/completions")
                                            .header("Content-Type", "application/json")
                                            .body(payload)
                                            .send()
                                            .await;
                                            
                                        // Calculate exact TTFT (Round trip to first byte of response)
                                        let ttft_ms = start.elapsed().as_millis();
                                        
                                        match response {
                                            Ok(res) => {
                                                let status = res.status().to_string();
                                                // Grab a snippet of the response to display
                                                let text = res.text().await.unwrap_or_else(|_| "Failed to parse body".to_string());
                                                let snippet = text.chars().take(80).collect::<String>();
                                                
                                                let _ = tx_api.send(Event::ApiResponse { ttft_ms, status, message: snippet }).await;
                                            }
                                            Err(e) => {
                                                let _ = tx_api.send(Event::ApiResponse { ttft_ms: 0, status: "ERROR".to_string(), message: e.to_string() }).await;
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
                                
                                // Graceful Systemctl Bindings
                                KeyCode::Char('S') => { // Capital S
                                    app.add_log(">>> SYSTEMCTL: Starting llama-router...".to_string());
                                    tokio::spawn(async { let _ = tokio::process::Command::new("sudo").args(["-n", "systemctl", "start", "llama-router"]).output().await; });
                                }
                                KeyCode::Char('X') => { // Capital X
                                    app.add_log(">>> SYSTEMCTL: Stopping llama-router...".to_string());
                                    tokio::spawn(async { let _ = tokio::process::Command::new("sudo").args(["-n", "systemctl", "stop", "llama-router"]).output().await; });
                                }
                                KeyCode::Char('R') => { // Capital R
                                    app.add_log(">>> SYSTEMCTL: Restarting llama-router...".to_string());
                                    tokio::spawn(async { let _ = tokio::process::Command::new("sudo").args(["-n", "systemctl", "restart", "llama-router"]).output().await; });
                                }
                                
                                KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                    app.add_log(">>> TACTICAL KILL-SWITCH ENGAGED...".to_string());
                                    tokio::spawn(async { let _ = tokio::process::Command::new("sudo").args(["-n", "killall", "-9", "llama-server"]).output().await; });
                                }
                                _ => {}
                            }
                        }
                    }
                    // Add the network response handler here (still inside the select! block)
                    Event::ApiResponse { ttft_ms, status, message } => {
                        app.last_ttft = ttft_ms;
                        app.last_api_result = format!("[{}] {}", status, message);
                        app.add_log(format!("API Strike Completed: {}ms", ttft_ms));
                    }
                    Event::ModelsFetched(models) => {
                        app.available_models = models;
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

                        // Manage the rolling sparkline history
                        if app.cpu_history.len() >= 100 {
                            app.cpu_history.remove(0);
                        }
                        app.cpu_history.push(cpu_load);
                    }
                    Event::LogLine(line) => {
                        app.add_log(line);
                    }
                    Event::Tick => {} // Triggers a redraw implicitly
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    // 5. Clean Teardown
    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}