# Saltnitor

**Saltnitor** is a high-performance, hardware-agnostic Terminal User Interface (TUI) built in Rust, serving as a central command center for orchestrating local Large Language Models (LLMs) and monitoring hybrid hardware pressure between GPU VRAM and System RAM.

Designed specifically for developers running `llama.cpp` on Linux, Saltnitor provides real-time deep telemetry, intelligent log analysis, and tactical control in a single, ultra-lightweight binary. It also exposes an **OpenAI-compatible control endpoint**, so any IDE or AI coding agent that speaks the OpenAI chat API (Pi, OpenCode, Cline, Aider, Continue, …) can drive **automatic model hot-swaps** just by addressing different models.

<img width="1920" height="1001" alt="image" src="https://github.com/user-attachments/assets/0b3bd3de-f3aa-4d23-be48-bc446c76a1f1" />


## 🚀 Key Features

- **Dynamic Hardware Probing & Telemetry**: Automatically identifies CPU architecture and NVIDIA GPU specifications on boot. Dynamically scales the UI to match your machine's thread count and memory limits, featuring real-time CPU load sparklines and precise VRAM/RAM saturation gauges.

- **Tactical Hardware Inspectors**:
    - **GPU Deep-Dive (`g`)**: Real-time VRAM allocation, core temperatures, wattage draw, and fan speeds. Includes an active process list to identify exactly which external applications are dominating your VRAM.
        
    - **CPU/System Deep-Dive (`c`)**: A balanced 60/40 UI split featuring inline `btop`-style gauges, a dynamic graphical equalizer showing load distribution across all physical/logical threads, and a deduplicated list of top RAM culprits.


- **Live Model Orchestration (Dual-Mode Bottom Deck)**:
    - **Auto-Tuning Hot-Swap**: Cycle available `.gguf` models dynamically. Features an intelligent **VRAM Oracle** that heuristically estimates the footprint of a model and warns you of potential OOM crashes before you execute the swap.
        
    - **Deep Engine Tuner (`t`)**: A paginated configuration manifest that generates a native Linux `router.env` file and executes a bash-wrapper translation to control the `llama-server` runtime on the fly:
        - *Page 1 (Compute & Memory)*: `ngl`, `ctx`, threads, micro-batching, parallel slots, Flash Attention, `mlock`, and exact KV Cache quantization algorithms (`q8_0`, `q4_0`, etc.).
        - *Page 2 (Context & Speculation)*: RoPE scaling, VRAM defragmentation thresholds, and Speculative Decoding targets (`-md`).
        - *Page 3 (Orchestration & Security)*: Core threading split (`-tb`), Continuous Batching, Context Shifting, and dynamic API Key authorization lock-downs.


- **Advanced API Interrogator (`i`)**: A built-in mini-console for firing test payloads directly to your local inference server.

    - **Granular Benchmarking**: Tracks millisecond-accurate Time-To-First-Token (TTFT) alongside precise, split Tokens-Per-Second (t/s) metrics for both **Prompt Evaluation** and **Generation**.
    - **Immune to Self-Lockout**: Dynamically injects Bearer Authentication tokens if the daemon's API Key security wall is engaged.
    - **Persistent Command History**: Bash-style history buffer with inline cursor editing, saved to `.saltnitor_history` on exit.

- **Tactical Incident Response**:
    - **Crash Dumping (`Ctrl+D`)**: Instantly export a post-mortem snapshot of your exact system state (VRAM/RAM pressure, temperatures, active model, and the last 100 log lines) to a timestamped file at `$HOME/saltnitor_crash_<timestamp>.txt`. The full path is printed to the log, and any write failure is reported (no more silent dumps to an unknown directory).
    - **Kill-Switch (`Ctrl+K`)**: A dedicated emergency binding that **stops the `llama-router` unit** (`systemctl stop`). Because the service runs with `Restart=always`, a plain process kill is respawned within seconds — stopping the unit is what actually frees VRAM and keeps it down until you restart it (`Shift+S`).


## 🛠 Prerequisites
- **OS**: Linux (Optimized for Pop!_OS / Ubuntu / Arch).
- **Systemd**: Required for log streaming and service management.
- **NVIDIA Drivers**: Required for GPU telemetry (via `nvidia-smi`).
- **llama.cpp**: A build whose `llama-server` supports the native router (`--models-preset`), orchestrated via the `launch_router.sh` bash wrapper.

## 📦 Installation & Setup

1. **Clone the Repository**
    ```bash
    git clone https://github.com/Saltless-bruh/saltnitor.git
    cd saltnitor
    ```
2. **Build for Release**
    ```bash
    cargo build --release
    ```
3. **Set up the router + hot-swap** — see [Hot-Swap Setup](#-hot-swap-setup-any-openai-compatible-ide--agent) below to create `router.ini`, `launch_router.sh`, and the `llama-router` service.
4. **Run Saltnitor** (as your normal user once the sudoers drop-in below is in place)
    ```bash
    ./target/release/saltnitor
    ```

## 🔄 Hot-Swap Setup (Any OpenAI-Compatible IDE / Agent)

The hot-swap is driven by `llama.cpp`'s native router. Saltnitor adds the VRAM oracle, the live TUI, and an OpenAI-compatible proxy in front of it. Setup is four small pieces.

### 1. Declare your models in `router.ini`
Each `[section]` **is the model id** that callers put in the `"model"` field. Keys are `llama-server` flags without the leading dashes; `[*]` holds global defaults. Section names are arbitrary — pick whatever you'll address from your IDE.

```ini
[*]                         # defaults applied to every model
flash-attn = on
threads    = 7
ctx-size   = 32768

[fast]                      # an agent requests  "model": "fast"
model = /home/you/models/qwen3-9b-Q5_K_XL.gguf
ngl   = 99

[deep]                      # an agent requests  "model": "deep"
model = /home/you/models/qwen3-30b-A3B-Q4_K_XL.gguf
ngl   = 99
override-tensor = .ffn_.*_exps.=CPU   # offload MoE experts to RAM
```

### 2. Launch the router via `launch_router.sh`
`--models-max 1` keeps exactly one model resident, so a request for a different id evicts the incumbent and loads the new one. `exec` makes `llama-server` the unit's main process so the Kill-Switch can actually stop it.

```bash
#!/usr/bin/env bash
set -euo pipefail
exec /usr/local/bin/llama-server \
  --models-preset /home/you/llama.cpp/router.ini \
  --models-max 1 --host 127.0.0.1 --port 8080
```

Run it under a systemd unit (`llama-router.service`, `User=<you>`, `Restart=always`, plus `KillSignal=SIGKILL` + `TimeoutStopSec=10` so the Kill-Switch is instant). Then grant your user passwordless control of just that service:

```sudoers
# /etc/sudoers.d/saltnitor   (visudo -f)
you ALL=(root) NOPASSWD: /usr/bin/systemctl start llama-router, \
  /usr/bin/systemctl stop llama-router, /usr/bin/systemctl restart llama-router
```

### 3. Tell Saltnitor about the models in `config.toml`
The control API reads this for the oracle. Profile keys **must match the `router.ini` section names**.

```toml
control_port = 8765
router_base  = "http://127.0.0.1:8080"
infer_bearer = "sk-saltnitor-2026"     # only needed if the router uses --api-key
reserve_vram_gb = 0.8
reserve_ram_gb  = 1.0

[profiles.fast]
model = "qwen3-9b-Q5_K_XL.gguf"
est_vram_gb = 9.0

[profiles.deep]
model = "qwen3-30b-A3B-Q4_K_XL.gguf"
offload = true
est_vram_gb = 9.0
est_ram_gb  = 18.0
```

### 4. Point your IDE/agent at an OpenAI-compatible base URL
Use your **section names** as the model ids. Two endpoints are available:

| Endpoint | URL | Behavior |
|---|---|---|
| **Through Saltnitor** (recommended) | `http://127.0.0.1:8765/v1` | Oracle-gated (refuses OOM loads); swap shown live in the TUI |
| **Straight to the router** | `http://127.0.0.1:8080/v1` | Router auto-swaps; no oracle gate or TUI indicator |

Set one agent/model to `fast` and another to `deep`, and **switching agents switches the model** — automatically. Ready-to-use configs for **Pi** and **OpenCode**, including a multi-agent "architect → scout" example that swaps models on delegation, are in [`integrations/`](./integrations/INTEGRATION.md).

## ⌨️ Quick Reference

| Key | Action |
|---|---|
| `q` | Quit Program |
| `h` | Open Interactive Command Manual |
| `PgUp / PgDn` | Scroll Log Streamer History |
| `t` | Open Deep Engine Tuner |
| `Tab` | Toggle Bottom Deck (Interrogator vs Hot-Swap) |
| `Enter` | Apply Tuner (write `router.ini` section + restart) / Fire Payload / Pin Model |
| `i` | Focus Active Bottom Deck (Insert Mode) |
| `Esc` | Exit Insert Mode |
| `Up / Down` | Cycle History / Sniper Targets |
| `g` | Toggle GPU Hardware Inspector |
| `c` | Toggle CPU/System Hardware Inspector |
| `Shift + S/X/R` | Daemon Start / Stop / Restart |
| `Ctrl+D` | Tactical Crash Dump (save state to `$HOME/saltnitor_crash_*.txt`) |
| `Ctrl+K` | Tactical Kill-Switch (`systemctl stop llama-router`) |

## ⚠️ Important Notes
- **Permission model**: Saltnitor uses `sudo -n` only for the three `systemctl` actions on `llama-router`. The recommended setup is the **sudoers drop-in** above, which lets you run the TUI as your normal user with no password prompts. Running the whole TUI with `sudo` also works but isn't necessary, and the service itself should run as your user (`User=<you>`), not root.
- **Model ids are your contract**: the id an agent sends must match a `router.ini` section name **and** a `[profiles.*]` key in `config.toml`. They are case-sensitive.
- **First call to a cold model is slower** (that's the load + warmup); subsequent calls are instant. Pre-warm with the control API's `/v1/ensure` if you want to hide it.
- **Terminal Sizing Guardrails**: Saltnitor requires a minimum terminal footprint of 80x16. If the window is resized below this threshold, rendering will halt to prevent mathematical panics.
