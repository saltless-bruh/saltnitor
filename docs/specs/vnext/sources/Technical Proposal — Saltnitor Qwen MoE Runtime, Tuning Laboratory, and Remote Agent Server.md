# Technical Proposal  
## Saltnitor as a Qwen MoE Runtime, Tuning Laboratory, and Remote Agent Inference Server

**Project:** Saltnitor  
**Target Model:** Qwen3.6-35B-A3B  
**Reference Quantization:** UD-Q4_K_M / Q4_K_M  
**Primary Runtime:** `thecodacus/llama.cpp` `perf` branch  
**Reference Hardware:** NVIDIA RTX 3060 12 GB, AMD Ryzen 7 7700, 32 GB DDR5, Pop!_OS  
**Primary Client Workloads:** OpenCode, DeepSeek Harness, other OpenAI-compatible coding agents  
**Deployment Model:** Home inference server accessed remotely through Tailscale

---

# 1. Executive Summary

This proposal extends **Saltnitor** from a llama.cpp monitoring and orchestration TUI into a complete **local-LLM runtime management, benchmarking, tuning, and remote-serving platform**.

Saltnitor already provides most of the infrastructure required for this role. The current project can monitor GPU and system state, control a systemd-managed llama.cpp daemon, expose an OpenAI-compatible control endpoint, hot-swap models through llama.cpp's native model router, tune runtime parameters, and measure TTFT, prompt-processing speed, and generation throughput. 

The proposed work builds on that foundation in three stages:

**Part 1 — Saltnitor Runtime Architecture**  
Replace the assumption of a single llama.cpp installation with a runtime abstraction capable of managing upstream llama.cpp and optimized forks such as Codacus's `perf` branch. Add first-class support for MoE-specific tuning, runtime version tracking, named configuration profiles, and reproducible engine configuration.

**Part 2 — Model Testing and Tuning Laboratory**  
Turn Saltnitor into an experimental harness for Qwen3.6-35B-A3B. It will automate benchmarking, expert routing trace collection, MoE cache tuning, MTP testing, CPU/GPU expert-placement experiments, context-size testing, and result persistence. The objective is not merely to maximize token generation speed but to determine the best configuration for real coding-agent workloads.

**Part 3 — Remote Server and Agent Client Architecture**  
Run Saltnitor and llama.cpp continuously on the home workstation while OpenCode, DeepSeek Harness, or another agent operates on a laptop elsewhere. Tailscale provides the private transport network. The coding agent remains on the laptop and manipulates the laptop's repository locally, while Qwen inference executes remotely on the RTX 3060 server.

The resulting system can be summarized as:

```text
                         HOME INFERENCE SERVER

┌──────────────────────────────────────────────────────────────┐
│ Pop!_OS                                                     │
│ Ryzen 7 7700 / 32 GB DDR5 / RTX 3060 12 GB                 │
│                                                              │
│                     SALTNITOR                                │
│                         │                                    │
│         ┌───────────────┼────────────────┐                   │
│         │               │                │                   │
│   Monitoring       Tuning Engine    Benchmark Lab            │
│         │               │                │                   │
│         └───────────────┼────────────────┘                   │
│                         │                                    │
│                 Runtime Manager                              │
│                 ┌───────┴────────┐                           │
│                 │                │                           │
│          Codacus llama.cpp   upstream llama.cpp              │
│                 │                                            │
│           llama-server                                     │
│                 │                                            │
│        Qwen3.6-35B-A3B UD-Q4_K_M                            │
└─────────────────┬────────────────────────────────────────────┘
                  │
             Tailscale
                  │
                  │ Internet
                  │
┌─────────────────▼────────────────────────────────────────────┐
│                    WORK LAPTOP                              │
│                                                             │
│ OpenCode / DeepSeek Harness / IDE Agent                    │
│            │                                                │
│            ├── reads files                                  │
│            ├── writes files                                 │
│            ├── executes shell                               │
│            ├── runs tests                                   │
│            └── sends inference requests ────────────────────┐│
│                                                             ││
└─────────────────────────────────────────────────────────────┘│
                  ▲                                            │
                  └────────────────────────────────────────────┘
```

The central design principle is:

> **Saltnitor manages the inference runtime.  
> llama.cpp executes the model.  
> Qwen provides reasoning.  
> The coding agent controls tools and the project.  
> Tailscale connects the two machines.**

---

# Part 1 — Saltnitor Runtime Architecture

# 2. Current Saltnitor Foundation

The existing Saltnitor architecture is already well aligned with this objective.

Saltnitor currently supports:

- GPU, CPU, RAM, VRAM, temperature, clock and process telemetry.
- systemd lifecycle control for the llama.cpp service.
- llama.cpp native `--models-preset` routing.
- dynamic model hot-swapping.
- a VRAM Oracle intended to reject unsafe model loads.
- an OpenAI-compatible control API.
- an interactive benchmark console.
- TTFT measurement.
- separate prompt-evaluation and generation throughput measurements.
- runtime parameter editing through its Deep Engine Tuner.
- model-specific configuration through `router.ini`.
- crash dumping and daemon recovery controls.

The current README explicitly describes Saltnitor as an orchestration layer in front of llama.cpp's native router and as an OpenAI-compatible endpoint usable by clients such as OpenCode, Cline, Aider and Continue. 

This makes the project a suitable base for the proposed system.

---

# 3. Problem With the Current Runtime Model

Saltnitor currently assumes that one llama.cpp installation is the inference engine.

Several locations are tied directly to a particular llama.cpp directory. For example, the launcher currently points at:

```text
/home/laz/ai-models/llama.cpp/router.ini
```

and searches for `llama-server` inside that llama.cpp installation. 

The tuner also writes directly to the same fixed `router.ini` path. 

That approach works for one runtime but becomes problematic when evaluating:

```text
upstream llama.cpp

versus

Codacus llama.cpp

versus

future experimental runtimes
```

The first architectural change should therefore be **runtime abstraction**.

---

# 4. Runtime Backend Architecture

Saltnitor should treat llama.cpp implementations as interchangeable execution backends.

```text
Saltnitor
   │
   ▼
RuntimeManager
   │
   ├── upstream
   │      └── llama-server
   │
   ├── codacus-stable
   │      └── llama-server
   │
   └── codacus-testing
          └── llama-server
```

A runtime definition should contain at minimum:

```toml
[runtimes.codacus-stable]

type = "llama.cpp"

root = "/home/laz/ai-runtimes/codacus-stable"

server = "/home/laz/ai-runtimes/codacus-stable/build/bin/llama-server"
bench  = "/home/laz/ai-runtimes/codacus-stable/build/bin/llama-bench"
trace  = "/home/laz/ai-runtimes/codacus-stable/build/bin/llama-moe-trace"

branch = "perf"
```

A second runtime could be:

```toml
[runtimes.upstream]

type = "llama.cpp"

root = "/home/laz/ai-runtimes/upstream"

server = "/home/laz/ai-runtimes/upstream/build/bin/llama-server"
bench  = "/home/laz/ai-runtimes/upstream/build/bin/llama-bench"
```

This architecture allows identical models and benchmark workloads to be tested against multiple engines.

---

# 5. Runtime Version Identity

Every runtime should be fingerprinted.

Saltnitor should collect:

```text
repository
branch
Git commit SHA
build date
CUDA version
compiler
runtime feature set
```

Example UI:

```text
ENGINE
──────────────────────────────────────
Runtime        Codacus llama.cpp
Branch         perf
Commit         27c54b4
Build          2026-09-15
CUDA           13.x
GPU            RTX 3060 12 GB

FEATURES
Expert cache       ✓
MoE tracing        ✓
Async CPU split    ✓
MTP                ✓
TurboQuant KV      ✓
Native router      ✓
```

The runtime commit SHA is especially important because Codacus's `perf` branch evolves independently from upstream.

Its current implementation contains MoE expert caching, async CPU execution, pinned mmap support, TurboQuant KV support, MTP support, CPU tensor parallelism, and `llama-moe-trace`, while remaining synchronized with upstream llama.cpp development. 

A benchmark without a runtime SHA should therefore be considered incomplete.

---

# 6. Stable and Testing Runtime Channels

Saltnitor should avoid modifying its production inference runtime directly through `git pull`.

Recommended layout:

```text
~/ai-runtimes/

├── upstream/
│   └── llama.cpp/
│
├── codacus-stable/
│   └── llama.cpp/
│
└── codacus-testing/
    └── llama.cpp/
```

The update lifecycle becomes:

```text
New Codacus version
        │
        ▼
codacus-testing
        │
        ▼
build
        │
        ▼
Saltnitor regression suite
        │
        ├── fail ──→ reject
        │
        └── pass
             │
             ▼
      codacus-stable
```

This prevents a new experimental fork commit from unexpectedly breaking the inference server used remotely during the workday.

---

# 7. Configuration Ownership

Runtime binaries and Saltnitor configuration should be separated.

Recommended structure:

```text
~/.config/saltnitor/
├── config.toml
├── router.ini
├── profiles/
│   ├── qwen36-safe.toml
│   ├── qwen36-agent.toml
│   └── qwen36-long-context.toml
└── workloads/
    ├── coding.json
    ├── reasoning.json
    └── tool-calling.json
```

Experimental and generated data should live separately:

```text
~/.local/share/saltnitor/
├── benchmarks/
├── traces/
├── moe-profiles/
└── crash-dumps/
```

No generated tuning data should live inside the llama.cpp source checkout.

---

# 8. MoE-Specific Tuning Page

The current Saltnitor tuner provides three configuration pages for compute/memory, context/speculation, and orchestration/security. 

A fourth page should be introduced:

```text
PAGE 4 — MoE / EXPERT ENGINE
```

Recommended controls:

| Control | Runtime argument/environment | Purpose |
|---|---|---|
| CPU MoE layers | `n-cpu-moe` | controls expert placement |
| Expert cache | enabled/disabled | enables Codacus hot-expert cache |
| Cache slots | `moe-cache-slots` | cached experts per layer |
| Cache profile | `moe-cache-profile` | expert routing profile |
| Async CPU | `sched-async-cpu` | overlaps CPU/GPU expert execution |
| MTP | `spec-type=draft-mtp` | speculative decoding |
| Host registration | `GGML_CUDA_REGISTER_HOST` | pinned host expert memory |
| Expert prefetch | `GGML_SCHED_PREFETCH_EXPERTS` | overlaps transfers and compute |

Codacus's fork explicitly supports expert-cache configuration inside llama.cpp `--models-preset` profiles, which means this design fits Saltnitor's existing router architecture without replacing it. 

---

# 9. Per-Model Versus Process-Level Configuration

Saltnitor must distinguish between two classes of settings.

## Model-level parameters

These belong inside a `router.ini` section:

```ini
[qwen36-agent]

model = /home/laz/models/Qwen3.6-35B-A3B-UD-Q4_K_M.gguf

ngl = 99
ctx-size = 32768
n-cpu-moe = 26

flash-attn = on

moe-cache-profile = /home/laz/.local/share/saltnitor/moe-profiles/qwen36-agent.csv
moe-cache-slots = 88
```

## Runtime-level parameters

These affect the llama.cpp process itself:

```text
GGML_CUDA_REGISTER_HOST=1
GGML_SCHED_PREFETCH_EXPERTS=1
```

They should therefore be represented as:

```toml
[runtimes.codacus-stable.env]

GGML_CUDA_REGISTER_HOST = "1"
GGML_SCHED_PREFETCH_EXPERTS = "1"
```

Changing a process-level option requires restarting the llama.cpp service.

Changing a model-level configuration may only require reloading or restarting the relevant model.

This distinction prevents Saltnitor from treating fundamentally different types of parameters as equivalent switches.

---

# 10. Named Tuning Profiles

Users should not have to remember individual argument combinations.

Saltnitor should introduce named model profiles.

Example:

```toml
[profiles.qwen36-safe]

runtime = "codacus-stable"
model = "/home/laz/models/Qwen3.6-35B-A3B-UD-Q4_K_M.gguf"

ctx_size = 16384
n_cpu_moe = 26

flash_attention = true

moe_cache = false
mtp = false
```

Agent profile:

```toml
[profiles.qwen36-agent]

runtime = "codacus-stable"
model = "/home/laz/models/Qwen3.6-35B-A3B-UD-Q4_K_M.gguf"

ctx_size = 32768
n_cpu_moe = 99

flash_attention = true

moe_cache = true
moe_cache_slots = 88
moe_cache_profile = "qwen36-agent.csv"

mtp = true
async_cpu = true
```

Long-context profile:

```toml
[profiles.qwen36-long]

runtime = "codacus-stable"

ctx_size = 65536

kv_quantization = true
moe_cache_slots = 64

mtp = true
```

This enables one-key switching between configurations rather than repeated manual editing.

---

# Part 2 — Qwen Testing and Tuning Laboratory

# 11. Tuning Objective

The goal is **not maximum benchmark token throughput**.

The target workload is a real coding agent.

Therefore the optimization objective should balance:

```text
Prompt-processing throughput
         +
Generation throughput
         +
Time to first token
         +
Usable context size
         +
Model quality
         +
Tool-call correctness
         +
VRAM stability
         +
RAM stability
         +
Long-session reliability
```

This matters because coding agents repeatedly perform cycles such as:

```text
reason
 ↓
tool call
 ↓
receive source code
 ↓
reason
 ↓
tool call
 ↓
receive test output
 ↓
reason
```

Prompt processing therefore becomes substantially more important than in ordinary chat.

---

# 12. Baseline Model

The proposed reference configuration is:

```text
Model:
Qwen3.6-35B-A3B

Quantization:
UD-Q4_K_M

Runtime:
Codacus llama.cpp

GPU:
RTX 3060 12 GB

CPU:
Ryzen 7 7700

RAM:
32 GB DDR5

Initial context:
32K
```

Q4_K_M is particularly useful as the baseline because Codacus's own RTX 3060 optimization work uses the Q4_K_M model configuration.

The objective is therefore to start from a configuration known to be practical on essentially the same GPU rather than blindly comparing every available quantization.

---

# 13. Baseline Before Optimization

All optimization features must initially be disabled.

Baseline:

```text
UD-Q4_K_M
-ngl 99
-n-cpu-moe 26
Flash Attention ON

MoE cache OFF
MTP OFF
expert prefetch OFF
host-register OFF
async experimental features OFF
```

The baseline provides the control sample.

Without it, later speedups cannot be attributed to a specific change.

---

# 14. Experimental Ladder

Testing should proceed incrementally.

```text
STAGE 0
Baseline
Q4_K_M
n-cpu-moe 26
       │
       ▼
STAGE 1
+ pinned host expert memory
       │
       ▼
STAGE 2
+ expert prefetch
       │
       ▼
STAGE 3
capture expert-routing profile
       │
       ▼
STAGE 4
enable MoE expert cache
       │
       ▼
STAGE 5
find optimal cache slots
       │
       ▼
STAGE 6
async CPU/GPU expert execution
       │
       ▼
STAGE 7
MTP speculative decoding
       │
       ▼
STAGE 8
context-size optimization
       │
       ▼
STAGE 9
coding-agent end-to-end evaluation
```

Only one major variable should change between adjacent experiments.

---

# 15. Pinned Expert Memory Test

Codacus provides:

```text
GGML_CUDA_REGISTER_HOST=1
```

The feature page-locks mmap-backed expert weights so GPU transfers can use direct DMA rather than an intermediate transfer path.

His RTX 3060 measurements report host-to-device transfer improvement from roughly 6–7 GB/s to approximately 20 GB/s under the tested workload. 

Saltnitor should record:

```text
PP t/s
TG t/s
TTFT
peak RAM
peak VRAM
GPU utilization
memory-controller utilization
```

before and after enabling this feature.

---

# 16. Expert Prefetch Test

Codacus additionally provides:

```text
GGML_SCHED_PREFETCH_EXPERTS=1
```

Instead of executing:

```text
copy expert
wait
compute
copy next expert
wait
compute
```

the runtime attempts:

```text
GPU compute expert N
        ║
        ║ concurrently
        ║
transfer expert N+1
```

Codacus reports approximately:

```text
~1143 prompt tok/s
        ↓
~1880 prompt tok/s
```

for his RTX 3060 Qwen3.6-35B-A3B benchmark with the optimized path enabled. 

Saltnitor should reproduce the experiment independently rather than assume the result transfers perfectly to the Ryzen 7 7700 system.

---

# 17. MoE Routing Profiler

Codacus's runtime contains `llama-moe-trace`.

Saltnitor should integrate this directly.

UI concept:

```text
┌─ MoE PROFILER ───────────────────────────────┐
│                                             │
│ Model       Qwen3.6-35B-A3B                 │
│                                             │
│ Workloads                                   │
│ [x] Coding                                  │
│ [x] General reasoning                       │
│ [x] Tool calling                            │
│ [ ] Creative                                │
│ [ ] Custom                                  │
│                                             │
│ Tokens / workload     512                   │
│                                             │
│        [ RUN PROFILER ]                     │
└─────────────────────────────────────────────┘
```

Saltnitor runs multiple traces:

```text
coding.csv
reasoning.csv
tool-calling.csv
```

and merges them:

```text
coding.csv
     +
reasoning.csv
     +
tool-calling.csv
     │
     ▼
qwen36-agent.csv
```

The resulting file becomes:

```ini
moe-cache-profile =
    /home/laz/.local/share/saltnitor/moe-profiles/qwen36-agent.csv
```

Codacus's documentation explicitly recommends merged routing profiles because profiles generated from multiple representative workloads generalize better than a single narrow workload. 

---

# 18. Automatic Expert-Cache Slot Tuner

The most important expert-cache tuning parameter is:

```text
--moe-cache-slots
```

Increasing it keeps more routed experts permanently resident in VRAM.

However:

```text
too few slots
→ wasted GPU memory
→ reduced acceleration

too many slots
→ insufficient space for KV/cache/compute
→ runtime OOM
```

Codacus recommends reserving additional VRAM beyond the expert pack because the runtime allocates more memory after model loading. 

This fits naturally with Saltnitor's existing VRAM Oracle.

Suggested procedure:

```text
Try 64
 ↓
stable?

yes
 ↓
80
 ↓
stable?

yes
 ↓
96
 ↓
stable?

no
 ↓
binary search 80–96
 ↓
find maximum stable slot count
 ↓
subtract safety margin
```

The result might be:

```text
Maximum loadable: 96
Maximum prompt-stable: 92
Recommended production: 88
```

The difference between “loads successfully” and “survives a 32K coding prompt” must remain explicit.

---

# 19. MTP Testing

Codacus's current fork supports:

```text
--spec-type draft-mtp
```

for supported Qwen architectures. 

MTP should be tested only after a stable expert-cache configuration has been established.

Measurements:

```text
TG tok/s
PP tok/s
TTFT
VRAM delta
acceptance rate
output equivalence / quality
tool-call reliability
```

The objective is not to turn MTP on merely because generation throughput increases.

If MTP introduces instability or reduces effective context, the non-MTP profile should remain available.

---

# 20. Context Testing

The tuning sequence should initially use:

```text
16K
or
32K
```

rather than immediately targeting 128K.

Recommended progression:

```text
16K
 ↓
32K
 ↓
48K
 ↓
64K
```

At every stage Saltnitor should measure:

```text
KV allocation
peak VRAM
prompt processing speed
TTFT
generation speed
expert cache capacity
```

Context competes directly with model buffers and expert-cache capacity.

Therefore:

```text
larger context
       ↓
larger KV
       ↓
less VRAM for hot experts
       ↓
potentially slower MoE execution
```

The optimum for coding agents may therefore be a smaller practical context with aggressive compaction rather than the model's theoretical maximum context.

---

# 21. Benchmark Workloads

Synthetic token benchmarks alone are insufficient.

Saltnitor should define three benchmark classes.

## Microbenchmark

Used for engine performance.

Examples:

```text
PP512
PP2048
PP8192

TG128
TG512
```

## Controlled model benchmark

Fixed realistic prompts:

```text
code comprehension
bug diagnosis
repository planning
long-file analysis
tool-call generation
```

## Agent benchmark

Run OpenCode or DeepSeek Harness against an actual repository task.

Measure:

```text
task completion time
number of LLM turns
number of tool calls
prompt tokens
generated tokens
failed tool calls
context compactions
test result
TTFT distribution
total wall-clock time
```

The third benchmark is ultimately the most important.

---

# 22. Benchmark Result Schema

Each test should be stored as a structured record.

Example:

```json
{
  "timestamp": "2026-09-15T20:30:00+07:00",

  "runtime": {
    "name": "codacus",
    "branch": "perf",
    "commit": "27c54b4"
  },

  "hardware": {
    "gpu": "RTX 3060 12GB",
    "cpu": "Ryzen 7 7700",
    "ram": "32GB DDR5"
  },

  "model": {
    "name": "Qwen3.6-35B-A3B",
    "quant": "UD-Q4_K_M"
  },

  "configuration": {
    "ctx": 32768,
    "ngl": 99,
    "n_cpu_moe": 99,
    "moe_cache_slots": 88,
    "mtp": true,
    "host_register": true,
    "expert_prefetch": true
  },

  "results": {
    "prompt_tps": 1850.4,
    "generation_tps": 69.1,
    "ttft_ms": 1120,

    "peak_vram_gb": 11.05,
    "peak_ram_gb": 25.7
  }
}
```

---

# 23. Benchmark Comparison UI

Saltnitor should expose a comparison view.

```text
Qwen3.6-35B-A3B / UD-Q4_K_M
RTX 3060 12 GB

┌─────────────────────────────────────────────────────────────┐
│ CONFIG            PP/s     TG/s     TTFT    VRAM    RAM    │
├─────────────────────────────────────────────────────────────┤
│ Baseline          1140     42.0     1.90s   10.1    22.4   │
│ Host Pin          1510     42.1     1.45s   10.1    22.4   │
│ + Prefetch        1840     42.1     1.12s   10.1    22.4   │
│ + Cache           1910     59.8     1.08s   10.9    22.0   │
│ + Async           1940     62.5     1.05s   10.9    22.0   │
│ + MTP             1935     71.4     1.04s   11.2    22.0   │
└─────────────────────────────────────────────────────────────┘
```

Saltnitor should identify:

```text
Best decode profile
Best prefill profile
Best long-context profile
Best agent profile
```

rather than declaring a single universal winner.

---

# 24. Production Profile Selection

The eventual profile used by OpenCode should be selected based on **agent behavior**, not llama-bench alone.

Example production profile:

```text
Qwen3.6 Agent

Quant:
UD-Q4_K_M

Runtime:
Codacus stable

Context:
32K

MoE:
expert cache enabled

MTP:
enabled if stable

Host registration:
enabled

Expert prefetch:
enabled

Flash Attention:
enabled
```

Exact values for:

```text
n-cpu-moe
moe-cache-slots
KV quantization
batch
ubatch
```

must be determined experimentally.

They should not be hardcoded into the architecture proposal.

---

# Part 3 — Remote Server and Agent Client Architecture

# 25. Deployment Objective

The desktop remains powered on at home.

The laptop is taken to work.

The laptop should be capable of running:

```text
OpenCode
DeepSeek Harness
Continue
Aider
custom agents
```

while using the home RTX 3060 for inference.

The laptop is therefore an **agent client**, not an inference node.

---

# 26. Responsibility Separation

## Home server

Responsible for:

```text
model storage
GGUF loading
GPU inference
CPU MoE inference
KV cache
model routing
runtime tuning
runtime telemetry
benchmarking
API serving
```

## Laptop

Responsible for:

```text
source repository
filesystem tools
shell
git
tests
IDE
agent harness
MCP tools
user interaction
```

This keeps the repository and tools close to the developer while centralizing expensive inference on the desktop.

---

# 27. Request Flow

Example OpenCode interaction:

```text
USER
 │
 ▼
OpenCode
 │
 │ sends task + context + tool definitions
 ▼
Tailscale
 │
 ▼
Saltnitor API
 │
 ▼
llama-server
 │
 ▼
Qwen3.6
 │
 │
 │ returns:
 │ "read src/auth.rs"
 ▼
OpenCode
 │
 │ executes tool LOCALLY on laptop
 ▼
src/auth.rs
 │
 │ source returned to Qwen
 ▼
Qwen
 │
 │ returns edit/tool call
 ▼
OpenCode
 │
 ├── edits project
 ├── runs tests
 └── reports result
```

The home machine never needs direct access to the laptop's working repository.

---

# 28. Network Architecture

Tailscale should provide private connectivity.

Recommended topology:

```text
                         INTERNET

                 encrypted Tailscale

              ┌────────────┴────────────┐
              │                         │
              ▼                         ▼

       Work Laptop                 Home PC
       100.x.x.x                   100.x.x.x

       OpenCode                    Saltnitor
                                     │
                                     ▼
                               llama-server
                                     │
                                     ▼
                                 Qwen3.6
```

No public router port forwarding should be required.

---

# 29. Saltnitor as the External Endpoint

The preferred topology is:

```text
Laptop
  │
  ▼
Tailscale
  │
  ▼
Saltnitor :8765
  │
  ▼
llama-server :8080
```

rather than:

```text
Laptop
  │
  ▼
llama-server directly
```

The current Saltnitor control API binds to localhost. 

It should be extended to support configurable binding:

```toml
[control]

bind = "tailscale"
port = 8765

auth = true
```

Internally:

```text
llama-server
127.0.0.1:8080
```

can remain inaccessible externally.

Only Saltnitor is exposed to the Tailscale interface.

---

# 30. Why Saltnitor Should Proxy Inference

Putting Saltnitor in front of llama-server allows it to enforce:

```text
authentication
runtime availability
VRAM safety
model selection
profile selection
hot swapping
logging
benchmark instrumentation
failure detection
```

Architecture:

```text
                     SALTNITOR

Request
   │
   ▼
Authentication
   │
   ▼
Model/profile resolver
   │
   ▼
VRAM/RAM oracle
   │
   ▼
Runtime health check
   │
   ▼
llama-server
   │
   ▼
Qwen
```

This makes Saltnitor the stable interface even if the underlying inference runtime changes.

---

# 31. OpenAI-Compatible Endpoint

Clients should see:

```text
http://saltnitor-home:8765/v1
```

or the relevant Tailscale IP.

Example:

```text
http://100.x.x.x:8765/v1
```

Available models may map to profiles:

```text
qwen36-fast
qwen36-agent
qwen36-long
```

The client therefore sends:

```json
{
  "model": "qwen36-agent",
  "messages": [...]
}
```

Saltnitor resolves:

```text
qwen36-agent
      │
      ▼
Codacus stable
      │
      ▼
Qwen3.6 UD-Q4_K_M
      │
      ▼
32K context
expert cache
MTP
other tuned settings
```

---

# 32. OpenCode Client

Conceptual OpenCode configuration:

```jsonc
{
  "model": "saltnitor/qwen36-agent",

  "providers": {
    "saltnitor": {
      "name": "Home Saltnitor",

      "package": "@opencode/ai/providers/openai-compatible",

      "settings": {
        "baseURL": "http://saltnitor-home:8765/v1",
        "apiKey": "{env:SALTNITOR_API_KEY}"
      },

      "models": {
        "qwen36-agent": {
          "modelID": "qwen36-agent"
        }
      }
    }
  }
}
```

OpenCode continues to execute all coding tools locally.

Only LLM inference leaves the laptop.

---

# 33. DeepSeek Harness Client

DeepSeek Harness can use the same architecture.

Provider:

```text
Provider:
Home Saltnitor

Protocol:
OpenAI-compatible

Base URL:
http://saltnitor-home:8765/v1

Model:
qwen36-agent
```

This allows OpenCode and DeepSeek Harness to share one backend without the backend needing agent-specific logic.

---

# 34. Daemon Architecture

The server should start automatically.

Recommended systemd chain:

```text
Pop!_OS boots
      │
      ▼
tailscaled
      │
      ▼
saltnitor.service
      │
      ▼
llama-router.service
      │
      ▼
Codacus llama-server
      │
      ▼
Qwen ready
```

Saltnitor should expose states such as:

```text
OFFLINE
STARTING
MODEL LOADING
READY
DEGRADED
OOM RECOVERY
CRASHED
```

A remote laptop should be able to query health before starting a long agent operation.

---

# 35. Remote Administrative Access

Tailscale should also allow SSH:

```text
Laptop
  │
  ▼
ssh home-ai
```

This provides a recovery path if:

```text
Saltnitor crashes
llama-server crashes
runtime update fails
model refuses to load
systemd service becomes unhealthy
```

The inference API and administrative SSH path remain logically separate.

---

# 36. Failure Recovery

Expected cases:

```text
llama-server crash
      │
      ▼
systemd restart
      │
      ▼
Saltnitor health probe
      │
      ▼
reload production profile
```

OOM:

```text
OOM detected
     │
     ▼
record crash state
     │
     ▼
terminate llama-server
     │
     ▼
restart known-safe profile
     │
     ▼
mark experimental configuration failed
```

Power interruption:

```text
AC lost
  │
  ▼
server powers down
  │
  ▼
AC returns
  │
  ▼
BIOS Restore-on-AC-Power = ON
  │
  ▼
Pop!_OS boot
  │
  ▼
services restart
```

This makes the workstation function more like an appliance than an interactive desktop session.

---

# 37. Security Boundary

The proposed deployment should expose only:

```text
Tailscale
Saltnitor API
SSH if required
```

It should not expose llama-server directly to the public Internet.

Recommended:

```text
llama-server:
127.0.0.1:8080

Saltnitor:
Tailscale-IP:8765
```

API authentication should be required even though the connection operates over Tailscale.

This provides defense in depth and prevents another device on the tailnet from automatically receiving unrestricted model access.

---

# 38. Performance Expectations

For remote use, network bandwidth is unlikely to be the bottleneck.

Agent traffic consists mainly of:

```text
prompts
source snippets
tool schemas
tool results
generated tokens
```

Even a large agent turn is small compared with ordinary file-transfer workloads.

The significant latency components are more likely:

```text
prompt prefill
model generation
tool execution
context processing
```

rather than network transfer.

Therefore the Part 2 optimization work directly determines the quality of the Part 3 remote-agent experience.

---

# 39. Proposed Development Phases

## Phase 1 — Runtime Refactor

Deliver:

```text
runtime abstraction
remove hardcoded llama.cpp paths
upstream runtime support
Codacus runtime support
runtime SHA/version display
stable/testing channels
```

Success criterion:

> Saltnitor can switch between upstream and Codacus llama.cpp without code changes.

---

## Phase 2 — Qwen MoE Controls

Deliver:

```text
MoE tuner page
n-cpu-moe control
Codacus environment controls
expert-cache controls
async CPU controls
MTP controls
```

Success criterion:

> All important Codacus Qwen optimization parameters can be configured through Saltnitor.

---

## Phase 3 — Profiling and Benchmark Lab

Deliver:

```text
llama-bench integration
llama-moe-trace integration
routing-profile generation
automatic slot search
benchmark persistence
configuration comparison
```

Success criterion:

> The user can determine a stable optimal configuration without manually constructing benchmark commands.

---

## Phase 4 — Agent Workload Validation

Deliver:

```text
coding benchmark workloads
tool-call benchmark
OpenCode end-to-end test
DeepSeek Harness end-to-end test
long-running agent test
```

Success criterion:

> Qwen can complete realistic repository tasks without tool-call or context-management failures.

---

## Phase 5 — Remote Serving

Deliver:

```text
Tailscale-aware control binding
authenticated Saltnitor API
localhost-only llama-server
remote health endpoint
automatic service recovery
```

Success criterion:

> Laptop at work can use the home model through OpenCode or DeepSeek Harness without exposing llama.cpp publicly.

---

# 40. Acceptance Criteria

The project should be considered successful when the following are demonstrated.

### Runtime

Saltnitor can manage at least:

```text
upstream llama.cpp
Codacus llama.cpp
```

without changing source code.

### Reproducibility

Every benchmark records:

```text
runtime commit
model
quantization
configuration
hardware
result
```

### Model stability

Qwen3.6-35B-A3B can sustain:

```text
32K agent workloads
multiple tool cycles
multiple repository operations
```

without persistent OOM failures.

### Tool calling

OpenCode and DeepSeek Harness receive valid structured tool calls from the model through the Saltnitor/llama.cpp stack.

### Performance

Codacus optimizations produce a measurable improvement over the baseline on the actual workstation.

### Remote operation

A laptop outside the home network can:

```text
connect
select model
send inference requests
run an agent session
```

through Tailscale.

### Recovery

If llama-server crashes, the production configuration can recover without physical access to the workstation.

---

# 41. Final Target Architecture

```text
                                 HOME

┌────────────────────────────────────────────────────────────────────────┐
│                      RTX 3060 AI SERVER                               │
│                                                                        │
│ Ryzen 7 7700                                                          │
│ DDR5 32 GB                                                            │
│ RTX 3060 12 GB                                                        │
│ Pop!_OS                                                               │
│                                                                        │
│ ┌──────────────────────── SALTNITOR ────────────────────────────────┐  │
│ │                                                                  │  │
│ │  Runtime Manager                                                 │  │
│ │     ├── upstream                                                 │  │
│ │     ├── codacus-stable                                           │  │
│ │     └── codacus-testing                                          │  │
│ │                                                                  │  │
│ │  Tuning Lab                                                      │  │
│ │     ├── MoE tuner                                                │  │
│ │     ├── expert profiler                                          │  │
│ │     ├── slot optimizer                                           │  │
│ │     ├── MTP tester                                               │  │
│ │     └── context tester                                           │  │
│ │                                                                  │  │
│ │  Benchmark Lab                                                   │  │
│ │     ├── llama-bench                                              │  │
│ │     ├── realistic workloads                                      │  │
│ │     └── historical comparison                                    │  │
│ │                                                                  │  │
│ │  API                                                             │  │
│ │     └── Tailscale-IP:8765                                        │  │
│ │                                                                  │  │
│ └────────────────────────────┬─────────────────────────────────────┘  │
│                              │                                         │
│                        localhost only                                  │
│                              │                                         │
│                              ▼                                         │
│                  Codacus llama-server                                  │
│                              │                                         │
│                              ▼                                         │
│                Qwen3.6-35B-A3B UD-Q4_K_M                              │
│                                                                        │
└──────────────────────────────┬─────────────────────────────────────────┘
                               │
                               │ Tailscale
                               │
                               │ Internet
                               │
┌──────────────────────────────▼─────────────────────────────────────────┐
│                         WORK LAPTOP                                   │
│                                                                        │
│                         OpenCode                                      │
│                            or                                          │
│                    DeepSeek Harness                                   │
│                              │                                         │
│              ┌───────────────┼────────────────┐                        │
│              │               │                │                        │
│           Files            Shell             Git                       │
│              │               │                │                        │
│              └───────────────┼────────────────┘                        │
│                              │                                         │
│                     Local Project                                      │
│                                                                        │
└────────────────────────────────────────────────────────────────────────┘
```

---

# 42. Conclusion

The proposed system does not require replacing Saltnitor with a new model-serving stack.

Instead, Saltnitor becomes the layer that makes an optimized llama.cpp runtime operationally usable.

The relationship becomes:

```text
Qwen
=
reasoning model

Codacus llama.cpp
=
optimized inference engine

Saltnitor
=
runtime control + observability + tuning + benchmarking + serving

Tailscale
=
private transport

OpenCode / DeepSeek Harness
=
agent execution environment
```

The most important architectural decision is to avoid coupling Saltnitor directly to Codacus-specific behavior.

Codacus functionality should be represented as **runtime capabilities** that Saltnitor can detect and expose.

This preserves Saltnitor's original purpose as a general llama.cpp control platform while allowing it to exploit specialized Qwen/MoE functionality when an appropriate runtime is installed.

The immediate implementation target should therefore be:

```text
1. Introduce runtime abstraction.

2. Remove remaining hardcoded llama.cpp paths.

3. Integrate Codacus as the first alternative backend.

4. Add MoE-specific configuration support.

5. Establish the Qwen3.6 Q4_K_M baseline.

6. Build the benchmark and routing-profile workflow.

7. Determine the actual optimal RTX 3060 configuration.

8. Only then promote that configuration to the remote production profile.

9. Expose Saltnitor through Tailscale.

10. Connect OpenCode and DeepSeek Harness to Saltnitor rather than directly to llama-server.
```

This sequence preserves reproducibility, makes performance claims measurable, minimizes operational risk, and turns the existing RTX 3060 workstation into a practical private inference server for day-to-day agentic development.