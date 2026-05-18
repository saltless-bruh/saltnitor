# Saltnitor

**Saltnitor** is a high-performance, hardware-agnostic Terminal User Interface (TUI) built in Rust, serving as a central command center for orchestrating local Large Language Models (LLMs) and monitoring hybrid hardware pressure between GPU VRAM and System RAM.

Designed specifically for developers running `llama.cpp` or `llama-router` on Linux, Saltnitor provides real-time deep telemetry, intelligent log analysis, and tactical control in a single, ultra-lightweight binary.

<img width="1921" height="1003" alt="image" src="https://github.com/user-attachments/assets/72348c2c-50c8-4d5e-8086-b39f20f96a38" />

## 🚀 Key Features

- **Dynamic Hardware Probing & Telemetry**: Automatically identifies CPU architecture and NVIDIA GPU specifications on boot. Dynamically scales the UI to match your machine's thread count and memory limits, featuring real-time CPU load sparklines and precise VRAM/RAM saturation gauges.
    
- **Tactical Hardware Inspectors**:
    
    - **GPU Deep-Dive (`g`)**: Real-time VRAM allocation, core temperatures, wattage draw, and fan speeds. Includes an active process list to identify exactly which external applications are dominating your VRAM.
        
    - **CPU/System Deep-Dive (`c`)**: A dynamic graphical equalizer showing load distribution across all threads, total system uptime, and SSD Swap spillover metrics.
        
- **Live Model Orchestration**:
    
    - **Model Selector (`m`)**: Scan and hot-swap GGUF models dynamically via the `llama-router` API. Automatically targets the active model for console payloads.
        
    - **Config Tuner (`t`)**: A multi-page tuner (use `Tab`) for `ngl`, `ctx`, threading, batching, cache types, rope scaling, draft settings, and prompt-cache flags, with live updates to `router.ini`.
        
- **Advanced API Interrogator (`i`)**: A built-in mini-console for firing test payloads directly to your local inference server.
    
    - **Granular Benchmarking**: Tracks millisecond-accurate Time-To-First-Token (TTFT) alongside precise, split Tokens-Per-Second (t/s) metrics for both **Prompt Evaluation** and **Generation**.
        
    - **Command History**: Features a Bash-style history buffer allowing you to cycle through previous payloads using the `Up` and `Down` arrow keys, complete with inline cursor editing. History persists to `.saltnitor_history` on exit.
        
    - **Clean Response Parsing**: Automatically extracts, cleans, and wraps conversational AI text from raw OpenAI-compatible JSON responses.
        
- **Intelligent Log Streamer**: Real-time `systemd` journal monitoring with keyword-based color highlighting for OOM errors, hot-swapping states, and inference activity.
    
- **Tactical Incident Response**:
    
    - **Crash Dumping (`Ctrl+D`)**: Instantly export a post-mortem snapshot of your exact system state (VRAM/RAM pressure, temperatures, active model, and the last 100 log lines) to a timestamped file during an Out-Of-Memory (OOM) event.
        
    - **Kill-Switch (`Ctrl+K`)**: A dedicated emergency binding to forcefully terminate rogue inference threads and clear hung ports.

- **Sudo-Aware Configuration**: Loads `~/.config/saltnitor/config.toml` from the invoking user's home (even under `sudo`), with CLI overrides for host, port, and service name.
        

## 🛠 Prerequisites

- **OS**: Linux (Optimized for Pop!_OS / Ubuntu).
    
- **Systemd**: Required for log streaming and service management.
    
- **NVIDIA Drivers**: Required for GPU telemetry (via `nvidia-smi`).
    
- **llama.cpp / llama-router**: Recommended daemon for dynamic model serving.
    

## 📦 Installation & Setup

1. **Clone the Repository**
    
    Bash
    
    ```
    git clone https://github.com/Saltless-bruh/saltnitor.git
    cd saltnitor
    ```
    
2. **Build for Release**
    
    Bash
    
    ```
    cargo build --release
    ```
    
3. **Run with Elevated Privileges**
    
    Because the program manages system services and performs deep port auditing, it requires `sudo` to function correctly.
    
    Bash
    
    ```
    sudo ./target/release/saltnitor
    
    # Or to install globally:
    sudo cp ./target/release/saltnitor /usr/local/bin/
    sudo saltnitor
    ```

## ⚙️ Configuration

Saltnitor merges CLI arguments with a TOML config file. When run under `sudo`, it still resolves the config from the invoking user's home directory via `SUDO_USER`.

**CLI flags**

```
--host <host>
--port <port>
--service-name <name>
```

**Config file** (`~/.config/saltnitor/config.toml`)

```
port = 8080
host = "127.0.0.1"
service_name = "llama-router"
default_ngl = 33
default_ctx = 8192
```
    

## ⌨️ Quick Reference

|**Key**|**Action**|
|---|---|
|`q`|Quit Program|
|`t`|Open Config Tuner|
|`m`|Open Target Model Selector|
|`i`|Focus API Interrogator (Insert Mode)|
|`Esc`|Exit Insert Mode|
|`Up` / `Down`|Cycle API Payload History (While in Insert Mode)|
|`Tab`|Cycle Config Tuner Pages|
|`Enter`|Apply Config Tuner Settings|
|`g`|Toggle GPU Hardware Inspector|
|`c`|Toggle CPU/System Hardware Inspector|
|`S`/`X`/`R` (Shift)|Daemon Start / Stop / Restart|
|`Ctrl+D`|Tactical Crash Dump (Save state to file)|
|`Ctrl+K`|Tactical Kill-Switch (`killall -9 llama-server`)|

## ⚠️ Important Notes

- **Elevated Environment**: If running via `sudo cargo run`, ensure your `PATH` is preserved or point directly to the compiled binary. `sudo` often hides user-installed tools like `cargo`.
    
- **Permission Transparency**: This tool uses `sudo -n` for background operations. You must launch the TUI itself with `sudo` to allow hotkeys like **Restart** and **Kill-Switch** to execute without interactive password prompts.
    
- **GPU Detection**: If no NVIDIA GPU is detected, Saltnitor will gracefully disable VRAM saturation gauges and the GPU Inspector while maintaining full CPU/RAM monitoring.

- **History File**: The API console history is saved to `.saltnitor_history` in the working directory on exit.
    

## 🛡 License

Distributed under the MIT License. See `LICENSE` for more information.
