# Saltnitor

**Saltnitor** is a Terminal User Interface (TUI) built in Rust for Linux with NVIDIA GPUs, serving as a command center for orchestrating local Large Language Models (LLMs) and monitoring hybrid hardware pressure between GPU VRAM and System RAM.

Designed for developers running `llama.cpp` on Linux, Saltnitor provides hardware telemetry, log viewing and service control in a single binary. It also exposes an **OpenAI-compatible control endpoint**, so any IDE or AI coding agent that speaks the OpenAI chat API (Pi, OpenCode, Cline, Aider, Continue, …) can drive **automatic model hot-swaps** just by addressing different models.

<img width="1920" height="1001" alt="image" src="https://github.com/user-attachments/assets/0b3bd3de-f3aa-4d23-be48-bc446c76a1f1" />


## 🚀 Key Features

- **Hardware Telemetry**: Reads CPU and NVIDIA GPU specifications on boot (via `sysinfo` and `nvidia-smi`) and shows CPU load sparklines and VRAM/RAM saturation gauges. Linux and NVIDIA only.

- **Hardware Inspectors**:
    - **GPU Inspector (`g`)**: VRAM allocation, temperature, power draw and fan speed, plus the compute processes using VRAM (PID, user, RAM, VRAM).

    - **CPU/System Inspector (`c`)**: Inline gauges, per-thread load bars and the top RAM processes (PID, user, RAM, VRAM).

    - **Process control (`x` / `X`)**: In either inspector, `x` sends SIGTERM and escalates after `[process] term_grace_ms`; `X` asks `y/N` and then sends SIGKILL. Both act on the **exact PID** you selected (checked against its start time and owner, signalled through a pidfd). Nothing is ever killed by name, and PID 1, Saltnitor itself, the llama-server runtime processes and processes owned by other users are refused with a reason.

- **Live Model Orchestration (Dual-Mode Bottom Deck)**:
    - **Hot-Swap**: Cycle the available `.gguf` models. A **VRAM oracle** estimates the footprint (from the file name, or from `est_vram_gb` / `est_ram_gb` in the profile) and warns before you swap. It is an estimate: it does not yet account for VRAM held by other processes.

    - **Router tuner (`t`)**: A paginated editor that writes the tuned flags into the active model's `[section]` of the `router.ini` named by `router_ini` in `config.toml`, then restarts the unit. Without `router_ini` the tuner refuses to apply. Only these keys are written: `ngl`, `ctx-size`, `batch-size`, `ubatch-size`, `threads`, `threads-batch`, `parallel`, `flash-attn`, `cache-type-k`, `cache-type-v`, `cont-batching`, `rope-freq-base`, `rope-freq-scale`, `defrag-thold`, `mlock`, `no-mmap`.
        - *Page 1 (Compute & Memory)*: `ngl`, `ctx`, threads, batch, parallel slots, Flash Attention, `mlock`, `no_mmap`, KV cache types.
        - *Page 2 (Context & Speculation)*: RoPE scaling, defrag threshold, and the draft-model controls. The draft-model controls are displayed but **not yet written** to `router.ini` (BD-17).
        - *Page 3 (Orchestration)*: threads per batch, u-batch, continuous batching, plus context-shift, metrics and API Key toggles. Context shift and metrics are **not yet written** (BD-17). The API Key toggle only decides whether the interrogator sends the client key; it does not change the router.

- **API Interrogator (`i`)**: A mini-console that sends a request through Saltnitor's own endpoint (`http://127.0.0.1:<control_port>/v1/chat/completions`) with the client key.
    - **Metrics**: Client-side time-to-first-token, and prompt / generation tokens-per-second taken from the runtime's `timings`. When the runtime sends no timings the rate is shown as an estimate (`est.`) or `n/a`, never as `0`.
    - **Client key**: Set `client_key_env` to the name of an environment variable holding the key; the interrogator sends it as a Bearer token.
    - **History**: The last 10 commands are saved on exit to `$XDG_STATE_HOME/saltnitor/history`, or `~/.local/state/saltnitor/history` when `XDG_STATE_HOME` is unset.

- **Incident Response**:
    - **Crash Dump (`Ctrl+D`)**: Exports system state (VRAM/RAM pressure, temperatures, active model, last 100 log lines) to `$HOME/saltnitor_crash_<timestamp>.txt`. The path is printed to the log and write failures are reported. Configured tokens are redacted from the dump and from logs.
    - **Kill-Switch (`Ctrl+K`)**: Runs `sudo -n systemctl stop <service>`. Because the service runs with `Restart=always`, a plain process kill is respawned within seconds; stopping the unit is what frees VRAM and keeps it down until you restart it (`Shift+S`).


## 🛠 Prerequisites
- **OS**: Linux (Optimized for Pop!_OS / Ubuntu / Arch).
- **Systemd**: Required for log streaming and service management.
- **NVIDIA Drivers**: Required for GPU telemetry (via `nvidia-smi`).
- **llama.cpp**: A build whose `llama-server` supports the native router (`--models-preset`), orchestrated via the `launch_router.sh` bash wrapper (example in `examples/external-mode/`).

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
Saltnitor reads `--config <path>`, else `$XDG_CONFIG_HOME/saltnitor/config.toml`, else `~/.config/saltnitor/config.toml`. The file is **strict**: a syntax error, a wrong type or an unknown key makes Saltnitor print the file, line, key and expectation and **exit with status 2**. It never falls back to defaults. CLI flags override the file. Profile keys **must match the `router.ini` section names**.

```toml
control_port = 8765
router_base  = "http://127.0.0.1:8080"
router_ini   = "/home/you/llama.cpp/router.ini"   # required for the tuner (Enter) to write anything
infer_bearer = "<router api-key if the router runs with --api-key>"   # only if the router uses --api-key
reserve_vram_gb = 0.8
reserve_ram_gb  = 1.0

# Secrets: prefer an environment variable or a private file over a literal in this file.
control_token_env  = "SALTNITOR_TOKEN"            # name of the env var holding the control token ...
# control_token_file = "/home/you/.config/saltnitor/token"   # ... or a file (mode 0600, owned by you)
client_key_env     = "SALTNITOR_CLIENT_KEY"       # env var the interrogator sends as its Bearer key
allow_query_token  = false                        # default; true re-enables ?token= on GET /v1/ensure/stream only
max_body_bytes     = 33554432                     # request body limit (default 32 MiB)

[timeouts]            # proxy to llama-server, in milliseconds (defaults shown)
connect_ms    = 5000
first_byte_ms = 600000
idle_ms       = 120000   # max silence between streamed chunks

[process]
term_grace_ms = 5000     # SIGTERM -> escalation window for the `x` key

[profiles.fast]
model = "qwen3-9b-Q5_K_XL.gguf"
est_vram_gb = 9.0

[profiles.deep]
model = "qwen3-30b-A3B-Q4_K_XL.gguf"
offload = true
est_vram_gb = 9.0
est_ram_gb  = 18.0
```

Authentication: when a control token is configured, **every `/v1` route requires `Authorization: Bearer <token>`**. Failures are JSON error envelopes. Client credentials are never forwarded to llama-server (use `infer_bearer` for that). With no token configured the API is open on loopback only. See [SECURITY.md](./SECURITY.md).

### 4. Point your IDE/agent at an OpenAI-compatible base URL
Use your **section names** as the model ids. Two endpoints are available:

| Endpoint | URL | Behavior |
|---|---|---|
| **Through Saltnitor** (recommended) | `http://127.0.0.1:8765/v1` | Oracle-gated (refuses loads it estimates will not fit); authenticated; responses are streamed through chunk by chunk; swap shown live in the TUI |
| **Straight to the router** | `http://127.0.0.1:8080/v1` | Router auto-swaps; no oracle gate or TUI indicator |

Set one agent/model to `fast` and another to `deep`, and **switching agents switches the model** — automatically. Point any OpenAI-compatible client at the base URL above and set its API key to your control token.

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
| `Up / Down` | Cycle History / Inspector Process Selection |
| `g` | Toggle GPU Hardware Inspector |
| `c` | Toggle CPU/System Hardware Inspector |
| `x` / `X` (in an inspector) | Terminate (SIGTERM, then escalate) / Kill (SIGKILL, `y/N` confirm) the selected PID |
| `Shift + S/X/R` | Daemon Start / Stop / Restart (outside an inspector) |
| `Ctrl+D` | Tactical Crash Dump (save state to `$HOME/saltnitor_crash_*.txt`) |
| `Ctrl+K` | Tactical Kill-Switch (`systemctl stop llama-router`) |

## ⚠️ Important Notes
- **Permission model**: Saltnitor uses `sudo -n` only for the three `systemctl` actions on the configured service (`llama-router` by default). The recommended setup is the **sudoers drop-in** above, which lets you run the TUI as your normal user with no password prompts. Running the whole TUI with `sudo` also works but isn't necessary, and the service itself should run as your user (`User=<you>`), not root.
- **Model ids are your contract**: the id an agent sends must match a `router.ini` section name **and** a `[profiles.*]` key in `config.toml`. They are case-sensitive.
- **First call to a cold model is slower** (the load plus a one-token warm-up); later calls to the resident model skip it. Pre-warm with the control API's `/v1/ensure` if you want to hide it.
- **Terminal Sizing Guardrails**: Saltnitor requires a minimum terminal footprint of 80x16. If the window is resized below this threshold, rendering will halt to prevent mathematical panics.
