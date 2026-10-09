# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

**Project:** Saltnitor — hardware-aware control plane and tuning lab for local `llama.cpp` inference (`saltnitor`, Rust 2024)
**Spec:** `saltnitor-vnext` r2.5 · baseline `master` @ `c89f278` · work branch `vnext`

This file is loaded on every turn. It is short on purpose: at most 220 lines (REQ-DOC-005). Read it, then read the spec.

---

## 1. This project is spec-driven

The specification is the source of truth. Code serves the spec, not the other way around.

| File | Role | When you read it |
|---|---|---|
| `docs/specs/vnext/requirements.md` | The contract: 274 requirements (`REQ-<AREA>-nnn`), 439 EARS acceptance criteria, invariants `INV-01…18`, decisions `DEC-*`, baseline defects `BD-01…34`, parameters §6 | §0–§6 once; then only the IDs your task cites |
| `docs/specs/vnext/tasks.md` | The work queue: 150 tasks, phases P0–P11, each ending in a gate G0–G11 | Every session; it is your working file |
| `docs/specs/vnext/sources/SALTNITOR_VNEXT_BLUEPRINT.md` — **[BP]** | The feature guide: what each new component promises and how it should behave (daemon, runtime backends, oracle v2, scheduler, proxy v2, benchmark lab, MoE lab, TUI redesign, error model, design rules §30) | When a task's **Read first** cites `BP §n`, or you need the intent behind a requirement |
| `docs/specs/vnext/sources/Technical Proposal — Saltnitor Qwen MoE Runtime, Tuning Laboratory, and Remote Agent Server.md` — **[TP]** | The *why*: the motivation for vNext. It turns Saltnitor into a runtime/tuning lab for Qwen3.6-35B-A3B on the Codacus `perf` fork, served remotely over Tailscale to OpenCode / DeepSeek Harness | When a task cites `TP §n`, or when judging whether a design choice serves the goal |
| `docs/specs/vnext/tools/spec_lint.py` | Keeps requirements and tasks consistent and checks test → requirement traceability | Before committing any spec or test change |
| `docs/specs/vnext/SPEC_AUDIT_REPORT.md` | Why the spec is shaped this way | Only when a rule seems arbitrary |

**Precedence:** `requirements.md` → BP → TP. The ACs are the contract. BP and TP explain intent and never add scope a task doesn't cite. Where BP and TP conflict, BP's order and design win (DEC-01).

The pack lives in `docs/specs/vnext/`: tools in `tools/`, BP/TP in `sources/`, baseline docs in `baseline/`, gate evidence in `evidence/`, R0/R1 captures in `tests/fixtures/captures/`.

- Look up a requirement: `grep -n -A12 "^#### REQ-PRX-002" docs/specs/vnext/requirements.md`
- Load one phase: `P=P0; sed -n "/<!-- phase:$P -->/,/<!-- \/phase:$P -->/p" docs/specs/vnext/tasks.md`
- Resolve a source citation such as `BP §12.5` or `TP §18`. Each file is about 1,900 lines, so never load one whole. `ctx_index(path, source: "BP")` it once per session, then `ctx_search(queries: [...], source: "BP")`. Batch every question into one call. `grep -nE "^#+ 12\.5[ .]"` + a ranged Read is the fallback.

**If code and spec disagree, the spec wins until the operator says otherwise.**

**This file and the spec override global rules** (`~/.claude/rules/`, e.g. ECC's Rust rules). Where they conflict — auto-`cargo fmt`, `mockall`, coverage targets — follow §5, §7, and §11.

---

## 2. The working loop

1. `git status`; `git log --oneline -15`; tail `docs/specs/vnext/PROGRESS.md`.
2. Run Standard Verification (§3) **before** changing anything. A new failure is your first task. The known baseline failures in fmt and clippy are not (§11).
3. The current phase is the first one whose gate isn't `PASSED` in `docs/specs/vnext/evidence/`.
4. Take the **lowest-numbered unchecked task** in that phase whose `Depends` are all `[x]`. The `Tn.0` gate-test task comes first. Do not skip ahead.
5. Read every cited ID. `REQ-X` means every AC. `REQ-X/AC3` means only that AC. The §3 invariants always apply.
6. Implement **one task**, following its **Files**, **Do**, and **Not in scope** fields. Stop when **Done when** holds.
7. Tick the box, append a PROGRESS.md line, and commit:
   ```
   T1.11: stream proxy body without buffering

   Task: T1.11
   Refs: REQ-PRX-002, REQ-PRX-003
   ```

One task at a time keeps the model on one goal. A whole phase in one context produces confident, half-verified work.

---

## 3. Commands

```bash
cargo build --locked                         # release: cargo build --release --locked
cargo run -- --port 8080 --service-name llama-router   # CLI flags override config.toml
cargo test --all --locked                    # single test: cargo test <name_substring> -- --nocapture
cargo fmt --all --check
cargo clippy --all-targets --all-features --locked -- -D warnings
python3 docs/specs/vnext/tools/spec_lint.py                   # --sync regenerates fields; --tests --phase P0 before a gate
python3 -m unittest discover -s docs/specs/vnext/tools        # spec_lint self-test
```

These arrive with their tasks: `scripts/check-invariants.sh` (T1.5), `scripts/gate.sh G<n>` (T0.10), `cargo test -p fake-llama-server` (T0.4), and `cargo test --features hardware-tests -- --ignored` (reference host only).
`scripts/smoke_control_api.sh` smoke-tests the **live** router and control API. It is a manual tool, never a test.

**Context discipline (context-mode).** Build, test, clippy, and lint output is large. Run it through `ctx_execute` / `ctx_batch_execute` (concurrency 1, since cargo holds a lock), then filter with `ctx_search`. Don't trim with `| head` or `| tail` inside the command, because those lines never get indexed. Use `Read` only on a file you are about to edit. Files outside the repo root (live `router.ini`, systemd unit, `config.toml`) are refused by `ctx_execute_file` unless a `Read(...)` allow rule exists.

---

## 4. How the code works today (`c89f278`)

It is one process. Everything flows through a single `tokio::mpsc::Sender<Event>`, which is the only way state reaches the UI.

```
 key poller ──┐                                              ┌─▶ ui::draw(&App)  (src/ui.rs)
 hw poller (sysinfo + nvidia-smi, 500ms) ─┤                  │
 journalctl -u <svc> -f ──────────────────┼─▶ mpsc<Event> ─▶ main loop mutates App (src/app.rs)
 /v1/models poller (5s) · ss port audit ──┤                  │   └─ key handlers spawn: sudo -n systemctl,
 control_api (axum :8765, 127.0.0.1) ─────┘                  │      killall -9, router.ini rewrite, interrogator
        │ /v1/ensure → oracle → 1-token warm load → router    │
        └ /v1/chat/completions → ensure(model) → forward ─────┴─▶ llama-server router :8080 (--models-max 1)
```

- Repository layout: `src/` (5 files today; P2+ adds `cli, client, config, daemon, runtime, model, scheduler, storage, telemetry, proxy, process, benchmark, service, tui`), `tools/fake-llama-server/` (T0.4), `tests/acceptance/` + `tests/fixtures/` (protected), `scripts/`, and `docs/specs/vnext/` (after T0.1).
- `src/main.rs` (~990 lines) holds config loading, preflight, every background task, and all key handling. `main()` alone is about 850 lines (BD-12). P2 splits it into modules.
- `src/control_api.rs` is the oracle and proxy. Its model ids are `router.ini` **section names**, matched to `[profiles.<id>]` in `~/.config/saltnitor/config.toml`. The oracle is a filename heuristic (`parse_params_b`/`parse_bpw`) unless `est_vram_gb` is set. `ensure_lock` serializes loads.
- Config: `~/.config/saltnitor/config.toml` (under `sudo`, `/home/$SUDO_USER/...` is used). CLI flags override it. Parse errors silently fall back to defaults (BD-01).

---

## 5. Definition of Done

A task is complete only when **all** of these hold:

1. Every cited AC is met by its own verification method: `(T)` test, `(T-HW)`, `(I)` inspection, `(D)` demonstration, `(A)` analysis.
2. Every test that proves an AC carries `/// Verifies: REQ-…/ACn` (enforced by `spec_lint.py --tests`).
3. SV passes, and CI is green on the pushed commit.
4. Nothing under the task's **Not in scope** was touched.
5. Docs and examples changed in the same commit as the behavior (REQ-DOC-007/AC3).
6. The PROGRESS.md line is written.

Do not mark `[x]` on partial work. Write `[!] BLOCKED: <task> — <AC> cannot be met because <evidence>. Options: A/B. Need: <decision>.`

---

## 6. Always — non-negotiable rules

**Inference**
- Inference belongs to llama.cpp. Never transform, repair, or re-tokenize model output. The proxy is byte-exact and must stream (INV-01, INV-08).
- Every capability comes from probing the real runtime binary, or from an explicit override recorded as `override`. Unsupported settings are rejected or hidden, never passed through (INV-02, INV-03).

**Processes and the runtime**
- Signal exact PIDs or pidfds. Never `killall`, `pkill`, or kill by name (INV-09).
- Never git, cmake, make, or write inside runtime channel directories from `src/` (INV-12).
- Serialize every model load. Upstream `--models-max` is not race-safe (DEC-03).

**Network**
- Loopback only unless config explicitly says otherwise. Detecting an address never causes binding. Raw `llama-server` is always loopback (INV-06, INV-07).

**Config and data**
- Invalid config is an error. Never silently replace it with defaults (INV-10).
- TOML profiles are the single source of truth. The router preset INI is generated into the state dir (DEC-04).
- Every benchmark, validation, memory, and crash record stores the reproducibility tuple: runtime fingerprint, model hash, resolved profile + env, and hardware (INV-14).

**Honesty**
- A value that wasn't measured is `null`/`n/a` with a reason, never `0` or a guess (INV-18).
- No literal performance or tuning numbers in `src/`. They come from stored records, profiles, or §6 defaults (INV-04, INV-17).
- `qwen`/`codacus` literals appear only in that fork's backend module and the parameter-registry data (INV-11).

---

## 7. Never — anti-goals that stop the work

| Don't | Why |
|---|---|
| Build a tensor runtime, sampler, or output post-processor | Saltnitor manages the runtime. llama.cpp executes the model (NG-01, NG-08) |
| Special-case test inputs, stub a feature, or return a placeholder to go green | Tests are the oracle for an agent. Gaming them is the failure this spec is built to catch (INV-13, REQ-TST-015) |
| Edit `requirements.md`, gate rows, `tests/acceptance/`, fixtures, snapshots, or `scripts/gate.sh` to pass | They are protected (CODEOWNERS). Only the operator changes the oracle (REQ-TST-007, -012) |
| Treat "it loaded" as "it works" | Load success is not stability. `stable` requires surviving the validation workload (INV-05) |
| Implement a later phase "while you're here" | Gates are sequential. P6's scheduler exists because the P7 lab needs leases (DEC-02) |
| Hard-code the design to Qwen, Codacus, or one fork's flag dialect | Forks differ, for example the MoE cache knob is slots vs. MiB (NG-07, DEC-17) |
| Add Ollama registry, web chat UI, multi-host, cloud providers, `/v1/responses`/`embeddings`/`completions` | Explicit non-goals this cycle (NG-02…05, NG-11) |
| Mock your own code when the fake runtime can exercise it | Prefer the real binary + `fake-llama-server`. Integration catches wiring bugs mocks hide |

---

## 8. Ask first — hard stops

These need a human. **Write a `BLOCKED:` entry and stop.** Do not improvise.

- Any `[HW]` step (needs the reference host and GPU) or `[HUMAN]` step (operator action or sign-off). Never tick one yourself, and never write `Verdict: PASSED` on a gate.
- The **live system outside this repo**. Read it for context, but never modify it from code or tests:
  `~/ai-models/llama.cpp/` (llama-server build, live `router.ini`, `examples/external-mode/launch_router.sh`, models) · `/etc/systemd/system/llama-router.service` · `/etc/sudoers.d/saltnitor` · `~/.config/saltnitor/config.toml` · `~/ai-runtimes/*` (future runtime channels, built by the operator only).
- New crates outside the policy (REQ-ARCH-008), public API or config-key changes the task doesn't describe, destructive git operations, and anything blocked by an open `OQ-*`.
- Security design beyond what `REQ-SEC-*` specifies.

---

## 9. Divergence protocol

Reality will contradict the spec. Silent divergence is not acceptable.

1. Stop the affected work.
2. File `CR-<n> · <date> · affects: <IDs> · found in: <task>` in `docs/specs/vnext/CHANGE_REQUESTS.md` with problem (plus evidence), proposal, and impact.
3. Wait for the operator. On approval, amend the requirement **in place** and log it in Appendix I (REQ-DOC-007).

When an AC is ambiguous, ask. A wrong interpretation implemented confidently costs more than a question. **Never** delete, renumber, or reuse an ID. Retired IDs stay listed in Appendix I.

---

## 10. Testing

- No test touches a real GPU, the live router, or the network. `tools/fake-llama-server` (T0.4) is scenario-driven: chunks, delays, hangs, crashes, OOM. It records argv, env, and requests to JSONL.
- `[PROP]` ACs get `proptest` properties, not just examples. TUI screens get ratatui `TestBackend` + `insta` snapshots.
- P0 is characterization only. `git diff c89f278 -- src/` may show test modules and snapshots, nothing else (REQ-MIG-003).
- Preserved behavior (`SHALL CONTINUE TO`, REQ-MIG-007) is tested before it is refactored.
- The baseline has **zero tests** (BD-10). Characterization tests are what make P1+ safe.

---

## 11. Known traps

- **SV is red at baseline, and that is expected until T1.3.** `cargo fmt --check` fails, and clippy reports 32 errors under `-D warnings`. The operator deferred the cleanup to Phase 1 (2026-09-28), where T1.3 (REQ-CI-001) makes CI green. In P0, only `cargo test` and `spec_lint.py` must pass. Never run `cargo fmt` or clippy `--fix` on `src/` in P0: it breaks G0's "no behavior change" diff check (REQ-MIG-003). New test code you add should still be fmt- and clippy-clean.
- **The "streaming" proxy buffers.** `h_chat` calls `resp.bytes().await`, so the whole response arrives at once (BD-02). axum's 2 MB body limit also applies (BD-24).
- **The resident fast path may never fire.** `router_loaded()` reads `loaded`/`state`/`status`, but current router builds report `"status":{"value":"loaded"}` (BD-32, suspected). Confirm it with a captured fixture.
- **The TUI hot-swap bypasses the oracle.** The TUI calls the router directly with an NGL guessed from the filename (BD-19). The oracle ignores external VRAM use (BD-13).
- **The tuner edits the live runtime.** It rewrites the file named by the configured `router_ini` in place, then runs `sudo -n systemctl restart` (BD-06, BD-25). Some tuner keys are never written (BD-17).
- **Router preset keys are whitelisted per build.** Per-model `ctx-size`/`ngl`/`override-tensor` may be silently ignored. Trust fixtures captured from the actual binary, not READMEs (REQ-RT-005).
- **No lease means mid-stream eviction.** A request for B evicts A while A is streaming (BD-23). This is fixed in P6, not before.
- **Running under `sudo` leaks root-owned files.** The history file is written relative to CWD and crash dumps go to `$HOME` (BD-21).

---

## 12. Reference numbers

```
Reference host   Pop!_OS · Ryzen 7 7700 (8C/16T) · 32 GB DDR5 · RTX 3060 12 GB · one GPU
Ports            Saltnitor 8765 · llama-server 8080 (loopback)
Oracle reserve   800 MB VRAM · 1 024 MB RAM      Risk (headroom)  LOW ≥ 1 024 · MEDIUM ≥ 512 · HIGH ≥ 0 · REJECT < 0 MB
Proxy            p95 overhead ≤ 10 ms · first chunk forwarded ≤ 200 ms · cancel ≤ 1 000 ms · body ≤ 32 MiB
Structure (P2+)  main.rs ≤ 150 lines · any fn ≤ 100 lines · CLAUDE.md ≤ 220 lines
```

The governing sentence, and the reason every boundary exists:

> **Saltnitor manages the runtime. llama.cpp executes the model. Qwen reasons. The coding agent controls tools and the project. Tailscale connects the machines.**
