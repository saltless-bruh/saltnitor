# Phase 1 — Hardening (v0.2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver Phase 1 of `saltnitor-vnext` (T1.0–T1.16, gate G1, tag `v0.2.0`): a byte-exact streaming proxy, one auth layer, strict config, JSON error envelopes, PID-only process control, no panics on fallible paths, no hardcoded secrets, and a CI that fails on real problems.

**Architecture:** The binary stays one process (TUI + control API). A thin `src/lib.rs` (created in T1.8, CR-7) exposes the new modules — `config_v1`, `error`, `auth`, `proxy_stream`, `process`, `interrogate` — plus `control_api`, `events`, `app`, so integration tests and the independently written acceptance suite can reach them. `main.rs` keeps the TUI loop and imports from the library. Acceptance tests (T1.0, written by a fresh session) drive the real binary in a pseudo-terminal against `tools/fake-llama-server`.

**Tech Stack:** Rust 1.95 (edition 2024), axum 0.8.9, reqwest 0.13.3 (`json`, `stream`), tokio 1.52, tokio-stream 0.1.18 (`time`), serde/serde_json, toml 1.1, `serde_path_to_error`, `subtle`, `uuid` (v7), `rustix` (`process`, already in the tree via crossterm), sysinfo 0.39, ratatui 0.30 + insta 1.48, proptest (dev), tempfile (dev), GitHub Actions (checkout v7, dtolnay/rust-toolchain, Swatinem/rust-cache v2, EmbarkStudios/cargo-deny-action v2, upload-artifact v7, attest-build-provenance v4), shellcheck 0.11.

**Spec:** `docs/superpowers/specs/2026-10-08-phase1-hardening-design.md` (design, operator-approved) — implements `docs/specs/vnext/tasks.md` Phase 1 against `docs/specs/vnext/requirements.md` r2.4 (r2.5 after Task 0). Executors read both.

## Global Constraints

- **Spec wins.** `requirements.md` → BP → TP; this plan never adds scope a task does not cite (CLAUDE.md §1). One task per commit; lowest-numbered unchecked task whose `Depends` are all `[x]` (CLAUDE.md §2).
- **Protected paths** (`docs/specs/**`, `tests/acceptance/`, `tests/fixtures/`, `scripts/gate.sh`, `.github/CODEOWNERS`, `**/snapshots/**`) change only in commits carrying a `Protected-change: <paths> — <why>` trailer; the operator approves the list in `evidence/G1.md` (CR-3, CR-8).
- **Never** edit an existing assertion, expected value, or snapshot without a stated reason in the commit (REQ-TST-007/AC2) — and only where the design (CR-8) lists it: T1.8 `pins_bd29_*`/`pins_bd28_*`, T1.13 inspectors + help, T1.15 interrogator deck, T1.16 tuner title.
- **Live system is read-only:** `~/ai-models/llama.cpp/`, `/etc/systemd/system/llama-router.service`, `/etc/sudoers.d/saltnitor`, `~/.config/saltnitor/config.toml`, `~/ai-runtimes/*`. The operator edits `config.toml` themselves when §11 of the design says so.
- **Secrets and home paths** never enter commits, fixtures, logs, captures, or test output. Tokens are read in-process and never printed.
- **Git:** push `vnext` after every task; never push `master`; never force-push; no PR without asking; scratch branches only as Task 4 and Task 6 describe, deleted afterwards.
- **`[HW]` / `[HUMAN]` rows** (R1 run, D1–D3, the verdict, the tag) need the operator. Write `BLOCKED:` and stop; never tick them.
- **Tests:** no GPU, no live router, no network (REQ-TST-010 family). Prefer the real binary + `fake-llama-server` over mocks; `mockall` is not used in this repo (CLAUDE.md overrides the global Rust rules). Every test proving an AC carries `/// Verifies: REQ-…/ACn`.
- **Lints from T1.3 on:** `cargo fmt --all --check` clean; `cargo clippy --all-targets --all-features --locked -- -D warnings` clean with `todo`, `unimplemented`, `dbg_macro`, `unwrap_used` denied in non-test code; every `#[expect]`/`#[allow]` carries `reason = "…"`.
- **Numbers:** §6 defaults only — `stream_forward_budget_ms` 200, `cancel_propagation_ms` 1000, `timeouts.connect_ms/first_byte_ms/idle_ms` 5 000/600 000/120 000, `max_body_bytes` 33 554 432, `process.term_grace_ms` 5 000. No other literal performance numbers in `src/` (INV-04, INV-17).
- **Context discipline:** cargo/clippy/test output goes through `ctx_execute` / `ctx_batch_execute` (concurrency 1), filtered with `ctx_search`; never `| head`/`| tail` inside the command.
- **Crate policy (REQ-ARCH-008):** only the crates in the Stack line above; each new crate is justified in its commit message and allowed by `deny.toml`.

## Review Focus

Inputs the spec implies but no task's own list exercises; each has a pinned test in the owning task (marked **[RF-n]** there):

1. **[RF-1] A request with `Authorization: bearer <token>` (lowercase scheme) or `Bearer  <token>` (two spaces).** RFC 9110 says the scheme is case-insensitive and allows 1\*SP; a reasonable client (curl `-H`, Python `requests`) must authenticate. → Task 11 (T1.9).
2. **[RF-2] A chat body whose `model` is present but not a string (`"model": 3`), or whose top level is a JSON array.** Must be `400 REQUEST_INVALID`, never forwarded, never a panic. → Task 13 (T1.11).
3. **[RF-3] A config with `control_token_file` pointing at a directory, or a key file that is 0600 but owned by another user.** Must exit 2 with the reason, not start with an empty token. → Task 12 (T1.10).
4. **[RF-4] A PID that exits between snapshot and keypress and is immediately recycled to a different user's process.** Must be `PROCESS_CHANGED`, never signalled. → Task 15 (T1.13).
5. **[RF-5] An upstream response with `Transfer-Encoding: chunked` and `Connection: close` headers.** Hop-by-hop headers must not be copied downstream (hyper would reject or double-frame the body); the body must still stream byte-exact. → Task 13 (T1.11).

---

## Skills applied

| Skill | Where it shaped the plan |
|---|---|
| `rust-testing` (project) | TDD cycle per task; integration tests in `tests/`; proptest for `[PROP]` ACs; `tokio::test`; no `sleep`-based waits where a channel/poll can replace them. `mockall` and coverage targets are overridden by CLAUDE.md §11. |
| `rust-patterns` (project) | `?`/`Result` on every fallible path (T1.14); exhaustive matches on `ErrorCode`; minimal `pub` surface in `lib.rs`; `Cow`-free simple APIs; no `unwrap` in non-test code. |
| `tui-design` (global) | Processes screen: one List per inspector, command on the bottom line (narrow pane); `X` uses the light `[y/N]` confirmation (destructive, no undo); never color-only signals; snapshot at 100×30 and the 80×16 minimum; keep business state testable without a terminal (`process.rs`, `interrogate.rs` pure functions). |
| `devops-engineer` (fullstack) | CI with least-privilege `permissions`, `concurrency` cancel-in-progress, pinned major tags, cache, `workflow_dispatch`; release pipeline with checksums + attestation; rollback = re-tag, documented in CHANGELOG. |
| context-mode | All build/test output via `ctx_execute`; docs indexed via `ctx_fetch_and_index` (API table below). |

## API facts checked against docs.rs (2026-10-08)

| Need | Verified API |
|---|---|
| Stream upstream body | `reqwest::Response::bytes_stream(self) -> impl Stream<Item = reqwest::Result<Bytes>>` — crate feature `stream` |
| Downstream streaming body | `axum::body::Body::from_stream<S>(S) where S: TryStream + Send + 'static, S::Ok: Into<Bytes>, S::Error: Into<BoxError>` |
| Idle-between-chunks timeout | `reqwest::ClientBuilder::read_timeout(Duration)` — "applies to each read operation, resets after a successful read"; `connect_timeout(Duration)`; per-item stream timeout also available as `tokio_stream::StreamExt::timeout` (feature `time`) |
| Middleware | `axum::middleware::from_fn_with_state(state, async fn(State<S>, Request, Next) -> Response)` applied with `.route_layer(...)` or `.layer(...)` |
| Body limit | `axum::extract::DefaultBodyLimit::disable()` + manual `axum::body::to_bytes(body, limit)` so the 413 is ours |
| Key path in config errors | `serde_path_to_error::deserialize(de)`, `err.path().to_string()` → `"dependencies.serde.version"`, `err.into_inner()` |
| TOML spans | `toml::de::Error::span() -> Option<Range<usize>>`, `.message()` |
| Request IDs | `uuid::Uuid::now_v7()` — feature `v7` |
| Constant-time compare | `subtle::ConstantTimeEq::ct_eq(&[u8], &[u8]) -> Choice` (short-circuits only on length mismatch) |
| pidfd | `rustix::process::{pidfd_open(Pid, PidfdFlags) -> Result<OwnedFd>, pidfd_send_signal(fd, Signal), kill_process(Pid, Signal), Pid::from_raw(i32) -> Option<Pid>, Signal::TERM, Signal::KILL}` — feature `process`; rustix 1.1.4 already in `Cargo.lock` via crossterm |
| Process identity | `sysinfo::Process::{start_time() -> u64 (s since epoch), user_id() -> Option<&Uid>, effective_user_id(), kill_with(Signal) -> Option<bool>}`; `/proc/<pid>/stat` field 22 for `start_time_ticks` |
| Cargo targets | `[[test]] name = "acceptance"; test = false` → not run by plain `cargo test`; `cargo test --test acceptance` runs it |
| pty for acceptance tests (brief) | `portable_pty::{native_pty_system, PtySize, CommandBuilder}`; `pair.slave.spawn_command`, `pair.master.try_clone_reader`, `take_writer` (MIT, wezterm) |
| GitHub Actions (latest tags) | `actions/checkout@v7`, `actions/upload-artifact@v7`, `actions/attest-build-provenance@v4`, `EmbarkStudios/cargo-deny-action@v2`, `dtolnay/rust-toolchain@1.95.0`, `Swatinem/rust-cache@v2` |

If a signature differs at execution time (crate minor bump), the executor records the actual one in the task's PROGRESS note and adapts the call; it never changes the behaviour the test pins.

## Conventions used by every task

**Standard Verification (SV).** Before and after each task (CLAUDE.md §3), through `ctx_execute`:

```bash
cargo test --workspace --locked
python3 docs/specs/vnext/tools/spec_lint.py
python3 -m unittest discover -s docs/specs/vnext/tools
# from Task 4 (T1.3) on, also:
cargo fmt --all --check
cargo clippy --all-targets --all-features --locked -- -D warnings
```

**Close-out procedure** (the last step of each task; "run Close-out" below means exactly this):

```bash
set -o pipefail
# 1. SV passes (above).
# 2. Tick the task and regenerate spec fields.
sed -i 's/^- \[ \] \*\*T1\.N — /- [x] **T1.N — /' docs/specs/vnext/tasks.md     # N = the task
python3 docs/specs/vnext/tools/spec_lint.py --sync && python3 docs/specs/vnext/tools/spec_lint.py
# 3. PROGRESS line: date · task · sha(filled after commit, use "pending") · DONE · note
printf '%s · T1.N · pending · DONE · <one-line note>\n' "$(date +%F)" >> docs/specs/vnext/PROGRESS.md
# 4. Commit with explicit paths (never `git add -A`), then fix the sha in PROGRESS and amend.
git add <paths>
git commit -F - <<'MSG'
T1.N: <summary>

Task: T1.N
Refs: <REQ ids from the task's Reqs line>
<Protected-change: <paths> — <why>   (only when a protected path changed)>

Co-Authored-By: Claude Mythos 5.1 <noreply@anthropic.com>
MSG
sha=$(git rev-parse --short HEAD); sed -i "s/ · T1.N · pending · / · T1.N · $sha · /" docs/specs/vnext/PROGRESS.md
git add docs/specs/vnext/PROGRESS.md && git commit --amend --no-edit -q
# 5. Push and wait for CI (required green from Task 4 on).
git push origin vnext
gh run list --branch vnext --limit 1 --json databaseId,status --jq '.[0].databaseId' | xargs -r gh run watch --exit-status
```

**Blocked.** If an AC cannot be met: `- [!] BLOCKED: T1.N — <AC> cannot be met because <evidence>. Options: A/B. Need: <decision>.` in `tasks.md`, a PROGRESS line with `BLOCKED`, commit, stop.

**Fake runtime in tests.** `fake_llama_server::spawn(Scenario)` → `Handle { addr, recorder, .. }`, `base_url()`, `loaded()`, `recorder.of_kind("request"|"disconnect"|"crash")`. Scenarios are TOML (`Scenario::from_str` via `toml::from_str`); faults: `status`, `chunks {items, delay_ms}`, `hang_before_headers`, `crash_after {chunks}`, `malformed`.

---

### Task 0: File CR-7 and CR-8, amend the spec (r2.5)

**Files:**
- Modify: `docs/specs/vnext/CHANGE_REQUESTS.md` (append CR-7, CR-8)
- Modify: `docs/specs/vnext/tasks.md` (T1.0 Done-when; T1.8 Files; T2.1 Do; G1 row 6)
- Modify: `docs/specs/vnext/requirements.md` (REQ-ARCH-002/AC1 wording; Appendix I row r2.5; header `r2.5`)
- Modify: `CLAUDE.md` (spec version line `r2.4` → `r2.5`)

**Interfaces:**
- Produces: the amended rules every later task relies on — acceptance target `test = false`; `src/lib.rs` created by T1.8; G1 row 6 pass rule.

- [ ] **Step 1: Append the two change requests**

Append to `docs/specs/vnext/CHANGE_REQUESTS.md`:

```markdown

CR-7 · 2026-10-08 · affects: T1.0 (Done-when), T1.8 (Files), T2.1 (Do), REQ-ARCH-002/AC1 · found in: Phase 1 design
Problem: T1.0 requires the acceptance tests to "compile … against the current code", but (a) `tests/` cannot import anything from a binary-only crate until T2.1 creates `src/lib.rs`, and (b) the process-control acceptance test calls an API that does not exist until T1.13. · Proposal: T1.8 creates `src/lib.rs` exposing P1's new modules plus `control_api`, `events`, `app`; `main.rs` imports from it; T2.1 becomes "extend `src/lib.rs`". The acceptance target is declared `[[test]] name = "acceptance", test = false` so a red suite does not break `cargo test`. T1.0's Done-when becomes: the tests compile, except tests that call a P1 API the author names in `tests/acceptance/README.md`, which may fail to compile only for that reason; failure output is recorded; the operator approves via CODEOWNERS review. · Impact: T1.0, T1.8, T2.1 text; REQ-ARCH-002/AC1 gains "(created in T1.8, extended in T2.1)".
Status: APPROVED (operator, 2026-10-08 — design P1-D3)

CR-8 · 2026-10-08 · affects: G1 row 6 (REQ-TST-007) · found in: Phase 1 design
Problem: G1 row 6 passes only with "no assertion or expected-value changes" under `tests/` and `**/snapshots/**`, but T1.8 must rewrite the CR-6 `pins_bd28_*`/`pins_bd29_*` pins, T1.13 adds `x`/`X` and PID/user columns to the inspectors and help, T1.15 changes the interrogator deck (PP/TG or `n/a`), and T1.16 changes the tuner title (BD-30). · Proposal: row 6 pass condition becomes "every changed assertion, expected value, or snapshot since the G0 commit sits in a commit that states why (REQ-TST-007/AC2) and carries a `Protected-change:` trailer, and the operator approves the list in `evidence/G1.md`". · Impact: G1 row text only; REQ-TST-007 unchanged.
Status: APPROVED (operator, 2026-10-08 — design §9)
```

- [ ] **Step 2: Amend tasks.md**

In the T1.0 block replace the **Done when** line with:

```markdown
  - **Done when:** the acceptance target is declared `test = false`; the tests compile, except tests that call a Phase 1 API the author names in `tests/acceptance/README.md`, which may fail to compile only for that reason; each test fails for the right reason against the current code (record the failure output); and the operator approves them via CODEOWNERS review (CR-7).
```

In the T1.8 block change `**Files:** \`src/error.rs\` (new), \`src/control_api.rs\`, \`tests/error_mapping.rs\`` to:

```markdown
  - **Files:** `src/error.rs` (new), `src/lib.rs` (new, CR-7), `src/control_api.rs`, `src/main.rs` (imports), `tests/error_mapping.rs`
```

In the T2.1 block, prefix its **Do** text (or add one if absent) with: `extend \`src/lib.rs\` (created in T1.8, CR-7) with the empty modules …`.

Replace G1 row 6 with:

```markdown
| 6 | Tests not weakened | `git diff <G0 commit> -- tests/ '**/snapshots/**'` | every changed assertion, expected value, or snapshot sits in a commit that states why and carries a `Protected-change:` trailer; the operator approves the list in `evidence/G1.md` (CR-8) | REQ-TST-007 |
```

- [ ] **Step 3: Amend requirements.md**

REQ-ARCH-002/AC1 → `- **AC1** \`src/lib.rs\` SHALL expose the modules that integration tests use (created in T1.8, extended in T2.1 — CR-7). (I)`. Bump the document header's revision to `r2.5` and append to Appendix I:

```markdown
| r2.5 | 2026-10-08 | CR-7: REQ-ARCH-002/AC1 wording; T1.0 Done-when; T1.8 Files; T2.1 Do. CR-8: G1 row 6 pass condition. No IDs added or retired. |
```

(Match the exact column layout of the existing Appendix I rows; `grep -n -A4 '^## Appendix I' docs/specs/vnext/requirements.md` shows it.) Update `CLAUDE.md` line 6: `**Spec:** \`saltnitor-vnext\` r2.5`.

- [ ] **Step 4: Lint and self-test**

Run: `python3 docs/specs/vnext/tools/spec_lint.py --sync && python3 docs/specs/vnext/tools/spec_lint.py && python3 -m unittest discover -s docs/specs/vnext/tools`
Expected: `spec_lint: OK` and `OK` from unittest. If `--sync` rewrote generated fields, include them.

- [ ] **Step 5: Commit (protected change)**

```bash
git add docs/specs/vnext/CHANGE_REQUESTS.md docs/specs/vnext/tasks.md docs/specs/vnext/requirements.md CLAUDE.md
git commit -F - <<'MSG'
Spec r2.5: file CR-7 (lib.rs at T1.8, acceptance target test=false) and CR-8 (G1 row 6)

Refs: REQ-ARCH-002, REQ-TST-007, REQ-TST-012, REQ-DOC-007
Protected-change: docs/specs/vnext/{CHANGE_REQUESTS,tasks,requirements}.md — CR-7/CR-8 approved by the operator in the Phase 1 design (2026-10-08)

Co-Authored-By: Claude Mythos 5.1 <noreply@anthropic.com>
MSG
git push origin vnext
```

---

### Task 1: Write the T1.0 brief and hand off to the fresh session

**Files:**
- Create: `docs/superpowers/briefs/g1-acceptance-brief.md`
- Modify: `tests/acceptance/README.md` (how the suite is declared and run)

**Interfaces:**
- Produces: the contract the fresh session writes against. Nothing about implementation.

- [ ] **Step 1: Write the brief**

```markdown
# Brief — G1 acceptance tests (T1.0)

You are the **independent author** of the Phase 1 acceptance suite. You will not implement
Phase 1. Read `CLAUDE.md`, then `docs/specs/vnext/tasks.md` (T1.0 and the G1 table), then
these requirements in `docs/specs/vnext/requirements.md`: REQ-PRX-002, REQ-PRX-003,
REQ-PRX-006, REQ-SEC-001, REQ-SEC-005, REQ-CFG-003, REQ-PROC-003, REQ-TST-012, REQ-TST-014,
REQ-TST-016, Appendix A, Appendix B.

## What to prove (black-box, against the built binary + `tools/fake-llama-server`)
1. `POST /v1/chat/completions` with `"stream": true`: the first SSE chunk reaches the client
   before the upstream has sent the last one (fake scenario: chunks 2 000 ms apart).
2. Downstream body bytes == upstream body bytes (streaming and non-streaming; include SSE
   comments, `data: [DONE]`, tool-call deltas).
3. Client cancellation: close the client mid-stream; the fake records a `disconnect` within 1 s.
4. Every route in Appendix A except `/healthz` returns `401` with the JSON envelope
   (`{"error":{"code":"AUTH_REQUIRED",…}}`) when no bearer is sent, given a config with a token.
5. `GET /v1/ensure/stream?token=<the token>` without a header is refused by default.
6. A malformed config → exit code 2 and the configured `control_port` stays free. Two cases:
   (a) type error — `est_vram_gb = "9gb"` under `[profiles.x]` → stderr contains
   `<file>:<line>:<col>`, `profiles.x.est_vram_gb`, `expected float`, `found string "9gb"`;
   (b) unknown key — `ctx_size = 32768` under `[profiles.x]` (v1 profiles know only `model`,
   `offload`, `est_vram_gb`, `est_ram_gb`) → stderr contains `profiles.x.ctx_size` and
   `unknown key`. (Trap: clap exits 2 on unknown flags — assert the stderr text, not only the
   code.)
7. Terminating by PID through the process-control module API kills one of two same-named
   `sleep` processes and leaves the other alive. Name the API you call in
   `tests/acceptance/README.md` (e.g. `saltnitor::process::{Target, terminate}` with the
   signature you expect); it does not exist yet, so that one test may fail to compile.
8. One compositional scenario (`g1_compositional`): auth + streaming + request ID
   (`X-Request-Id` echoed, or generated when absent) + cancellation in a single flow.

## Harness facts
- The binary: `env!("CARGO_BIN_EXE_saltnitor")`. It starts a TUI and needs a pseudo-terminal:
  use the `portable-pty` crate (dev-dependency; `native_pty_system().openpty(PtySize{rows:40,
  cols:120,..})`, `slave.spawn_command(CommandBuilder)`), or `script -qfec "<cmd>" /dev/null`
  as a fallback. Pass `--config <tmp.toml>` (arrives with T1.7; until then the binary ignores it
  and reads `$HOME/.config/saltnitor/config.toml` — point `HOME` at a temp dir).
- Ports: bind `127.0.0.1:0`, read the port, release, pass it as `control_port`; the router
  base is the fake's `base_url()`.
- The fake: `fake_llama_server::spawn(Scenario)`; scenario TOML as in
  `tests/fixtures/scenarios/*.toml`; `handle.recorder.of_kind("disconnect")`.
- `/v1/ensure/stream` is GET-only until P6.
- The config token key is `control_token = "…"` (v1); a bearer is sent as
  `Authorization: Bearer <token>`.

## Rules
- Files: `tests/acceptance/g1_*.rs` as modules of one target `tests/acceptance/main.rs`;
  declare in `Cargo.toml`: `[[test]] name = "acceptance"; path = "tests/acceptance/main.rs";
  test = false`. Add the G1 rows to `scripts/gate.sh` (`gate_g1`) mirroring `gate_g0`.
- Every test carries `/// Verifies: REQ-…/ACn`. No sleeps where a poll/channel works; generous
  timeouts (≥ 10 s) for process start.
- Record the red output in `tests/acceptance/README.md` under "G1 red run".
- Work on a branch `p1/acceptance`, commit with a `Protected-change:` trailer, open a PR into
  `vnext`. Do not touch `src/`.
```

- [ ] **Step 2: Update `tests/acceptance/README.md`**

Append:

```markdown

## Declaration and invocation
The suite is one cargo test target, `acceptance` (`tests/acceptance/main.rs`, `test = false`),
so plain `cargo test` never runs it while it is red. Run it with `cargo test --test acceptance`
(informational in CI until G1; required at G1). G1 rows 2–3 call it.
```

- [ ] **Step 3: Commit and hand off**

```bash
git add docs/superpowers/briefs/g1-acceptance-brief.md tests/acceptance/README.md
git commit -F - <<'MSG'
T1.0 brief: behaviour-only briefing for the independent acceptance-test session

Refs: REQ-TST-012, REQ-TST-014, REQ-TST-016
Protected-change: tests/acceptance/README.md — invocation note only, no tests

Co-Authored-By: Claude Mythos 5.1 <noreply@anthropic.com>
MSG
git push origin vnext
```

**STOP.** Tell the operator: open a fresh Claude Code session in this repo and paste `docs/superpowers/briefs/g1-acceptance-brief.md`. Do not start Task 2 until the `p1/acceptance` PR is merged into `vnext` and T1.0 is ticked by that session (with its PROGRESS line). Then `git pull --ff-only origin vnext`.

---

### Task 2: T1.1 — Lockfile and toolchain

**Files:**
- Modify: `.gitignore` (remove `Cargo.lock`), `Cargo.toml` (`rust-version`)
- Create: `rust-toolchain.toml`; track `Cargo.lock`

**Interfaces:**
- Produces: a reproducible build every later task runs with `--locked`.

- [ ] **Step 1: Write the failing check**

Run: `git ls-files Cargo.lock | grep -q . && echo tracked || echo untracked; grep -n 'Cargo.lock' .gitignore`
Expected: `untracked` and the `.gitignore` line `Cargo.lock` (this is the red state).

- [ ] **Step 2: Make the change**

```bash
sed -i '/^Cargo\.lock$/d' .gitignore
printf '[toolchain]\nchannel = "1.95.0"\ncomponents = ["rustfmt", "clippy"]\nprofile = "minimal"\n' > rust-toolchain.toml
```

In `Cargo.toml` `[package]` add after `edition = "2024"`:

```toml
rust-version = "1.95"   # sysinfo 0.39 requires 1.95; CI's msrv job builds with exactly this
```

- [ ] **Step 3: Verify**

Run (ctx_execute): `cargo build --locked && cargo test --workspace --locked`
Expected: success, no lockfile change (`git diff --stat Cargo.lock` empty after `git add Cargo.lock`).
Run: `git ls-files Cargo.lock` after `git add Cargo.lock` → prints the path; `grep -n "Cargo.lock" .gitignore` → nothing.

- [ ] **Step 4: Run Close-out** for T1.1 (`Refs: REQ-REPO-001, REQ-REPO-008, REQ-REL-006`; note `Cargo.lock tracked (BD-08 fixed); rust-version 1.95; toolchain pinned`). `git add .gitignore Cargo.toml Cargo.lock rust-toolchain.toml docs/specs/vnext/tasks.md docs/specs/vnext/PROGRESS.md`.

---

### Task 3: T1.2 — Repository hygiene

**Files:**
- Remove from index: `.saltnitor_history`, `crash_dump_20260511_163653.txt`, `.vscode/settings.json`, `.vscode/web-timemanager.json`, `legacy.zip` (after the diff below)
- Rename: `Contributing_Guidelines.md` → `CONTRIBUTING.md`
- Create: `SECURITY.md`, `CHANGELOG.md`
- Modify: `.gitignore`

- [ ] **Step 1: Red check**

Run: `git ls-files | grep -E 'saltnitor_history|crash_dump_|legacy\.zip|\.vscode/'`
Expected: five paths listed.

- [ ] **Step 2: Decide the legacy archive (evidence, not guesswork)**

```bash
tmp=$(mktemp -d) && unzip -q legacy.zip -d "$tmp"
for f in main events app ui; do
  found=no
  for rev in $(git log --all --format=%h -- "src/$f.rs"); do
    if git show "$rev:src/$f.rs" 2>/dev/null | cmp -s - "$tmp/legacy/$f.rs"; then found=$rev; break; fi
  done
  echo "legacy/$f.rs: $found"
done
```

Expected: every line ends with a commit hash. **If any line says `no`:** `BLOCKED: T1.2 — legacy/<f>.rs has no identical version in history. Options: A tag legacy-archive at HEAD with the file content committed under docs/legacy/; B drop it. Need: decision.` and stop. If all match: continue (the decision goes into CHANGELOG).

- [ ] **Step 3: Apply**

```bash
git rm -q --cached .saltnitor_history crash_dump_20260511_163653.txt .vscode/settings.json .vscode/web-timemanager.json
git rm -q legacy.zip
git mv Contributing_Guidelines.md CONTRIBUTING.md
cat >> .gitignore <<'EOF'
# local state and editor files (T1.2)
.saltnitor_history
crash_dump_*.txt
saltnitor_crash_*.txt
.vscode/
.obsidian/
.claude/settings.local.json
__pycache__/
EOF
```

`SECURITY.md` (stub, completed in T1.16):

```markdown
# Security

Saltnitor manages a local llama.cpp runtime and exposes an OpenAI-compatible control API on
loopback. The threat model and reporting process are completed in v0.2.0 (T1.16).

## Reporting
Open a private security advisory on the repository, or email the maintainer named in
`Cargo.toml`. Do not file public issues for vulnerabilities.
```

`CHANGELOG.md` (Keep a Changelog):

```markdown
# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow SemVer.

## [Unreleased]

### Removed
- `legacy.zip` (pre-control-API sources from May 2026): every file matched a committed
  revision of `src/` (checked with `git log --all -p -- src/`), so the archive was dropped
  without a tag.
- Tracked local state (`.saltnitor_history`, a crash dump, `.vscode/`) — now ignored.

### Changed
- `Contributing_Guidelines.md` → `CONTRIBUTING.md`.

## [0.1.0] — 2026-05-23
Baseline (`master` @ `c89f278`). See `docs/specs/vnext/baseline/`.
```

- [ ] **Step 4: Verify**

Run: `git ls-files | grep -E 'saltnitor_history|crash_dump_|legacy\.zip|\.vscode/'` → empty. `test -f .vscode/web-timemanager.json && echo "local copy kept"` → kept (the operator's edit survives). `cargo test --workspace --locked` → green.

- [ ] **Step 5: Run Close-out** for T1.2 (`Refs: REQ-REPO-002, REQ-REPO-003, REQ-REPO-006, REQ-DOC-003`). Add paths: `.gitignore CONTRIBUTING.md SECURITY.md CHANGELOG.md` plus the removals already staged.

---

### Task 4: T1.3 — CI pipeline (three commits)

**Files:**
- Modify: `src/*.rs` (fmt, then clippy fixes), `Cargo.toml` (`[lints.clippy]`, `[workspace.lints]`), `tools/fake-llama-server/Cargo.toml` (`[lints] workspace = true`)
- Create: `.github/workflows/ci.yml`, `clippy.toml`, `deny.toml`, `scripts/check-test-counts.sh`
- Delete: `.github/workflows/rust.yml`

**Interfaces:**
- Produces: `scripts/check-test-counts.sh` (exit 1 when any suite in `docs/specs/vnext/evidence/test-baseline.txt` has fewer tests than its minimum); the lint set every later task must satisfy.

- [ ] **Step 1 (commit ①): format only**

Run: `cargo fmt --all && cargo test --workspace --locked`
Expected: tests green, snapshots unchanged (`git status --short src/snapshots` empty). If `cargo fmt` touched `src/snapshots/` or any `.snap`: stop, revert, investigate — fmt must not reach snapshots.

```bash
git add src tools
git commit -q -m "T1.3 (1/3): cargo fmt --all, no behavior change

Task: T1.3
Refs: REQ-CI-001

Co-Authored-By: Claude Mythos 5.1 <noreply@anthropic.com>"
```

- [ ] **Step 2 (commit ②): fix the 11 clippy warnings**

Run (ctx_execute): `cargo clippy --all-targets --all-features --locked -- -D warnings 2>&1`
Fix each of the baseline warnings (`docs/specs/vnext/baseline/clippy-baseline.txt`) **without changing behaviour**:

| Warning | Fix |
|---|---|
| `main.rs` / `control_api.rs` / `ui.rs`: `this if statement can be collapsed` | collapse with `&&` / `let … && …` (edition 2024 let-chains) |
| `main.rs`: `this if can be collapsed into the outer match` | move the condition into the match arm guard |
| `main.rs`: `called Iterator::last on a DoubleEndedIterator` | `.next_back()` |
| `main.rs`: `consider using sort_by_key` | `sort_by_key(|p| std::cmp::Reverse(p.memory()))` (keeps descending order) |
| `main.rs`: `stripping a prefix manually` | `strip_prefix` |
| `main.rs` / `ui.rs`: `the loop variable i is used to index` | `iter().enumerate()` |
| `app.rs`: `too many arguments (11/7)` on `App::new` | `#[expect(clippy::too_many_arguments, reason = "App::new is restructured by the P2 TUI decomposition (T2.7)")]` |
| `events.rs`: `large size difference between variants` | `#[expect(clippy::large_enum_variant, reason = "Event is split per screen in P2 (T2.7)")]` on the enum |

Re-run clippy until clean. Run `cargo test --workspace --locked` — all green, snapshots unchanged. Commit as `T1.3 (2/3): clippy baseline fixed, no behavior change` (same trailer block).

- [ ] **Step 3 (commit ③): lints, deny, counts script, workflow**

`Cargo.toml` additions:

```toml
[lints]
workspace = true

[workspace.lints.clippy]
todo = "deny"
unimplemented = "deny"
dbg_macro = "deny"
unwrap_used = "deny"
expect_used = "warn"
```

(`tools/fake-llama-server/Cargo.toml` is **test tooling**, outside REQ-CI-007's "non-test code": give it its own table `[lints.clippy]\ntodo = "deny"\nunimplemented = "deny"\ndbg_macro = "deny"` and leave `unwrap_used`/`expect_used` alone there.) Tests keep `unwrap`: add `#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]` is not possible per-module from the workspace table, so put at the top of every `#[cfg(test)] mod tests` block and every file under `tests/`: `#![allow(clippy::unwrap_used, clippy::expect_used)]` with the comment `// tests may unwrap: a panic is the failure signal (REQ-CI-007 scopes the deny to non-test code)`. For the production unwraps that T1.14 removes, add at the top of `src/main.rs` and `src/control_api.rs`:

```rust
#![expect(clippy::unwrap_used, clippy::expect_used, reason = "removed by T1.14 (REQ-ERR-004); expect warns once no unwrap remains")]
```

(`-D warnings` turns the `expect_used = "warn"` entry into an error, so the attribute must name both lints.)

`clippy.toml`:

```toml
# Per-function size is enforced from P2 (REQ-ARCH-001); P1 only records the ceiling.
too-many-lines-threshold = 100
```

`deny.toml`:

```toml
[graph]
all-features = true

[advisories]
version = 2
yanked = "deny"

[licenses]
version = 2
allow = [
  "MIT", "Apache-2.0", "Apache-2.0 WITH LLVM-exception", "BSD-2-Clause", "BSD-3-Clause",
  "ISC", "Zlib", "Unicode-3.0", "MPL-2.0", "CC0-1.0", "0BSD", "OpenSSL",
]
confidence-threshold = 0.9

[bans]
multiple-versions = "warn"
wildcards = "deny"

[sources]
unknown-registry = "deny"
unknown-git = "deny"
```

`scripts/check-test-counts.sh` (shellcheck-clean):

```bash
#!/usr/bin/env bash
# Fail when any suite runs fewer tests than docs/specs/vnext/evidence/test-baseline.txt allows.
# Verifies: REQ-CI-005/AC1
set -euo pipefail
ROOT="$(git rev-parse --show-toplevel)"; cd "$ROOT"
BASE=docs/specs/vnext/evidence/test-baseline.txt
[[ -f $BASE ]] || { echo "check-test-counts: missing $BASE"; exit 2; }
cargo test --workspace --locked -- --list 2>&1 | python3 - "$BASE" <<'PY'
import re, sys
base = {}
for line in open(sys.argv[1], encoding="utf-8"):
    if "\t" in line and not line.startswith("#"):
        name, n = line.rstrip("\n").split("\t"); base[name] = int(n)
now, suite = {}, None
for line in sys.stdin:
    m = re.match(r"\s*Running (\S+(?: \S+)?) \(\S*/deps/(.+?)-[0-9a-f]+\)", line)
    if m: suite = f"{m.group(2)} {m.group(1)}"; continue
    m = re.match(r"(\d+) tests?, \d+ benchmarks?$", line.strip())
    if m and suite: now[suite] = int(m.group(1)); suite = None
bad = [f"{s}: {now.get(s, 'missing')} < {n}" for s, n in base.items() if now.get(s, -1) < n]
print("\n".join(bad) or "test counts >= baseline for every suite")
sys.exit(1 if bad else 0)
PY
```

`.github/workflows/ci.yml` (replaces `rust.yml`; `git rm .github/workflows/rust.yml`):

```yaml
name: CI
on:
  push: { branches: [master, vnext] }
  pull_request: { branches: [master, vnext] }
  workflow_dispatch:
permissions: { contents: read }
concurrency:
  group: ci-${{ github.ref }}
  cancel-in-progress: true
env: { CARGO_TERM_COLOR: always, CARGO_INCREMENTAL: "0" }

jobs:
  fmt:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: dtolnay/rust-toolchain@1.95.0
        with: { components: rustfmt }
      - run: cargo fmt --all --check   # Verifies: REQ-CI-001/AC1

  clippy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: dtolnay/rust-toolchain@1.95.0
        with: { components: clippy }
      - uses: Swatinem/rust-cache@v2
      # Verifies: REQ-CI-001/AC1, REQ-CI-007/AC1, REQ-DOC-006/AC2 (placeholder lints are deny-level in Cargo.toml)
      - run: cargo clippy --all-targets --all-features --locked -- -D warnings

  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: dtolnay/rust-toolchain@1.95.0
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --all --locked           # Verifies: REQ-CI-001/AC1, REQ-CI-003/AC1 (protocol tests run against the fake runtime)
      - run: scripts/check-test-counts.sh        # Verifies: REQ-CI-005/AC1
      - run: cargo test --workspace --locked --features hardware-tests --no-run   # Verifies: REQ-CI-002/AC1 (compiles, never runs ignored HW tests)

  build-release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: dtolnay/rust-toolchain@1.95.0
      - uses: Swatinem/rust-cache@v2
      - run: cargo build --release --locked      # Verifies: REQ-CI-001/AC1

  msrv:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: dtolnay/rust-toolchain@1.95.0      # Verifies: REQ-REPO-008/AC1 (rust-version in Cargo.toml)
      - run: cargo build --locked

  deny:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: EmbarkStudios/cargo-deny-action@v2  # Verifies: REQ-CI-006/AC1, REQ-ARCH-008/AC1
        with: { command: check }

  spec:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - run: python3 docs/specs/vnext/tools/spec_lint.py            # Verifies: REQ-DOC-006/AC2
      - run: python3 -m unittest discover -s docs/specs/vnext/tools

  acceptance:
    runs-on: ubuntu-latest
    continue-on-error: true   # informational until G1 (T1.0 Do)
    steps:
      - uses: actions/checkout@v7
      - uses: dtolnay/rust-toolchain@1.95.0
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --test acceptance --locked
```

Add the feature to `Cargo.toml`:

```toml
[features]
hardware-tests = []   # REQ-CI-002: GPU / real-runtime tests sit behind this + #[ignore]
```

- [ ] **Step 4: Verify locally**

```bash
cargo install cargo-deny --locked      # once; ~2 min
cargo deny check                        # fix: add only permissive licenses the output names; anything copyleft/unknown → BLOCKED
chmod +x scripts/check-test-counts.sh && ~/.local/bin/shellcheck scripts/check-test-counts.sh && scripts/check-test-counts.sh
cargo fmt --all --check && cargo clippy --all-targets --all-features --locked -- -D warnings && cargo test --workspace --locked
```

Expected: all exit 0. **Test-count suites:** if moving nothing yet, names are unchanged (lib.rs arrives in T1.8).

- [ ] **Step 5: Commit ③ and prove CI fails on `todo!()`**

Commit as `T1.3 (3/3): CI pipeline (fmt, clippy, test+counts, release, msrv, deny, spec, acceptance)` with `Refs: REQ-CI-001, REQ-CI-002, REQ-CI-003, REQ-CI-005, REQ-CI-006, REQ-CI-007, REQ-DOC-006, REQ-ARCH-008, REQ-REPO-008`. Push `vnext`; wait for green (`gh run watch`).

```bash
git checkout -q -b scratch/t1.3-todo
printf '\nfn _t13_probe() { todo!() }\n' >> src/events.rs
git commit -qam "scratch: todo!() probe (never merged)" && git push -q -u origin scratch/t1.3-todo
gh workflow run ci.yml --ref scratch/t1.3-todo && sleep 20
run=$(gh run list --branch scratch/t1.3-todo --workflow ci.yml --limit 1 --json databaseId,url --jq '.[0]')
echo "$run"; gh run watch "$(echo "$run" | jq -r .databaseId)" || true   # expected: clippy job FAILS
git checkout -q vnext && git branch -qD scratch/t1.3-todo && git push -q origin --delete scratch/t1.3-todo
```

Record the run URL in the T1.3 PROGRESS note (`todo!() probe failed CI: <url>`).

- [ ] **Step 6: Run Close-out** for T1.3 (the tick + PROGRESS line; SV now includes fmt and clippy).

---

### Task 5: T1.4 — Release pipeline

**Files:**
- Modify: `.github/workflows/release.yml` (rewrite)

- [ ] **Step 1: Replace the workflow**

```yaml
name: Release
on:
  release: { types: [published] }
  workflow_dispatch:
    inputs:
      dry_run: { description: "Build + checksums only (no upload to a release)", type: boolean, default: true }
permissions: { contents: read }

jobs:
  build:
    runs-on: ubuntu-latest
    permissions:
      contents: write        # gh release upload (real releases only)
      id-token: write        # Verifies: REQ-REL-006/AC1 (provenance attestation)
      attestations: write
    steps:
      - uses: actions/checkout@v7
      - uses: dtolnay/rust-toolchain@1.95.0   # pinned toolchain (REQ-REL-006/AC1)
      - run: cargo build --release --locked   # Verifies: REQ-CI-004/AC1
      - name: Package and checksum
        run: |
          mkdir -p dist
          cp target/release/saltnitor dist/saltnitor-linux-amd64
          (cd dist && sha256sum saltnitor-linux-amd64 > SHA256SUMS)   # Verifies: REQ-CI-004/AC1
          cat dist/SHA256SUMS
      - uses: actions/attest-build-provenance@v4
        with: { subject-path: dist/saltnitor-linux-amd64 }
      - uses: actions/upload-artifact@v7
        with: { name: saltnitor-${{ github.sha }}, path: dist/ }
      - name: Upload to the GitHub release
        if: github.event_name == 'release'      # Verifies: REQ-CI-004/AC2 (gh CLI, not the archived upload-release-asset)
        env: { GH_TOKEN: "${{ github.token }}" }
        run: gh release upload "${{ github.event.release.tag_name }}" dist/saltnitor-linux-amd64 dist/SHA256SUMS --clobber
```

- [ ] **Step 2: Verify the dry run**

```bash
git add .github/workflows/release.yml && git commit -q -m "T1.4: release pipeline (locked build, SHA256SUMS, attestation, gh release upload)

Task: T1.4
Refs: REQ-CI-004

Co-Authored-By: Claude Mythos 5.1 <noreply@anthropic.com>" && git push -q origin vnext
gh workflow run release.yml --ref vnext -f dry_run=true && sleep 20
id=$(gh run list --workflow release.yml --branch vnext --limit 1 --json databaseId --jq '.[0].databaseId'); gh run watch "$id" --exit-status
gh run download "$id" -D /tmp/claude-1000/-home-laz-saltnitor/*/scratchpad/rel && cat /tmp/claude-1000/-home-laz-saltnitor/*/scratchpad/rel/*/SHA256SUMS
```

Expected: run succeeds; `SHA256SUMS` has one line for `saltnitor-linux-amd64`; `grep -c upload-release-asset .github/workflows/release.yml` → 0. Record the run URL in PROGRESS.

- [ ] **Step 3: Run Close-out** for T1.4 (amend the tick + PROGRESS into a follow-up commit).

---

### Task 6: T1.5 — Invariant scan with ratchet

**Files:**
- Create: `scripts/check-invariants.sh`, `scripts/invariants-baseline.txt`, `scripts/test-check-invariants.sh`
- Modify: `.github/workflows/ci.yml` (new `invariants` job)

**Interfaces:**
- Produces: `scripts/check-invariants.sh` exit 0/1; output lines `RULE <id> <file>:<line>: <message>\n  fix: <remediation>`; inline marker `invariants: allow <rule-id> — <reason>` on the same line exempts it.

- [ ] **Step 1: Write the self-test first (red)**

`scripts/test-check-invariants.sh`:

```bash
#!/usr/bin/env bash
# Self-test: every rule must fire on a planted violation and stay quiet on the marker.
# Verifies: REQ-CI-008/AC1, REQ-CI-008/AC2
set -euo pipefail
ROOT="$(git rev-parse --show-toplevel)"
work=$(mktemp -d); trap 'rm -rf "$work"' EXIT
git -C "$ROOT" worktree add -q "$work/wt" HEAD
cd "$work/wt"
: > scripts/invariants-baseline.txt      # empty ratchet: everything must fire
fail=0
check() { # rule, planted text, file
  printf '%s\n' "$2" >> "$3"
  if out=$(scripts/check-invariants.sh 2>&1); then echo "FAIL: $1 did not fire"; fail=1
  elif ! grep -q "^RULE $1 " <<<"$out" || ! grep -q "  fix: " <<<"$out"; then echo "FAIL: $1 fired without rule id or remediation"; echo "$out"; fail=1
  else echo "ok: $1"; fi
  git checkout -q -- "$3" 2>/dev/null || rm -f "$3"
}
check kill-by-name 'let _ = Command::new("killall").arg("x");' src/events.rs
check home-path    'const P: &str = "/home/someone/x";' src/events.rs
check secret       'const K: &str = "sk-abcdefghijklmnop";' src/events.rs
check build-tool   'Command::new("cmake").arg("..");' src/events.rs
check artifact     'tracked' router.ini
# marker suppresses
printf '%s\n' 'const P: &str = "/home/someone/x"; // invariants: allow home-path — sanitizer test input' >> src/events.rs
if scripts/check-invariants.sh >/dev/null 2>&1; then echo "ok: marker honoured"; else echo "FAIL: marker not honoured"; fail=1; fi
git checkout -q -- src/events.rs
# ratchet only shrinks: a listed entry that no longer occurs fails
printf 'home-path\tsrc/events.rs:9999\n' > scripts/invariants-baseline.txt
if scripts/check-invariants.sh >/dev/null 2>&1; then echo "FAIL: stale ratchet entry accepted"; fail=1; else echo "ok: stale ratchet entry rejected"; fi
cd "$ROOT" && git worktree remove -f "$work/wt"
exit $fail
```

Run: `chmod +x scripts/test-check-invariants.sh && scripts/test-check-invariants.sh`
Expected: fails (`check-invariants.sh: No such file`).

- [ ] **Step 2: Write the scanner**

`scripts/check-invariants.sh`:

```bash
#!/usr/bin/env bash
# Invariant scan (REQ-CI-008/AC1): fails on new violations; pre-existing ones live in the ratchet
# file scripts/invariants-baseline.txt (rule<TAB>file:line), which may only shrink (AC3).
# Each failure prints the rule, the location, and how to fix it (AC2).
# Exempt one line with: invariants: allow <rule> — <reason>
set -uo pipefail
ROOT="$(git rev-parse --show-toplevel)"; cd "$ROOT"
BASELINE=scripts/invariants-baseline.txt
declare -A FIX=(
  [kill-by-name]="signal an exact PID through src/process.rs (INV-09); never killall/pkill"
  [home-path]="read the path from config (router_ini, XDG dirs) or an env var; never a user's home (INV-16)"
  [secret]="load the key from <key>_env / <key>_file (REQ-SEC-003); never a literal"
  [build-tool]="runtime channels are built by the operator; src/ never runs git/cmake/make (INV-12)"
  [artifact]="untrack it (git rm --cached) and add it to .gitignore (REQ-REPO-002)"
)
hits=()   # "rule<TAB>file:line<TAB>message"
scan() { # rule, grep -E pattern, pathspec...
  local rule=$1 pat=$2; shift 2
  while IFS= read -r line; do
    [[ $line == *"invariants: allow $rule"* ]] && { echo "marker: $line"; continue; }
    hits+=("$rule"$'\t'"${line%%:*}:$(cut -d: -f2 <<<"$line")"$'\t'"$(cut -d: -f3- <<<"$line")")
  done < <(git grep -nE "$pat" -- "$@" 2>/dev/null || true)
}
scan kill-by-name '\b(killall|pkill)\b' 'src/' 'tools/*/src/'
scan home-path    '/home/[a-z_][a-z0-9_-]*/' ':!docs/specs' ':!*.md' ':!tests/fixtures/captures'
scan secret       '(sk-[A-Za-z0-9]{8,}|Bearer [A-Za-z0-9._-]{12,}|api[_-]?key *= *"[^"$<]{8,}")' ':!docs/specs' ':!*.md'
scan build-tool   'Command::new\("(git|cmake|make)"\)' 'src/'
for f in $(git ls-files -- 'router.ini' '*.gguf' 'target/' 'saltnitor_crash_*' '.saltnitor_history' 'crash_dump_*'); do
  hits+=("artifact"$'\t'"$f:1"$'\t'"tracked runtime artifact")
done
# ratchet
declare -A base=(); while IFS=$'\t' read -r r loc; do [[ -n $r && $r != \#* ]] && base["$r"$'\t'"$loc"]=1; done < "$BASELINE"
declare -A seen=()
status=0
for h in "${hits[@]}"; do
  IFS=$'\t' read -r rule loc msg <<<"$h"; key="$rule"$'\t'"$loc"; seen["$key"]=1
  [[ -n ${base[$key]:-} ]] && continue
  printf 'RULE %s %s: %s\n  fix: %s\n' "$rule" "$loc" "$msg" "${FIX[$rule]}"; status=1
done
for key in "${!base[@]}"; do
  if [[ -z ${seen[$key]:-} ]]; then
    IFS=$'\t' read -r rule loc <<<"$key"
    printf 'RULE ratchet %s: baseline entry no longer occurs (%s)\n  fix: delete the line from %s — the ratchet only shrinks (REQ-CI-008/AC3)\n' "$loc" "$rule" "$BASELINE"; status=1
  fi
done
[[ $status -eq 0 ]] && echo "invariants: clean (${#base[@]} ratcheted)"
exit $status
```

- [ ] **Step 3: Seed the ratchet with today's violations**

```bash
chmod +x scripts/check-invariants.sh; : > scripts/invariants-baseline.txt
scripts/check-invariants.sh | awk '/^RULE [a-z-]+ /{print $2 "\t" substr($3,1,length($3)-1)}' > scripts/invariants-baseline.txt
scripts/check-invariants.sh      # expected: "invariants: clean (N ratcheted)"
~/.local/bin/shellcheck scripts/check-invariants.sh scripts/test-check-invariants.sh
scripts/test-check-invariants.sh # expected: every line "ok:", exit 0
```

Expected ratchet entries: `kill-by-name` ×3 (`src/main.rs` 83, 468, 494 region), `home-path` ×2 (`src/main.rs:578`, `tools/fake-llama-server/src/record.rs:418`), `secret` ×2 (`src/main.rs` 660, 748) — plus `launch_router.sh`/`test_control_api.sh` lines until T1.6 moves them. Line numbers are whatever the scan prints after T1.3's fmt.

- [ ] **Step 4: Wire into CI**

Append to `ci.yml`:

```yaml
  invariants:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - run: scripts/check-invariants.sh          # Verifies: REQ-CI-008/AC1, REQ-CI-008/AC3
      - run: scripts/test-check-invariants.sh     # Verifies: REQ-CI-008/AC2
```

- [ ] **Step 5: Prove it on a scratch branch**

Commit (`T1.5: invariant scan with ratchet`, `Refs: REQ-CI-008, REQ-REPO-004, REQ-REPO-005, REQ-PROC-006`), push, wait green. Then:

```bash
git checkout -q -b scratch/t1.5-violation
printf '\nconst _T15: &str = "/home/probe/x";\n' >> src/events.rs
git commit -qam "scratch: planted invariant violation (never merged)" && git push -q -u origin scratch/t1.5-violation
gh workflow run ci.yml --ref scratch/t1.5-violation && sleep 20
id=$(gh run list --branch scratch/t1.5-violation --workflow ci.yml --limit 1 --json databaseId --jq '.[0].databaseId')
gh run watch "$id" || true; gh run view "$id" --log-failed | grep -A1 '^.*RULE home-path'   # expected: the rule + fix line
git checkout -q vnext && git branch -qD scratch/t1.5-violation && git push -q origin --delete scratch/t1.5-violation
```

- [ ] **Step 6: Run Close-out** for T1.5 (PROGRESS note includes the scratch run URL and `N ratcheted`).

---

### Task 7: T1.6 — Remove hardcoded credentials and paths

**Files:**
- Modify: `src/main.rs` (`TomlConfig` + tuner + hot-swap + interrogator bearer), `CLAUDE.md`, `docs/superpowers/specs/2026-09-28-phase0-baseline-design.md`, `docs/superpowers/plans/2026-09-28-phase0-baseline.md`, `tools/fake-llama-server/src/record.rs` (test string), `README.md` (example key), `scripts/invariants-baseline.txt`
- Move: `launch_router.sh` → `examples/external-mode/launch_router.sh`; `test_control_api.sh` → `scripts/smoke_control_api.sh`
- Test: `src/main.rs` `#[cfg(test)]` (bearer helper), `scripts/test-check-invariants.sh` (unchanged)

**Interfaces:**
- Produces: config keys `router_ini: Option<String>`, `client_key_env: Option<String>` on `TomlConfig` (T1.7 carries them into `ConfigV1`); `fn client_bearer(env_name: Option<&str>) -> Option<String>` in `main.rs` (moved to `config_v1` in T1.7).

- [ ] **Step 1: Failing unit test for the bearer helper**

Append to the `#[cfg(test)] mod tests` in `src/main.rs`:

```rust
    /// Verifies: REQ-SEC-013/AC1
    #[test]
    fn client_bearer_comes_only_from_the_named_env_var() {
        // SAFETY (test): single-threaded access to this unique variable name.
        unsafe { std::env::set_var("SALTNITOR_T16_KEY", "k-from-env") };
        assert_eq!(client_bearer(Some("SALTNITOR_T16_KEY")).as_deref(), Some("k-from-env"));
        assert_eq!(client_bearer(Some("SALTNITOR_T16_MISSING")), None);
        assert_eq!(client_bearer(None), None);
    }
```

Run: `cargo test --bin saltnitor client_bearer` → fails to compile (`client_bearer` undefined).

- [ ] **Step 2: Implement**

In `src/main.rs`:

```rust
/// The bearer the TUI sends on its own HTTP calls: read from the env var named by
/// `client_key_env` (REQ-SEC-013). Never a literal, never logged.
fn client_bearer(env_name: Option<&str>) -> Option<String> {
    env_name.and_then(|n| std::env::var(n).ok()).filter(|v| !v.is_empty())
}
```

`TomlConfig` gains `router_ini: Option<String>` and `client_key_env: Option<String>` (both `Option`, default None). Replace the three literals:

- `main.rs:578` `const ROUTER_INI: &str = "/home/laz/…"` → the tuner reads `app.router_ini: Option<String>` (new `App` field, set from `toml_conf.router_ini` in `main`). When `None`: `app.add_log(">>> TUNER: refusing to apply — set `router_ini` in config.toml (REQ-SEC-013, DEC-04)")` and return without spawning.
- `main.rs:660` and `:748` `req.header("Authorization", "Bearer sk-saltnitor-2026")` → `if let Some(b) = client_bearer_value.clone() { req = req.header("Authorization", format!("Bearer {b}")); }` where `client_bearer_value = client_bearer(toml_conf.client_key_env.as_deref())` is computed once in `main` and stored on `App` as `client_bearer: Option<String>`; the `k` toggle (`app.api_key`) still gates whether the header is added.

Move the scripts with `git mv`; in both, replace literal tokens with `"${SALTNITOR_CONTROL_TOKEN:?set SALTNITOR_CONTROL_TOKEN}"` / `"${LLAMA_API_KEY:-}"` and paths with `"${LLAMA_HOME:?set LLAMA_HOME (e.g. \$HOME/ai-models/llama.cpp)}"`. Replace `README.md:111` `infer_bearer = "sk-saltnitor-2026"` with `infer_bearer = "<router api-key if the router runs with --api-key>"`. Replace `/home/laz` in `CLAUDE.md:195` with `the configured \`router_ini\``; in the two P0 docs replace `/home/laz/` with `/home/<user>/`; in `record.rs:418` change the test input to `/home/someone/ai-models/x` and add the marker ` // invariants: allow home-path — sanitizer test input`.

- [ ] **Step 3: Verify**

```bash
cargo test --workspace --locked && cargo clippy --all-targets --all-features --locked -- -D warnings && cargo fmt --all --check
git grep -n -e 'sk-saltnitor-2026' -e '/home/laz' -- ':!docs/specs'     # expected: empty
scripts/check-invariants.sh                                                 # expected: RULE ratchet … for every fixed entry
```

Delete the now-stale ratchet lines it names (the file only shrinks), re-run → `invariants: clean (N ratcheted)` with N smaller than before (record both numbers). `~/.local/bin/shellcheck scripts/smoke_control_api.sh examples/external-mode/launch_router.sh` clean.

- [ ] **Step 4: Run Close-out** for T1.6 (`Refs: REQ-SEC-013, REQ-REPO-004, REQ-REPO-005, REQ-DOC-004`). PROGRESS note: `ratchet 9 → 2` (actual numbers). **Tell the operator** (in the final message): add `router_ini = "<path>/router.ini"` and optionally `client_key_env = "SALTNITOR_CONTROL_TOKEN"` to `~/.config/saltnitor/config.toml`, or the tuner refuses to apply.

---

### Task 8: T1.7 — Strict config loading (schema v1)

**Files:**
- Create: `src/config_v1.rs`, `tests/config_strict.rs`, `tests/data/live-shape.toml`
- Modify: `src/main.rs` (`Cli --config`, replace `TomlConfig`/`load_config`, startup order)
- Test: unit tests inside `src/config_v1.rs`; black-box tests in `tests/config_strict.rs`

**Interfaces:**
- Produces: `config_v1::{ConfigV1, ProfileV1, Timeouts, ProcessCfg, ConfigError, Loaded, Source, load, parse, resolve_path, expand_path, SCHEMA_VERSION, DEFAULT_MAX_BODY_BYTES}` with the signatures below. Later tasks read `cfg.timeouts`, `cfg.max_body_bytes`, `cfg.process.term_grace_ms`, `cfg.allow_query_token`, `cfg.control_token{,_env,_file}`, `cfg.router_ini`, `cfg.client_key_env`.
- Consumes: `client_bearer` from Task 7 (moves here as `config_v1::client_bearer`).

- [ ] **Step 1: Failing unit tests (write the module skeleton with the tests only)**

Create `src/config_v1.rs` containing only the `#[cfg(test)]` module below plus `use` lines; declare `mod config_v1;` in `main.rs`. The tests reference items that do not exist yet → red.

```rust
#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)] // tests may unwrap: a panic is the failure signal
    use super::*;
    use std::collections::HashMap;

    fn env(map: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + '_ {
        move |k| map.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string())
    }

    /// Verifies: REQ-CFG-003/AC2
    #[test]
    fn type_error_reports_file_line_col_key_expected_found() {
        let text = "port = 8080\n[profiles.qwen36]\nmodel = \"m.gguf\"\nest_vram_gb = \"9gb\"\n";
        let e = parse(text, Path::new("config.toml"), &env(&[])).unwrap_err();
        let s = e.to_string();
        assert!(s.starts_with("config.toml:4:"), "line is required: {s}");
        assert!(s.ends_with(": profiles.qwen36.est_vram_gb: expected float, found string \"9gb\""), "{s}");
        assert!(e.col.is_some(), "column is required (REQ-CFG-003/AC2)");
    }

    /// Verifies: REQ-CFG-004/AC1, REQ-CFG-004/AC2
    #[test]
    fn unknown_key_is_rejected_with_the_closest_known_key() {
        let text = "[profiles.a]\nmodel = \"m\"\nest_vram_bg = 1.0\n";
        let e = parse(text, Path::new("c.toml"), &env(&[])).unwrap_err();
        assert_eq!(e.key, "profiles.a.est_vram_bg");
        assert_eq!(e.found, "unknown key `est_vram_bg`");
        assert_eq!(e.hint.as_deref(), Some("did you mean `est_vram_gb`?"));
        assert!(e.line.is_some(), "unknown keys carry a span: {e}");
    }

    /// Verifies: REQ-CFG-005/AC2
    #[test]
    fn unsupported_schema_version_is_config_invalid() {
        let e = parse("schema_version = 2\n", Path::new("c.toml"), &env(&[])).unwrap_err();
        assert_eq!((e.key.as_str(), e.expected.as_str(), e.found.as_str()), ("schema_version", "1", "2"));
    }

    /// Verifies: REQ-CFG-007/AC1
    #[test]
    fn paths_expand_tilde_vars_braces_and_resolve_relative_to_the_config_dir() {
        let e = env(&[("HOME", "/h"), ("X", "ex")]);
        assert_eq!(expand_path("~/a", Path::new("/cfg"), &e).unwrap(), "/h/a");
        assert_eq!(expand_path("$X/b", Path::new("/cfg"), &e).unwrap(), "/cfg/ex/b");
        assert_eq!(expand_path("/p/${X}_q", Path::new("/cfg"), &e).unwrap(), "/p/ex_q");
        assert_eq!(expand_path("rel/r.ini", Path::new("/cfg"), &e).unwrap(), "/cfg/rel/r.ini");
    }

    /// Verifies: REQ-CFG-007/AC2
    #[test]
    fn undefined_variable_in_a_path_is_config_invalid() {
        let text = "router_ini = \"$NOPE/router.ini\"\n";
        let e = parse(text, Path::new("c.toml"), &env(&[])).unwrap_err();
        assert_eq!(e.key, "router_ini");
        assert_eq!(e.found, "undefined variable NOPE");
    }

    /// Verifies: REQ-CFG-001/AC1, REQ-CFG-001/AC3
    #[test]
    fn resolution_prefers_flag_then_xdg_then_home_and_ignores_sudo_user() {
        let e = env(&[("HOME", "/me"), ("XDG_CONFIG_HOME", "/xdg"), ("SUDO_USER", "root")]);
        assert_eq!(resolve_path(Some(Path::new("/f.toml")), &e), (PathBuf::from("/f.toml"), Source::Flag));
        assert_eq!(resolve_path(None, &e), (PathBuf::from("/xdg/saltnitor/config.toml"), Source::Xdg));
        let e2 = env(&[("HOME", "/me"), ("SUDO_USER", "root")]);
        assert_eq!(resolve_path(None, &e2), (PathBuf::from("/me/.config/saltnitor/config.toml"), Source::Home));
    }

    /// Verifies: REQ-CFG-002/AC1, REQ-CFG-001/AC2
    #[test]
    fn missing_default_file_starts_with_defaults_and_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().to_string_lossy().into_owned();
        let l = load(None, &env(&[("HOME", &home)])).unwrap();
        assert_eq!(l.source, Source::Defaults);
        assert_eq!(l.config, ConfigV1::default());
        assert!(l.notes[0].contains(&home), "resolved path is logged: {:?}", l.notes);
        assert!(l.notes.iter().any(|n| n.contains("documented defaults")));
    }

    /// Verifies: REQ-CFG-002/AC2
    #[test]
    fn flag_pointing_at_a_missing_file_is_an_error() {
        let e = load(Some(Path::new("/nonexistent/x.toml")), &env(&[])).unwrap_err();
        assert_eq!(e.file, PathBuf::from("/nonexistent/x.toml"));
        assert_eq!(e.expected, "a readable config file");
    }

    /// Verifies: REQ-TST-001/AC1 (config validation unit coverage)
    #[test]
    fn live_shape_fixture_loads_with_every_profile() {
        let text = include_str!("../tests/data/live-shape.toml");
        let cfg = parse(text, Path::new("live-shape.toml"), &env(&[("HOME", "/h")])).unwrap();
        assert_eq!(cfg.control_port, Some(8765));
        assert_eq!(cfg.profiles.len(), 3);
        assert_eq!(cfg.timeouts, Timeouts::default());
        let _: HashMap<String, ProfileV1> = cfg.profiles.clone().into_iter().collect();
    }
}
```

`tests/data/live-shape.toml` — the operator's config with its **exact keys and tables** and every value replaced (write it from `sed -E 's/(token|bearer|key)[[:space:]]*=.*/\1 = "REDACTED"/I; s/=.*gguf"/= "model.gguf"/' ~/.config/saltnitor/config.toml` — **read only**, review the result for anything personal before committing):

```toml
# Shape of the reference host's v1 config (values replaced). Loading this proves the strict
# loader accepts the live key set (REQ-CFG-003 must never reject a valid live config).
port = 8080
host = "127.0.0.1"
service_name = "llama-router"
default_ngl = 33
default_ctx = 8192
control_port = 8765
control_token = "REDACTED"
router_base = "http://127.0.0.1:8080"
infer_bearer = "REDACTED"
reserve_vram_gb = 0.8
reserve_ram_gb = 1.0

[profiles.A_STD]
model = "model.gguf"
offload = false
est_vram_gb = 9.0
est_ram_gb = 0.0

[profiles.A_FOCUS]
model = "model.gguf"
offload = false
est_vram_gb = 11.0
est_ram_gb = 0.0

[profiles.B]
model = "model.gguf"
offload = true
est_vram_gb = 9.0
est_ram_gb = 18.0
```

Add `tempfile = "3"` under `[dev-dependencies]`. Run: `cargo test --bin saltnitor config_v1` → compile errors (red).

- [ ] **Step 2: Implement the module**

```rust
//! Strict v1 config loader (REQ-CFG-001…005, REQ-CFG-007; INV-10). Parses with
//! `deny_unknown_fields`, reports `file:line:col`, the dotted key path, what was expected and
//! what was found, and never falls back to defaults on a malformed file.
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: u32 = 1;
/// §6 default for `max_body_bytes` (32 MiB).
pub const DEFAULT_MAX_BODY_BYTES: u64 = 33_554_432;

#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ConfigV1 {
    pub schema_version: Option<u32>,
    pub port: Option<u16>,
    pub host: Option<String>,
    pub service_name: Option<String>,
    pub default_ngl: Option<i32>,
    pub default_ctx: Option<i32>,
    pub control_port: Option<u16>,
    pub control_token: Option<String>,
    pub control_token_env: Option<String>,
    pub control_token_file: Option<String>,
    pub router_base: Option<String>,
    pub infer_bearer: Option<String>,
    pub reserve_vram_gb: Option<f64>,
    pub reserve_ram_gb: Option<f64>,
    pub router_ini: Option<String>,
    pub client_key_env: Option<String>,
    pub allow_query_token: Option<bool>,
    pub max_body_bytes: Option<u64>,
    #[serde(default)]
    pub timeouts: Timeouts,
    #[serde(default)]
    pub process: ProcessCfg,
    #[serde(default)]
    pub profiles: BTreeMap<String, ProfileV1>,
}

/// §6 defaults: connect 5 000 ms, first byte 600 000 ms, idle between chunks 120 000 ms.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct Timeouts {
    pub connect_ms: u64,
    pub first_byte_ms: u64,
    pub idle_ms: u64,
}
impl Default for Timeouts {
    fn default() -> Self {
        Self { connect_ms: 5_000, first_byte_ms: 600_000, idle_ms: 120_000 }
    }
}

/// §6 default: `process.term_grace_ms` 5 000.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct ProcessCfg {
    pub term_grace_ms: u64,
}
impl Default for ProcessCfg {
    fn default() -> Self {
        Self { term_grace_ms: 5_000 }
    }
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProfileV1 {
    pub model: String,
    #[serde(default)]
    pub offload: bool,
    pub est_vram_gb: Option<f64>,
    pub est_ram_gb: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigError {
    pub file: PathBuf,
    pub line: Option<usize>,
    pub col: Option<usize>,
    pub key: String,
    pub expected: String,
    pub found: String,
    pub hint: Option<String>,
}
impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.file.display())?;
        if let (Some(l), Some(c)) = (self.line, self.col) {
            write!(f, ":{l}:{c}")?;
        }
        write!(f, ": ")?;
        if !self.key.is_empty() {
            write!(f, "{}: ", self.key)?;
        }
        write!(f, "expected {}, found {}", self.expected, self.found)?;
        if let Some(h) = &self.hint {
            write!(f, "\n  hint: {h}")?;
        }
        Ok(())
    }
}
impl std::error::Error for ConfigError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source { Flag, Xdg, Home, Defaults }
impl Source {
    fn label(self) -> &'static str {
        match self { Source::Flag => "--config", Source::Xdg => "$XDG_CONFIG_HOME", Source::Home => "~/.config", Source::Defaults => "defaults" }
    }
}

#[derive(Debug, Clone)]
pub struct Loaded {
    pub config: ConfigV1,
    pub path: PathBuf,
    pub source: Source,
    /// Lines the caller logs at startup (resolved path, defaults notice, deprecations).
    pub notes: Vec<String>,
}

/// Environment lookup, injected so tests never touch the process environment.
pub type Env<'a> = &'a dyn Fn(&str) -> Option<String>;

/// `--config` wins; else `$XDG_CONFIG_HOME/saltnitor/config.toml`; else `~/.config/…`.
/// `SUDO_USER` is deliberately ignored (REQ-CFG-001/AC3).
pub fn resolve_path(flag: Option<&Path>, env: Env) -> (PathBuf, Source) {
    if let Some(p) = flag {
        return (p.to_path_buf(), Source::Flag);
    }
    if let Some(x) = env("XDG_CONFIG_HOME").filter(|s| !s.is_empty()) {
        return (PathBuf::from(x).join("saltnitor/config.toml"), Source::Xdg);
    }
    let home = env("HOME").unwrap_or_default();
    (PathBuf::from(home).join(".config/saltnitor/config.toml"), Source::Home)
}

pub fn load(flag: Option<&Path>, env: Env) -> Result<Loaded, ConfigError> {
    let (path, source) = resolve_path(flag, env);
    let mut notes = vec![format!("config: {} ({})", path.display(), source.label())];
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && source != Source::Flag => {
            notes.push("config: no file found; starting with documented defaults".to_string());
            return Ok(Loaded { config: ConfigV1::default(), path, source: Source::Defaults, notes });
        }
        Err(e) => {
            return Err(ConfigError {
                file: path, line: None, col: None, key: String::new(),
                expected: "a readable config file".into(), found: e.to_string(), hint: None,
            });
        }
    };
    let config = parse(&text, &path, env)?;
    if config.schema_version.is_none() {
        notes.push("config: schema_version absent; treated as v1".to_string());
    }
    Ok(Loaded { config, path, source, notes })
}

pub fn parse(text: &str, file: &Path, env: Env) -> Result<ConfigV1, ConfigError> {
    let de = toml::de::Deserializer::parse(text).map_err(|e| toml_error(file, text, "", &e))?;
    let mut cfg: ConfigV1 = serde_path_to_error::deserialize(de).map_err(|e| {
        let key = match e.path().to_string() {
            p if p == "." => String::new(),
            p => p,
        };
        toml_error(file, text, &key, e.inner())
    })?;
    validate(&mut cfg, file, env)?;
    Ok(cfg)
}

fn validate(cfg: &mut ConfigV1, file: &Path, env: Env) -> Result<(), ConfigError> {
    let err = |key: &str, expected: &str, found: String| ConfigError {
        file: file.to_path_buf(), line: None, col: None, key: key.into(),
        expected: expected.into(), found, hint: None,
    };
    if let Some(v) = cfg.schema_version && v != SCHEMA_VERSION {
        return Err(err("schema_version", "1", v.to_string()));
    }
    if let Some(rb) = &cfg.router_base && !(rb.starts_with("http://") || rb.starts_with("https://")) {
        return Err(err("router_base", "an http(s) URL", format!("{rb:?}")));
    }
    let base = file.parent().unwrap_or_else(|| Path::new("."));
    if let Some(p) = cfg.router_ini.take() {
        cfg.router_ini = Some(expand_path(&p, base, env).map_err(|e| err("router_ini", "an expandable path", e))?);
    }
    if let Some(p) = cfg.control_token_file.take() {
        cfg.control_token_file = Some(expand_path(&p, base, env).map_err(|e| err("control_token_file", "an expandable path", e))?);
    }
    Ok(())
}

/// Expand `~`, `$VAR`, `${VAR}`; resolve a relative result against `base` (REQ-CFG-007).
pub fn expand_path(raw: &str, base: &Path, env: Env) -> Result<String, String> {
    let mut s = raw.to_string();
    if s == "~" || s.starts_with("~/") {
        let home = env("HOME").ok_or_else(|| "undefined variable HOME".to_string())?;
        s.replace_range(0..1, &home);
    }
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while let Some(off) = s[i..].find('$') {
        out.push_str(&s[i..i + off]);
        let after = &s[i + off + 1..];
        let (name, consumed) = if let Some(inner) = after.strip_prefix('{') {
            let close = inner.find('}').ok_or_else(|| "unterminated `${`".to_string())?;
            (&inner[..close], close + 2)
        } else {
            let n = after.bytes().take_while(|b| b.is_ascii_alphanumeric() || *b == b'_').count();
            (&after[..n], n)
        };
        if name.is_empty() {
            return Err("`$` must be followed by a variable name".to_string());
        }
        out.push_str(&env(name).ok_or_else(|| format!("undefined variable {name}"))?);
        i += off + 1 + consumed;
    }
    out.push_str(&s[i..]);
    let p = Path::new(&out);
    Ok(if p.is_absolute() { out } else { base.join(p).to_string_lossy().into_owned() })
}

fn toml_error(file: &Path, text: &str, key: &str, e: &toml::de::Error) -> ConfigError {
    let (line, col) = match e.span() {
        Some(s) => {
            let (l, c) = line_col(text, s.start);
            (Some(l), Some(c))
        }
        None => (None, None),
    };
    let (key, expected, found, hint) = humanize(key, e.message());
    ConfigError { file: file.to_path_buf(), line, col, key, expected, found, hint }
}

fn line_col(text: &str, offset: usize) -> (usize, usize) {
    let before = &text[..offset.min(text.len())];
    let line = before.matches('\n').count() + 1;
    let col = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
    (line, col)
}

/// Turn serde's wording into the REQ-CFG-003/AC2 shape: (key, expected, found, hint).
fn humanize(key: &str, msg: &str) -> (String, String, String, Option<String>) {
    if let Some(rest) = msg.strip_prefix("unknown field `") {
        let (field, tail) = rest.split_once('`').unwrap_or((rest, ""));
        let known: Vec<String> = tail.split('`').skip(1).step_by(2).map(str::to_string).collect();
        let hint = closest(field, &known).map(|k| format!("did you mean `{k}`?"));
        let expected = if known.is_empty() { "no keys here".to_string() } else { format!("one of {}", known.join(", ")) };
        return (join(key, field), expected, format!("unknown key `{field}`"), hint);
    }
    if let Some(rest) = msg.strip_prefix("missing field `") {
        let field = rest.trim_end_matches('`');
        return (join(key, field), "a value".to_string(), "nothing".to_string(), None);
    }
    if let Some(rest) = msg.strip_prefix("invalid type: ")
        && let Some((found, expected)) = rest.rsplit_once(", expected ")
    {
        return (key.to_string(), type_name(expected), found.to_string(), None);
    }
    if let Some(rest) = msg.strip_prefix("invalid value: ")
        && let Some((found, expected)) = rest.rsplit_once(", expected ")
    {
        return (key.to_string(), expected.to_string(), found.to_string(), None);
    }
    (key.to_string(), "valid TOML".to_string(), msg.to_string(), None)
}

fn type_name(serde_expected: &str) -> String {
    match serde_expected {
        "u8" | "u16" | "u32" | "u64" | "i8" | "i16" | "i32" | "i64" | "usize" | "isize" => "integer".into(),
        "f32" | "f64" => "float".into(),
        "a string" | "string" => "string".into(),
        "a boolean" | "boolean" => "boolean".into(),
        "a map" | "a table" | "map" => "table".into(),
        other => other.trim_start_matches("a ").to_string(),
    }
}

fn join(path: &str, field: &str) -> String {
    if path.is_empty() { field.to_string() } else { format!("{path}.{field}") }
}

fn closest(field: &str, known: &[String]) -> Option<String> {
    known.iter().map(|k| (edit_distance(field, k), k)).filter(|(d, _)| *d <= 3).min_by_key(|(d, _)| *d).map(|(_, k)| k.clone())
}

fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur.push((prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// The bearer the TUI sends on its own HTTP calls, from the env var named by `client_key_env`
/// (REQ-SEC-013). Never a literal, never logged. (Moved here from `main.rs`, T1.6.)
pub fn client_bearer(env_name: Option<&str>) -> Option<String> {
    env_name.and_then(|n| std::env::var(n).ok()).filter(|v| !v.is_empty())
}
```

Add to `Cargo.toml` `[dependencies]`: `serde_path_to_error = "0.1"` (justify in the commit: dotted key path in diagnostics, REQ-CFG-003/AC2; MIT/Apache-2.0). If `toml::de::Deserializer::parse` is not found by rustc, the toml line's API is `toml::Deserializer::new(text)` returning the deserializer directly (older 0.8 shape) — use that and drop the first `map_err`; record which in PROGRESS.

Run: `cargo test --bin saltnitor config_v1` → all green. If `type_error_reports_…` differs only in serde's wording (e.g. `expected f64` vs `expected a float`), extend `type_name`'s match — never the assertion.

- [ ] **Step 3: Failing black-box tests**

`tests/config_strict.rs`:

```rust
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests may unwrap: a panic is the failure signal
//! Runs the real binary: a malformed config must exit 2 before any port is bound (REQ-CFG-003).
use std::io::Write;
use std::net::TcpListener;
use std::process::Command;

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

fn run(config: &str) -> (i32, String, u16, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let port = free_port();
    let path = dir.path().join("config.toml");
    std::fs::File::create(&path).unwrap().write_all(config.replace("{PORT}", &port.to_string()).as_bytes()).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_saltnitor"))
        .arg("--config").arg(&path)
        .env("HOME", dir.path())
        .output().unwrap();
    (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stderr).into_owned(), port, dir)
}

/// Verifies: REQ-CFG-003/AC1, REQ-CFG-003/AC2, REQ-CFG-003/AC3
#[test]
fn type_error_exits_2_with_the_diagnostic_and_leaves_the_port_free() {
    let (code, err, port, _d) = run("control_port = {PORT}\n[profiles.qwen36]\nmodel = \"m.gguf\"\nest_vram_gb = \"9gb\"\n");
    assert_eq!(code, 2, "stderr: {err}");
    for needle in ["config.toml:4:", "profiles.qwen36.est_vram_gb", "expected float", "found string \"9gb\""] {
        assert!(err.contains(needle), "missing {needle:?} in {err}");
    }
    assert!(TcpListener::bind(("127.0.0.1", port)).is_ok(), "control port must be free after exit 2");
}

/// Verifies: REQ-CFG-004/AC1, REQ-CFG-004/AC2
#[test]
fn unknown_key_exits_2_with_a_hint() {
    let (code, err, _p, _d) = run("control_port = {PORT}\ncontrol_prot = 1\n");
    assert_eq!(code, 2, "stderr: {err}");
    assert!(err.contains("unknown key `control_prot`") && err.contains("did you mean `control_port`?"), "{err}");
}

/// Verifies: REQ-CFG-002/AC2
#[test]
fn missing_config_flag_file_exits_2() {
    let dir = tempfile::tempdir().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_saltnitor"))
        .arg("--config").arg(dir.path().join("absent.toml")).env("HOME", dir.path()).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("absent.toml"));
}
```

Run: `cargo test --test config_strict` → fails (the binary ignores `--config`: clap exits 2 with "unexpected argument" — note the stderr assertion is what makes this red for the right reason).

- [ ] **Step 4: Wire `main.rs`**

- `Cli` gains `#[arg(long, value_name = "FILE")] config: Option<std::path::PathBuf>,`.
- Delete `TomlConfig` and `load_config`. At the top of `main()`:

```rust
    let cli = Cli::parse();
    let loaded = match config_v1::load(cli.config.as_deref(), &|k| std::env::var(k).ok()) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("saltnitor: invalid config (CONFIG_INVALID)\n{e}");
            std::process::exit(2); // before any listener, task, or child (REQ-CFG-003/AC1)
        }
    };
    for n in &loaded.notes {
        eprintln!("saltnitor: {n}"); // REQ-CFG-001/AC2
    }
    let toml_conf = loaded.config;
```

- Every former `toml_conf.profiles.clone()` becomes `profile_metas(&toml_conf)`:

```rust
fn profile_metas(cfg: &config_v1::ConfigV1) -> HashMap<String, control_api::ProfileMeta> {
    cfg.profiles.iter().map(|(id, p)| (id.clone(), control_api::ProfileMeta {
        model: p.model.clone(), offload: p.offload, est_vram_gb: p.est_vram_gb, est_ram_gb: p.est_ram_gb,
    })).collect()
}
```

- `client_bearer` moves to `config_v1` (delete the `main.rs` copy and its test; re-add that test inside `config_v1`'s test module with the same body). After `App::new`, push `loaded.notes` into the log pane: `for n in &loaded.notes { app.add_log(format!(">>> CONFIG: {n}")); }` (clone `notes` before `toml_conf` is moved).

Run: `cargo test --workspace --locked` → green; `cargo clippy … -D warnings` and `cargo fmt --all --check` clean.

- [ ] **Step 5: Run Close-out** for T1.7 (`Refs: REQ-CFG-001, REQ-CFG-002, REQ-CFG-003, REQ-CFG-004, REQ-CFG-005, REQ-CFG-007, REQ-TST-001`; paths `src/config_v1.rs src/main.rs tests/config_strict.rs tests/data/live-shape.toml Cargo.toml Cargo.lock`). PROGRESS note: `strict loader; 9 unit + 3 black-box tests; SUDO_USER switch removed`.

---

### Task 9: T1.8 — Error envelope and mapping; library target (CR-7)

**Files:**
- Create: `src/error.rs`, `src/lib.rs`, `tests/error_mapping.rs`
- Modify: `src/control_api.rs` (errors → `ApiError`; fallback route; `pins_bd29_*` rewritten), `src/main.rs` (imports from the library), `docs/specs/vnext/evidence/test-baseline.txt` (suite rename — protected)

**Interfaces:**
- Produces: `saltnitor::error::{ErrorCode, ApiError}`; `ErrorCode::ALL`, `ErrorCode::status(self) -> Option<StatusCode>`, `ErrorCode::kind(self) -> &'static str`, `ErrorCode::as_str(self) -> &'static str`; `ApiError::new(code, msg)`, `.details(Value)`, `.request_id(String)`, `impl IntoResponse`; `saltnitor::{app, config_v1, control_api, error, events, ui}` modules.

- [ ] **Step 1: Failing table test**

`tests/error_mapping.rs`:

```rust
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests may unwrap: a panic is the failure signal
use axum::response::IntoResponse;
use saltnitor::error::{ApiError, ErrorCode};
use ErrorCode::*;

/// Appendix B, transcribed. Changing a row here is a spec change (REQ-ERR-002/AC1).
const APPENDIX_B: [(ErrorCode, &str, Option<u16>, &str); 22] = [
    (AuthRequired, "AUTH_REQUIRED", Some(401), "authentication_error"),
    (AuthForbidden, "AUTH_FORBIDDEN", Some(403), "permission_error"),
    (RequestInvalid, "REQUEST_INVALID", Some(400), "invalid_request_error"),
    (PayloadTooLarge, "PAYLOAD_TOO_LARGE", Some(413), "invalid_request_error"),
    (EndpointNotSupported, "ENDPOINT_NOT_SUPPORTED", Some(404), "invalid_request_error"),
    (ModelNotFound, "MODEL_NOT_FOUND", Some(404), "invalid_request_error"),
    (ModelBusy, "MODEL_BUSY", Some(503), "server_error"),
    (OracleRejected, "ORACLE_REJECTED", Some(507), "server_error"),
    (RuntimeNotFound, "RUNTIME_NOT_FOUND", Some(503), "server_error"),
    (RuntimeCapabilityMissing, "RUNTIME_CAPABILITY_MISSING", Some(422), "invalid_request_error"),
    (RuntimeStartFailed, "RUNTIME_START_FAILED", Some(502), "server_error"),
    (RuntimeUnhealthy, "RUNTIME_UNHEALTHY", Some(502), "server_error"),
    (UpstreamTimeout, "UPSTREAM_TIMEOUT", Some(504), "server_error"),
    (UpstreamStreamAborted, "UPSTREAM_STREAM_ABORTED", None, "server_error"),
    (GpuOom, "GPU_OOM", Some(503), "server_error"),
    (ProcessChanged, "PROCESS_CHANGED", Some(409), "invalid_request_error"),
    (ProcessProtected, "PROCESS_PROTECTED", Some(403), "permission_error"),
    (ValidationRequired, "VALIDATION_REQUIRED", Some(409), "invalid_request_error"),
    (BenchmarkFailed, "BENCHMARK_FAILED", Some(500), "server_error"),
    (ShuttingDown, "SHUTTING_DOWN", Some(503), "server_error"),
    (ConfigInvalid, "CONFIG_INVALID", Some(422), "invalid_request_error"),
    (Internal, "INTERNAL", Some(500), "server_error"),
];

/// Verifies: REQ-ERR-002/AC1, REQ-ERR-003/AC1
#[test]
fn every_appendix_b_code_maps_to_its_status_and_type() {
    assert_eq!(ErrorCode::ALL.len(), APPENDIX_B.len());
    for (code, name, status, kind) in APPENDIX_B {
        assert!(ErrorCode::ALL.contains(&code), "{name} missing from ALL");
        assert_eq!(code.as_str(), name);
        assert_eq!(code.status().map(|s| s.as_u16()), status, "{name}");
        assert_eq!(code.kind(), kind, "{name}");
    }
}

/// Verifies: REQ-ERR-001/AC1
#[tokio::test]
async fn envelope_has_exactly_the_five_fields_and_json_content_type() {
    let resp = ApiError::new(ModelBusy, "A is loading").request_id("req-1").into_response();
    assert_eq!(resp.status().as_u16(), 503);
    assert_eq!(resp.headers()["content-type"], "application/json");
    assert_eq!(resp.headers()["retry-after"], "1");
    let body = axum::body::to_bytes(resp.into_body(), 1 << 16).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let e = v["error"].as_object().unwrap();
    let mut keys: Vec<_> = e.keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, ["code", "details", "message", "request_id", "type"]);
    assert_eq!(e["code"], "MODEL_BUSY");
    assert_eq!(e["type"], "server_error");
    assert_eq!(e["request_id"], "req-1");
    assert!(e["details"].is_null());
}
```

Run: `cargo test --test error_mapping` → fails: `saltnitor` has no library target.

- [ ] **Step 2: Create the library and the module**

`src/lib.rs`:

```rust
//! Saltnitor library surface (CR-7, REQ-ARCH-002). Integration tests and the acceptance suite
//! import from here; `src/main.rs` owns the TUI loop and imports these same modules.
#![expect(clippy::unwrap_used, clippy::expect_used, reason = "removed by T1.14 (REQ-ERR-004); expect warns once no unwrap remains")]
pub mod app;
pub mod config_v1;
pub mod control_api;
pub mod error;
pub mod events;
pub mod ui;
```

(Remove the file-level `#![expect(clippy::unwrap_used …)]` from `control_api.rs` only if clippy says it is unfulfilled; `main.rs` keeps its own.) `src/main.rs`: delete `mod app; mod control_api; mod events; mod ui; mod config_v1;` and add `use saltnitor::{app, config_v1, control_api, events, ui};` (keep every existing `use crate::…`-free path working; `app::App`, `events::Event`, `ui::draw` resolve through the `use`).

`src/error.rs`:

```rust
//! Error envelope and code catalog (REQ-ERR-001/002/003; Appendix B; DEC-08).
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    AuthRequired, AuthForbidden, RequestInvalid, PayloadTooLarge, EndpointNotSupported,
    ModelNotFound, ModelBusy, OracleRejected, RuntimeNotFound, RuntimeCapabilityMissing,
    RuntimeStartFailed, RuntimeUnhealthy, UpstreamTimeout, UpstreamStreamAborted, GpuOom,
    ProcessChanged, ProcessProtected, ValidationRequired, BenchmarkFailed, ShuttingDown,
    ConfigInvalid, Internal,
}

impl ErrorCode {
    pub const ALL: [ErrorCode; 22] = [
        Self::AuthRequired, Self::AuthForbidden, Self::RequestInvalid, Self::PayloadTooLarge,
        Self::EndpointNotSupported, Self::ModelNotFound, Self::ModelBusy, Self::OracleRejected,
        Self::RuntimeNotFound, Self::RuntimeCapabilityMissing, Self::RuntimeStartFailed,
        Self::RuntimeUnhealthy, Self::UpstreamTimeout, Self::UpstreamStreamAborted, Self::GpuOom,
        Self::ProcessChanged, Self::ProcessProtected, Self::ValidationRequired, Self::BenchmarkFailed,
        Self::ShuttingDown, Self::ConfigInvalid, Self::Internal,
    ];

    /// Appendix B status. `None` for `UPSTREAM_STREAM_ABORTED`: headers are already sent, it is
    /// only logged. `CONFIG_INVALID` is exit code 2 at startup and 422 on an admin reload.
    pub fn status(self) -> Option<StatusCode> {
        use StatusCode as S;
        Some(match self {
            Self::AuthRequired => S::UNAUTHORIZED,
            Self::AuthForbidden | Self::ProcessProtected => S::FORBIDDEN,
            Self::RequestInvalid => S::BAD_REQUEST,
            Self::PayloadTooLarge => S::PAYLOAD_TOO_LARGE,
            Self::EndpointNotSupported | Self::ModelNotFound => S::NOT_FOUND,
            Self::ModelBusy | Self::RuntimeNotFound | Self::GpuOom | Self::ShuttingDown => S::SERVICE_UNAVAILABLE,
            Self::OracleRejected => S::INSUFFICIENT_STORAGE,
            Self::RuntimeCapabilityMissing | Self::ConfigInvalid => S::UNPROCESSABLE_ENTITY,
            Self::RuntimeStartFailed | Self::RuntimeUnhealthy => S::BAD_GATEWAY,
            Self::UpstreamTimeout => S::GATEWAY_TIMEOUT,
            Self::UpstreamStreamAborted => return None,
            Self::ProcessChanged | Self::ValidationRequired => S::CONFLICT,
            Self::BenchmarkFailed | Self::Internal => S::INTERNAL_SERVER_ERROR,
        })
    }

    pub fn kind(self) -> &'static str {
        match self {
            Self::AuthRequired => "authentication_error",
            Self::AuthForbidden | Self::ProcessProtected => "permission_error",
            Self::RequestInvalid | Self::PayloadTooLarge | Self::EndpointNotSupported | Self::ModelNotFound
            | Self::RuntimeCapabilityMissing | Self::ProcessChanged | Self::ValidationRequired
            | Self::ConfigInvalid => "invalid_request_error",
            Self::ModelBusy | Self::OracleRejected | Self::RuntimeNotFound | Self::RuntimeStartFailed
            | Self::RuntimeUnhealthy | Self::UpstreamTimeout | Self::UpstreamStreamAborted | Self::GpuOom
            | Self::BenchmarkFailed | Self::ShuttingDown | Self::Internal => "server_error",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::AuthRequired => "AUTH_REQUIRED", Self::AuthForbidden => "AUTH_FORBIDDEN",
            Self::RequestInvalid => "REQUEST_INVALID", Self::PayloadTooLarge => "PAYLOAD_TOO_LARGE",
            Self::EndpointNotSupported => "ENDPOINT_NOT_SUPPORTED", Self::ModelNotFound => "MODEL_NOT_FOUND",
            Self::ModelBusy => "MODEL_BUSY", Self::OracleRejected => "ORACLE_REJECTED",
            Self::RuntimeNotFound => "RUNTIME_NOT_FOUND", Self::RuntimeCapabilityMissing => "RUNTIME_CAPABILITY_MISSING",
            Self::RuntimeStartFailed => "RUNTIME_START_FAILED", Self::RuntimeUnhealthy => "RUNTIME_UNHEALTHY",
            Self::UpstreamTimeout => "UPSTREAM_TIMEOUT", Self::UpstreamStreamAborted => "UPSTREAM_STREAM_ABORTED",
            Self::GpuOom => "GPU_OOM", Self::ProcessChanged => "PROCESS_CHANGED",
            Self::ProcessProtected => "PROCESS_PROTECTED", Self::ValidationRequired => "VALIDATION_REQUIRED",
            Self::BenchmarkFailed => "BENCHMARK_FAILED", Self::ShuttingDown => "SHUTTING_DOWN",
            Self::ConfigInvalid => "CONFIG_INVALID", Self::Internal => "INTERNAL",
        }
    }
}

/// One daemon-generated HTTP error (REQ-ERR-001). Rendered as
/// `{"error":{"code","message","type","details","request_id"}}` with `application/json`.
#[derive(Debug, Clone)]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: String,
    pub details: Option<Value>,
    pub request_id: Option<String>,
}

impl ApiError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), details: None, request_id: None }
    }
    pub fn details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }
    pub fn request_id(mut self, id: impl Into<String>) -> Self {
        self.request_id = Some(id.into());
        self
    }
    pub fn body(&self) -> Value {
        json!({ "error": {
            "code": self.code.as_str(), "message": self.message, "type": self.code.kind(),
            "details": self.details, "request_id": self.request_id,
        }})
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.message)
    }
}
impl std::error::Error for ApiError {}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        // UPSTREAM_STREAM_ABORTED is never rendered (headers already sent); 500 is the safe fallback.
        let status = self.code.status().unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let mut resp = (status, [(header::CONTENT_TYPE, "application/json")], self.body().to_string()).into_response();
        if self.code == ErrorCode::ModelBusy {
            resp.headers_mut().insert(header::RETRY_AFTER, HeaderValue::from_static("1"));
        }
        resp
    }
}
```

Run: `cargo test --test error_mapping` → green.

- [ ] **Step 3: Rewrite the `pins_bd29_*` characterization tests (CR-6 → fixed), then make them pass**

In `src/control_api.rs`'s test module, rename and rewrite (commit message states why — CR-6/CR-8):

| Old | New name | New assertion |
|---|---|---|
| `pins_bd29_ensure_oracle_reject_is_507_json` | `ensure_oracle_reject_is_507_envelope` | `(507, "application/json", body)` with `error.code == "ORACLE_REJECTED"` and `error.details.need_vram_gb` present |
| `pins_bd29_chat_errors_are_plain_text` | `chat_errors_are_json_envelopes` | missing model → `(400, json, code "REQUEST_INVALID")` |
| `pins_bd29_chat_oracle_reject_is_503_plain_text` | `chat_oracle_reject_is_507_envelope` | `507`, `ORACLE_REJECTED` |
| `pins_bd29_chat_load_failure_is_502_plain_text` | `chat_load_failure_is_502_envelope` | `502`, `RUNTIME_START_FAILED`, `message` contains `router returned 500` |
| `pins_bd29_chat_upstream_body_failure_is_502_plain_text` | `chat_upstream_failure_is_502_envelope` | `502`, `RUNTIME_UNHEALTHY` |

Add: `/// Verifies: REQ-PRX-020/AC1` test `unknown_v1_path_is_404_endpoint_not_supported`: `GET /v1/embeddings` → `(404, application/json, code ENDPOINT_NOT_SUPPORTED)`; and `POST /nope` → same. `pins_bd28_*` stays (BD-28 is fixed by T4.9).

Implementation in `control_api.rs`:
- `h_chat`: `missing 'model'` → `ApiError::new(ErrorCode::RequestInvalid, "missing 'model' in request body").into_response()`; `EnsureOutcome::Bad(e)` → `ModelNotFound` with message `unknown model '{model}': {e}`; `Oom(i)` → `OracleRejected` with `details(json!({"need_vram_gb": i.need_vram_gb, "total_vram_gb": i.total_vram_gb, "need_ram_gb": i.need_ram_gb, "total_ram_gb": i.total_ram_gb}))`; `Err(e)` → `RuntimeStartFailed` `load failed: {e}`; `router read failed`/`router unreachable`/`proxy build error` → `RuntimeUnhealthy`.
- `h_ensure`: `Oom` → the same `OracleRejected` envelope (507); `Bad` → `ModelNotFound` 404; `Err` → `RuntimeStartFailed` 502; the in-handler `bad token` → `ApiError::new(AuthRequired, "missing or invalid credentials")` (T1.9 removes it). `Loaded`/`AlreadyResident` keep the `EnsureResponse` JSON (REQ-MIG-007/AC2).
- `h_ensure_stream`: the `bad token` plain text → the `AuthRequired` envelope.
- `serve`: `.fallback(h_not_supported)` where `async fn h_not_supported(uri: axum::http::Uri) -> ApiError { ApiError::new(ErrorCode::EndpointNotSupported, format!("{} is not supported", uri.path())) }`.

Run: `cargo test --workspace --locked` → green (the old pins no longer exist; the new names pass).

- [ ] **Step 4: Test-baseline suite rename (protected)**

Run `scripts/gate.sh counts` and compare with `docs/specs/vnext/evidence/test-baseline.txt`: the `saltnitor src/main.rs` unit suite now contains only the `main.rs` tests (upsert); `saltnitor src/lib.rs` carries the moved ones. Update the baseline lines: rename, and set each count to today's number (`cargo test -- --list`), never lower than the old one for the same tests. `scripts/check-test-counts.sh` → exit 0.

- [ ] **Step 5: Run Close-out** for T1.8 (`Refs: REQ-ERR-001, REQ-ERR-002, REQ-ERR-003, REQ-PRX-020`; trailer `Protected-change: docs/specs/vnext/evidence/test-baseline.txt — lib target moves unit suites (CR-7); src/control_api.rs test module: pins_bd29_* rewritten as the fixed behaviour (CR-6, CR-8)`).

---

### Task 10: T1.9 — Authentication middleware and route policy

**Files:**
- Create: `src/auth.rs`, `tests/auth_policy.rs`
- Modify: `src/control_api.rs` (router built from the policy table; auth layer; in-handler token checks removed; `ConnectInfo`), `src/lib.rs` (`pub mod auth`), `src/main.rs` (startup warning for `allow_query_token`), `Cargo.toml` (`subtle`, dev `proptest`)

**Interfaces:**
- Produces: `auth::{Auth, Scope, RoutePolicy, POLICY, policy_for, Presented, decide, AuthState, middleware, bearer_from_header}`; `control_api::{router(api: Arc<ControlApi>, auth: Arc<AuthState>) -> Result<Router, String>, HANDLER_PATHS, serve}`; `ControlApi::allow_query_token(bool)` builder.
- Consumes: `error::{ApiError, ErrorCode}` (Task 9).

- [ ] **Step 1: Failing tests**

`tests/auth_policy.rs`:

```rust
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests may unwrap: a panic is the failure signal
//! Enumerates the route policy against the real router (REQ-SEC-001, REQ-SEC-005, REQ-PRX-016).
use axum::http::Method;
use fake_llama_server::Scenario;
use proptest::prelude::*;
use saltnitor::auth::{decide, Auth, Presented, RoutePolicy, Scope, POLICY};
use saltnitor::control_api::{serve, ControlApi, HANDLER_PATHS};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

const TOKEN: &str = "test-token-0123456789";

async fn rig(allow_query: bool) -> (String, mpsc::Receiver<saltnitor::events::Event>, fake_llama_server::Handle) {
    let fake = fake_llama_server::spawn(toml::from_str::<Scenario>(include_str!("fixtures/scenarios/control-api-legacy.toml")).unwrap()).await;
    let (tx, rx) = mpsc::channel(256);
    let api = Arc::new(ControlApi::new(Default::default(), fake.base_url(), None, Some(TOKEN.into()), 0.0, 0.0, tx).allow_query_token(allow_query));
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    tokio::spawn(serve(api, addr));
    let base = format!("http://{addr}");
    let http = reqwest::Client::new();
    for _ in 0..100 {
        if http.get(format!("{base}/healthz")).send().await.is_ok() { return (base, rx, fake); }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("control API did not come up");
}

fn body_for(path: &str) -> &'static str {
    match path { "/v1/chat/completions" => r#"{"model":"A","messages":[]}"#, "/v1/ensure" => r#"{"profile":"A"}"#, _ => "" }
}

/// Verifies: REQ-SEC-001/AC3
#[test]
fn every_handler_has_a_policy_and_every_policy_a_handler() {
    let handlers: BTreeSet<&str> = HANDLER_PATHS.iter().copied().collect();
    let policies: BTreeSet<&str> = POLICY.iter().map(|p| p.path).collect();
    assert_eq!(handlers, policies, "route without a policy entry, or policy without a route");
}

/// Verifies: REQ-SEC-001/AC1, REQ-SEC-001/AC2, REQ-SEC-005/AC1, REQ-PRX-016/AC1
#[tokio::test]
async fn every_route_gets_the_policy_outcome() {
    let (base, _rx, _fake) = rig(false).await;
    let http = reqwest::Client::new();
    for p in POLICY {
        for m in p.methods {
            let url = format!("{base}{}", p.path);
            let send = |bearer: Option<&str>| {
                let mut r = http.request(m.clone(), &url).body(body_for(p.path)).header("content-type", "application/json");
                if let Some(b) = bearer { r = r.header("authorization", format!("Bearer {b}")); }
                r.send()
            };
            let none = send(None).await.unwrap();
            let wrong = send(Some("nope")).await.unwrap();
            let right = send(Some(TOKEN)).await.unwrap();
            match p.auth {
                Auth::Public => assert_ne!(none.status(), 401, "{} {}", m, p.path),
                Auth::Required => {
                    assert_eq!(none.status(), 401, "{} {} without credentials", m, p.path);
                    assert_eq!(wrong.status(), 401, "{} {} wrong token", m, p.path);
                    let v: serde_json::Value = none.json().await.unwrap();
                    assert_eq!(v["error"]["code"], "AUTH_REQUIRED");
                    assert_ne!(right.status(), 401, "{} {} with the token", m, p.path);
                }
            }
        }
    }
    let h: serde_json::Value = http.get(format!("{base}/healthz")).send().await.unwrap().json().await.unwrap();
    assert_eq!(h, serde_json::json!({"status": "ok"}));
}

/// Verifies: REQ-SEC-005/AC2
#[tokio::test]
async fn query_token_is_ignored_by_default() {
    let (base, _rx, _fake) = rig(false).await;
    let r = reqwest::get(format!("{base}/v1/ensure/stream?profile=A&token={TOKEN}")).await.unwrap();
    assert_eq!(r.status(), 401);
}

/// Verifies: REQ-SEC-005/AC3
#[tokio::test]
async fn query_token_in_compat_mode_works_only_on_get_ensure_stream_and_is_redacted() {
    let (base, mut rx, _fake) = rig(true).await;
    let ok = reqwest::get(format!("{base}/v1/ensure/stream?profile=A&token={TOKEN}")).await.unwrap();
    assert_ne!(ok.status(), 401);
    let other = reqwest::get(format!("{base}/v1/models?token={TOKEN}")).await.unwrap();
    assert_eq!(other.status(), 401, "?token= is never accepted elsewhere");
    while let Ok(ev) = rx.try_recv() {
        if let saltnitor::events::Event::LogLine(l) = ev { assert!(!l.contains(TOKEN), "token leaked into a log line: {l}"); }
    }
}

/// Verifies: REQ-SEC-011/AC1
#[tokio::test]
async fn failed_auth_is_logged_once_per_peer_per_second_without_the_token() {
    let (base, mut rx, _fake) = rig(false).await;
    let http = reqwest::Client::new();
    for _ in 0..3 { http.get(format!("{base}/v1/models")).header("authorization", "Bearer wrong-secret").send().await.unwrap(); }
    tokio::time::sleep(Duration::from_millis(50)).await;
    let mut lines = vec![];
    while let Ok(ev) = rx.try_recv() { if let saltnitor::events::Event::LogLine(l) = ev && l.starts_with("auth: 401") { lines.push(l); } }
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains("127.0.0.1") && lines[0].contains("/v1/models") && !lines[0].contains("wrong-secret"));
}

/// Verifies: REQ-SEC-015/AC1
#[tokio::test]
async fn no_cors_headers_by_default() {
    let (base, _rx, _fake) = rig(false).await;
    let r = reqwest::Client::new().get(format!("{base}/healthz")).header("origin", "http://evil.example").send().await.unwrap();
    assert!(r.headers().get("access-control-allow-origin").is_none());
}

/// Verifies: REQ-SEC-001/AC2 (wrong scope → 403; admin routes arrive in P3, so the decision is unit-tested)
#[test]
fn admin_scope_with_an_inference_token_is_forbidden() {
    let admin = RoutePolicy { path: "/admin/x", methods: &[Method::GET], auth: Auth::Required, scope: Scope::Admin, query_token_compat: false };
    let p = Presented { token_configured: true, bearer_valid: true, query_valid: false, allow_query: false, is_get: true };
    assert_eq!(decide(&admin, &Method::GET, p), Err(saltnitor::error::ErrorCode::AuthForbidden));
}

proptest! {
    /// Verifies: REQ-SEC-001/AC2 [PROP], REQ-SEC-005/AC1 — [RF-1] scheme case and extra spaces
    #[test]
    fn bearer_header_parsing_is_tolerant_and_exact(scheme in "[Bb][Ee][Aa][Rr][Ee][Rr]", spaces in " {1,3}", junk in "[A-Za-z0-9._-]{1,40}") {
        let header = format!("{scheme}{spaces}{TOKEN}");
        prop_assert_eq!(saltnitor::auth::bearer_from_header(&header), Some(TOKEN));
        prop_assume!(junk != TOKEN);
        prop_assert_eq!(saltnitor::auth::bearer_from_header(&format!("Bearer {junk}")), Some(junk.as_str()));
        prop_assert!(!saltnitor::auth::token_matches(Some(TOKEN), &junk));
        prop_assert!(saltnitor::auth::token_matches(Some(TOKEN), TOKEN));
    }
}
```

(`tests/fixtures/scenarios/control-api-legacy.toml` already exists; `include_str!` resolves relative to `tests/`.) Add `subtle = "2"` to `[dependencies]` (justify: constant-time compare, REQ-SEC-004; BSD-3-Clause) and `proptest = "1"` to `[dev-dependencies]`. Run: `cargo test --test auth_policy` → compile errors (red).

- [ ] **Step 2: Implement `src/auth.rs`**

```rust
//! One authentication layer for the whole router, driven by the Appendix A policy table
//! (REQ-SEC-001/002/004/005/011/015, REQ-PRX-016). Handlers contain no auth logic.
use crate::error::{ApiError, ErrorCode};
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{header, Method, Uri};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use subtle::ConstantTimeEq;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Auth { Public, Required }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope { None, Inference, Admin }

#[derive(Debug, Clone, Copy)]
pub struct RoutePolicy {
    pub path: &'static str,
    pub methods: &'static [Method],
    pub auth: Auth,
    pub scope: Scope,
    /// `?token=` accepted only here, only on GET, only when `allow_query_token` (REQ-SEC-005/AC3).
    pub query_token_compat: bool,
}

/// Appendix A, the P1 subset. A route exists only if it is listed here AND in
/// `control_api::HANDLER_PATHS`; `tests/auth_policy.rs` fails otherwise (REQ-SEC-001/AC3).
pub const POLICY: &[RoutePolicy] = &[
    RoutePolicy { path: "/healthz", methods: &[Method::GET], auth: Auth::Public, scope: Scope::None, query_token_compat: false },
    RoutePolicy { path: "/v1/models", methods: &[Method::GET], auth: Auth::Required, scope: Scope::Inference, query_token_compat: false },
    RoutePolicy { path: "/v1/status", methods: &[Method::GET], auth: Auth::Required, scope: Scope::Inference, query_token_compat: false },
    RoutePolicy { path: "/v1/chat/completions", methods: &[Method::POST], auth: Auth::Required, scope: Scope::Inference, query_token_compat: false },
    RoutePolicy { path: "/v1/ensure", methods: &[Method::POST], auth: Auth::Required, scope: Scope::Inference, query_token_compat: false },
    RoutePolicy { path: "/v1/ensure/stream", methods: &[Method::GET], auth: Auth::Required, scope: Scope::Inference, query_token_compat: true },
];

pub fn policy_for(path: &str) -> Option<&'static RoutePolicy> {
    POLICY.iter().find(|p| p.path == path)
}

/// What the request presented, reduced to booleans so the decision is a pure function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Presented {
    pub token_configured: bool,
    pub bearer_valid: bool,
    pub query_valid: bool,
    pub allow_query: bool,
    pub is_get: bool,
}

/// The policy decision (REQ-SEC-001/AC2): `Ok` = pass, else the error code to render.
pub fn decide(policy: &RoutePolicy, method: &Method, p: Presented) -> Result<(), ErrorCode> {
    if !policy.methods.contains(method) {
        return Err(ErrorCode::EndpointNotSupported);
    }
    if policy.auth == Auth::Public || !p.token_configured {
        return Ok(());
    }
    let via_query = policy.query_token_compat && p.allow_query && p.is_get && p.query_valid;
    if !(p.bearer_valid || via_query) {
        return Err(ErrorCode::AuthRequired);
    }
    match policy.scope {
        Scope::None | Scope::Inference => Ok(()),
        Scope::Admin => Err(ErrorCode::AuthForbidden), // inference keys never grant admin (P3 adds admin tokens)
    }
}

/// `Authorization: Bearer <token>` — scheme case-insensitive, one or more spaces (RFC 9110 §11.4).
pub fn bearer_from_header(value: &str) -> Option<&str> {
    let (scheme, rest) = value.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let t = rest.trim_start_matches(' ');
    (!t.is_empty()).then_some(t)
}

/// Constant-time comparison (REQ-SEC-004). Length mismatch short-circuits, as `subtle` documents.
pub fn token_matches(expected: Option<&str>, presented: &str) -> bool {
    match expected {
        Some(e) => e.as_bytes().ct_eq(presented.as_bytes()).into(),
        None => false,
    }
}

fn query_token(uri: &Uri) -> Option<String> {
    uri.query()?.split('&').find_map(|kv| kv.strip_prefix("token=").map(|v| v.to_string()))
}

/// The same URI without `token=` (REQ-SEC-005/AC3: redacted everywhere downstream).
fn strip_query_token(uri: &Uri) -> Uri {
    let Some(q) = uri.query() else { return uri.clone() };
    let kept: Vec<&str> = q.split('&').filter(|kv| !kv.starts_with("token=")).collect();
    let pq = if kept.is_empty() { uri.path().to_string() } else { format!("{}?{}", uri.path(), kept.join("&")) };
    pq.parse().unwrap_or_else(|_| uri.clone())
}

pub struct AuthState {
    token: Option<String>,
    allow_query_token: bool,
    log: Box<dyn Fn(String) + Send + Sync>,
    last_failure: Mutex<HashMap<IpAddr, Instant>>,
}

impl AuthState {
    pub fn new(token: Option<String>, allow_query_token: bool, log: impl Fn(String) + Send + Sync + 'static) -> Arc<Self> {
        Arc::new(Self { token, allow_query_token, log: Box::new(log), last_failure: Mutex::new(HashMap::new()) })
    }

    /// At most one line per peer per second (REQ-SEC-011/AC1); never the credential.
    fn log_failure(&self, peer: Option<SocketAddr>, method: &Method, path: &str, reason: &str) {
        let ip = peer.map(|p| p.ip());
        if let Some(ip) = ip
            && let Ok(mut m) = self.last_failure.lock()
        {
            let now = Instant::now();
            if m.get(&ip).is_some_and(|t| now.duration_since(*t) < Duration::from_secs(1)) {
                return;
            }
            m.insert(ip, now);
        }
        let peer = peer.map_or_else(|| "unknown".to_string(), |p| p.to_string());
        (self.log)(format!("auth: 401 listener=loopback peer={peer} {method} {path} ({reason})"));
    }
}

pub async fn middleware(State(auth): State<Arc<AuthState>>, mut req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let Some(policy) = policy_for(&path) else {
        return ApiError::new(ErrorCode::EndpointNotSupported, format!("{path} is not supported")).into_response();
    };
    let bearer_valid = req.headers().get(header::AUTHORIZATION).and_then(|v| v.to_str().ok())
        .and_then(bearer_from_header).is_some_and(|t| token_matches(auth.token.as_deref(), t));
    let query_valid = query_token(req.uri()).is_some_and(|t| token_matches(auth.token.as_deref(), &t));
    let presented = Presented {
        token_configured: auth.token.is_some(), bearer_valid, query_valid,
        allow_query: auth.allow_query_token, is_get: method == Method::GET,
    };
    match decide(policy, &method, presented) {
        Ok(()) => {
            if query_valid {
                *req.uri_mut() = strip_query_token(req.uri());
            }
            next.run(req).await
        }
        Err(ErrorCode::AuthRequired) => {
            let peer = req.extensions().get::<ConnectInfo<SocketAddr>>().map(|c| c.0);
            let reason = if req.headers().contains_key(header::AUTHORIZATION) { "invalid credentials" } else { "no credentials" };
            auth.log_failure(peer, &method, &path, reason);
            ApiError::new(ErrorCode::AuthRequired, "missing or invalid credentials").into_response()
        }
        Err(ErrorCode::AuthForbidden) => ApiError::new(ErrorCode::AuthForbidden, "this credential lacks the required scope").into_response(),
        Err(code) => ApiError::new(code, format!("{method} {path} is not supported")).into_response(),
    }
}
```

- [ ] **Step 3: Build the router from the policy table**

In `control_api.rs`:

```rust
/// Every path `router()` can serve. Must equal the policy's path set (REQ-SEC-001/AC3).
pub const HANDLER_PATHS: &[&str] = &["/healthz", "/v1/models", "/v1/status", "/v1/chat/completions", "/v1/ensure", "/v1/ensure/stream"];

fn handler_for(path: &str) -> Option<MethodRouter<Arc<ControlApi>>> {
    Some(match path {
        "/healthz" => get(h_health),
        "/v1/models" => get(h_models),
        "/v1/status" => get(h_status),
        "/v1/chat/completions" => post(h_chat),
        "/v1/ensure" => post(h_ensure),
        "/v1/ensure/stream" => get(h_ensure_stream),
        _ => return None,
    })
}

/// The router: one route per policy row, the auth layer over everything, default-deny fallback.
pub fn router(api: Arc<ControlApi>, auth: Arc<crate::auth::AuthState>) -> Result<Router, String> {
    let mut r = Router::new();
    for p in crate::auth::POLICY {
        let h = handler_for(p.path).ok_or_else(|| format!("policy entry {} has no handler", p.path))?;
        r = r.route(p.path, h);
    }
    Ok(r.fallback(h_not_supported)
        .layer(axum::middleware::from_fn_with_state(auth, crate::auth::middleware))
        .with_state(api))
}

pub async fn serve(api: Arc<ControlApi>, addr: std::net::SocketAddr) {
    let log_tx = api.tx.clone();
    let auth = crate::auth::AuthState::new(api.control_token.clone(), api.allow_query_token, move |line| {
        let _ = log_tx.try_send(Event::LogLine(line));
    });
    let app = match router(api, auth) {
        Ok(a) => a,
        Err(e) => { eprintln!("[control_api] {e}"); return; } // becomes Event::Error in T1.14
    };
    match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => { let _ = axum::serve(l, app.into_make_service_with_connect_info::<std::net::SocketAddr>()).await; }
        Err(e) => eprintln!("[control_api] bind {} failed: {}", addr, e),
    }
}
```

`h_health` → `Json(json!({"status": "ok"}))`. Remove `auth_ok` and every token check from `h_ensure` / `h_ensure_stream`; drop `EnsureRequest.token` (the layer strips it). Add `allow_query_token: bool` to `ControlApi` (default `false`) with `pub fn allow_query_token(mut self, b: bool) -> Self`. In `main.rs`: `.allow_query_token(toml_conf.allow_query_token.unwrap_or(false))`; when true, `eprintln!("saltnitor: security: allow_query_token=true — ?token= is accepted on GET /v1/ensure/stream only; the value is redacted in logs")` and the same line into `app.add_log`.

Existing characterization test `ensure_stream_rejects_a_wrong_query_token` still passes (401 is the outcome). The ignored probe `bd04_query_token_is_always_accepted` is now false: convert it to a passing, non-ignored test named `query_token_is_refused_by_default_bd04_fixed` (commit states: BD-04 fixed by T1.9).

Run: `cargo test --workspace --locked` → green, including `tests/auth_policy.rs`.

- [ ] **Step 4: Run Close-out** for T1.9 (`Refs: REQ-SEC-001, REQ-SEC-002, REQ-SEC-004, REQ-SEC-005, REQ-SEC-011, REQ-SEC-015, REQ-PRX-016, REQ-TST-008`; trailer `Protected-change: src/control_api.rs test module — bd04 probe becomes the fixed-behaviour test (BD-04 fixed)`).

---

### Task 11: T1.10 — Secret sourcing and redaction

**Files:**
- Modify: `src/config_v1.rs` (`resolve_control_token`, `validate_key_file`), `src/auth.rs` (`Redactor`), `src/app.rs` (`redactor` field, `add_log` redacts, `crash_dump_text`), `src/main.rs` (token resolution; crash dump uses `crash_dump_text`), `Cargo.toml` (`rustix` with `process`)
- Create: `tests/secrets.rs`

**Interfaces:**
- Produces: `config_v1::resolve_control_token(cfg: &ConfigV1, env: Env, notes: &mut Vec<String>) -> Result<Option<String>, ConfigError>`; `config_v1::validate_key_file(is_file: bool, mode: u32, owner_uid: u32, my_uid: u32) -> Result<(), String>`; `auth::Redactor::new(secrets: Vec<String>)`, `Redactor::redact(&self, line: &str) -> String`; `App::crash_dump_text(&self, timestamp: &str) -> String`.

- [ ] **Step 1: Failing tests**

`tests/secrets.rs`:

```rust
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests may unwrap: a panic is the failure signal
use saltnitor::auth::Redactor;
use saltnitor::config_v1::{resolve_control_token, validate_key_file, ConfigV1};
use std::os::unix::fs::PermissionsExt;

fn env(map: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + '_ {
    move |k| map.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string())
}

/// Verifies: REQ-SEC-003/AC1
#[test]
fn token_from_env_and_from_a_private_file() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("key");
    std::fs::write(&f, "file-secret\n").unwrap();
    std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o600)).unwrap();
    let mut notes = vec![];
    let cfg = ConfigV1 { control_token_env: Some("SALT_T110".into()), ..Default::default() };
    assert_eq!(resolve_control_token(&cfg, &env(&[("SALT_T110", "env-secret")]), &mut notes).unwrap().as_deref(), Some("env-secret"));
    let cfg = ConfigV1 { control_token_file: Some(f.to_string_lossy().into_owned()), ..Default::default() };
    assert_eq!(resolve_control_token(&cfg, &env(&[]), &mut notes).unwrap().as_deref(), Some("file-secret"));
    assert!(notes.iter().all(|n| !n.contains("secret")), "notes must never carry the value: {notes:?}");
}

/// Verifies: REQ-SEC-003/AC2 — [RF-3] group-readable file, directory, foreign owner
#[test]
fn unsafe_key_files_are_config_invalid() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("key");
    std::fs::write(&f, "s").unwrap();
    std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o640)).unwrap();
    let cfg = ConfigV1 { control_token_file: Some(f.to_string_lossy().into_owned()), ..Default::default() };
    let e = resolve_control_token(&cfg, &env(&[]), &mut vec![]).unwrap_err();
    assert_eq!(e.key, "control_token_file");
    assert!(e.found.contains("0640"), "{e}");
    let cfg = ConfigV1 { control_token_file: Some(dir.path().to_string_lossy().into_owned()), ..Default::default() };
    assert!(resolve_control_token(&cfg, &env(&[]), &mut vec![]).unwrap_err().found.contains("not a regular file"));
    assert!(validate_key_file(true, 0o600, 1234, 1000).unwrap_err().contains("owned by uid 1234"));
    assert!(validate_key_file(true, 0o600, 1000, 1000).is_ok());
}

/// Verifies: REQ-SEC-003/AC4
#[test]
fn literal_token_still_works_with_a_deprecation_warning() {
    let cfg = ConfigV1 { control_token: Some("lit".into()), ..Default::default() };
    let mut notes = vec![];
    assert_eq!(resolve_control_token(&cfg, &env(&[]), &mut notes).unwrap().as_deref(), Some("lit"));
    assert!(notes.iter().any(|n| n.contains("deprecated")), "{notes:?}");
    let two = ConfigV1 { control_token: Some("a".into()), control_token_env: Some("B".into()), ..Default::default() };
    assert_eq!(resolve_control_token(&two, &env(&[("B", "b")]), &mut vec![]).unwrap_err().key, "control_token");
}

/// Verifies: REQ-SEC-006/AC1
#[test]
fn logs_and_crash_dumps_never_contain_tokens() {
    let r = Redactor::new(vec!["s3cr3t-value".into()]);
    assert_eq!(r.redact("auth ok s3cr3t-value done"), "auth ok <redacted> done");
    assert_eq!(r.redact("Authorization: Bearer abc.def-123"), "Authorization: Bearer <redacted>");
    assert_eq!(r.redact("GET /v1/ensure/stream?profile=A&token=zzz9"), "GET /v1/ensure/stream?profile=A&token=<redacted>");
    let mut app = saltnitor::app::App::new("cpu".into(), 1, 1.0, "gpu".into(), 1.0, false, "127.0.0.1".into(), 8080, "svc".into(), 1, 1);
    app.redactor = r;
    app.add_log("router said Bearer s3cr3t-value".into());
    let dump = app.crash_dump_text("20260101_000000");
    assert!(!dump.contains("s3cr3t-value") && dump.contains("<redacted>"), "{dump}");
}
```

Add `rustix = { version = "1", features = ["process"] }` to `[dependencies]` (justify: `geteuid` here, pidfd in T1.13; Apache-2.0/MIT; already a transitive dependency). Run: `cargo test --test secrets` → compile errors.

- [ ] **Step 2: Implement**

`config_v1.rs` additions:

```rust
/// Pure check behind REQ-SEC-003/AC2: regular file, no group/other bits, owned by us.
pub fn validate_key_file(is_file: bool, mode: u32, owner_uid: u32, my_uid: u32) -> Result<(), String> {
    if !is_file {
        return Err("not a regular file".to_string());
    }
    if mode & 0o077 != 0 {
        return Err(format!("mode {:04o} is readable by group or others; chmod 600", mode & 0o7777));
    }
    if owner_uid != my_uid {
        return Err(format!("owned by uid {owner_uid}, not the daemon user {my_uid}"));
    }
    Ok(())
}

/// Exactly one of `control_token` (deprecated literal), `control_token_env`, `control_token_file`.
pub fn resolve_control_token(cfg: &ConfigV1, env: Env, notes: &mut Vec<String>) -> Result<Option<String>, ConfigError> {
    let err = |key: &str, expected: &str, found: String| ConfigError {
        file: PathBuf::from("config.toml"), line: None, col: None, key: key.into(), expected: expected.into(), found, hint: None,
    };
    let set = [cfg.control_token.is_some(), cfg.control_token_env.is_some(), cfg.control_token_file.is_some()]
        .iter().filter(|b| **b).count();
    if set > 1 {
        return Err(err("control_token", "exactly one of control_token, control_token_env, control_token_file", format!("{set} set")));
    }
    if let Some(t) = &cfg.control_token {
        notes.push("control_token: a literal secret in config.toml is deprecated; use control_token_env or control_token_file (REQ-SEC-003)".to_string());
        return Ok(Some(t.clone()));
    }
    if let Some(name) = &cfg.control_token_env {
        let v = env(name).filter(|v| !v.is_empty()).ok_or_else(|| err("control_token_env", "an environment variable with a value", format!("{name} is unset or empty")))?;
        return Ok(Some(v));
    }
    if let Some(path) = &cfg.control_token_file {
        use std::os::unix::fs::MetadataExt;
        let meta = std::fs::metadata(path).map_err(|e| err("control_token_file", "a readable file", e.to_string()))?;
        let my_uid = rustix::process::geteuid().as_raw();
        validate_key_file(meta.is_file(), meta.mode(), meta.uid(), my_uid).map_err(|m| err("control_token_file", "a private key file", m))?;
        let v = std::fs::read_to_string(path).map_err(|e| err("control_token_file", "a readable file", e.to_string()))?;
        let v = v.trim_end_matches(['\n', '\r']).to_string();
        if v.is_empty() {
            return Err(err("control_token_file", "a non-empty token", "empty file".to_string()));
        }
        return Ok(Some(v));
    }
    Ok(None)
}
```

`auth.rs` addition:

```rust
/// Removes known secret values and credential-shaped substrings from a log line (REQ-SEC-006/AC1).
#[derive(Debug, Clone, Default)]
pub struct Redactor {
    secrets: Vec<String>,
    bearer: Option<regex::Regex>,
    query: Option<regex::Regex>,
}
impl Redactor {
    pub fn new(secrets: Vec<String>) -> Self {
        Self {
            secrets: secrets.into_iter().filter(|s| !s.is_empty()).collect(),
            bearer: regex::Regex::new(r"(?i)(bearer\s+)[A-Za-z0-9._~+/=-]+").ok(),
            query: regex::Regex::new(r"([?&]token=)[^&\s]+").ok(),
        }
    }
    pub fn redact(&self, line: &str) -> String {
        let mut out = line.to_string();
        for s in &self.secrets {
            out = out.replace(s, "<redacted>");
        }
        if let Some(re) = &self.bearer { out = re.replace_all(&out, "${1}<redacted>").into_owned(); }
        if let Some(re) = &self.query { out = re.replace_all(&out, "${1}<redacted>").into_owned(); }
        out
    }
}
```

`app.rs`: field `pub redactor: crate::auth::Redactor` (default `Redactor::default()` in `new`); `add_log` becomes `let log = self.redactor.redact(&log);` before pushing. Add `pub fn crash_dump_text(&self, timestamp: &str) -> String` containing the exact format string from `main.rs`'s Ctrl+D handler (header, target model, VRAM/RAM/temp/power/CPU, then `self.logs`), and make the handler call it (the handler keeps only the file I/O). Build the app's redactor in `main.rs` from `[control_token, infer_bearer, client_bearer]` values that are `Some`. `main.rs` resolves the token with `resolve_control_token` right after `load` (exit 2 on `Err`, notes appended) and passes it to `ControlApi::new` instead of `toml_conf.control_token`.

Run: `cargo test --workspace --locked` → green; fmt/clippy clean.

- [ ] **Step 3: Run Close-out** for T1.10 (`Refs: REQ-SEC-003, REQ-SEC-006`).

---

### Task 12: T1.11 — Streaming proxy core (+ additive fake-runtime features)

**Files:**
- Create: `src/proxy_stream.rs`, `tests/proxy_streaming.rs`
- Modify: `src/control_api.rs` (`h_chat` rewritten; `ControlApi::limits`; request-id layer in `router`), `src/auth.rs` (request_id into the 401 envelope), `src/lib.rs` (`pub mod proxy_stream`), `src/main.rs` (limits from config), `Cargo.toml` (reqwest `stream`, tokio-stream `time`, `uuid`)
- Modify (fake, additive): `tools/fake-llama-server/src/scenario.rs` (`Fault::RawChunks`), `tools/fake-llama-server/src/engine.rs` (raw chunk streaming with send timestamps, in-flight counter, drop-detected disconnect), `tools/fake-llama-server/src/lib.rs` (`Handle::in_flight()`), `tools/fake-llama-server/README.md`

**Interfaces:**
- Produces: `proxy_stream::{ProxyLimits, upstream_client, HOP_BY_HOP, CLIENT_CREDENTIALS, is_hop_by_hop, forwardable_request_headers, forwardable_response_headers, validate_chat_body, read_body_limited, forward, RequestId, request_id_for, request_id_middleware}`; `ControlApi::limits(ProxyLimits) -> Self`; fake: `Fault::RawChunks { items, delay_ms, code, content_type, headers }`, recorder events `chunk_sent {path, index, t_unix_ms}`, `Handle::in_flight() -> usize`.
- Consumes: `error::{ApiError, ErrorCode}`, `config_v1::Timeouts`.

- [ ] **Step 1: Fake runtime additions, test-first (in `tools/fake-llama-server/tests/http.rs`)**

```rust
/// Verifies: REQ-TST-011/AC1 (raw chunk fault; send timestamps; in-flight count; drop-detected disconnect)
#[tokio::test]
async fn raw_chunks_stream_verbatim_with_timestamps_and_track_in_flight() {
    let mut s = Scenario::default();
    s.routes.insert("POST /v1/chat/completions".into(), Fault::RawChunks {
        items: vec![": hi\n\n".into(), "data: [DONE]\n\n".into()], delay_ms: 50, code: 200,
        content_type: "text/event-stream".into(), headers: vec![("x-fake-upstream".into(), "1".into())],
    });
    let h = fake_llama_server::spawn(s).await;
    assert_eq!(h.in_flight(), 0);
    let r = reqwest::Client::new().post(format!("{}/v1/chat/completions", h.base_url())).body("{}").send().await.unwrap();
    assert_eq!(r.headers()["x-fake-upstream"], "1");
    assert_eq!(r.text().await.unwrap(), ": hi\n\ndata: [DONE]\n\n");
    let sent = h.recorder.of_kind("chunk_sent");
    assert_eq!(sent.len(), 2);
    assert!(sent[1]["t_unix_ms"].as_u64().unwrap() >= sent[0]["t_unix_ms"].as_u64().unwrap() + 50);
    assert_eq!(h.in_flight(), 0, "completed streams leave the counter where it was");
}
```

Run: `cargo test -p fake-llama-server raw_chunks` → compile error (no `RawChunks`).

Implement:

`scenario.rs` — add the variant (TOML: `kind = "raw_chunks"`):

```rust
    /// Stream these byte pieces verbatim (no SSE wrapping): the first immediately, then `delay_ms`
    /// before each later piece. Records `chunk_sent {index, t_unix_ms}` per piece.
    RawChunks {
        items: Vec<String>,
        #[serde(default)]
        delay_ms: u64,
        #[serde(default = "default_200")]
        code: u16,
        #[serde(default = "default_sse")]
        content_type: String,
        /// Extra response headers (the proxy must drop the hop-by-hop ones among them).
        #[serde(default)]
        headers: Vec<(String, String)>,
    },
```

with `fn default_200() -> u16 { 200 }` and `fn default_sse() -> String { "text/event-stream".into() }`.

`engine.rs` — `Shared` gains `pub(crate) in_flight: std::sync::atomic::AtomicUsize` (init 0). In `apply_fault`, a new arm:

```rust
        Fault::RawChunks { items, delay_ms, code, content_type, headers } => {
            raw_stream(st.clone(), path, items, delay_ms, code, &content_type, headers)
        }
```

```rust
/// A body stream that reports its own end: normal completion or a drop (client went away).
struct Tracked {
    inner: ReceiverStream<Result<Bytes, std::io::Error>>,
    st: Arc<Shared>,
    path: String,
    finished: bool,
}
impl futures_core::Stream for Tracked {
    type Item = Result<Bytes, std::io::Error>;
    fn poll_next(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
        let r = std::pin::Pin::new(&mut self.inner).poll_next(cx);
        if let std::task::Poll::Ready(None) = r {
            self.finished = true;
        }
        r
    }
}
impl Drop for Tracked {
    fn drop(&mut self) {
        self.st.in_flight.fetch_sub(1, Ordering::SeqCst);
        if !self.finished {
            self.st.recorder.disconnect(&self.path);
        }
    }
}

fn unix_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn raw_stream(st: Arc<Shared>, path: &str, items: Vec<String>, delay_ms: u64, code: u16, content_type: &str, headers: Vec<(String, String)>) -> Response {
    st.in_flight.fetch_add(1, Ordering::SeqCst);
    let (tx, rx) = mpsc::channel::<Result<Bytes, std::io::Error>>(1);
    let path_s = path.to_string();
    let st2 = st.clone();
    tokio::spawn(async move {
        for (i, piece) in items.into_iter().enumerate() {
            if i > 0 && delay_ms > 0 {
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }
            if tx.send(Ok(Bytes::from(piece))).await.is_err() {
                return; // Tracked's Drop records the disconnect
            }
            st2.recorder.record("chunk_sent", json!({ "path": path_s, "index": i, "t_unix_ms": unix_ms() }));
        }
    });
    let mut b = Response::builder().status(StatusCode::from_u16(code).unwrap_or(StatusCode::OK)).header(header::CONTENT_TYPE, content_type);
    for (k, v) in headers {
        b = b.header(k, v);
    }
    b.body(Body::from_stream(Tracked { inner: ReceiverStream::new(rx), st, path: path.to_string(), finished: false }))
        .expect("valid streaming response")
}
```

`futures_core` is already in the tree (via tokio-stream/reqwest); add `futures-core = "0.3"` to the fake's `[dependencies]` only if `cargo build` says it is not resolvable through `tokio_stream::Stream` — prefer `use tokio_stream::Stream;` which re-exports the same trait. `lib.rs`: `pub fn in_flight(&self) -> usize { self.shared.in_flight.load(Ordering::SeqCst) }`. README: document `raw_chunks` and `chunk_sent`. Run: `cargo test -p fake-llama-server` → all green (the 39 existing + 1).

- [ ] **Step 2: Failing proxy tests**

`tests/proxy_streaming.rs`:

```rust
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests may unwrap: a panic is the failure signal
//! The proxy streams byte-exact through Saltnitor's real router (REQ-PRX-001…005, -009; REQ-SEC-012).
use fake_llama_server::{Fault, Handle, Scenario};
use proptest::prelude::*;
use saltnitor::control_api::{serve, ControlApi, ProfileMeta};
use saltnitor::proxy_stream::{is_hop_by_hop, request_id_for, ProxyLimits};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

fn scenario(fault: Fault) -> Scenario {
    let mut s = Scenario::default();
    s.models_shape = fake_llama_server::ModelsShape::Legacy;
    s.models.push(fake_llama_server::ModelSpec { id: "A".into(), loaded: true });
    s.routes.insert("POST /v1/chat/completions".into(), fault);
    s
}
fn raw(items: &[&str], delay_ms: u64) -> Fault {
    Fault::RawChunks { items: items.iter().map(|s| s.to_string()).collect(), delay_ms, code: 200, content_type: "text/event-stream".into(), headers: vec![] }
}
fn profile() -> ProfileMeta { ProfileMeta { model: "a.gguf".into(), offload: false, est_vram_gb: Some(0.1), est_ram_gb: Some(0.1) } }

struct Rig { base: String, fake: Handle, _rx: mpsc::Receiver<saltnitor::events::Event>, http: reqwest::Client }
async fn rig(s: Scenario, upstream_bearer: Option<&str>, limits: ProxyLimits) -> Rig {
    let fake = fake_llama_server::spawn(s).await;
    let (tx, rx) = mpsc::channel(256);
    let api = Arc::new(ControlApi::new(HashMap::from([("A".to_string(), profile())]), fake.base_url(), upstream_bearer.map(str::to_string), None, 0.0, 0.0, tx).limits(limits));
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    tokio::spawn(serve(api, addr));
    let base = format!("http://{addr}");
    let http = reqwest::Client::new();
    for _ in 0..100 {
        if http.get(format!("{base}/healthz")).send().await.is_ok() { break; }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    Rig { base, fake, _rx: rx, http }
}
fn limits() -> ProxyLimits { ProxyLimits { connect: Duration::from_secs(5), first_byte: Duration::from_secs(600), idle: Duration::from_secs(120), max_body_bytes: 33_554_432 } }
fn now_ms() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64 }

/// Verifies: REQ-PRX-002/AC1, REQ-PRX-002/AC2
#[tokio::test]
async fn first_chunk_is_forwarded_before_the_second_is_sent() {
    let r = rig(scenario(raw(&["data: A\n\n", "data: B\n\n"], 2_000)), None, limits()).await;
    let mut resp = r.http.post(format!("{}/v1/chat/completions", r.base)).body(r#"{"model":"A","stream":true}"#).send().await.unwrap();
    let first = resp.chunk().await.unwrap().unwrap();
    let t_recv = now_ms();
    assert_eq!(&first[..], b"data: A\n\n");
    let sent = r.fake.recorder.of_kind("chunk_sent");
    let t_sent_a = sent[0]["t_unix_ms"].as_u64().unwrap();
    assert!(t_recv.saturating_sub(t_sent_a) <= 200, "A took {} ms after the flush (budget 200)", t_recv.saturating_sub(t_sent_a));
    assert_eq!(sent.len(), 1, "B must not have been sent yet");
    let rest = resp.bytes().await.unwrap();
    assert_eq!(&rest[..], b"data: B\n\n");
}

/// Verifies: REQ-PRX-004/AC1, REQ-PRX-004/AC2 — [RF-5] hop-by-hop headers from upstream
#[tokio::test]
async fn status_and_end_to_end_headers_pass_hop_by_hop_dropped_request_id_added() {
    let fault = Fault::RawChunks { items: vec!["x".into()], delay_ms: 0, code: 201, content_type: "text/plain".into(),
        headers: vec![("cache-control".into(), "no-cache".into()), ("x-fake-upstream".into(), "1".into()), ("keep-alive".into(), "timeout=5".into()), ("proxy-connection".into(), "keep-alive".into())] };
    let r = rig(scenario(fault), None, limits()).await;
    let resp = r.http.post(format!("{}/v1/chat/completions", r.base)).body(r#"{"model":"A"}"#).send().await.unwrap();
    assert_eq!(resp.status(), 201);
    assert_eq!(resp.headers()["content-type"], "text/plain");
    assert_eq!(resp.headers()["cache-control"], "no-cache");
    assert_eq!(resp.headers()["x-fake-upstream"], "1");
    assert!(resp.headers().get("keep-alive").is_none() && resp.headers().get("proxy-connection").is_none());
    assert!(resp.headers().get("x-request-id").is_some());
    assert_eq!(resp.text().await.unwrap(), "x");
}

/// Verifies: REQ-SEC-012/AC1, REQ-PRX-005/AC1
#[tokio::test]
async fn client_credentials_are_stripped_and_the_body_is_forwarded_verbatim() {
    let r = rig(scenario(raw(&["ok"], 0)), Some("router-key"), limits()).await;
    let body = r#"{"model":"A",  "messages":[{"role":"user","content":"hi"}] ,"extra":1}"#;
    r.http.post(format!("{}/v1/chat/completions", r.base)).header("authorization", "Bearer client-secret").header("cookie", "a=b").header("proxy-authorization", "Basic x").body(body).send().await.unwrap();
    let reqs = r.fake.recorder.of_kind("request");
    let chat = reqs.iter().filter(|e| e["path"] == "/v1/chat/completions").last().unwrap();
    assert_eq!(chat["body"], body, "bytes must be forwarded unmodified");
    assert_eq!(chat["has_authorization"], true, "the upstream credential, not the client's");
    assert!(chat["headers"].get("cookie").is_none() && chat["headers"].get("proxy-authorization").is_none());
}

/// Verifies: REQ-PRX-009/AC1, REQ-PRX-009/AC2
#[tokio::test]
async fn request_ids_are_kept_when_valid_generated_otherwise_and_forwarded() {
    let r = rig(scenario(raw(&["ok"], 0)), None, limits()).await;
    let resp = r.http.post(format!("{}/v1/chat/completions", r.base)).header("x-request-id", "abc.DEF-123_x").body(r#"{"model":"A"}"#).send().await.unwrap();
    assert_eq!(resp.headers()["x-request-id"], "abc.DEF-123_x");
    let resp = r.http.post(format!("{}/v1/chat/completions", r.base)).header("x-request-id", "bad id!").body(r#"{"model":"A"}"#).send().await.unwrap();
    let generated = resp.headers()["x-request-id"].to_str().unwrap().to_string();
    assert_ne!(generated, "bad id!");
    assert!(uuid::Uuid::parse_str(&generated).is_ok_and(|u| u.get_version_num() == 7));
    let reqs = r.fake.recorder.of_kind("request");
    assert!(reqs.iter().any(|e| e["headers"]["x-request-id"] == "abc.DEF-123_x"));
    let err = r.http.post(format!("{}/v1/chat/completions", r.base)).header("x-request-id", "err-1").body(r#"{"nomodel":true}"#).send().await.unwrap();
    let v: serde_json::Value = err.json().await.unwrap();
    assert_eq!(v["error"]["request_id"], "err-1", "error bodies carry the id");
}

/// Verifies: REQ-PRX-018/AC1 — [RF-2] model present but not a string; top-level array
#[tokio::test]
async fn non_string_model_or_non_object_body_is_400_and_never_forwarded() {
    let r = rig(scenario(raw(&["ok"], 0)), None, limits()).await;
    for body in [r#"{"model":3}"#, r#"[{"model":"A"}]"#, "not json"] {
        let resp = r.http.post(format!("{}/v1/chat/completions", r.base)).body(body).send().await.unwrap();
        assert_eq!(resp.status(), 400, "{body}");
        let v: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(v["error"]["code"], "REQUEST_INVALID");
    }
    assert!(r.fake.recorder.of_kind("request").iter().all(|e| e["path"] != "/v1/chat/completions"));
}

/// Verifies: REQ-PRX-002/AC3 (inspection, mechanised)
#[test]
fn proxy_stream_never_buffers_an_upstream_body() {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/proxy_stream.rs")).unwrap();
    let re = regex::Regex::new(r"\.(bytes|text|json)\(\)").unwrap();
    assert!(!re.is_match(&src), "proxy_stream.rs must stream, not buffer");
}

#[test]
fn request_id_validation_examples() {
    assert_eq!(request_id_for(Some("ok-1")), "ok-1");
    assert_ne!(request_id_for(Some("")), "");
    assert_ne!(request_id_for(Some(&"x".repeat(129))), "x".repeat(129));
    assert!(is_hop_by_hop("Transfer-Encoding", None) && is_hop_by_hop("x-custom", Some("x-custom, close")) && !is_hop_by_hop("content-type", None));
}

fn sse_piece() -> impl Strategy<Value = String> {
    prop_oneof![
        Just(": keep-alive\n\n".to_string()),
        Just("data: [DONE]\n\n".to_string()),
        Just("data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"a\\\":1}\"}}]}}]}\n\n".to_string()),
        Just("data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"hmm\"}}]}\n\n".to_string()),
        "[a-z0-9 ,:{}\\[\\]\"]{0,40}\n?".prop_map(|s| s),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 24, .. ProptestConfig::default() })]
    /// Verifies: REQ-PRX-003/AC1 [PROP]
    #[test]
    fn downstream_bytes_equal_upstream_bytes_for_any_chunking(pieces in prop::collection::vec(sse_piece(), 1..12), code in prop_oneof![Just(200u16), Just(404u16), Just(500u16)], stream in any::<bool>()) {
        let expected: String = pieces.concat();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let got = rt.block_on(async {
            let fault = Fault::RawChunks { items: pieces.clone(), delay_ms: 0, code, content_type: "text/event-stream".into(), headers: vec![] };
            let r = rig(scenario(fault), None, limits()).await;
            let body = if stream { r#"{"model":"A","stream":true}"# } else { r#"{"model":"A"}"# };
            let resp = r.http.post(format!("{}/v1/chat/completions", r.base)).body(body).send().await.unwrap();
            (resp.status().as_u16(), resp.bytes().await.unwrap())
        });
        prop_assert_eq!(got.0, code);
        prop_assert_eq!(&got.1[..], expected.as_bytes());
    }
}
```

Add to `Cargo.toml`: reqwest features `["json", "stream"]`; `tokio-stream = { version = "0.1", features = ["time"] }`; `uuid = { version = "1", features = ["v7"] }` (justify: request IDs, REQ-PRX-009; MIT/Apache-2.0). Run: `cargo test --test proxy_streaming` → compile errors.

- [ ] **Step 3: Implement `src/proxy_stream.rs`**

```rust
//! Streaming proxy core (REQ-PRX-002…005, REQ-PRX-008…011, REQ-PRX-017/018, REQ-SEC-012; INV-01,
//! INV-08). Bytes in, bytes out: the body is never parsed into a value, never re-serialized, and
//! never buffered (`.bytes()`/`.text()`/`.json()` do not appear in this file — REQ-PRX-002/AC3).
use crate::config_v1::{Timeouts, DEFAULT_MAX_BODY_BYTES};
use crate::error::{ApiError, ErrorCode};
use axum::body::{Body, Bytes};
use axum::extract::Request;
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use std::pin::Pin;
use std::sync::LazyLock;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio_stream::Stream;

#[derive(Debug, Clone, Copy)]
pub struct ProxyLimits {
    pub connect: Duration,
    pub first_byte: Duration,
    pub idle: Duration,
    pub max_body_bytes: u64,
}
impl ProxyLimits {
    pub fn from_config(t: &Timeouts, max_body_bytes: Option<u64>) -> Self {
        Self {
            connect: Duration::from_millis(t.connect_ms),
            first_byte: Duration::from_millis(t.first_byte_ms),
            idle: Duration::from_millis(t.idle_ms),
            max_body_bytes: max_body_bytes.unwrap_or(DEFAULT_MAX_BODY_BYTES),
        }
    }
}
impl Default for ProxyLimits {
    fn default() -> Self {
        Self::from_config(&Timeouts::default(), None)
    }
}

/// The upstream client: connect and idle-between-reads timeouts come from config (REQ-PRX-010/AC1).
pub fn upstream_client(l: &ProxyLimits) -> reqwest::Client {
    reqwest::Client::builder().connect_timeout(l.connect).read_timeout(l.idle).build().unwrap_or_default()
}

/// RFC 9110 §7.6.1 (REQ-PRX-004/AC2).
pub const HOP_BY_HOP: [&str; 7] = ["connection", "keep-alive", "proxy-connection", "te", "trailer", "transfer-encoding", "upgrade"];
/// Never forwarded upstream (REQ-SEC-012/AC1).
pub const CLIENT_CREDENTIALS: [&str; 3] = ["authorization", "cookie", "proxy-authorization"];

pub fn is_hop_by_hop(name: &str, connection_header: Option<&str>) -> bool {
    let n = name.to_ascii_lowercase();
    HOP_BY_HOP.contains(&n.as_str())
        || connection_header.is_some_and(|c| c.split(',').any(|t| t.trim().eq_ignore_ascii_case(&n)))
}

fn connection_value(h: &HeaderMap) -> Option<&str> {
    h.get(header::CONNECTION).and_then(|v| v.to_str().ok())
}

/// Request headers that go upstream: end-to-end only, minus credentials and the ones we own.
pub fn forwardable_request_headers(h: &HeaderMap) -> Vec<(HeaderName, HeaderValue)> {
    let conn = connection_value(h);
    h.iter()
        .filter(|(n, _)| {
            let s = n.as_str();
            !is_hop_by_hop(s, conn) && !CLIENT_CREDENTIALS.contains(&s) && s != "host" && s != "content-length" && s != "x-request-id"
        })
        .map(|(n, v)| (n.clone(), v.clone()))
        .collect()
}

/// Response headers that go downstream: end-to-end only (status is copied separately).
pub fn forwardable_response_headers(h: &HeaderMap) -> Vec<(HeaderName, HeaderValue)> {
    let conn = connection_value(h);
    h.iter().filter(|(n, _)| !is_hop_by_hop(n.as_str(), conn)).map(|(n, v)| (n.clone(), v.clone())).collect()
}

#[derive(Deserialize)]
struct ModelProbe {
    model: Option<serde_json::Value>,
}

pub struct Validated {
    pub model: String,
}

/// A JSON object with a string `model` (REQ-PRX-018/AC1). The bytes are only inspected, never
/// re-serialized (AC2): the caller forwards the original buffer.
pub fn validate_chat_body(body: &[u8]) -> Result<Validated, ApiError> {
    let probe: ModelProbe = serde_json::from_slice(body)
        .map_err(|e| ApiError::new(ErrorCode::RequestInvalid, format!("body must be a JSON object: {e}")))?;
    match probe.model {
        Some(serde_json::Value::String(m)) if !m.is_empty() => Ok(Validated { model: m }),
        Some(_) => Err(ApiError::new(ErrorCode::RequestInvalid, "'model' must be a non-empty string")),
        None => Err(ApiError::new(ErrorCode::RequestInvalid, "missing 'model' in request body")),
    }
}

/// Read at most `max` bytes (REQ-PRX-017/AC1). A declared `Content-Length` above the limit is
/// refused before reading anything.
pub async fn read_body_limited(body: Body, content_length: Option<u64>, max: u64) -> Result<Bytes, ApiError> {
    let too_large = || ApiError::new(ErrorCode::PayloadTooLarge, format!("request body exceeds max_body_bytes ({max})"));
    if content_length.is_some_and(|n| n > max) {
        return Err(too_large());
    }
    let limit = usize::try_from(max).unwrap_or(usize::MAX);
    axum::body::to_bytes(body, limit).await.map_err(|_| too_large())
}

pub fn content_length(h: &HeaderMap) -> Option<u64> {
    h.get(header::CONTENT_LENGTH).and_then(|v| v.to_str().ok()).and_then(|s| s.parse().ok())
}

/// Ends the downstream body with an error (hyper aborts the connection) the moment the upstream
/// body fails: no synthetic `[DONE]`, nothing fabricated (REQ-PRX-011/AC1).
struct AbortOnError {
    inner: Pin<Box<dyn Stream<Item = reqwest::Result<Bytes>> + Send>>,
    request_id: String,
    log: Box<dyn Fn(String) + Send + Sync>,
    failed: bool,
}
impl Stream for AbortOnError {
    type Item = Result<Bytes, std::io::Error>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.failed {
            return Poll::Ready(None);
        }
        match self.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(b))) => Poll::Ready(Some(Ok(b))),
            Poll::Ready(Some(Err(e))) => {
                self.failed = true;
                let kind = if e.is_timeout() { "idle timeout between chunks" } else { "upstream failed mid-stream" };
                (self.log)(format!("UPSTREAM_STREAM_ABORTED request_id={} {kind}", self.request_id));
                Poll::Ready(Some(Err(std::io::Error::other("upstream stream aborted"))))
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Forward `body` to `url` and stream the reply back (REQ-PRX-002/003/004/005/008/010).
pub async fn forward(
    client: &reqwest::Client,
    url: &str,
    upstream_bearer: Option<&str>,
    req_headers: &HeaderMap,
    body: Bytes,
    request_id: &str,
    limits: &ProxyLimits,
    log: impl Fn(String) + Send + Sync + 'static,
) -> Response {
    let mut rb = client.post(url).body(body).header("x-request-id", request_id);
    for (n, v) in forwardable_request_headers(req_headers) {
        rb = rb.header(n, v);
    }
    if let Some(b) = upstream_bearer {
        rb = rb.header(header::AUTHORIZATION, format!("Bearer {b}"));
    }
    let upstream = match tokio::time::timeout(limits.first_byte, rb.send()).await {
        Err(_) => return ApiError::new(ErrorCode::UpstreamTimeout, "no response headers from the runtime within first_byte_ms").request_id(request_id).into_response(),
        Ok(Err(e)) if e.is_timeout() => return ApiError::new(ErrorCode::UpstreamTimeout, "the runtime did not answer within the configured timeout").request_id(request_id).into_response(),
        Ok(Err(e)) => return ApiError::new(ErrorCode::RuntimeUnhealthy, format!("runtime connection failed before response headers: {}", e.without_url())).request_id(request_id).into_response(),
        Ok(Ok(r)) => r,
    };
    let status = StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut b = Response::builder().status(status);
    for (n, v) in forwardable_response_headers(upstream.headers()) {
        b = b.header(n, v);
    }
    b = b.header("x-request-id", request_id);
    let stream = AbortOnError { inner: Box::pin(upstream.bytes_stream()), request_id: request_id.to_string(), log: Box::new(log), failed: false };
    b.body(Body::from_stream(stream)).unwrap_or_else(|e| {
        ApiError::new(ErrorCode::RuntimeUnhealthy, format!("could not relay the runtime response: {e}")).request_id(request_id).into_response()
    })
}

// ── Request IDs (REQ-PRX-009) ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RequestId(pub String);

static ID_RE: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"^[A-Za-z0-9._-]{1,128}$").unwrap_or_else(|_| unreachable!("static regex")));

/// Keep a well-formed incoming id; otherwise mint a UUIDv7.
pub fn request_id_for(incoming: Option<&str>) -> String {
    match incoming {
        Some(s) if ID_RE.is_match(s) => s.to_string(),
        _ => uuid::Uuid::now_v7().to_string(),
    }
}

/// Outermost layer: assigns the id, exposes it as an extension, echoes it on every response.
pub async fn request_id_middleware(mut req: Request, next: Next) -> Response {
    let id = request_id_for(req.headers().get("x-request-id").and_then(|v| v.to_str().ok()));
    if let Ok(v) = HeaderValue::from_str(&id) {
        req.headers_mut().insert("x-request-id", v.clone());
        req.extensions_mut().insert(RequestId(id));
        let mut resp = next.run(req).await;
        resp.headers_mut().entry("x-request-id").or_insert(v);
        return resp;
    }
    next.run(req).await
}
```

(`unwrap_or_else(|_| unreachable!(…))` on a literal regex is the one accepted panic path: the pattern is a compile-time constant; clippy's `unwrap_used` is satisfied. `reqwest::Error::without_url()` removes the URL from the message.)

- [ ] **Step 4: Wire `control_api.rs`, `auth.rs`, `main.rs`**

`ControlApi`: field `limits: ProxyLimits` (default), `pub fn limits(mut self, l: ProxyLimits) -> Self { self.http = proxy_stream::upstream_client(&l); self.limits = l; self }`; `new` builds `http` with `upstream_client(&ProxyLimits::default())`. `router()`: add `.layer(axum::middleware::from_fn(proxy_stream::request_id_middleware))` **after** the auth layer (outermost). `auth::middleware` fills `request_id` from `req.extensions().get::<RequestId>()` into its envelopes. New `h_chat`:

```rust
async fn h_chat(State(api): State<Arc<ControlApi>>, Extension(rid): Extension<RequestId>, headers: HeaderMap, body: Body) -> Response {
    let rid = rid.0;
    let body = match read_body_limited(body, content_length(&headers), api.limits.max_body_bytes).await {
        Ok(b) => b,
        Err(e) => return e.request_id(&rid).into_response(),
    };
    let model = match validate_chat_body(&body) {
        Ok(v) => v.model,
        Err(e) => return e.request_id(&rid).into_response(),
    };
    match api.ensure(EnsureRequest { profile: model.clone(), ..Default::default() }, &ProgressSink::None).await {
        EnsureOutcome::Bad(e) => return ApiError::new(ErrorCode::ModelNotFound, format!("unknown model '{model}': {e}")).request_id(&rid).into_response(),
        EnsureOutcome::Oom(i) => return ApiError::new(ErrorCode::OracleRejected, format!("'{model}' would not fit")).details(json!({"need_vram_gb": i.need_vram_gb, "total_vram_gb": i.total_vram_gb, "need_ram_gb": i.need_ram_gb, "total_ram_gb": i.total_ram_gb})).request_id(&rid).into_response(),
        EnsureOutcome::Err(e) => return ApiError::new(ErrorCode::RuntimeStartFailed, format!("load failed: {e}")).request_id(&rid).into_response(),
        EnsureOutcome::Loaded { .. } | EnsureOutcome::AlreadyResident { .. } => {}
    }
    let url = format!("{}/v1/chat/completions", api.router_base);
    let tx = api.tx.clone();
    proxy_stream::forward(&api.http, &url, api.infer_bearer.as_deref(), &headers, body, &rid, &api.limits, move |l| { let _ = tx.try_send(Event::LogLine(l)); }).await
}
```

(Match the exact `EnsureOutcome` variant names in the file; the `_ => {}` arm becomes explicit.) `main.rs`: `.limits(ProxyLimits::from_config(&toml_conf.timeouts, toml_conf.max_body_bytes))`. The T0.6 characterization tests `chat_*` keep passing (same bytes, now streamed); `bd02_first_byte_timing` (ignored probe) → convert to the non-ignored `first_byte_arrives_before_completion_bd02_fixed` asserting the first chunk arrives before the fake's last `chunk_sent` (commit states: BD-02 fixed by T1.11).

Run: `cargo test --workspace --locked` → green; `grep -nE '\.(bytes|text|json)\(\)' src/proxy_stream.rs` → empty; fmt/clippy clean.

- [ ] **Step 5: Run Close-out** for T1.11 (`Refs: REQ-PRX-001, REQ-PRX-002, REQ-PRX-003, REQ-PRX-004, REQ-PRX-005, REQ-PRX-009, REQ-SEC-012, REQ-MIG-007, REQ-TST-008`; trailer `Protected-change: src/control_api.rs test module — bd02 probe becomes the fixed-behaviour test (BD-02 fixed)`).

---

### Task 13: T1.12 — Cancellation, timeouts, failures, limits

**Files:**
- Create: `tests/proxy_failures.rs`
- Modify: `src/proxy_stream.rs` only if a test exposes a gap (the code paths exist since Task 12)

**Interfaces:**
- Consumes: everything from Task 12; fake faults `hang_before_headers`, `crash_after`, `malformed`, `status`, `raw_chunks`; `Handle::in_flight()`.

- [ ] **Step 1: Failing tests**

`tests/proxy_failures.rs` (reuse the `rig`, `scenario`, `raw`, `profile`, `limits` helpers from `tests/proxy_streaming.rs` by moving them into `tests/common/mod.rs` with `pub` items and `mod common;` in both files — do that move in this task):

```rust
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests may unwrap: a panic is the failure signal
mod common;
use common::*;
use fake_llama_server::Fault;
use saltnitor::proxy_stream::ProxyLimits;
use std::time::{Duration, Instant};

fn short() -> ProxyLimits {
    ProxyLimits { connect: Duration::from_millis(500), first_byte: Duration::from_millis(500), idle: Duration::from_millis(300), max_body_bytes: 33_554_432 }
}

/// Verifies: REQ-PRX-006/AC1, REQ-TST-002/AC1 (cancellation)
#[tokio::test]
async fn client_disconnect_reaches_the_fake_within_one_second() {
    let r = rig(scenario(raw(&["data: 1\n\n"; 200], 50)), None, limits()).await;
    let mut resp = r.http.post(format!("{}/v1/chat/completions", r.base)).body(r#"{"model":"A","stream":true}"#).send().await.unwrap();
    let _ = resp.chunk().await.unwrap();
    assert_eq!(r.fake.in_flight(), 1);
    drop(resp);
    let t = Instant::now();
    loop {
        if !r.fake.recorder.of_kind("disconnect").is_empty() && r.fake.in_flight() == 0 { break; }
        assert!(t.elapsed() < Duration::from_secs(1), "fake did not see the close within cancel_propagation_ms");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// Verifies: REQ-PRX-008/AC1, REQ-TST-002/AC1 (4xx/5xx passthrough)
#[tokio::test]
async fn upstream_errors_pass_through_byte_exact() {
    for (code, body) in [(400u16, r#"{"error":{"message":"bad"}}"#), (503, "slot busy"), (500, "")] {
        let r = rig(scenario(Fault::Status { code, body: body.into() }), None, limits()).await;
        let resp = r.http.post(format!("{}/v1/chat/completions", r.base)).body(r#"{"model":"A"}"#).send().await.unwrap();
        assert_eq!(resp.status().as_u16(), code);
        assert_eq!(resp.text().await.unwrap(), body);
    }
}

/// Verifies: REQ-PRX-008/AC2
#[tokio::test]
async fn reset_or_garbage_before_headers_is_502() {
    let r = rig(scenario(Fault::Malformed), None, limits()).await;
    // Malformed on a non-stream chat yields a 200 with a truncated JSON body: that passes through
    // (bytes are the runtime's). The 502 is for a connection that dies before headers:
    drop(r);
    let dead = rig(scenario(raw(&["x"], 0)), None, short()).await;
    let base = dead.base.clone();
    let port = dead.fake.addr.port();
    drop(dead.fake); // the upstream goes away; the next connect is refused
    let (tx, _rx) = tokio::sync::mpsc::channel(8);
    let api = std::sync::Arc::new(saltnitor::control_api::ControlApi::new(
        std::collections::HashMap::from([("A".to_string(), profile())]), format!("http://127.0.0.1:{port}"), None, None, 0.0, 0.0, tx).limits(short()));
    let _ = base; // the earlier server is gone with its rig; serve a fresh one
    let p = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], p));
    tokio::spawn(saltnitor::control_api::serve(api, addr));
    tokio::time::sleep(Duration::from_millis(50)).await;
    let resp = reqwest::Client::new().post(format!("http://{addr}/v1/chat/completions")).body(r#"{"model":"A"}"#).send().await.unwrap();
    let v: serde_json::Value = resp.json().await.unwrap();
    assert!(["RUNTIME_UNHEALTHY", "RUNTIME_START_FAILED"].contains(&v["error"]["code"].as_str().unwrap()), "{v}");
}

/// Verifies: REQ-PRX-010/AC2, REQ-TST-002/AC1 (timeouts)
#[tokio::test]
async fn hang_before_headers_is_504() {
    let r = rig(scenario(Fault::HangBeforeHeaders), None, short()).await;
    let resp = r.http.post(format!("{}/v1/chat/completions", r.base)).body(r#"{"model":"A"}"#).send().await.unwrap();
    assert_eq!(resp.status(), 504);
    let v: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(v["error"]["code"], "UPSTREAM_TIMEOUT");
}

/// Verifies: REQ-PRX-011/AC1, REQ-PRX-010/AC1 (idle timeout)
#[tokio::test]
async fn mid_stream_crash_truncates_without_done_and_logs_the_abort() {
    let mut r = rig(scenario(Fault::CrashAfter { chunks: 2 }), None, limits()).await;
    let mut rx = r.take_events();
    let resp = r.http.post(format!("{}/v1/chat/completions", r.base)).body(r#"{"model":"A","stream":true}"#).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let got = resp.bytes().await;                       // reqwest reports the truncation as an error
    let text = match got { Ok(b) => String::from_utf8_lossy(&b).into_owned(), Err(_) => String::new() };
    assert!(!text.contains("[DONE]"), "no synthetic [DONE]: {text}");
    tokio::time::sleep(Duration::from_millis(50)).await;
    let mut logged = false;
    while let Ok(ev) = rx.try_recv() { if let saltnitor::events::Event::LogLine(l) = ev && l.starts_with("UPSTREAM_STREAM_ABORTED request_id=") { logged = true; } }
    assert!(logged, "abort must be recorded with the request id");
    // idle timeout: a stream that stalls longer than idle_ms is aborted the same way
    let r2 = rig(scenario(raw(&["data: a\n\n", "data: b\n\n"], 2_000)), None, short()).await;
    let resp = r2.http.post(format!("{}/v1/chat/completions", r2.base)).body(r#"{"model":"A","stream":true}"#).send().await.unwrap();
    let t = Instant::now();
    let got = resp.bytes().await;
    assert!(t.elapsed() < Duration::from_millis(1_500), "idle_ms=300 must abort before the 2 s chunk");
    assert!(got.map(|b| !b.ends_with(b"data: b\n\n")).unwrap_or(true));
}

/// Verifies: REQ-PRX-017/AC1
#[tokio::test]
async fn thirty_three_mib_is_413_and_thirty_one_mib_passes() {
    let r = rig(scenario(raw(&["ok"], 0)), None, limits()).await;
    let big = |mib: usize| { let pad = "x".repeat(mib * 1024 * 1024); format!(r#"{{"model":"A","pad":"{pad}"}}"#) };
    let resp = r.http.post(format!("{}/v1/chat/completions", r.base)).body(big(33)).send().await.unwrap();
    assert_eq!(resp.status(), 413);
    let v: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(v["error"]["code"], "PAYLOAD_TOO_LARGE");
    let resp = r.http.post(format!("{}/v1/chat/completions", r.base)).body(big(31)).send().await.unwrap();
    assert_eq!(resp.status(), 200);
}

/// Verifies: REQ-PRX-018/AC1
#[tokio::test]
async fn missing_model_is_400() {
    let r = rig(scenario(raw(&["ok"], 0)), None, limits()).await;
    let resp = r.http.post(format!("{}/v1/chat/completions", r.base)).body(r#"{"messages":[]}"#).send().await.unwrap();
    assert_eq!(resp.status(), 400);
}
```

`tests/common/mod.rs` exposes `pub struct Rig { pub base, pub fake, pub http, rx: Option<Receiver<Event>> }` with `pub fn take_events(&mut self) -> Receiver<Event>`; adjust `rig` accordingly (`let mut r = rig(...)`). Run: `cargo test --test proxy_failures` → red for the reasons the test names (compile first, then behaviour).

- [ ] **Step 2: Make them pass**

Expected gaps and their fixes, in order of likelihood:
- *Idle timeout not firing:* `read_timeout` applies per read; confirm the client in `ControlApi::limits` is the one `forward` uses (`&api.http`).
- *413 arrives as hyper's own 413 or a connection reset for the 33 MiB body:* the `Content-Length` pre-check returns the envelope before reading; if the client streams without a length, `to_bytes` errors → envelope. Both paths are in `read_body_limited`.
- *Disconnect test slow:* the fake's `Tracked` drop fires when hyper drops the body; the 50 ms chunk cadence guarantees a failed send within ≤ 100 ms regardless.

Never adjust the thresholds in the tests: 1 s, 504, 413/200, 400 are the spec's.

Run: `cargo test --workspace --locked` → green; fmt/clippy clean.

- [ ] **Step 3: Run Close-out** for T1.12 (`Refs: REQ-PRX-006, REQ-PRX-008, REQ-PRX-010, REQ-PRX-011, REQ-PRX-017, REQ-PRX-018, REQ-TST-002`).

---

### Task 14: T1.13 — PID-based process control

**Files:**
- Create: `src/process.rs`, `tests/process_control.rs`
- Modify: `src/lib.rs` (`pub mod process`), `src/events.rs` (`HardwareUpdate.processes`, `users`), `src/app.rs` (process fields, `pending_kill`, `confirm_line`), `src/main.rs` (hw poller, preflight without `killall`, inspector keys), `src/ui.rs` (rows, bottom line, help), `src/snapshots/saltnitor__ui__tests__{gpu_inspector,cpu_inspector,help}.snap` (protected), `scripts/invariants-baseline.txt` (kill-by-name entries removed)

**Interfaces:**
- Produces: `process::{ProcessInfo, GpuApp, parse_compute_apps, table, Identity, identity, snapshot_from_sysinfo, Target, Guard, Refusal, TermOutcome, check, terminate, kill, runtime_tree}` with the signatures in Step 2.
- Consumes: `error::ErrorCode::{ProcessChanged, ProcessProtected}`; `config_v1::ProcessCfg::term_grace_ms`; `rustix::process`.

- [ ] **Step 1: Failing tests**

`tests/process_control.rs`:

```rust
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests may unwrap: a panic is the failure signal
//! Real processes, exact PIDs (REQ-PROC-001…007).
use proptest::prelude::*;
use saltnitor::error::ErrorCode;
use saltnitor::process::{check, identity, kill, parse_compute_apps, table, terminate, Guard, Identity, ProcessInfo, Refusal, Target, TermOutcome};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

fn sleeper() -> Child {
    Command::new("sleep").arg("300").stdout(Stdio::null()).spawn().unwrap()
}
fn info(child: &Child) -> ProcessInfo {
    let id = identity(child.id()).expect("child is alive");
    ProcessInfo { pid: child.id(), name: "sleep".into(), memory_bytes: 0, gpu_memory_bytes: None, gpu_memory_reason: None, command: None, start_time_ticks: id.start_time_ticks, uid: id.uid }
}
fn own_guard() -> Guard { Guard::current(vec![]) }
fn alive(pid: u32) -> bool { identity(pid).is_some() }

/// Verifies: REQ-PROC-003/AC1, REQ-PROC-003/AC3, REQ-PROC-006/AC1
#[test]
fn terminating_one_of_two_same_named_processes_leaves_the_other_alive() {
    let (mut a, mut b) = (sleeper(), sleeper());
    let t = Target::select(&info(&a));
    assert_eq!(terminate(&t, &own_guard(), Duration::from_secs(5)).unwrap(), TermOutcome::Exited);
    assert!(alive(b.id()), "the same-named sibling must keep running");
    let _ = a.wait();
    b.kill().unwrap(); let _ = b.wait();
}

/// Verifies: REQ-PROC-003/AC1, REQ-PROC-003/AC2
#[test]
fn sigterm_ignorer_is_still_running_then_kill_ends_it() {
    let mut c = Command::new("sh").args(["-c", "trap '' TERM; exec sleep 300"]).stdout(Stdio::null()).spawn().unwrap();
    std::thread::sleep(Duration::from_millis(200)); // let exec happen
    let t = Target::select(&info(&c));
    assert_eq!(terminate(&t, &own_guard(), Duration::from_millis(300)).unwrap(), TermOutcome::StillRunning);
    assert!(alive(c.id()));
    kill(&t, &own_guard()).unwrap();
    let _ = c.wait();
    assert!(!alive(c.id()));
}

/// Verifies: REQ-PROC-004/AC1 — [RF-4] identity drift between snapshot and action
#[test]
fn changed_identity_is_refused_and_nothing_is_signalled() {
    let mut c = sleeper();
    let mut stale = info(&c);
    stale.start_time_ticks += 1; // a recycled PID would differ here
    let t = Target::without_pidfd(&stale);
    let e = terminate(&t, &own_guard(), Duration::from_millis(100)).unwrap_err();
    assert!(matches!(e, Refusal::Changed(_)));
    assert_eq!(e.code(), ErrorCode::ProcessChanged);
    assert!(alive(c.id()));
    c.kill().unwrap(); let _ = c.wait();
}

/// Verifies: REQ-PROC-005/AC1
#[test]
fn protected_targets_are_refused_with_a_reason() {
    let one = Target::without_pidfd(&ProcessInfo { pid: 1, name: "init".into(), memory_bytes: 0, gpu_memory_bytes: None, gpu_memory_reason: None, command: None, start_time_ticks: 0, uid: 0 });
    assert!(matches!(check(&one, &own_guard()), Err(Refusal::Protected(r)) if r.contains("PID 1")));
    let my = identity(std::process::id()).unwrap();
    let me = ProcessInfo { pid: std::process::id(), name: "saltnitor".into(), memory_bytes: 0, gpu_memory_bytes: None, gpu_memory_reason: None, command: None, start_time_ticks: my.start_time_ticks, uid: my.uid };
    assert!(matches!(check(&Target::without_pidfd(&me), &own_guard()), Err(Refusal::Protected(r)) if r.contains("itself")));
    let mut c = sleeper();
    let g = Guard { own_uids: vec![], self_pid: std::process::id(), runtime_pids: vec![] };
    assert!(matches!(check(&Target::select(&info(&c)), &g), Err(Refusal::Protected(r)) if r.contains("owned by uid")));
    let g = Guard { own_uids: own_guard().own_uids, self_pid: std::process::id(), runtime_pids: vec![c.id()] };
    assert!(matches!(check(&Target::select(&info(&c)), &g), Err(Refusal::Protected(r)) if r.contains("runtime")));
    c.kill().unwrap(); let _ = c.wait();
}

/// Verifies: REQ-PROC-004/AC2 (pidfd where available; kill(2) fallback is exercised explicitly)
#[test]
fn pidfd_is_taken_at_selection_and_the_fallback_also_works() {
    let mut c = sleeper();
    let t = Target::select(&info(&c));
    assert!(t.has_pidfd(), "Linux ≥ 5.3 provides pidfd_open");
    let fallback = Target::without_pidfd(&info(&c));
    assert_eq!(terminate(&fallback, &own_guard(), Duration::from_secs(5)).unwrap(), TermOutcome::Exited);
    let _ = c.wait();
}

/// Verifies: REQ-PROC-007/AC1
#[test]
fn nvidia_smi_compute_apps_parse_with_null_memory_and_a_reason() {
    let csv = "4242, /usr/bin/llama-server, 6144\n4243, python3, [N/A]\n";
    let apps = parse_compute_apps(csv);
    assert_eq!(apps[0].pid, 4242);
    assert_eq!(apps[0].used_memory_bytes, Some(6144 * 1024 * 1024));
    assert_eq!(apps[1].used_memory_bytes, None);
    assert!(apps[1].reason.as_deref().unwrap().contains("[N/A]"));
}

fn raw_row() -> impl Strategy<Value = ProcessInfo> {
    (1u32..50_000, prop_oneof![Just("sleep"), Just("python3"), Just("llama-server")], 0u64..1 << 34).prop_map(|(pid, name, mem)| ProcessInfo {
        pid, name: name.into(), memory_bytes: mem, gpu_memory_bytes: None, gpu_memory_reason: None, command: None, start_time_ticks: 1, uid: 1000,
    })
}

proptest! {
    /// Verifies: REQ-PROC-002/AC1 [PROP], REQ-PROC-001/AC1
    #[test]
    fn listing_has_exactly_one_entry_per_pid_whatever_the_names(rows in prop::collection::vec(raw_row(), 0..40)) {
        let out = table(rows.clone(), &[]);
        let mut pids: Vec<u32> = rows.iter().map(|r| r.pid).collect();
        pids.sort(); pids.dedup();
        let mut got: Vec<u32> = out.iter().map(|r| r.pid).collect();
        got.sort();
        prop_assert_eq!(got.clone(), pids);
        prop_assert_eq!(got.len(), out.len(), "no duplicate PIDs");
    }
}
```

Run: `cargo test --test process_control` → compile errors.

- [ ] **Step 2: Implement `src/process.rs`**

```rust
//! PID-based process control (REQ-PROC-001…007, REQ-TUI-010; INV-09). Every action targets one
//! PID, is identity-checked against the snapshot the operator acted on, and is signalled through
//! a pidfd taken at selection time where the kernel supports it.
use crate::error::ErrorCode;
use rustix::process::{Pid, PidfdFlags, Signal};
use std::os::fd::OwnedFd;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub memory_bytes: u64,
    pub gpu_memory_bytes: Option<u64>,
    /// Why `gpu_memory_bytes` is `None` (INV-18: never a silent 0).
    pub gpu_memory_reason: Option<String>,
    pub command: Option<String>,
    pub start_time_ticks: u64,
    pub uid: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GpuApp {
    pub pid: u32,
    pub name: String,
    pub used_memory_bytes: Option<u64>,
    pub reason: Option<String>,
}

/// `nvidia-smi --query-compute-apps=pid,process_name,used_memory --format=csv,noheader,nounits`
/// (`used_memory` is MiB; `[N/A]` and non-numbers become `None` with the raw text as the reason).
pub fn parse_compute_apps(csv: &str) -> Vec<GpuApp> {
    csv.lines().filter_map(|line| {
        let mut f = line.split(',').map(str::trim);
        let pid = f.next()?.parse().ok()?;
        let name = f.next()?.to_string();
        let raw = f.next().unwrap_or("");
        let (used_memory_bytes, reason) = match raw.parse::<u64>() {
            Ok(mib) => (Some(mib * 1024 * 1024), None),
            Err(_) => (None, Some(format!("nvidia-smi reported {raw:?} for used_memory"))),
        };
        Some(GpuApp { pid, name, used_memory_bytes, reason })
    }).collect()
}

/// One entry per PID (REQ-PROC-002/AC1); GPU rows merge by PID. Rows keep input order.
pub fn table(raw: Vec<ProcessInfo>, gpu: &[GpuApp]) -> Vec<ProcessInfo> {
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<ProcessInfo> = raw.into_iter().filter(|p| seen.insert(p.pid)).collect();
    for g in gpu {
        if let Some(p) = out.iter_mut().find(|p| p.pid == g.pid) {
            p.gpu_memory_bytes = g.used_memory_bytes;
            p.gpu_memory_reason = g.reason.clone();
        } else if let Some(id) = identity(g.pid) {
            out.push(ProcessInfo { pid: g.pid, name: g.name.clone(), memory_bytes: 0, gpu_memory_bytes: g.used_memory_bytes, gpu_memory_reason: g.reason.clone(), command: None, start_time_ticks: id.start_time_ticks, uid: id.uid });
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identity {
    pub start_time_ticks: u64,
    pub uid: u32,
}

/// `/proc/<pid>/stat` field 22 and `/proc/<pid>/status` `Uid:`. `None` when the PID is gone or a
/// zombie (a reaped-but-not-waited child counts as exited).
pub fn identity(pid: u32) -> Option<Identity> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after = &stat[stat.rfind(')')? + 1..];
    let fields: Vec<&str> = after.split_whitespace().collect(); // fields[0] is field 3 (state)
    if fields.first() == Some(&"Z") {
        return None;
    }
    let start_time_ticks = fields.get(19)?.parse().ok()?; // field 22
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let uid = status.lines().find_map(|l| l.strip_prefix("Uid:"))?.split_whitespace().next()?.parse().ok()?;
    Some(Identity { start_time_ticks, uid })
}

/// Build the raw table from sysinfo (name, memory, command) plus `/proc` identity.
pub fn snapshot_from_sysinfo(sys: &sysinfo::System) -> Vec<ProcessInfo> {
    sys.processes().values().filter_map(|p| {
        let pid = p.pid().as_u32();
        let id = identity(pid)?;
        let command = p.cmd().iter().map(|s| s.to_string_lossy()).collect::<Vec<_>>().join(" ");
        Some(ProcessInfo {
            pid, name: p.name().to_string_lossy().into_owned(), memory_bytes: p.memory(),
            gpu_memory_bytes: None, gpu_memory_reason: Some("not a GPU process".into()),
            command: (!command.is_empty()).then_some(command), start_time_ticks: id.start_time_ticks, uid: id.uid,
        })
    }).collect()
}

/// What the operator selected: PID + identity snapshot + a pidfd when the kernel gives one.
#[derive(Debug)]
pub struct Target {
    pub pid: u32,
    pub name: String,
    pub identity: Identity,
    pidfd: Option<OwnedFd>,
}

impl Target {
    pub fn select(info: &ProcessInfo) -> Self {
        let pidfd = Pid::from_raw(i32::try_from(info.pid).unwrap_or(0))
            .and_then(|p| rustix::process::pidfd_open(p, PidfdFlags::empty()).ok());
        Self { pid: info.pid, name: info.name.clone(), identity: Identity { start_time_ticks: info.start_time_ticks, uid: info.uid }, pidfd }
    }
    /// The `kill(2)` fallback path (kernels without pidfd), also used by tests.
    pub fn without_pidfd(info: &ProcessInfo) -> Self {
        Self { pid: info.pid, name: info.name.clone(), identity: Identity { start_time_ticks: info.start_time_ticks, uid: info.uid }, pidfd: None }
    }
    pub fn has_pidfd(&self) -> bool {
        self.pidfd.is_some()
    }
}

/// Who may be signalled (REQ-PROC-005). `own_uids` = {euid, ruid, SUDO_UID} (design P1-D8).
#[derive(Debug, Clone)]
pub struct Guard {
    pub own_uids: Vec<u32>,
    pub self_pid: u32,
    pub runtime_pids: Vec<u32>,
}
impl Guard {
    pub fn current(runtime_pids: Vec<u32>) -> Self {
        let mut own_uids = vec![rustix::process::geteuid().as_raw(), rustix::process::getuid().as_raw()];
        if let Some(s) = std::env::var("SUDO_UID").ok().and_then(|v| v.parse().ok()) {
            own_uids.push(s);
        }
        own_uids.dedup();
        Self { own_uids, self_pid: std::process::id(), runtime_pids }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    Changed(String),
    Protected(String),
}
impl Refusal {
    pub fn code(&self) -> ErrorCode {
        match self { Refusal::Changed(_) => ErrorCode::ProcessChanged, Refusal::Protected(_) => ErrorCode::ProcessProtected }
    }
    pub fn reason(&self) -> &str {
        match self { Refusal::Changed(r) | Refusal::Protected(r) => r }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TermOutcome { Exited, StillRunning }

/// Structural protection, then identity, then ownership. Nothing is signalled on `Err`.
pub fn check(t: &Target, g: &Guard) -> Result<(), Refusal> {
    if t.pid == 1 {
        return Err(Refusal::Protected("PID 1 is never a target".into()));
    }
    if t.pid == g.self_pid {
        return Err(Refusal::Protected("saltnitor will not signal itself".into()));
    }
    if g.runtime_pids.contains(&t.pid) {
        return Err(Refusal::Protected(format!("PID {} belongs to the runtime's process tree; stop the runtime through its lifecycle (Ctrl+K)", t.pid)));
    }
    match identity(t.pid) {
        Some(now) if now == t.identity => {}
        Some(now) => return Err(Refusal::Changed(format!("PID {} changed since selection (start {} → {}, uid {} → {})", t.pid, t.identity.start_time_ticks, now.start_time_ticks, t.identity.uid, now.uid))),
        None => return Err(Refusal::Changed(format!("PID {} no longer exists", t.pid))),
    }
    if !g.own_uids.contains(&t.identity.uid) {
        return Err(Refusal::Protected(format!("PID {} is owned by uid {}, not the operator", t.pid, t.identity.uid)));
    }
    Ok(())
}

fn send(t: &Target, sig: Signal) -> std::io::Result<()> {
    match &t.pidfd {
        Some(fd) => rustix::process::pidfd_send_signal(fd, sig).map_err(std::io::Error::from),
        None => {
            let pid = Pid::from_raw(i32::try_from(t.pid).unwrap_or(0)).ok_or_else(|| std::io::Error::other("invalid pid"))?;
            rustix::process::kill_process(pid, sig).map_err(std::io::Error::from)
        }
    }
}

/// SIGTERM the exact PID, wait up to `grace`, report (REQ-PROC-003/AC1). Blocking: call from
/// `spawn_blocking`.
pub fn terminate(t: &Target, g: &Guard, grace: Duration) -> Result<TermOutcome, Refusal> {
    check(t, g)?;
    if let Err(e) = send(t, Signal::TERM) {
        return Err(Refusal::Changed(format!("SIGTERM to {} failed: {e}", t.pid)));
    }
    let deadline = Instant::now() + grace;
    loop {
        if identity(t.pid) != Some(t.identity) {
            return Ok(TermOutcome::Exited);
        }
        if Instant::now() >= deadline {
            return Ok(TermOutcome::StillRunning);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// SIGKILL, only after a separate confirmation (REQ-PROC-003/AC2).
pub fn kill(t: &Target, g: &Guard) -> Result<(), Refusal> {
    check(t, g)?;
    send(t, Signal::KILL).map_err(|e| Refusal::Changed(format!("SIGKILL to {} failed: {e}", t.pid)))
}

/// The external unit's main PID and its descendants (`systemctl show -p MainPID --value`).
pub fn runtime_tree(service_name: &str) -> Vec<u32> {
    let out = std::process::Command::new("systemctl").args(["show", "-p", "MainPID", "--value", service_name]).output();
    let Some(root) = out.ok().and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<u32>().ok()).filter(|p| *p > 0) else {
        return vec![];
    };
    let mut tree = vec![root];
    let mut i = 0;
    while i < tree.len() {
        let parent = tree[i];
        if let Ok(rd) = std::fs::read_dir("/proc") {
            for e in rd.flatten() {
                let Ok(pid) = e.file_name().to_string_lossy().parse::<u32>() else { continue };
                let ppid = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()
                    .and_then(|s| s.lines().find_map(|l| l.strip_prefix("PPid:")).and_then(|v| v.trim().parse::<u32>().ok()));
                if ppid == Some(parent) && !tree.contains(&pid) {
                    tree.push(pid);
                }
            }
        }
        i += 1;
    }
    tree
}
```

Run: `cargo test --test process_control` → green.

- [ ] **Step 3: Wire the TUI (events, app, poller, keys, rendering, snapshots)**

- `events.rs`: `HardwareUpdate` replaces `gpu_processes: Vec<(String, f64)>` and `sys_processes: Vec<(String, f64)>` with `processes: Vec<ProcessInfo>` and `users: HashMap<u32, String>`.
- `app.rs`: `gpu_processes: Vec<ProcessInfo>` (rows with `gpu_memory_bytes.is_some()`, or a reason other than "not a GPU process"), `sys_processes: Vec<ProcessInfo>` (memory > 1 MiB, descending), `users: HashMap<u32, String>`, `pending_kill: Option<Target>`, `confirm_line: Option<String>`.
- `main.rs` poller: build `raw = process::snapshot_from_sysinfo(&sys)`; GPU apps from `nvidia-smi --query-compute-apps=pid,process_name,used_memory --format=csv,noheader,nounits` through `parse_compute_apps`; `processes = process::table(raw, &gpu)`; users via `sysinfo::Users::new_with_refreshed_list()` every 30th poll (uid → name, fallback the uid as text). Remove the name-dedupe block entirely (BD-05).
- `main.rs` preflight: `required_cmds = ["journalctl", "ss", "systemctl"]`.
- Inspector keys (both inspectors): `x` / `Delete` → `let t = Target::select(info); let svc = app.service_name.clone(); let grace = Duration::from_millis(app.term_grace_ms); tokio::task::spawn_blocking(move || { let g = Guard::current(process::runtime_tree(&svc)); process::terminate(&t, &g, grace) })` then map the result to a log line: `>>> PROCESS: SIGTERM {pid} ({name}) → exited` / `→ still_running after {grace} ms; press X to SIGKILL` / `>>> PROCESS: refused {CODE}: {reason}`. `X` → `app.pending_kill = Some(Target::select(info)); app.confirm_line = Some(format!("SIGKILL {} ({})? [y/N]", pid, name))`. While `pending_kill.is_some()`: `y` → `spawn_blocking(kill)` and log `>>> PROCESS: SIGKILL {pid} ({name}) sent` or the refusal; any other key → clear both and log `>>> PROCESS: kill cancelled`. `app.term_grace_ms` comes from `toml_conf.process.term_grace_ms`.
- `ui.rs` rows: `format!("  {:>6} {:<8.8} {:<14.14} {:>6.1}G {:>6}", p.pid, user, p.name, ram_gb, vram)` where `vram` is `format!("{:.1}G", gb)` or `"n/a"`; bottom title = `confirm_line` when set, else the selected row's `command` (or `[Up/Dn] Target | [x] Term | [X] Kill` when nothing is selected). Help screen adds `  [x] Terminate target (SIGTERM) | [X] Kill (asks y/N)` and drops the SIGKILL wording from `[Up/Dn] Cycle … Sniper Targets`.
- Snapshot fixtures in `ui.rs` tests: `ProcessInfo { pid: 4242, name: "llama-server".into(), memory_bytes: 9_500_000_000, gpu_memory_bytes: Some(6_100_000_000), … uid: 1000 }` etc., `users = {1000: "laz"}` → **no**: use `"operator"` as the fixture user name (never a real name). Add a snapshot `cpu_inspector_confirm` with `confirm_line = Some("SIGKILL 4242 (llama-server)? [y/N]")`.

Run: `INSTA_UPDATE=always cargo test -p saltnitor ui::` then `git diff src/snapshots` — review that only the three expected snapshots changed plus one new file; `cargo test --workspace --locked` green; `scripts/check-invariants.sh` → remove the stale `kill-by-name` ratchet lines → `invariants: clean`. `cargo tree -i rustix -e features | grep process` shows the feature.

- [ ] **Step 4: Run Close-out** for T1.13 (`Refs: REQ-PROC-001…007, REQ-TUI-010, REQ-TST-001, REQ-TST-008`; trailer `Protected-change: src/snapshots/saltnitor__ui__tests__{gpu_inspector,cpu_inspector,help}.snap (+ cpu_inspector_confirm) — Processes screen shows PID/user/RAM/VRAM and x/X keys (REQ-TUI-010); scripts/invariants-baseline.txt shrinks`). PROGRESS note: `killall gone (BD-05, BD-26 fixed); pidfd; 7 tests + 1 prop`.

---

### Task 15: T1.14 — Error surfacing and panic removal

**Files:**
- Modify: `src/events.rs` (`Event::Error`), `src/app.rs` (`last_error`), `src/ui.rs` (status marker), `src/control_api.rs` (`serve` reports via events), `src/main.rs` (journal spawn, every `unwrap`/`expect` on fallible paths), `src/lib.rs` (drop the `#![expect]`)
- Create: `tests/error_surfacing.rs`

**Interfaces:**
- Produces: `Event::Error { source: String, message: String }`; `App::last_error: Option<String>`.

- [ ] **Step 1: Failing test**

```rust
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests may unwrap: a panic is the failure signal
use saltnitor::control_api::{serve, ControlApi};
use saltnitor::events::Event;
use std::sync::Arc;

/// Verifies: REQ-ERR-005/AC1 (bind failure is a visible event, not an eprintln behind the TUI)
#[tokio::test]
async fn occupied_control_port_produces_an_error_event() {
    let holder = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = holder.local_addr().unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::channel(8);
    let api = Arc::new(ControlApi::new(Default::default(), "http://127.0.0.1:1".into(), None, None, 0.0, 0.0, tx));
    serve(api, addr).await; // returns once binding failed
    let ev = rx.recv().await.unwrap();
    match ev {
        Event::Error { source, message } => { assert_eq!(source, "control_api"); assert!(message.contains(&addr.to_string()) && message.contains("bind")); }
        other => panic!("expected Event::Error, got {other:?}"),
    }
}
```

Run: `cargo test --test error_surfacing` → compile error (no `Event::Error`).

- [ ] **Step 2: Implement**

- `events.rs`: `/// A background failure the operator must see (REQ-ERR-005/AC1)\n Error { source: String, message: String },`.
- `control_api.rs` `serve`: both `eprintln!` paths become `let _ = api.tx.send(Event::Error { source: "control_api".into(), message: format!("bind {addr} failed: {e}") }).await;` (and the router-build error likewise).
- `main.rs` event loop: `Event::Error { source, message } => { app.last_error = Some(format!("{source}: {message}")); app.add_log(format!(">>> ERROR [{source}]: {message}")); }`; `ui.rs` header line shows `Span::styled(format!(" ⚠ {} ", e), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))` when `last_error` is `Some` — add a snapshot `dashboard_with_error` (protected; trailer).
- Journal task: `match Command::new("journalctl")….spawn() { Ok(c) => c, Err(e) => { let _ = tx_logs.send(Event::Error { source: "journal".into(), message: format!("cannot spawn journalctl: {e}") }).await; return; } }`; `child.stdout.take()` → `let Some(stdout) = … else { send Event::Error …; return; }`.
- Remove `#![expect(clippy::unwrap_used, clippy::expect_used …)]` from `src/lib.rs` and `src/main.rs`, run clippy, and fix every reported site on I/O, parsing, subprocess, and channel paths with `?`, `let … else`, `unwrap_or_else` producing an `Event::Error`/log line, or `match`. Lines 242–243 / 364 / 366 (BD-16) are the known ones. `main()` already returns `Result<(), Box<dyn Error>>`: terminal setup errors propagate with `?`.

Run: `cargo clippy --all-targets --all-features --locked -- -D warnings` → clean with no `expect` attribute left for `unwrap_used`; `cargo test --workspace --locked` green; `INSTA_UPDATE=always cargo test -p saltnitor ui::` for the new snapshot only.

- [ ] **Step 3: Run Close-out** for T1.14 (`Refs: REQ-ERR-004, REQ-ERR-005`; trailer `Protected-change: src/snapshots/saltnitor__ui__tests__dashboard_with_error.snap — new snapshot`). PROGRESS note: `BD-15, BD-16 fixed; 0 unwrap/expect on fallible paths; unwrap_used deny active without exceptions`.

---

### Task 16: T1.15 — Interrogator truthfulness

**Files:**
- Create: `src/interrogate.rs`, `tests/data/sse/with_timings.txt`, `tests/data/sse/without_timings.txt`
- Modify: `src/lib.rs` (`pub mod interrogate`), `src/events.rs` (`ApiStreamEnd { metrics: Metrics, status }`), `src/app.rs` (`last_metrics`, history path), `src/main.rs` (interrogator task → Saltnitor's endpoint; `timings_per_token`), `src/ui.rs` (deck shows PP/TG or `n/a`/`est.`), `src/snapshots/saltnitor__ui__tests__dashboard.snap` (protected)

**Interfaces:**
- Produces: `interrogate::{Metrics, Source, metrics_from_transcript, parse_timings, render, history_path}`.

- [ ] **Step 1: Failing unit tests (inside `src/interrogate.rs`)**

```rust
#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)] // tests may unwrap: a panic is the failure signal
    use super::*;

    /// Verifies: REQ-TUI-007/AC2
    #[test]
    fn timings_in_the_final_chunk_give_measured_pp_and_tg() {
        let m = metrics_from_transcript(include_str!("../tests/data/sse/with_timings.txt"), Some(530), Some(1_000));
        assert_eq!(m.source, Source::Measured);
        assert!((m.pp_tps.unwrap() - 500.0).abs() < 0.01, "{m:?}"); // prompt_n 25 / prompt_ms 50
        assert!((m.tg_tps.unwrap() - 40.0).abs() < 0.01, "{m:?}");  // predicted_n 20 / predicted_ms 500
        assert_eq!(render(&m), ("TTFT: 530ms".to_string(), "PP: 500.0 t/s".to_string(), "TG: 40.0 t/s".to_string()));
    }

    /// Verifies: REQ-TUI-007/AC2, REQ-TUI-006/AC1
    #[test]
    fn no_timings_means_estimated_or_not_available_never_zero() {
        let m = metrics_from_transcript(include_str!("../tests/data/sse/without_timings.txt"), Some(530), Some(1_000));
        assert_eq!(m.source, Source::Estimated);
        assert_eq!(m.pp_tps, None);
        assert_eq!(m.content_chunks, 3);
        assert_eq!(render(&m), ("TTFT: 530ms".to_string(), "PP: n/a".to_string(), "TG: 3.0 t/s (est.)".to_string()));
        let none = metrics_from_transcript("", None, None);
        assert_eq!(render(&none), ("TTFT: n/a".to_string(), "PP: n/a".to_string(), "TG: n/a".to_string()));
    }

    /// Verifies: REQ-TUI-007/AC3
    #[test]
    fn history_lives_under_xdg_state_home() {
        let e = |k: &str| match k { "XDG_STATE_HOME" => Some("/st".to_string()), "HOME" => Some("/h".to_string()), _ => None };
        assert_eq!(history_path(&e), std::path::PathBuf::from("/st/saltnitor/history"));
        let e2 = |k: &str| (k == "HOME").then(|| "/h".to_string());
        assert_eq!(history_path(&e2), std::path::PathBuf::from("/h/.local/state/saltnitor/history"));
    }
}
```

`tests/data/sse/with_timings.txt` (what llama-server emits with `"timings_per_token": true`, last chunk carries `timings`; the R1 capture confirms chunks carry no `timings` without that flag):

```
data: {"choices":[{"delta":{"role":"assistant","content":"Hel"},"index":0}],"object":"chat.completion.chunk"}

data: {"choices":[{"delta":{"content":"lo"},"index":0}],"object":"chat.completion.chunk"}

data: {"choices":[{"delta":{"content":"!"},"finish_reason":"stop","index":0}],"object":"chat.completion.chunk","timings":{"prompt_n":25,"prompt_ms":50.0,"predicted_n":20,"predicted_ms":500.0}}

data: [DONE]

```

`without_timings.txt`: the same three content chunks without the `timings` object. Run: `cargo test --lib interrogate` → compile errors.

- [ ] **Step 2: Implement `src/interrogate.rs`**

```rust
//! Interrogator metrics from an SSE transcript (REQ-TUI-006, REQ-TUI-007). Measured numbers come
//! only from the runtime's `timings`; otherwise the value is an estimate labeled `est.` or `n/a`.
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Source { #[default] Unavailable, Estimated, Measured }

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Metrics {
    pub ttft_ms: Option<u128>,
    pub pp_tps: Option<f64>,
    pub tg_tps: Option<f64>,
    pub source: Source,
    pub content_chunks: usize,
}

/// `timings.{prompt_n, prompt_ms, predicted_n, predicted_ms}` → (PP t/s, TG t/s).
pub fn parse_timings(chunk: &serde_json::Value) -> Option<(f64, f64)> {
    let t = chunk.get("timings")?;
    let rate = |n: &str, ms: &str| -> Option<f64> {
        let n = t.get(n)?.as_f64()?;
        let ms = t.get(ms)?.as_f64()?;
        (ms > 0.0).then(|| n / ms * 1000.0)
    };
    Some((rate("prompt_n", "prompt_ms")?, rate("predicted_n", "predicted_ms")?))
}

pub fn metrics_from_transcript(transcript: &str, ttft_ms: Option<u128>, gen_elapsed_ms: Option<u128>) -> Metrics {
    let mut m = Metrics { ttft_ms, ..Default::default() };
    let mut measured = None;
    for data in transcript.lines().filter_map(|l| l.strip_prefix("data: ")).filter(|d| *d != "[DONE]") {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(data) else { continue };
        if v["choices"][0]["delta"].get("content").and_then(|c| c.as_str()).is_some() {
            m.content_chunks += 1;
        }
        if let Some(t) = parse_timings(&v) {
            measured = Some(t);
        }
    }
    match (measured, gen_elapsed_ms) {
        (Some((pp, tg)), _) => { m.pp_tps = Some(pp); m.tg_tps = Some(tg); m.source = Source::Measured; }
        (None, Some(ms)) if ms > 0 && m.content_chunks > 0 => { m.tg_tps = Some(m.content_chunks as f64 / ms as f64 * 1000.0); m.source = Source::Estimated; }
        _ => {}
    }
    m
}

/// Display strings for the deck: never `0` for an unmeasured value (INV-18).
pub fn render(m: &Metrics) -> (String, String, String) {
    let ttft = m.ttft_ms.map_or("TTFT: n/a".to_string(), |t| format!("TTFT: {t}ms"));
    let suffix = if m.source == Source::Estimated { " (est.)" } else { "" };
    let pp = m.pp_tps.map_or("PP: n/a".to_string(), |v| format!("PP: {v:.1} t/s{suffix}"));
    let tg = m.tg_tps.map_or("TG: n/a".to_string(), |v| format!("TG: {v:.1} t/s{suffix}"));
    (ttft, pp, tg)
}

/// `$XDG_STATE_HOME/saltnitor/history`, else `~/.local/state/saltnitor/history` (REQ-TUI-007/AC3).
pub fn history_path(env: &dyn Fn(&str) -> Option<String>) -> PathBuf {
    match env("XDG_STATE_HOME").filter(|s| !s.is_empty()) {
        Some(s) => PathBuf::from(s).join("saltnitor/history"),
        None => PathBuf::from(env("HOME").unwrap_or_default()).join(".local/state/saltnitor/history"),
    }
}
```

- [ ] **Step 3: Wire the TUI**

- `events.rs`: `ApiStreamEnd { metrics: crate::interrogate::Metrics, status: String }` (replaces `eval_tps`/`gen_tps`).
- `main.rs` interrogator task: URL `http://127.0.0.1:{control_port}/v1/chat/completions` (the daemon's own endpoint, REQ-TUI-007/AC1); header `Authorization: Bearer <client_bearer>` whenever `app.client_bearer` is `Some` (independent of the `k` toggle); the payload gets `"stream": true` **and** `"timings_per_token": true`; TTFT measured client-side at the first body chunk; the whole transcript is accumulated and `metrics_from_transcript(&transcript, Some(ttft), Some(elapsed_since_first_chunk))` is sent in `ApiStreamEnd`. `control_port` is stored on `App` at startup.
- `app.rs`: `last_metrics: Metrics` replaces `last_eval_tps`/`last_gen_tps`; history is read from and written to `interrogate::history_path(&|k| std::env::var(k).ok())` (create the directory with `create_dir_all` before writing; BD-21). The old `.saltnitor_history` is never read again.
- `ui.rs` deck line: `let (ttft, pp, tg) = interrogate::render(&app.last_metrics);` rendered as three spans; the status log line in `main.rs` (`API Strike: …`) prints the same three strings.

Run: `INSTA_UPDATE=always cargo test -p saltnitor ui::` (the `dashboard` snapshot now reads `TTFT: n/a | PP: n/a | TG: n/a` for an untouched app); `cargo test --workspace --locked` green; fmt/clippy clean.

- [ ] **Step 4: Run Close-out** for T1.15 (`Refs: REQ-TUI-006, REQ-TUI-007`; trailer `Protected-change: src/snapshots/saltnitor__ui__tests__dashboard.snap — interrogator deck renders n/a instead of 0 (INV-18, BD-18)`). PROGRESS note: `interrogator via :8765 with client key; timings_per_token; history under XDG_STATE_HOME (BD-18, BD-21)`.

---

### Task 17: T1.16 — README, CHANGELOG, SECURITY truthfulness

**Files:**
- Modify: `README.md`, `CHANGELOG.md`, `SECURITY.md`, `src/ui.rs` (tuner title), `src/snapshots/saltnitor__ui__tests__tuner_page_{1,2,3}.snap` (protected)
- Create: `docs/specs/vnext/evidence/claims.md` (protected)

- [ ] **Step 1: Claims ledger first (it decides what the README may say)**

Extract every feature claim: `grep -nE '^(- |\* |#)' README.md` plus prose sentences with "streams", "oracle", "hot-swap", "kill", "tuner", "auth", "token". Write `docs/specs/vnext/evidence/claims.md`:

```markdown
# README claims ledger (REQ-DOC-001)

| # | README claim (line) | Evidence | Status |
|---|---|---|---|
| 1 | "streams token-by-token through Saltnitor" | `tests/proxy_streaming.rs::first_chunk_is_forwarded_before_the_second_is_sent`; G1 D1 | kept |
| 2 | "VRAM oracle refuses loads that would OOM" | `src/control_api.rs` tests `ensure_oracle_reject_is_507_envelope`, `chat_oracle_reject_is_507_envelope` | kept |
| 3 | "hot-swap by model id (`model` = router section)" | `src/control_api.rs` tests `ensure_*`; REQ-MIG-007/AC1 | kept |
| 4 | "bearer auth on every /v1 route" | `tests/auth_policy.rs::every_route_gets_the_policy_outcome` | kept (new in v0.2) |
| 5 | "process sniper kills by name" | — | removed: replaced by "terminate/kill by exact PID" (`tests/process_control.rs`) |
| 6 | "generates router.env" | — | removed (BD-22): the tuner edits `router_ini` sections |
| 7 | "see integrations/INTEGRATION.md" | — | removed (BD-22): file does not exist; the section moved into README "Integrations" |
| 8 | "prompt and generation t/s" | `src/interrogate.rs` tests; requires `timings` from the runtime, else `est.`/`n/a` | reworded |
| 9 | "oracle warning in the hot-swap deck" | `src/snapshots/saltnitor__ui__tests__deck_hot_swap.snap` | kept |
```

(Fill the table from the actual README; every row either names a test/snapshot/gate row or says `removed`/`reworded`.)

- [ ] **Step 2: README, CHANGELOG, SECURITY, title**

README: rewrite the affected paragraphs so each remaining claim matches its ledger row; document `--config`, `router_ini`, `client_key_env`, `control_token_env`/`_file`, `allow_query_token`, `[timeouts]`, `max_body_bytes`, `[process] term_grace_ms`, the `x`/`X` keys, and the history location. CHANGELOG `## [0.2.0] — <date>` under `[Unreleased]` → moved at tag time, with sections **Security** (bearer enforced when a token is configured; `?token=` off by default; secrets from env/file; credentials never forwarded upstream), **Changed** (strict config, exit 2; `SUDO_USER` switching removed; `router_ini` required for the tuner; kill by PID with confirmation; interrogator via the daemon; history path; `Cargo.lock` tracked; CI), **Fixed** (BD-01, -02, -03, -04, -05, -06 partially, -07, -08, -10, -11, -12 no, -15, -16, -18, -21, -22, -24, -26, -29, -30 — list only those the tasks actually closed, checked against `baseline/DEFECTS.md`), **Removed** (`killall`, literal keys, `router.env` wording). SECURITY.md: threat model — tailnet peers (every `/v1` call authenticated; raw llama-server stays loopback), local users (key files 0600, no secrets in logs/crash dumps), what is out of scope in P1 (admin scope, remote listeners — P3/P9); reporting as in the stub. `ui.rs:355`: `" Deep router.env Tuner [Page {}/3] "` → `" router.ini Tuner [Page {}/3] "` (BD-30); `INSTA_UPDATE=always cargo test -p saltnitor ui::tests::tuner`.

Also mark the fixed defects in `docs/specs/vnext/baseline/DEFECTS.md` `Fixed by:` fields (protected; the ledger rows cite them).

- [ ] **Step 3: Verify and Close-out**

`cargo test --workspace --locked` green; every README claim has a ledger row (`python3 - <<'PY'` that counts README feature bullets vs ledger rows, print both — equal). Run Close-out for T1.16 (`Refs: REQ-DOC-001, REQ-DOC-003`; trailer `Protected-change: docs/specs/vnext/evidence/claims.md (new), docs/specs/vnext/baseline/DEFECTS.md (Fixed-by fields), src/snapshots/saltnitor__ui__tests__tuner_page_{1,2,3}.snap — tuner title (BD-30)`).

---

### Task 18: Gate G1 — evidence, real-world check, verification, verdict

**Files:**
- Create: `docs/specs/vnext/evidence/G1.md` (from `TEMPLATE.md`), `tests/fixtures/captures/g1/` (R0/R1 label `g1`)
- Modify: `scripts/real-check.sh` (bearer + token resolution), `docs/specs/vnext/evidence/test-baseline.txt` (counts raised to today's), `docs/specs/vnext/PROGRESS.md`

- [ ] **Step 1: Mechanical rows**

```bash
git pull -q --ff-only origin vnext
scripts/gate.sh G1 | tee /tmp/claude-1000/-home-laz-saltnitor/*/scratchpad/g1-rows.md
```

Expected: rows 1–7 PASS (row 1 = CI green on HEAD: `gh run list --branch vnext --limit 1`; row 2/3 = `cargo test --test acceptance` all pass, unmodified since merge — `git log --oneline <merge>..HEAD -- tests/acceptance` empty; row 4 = `scripts/check-invariants.sh` clean **and** `scripts/invariants-baseline.txt` empty; row 5 = the T1.1/T1.2 `git ls-files` checks; row 6 = the CR-8 list; row 7 = `spec_lint.py --tests --phase P1` exit 0). If `gate_g1` is missing or a row is wrong in `scripts/gate.sh` (T1.0's session wrote it): write `BLOCKED: G1 — gate.sh row <n> …` and ask the operator to approve a `Protected-change` fix; never edit it silently. Raise `evidence/test-baseline.txt` to today's per-suite counts (only upward) in the evidence commit.

- [ ] **Step 2: Real-world check (R0 + R1, operator "go" — `[HW]`)**

Update `scripts/real-check.sh`: the Python snippet that exports `RC_CONTROL_TOKEN` resolves it the T1.10 way (`control_token`, else `os.environ[control_token_env]`, else the file's contents) and the `curl` calls send `-H "Authorization: Bearer $RC_CONTROL_TOKEN"`. Then **stop and ask**: the operator must (a) have added `router_ini` to the live `config.toml` (design §11), (b) say "go". Run `scripts/real-check.sh --label g1 --a A_STD --b B --yes`; diff `tests/fixtures/captures/g1/r1.json` against `captures/baseline/r1.json` (same step names, each `PASS`; first-byte latency now well under the baseline's 1 208 ms). Record both in `G1.md`.

- [ ] **Step 3: Runtime falsification (D1–D3 and the four claims)**

Exact procedures, outputs pasted into `G1.md`:

```bash
# D1 — real streaming through :8765 against the slow-stream fake (claim: "streams token-by-token")
cargo build --locked && fp=$(python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1])')
cargo run -q -p fake-llama-server -- --scenario tests/fixtures/scenarios/bd02-slow-stream.toml --port "$fp" &
cat > "$SCRATCH/g1.toml" <<EOF
control_port = 18765
router_base = "http://127.0.0.1:$fp"
control_token_env = "SALTNITOR_G1_TOKEN"
[profiles.A]
model = "a.gguf"
est_vram_gb = 0.1
est_ram_gb = 0.1
EOF
SALTNITOR_G1_TOKEN=g1-secret HOME="$SCRATCH" script -qfec "stty cols 120 rows 40; target/debug/saltnitor --config $SCRATCH/g1.toml" /dev/null &
sleep 2
curl -sN -H "Authorization: Bearer g1-secret" -d '{"model":"A","stream":true}' http://127.0.0.1:18765/v1/chat/completions | ts '%.s'   # 5 lines ~200 ms apart → NOT_FALSIFIED
# claim: unauthenticated inference is refused
curl -si -d '{"model":"A"}' http://127.0.0.1:18765/v1/chat/completions | head -1            # HTTP/1.1 401 → NOT_FALSIFIED
curl -si 'http://127.0.0.1:18765/v1/ensure/stream?profile=A&token=g1-secret' | head -1       # 401 → NOT_FALSIFIED
# D2 / claim: a malformed config never starts the app
printf 'control_port = 18766\n[profiles.x]\nmodel = "m"\nest_vram_gb = "9gb"\n' > "$SCRATCH/bad.toml"
target/debug/saltnitor --config "$SCRATCH/bad.toml"; echo "exit=$?"; ss -ltn | grep -c 18766   # exit=2, 0 → NOT_FALSIFIED
# D3 / claim: the process sniper targets exactly one PID (TUI, operator at the keyboard)
sleep 300 & sleep 300 &   # open the CPU inspector, select one `sleep`, press x; then X + y on the other
```

Verifier: dispatch a fresh-context reviewer agent with `docs/specs/vnext/tools/verifier-prompt.md`, `requirements.md`, `git diff c155628..HEAD`, and `G1.md`; it fills the rubric and confirms every row and every `spec_lint.py --list-phase P1` AC. Fix every gap it finds as `T1.<n>-fix-k` commits, then rerun Step 1.

- [ ] **Step 4: Evidence commit and the operator's verdict**

`G1.md` holds: HEAD sha, CI run URL, the rows table, manual rows, R0/R1 diff, the protected-path list (`scripts/gate.sh G1` prints it), the CR-8 list (`git log --format='%h %s' c155628..HEAD -- tests src/snapshots docs/specs/vnext/evidence`), rubric findings, falsification results, SHOULD items skipped with justification, and `Verdict: _pending operator_`. Commit with `Protected-change: docs/specs/vnext/evidence/G1.md, evidence/test-baseline.txt, tests/fixtures/captures/g1/ — gate evidence`, push, PROGRESS line `G1 · <sha> · READY`.

**STOP.** Ask the operator for the verdict. On `PASSED`: they (or you, on their explicit instruction, quoting their words) write `Verdict: PASSED — signed: <name>, <date>`, commit, `git tag -a v0.2.0 -m "Saltnitor v0.2.0 — Hardening (G1 PASSED)"`, `git push origin vnext v0.2.0`, PROGRESS `G1 · PASSED`, and move CHANGELOG's `[Unreleased]` into `[0.2.0]`. Then **ask** before opening the `vnext → master` PR.

---

## Self-review record (plan author, 2026-10-09)

- **Spec coverage:** design §3 → Tasks 0–1; §4 → 2–7; §5 → 8–11; §6 → 12–13; §7 → 14; §8 → 15–18; §9 → Task 0; §10 crates → Tasks 4, 8, 10, 11, 12 commit messages; §11 operator actions → Tasks 7, 11, 18; §12 risks → Task 1 (brief), 12 (fake drop detection), 14 (pidfd fallback test), 18 (auth rollout checked by R1).
- **Every P1 task** T1.0–T1.16 has a task; G1 has Task 18; CR-7/CR-8 have Task 0.
- **Review Focus:** RF-1 → Task 10 proptest; RF-2 → Task 12 `non_string_model_or_non_object_body_is_400…`; RF-3 → Task 11 `unsafe_key_files_are_config_invalid`; RF-4 → Task 14 `changed_identity_is_refused…`; RF-5 → Task 12 `status_and_end_to_end_headers_pass…`.
- **Type consistency:** `ProxyLimits` fields (`connect, first_byte, idle, max_body_bytes`) identical in Tasks 12/13; `ProcessInfo` fields identical in Tasks 14 tests and impl; `ConfigV1` keys in Task 8 match the keys Tasks 10–14 read; `ApiError::{new, details, request_id}` as used in Tasks 10, 12, 13; `Event::Error { source, message }` in Tasks 15; `Metrics`/`render` in Task 16.
- **Known judgement calls recorded for the executor:** `toml::de::Deserializer::parse` vs `toml::Deserializer::new` (Task 8); serde's `expected` wording mapped in `type_name` (Task 8); hyper's disconnect detection backed by the 50 ms cadence (Task 13); `timings` only with `timings_per_token` (Task 16).
