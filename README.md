# Saltnitor

**Saltnitor** is a high-performance, hardware-agnostic Terminal User Interface (TUI) built in Rust, serving as a central command center for orchestrating local Large Language Models (LLMs) and monitoring hybrid hardware pressure between GPU VRAM and System RAM.

Designed specifically for developers running `llama.cpp` on Linux, Saltnitor provides real-time deep telemetry, intelligent log analysis, and tactical control in a single, ultra-lightweight binary.

<img width="1920" height="1001" alt="image" src="https://github.com/user-attachments/assets/0b3bd3de-f3aa-4d23-be48-bc446c76a1f1" />


## 🚀 Key Features

- **Dynamic Hardware Probing & Telemetry**: Automatically identifies CPU architecture and NVIDIA GPU specifications on boot. Dynamically scales the UI to match your machine's thread count and memory limits, featuring real-time CPU load sparklines and precise VRAM/RAM saturation gauges.
    
- **Tactical Hardware Inspectors**:
    - **GPU Deep-Dive (`g`)**: Real-time VRAM allocation, core temperatures, wattage draw, and fan speeds. Includes an active process list to identify exactly which external applications are dominating your VRAM.
    <img width="1920" height="154" alt="image" src="https://github.com/user-attachments/assets/fe770fcf-1f60-480e-97f7-04ecdaf9f6a2" />

        
    - **CPU/System Deep-Dive (`c`)**: A balanced 60/40 UI split featuring inline `btop`-style gauges, a dynamic graphical equalizer showing load distribution across all physical/logical threads, and a deduplicated list of top RAM culprits.
    <img width="1920" height="154" alt="image" src="https://github.com/user-attachments/assets/70abade2-4e9d-4ada-930f-21d5d9992168" />


- **Live Model Orchestration (Dual-Mode Bottom Deck)**:
    - **Auto-Tuning Hot-Swap**: Cycle available `.gguf` models dynamically. Features an intelligent **VRAM Oracle** that heuristically estimates the footprint of a model and warns you of potential OOM crashes before you execute the swap.
    <img width="1920" height="115" alt="image" src="https://github.com/user-attachments/assets/578e0c6c-c9dc-4898-9a3b-e28d57251970" />

        
    - **Deep Engine Tuner (`t`)**: A paginated configuration manifest that generates a native Linux `router.env` file and executes a bash-wrapper translation to control the `llama-server` runtime on the fly:
        - *Page 1 (Compute & Memory)*: `ngl`, `ctx`, threads, micro-batching, parallel slots, Flash Attention, `mlock`, and exact KV Cache quantization algorithms (`q8_0`, `q4_0`, etc.).
    <img width="515" height="403" alt="image" src="https://github.com/user-attachments/assets/b790cf6f-7288-4890-b11f-e9b79d953055" />

        - *Page 2 (Context & Speculation)*: RoPE scaling, VRAM defragmentation thresholds, and Speculative Decoding targets (`-md`).
    <img width="515" height="403" alt="image" src="https://github.com/user-attachments/assets/6e0f8764-91fc-42ec-b07f-2f4b2f9627a2" />

        - *Page 3 (Orchestration & Security)*: Core threading split (`-tb`), Continuous Batching, Context Shifting, and dynamic API Key authorization lock-downs.
    <img width="515" height="403" alt="image" src="https://github.com/user-attachments/assets/f24cffab-4bb9-4c2c-8736-8605e883fd7e" />


- **Advanced API Interrogator (`i`)**: A built-in mini-console for firing test payloads directly to your local inference server.
<img width="1920" height="112" alt="image" src="https://github.com/user-attachments/assets/a7174bab-3d79-476e-bcd5-e96a6c64d30a" />

    - **Granular Benchmarking**: Tracks millisecond-accurate Time-To-First-Token (TTFT) alongside precise, split Tokens-Per-Second (t/s) metrics for both **Prompt Evaluation** and **Generation**.
    - **Immune to Self-Lockout**: Dynamically injects Bearer Authentication tokens if the daemon's API Key security wall is engaged via the Tuner.
    - **Persistent Command History**: Bash-style history buffer with inline cursor editing, saved to `.saltnitor_history` on exit.

- **Tactical Incident Response**:
    - **Crash Dumping (`Ctrl+D`)**: Instantly export a post-mortem snapshot of your exact system state (VRAM/RAM pressure, temperatures, active model, and the last 100 log lines) to a timestamped file during an Out-Of-Memory (OOM) event.
    - **Kill-Switch (`Ctrl+K`)**: A dedicated emergency binding to forcefully terminate rogue inference threads and clear hung ports.


## 🛠 Prerequisites
- **OS**: Linux (Optimized for Pop!_OS / Ubuntu / Arch).
- **Systemd**: Required for log streaming and service management.
- **NVIDIA Drivers**: Required for GPU telemetry (via `nvidia-smi`).
- **llama.cpp**: Engine must be orchestrated via a custom `launch_router.sh` bash wrapper.

## 📦 Installation & Setup

1. **Clone the Repository**
    ```bash
    git clone [https://github.com/Saltless-bruh/saltnitor.git](https://github.com/Saltless-bruh/saltnitor.git)
    cd saltnitor
    ```
2. **Build for Release**
    ```bash
    cargo build --release
    ```
3. **Run with Elevated Privileges**
    Because the program manages system services and performs deep port auditing, it requires `sudo` to function correctly.
    ```bash
    sudo ./target/release/saltnitor
    ```

## ⌨️ Quick Reference

| Key | Action |
|---|---|
| `q` | Quit Program |
| `h` | Open Interactive Command Manual |
| `PgUp / PgDn` | Scroll Log Streamer History |
| `t` | Open Deep Engine Tuner |
| `Tab` | Toggle Bottom Deck (Interrogator vs Hot-Swap) |
| `Enter` | Apply Config Tuner Settings / Fire Payload / Swap Model |
| `i` | Focus Active Bottom Deck (Insert Mode) |
| `Esc` | Exit Insert Mode |
| `Up / Down` | Cycle History / Sniper Targets |
| `g` | Toggle GPU Hardware Inspector |
| `c` | Toggle CPU/System Hardware Inspector |
| `Shift + S/X/R` | Daemon Start / Stop / Restart |
| `Ctrl+D` | Tactical Crash Dump (Save state to file) |
| `Ctrl+K` | Tactical Kill-Switch (`killall -9 llama-server`) |

## ⚠️ Important Notes
- **Permission Transparency**: This tool uses `sudo -n` for background operations. You must launch the TUI itself with `sudo` to allow hotkeys like **Restart** and **Kill-Switch** to execute without interactive password prompts.
- **Terminal Sizing Guardrails**: Saltnitor requires a minimum terminal footprint of 80x16. If the window is resized below this threshold, rendering will halt to prevent mathematical panics.
