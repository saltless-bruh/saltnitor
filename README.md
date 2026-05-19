# Saltnitor

**Saltnitor** is a high-performance, hardware-agnostic Terminal User Interface (TUI) built in Rust, serving as a central command center for orchestrating local Large Language Models (LLMs) and monitoring hybrid hardware pressure between GPU VRAM and System RAM.

Designed specifically for developers running `llama.cpp` or `llama-router` on Linux, Saltnitor provides real-time deep telemetry, intelligent log analysis, and tactical control in a single, ultra-lightweight binary.

<img width="1920" height="1044" alt="image" src="https://github.com/user-attachments/assets/57a2bc4b-b278-47ec-a0be-63a29af12679" />

## 🚀 Key Features

- **Dynamic Hardware Probing & Telemetry**: Automatically identifies CPU architecture and NVIDIA GPU specifications on boot. Dynamically scales the UI to match your machine's thread count and memory limits, featuring real-time CPU load sparklines and precise VRAM/RAM saturation gauges.
    
- **Tactical Hardware Inspectors**:
    
    - **GPU Deep-Dive (`g`)**: Real-time VRAM allocation, core temperatures, wattage draw, and fan speeds. Includes an active process list to identify exactly which external applications are dominating your VRAM.
      <img width="467" height="401" alt="image" src="https://github.com/user-attachments/assets/c7610f56-9c38-47aa-9ddc-18fa97f29685" />


        
    - **CPU/System Deep-Dive (`c`)**: A dynamic graphical equalizer showing load distribution across all threads, total system uptime, and SSD Swap spillover metrics.
      <img width="552" height="479" alt="image" src="https://github.com/user-attachments/assets/e3897b3a-aac4-49d2-a9d4-6746505c21c0" />

        
- **Live Model Orchestration**:
    
    - **Model Selector (`m`)**: Scan and hot-swap GGUF models dynamically via the `llama-router` API. Automatically targets the active model for console payloads.
      <img width="513" height="405" alt="image" src="https://github.com/user-attachments/assets/d12f627f-5cd4-47c3-9169-4cfb83beb520" />

        
    - **Deep Engine Tuner (`t`)**: A paginated, 3-panel configuration manifest (cycle with `Tab`) allowing live, on-the-fly injection of advanced `llama.cpp` parameters directly to `router.ini`:
        
        - _Page 1 (Compute & Memory)_: `ngl`, `ctx`, threads, batching, parallel slots, Flash Attention, mlock, no_mmap, and KV Cache quantization.
      <img width="494" height="337" alt="image" src="https://github.com/user-attachments/assets/b7678cfb-d16c-4848-ae47-b6986f09e9c1" />

            
        - _Page 2 (Context & Caching)_: RoPE scaling, defragmentation thresholds, speculative decoding (Draft tokens), and persistent Prompt Caching to SSD.
      <img width="494" height="337" alt="image" src="https://github.com/user-attachments/assets/cf2c45cc-2efb-4cf6-8af8-89e31c803ce3" />

            
        - _Page 3 (Default Sampling)_: Temperature, Top-K, Top-P, Min-P, and Repetition Penalties.
      <img width="494" height="337" alt="image" src="https://github.com/user-attachments/assets/0291d190-7349-454a-99a4-ffdf369b0f0a" />

            
- **Advanced API Interrogator (`i`)**: A built-in mini-console for firing test payloads directly to your local inference server.
  <img width="1899" height="108" alt="image" src="https://github.com/user-attachments/assets/80f5dcbc-3022-4d7b-8578-75ba4cdc56e5" />

    
    - **Granular Benchmarking**: Tracks millisecond-accurate Time-To-First-Token (TTFT) alongside precise, split Tokens-Per-Second (t/s) metrics for both **Prompt Evaluation** and **Generation**.
        
    - **Persistent Command History**: Features a Bash-style history buffer allowing you to cycle through previous payloads using the `Up` and `Down` arrow keys, complete with inline cursor editing. History is saved to `.saltnitor_history` on exit.
        
    - **Clean Response Parsing**: Automatically extracts, cleans, and wraps conversational AI text from raw OpenAI-compatible JSON responses.
        
- **Intelligent Log Streamer**: Real-time `systemd` journal monitoring with keyword-based color highlighting for OOM errors, hot-swapping states, and inference activity.
    
- **Tactical Incident Response**:
    
    - **Crash Dumping (`Ctrl+D`)**: Instantly export a post-mortem snapshot of your exact system state (VRAM/RAM pressure, temperatures, active model, and the last 100 log lines) to a timestamped file during an Out-Of-Memory (OOM) event.
        
    - **Kill-Switch (`Ctrl+K`)**: A dedicated emergency binding to forcefully terminate rogue inference threads and clear hung ports.
        

## 🛠 Prerequisites

- **OS**: Linux (Optimized for Pop!_OS / Ubuntu).
    
- **Systemd**: Required for log streaming and service management.
    
- **NVIDIA Drivers**: Required for GPU telemetry (via `nvidia-smi`).
    
- **llama.cpp / llama-router**: Recommended daemon for dynamic model serving.
    

## 📦 Installation & Setup

1. **Clone the Repository**
    
    ```
    git clone [https://github.com/Saltless-bruh/saltnitor.git](https://github.com/Saltless-bruh/saltnitor.git)
    cd saltnitor
    ```
    
2. **Build for Release**
    
    ```
    cargo build --release
    ```
    
3. **Run with Elevated Privileges**
    
    Because the program manages system services and performs deep port auditing, it requires `sudo` to function correctly.
    
    ```
    sudo ./target/release/saltnitor
    
    # Or to install globally:
    sudo cp ./target/release/saltnitor /usr/local/bin/
    sudo saltnitor
    ```
    

## ⚙️ Persistent Configuration

Saltnitor utilizes a layered configuration architecture. It intelligently merges CLI arguments with a persistent TOML config file. **Crucially, it is Sudo-Aware:** when run under `sudo`, it bypasses the root profile and correctly resolves the config from the invoking user's home directory.

**1. CLI Override Flags**

```
sudo saltnitor --host 192.168.1.5 --port 8081 --service-name my-llama
```

**2. Global Config File** (`~/.config/saltnitor/config.toml`)

```
# Saltnitor Defaults
port = 8080
host = "127.0.0.1"
service_name = "llama-router"

# Tuner Engine Limits
default_ngl = 33
default_ctx = 8192
```

## ⌨️ Quick Reference

|   |   |
|---|---|
|**Key**|**Action**|
|`q`|Quit Program|
|`h`|Open Interactive Command Manual|
|`PgUp` / `PgDn`|Scroll Log Streamer History|
|`t`|Open Deep Engine Tuner|
|`Tab`|Cycle Config Tuner Pages|
|`Enter`|Apply Config Tuner Settings|
|`m`|Open Target Model Selector|
|`i`|Focus API Interrogator (Insert Mode)|
|`Esc`|Exit Insert Mode|
|`Up` / `Down`|Cycle API Payload History (While in Insert Mode)|
|`g`|Toggle GPU Hardware Inspector|
|`c`|Toggle CPU/System Hardware Inspector|
|`Shift + S/X/R`|Daemon Start / Stop / Restart|
|`Ctrl+D`|Tactical Crash Dump (Save state to file)|
|`Ctrl+K`|Tactical Kill-Switch (`killall -9 llama-server`)|

## ⚠️ Important Notes

- **Elevated Environment**: If running via `sudo cargo run`, ensure your `PATH` is preserved or point directly to the compiled binary. `sudo` often hides user-installed tools like `cargo`.
    
- **Permission Transparency**: This tool uses `sudo -n` for background operations. You must launch the TUI itself with `sudo` to allow hotkeys like **Restart** and **Kill-Switch** to execute without interactive password prompts.
    
- **GPU Detection**: If no NVIDIA GPU is detected, Saltnitor will gracefully disable VRAM saturation gauges and the GPU Inspector while maintaining full CPU/RAM monitoring.
    
- **Terminal Sizing Guardrails**: Saltnitor requires a minimum terminal footprint of 80x24. If the window is resized below this threshold, rendering will halt to prevent mathematical panics.
    

## 🛡 License

Distributed under the MIT License. See `LICENSE` for more information.
