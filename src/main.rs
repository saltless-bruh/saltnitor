use app::{App, ProcPane};
use clap::Parser;
use crossterm::{
    ExecutableCommand,
    event::{self, Event as CEvent, KeyCode, KeyModifiers},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use events::Event;
use proc_keys::ProcKey;
use ratatui::{Terminal, backend::CrosstermBackend};
use saltnitor::{
    app, auth, config_v1, control_api, events, gpu, hotswap, interrogate, proc_keys, process,
    proxy_stream, systemctl, ui,
};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::{io, time::Duration};
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

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

    /// Config file to load (default: $XDG_CONFIG_HOME/saltnitor/config.toml, then ~/.config/...).
    #[arg(long, value_name = "FILE")]
    config: Option<std::path::PathBuf>,
}

/// Oracle metadata per profile, in the shape `control_api` consumes.
fn profile_metas(cfg: &config_v1::ConfigV1) -> HashMap<String, control_api::ProfileMeta> {
    cfg.profiles
        .iter()
        .map(|(id, p)| {
            (
                id.clone(),
                control_api::ProfileMeta {
                    model: p.model.clone(),
                    offload: p.offload,
                    est_vram_gb: p.est_vram_gb,
                    est_ram_gb: p.est_ram_gb,
                },
            )
        })
        .collect()
}

// --- Process control (REQ-PROC-003, REQ-TUI-010): exact PIDs, results come back as log lines ---

/// SIGTERM the selected PID on a blocking thread; the outcome arrives as a `LogLine`.
fn start_terminate(app: &mut App, info: &process::ProcessInfo, tx: &mpsc::Sender<Event>) {
    let target = process::Target::select(info);
    let (svc, grace_ms) = (app.service_name.clone(), app.term_grace_ms);
    let tx = tx.clone();
    app.add_log(format!(
        ">>> PROCESS: SIGTERM {} ({}) requested",
        info.pid, info.name
    ));
    tokio::task::spawn_blocking(move || {
        let guard = process::Guard::current(process::runtime_tree(&svc));
        let result = process::terminate_guarded(&target, &guard, Duration::from_millis(grace_ms));
        let _ = tx.blocking_send(Event::LogLine(process::describe_terminate(
            &target, &result, grace_ms,
        )));
    });
}

/// Arm the SIGKILL confirmation for the selected PID.
fn ask_kill(app: &mut App, info: &process::ProcessInfo) {
    app.confirm_line = Some(format!("SIGKILL {} ({})? [y/N]", info.pid, info.name));
    app.pending_kill = Some(process::Target::select(info));
}

/// The operator pressed `y`: SIGKILL the pinned target on a blocking thread.
fn confirm_kill(app: &mut App, target: process::Target, tx: &mpsc::Sender<Event>) {
    let svc = app.service_name.clone();
    let tx = tx.clone();
    tokio::task::spawn_blocking(move || {
        let guard = process::Guard::current(process::runtime_tree(&svc));
        let result = process::kill_guarded(&target, &guard);
        let _ = tx.blocking_send(Event::LogLine(process::describe_kill(&target, &result)));
    });
}

// --- Pre-Flight Dependency Checker ---
fn check_dependencies() -> Result<(), String> {
    let required_cmds = ["journalctl", "ss", "systemctl"];
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
        return Err(format!(
            "Missing critical Linux dependencies: {}",
            missing.join(", ")
        ));
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
    for (i, line) in lines.iter().enumerate().skip(start + 1) {
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') {
            end = i;
            break;
        }
    }

    let mut out: Vec<String> = lines[..=start].to_vec();
    let mut remaining: Vec<(String, String)> = kv.to_vec();
    for line in &lines[(start + 1)..end] {
        let trimmed = line.trim_start();
        let is_comment = trimmed.starts_with(';') || trimmed.starts_with('#');
        let key = trimmed.split('=').next().map(str::trim).unwrap_or("");
        if !is_comment
            && !key.is_empty()
            && let Some(pos) = remaining.iter().position(|(k, _)| k == key)
        {
            let (k, v) = remaining.remove(pos);
            out.push(format!("{} = {}", k, v));
            continue;
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
    let loaded = match config_v1::load(cli.config.as_deref(), &|k| std::env::var(k).ok()) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("saltnitor: invalid config (CONFIG_INVALID)\n{e}");
            std::process::exit(2); // before any listener, task, or child (REQ-CFG-003/AC1)
        }
    };
    for n in &loaded.notes {
        eprintln!("saltnitor: {n}"); // REQ-CFG-001/AC2
    }
    let mut config_notes = loaded.notes.clone();
    let toml_conf = loaded.config;
    let mut token_notes = Vec::new();
    let control_token = match config_v1::resolve_control_token(
        &toml_conf,
        &loaded.path,
        &|k| std::env::var(k).ok(),
        &mut token_notes,
    ) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("saltnitor: invalid config (CONFIG_INVALID)\n{e}");
            std::process::exit(2); // REQ-SEC-003/AC2
        }
    };
    for n in &token_notes {
        eprintln!("saltnitor: {n}");
    }
    config_notes.extend(token_notes);
    let proxy_limits =
        proxy_stream::ProxyLimits::from_config(&toml_conf.timeouts, toml_conf.max_body_bytes);
    if let Err(e) = proxy_stream::upstream_client(&proxy_limits) {
        eprintln!("saltnitor: {e}"); // before any listener or the terminal (REQ-ERR-005/AC1)
        std::process::exit(1);
    }

    // 1. Load Configuration
    let final_port = cli.port.or(toml_conf.port).unwrap_or(8080);
    let final_host = cli
        .host
        .or(toml_conf.host.clone())
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let final_svc = cli
        .service_name
        .or(toml_conf.service_name.clone())
        .unwrap_or_else(|| "llama-router".to_string());
    let final_ngl = toml_conf.default_ngl.unwrap_or(33);
    let final_ctx = toml_conf.default_ctx.unwrap_or(8192);

    // --- Enforce Pre-Flight Checks ---
    if let Err(e) = check_dependencies() {
        eprintln!("\n[!] SALTNITOR BOOT SEQUENCE HALTED");
        eprintln!("[!] {}", e);
        eprintln!(
            "[!] Please install the required packages (e.g., 'iproute2', 'psmisc', 'systemd') and try again.\n"
        );
        std::process::exit(1);
    }

    // 2. Pre-Flight Hardware Scan
    let mut sys = System::new_all();
    sys.refresh_cpu_specifics(CpuRefreshKind::everything());
    sys.refresh_memory();

    // Get Dynamic CPU & RAM
    let cpu_name = sys
        .cpus()
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "Unknown CPU".to_string());
    let cpu_core_count = sys.cpus().len();
    let ram_total = sys.total_memory() as f64 / 1_073_741_824.0;

    // Probing for NVIDIA GPU. `has_nvidia` only when the card's name and total VRAM were
    // actually read; otherwise the reason is logged and the GPU panes say n/a (INV-18).
    let mut gpu_name = "NO NVIDIA GPU DETECTED".to_string();
    let mut vram_total = 0.0; // unknown; never shown while has_nvidia is false
    let mut has_nvidia = false;
    let mut gpu_probe_note = None;

    match std::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,memory.total",
            "--format=csv,noheader,nounits",
        ])
        .output()
    {
        Ok(output) if output.status.success() => {
            match gpu::parse_device(&String::from_utf8_lossy(&output.stdout)) {
                Ok((name, total_gb)) => {
                    gpu_name = name;
                    vram_total = total_gb;
                    has_nvidia = true;
                }
                Err(reason) => gpu_probe_note = Some(format!("GPU probe: {reason}")),
            }
        }
        Ok(output) => {
            gpu_probe_note = Some(format!(
                "GPU probe: nvidia-smi exited with {}",
                output.status
            ));
        }
        // Not installed: a CPU-only host is legitimate, the header already says so.
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => gpu_probe_note = Some(format!("GPU probe: cannot run nvidia-smi: {e}")),
    }

    // 3. Terminal Initialization
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 4. Application State & Channels
    let mut app = App::new(
        cpu_name,
        cpu_core_count,
        ram_total,
        gpu_name,
        vram_total,
        has_nvidia,
        final_host.clone(),
        final_port,
        final_svc.clone(),
        final_ngl,
        final_ctx,
    );
    app.router_ini = toml_conf.router_ini.clone();
    app.term_grace_ms = toml_conf.process.term_grace_ms;
    app.control_port = toml_conf.control_port.unwrap_or(8765);
    app.client_bearer = config_v1::client_bearer(toml_conf.client_key_env.as_deref());
    app.infer_bearer = toml_conf.infer_bearer.clone();
    app.redactor = auth::Redactor::new(
        [
            control_token.clone(),
            toml_conf.infer_bearer.clone(),
            app.client_bearer.clone(),
        ]
        .into_iter()
        .flatten()
        .collect(),
    );
    for n in &config_notes {
        app.add_log(format!(">>> CONFIG: {n}"));
    }
    if let Some(note) = &gpu_probe_note {
        app.add_log(format!(">>> {note}"));
        app.last_error = Some(format!("gpu: {note}"));
    }
    let (tx, mut rx) = mpsc::channel::<Event>(100);

    // --- Headless Control API (Saltcode native-router bridge) ---
    // Sits in front of llama.cpp's native router (--models-preset). Provides the
    // VRAM oracle + /v1/ensure that the router itself lacks. Binds 127.0.0.1 only.
    {
        let controller = Arc::new(
            control_api::ControlApi::new(
                profile_metas(&toml_conf),
                toml_conf
                    .router_base
                    .clone()
                    .unwrap_or_else(|| format!("http://{}:{}", final_host, final_port)),
                toml_conf.infer_bearer.clone(),
                control_token.clone(),
                toml_conf.reserve_vram_gb.unwrap_or(0.8),
                toml_conf.reserve_ram_gb.unwrap_or(1.0),
                tx.clone(),
            )
            .and_then(|c| {
                c.allow_query_token(toml_conf.allow_query_token.unwrap_or(false))
                    .limits(proxy_limits)
            })?,
        );
        if toml_conf.allow_query_token.unwrap_or(false) {
            let warning = "saltnitor: security: allow_query_token=true — ?token= is accepted on GET /v1/ensure/stream only; the value is redacted in logs";
            eprintln!("{warning}");
            app.add_log(warning.to_string());
        }
        let control_port = toml_conf.control_port.unwrap_or(8765);
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], control_port));
        tokio::spawn(async move {
            control_api::serve(controller, addr).await;
        });
    }

    // 5. Start Event Producers (Background Tasks)
    let tx_keys = tx.clone();
    let tx_logs = tx.clone();

    // Task A: Keyboard Input Stream
    tokio::spawn(async move {
        loop {
            match event::poll(Duration::from_millis(250)) {
                Ok(true) => match event::read() {
                    Ok(CEvent::Key(key)) => {
                        if tx_keys.send(Event::Key(key)).await.is_err() {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        let _ = tx_keys
                            .send(Event::Error {
                                source: "keys".into(),
                                message: format!("terminal read failed: {e}"),
                            })
                            .await;
                        break;
                    }
                },
                Ok(false) => {}
                Err(e) => {
                    let _ = tx_keys
                        .send(Event::Error {
                            source: "keys".into(),
                            message: format!("terminal poll failed: {e}"),
                        })
                        .await;
                    break;
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
                .with_processes(ProcessRefreshKind::everything().without_tasks()),
        );

        let mut poll_count: u32 = 0;
        let mut gpu_failing = false;
        loop {
            tokio::time::sleep(Duration::from_millis(1000)).await;
            sys.refresh_cpu_specifics(CpuRefreshKind::everything());
            sys.refresh_memory();
            sys.refresh_processes_specifics(
                ProcessesToUpdate::All,
                true,
                ProcessRefreshKind::everything().without_tasks(),
            );

            // System Metrics
            let ram_used = sys.used_memory() as f64 / 1_073_741_824.0;
            let swap_used = sys.used_swap() as f64 / 1_073_741_824.0;
            let swap_total = sys.total_swap() as f64 / 1_073_741_824.0;
            let sys_uptime = System::uptime();

            let cpu_cores: Vec<f32> = sys.cpus().iter().map(|c| c.cpu_usage()).collect();
            let cpu_load = cpu_cores.iter().sum::<f32>() / cpu_cores.len() as f32;

            // One row per PID (BD-05): names are never a key.
            let raw = process::snapshot_from_sysinfo(&sys);
            // NVIDIA metrics: a failed query is `None` plus ONE error event on the transition to
            // failing (not every second), never zeros (INV-18, REQ-ERR-005/AC1).
            let mut gpu = None;
            let mut gpu_apps = Vec::new();
            let mut gpu_problem: Option<String> = None;
            if has_nvidia {
                // Expanded query to grab 9 specific data points at once
                match std::process::Command::new("nvidia-smi")
                    .args(["--query-gpu=memory.used,temperature.gpu,power.draw,power.limit,utilization.gpu,utilization.memory,fan.speed,clocks.gr,clocks.mem", "--format=csv,noheader,nounits"])
                    .output()
                {
                    Ok(o) if o.status.success() => match gpu::parse_sample(&String::from_utf8_lossy(&o.stdout)) {
                        Ok(sample) => gpu = Some(sample),
                        Err(reason) => gpu_problem = Some(reason),
                    },
                    Ok(o) => gpu_problem = Some(format!("nvidia-smi exited with {}", o.status)),
                    Err(e) => gpu_problem = Some(format!("cannot run nvidia-smi: {e}")),
                }
                // Processes: exact PIDs, merged into the table
                match std::process::Command::new("nvidia-smi")
                    .args([
                        "--query-compute-apps=pid,process_name,used_memory",
                        "--format=csv,noheader,nounits",
                    ])
                    .output()
                {
                    Ok(o) if o.status.success() => {
                        gpu_apps = process::parse_compute_apps(&String::from_utf8_lossy(&o.stdout));
                    }
                    Ok(o) => {
                        gpu_problem.get_or_insert(format!(
                            "nvidia-smi (compute apps) exited with {}",
                            o.status
                        ));
                    }
                    Err(e) => {
                        gpu_problem
                            .get_or_insert(format!("cannot run nvidia-smi (compute apps): {e}"));
                    }
                }
            }
            match (&gpu_problem, gpu_failing) {
                (Some(reason), false) => {
                    let _ = tx_hw
                        .send(Event::Error {
                            source: "gpu".into(),
                            message: reason.clone(),
                        })
                        .await;
                }
                (None, true) => {
                    let _ = tx_hw
                        .send(Event::LogLine(">>> GPU: telemetry restored".to_string()))
                        .await;
                }
                _ => {}
            }
            gpu_failing = gpu_problem.is_some();
            let processes = process::table(raw, &gpu_apps);

            // uid → user name, refreshed on the first poll and every 30th after it
            let users = if poll_count.is_multiple_of(30) {
                sysinfo::Users::new_with_refreshed_list()
                    .iter()
                    .map(|u| (**u.id(), u.name().to_string()))
                    .collect()
            } else {
                HashMap::new()
            };
            poll_count = poll_count.wrapping_add(1);

            let _ = tx_hw
                .send(Event::HardwareUpdate {
                    ram_used,
                    cpu_load: cpu_load as u64,
                    gpu,
                    cpu_cores,
                    swap_used,
                    swap_total,
                    processes,
                    users,
                    sys_uptime,
                })
                .await;

            let _ = tx_hw.send(Event::Tick).await;
        }
    });

    // Task C: Journalctl Log Streamer
    let service_c = final_svc.clone();
    tokio::spawn(async move {
        // Stream the dynamic service logs asynchronously
        let mut child = match Command::new("journalctl")
            .args(["-u", &service_c, "-f", "-n", "30"])
            .stdout(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                let _ = tx_logs
                    .send(Event::Error {
                        source: "journal".into(),
                        message: format!("cannot spawn journalctl: {e}"),
                    })
                    .await;
                return;
            }
        };

        let Some(stdout) = child.stdout.take() else {
            let _ = tx_logs
                .send(Event::Error {
                    source: "journal".into(),
                    message: "cannot capture journalctl stdout".into(),
                })
                .await;
            return;
        };
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
            if let Ok(res) = client.get(&url).send().await
                && let Ok(json) = res.json::<serde_json::Value>().await
                && let Some(data) = json.get("data").and_then(|d| d.as_array())
            {
                let models: Vec<String> = data
                    .iter()
                    .filter_map(|m| {
                        m.get("id")
                            .and_then(|id| id.as_str())
                            .map(|s| s.to_string())
                    })
                    .collect();

                let _ = tx_models.send(Event::ModelsFetched(models)).await;
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
                            proc_name = stdout[start..start + end].to_string();
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
                        if let Some(target) = app.pending_kill.take() {
                            // --- SIGKILL confirmation: only `y` proceeds (REQ-PROC-003/AC2) ---
                            app.confirm_line = None;
                            if key.code == KeyCode::Char('y') { confirm_kill(&mut app, target, &tx); }
                            else { app.add_log(">>> PROCESS: kill cancelled".to_string()); }
                        } else if app.show_gpu_inspector || app.show_sys_inspector {
                            // --- PROCESS SNIPER (GPU or CPU/RAM pane); keys classified by proc_keys (D3 finding) ---
                            let (pane, close_char) = if app.show_gpu_inspector { (ProcPane::Gpu, 'g') } else { (ProcPane::Sys, 'c') };
                            match proc_keys::classify(&key, close_char) {
                                ProcKey::Close => {
                                    if app.show_gpu_inspector { app.show_gpu_inspector = false; } else { app.show_sys_inspector = false; }
                                }
                                ProcKey::Up => app.move_proc_cursor(pane, false),
                                ProcKey::Down => app.move_proc_cursor(pane, true),
                                ProcKey::Terminate => {
                                    // the PID the operator selected, found in the current rows (not a list index)
                                    let sel = app.selected_proc(pane).cloned();
                                    if let Some(info) = sel { start_terminate(&mut app, &info, &tx); }
                                }
                                ProcKey::Kill => {
                                    let sel = app.selected_proc(pane).cloned();
                                    if let Some(info) = sel { ask_kill(&mut app, &info); }
                                }
                                ProcKey::Other => {}
                            }
                        } else if app.show_tuner {
                            // --- DEEP TUNER MENU CONTROLS (Keep exact same as before) ---
                            match key.code {
                                KeyCode::Esc | KeyCode::Char('t') => app.show_tuner = false,
                                KeyCode::Tab => { app.tuner_page = (app.tuner_page + 1) % 3; app.tuner_selected = 0; }
                                KeyCode::Up if app.tuner_selected > 0 => { app.tuner_selected -= 1; }
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
                                    } else if app.router_ini.is_none() {
                                        app.add_log(">>> TUNER: refusing to apply — set `router_ini` in config.toml (REQ-SEC-013, DEC-04)".to_string());
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
                                        let router_ini = app.router_ini.clone().unwrap_or_default();
                                        let tx_t = tx.clone();
                                        tokio::spawn(async move {
                                            let router_ini_path = router_ini.as_str();
                                            match tokio::fs::read_to_string(router_ini_path).await {
                                                Ok(content) => match upsert_ini_section(&content, &section, &kv) {
                                                    Some(updated) => {
                                                        if tokio::fs::write(router_ini_path, updated).await.is_ok() {
                                                            let _ = tx_t.send(Event::LogLine(format!(">>> TUNER: [{}] updated. Restarting router...", section))).await;
                                                            systemctl::run_and_report("sudo", "restart", &svc_name, &tx_t, &format!(">>> TUNER: router restarted. If it does not come back, a key may be unsupported - check: journalctl -u {svc_name}")).await;
                                                        } else {
                                                            let _ = tx_t.send(Event::LogLine(format!(">>> TUNER ERROR: cannot write {}", router_ini_path))).await;
                                                        }
                                                    }
                                                    None => { let _ = tx_t.send(Event::LogLine(format!(">>> TUNER ERROR: section [{}] not found in router.ini", section))).await; }
                                                },
                                                Err(_) => { let _ = tx_t.send(Event::LogLine(format!(">>> TUNER ERROR: cannot read {}", router_ini_path))).await; }
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
                                        if let Some(i) = app.hot_swap_state.selected()
                                            && let Some(chosen_model) = app.available_models.get(i).cloned() {
                                                app.console_focused = false;

                                                // 1. Calculate NGL
                                                let mut auto_ngl = 99;
                                                let model_upper = chosen_model.to_uppercase();
                                                for word in model_upper.replace("-", " ").replace("_", " ").split_whitespace() {
                                                    if word.ends_with("B")
                                                        && let Ok(p) = word.trim_end_matches('B').parse::<f64>()
                                                            && p > 14.0 { auto_ngl = 24; }
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
                                                // The direct hop to the raw router uses the router's own key
                                                // (`infer_bearer`), never the daemon client key, and only for a
                                                // loopback router (INV-06, REQ-SEC-012/AC1).
                                                let host_api = app.host.clone();
                                                let port_api = app.port;
                                                let warmup_model = chosen_model.clone();
                                                let bearer = hotswap::direct_router_bearer(&app.host, app.api_key, app.infer_bearer.as_deref());
                                                let tx_warmup = tx.clone();
                                                tokio::spawn(async move {
                                                    match hotswap::warm_load(&host_api, port_api, bearer, &warmup_model).await {
                                                        Ok(()) => {
                                                            let _ = tx_warmup.send(Event::ActiveModelSet(warmup_model.clone())).await;
                                                            let _ = tx_warmup.send(Event::LogLine(format!(">>> HOT-SWAP: [{}] resident & warm.", warmup_model))).await;
                                                        }
                                                        Err(reason) => { let _ = tx_warmup.send(Event::Error { source: "hot-swap".into(), message: reason }).await; }
                                                    }
                                                });
                                            }
                                    }
                                    _ => {}
                                }
                            } else { // INTERROGATOR MODE
                                match key.code {
                                    KeyCode::Esc => app.console_focused = false, // Exit insert mode
                                    KeyCode::Left if app.console_cursor > 0 => { app.console_cursor -= 1; }
                                    KeyCode::Right if app.console_cursor < app.console_input.chars().count() => { app.console_cursor += 1; }
                                    KeyCode::Up
                                        if !app.console_history.is_empty() && app.history_index > 0 => {
                                            app.history_index -= 1;
                                            app.console_input = app.console_history[app.history_index].clone();
                                            app.console_cursor = app.console_input.chars().count();
                                        }
                                    KeyCode::Down
                                        if app.history_index < app.console_history.len() => {
                                            app.history_index += 1;
                                            if app.history_index == app.console_history.len() {
                                                app.console_input = String::new();
                                            } else {
                                                app.console_input = app.console_history[app.history_index].clone();
                                            }
                                            app.console_cursor = app.console_input.chars().count();
                                        }
                                    KeyCode::Char(c) => {
                                        let mut chars: Vec<char> = app.console_input.chars().collect();
                                        chars.insert(app.console_cursor, c);
                                        app.console_input = chars.into_iter().collect();
                                        app.console_cursor += 1;
                                    }
                                    KeyCode::Backspace
                                        if app.console_cursor > 0 => {
                                            let mut chars: Vec<char> = app.console_input.chars().collect();
                                            chars.remove(app.console_cursor - 1);
                                            app.console_input = chars.into_iter().collect();
                                            app.console_cursor -= 1;
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
                                        // The interrogator goes through Saltnitor's own endpoint with the client key (REQ-TUI-007/AC1).
                                        let control_port = app.control_port;
                                        let bearer = app.client_bearer.clone();
                                        tokio::spawn(interrogate::strike(control_port, bearer, payload, tx_api));
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
                                    let tx_s = tx.clone();
                                    tokio::spawn(async move { systemctl::run_and_report("sudo", "start", &svc, &tx_s, &format!(">>> SYSTEMCTL: {svc} started.")).await; });
                                }
                                KeyCode::Char('X') => {
                                    let svc = app.service_name.clone();
                                    app.add_log(format!(">>> SYSTEMCTL: Stopping {}...", svc));
                                    let tx_s = tx.clone();
                                    tokio::spawn(async move { systemctl::run_and_report("sudo", "stop", &svc, &tx_s, &format!(">>> SYSTEMCTL: {svc} stopped.")).await; });
                                }
                                KeyCode::Char('R') => {
                                    let svc = app.service_name.clone();
                                    app.add_log(format!(">>> SYSTEMCTL: Restarting {}...", svc));
                                    let tx_s = tx.clone();
                                    tokio::spawn(async move { systemctl::run_and_report("sudo", "restart", &svc, &tx_s, &format!(">>> SYSTEMCTL: {svc} restarted.")).await; });
                                }
                                KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                    // Kill-switch must STOP the unit. With Restart=always a bare SIGKILL is
                                    // respawned in ~2s and VRAM never frees; stopping the unit wins and stays down.
                                    let svc = app.service_name.clone();
                                    app.add_log(">>> TACTICAL KILL-SWITCH: stopping unit (frees VRAM, stays down)...".to_string());
                                    let tx_k = tx.clone();
                                    tokio::spawn(async move {
                                        systemctl::run_and_report("sudo", "stop", &svc, &tx_k, ">>> KILL-SWITCH: unit stopped, VRAM freed. (Shift+S to restart.)").await;
                                    });
                                }
                                KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                    // Crash dump -> an ABSOLUTE path under $HOME, and report where it went (or
                                    // if it failed) instead of writing to an unknown CWD and swallowing errors.
                                    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
                                    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                                    let path = format!("{}/saltnitor_crash_{}.txt", home, timestamp);
                                    let content = app.crash_dump_text(&timestamp);
                                    app.add_log(format!(">>> CRASH DUMP -> {}", path));
                                    let tx_d = tx.clone();
                                    tokio::spawn(async move {
                                        use tokio::io::AsyncWriteExt;
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
                    Event::ApiStreamEnd { metrics, status } => {
                        let (ttft, pp, tg) = interrogate::render(&metrics);
                        app.last_metrics = metrics;
                        let final_msg = format!("[{}] {}", status, app.last_api_result.chars().take(30).collect::<String>());
                        app.add_log(format!("API Strike: {ttft} | {pp} | {tg} | {final_msg}"));
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
                    Event::HardwareUpdate { ram_used, cpu_load, gpu, cpu_cores, swap_used, swap_total, processes, users, sys_uptime } => {
                        app.ram_used = ram_used;
                        app.cpu_cores = cpu_cores;
                        app.swap_used = swap_used;
                        app.swap_total = swap_total;
                        app.sys_uptime = sys_uptime;
                        app.apply_gpu_sample(gpu);

                        app.set_processes(&processes, users);

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
                    Event::Error { source, message } => {
                        if source == "interrogator" {
                            // Show why the strike failed in the deck instead of leaving "Sending payload...".
                            if app.last_api_result == "Sending payload..." { app.last_api_result.clear(); }
                            app.last_api_result.push_str(&format!("ERROR: {message}"));
                        }
                        app.last_error = Some(format!("{source}: {message}"));
                        app.add_log(format!(">>> ERROR [{source}]: {message}"));
                    }
                    Event::Tick => {}
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Save the history buffer; a failure is printed after the alternate screen is left (below).
    let history_error = if app.console_history.is_empty() {
        None
    } else {
        let path = interrogate::history_path(&|k| std::env::var(k).ok());
        interrogate::save_history(&path, &app.console_history.join("\n")).err()
    };

    // 6. Clean Teardown
    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;
    if let Some(e) = history_error {
        eprintln!("saltnitor: {e}");
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    // tests may unwrap: a panic is the failure signal (REQ-CI-007 scopes the deny to non-test code)
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::upsert_ini_section;

    fn kv(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[test]
    fn upsert_replaces_appends_and_keeps_comments_and_other_sections() {
        let ini = "[*]\nctx-size = 1\n\n[A]\nmodel = /m/a.gguf\nctx-size = 4096\n; ctx-size = 2\n# note\n\n[B]\nctx-size = 1\n";
        let got = upsert_ini_section(
            ini,
            "A",
            &kv(&[("ctx-size", "8192"), ("n-gpu-layers", "99")]),
        );
        assert_eq!(
            got.as_deref(),
            Some(
                "[*]\nctx-size = 1\n\n[A]\nmodel = /m/a.gguf\nctx-size = 8192\n; ctx-size = 2\n# note\n\nn-gpu-layers = 99\n[B]\nctx-size = 1\n"
            )
        );
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[test]
    fn upsert_matches_keys_without_spaces_and_normalizes_the_line() {
        assert_eq!(
            upsert_ini_section("[A]\nctx-size=4096\n", "A", &kv(&[("ctx-size", "1")])).as_deref(),
            Some("[A]\nctx-size = 1\n")
        );
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[test]
    fn upsert_in_the_last_section_appends_and_ends_with_newline() {
        assert_eq!(
            upsert_ini_section("[A]\nmodel = x", "A", &kv(&[("n-gpu-layers", "99")])).as_deref(),
            Some("[A]\nmodel = x\nn-gpu-layers = 99\n")
        );
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[test]
    fn upsert_matches_a_header_with_surrounding_whitespace() {
        assert_eq!(
            upsert_ini_section("  [A]  \nk = 1\n", "A", &kv(&[("k", "2")])).as_deref(),
            Some("  [A]  \nk = 2\n")
        );
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[test]
    fn upsert_missing_section_is_none() {
        assert_eq!(
            upsert_ini_section("[A]\nk = 1\n", "B", &kv(&[("k", "2")])),
            None
        );
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[test]
    fn upsert_stops_at_the_next_section_when_several_follow() {
        let ini = "[A]\nk = 1\n[B]\nn = 2\n[C]\nn = 3\n";
        assert_eq!(
            upsert_ini_section(ini, "A", &kv(&[("n", "7")])).as_deref(),
            Some("[A]\nk = 1\nn = 7\n[B]\nn = 2\n[C]\nn = 3\n")
        );
        assert_eq!(
            upsert_ini_section(ini, "B", &kv(&[("n", "7")])).as_deref(),
            Some("[A]\nk = 1\n[B]\nn = 7\n[C]\nn = 3\n")
        );
    }
}
