# SALTNITOR vNext — Upgrade & Modernization Blueprint

**Repository:** `saltless-bruh/saltnitor`  
**Document type:** Engineering blueprint / implementation specification  
**Primary target:** Saltnitor vNext  
**Primary runtime family:** `llama.cpp` and compatible forks  
**Flagship validation runtime:** `thecodacus/llama.cpp` (`perf`)  
**Flagship validation model:** Qwen3.6-35B-A3B, Q4_K_M / UD-Q4_K_M  
**Reference host:** Pop!_OS, Ryzen 7 7700, 32 GB DDR5, RTX 3060 12 GB  
**Status:** Proposed

---

## 1. Purpose

Saltnitor should evolve from a monolithic llama.cpp monitoring/orchestration TUI into a **hardware-aware runtime control plane, tuning laboratory, benchmark harness, and remote model gateway**.

The project should keep its current strengths:

- terminal-first operation,
- low overhead,
- direct visibility into CPU, RAM, GPU, VRAM, model state, and logs,
- llama.cpp-native behavior rather than hiding the runtime,
- hot model switching,
- OpenAI-compatible access,
- runtime tuning,
- crash/recovery controls.

The upgrade should not turn Saltnitor into another inference engine or generic desktop chat application. `llama.cpp` remains responsible for inference. Saltnitor becomes the system that makes llama.cpp **observable, reproducible, safe to tune, and practical to operate remotely**.

### Target identity

> **Saltnitor is a hardware-aware runtime control plane and tuning laboratory for local llama.cpp inference. It manages model residency, monitors host pressure, benchmarks runtime configurations, and exposes local models to OpenAI-compatible clients and coding agents.**

---

# 2. Scope

This blueprint covers:

1. core architectural hardening;
2. daemon/TUI separation;
3. runtime backend abstraction;
4. configuration redesign;
5. hardware telemetry modernization;
6. VRAM/RAM admission control;
7. OpenAI-compatible proxy correctness;
8. model residency and request scheduling;
9. reproducible benchmark infrastructure;
10. Codacus/Qwen MoE integration;
11. agent-oriented remote serving;
12. test strategy and CI;
13. migration from the current repository;
14. release milestones and acceptance criteria.

## Non-goals

Saltnitor should **not** initially attempt to:

- replace llama.cpp;
- implement its own tensor runtime;
- become an Ollama-compatible model registry;
- implement a full web chat UI;
- become a general Kubernetes-style distributed inference scheduler;
- manage arbitrary cloud providers;
- duplicate every llama.cpp feature;
- hard-code itself around Qwen or Codacus.

Qwen/Codacus is the flagship workload used to prove the architecture, not the permanent boundary of the project.

---

# 3. Current Baseline

The current repository already contains valuable building blocks:

- Rust + Tokio async runtime;
- Ratatui/Crossterm TUI;
- CPU/RAM telemetry using `sysinfo`;
- NVIDIA telemetry via `nvidia-smi`;
- systemd integration;
- live journal logging;
- llama.cpp native `--models-preset` routing;
- model hot-swap;
- model footprint oracle;
- OpenAI-compatible `/v1/models` and `/v1/chat/completions` control API;
- `/v1/ensure` and SSE load-progress API;
- API interrogator;
- TTFT, prompt-evaluation and generation speed measurements;
- a three-page runtime tuner;
- crash dumps and emergency stop controls.

The project should be upgraded in place rather than rewritten.

---

# 4. Major Problems to Resolve First

These are **P0** issues. New Qwen/Codacus functionality should not be considered production-ready until they are resolved.

## 4.1 Monolithic application structure

`main.rs` currently owns too many unrelated responsibilities:

- configuration;
- startup validation;
- telemetry;
- model discovery;
- port auditing;
- journal streaming;
- key handling;
- process control;
- tuner state mutation;
- hot-swap behavior;
- service management.

This creates high coupling and will become difficult to maintain once runtime backends, MoE profiling, benchmark persistence, remote clients, and request scheduling are added.

### Requirement

`main.rs` should become a thin bootstrap layer.

---

## 4.2 OpenAI streaming proxy must stream for real

The current proxy reads the upstream response body to completion before returning it.

For `stream: true`, this defeats:

- incremental token delivery;
- time-to-first-token;
- streaming tool calls;
- responsive coding agents;
- realistic remote usage.

### Requirement

Proxy the upstream byte stream incrementally.

Desired flow:

```text
llama-server token/event
        ↓
Saltnitor receives chunk
        ↓
Saltnitor immediately forwards chunk
        ↓
OpenCode / client receives chunk
```

Saltnitor must not buffer the complete SSE response unless explicitly requested.

---

## 4.3 Authentication must be centralized

Authentication must not be selectively implemented in individual handlers.

### Requirement

Introduce API authentication middleware.

Suggested policy:

```text
/healthz                    optionally public
/v1/*                       authenticated
/admin/*                    authenticated + admin scope
/local/*                    local-only
```

At minimum:

- `/v1/models`
- `/v1/status`
- `/v1/chat/completions`
- `/v1/ensure`
- `/v1/ensure/stream`

must follow one explicit policy.

Do not rely on “it is localhost only” once remote Tailscale access is introduced.

---

## 4.4 Avoid bearer tokens in query strings

`?token=...` is convenient for browser `EventSource`, but query parameters are easily captured in:

- logs;
- shell history;
- reverse-proxy access logs;
- screenshots;
- diagnostics.

### Requirement

Prefer header-authenticated streaming using fetch/chunked responses or authenticated POST-based streaming.

If query-token compatibility remains, it must be opt-in and clearly marked as compatibility mode.

---

## 4.5 Strict configuration validation

Malformed TOML should never silently result in a default configuration.

### Requirement

Startup behavior:

```text
valid config
    → start

missing optional config
    → default allowed

malformed config
    → fail startup with precise diagnostic
```

Example:

```text
config.toml:42
profiles.qwen36.ctx_size
expected integer, found string "32768k"
```

Configuration should be versioned.

Example:

```toml
schema_version = 2
```

---

## 4.6 Process control must use PID, not `killall`

A process management UI must target an exact process.

### Replace

```text
killall -9 python
```

with:

```text
PID 18472
SIGTERM
wait N seconds
optional SIGKILL
```

Process objects should include:

```rust
struct ProcessInfo {
    pid: u32,
    name: String,
    memory_bytes: u64,
    gpu_memory_bytes: Option<u64>,
    command: Option<String>,
}
```

Emergency hard-kill should be a separate explicit action.

---

## 4.7 Reproducible builds

For an executable/daemon project, commit `Cargo.lock`.

CI and releases should use the lockfile.

---

# 5. Target Architecture

```text
                         ┌───────────────────────┐
                         │       Clients         │
                         │ OpenCode / DSH / CLI  │
                         └───────────┬───────────┘
                                     │
                              OpenAI-compatible
                                     │
                                     ▼
┌────────────────────────────────────────────────────────────────┐
│                         SALTNITORD                              │
│                                                                │
│  ┌──────────────┐   ┌────────────────┐   ┌─────────────────┐  │
│  │ API / Proxy  │   │ Model Scheduler│   │ Runtime Manager │  │
│  └──────┬───────┘   └───────┬────────┘   └────────┬────────┘  │
│         │                   │                     │           │
│         └───────────────────┼─────────────────────┘           │
│                             │                                 │
│          ┌──────────────────┼──────────────────┐              │
│          │                  │                  │              │
│          ▼                  ▼                  ▼              │
│      Telemetry           Oracle           Benchmark Lab       │
│          │                  │                  │              │
│          └──────────────────┼──────────────────┘              │
│                             │                                 │
└─────────────────────────────┼─────────────────────────────────┘
                              │
                         RuntimeBackend
                              │
              ┌───────────────┴────────────────┐
              │                                │
              ▼                                ▼
      upstream llama.cpp               Codacus llama.cpp
              │                                │
              └───────────────┬────────────────┘
                              ▼
                         llama-server
                              │
                              ▼
                            GGUF
```

The TUI becomes a client of the daemon rather than the owner of the daemon lifecycle.

---

# 6. Daemon/TUI Separation

## 6.1 Saltnitor daemon

The daemon owns:

- runtime processes;
- model state;
- OpenAI-compatible API;
- model load/swap scheduling;
- telemetry collection;
- benchmark execution;
- persistent history;
- admission control;
- service health;
- runtime capability detection.

Command:

```bash
saltnitor daemon
```

or optionally a dedicated binary:

```bash
saltnitord
```

## 6.2 TUI

The TUI becomes an operator client.

Command:

```bash
saltnitor tui
```

It connects to the local daemon over:

1. Unix domain socket preferred for local administration;
2. loopback HTTP as fallback.

The TUI should not need root privileges.

## 6.3 CLI

Add non-interactive commands:

```bash
saltnitor status
saltnitor runtime list
saltnitor runtime use codacus-stable
saltnitor model list
saltnitor profile list
saltnitor profile apply qwen36-agent
saltnitor bench run qwen36-agent
saltnitor bench compare RUN_A RUN_B
saltnitor trace qwen36-agent
saltnitor service restart
```

This makes Saltnitor usable from:

- scripts;
- SSH;
- CI;
- agent tools;
- system automation.

---

# 7. Proposed Rust Module Layout

```text
src/
├── main.rs
├── lib.rs
│
├── config/
│   ├── mod.rs
│   ├── schema.rs
│   ├── loader.rs
│   └── validation.rs
│
├── daemon/
│   ├── mod.rs
│   ├── server.rs
│   └── state.rs
│
├── runtime/
│   ├── mod.rs
│   ├── backend.rs
│   ├── capability.rs
│   ├── process.rs
│   ├── llama_cpp.rs
│   └── codacus.rs
│
├── model/
│   ├── mod.rs
│   ├── profile.rs
│   ├── gguf.rs
│   ├── residency.rs
│   └── oracle.rs
│
├── scheduler/
│   ├── mod.rs
│   ├── queue.rs
│   └── lease.rs
│
├── telemetry/
│   ├── mod.rs
│   ├── system.rs
│   ├── gpu.rs
│   ├── nvml.rs
│   ├── nvidia_smi.rs
│   └── llama_metrics.rs
│
├── proxy/
│   ├── mod.rs
│   ├── routes.rs
│   ├── openai.rs
│   ├── streaming.rs
│   ├── auth.rs
│   └── health.rs
│
├── benchmark/
│   ├── mod.rs
│   ├── runner.rs
│   ├── workloads.rs
│   ├── result.rs
│   ├── storage.rs
│   └── compare.rs
│
├── service/
│   ├── mod.rs
│   └── systemd.rs
│
├── tui/
│   ├── mod.rs
│   ├── app.rs
│   ├── events.rs
│   ├── state.rs
│   ├── screens/
│   │   ├── overview.rs
│   │   ├── runtime.rs
│   │   ├── tuner.rs
│   │   ├── benchmark.rs
│   │   ├── logs.rs
│   │   └── processes.rs
│   └── widgets/
│
└── error.rs
```

Do not force this exact tree if implementation evidence suggests a simpler split. The requirement is separation of responsibility, not directory ceremony.

---

# 8. Runtime Backend Abstraction

## 8.1 RuntimeBackend

Introduce a runtime interface.

Conceptual API:

```rust
trait RuntimeBackend {
    fn id(&self) -> &str;
    fn executable(&self) -> &Path;
    fn fingerprint(&self) -> Result<RuntimeFingerprint>;
    fn capabilities(&self) -> Result<RuntimeCapabilities>;
    fn validate_profile(&self, profile: &ModelProfile) -> Result<()>;
    async fn start(&self, profile: &ModelProfile) -> Result<RuntimeHandle>;
    async fn stop(&self) -> Result<()>;
    async fn health(&self) -> Result<RuntimeHealth>;
}
```

The exact Rust async trait strategy may differ.

## 8.2 Runtime capability discovery

Saltnitor must not infer features solely from the runtime name.

Example capabilities:

```rust
struct RuntimeCapabilities {
    native_router: bool,
    metrics: bool,
    cpu_moe: bool,
    moe_cache: bool,
    moe_trace: bool,
    mtp: bool,
    turboquant_kv: bool,
    async_cpu_split: bool,
    model_load_api: bool,
}
```

Discovery can combine:

- `--help` probing;
- `--version`;
- executable presence;
- known API feature probes;
- optional manually declared overrides.

The UI displays only supported controls.

## 8.3 Runtime fingerprint

Every benchmark and crash report should record:

```text
runtime name
repository
branch
commit SHA
binary hash
build date
compiler
CUDA version
feature capabilities
```

This is essential because llama.cpp and experimental forks evolve rapidly.

---

# 9. Runtime Channels

Recommended local layout:

```text
~/ai-runtimes/
├── upstream/
├── codacus-stable/
└── codacus-testing/
```

Saltnitor should not automatically `git pull` its active production runtime.

Upgrade flow:

```text
new runtime commit
       ↓
testing channel
       ↓
build
       ↓
capability validation
       ↓
regression suite
       ↓
benchmark comparison
       ↓
promote to stable
```

Promotion should be explicit.

---

# 10. Configuration Redesign

Suggested high-level configuration:

```toml
schema_version = 2

[daemon]
bind = "127.0.0.1"
port = 8765

[security]
api_key_env = "SALTNITOR_API_KEY"

[storage]
data_dir = "~/.local/share/saltnitor"

[runtimes.upstream]
server = "/home/user/ai-runtimes/upstream/build/bin/llama-server"
bench = "/home/user/ai-runtimes/upstream/build/bin/llama-bench"

[runtimes.codacus-stable]
server = "/home/user/ai-runtimes/codacus-stable/build/bin/llama-server"
bench = "/home/user/ai-runtimes/codacus-stable/build/bin/llama-bench"
moe_trace = "/home/user/ai-runtimes/codacus-stable/build/bin/llama-moe-trace"

[runtimes.codacus-stable.env]
GGML_CUDA_REGISTER_HOST = "1"
GGML_SCHED_PREFETCH_EXPERTS = "1"

[profiles.qwen36-agent]
runtime = "codacus-stable"
model = "/home/user/models/Qwen3.6-35B-A3B-UD-Q4_K_M.gguf"
ctx_size = 32768
ngl = 99
flash_attention = true
```

Separate:

- runtime configuration;
- model profile configuration;
- global daemon configuration;
- secrets;
- generated benchmark data.

---

# 11. Telemetry Redesign

## 11.1 Provider architecture

```text
TelemetryManager
    │
    ├── SystemProvider
    ├── GpuProvider
    └── RuntimeMetricsProvider
```

## 11.2 NVIDIA

Prefer direct NVML access.

Fallback:

```text
NVML unavailable
      ↓
nvidia-smi provider
```

Do not make subprocess parsing the primary long-running monitoring path.

## 11.3 Runtime metrics

Where llama.cpp exposes native metrics, consume them rather than re-parsing log strings.

Combine:

```text
llama.cpp metrics
+
NVML
+
sysinfo
+
Saltnitor state
```

## 11.4 Sampling

Different metrics do not need identical frequency.

Example:

```text
GPU utilization          500 ms
VRAM                     500 ms
CPU                      1 s
RAM                      1 s
process table             2 s
runtime version          startup only
static hardware          startup only
```

---

# 12. VRAM/RAM Oracle v2

The current filename/parameter-count heuristic should become only the fallback predictor.

## 12.1 Prediction hierarchy

Preferred order:

```text
1. exact observed configuration
2. nearest historical configuration
3. GGUF metadata + runtime model
4. filename heuristic
```

## 12.2 Configuration identity

Calculate a configuration hash using:

```text
model file hash
runtime fingerprint
context
batch
ubatch
KV type
GPU layers
CPU-MoE
expert cache slots
MTP
parallel slots
other memory-relevant arguments
```

## 12.3 Observed profile

Persist:

```json
{
  "configuration_hash": "...",
  "peak_vram_mb": 11284,
  "peak_ram_mb": 25792,
  "startup_vram_mb": 10220,
  "context_vram_mb": 10600,
  "stable": true
}
```

## 12.4 Admission calculation

Do not assume total VRAM is fully available.

Account for:

```text
total VRAM
- display/driver usage
- external GPU processes
- safety reserve
= allocatable budget
```

Likewise for system RAM.

## 12.5 Confidence

Oracle decisions should expose confidence:

```text
EXACT_OBSERVED
INTERPOLATED
GGUF_ESTIMATE
HEURISTIC
UNKNOWN
```

UI example:

```text
Predicted VRAM: 10.9 GB
Available:      11.4 GB
Headroom:        0.5 GB
Confidence:      OBSERVED
Risk:            MEDIUM
```

---

# 13. Model Residency and Scheduling

Raw `--models-max 1` routing is not enough for safe multi-client behavior.

## 13.1 Problem

Client A may be using Model A while Client B requests Model B.

An immediate swap may:

- interrupt A;
- thrash A → B → A;
- multiply model load delays;
- create unpredictable agent failures.

## 13.2 Add model lease state

```text
Model A resident
active_requests = 2

request Model B arrives
       ↓
queue B
       ↓
wait active_requests == 0
       ↓
swap
       ↓
serve B
```

## 13.3 Policy modes

Support:

```text
queue          default
reject         return MODEL_BUSY
force          admin-only
```

## 13.4 Request identity

Track:

```text
request ID
client
model
start time
stream/non-stream
tokens
status
```

This later enables meaningful operational metrics.

---

# 14. OpenAI-Compatible Proxy v2

Minimum routes:

```text
GET  /healthz
GET  /v1/models
GET  /v1/status
POST /v1/chat/completions
POST /v1/ensure
```

Future:

```text
POST /v1/responses
POST /v1/embeddings  only if underlying runtime/profile supports it
```

## Requirements

- exact streaming preservation;
- request cancellation propagation;
- response status/header preservation;
- model admission before forwarding;
- structured error responses;
- authentication middleware;
- request IDs;
- metrics;
- timeout policy;
- graceful shutdown.

Do not rewrite model output.

Saltnitor is a transport/control plane, not an LLM response post-processor.

---

# 15. Tailscale / Remote Operation

Default:

```text
llama-server:
127.0.0.1:8080

Saltnitor:
127.0.0.1:8765
```

Remote mode:

```text
llama-server:
127.0.0.1:8080

Saltnitor:
<Tailscale interface>:8765
```

Saltnitor is the only externally reachable inference endpoint.

Do not bind raw llama-server publicly by default.

Add config:

```toml
[remote]
mode = "tailscale"
```

Saltnitor may detect the Tailscale address but must never silently expose itself.

Remote exposure requires explicit configuration.

---

# 16. Codacus / Qwen MoE Integration

Codacus support should be implemented as runtime capabilities, not hard-coded project identity.

## 16.1 Process-level features

Examples:

```text
GGML_CUDA_REGISTER_HOST
GGML_SCHED_PREFETCH_EXPERTS
```

Changing these requires a runtime restart.

## 16.2 Model-level features

Examples:

```text
n-cpu-moe
moe-cache-profile
moe-cache-slots
sched-async-cpu
spec-type=draft-mtp
KV cache types
```

Store these in model/tuning profiles.

## 16.3 MoE tuner screen

Add a dedicated page:

```text
MoE / EXPERT ENGINE

CPU MoE Layers
Host Registration
Expert Prefetch
MoE Cache
Cache Profile
Cache Slots
Async CPU Split
MTP
KV Compression
```

Unsupported features are hidden or disabled based on runtime capabilities.

---

# 17. `llama-moe-trace` Integration

Add first-class profiling.

Example flow:

```text
select model
   ↓
select workloads
   ↓
run code trace
   ↓
run reasoning trace
   ↓
run tool-use trace
   ↓
merge routing data
   ↓
save model profile
```

Storage:

```text
~/.local/share/saltnitor/moe-profiles/
```

Each profile should record:

```text
model hash
runtime SHA
creation time
workloads
number of generated tokens
profile path
```

---

# 18. Automatic Expert Cache Slot Tuning

Implement controlled search.

Example:

```text
64 → stable
80 → stable
96 → OOM
88 → stable
92 → unstable at 32K
90 → stable
```

Production recommendation:

```text
safe maximum = measured stable maximum - configured reserve
```

The tuner must test more than model load.

A candidate passes only if it survives a configured workload.

Example validation:

```text
load
warmup
8K prompt
32K prompt
generation
cooldown
```

---

# 19. Benchmark Laboratory

Benchmarking should become a first-class subsystem rather than only a TUI interrogator.

## 19.1 Benchmark classes

### A. Engine microbenchmark

Examples:

```text
PP512
PP2048
PP8192
TG128
TG512
```

### B. Controlled inference workload

Examples:

- code comprehension;
- bug diagnosis;
- tool-call selection;
- long-context code;
- reasoning.

### C. Agent benchmark

Run through an actual client or replay a captured agent workload.

Track:

```text
wall-clock task time
number of inference turns
tool calls
failed tool calls
prompt tokens
completion tokens
compactions
tests passed/failed
model reloads
```

## 19.2 Result storage

Prefer SQLite once benchmark volume grows.

Possible tables:

```text
runtime
model
profile
benchmark_run
metric
workload
crash
```

JSON export remains supported.

## 19.3 Benchmark identity

Every run records:

```text
Saltnitor version
runtime SHA
model hash
quant
profile
hardware
driver
CUDA
OS/kernel
timestamp
```

---

# 20. Comparison Engine

Provide:

```bash
saltnitor bench compare RUN_101 RUN_133
```

Metrics:

```text
prompt t/s
generation t/s
TTFT
peak VRAM
peak RAM
power
GPU utilization
wall time
stability
```

TUI view:

```text
PROFILE             PP/s    TG/s    TTFT   VRAM   RAM   PASS
baseline            1140    42.0    1.90   10.1   22.4   ✓
host-pin            1510    42.1    1.45   10.1   22.4   ✓
prefetch            1840    42.1    1.12   10.1   22.4   ✓
cache-88            1910    59.8    1.08   10.9   22.0   ✓
cache-88-mtp        1935    71.4    1.04   11.2   22.0   ✓
```

Never invent benchmark values. All UI metrics must come from actual run records.

---

# 21. TUI Redesign

Keep the TUI identity.

Suggested top-level screens:

```text
F1 Overview
F2 Runtime
F3 Models
F4 Tuner
F5 Benchmark
F6 Processes
F7 Logs
F8 Remote/API
```

## Overview

Show only operational essentials:

```text
daemon state
runtime
active model
VRAM/RAM
GPU utilization
PP/TG
TTFT
active requests
queued requests
```

## Runtime

```text
backend
branch
SHA
features
binary
CUDA
health
```

## Tuner

Split into logical pages:

```text
Compute
Context/KV
MoE
Speculation
Server
```

## Benchmark

Allow:

```text
run
compare
history
promote profile
```

---

# 22. Service Management

Saltnitor daemon should own runtime lifecycle directly where possible.

systemd can supervise Saltnitor itself:

```text
saltnitor.service
```

Saltnitor then supervises llama-server as a child process.

Benefits:

- runtime binary switching;
- environment switching;
- direct PID ownership;
- better logs;
- easier restart;
- no shell wrapper required;
- simpler stable/testing runtime changes.

Alternative mode may continue to support external systemd-managed llama-server.

Define both:

```text
runtime_process_mode = "managed"
runtime_process_mode = "external"
```

Recommended default: `managed`.

---

# 23. Error Model

Introduce structured errors.

Examples:

```text
CONFIG_INVALID
RUNTIME_NOT_FOUND
RUNTIME_CAPABILITY_MISSING
MODEL_NOT_FOUND
MODEL_BUSY
ORACLE_REJECTED
RUNTIME_START_FAILED
RUNTIME_UNHEALTHY
UPSTREAM_TIMEOUT
UPSTREAM_STREAM_ABORTED
GPU_OOM
BENCHMARK_FAILED
```

API response example:

```json
{
  "error": {
    "code": "ORACLE_REJECTED",
    "message": "Predicted VRAM use exceeds allocatable budget",
    "details": {
      "required_mb": 11620,
      "available_mb": 11102,
      "confidence": "OBSERVED"
    }
  }
}
```

---

# 24. Repository Hygiene

Remove runtime artifacts from source control.

Ignore:

```text
.saltnitor_history
saltnitor_crash_*.txt
*.trace.csv
benchmark-results/
*.local.toml
```

Remove `legacy.zip` unless it intentionally contains unique source not represented in Git history.

Commit `Cargo.lock`.

Recommended root:

```text
Cargo.toml
Cargo.lock
README.md
LICENSE
CHANGELOG.md
CONTRIBUTING.md
SECURITY.md
docs/
examples/
src/
tests/
.github/
```

---

# 25. CI

Minimum CI gates:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo build --release --locked
```

Add protocol tests using a fake upstream server.

Hardware-dependent tests should not be required on GitHub-hosted CI.

Optional local suite:

```bash
cargo test --features hardware-tests -- --ignored
```

---

# 26. Test Strategy

## Unit tests

Must cover:

- config validation;
- path expansion;
- runtime capability parsing;
- INI/profile mutation;
- Oracle calculations;
- process selection;
- benchmark serialization;
- model state machine;
- scheduler behavior.

## Protocol tests

Mock llama-server.

Verify:

- `/v1/models`;
- normal chat completion;
- SSE forwarding;
- cancellation;
- upstream 4xx/5xx;
- malformed upstream response;
- authentication;
- request IDs;
- timeout.

## Residency tests

Simulate:

```text
A active
B requested
B queued
A completes
B loads
```

## Regression tests

Maintain fixtures for known runtime `--help` output so capability parsing does not silently break.

## Hardware acceptance tests

Run on real RTX 3060 system:

```text
baseline inference
model load/unload
32K prompt
MoE cache
MTP
remote client
long-running daemon
```

---

# 27. Migration Plan

Do not perform a giant rewrite.

## Phase 0 — Freeze behavior

Before moving code:

- document current commands;
- capture current screenshots;
- save a known-good router configuration;
- record baseline tests;
- record current API behavior.

## Phase 1 — P0 correctness

Implement:

- true streaming proxy;
- auth middleware;
- strict config;
- PID process control;
- Cargo.lock;
- CI hardening;
- repo cleanup.

No major feature changes.

## Phase 2 — Extract modules

Move behavior out of `main.rs`.

Goal:

```text
same observable behavior
different internal structure
```

## Phase 3 — Daemon/TUI split

Move state ownership into daemon.

TUI consumes daemon state.

## Phase 4 — Runtime abstraction

Add upstream llama.cpp as Backend 1.

Prove no behavior regression.

Add Codacus as Backend 2.

## Phase 5 — Telemetry + Oracle v2

Add NVML provider.

Add observed memory records.

## Phase 6 — Benchmark lab

Add persistent runs and comparison.

## Phase 7 — Codacus MoE lab

Add:

- host pinning;
- prefetch;
- trace;
- expert cache;
- slot tuning;
- async CPU;
- MTP.

## Phase 8 — Remote agent serving

Add Tailscale-safe binding.

Validate with OpenCode and DeepSeek Harness.

---

# 28. Version Roadmap

Suggested:

## v0.2 — Hardening

- streaming fixed;
- auth middleware;
- strict config;
- PID process management;
- tests/CI;
- repository cleanup.

## v0.3 — Daemon Architecture

- headless daemon;
- TUI client;
- CLI;
- managed runtime process.

## v0.4 — Runtime Backends

- upstream backend;
- runtime capabilities;
- runtime fingerprint;
- stable/testing channels;
- Codacus backend.

## v0.5 — Tuning Laboratory

- benchmark database;
- comparison UI;
- Oracle v2;
- MoE profiler;
- cache slot tuner;
- MTP support.

## v0.6 — Remote Agent Server

- Tailscale mode;
- request scheduler;
- authenticated OpenAI gateway;
- OpenCode validation;
- DeepSeek Harness validation.

## v1.0

Only after:

- stable config schema;
- stable daemon API;
- documented upgrade process;
- sustained real-world usage;
- regression suite;
- reproducible release artifacts.

---

# 29. Acceptance Gates

A phase cannot be declared complete because its UI exists.

## Hardening gate

- streaming client receives first SSE chunk before request completion;
- unauthorized `/v1/chat/completions` fails when auth enabled;
- malformed TOML prevents startup;
- process kill targets exact PID;
- CI passes.

## Daemon gate

- TUI can exit while inference continues;
- daemon can run without a terminal;
- CLI can inspect status;
- daemon survives client disconnects.

## Runtime gate

- switch upstream ↔ Codacus without editing source;
- capabilities detected correctly;
- runtime SHA visible;
- unsupported settings rejected before start.

## Oracle gate

- observed configurations are persisted;
- external VRAM use changes admission result;
- confidence is exposed;
- predicted vs actual peak is measured.

## Benchmark gate

- benchmark can be rerun from stored configuration;
- results include runtime/model fingerprints;
- comparison is generated from actual records.

## Remote gate

- work laptop connects through Tailscale;
- llama-server remains localhost-only;
- OpenCode streams tokens correctly;
- tool calls work;
- model swap does not terminate an active request.

---

# 30. Design Rules

These rules should be treated as architectural invariants.

1. **Inference belongs to llama.cpp.**
2. **Saltnitor does not fake runtime capabilities.**
3. **A setting that the active runtime does not support must be rejected or hidden.**
4. **Benchmark claims require stored evidence.**
5. **Model-load success is not sufficient proof of stability.**
6. **Remote exposure must be explicit.**
7. **Raw llama-server remains private by default.**
8. **Do not sacrifice streaming semantics in the proxy.**
9. **Do not kill processes by name.**
10. **Do not silently recover from invalid configuration by discarding it.**
11. **Do not tie core architecture to Qwen or one llama.cpp fork.**
12. **Do not update the active production runtime without validation.**
13. **Do not use tests as substitutes for real functionality.**
14. **Preserve reproducibility: runtime SHA + model hash + profile + hardware.**
15. **Every optimization must be disable-able so a baseline remains available.**

---

# 31. Flagship Qwen Validation Path

Reference baseline:

```text
Qwen3.6-35B-A3B
UD-Q4_K_M
RTX 3060 12 GB
Ryzen 7 7700
32 GB DDR5
Pop!_OS
```

Tuning ladder:

```text
Baseline
   ↓
host registration
   ↓
expert prefetch
   ↓
routing trace
   ↓
expert cache
   ↓
cache slot search
   ↓
async CPU split
   ↓
MTP
   ↓
context sweep
   ↓
agent benchmark
```

Do not change all variables simultaneously.

Store each run.

Final production profile should be selected using agent task performance, not raw TG tokens/sec alone.

---

# 32. Recommended First 30 Days of Work

## Week 1

- create `vnext` branch;
- commit Cargo.lock;
- clean runtime artifacts;
- strict config errors;
- auth middleware;
- write streaming integration test;
- fix proxy streaming.

## Week 2

- extract config/proxy/telemetry/service modules;
- reduce `main.rs`;
- implement PID process objects;
- introduce structured error types;
- improve CI.

## Week 3

- introduce daemon-owned state;
- run TUI as client;
- add CLI status/profile commands;
- preserve current visual experience.

## Week 4

- implement RuntimeBackend;
- add upstream backend;
- add capability detection;
- add runtime fingerprint;
- begin Codacus backend.

At the end of 30 days, the goal is **not** expert-cache tuning.

The goal is a stable architectural base that makes expert-cache tuning safe to add.

---

# 33. Definition of Success

Saltnitor vNext is successful when it can truthfully demonstrate:

> On one Linux workstation, Saltnitor can discover and supervise multiple llama.cpp runtime builds, safely select a runtime/model profile based on actual hardware availability, expose a streaming OpenAI-compatible endpoint, benchmark and reproduce tuning configurations, and serve a remote coding agent without requiring the agent to know how the underlying inference runtime is configured.

For the flagship demonstration:

```text
Work laptop
    ↓ Tailscale
Saltnitor
    ↓
Codacus llama.cpp
    ↓
Qwen3.6-35B-A3B
    ↓
RTX 3060 12 GB
```

OpenCode or DeepSeek Harness should be able to complete a real repository task while Saltnitor records:

- runtime identity;
- active model;
- memory pressure;
- inference performance;
- tool-call workload;
- request history;
- crashes/retries;
- the exact tuning profile used.

That end-to-end proof is more valuable than any individual TUI feature.

---

# 34. Final Priority Order

If development time is constrained, implement in this order:

```text
P0
Correctness and safety
    ↓
P1
Daemon / TUI separation
    ↓
P2
Runtime abstraction
    ↓
P3
Telemetry + Oracle v2
    ↓
P4
Benchmark persistence
    ↓
P5
Codacus/Qwen tuning
    ↓
P6
Remote agent serving
    ↓
P7
Additional runtimes / GPUs / UI polish
```

Do not invert this order by starting with Qwen-specific controls.

The architecture is the upgrade.  
The Codacus/Qwen integration is the first serious workload used to prove it.
