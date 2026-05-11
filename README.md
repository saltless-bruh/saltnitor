# Saltnitor

**Saltnitor** is a high-performance, hardware-agnostic Terminal User Interface (TUI) built in Rust. Its serving as a central "Command Center" for orchestrating local Large Language Models (LLMs) and monitoring hybrid hardware pressure between GPU VRAM and System RAM.

Designed specifically for developers running `llama.cpp` or `llama-router` on Linux, Saltnitor provides real-time telemetry, log analysis, and tactical control in a single, lightweight binary.

<img width="1920" height="1048" alt="image" src="https://github.com/user-attachments/assets/e0b96bd1-e880-46de-a6f8-d6297ff547cc" />

## 🚀 Key Features

- **Dynamic Hardware Probing**: On boot, Saltnitor automatically identifies your CPU architecture (AVX-512 support, core counts) and NVIDIA GPU specifications. It dynamically scales its UI to match your machine's thread count and memory limits.
    
- **Tactical Hardware Inspectors**:
    
    - **GPU Deep-Dive (`g`)**: Real-time VRAM allocation, core temperatures, wattage draw, and fan speeds. Includes a process list to identify exactly which apps are occupying VRAM.
        
    - **CPU/System Deep-Dive (`c`)**: A dynamic graphical equalizer showing load distribution across all threads, system uptime, and SSD Swap spillover metrics.
        
- **Live Model Orchestration**:
    
    - **Model Selector (`m`)**: Scan and hot-swap GGUF models via the `llama-router` API.
        
    - **Config Tuner (`t`)**: Modify `ngl` (GPU Layers) and `ctx` (Context Size) on the fly with live updates to `router.ini`.
        
- **API Interrogator (`i`)**: A built-in mini-console to fire test payloads directly to your local inference server with millisecond-accurate Time-To-First-Token (TTFT) tracking.
    
- **Systemd Log Streamer**: Real-time journal monitoring with keyword-based color highlighting for OOM errors, loading states, and inference activity.
    
- **Tactical Kill-Switch**: A dedicated emergency binding (`Ctrl+K`) to forcefully terminate rogue inference threads and clear ports.
    

## 🛠 Prerequisites

- **OS**: Linux (Optimized for Pop!_OS / Ubuntu).
    
- **Systemd**: Required for log streaming and service management.
    
- **NVIDIA Drivers**: Required for GPU telemetry (via `nvidia-smi`).
    
- **llama-router**: Recommended daemon for dynamic model serving.
    

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
    
    Because the program manages system services and performs port auditing, it requires `sudo` to function correctly.
    
    ```
    sudo saltnitor
    ```
    

## ⌨️ Quick Reference

|   |   |
|---|---|
|**Key**|**Action**|
|`q`|Quit Program|
|`t`|Open Config Tuner|
|`m`|Open Model Selector|
|`i`|Focus API Interrogator (Insert Mode)|
|`g`|Toggle GPU Inspector|
|`c`|Toggle CPU/System Inspector|
|`S/X/R`|Daemon Start / Stop / Restart|
|`Ctrl+K`|Tactical Kill-Switch (killall llama-server)|

## ⚠️ Important Notes

- **Elevated Environment**: If running via `sudo cargo run`, ensure your `PATH` is preserved or point directly to the compiled binary. `sudo` often hides user-installed tools like `cargo`.
    
- **Permission Transparency**: This tool uses `sudo -n` for background operations. You must launch the TUI itself with `sudo` to allow hotkeys like **Restart** and **Kill-Switch** to execute without interactive password prompts.
    
- **GPU Detection**: If no NVIDIA GPU is detected, Saltnitor will gracefully disable VRAM saturation gauges and the GPU Inspector while maintaining full CPU/RAM monitoring.
    

## 🛡 License

Distributed under the MIT License. See `LICENSE` for more information.
