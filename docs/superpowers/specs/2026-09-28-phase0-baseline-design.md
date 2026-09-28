# Phase 0 — Baseline Freeze & Test Harness: Implementation Design

- **Date:** 2026-09-28 · **Spec:** `saltnitor-vnext` r2.1 · **Phase:** P0 (tasks T0.1–T0.11, gate G0)
- **Baseline:** `master` @ `c89f278` · **Work branch:** `vnext`
- **Status:** approved in brainstorming (operator, 2026-09-28); awaiting written-spec review

This document says *how* Phase 0 will be carried out. *What* Phase 0 must achieve is fixed by
`tasks.md` (Phase 0 block) and the requirements it cites; where this document and the spec
disagree, the spec wins, except for the three change requests in §8, which the operator approved
in principle and which are filed formally in T0.1.

---

## 1. Intent

Phase 0 pins today's behavior into tests and builds the test tooling every later phase needs.
**No production behavior changes.** Success means:

1. Every behavior listed in REQ-MIG-002/005/007 is pinned by a test that passes on unmodified code.
2. A scenario-driven fake runtime exists for all later protocol, scheduler, and benchmark tests.
3. A **real-world baseline** exists: a recording of the live router and one real journey through
   Saltnitor, so each later gate can show the *upgraded* system still works on the real machine,
   not only in the controlled test environment (operator intent, §4).
4. Gate G0 passes, with the verdict written by the operator.

## 2. Operator decisions (2026-09-28)

| # | Question | Decision |
|---|---|---|
| D1 | Oracle reads real hardware (`nvidia-smi`, `/proc/meminfo`), no seam allowed in P0 | Make outcomes machine-independent with extreme estimates; assert hardware numbers by shape only. Tests may run `nvidia-smi` read-only. |
| D2 | Source of live data for T0.8 and BD-32 | The agent captures both itself, read-only; operator approves via the diff. |
| D3 | CI only triggers on `master` | Add `vnext` to `rust.yml` triggers in T0.1 (CR-1). |
| D4 | Pushing | Push `vnext` after each task that passes locally. Never push `master`, never force-push, no PR without asking. |
| D5 | Fake runtime design | Custom axum engine, scenario as data, lib + bin. |
| D6 | Record/replay of the real router | Keep it, as the real-world check (§4, CR-2). |
| D7 | Branch protection vs per-task pushes | Full protection on `master`; `vnext` guarded against force-push/deletion only, protected paths tracked per gate (CR-3). |

## 3. Order of work and commit discipline

Lowest-numbered task whose dependencies are done, one task per commit, push after each:

```
T0.1 (+ file CR-1, CR-2, CR-3) → T0.2 → T0.3 (BD-01…31; BD-32 BLOCKED until T0.4)
→ T0.4 (engine + record/replay) → [close BD-32, commit cites T0.3; T0.3 box ticks only now]
→ T0.5 → T0.6 → T0.7 → T0.8 → T0.9 → T0.10 → T0.11 → G0 evidence
```

Commit format per CLAUDE.md §2 (`T0.n: summary`, `Task:`, `Refs:`). Each task ends with a
PROGRESS.md line and ticks its box only when its **Done when** holds.

**Per-task verification in P0** (SV is red at baseline for fmt/clippy until T1.3):

- `cargo test --all --locked` green; `python3 docs/specs/vnext/tools/spec_lint.py` exit 0.
- `git diff c89f278 -- src/` shows only `#[cfg(test)]` modules and `src/snapshots/`.
- **No new lint debt:** capture the clippy warning list (`--message-format short`) before and after;
  the set difference must be empty. Test modules added are checked with `rustfmt --check` on a
  scratch copy containing only the added module.

## 4. Real-world check: record/replay (CR-2)

Two tiers. Neither is part of `cargo test`; tests never touch the live router (CLAUDE.md §10).

```
                 P0 (now)                                   each later gate G1…G11
real router ─▶ R0 record (read-only GETs)      ─┐        R0 again ─▶ drift vs baseline
               liveness + response shapes      │
               [agent runs it]                 ├─▶ tests/fixtures/captures/baseline/
real router ─▶ R1 exercise (through saltnitor) │        R1 again ─▶ same journey through the
 + GPU         ensure A_STD → stream chat →    │                    UPGRADED binary, diff vs
               ensure B (swap) → status       ─┘                    baseline → evidence/G<n>.md
               [HW: operator says "go" per run]
```

**R0 — `fake-llama-server record --upstream http://127.0.0.1:8080 --out <dir>`**
- Fixed allow-list of GET endpoints: `/health`, `/models`, `/v1/models`, `/props`, `/slots`,
  `/metrics`. Never chat, `/models/load`, or `/models/unload` (these change GPU state).
- Saves each body plus `manifest.json` (endpoint, status, latency ms, content type, capture time).
  The bearer is sent but never written; `/home/<user>/` becomes `${HOME}/`; string values of
  user-text keys (`prompt`, `content`, `text`, `generated`, `generated_text` — e.g. in `/slots`) become
  `<redacted>`, so no prompt text reaches the public repo.
- Prints a table per endpoint: `LIVE` / `DOWN` / `DEGRADED` (non-2xx or slow), and `SHAPE-OK` /
  `SHAPE-DRIFT` versus the previous capture (JSON key-path set comparison, values ignored).
- Endpoints the build does not expose (404) are recorded as `absent`, not failures.

**R1 — `scripts/real-check.sh` `[HW]`**
- Starts the built `saltnitor` under `script` (a pseudo-terminal; `tmux` is not installed) against the live router,
  unless one is already serving the control API,
  (live config read-only), then via the control API: `POST /v1/ensure {"profile":"A_STD"}` →
  streaming chat with a fixed short prompt and `max_tokens` 16 → `POST /v1/ensure {"profile":"B"}`
  → `GET /v1/status`.
- Records per step: HTTP status, outcome, ensure `load_ms`, time to first byte, total time, chunk
  count, response key paths. **Not** the generated text (non-deterministic).
- Loads and evicts models on the GPU, so it runs only after the operator says "go". The agent never
  ticks the `[HW]` step.
- Profile names are required arguments (`--a A_STD --b B` for the P0 baseline); nothing
  Qwen-specific is hard-coded (INV-11). Refuses to run without `--yes`.

**Replay:** a scenario may set `replay_from = "<capture dir>"`; the engine then serves recorded
bodies byte-for-byte for those endpoints, and scripted faults still apply on top.

Captures live under `tests/fixtures/captures/` (CODEOWNERS-protected from T0.10), so the operator
approves every capture. P0 produces `baseline/` (R0 + one R1 run).

## 5. Fake runtime — `tools/fake-llama-server` (T0.4)

```
tools/fake-llama-server/
  Cargo.toml        publish = false; axum, tokio, tokio-stream, serde, serde_json, toml, reqwest
                    (all at versions already in the tree — no new crate families)
  src/lib.rs        pub fn spawn(Scenario) -> Handle { addr, recorder }   (in-process)
  src/scenario.rs   Scenario, RouteScript, Fault — serde (TOML) + builder methods
  src/engine.rs     axum routes: /health /v1/models /models /models/load /models/unload
                    /v1/chat/completions (SSE + JSON) /metrics /slots
  src/recorder.rs   JSONL: argv, env, requests (method, path, headers minus auth, body),
                    client-disconnect timestamps
  src/record.rs     R0 capture + liveness/drift table
  src/main.rs       llama-server-style argv (--host, --port; the rest recorded and ignored);
                    env FAKE_SCENARIO (TOML), FAKE_RECORD (JSONL); prints FAKE_LISTENING <addr>;
                    `record --upstream URL --out DIR [--baseline DIR] [--bearer-env VAR]`;
                    --help / --version → fixture text selected by FAKE_HELP_FIXTURE / FAKE_VERSION_FIXTURE
                    (records only env vars prefixed LLAMA_ GGML_ CUDA_ HIP_ FAKE_)
  tests/*.rs        one test per scenario feature; process-level tests use
                    env!("CARGO_BIN_EXE_fake-llama-server")
  README.md         scenario format; how other crates' tests locate the binary
                    (`cargo build -p fake-llama-server`, then target/<profile>/fake-llama-server)
```

**Faults, per route:** `Status { code, body }` · `Chunks { items, delay_ms }` · `HangBeforeHeaders`
· `CrashAfter { chunks }` (binary exits non-zero; in-process drops the connection) · `Malformed`
(truncated JSON / broken SSE framing) · `OomIf { arg, gt }` (binary exits at startup with
llama.cpp-style OOM text when an argv value exceeds the threshold).

**Model state is real inside the fake:** load/unload mutate the set `/models` reports.
`models_shape = "status_object"` (`"status":{"value":"loaded"}`, current upstream) or `"legacy"`
(the fields today's `router_loaded()` reads), so BD-32 is reproducible both ways.

**Root `Cargo.toml`:** add `[workspace] members = [".", "tools/fake-llama-server"]`; the root
package is otherwise unchanged. New `[dev-dependencies]`: `fake-llama-server = { path = … }`,
`insta` (named by the spec for TUI snapshots). Nothing under `src/` changes.

**Done when:** `cargo test -p fake-llama-server` covers every fault, both `/models` shapes,
load/unload state, recorder output, `--help`/`--version` fixtures, `OomIf`, and R0 record against
an in-process fake upstream (including an `absent` endpoint and a `SHAPE-DRIFT` case).

## 6. Characterization tests (T0.5, T0.6, T0.7)

All tests live in `#[cfg(test)] mod tests` blocks appended to existing `src/` files (the crate is
binary-only, so there is no external test target), and each carries `/// Verifies: REQ-…/ACn`.

**T0.5 — pure helpers (table-driven)**
- `upsert_ini_section`: replace key · append key · comments kept · other sections untouched ·
  missing section → `None`.
- `estimate_footprint`: `est_*` overrides win · `offload` split · filename-heuristic path.
- `parse_params_b`: `…30B-A3B…`, `…7b…`, no size → `None`. `parse_bpw`: `Q4_K_M`, `Q8_0`, `F16`,
  unknown → default.
- `Stage::from_outcome`: each `EnsureOutcome` variant → its terminal frame.

**T0.6 — control API against the fake**
```
test ─▶ fake::spawn(scenario) ─▶ fake_addr
     ─▶ ControlApi::new(profiles, "http://fake_addr", …, reserve_vram=0, reserve_ram=0, tx)
     ─▶ free loopback port (bind 127.0.0.1:0, read, drop) ─▶ tokio::spawn(serve(api, addr))
     ─▶ poll /healthz until 200 (retry on bind race) ─▶ reqwest against the real HTTP surface
```
- The real `serve()` is used because its route table is private to it; copying the table into a
  test would only verify the copy.
- Coverage: `/healthz`; `/v1/models` passthrough; `/v1/status` (fields present and numeric, values
  not asserted — D1); `/v1/ensure` → `already_resident`, `loaded` (estimates 0), unknown profile,
  oracle reject (`est_vram_gb = 1e6`), 401 with a control token; `/v1/ensure/stream` stage order
  `received → oracle_ok → loading → loaded`; non-stream chat (ensure then forward); upstream 5xx
  passthrough.
- **Defects are pinned, not endorsed (REQ-MIG-002/AC3):** `already_resident` uses the `legacy`
  `/models` shape; the `status_object` shape (BD-32) is not asserted either way, with a comment
  citing BD-32. BD-02: the fake streams 5 chunks 200 ms apart; the test measures time-to-first-byte
  direct vs through Saltnitor and prints both; the numbers go into `DEFECTS.md`. No assertion.
- Once the R0 baseline exists, the `/v1/models` passthrough also runs against the replayed capture.
- Scenario files: `tests/fixtures/scenarios/*.toml`.

**T0.7 — TUI snapshots**
- Fixture: `App::new(fixed args)`, then overwrite every host-dependent field — console history
  (the constructor reads `.saltnitor_history` from the CWD), logs, sparkline series, process lists,
  uptime. `ui.rs` does not read the clock.
- 11 insta snapshots on `TestBackend` 100×30: dashboard (= deck mode 0, Interrogator) · deck mode 1
  (Hot-Swap) · tuner pages 1–3 · GPU inspector · CPU inspector · help · search; plus the
  too-small refusal at 79×16 and 80×15.
- Snapshots in `src/snapshots/` (insta default for tests in `src/ui.rs`).
- Each snapshot is the `TestBackend` buffer's `Debug` form: text **and** style runs (fg/bg/modifiers).
  The plain `Display` form captures text only, which would leave color-only meaning (highlights,
  the red blinking size warning) unpinned.

## 7. Docs, protection, gate (T0.1, T0.2, T0.3, T0.8, T0.9, T0.10, T0.11, G0)

**T0.1** — create `vnext`; `git mv specs/*` → `docs/specs/vnext/` with `spec_lint.py` and
`test_spec_lint.py` into `tools/` (the pack's own layout); BP and TP → `sources/`; empty
`PROGRESS.md`, `CHANGE_REQUESTS.md` (with CR-1…3 filed), `baseline/`, `evidence/`; rewrite
CLAUDE.md paths; move this design doc's commit onto `vnext` (§9); apply CR-1 (one trigger line).

**T0.2 `BEHAVIOR.md`** — tables for CLI flags, config keys + defaults, keybindings, routes (method,
auth, status codes, body shape), files read/written, external commands; each row cites
`file:line` at `c89f278`. A completeness check (`docs/specs/vnext/tools/check_baseline_docs.py` (also checks DEFECTS.md)) fails
if any `.route(` in `control_api::serve` or any `KeyCode::` arm in `main.rs` is not cited.

**T0.3 `DEFECTS.md`** — 32 entries: Evidence (`file:line`, re-checked), Repro (one line), Status
`confirmed`/`disputed` + reason, `Fixed by:` empty. BD-32 closes from the R0 capture after T0.4;
BD-02 gets T0.6 timings.

**T0.8** — `known-good.ini` from the live `~/ai-models/llama.cpp/router.ini` (read-only):
`/home/laz/…` → `${MODELS_DIR}/…`, hosts and keys → placeholders. `readme-example.ini` from
README. Check: `grep -n "/home/" tests/fixtures/router/*` shows only placeholders.

**T0.9** — files already moved in T0.1; run `--sync`, make `--tests --phase P0` pass with the tags
from T0.5–T0.7, run the self-test.

**T0.10**
```
.github/CODEOWNERS         docs/specs/** tests/acceptance/** tests/fixtures/** scripts/gate.sh
                           **/snapshots/**  → @saltless-bruh
scripts/gate.sh G<n>       automated rows → PASS/FAIL; manual rows → MANUAL (never auto-passed);
                           lists every protected-path change since the previous gate (CR-3)
evidence/TEMPLATE.md       Appendix A, including the anti-gaming rubric
tools/verifier-prompt.md   fresh reviewer receives only requirements, diff, evidence
```
For G0, automated rows are 1, 2, 3, 6 plus the inventory check; rows 4, 5, 7, V are manual.

**T0.11** — `cargo test --all -- --list` per suite → `evidence/test-baseline.txt`, all non-zero.

**G0 evidence (`evidence/G0.md`)** — `gate.sh G0` output; smoke (row 7): `saltnitor` run against the
fake under `script` with an isolated `HOME` and `control_port = 18765` (so the operator's live
config and running instance are untouched), screen captured from the `script` log, `curl
127.0.0.1:18765/healthz` → 200; R0 + R1 baseline; protected-path change list; verifier report
(fresh-context reviewer agent launched only with the operator's okay, or the operator). The agent
never writes `Verdict: PASSED`.

## 8. Change requests to file in T0.1

- **CR-1 · affects: T0.1, REQ-CI-001 (scope)** — add `vnext` to `push`/`pull_request` branches in
  `.github/workflows/rust.yml` so each P0 commit gets CI. No other workflow change; T1.3 still owns
  the CI rework.
- **CR-2 · affects: T0.4, T0.3, gate evidence template** — add R0 record/replay to
  `fake-llama-server` and `scripts/real-check.sh` (R1, `[HW]`); captures under
  `tests/fixtures/captures/`; each gate's evidence includes an R0 + R1 rerun diffed against the
  baseline.
- **CR-3 · affects: T0.10, REQ-TST-012/AC1** — full branch protection (code-owner review) on
  `master`; `vnext` protected against force-push and deletion only. Protected-path changes on
  `vnext` go in commits with a `Protected-change:` trailer (PROGRESS.md lines and task-checkbox flips
  excepted); `gate.sh` lists them and the
  operator approves them in `evidence/G<n>.md`; CODEOWNERS review is enforced on the `vnext →
  master` PR.

## 9. Constraints and risks

- **Binary-only crate** → all characterization tests are in-file `#[cfg(test)]` modules.
- **Hardware in tests (D1)** → `nvidia-smi` is invoked read-only; a missing binary falls back to
  defaults that the zero/extreme estimates still decide deterministically.
- **`Cargo.lock` is gitignored until T1.1** → `--locked` works locally only; CI (CR-1) runs
  `cargo build/test` without `--locked`, unchanged.
- **Port race in T0.6** → bind-drop-rebind is retried; tests use loopback only (INV-06).
- **R1 depends on the live router being up and on the operator's go** → if either is missing, the
  step is `BLOCKED:` and G0 records it; G0 cannot pass without the R1 baseline.
- **New crates:** `insta` only. The fake reuses crates already in the tree.
- **This design doc** is not committed on `master` (that would move the baseline); it is committed
  as part of T0.1 on `vnext`.
