# Phase 1 — Hardening (v0.2): Implementation Design

- **Date:** 2026-10-08 · **Spec:** `saltnitor-vnext` r2.4 · **Phase:** P1 (tasks T1.0–T1.16, gate G1, tag `v0.2.0`)
- **Entry state:** G0 PASSED (signed 2026-09-29) · `master` = `vnext` = `a5cdbcf` · 71 P1 acceptance criteria have no test yet
- **Status:** approved section by section in brainstorming (operator, 2026-09-30 → 2026-10-08); awaiting written-spec review

This document says *how* Phase 1 will be carried out. *What* Phase 1 must achieve is fixed by
`docs/specs/vnext/tasks.md` (Phase 1 block) and the requirements it cites; where this document and the
spec disagree, the spec wins, except for the two change requests in §9, which the operator approved in
principle here and which are filed formally before T1.0 starts.

---

## 1. Intent

Phase 1 fixes the correctness and safety defects Phase 0 pinned (BP §4): the buffering proxy, auth
scattered through handlers, config that silently falls back to defaults, kill-by-name, panics in
background tasks, hardcoded secrets and paths, and a CI that checks nothing. No major new features.
Success means:

1. Every P1 acceptance criterion is met by its verification method, and `spec_lint.py --tests --phase P1`
   passes (REQ-TST-016/AC2).
2. The G1 acceptance tests — written by a session that implements nothing in P1 — pass unmodified.
3. CI is green on `vnext` and provably fails on a `todo!()` and on a planted invariant violation.
4. R0/R1 rerun on the operator's machine shows the hardened build still serves the real router.
5. Gate G1 passes with the verdict written by the operator; `v0.2.0` is tagged.

## 2. Operator decisions (2026-09-30 → 2026-10-08)

| # | Question | Decision |
|---|---|---|
| P1-D1 | Who writes T1.0 | A **fresh Claude Code session**, started from a behaviour-only brief (§3). It never implements P1. |
| P1-D2 | Auth model | **Bearer header only** on every `/v1/*` route, per RFC 9700 §4.3.2 and OAuth 2.1 §5.1. `?token=` is off; `allow_query_token` stays an opt-in compatibility switch the live config leaves off. Tokens come from env or a `0600` file; the literal `control_token` keeps working with a deprecation warning. |
| P1-D3 | How tests reach the code | **Approach A:** a thin `src/lib.rs` arrives with T1.8 (CR-7); acceptance tests run the real TUI binary inside a pseudo-terminal. No `--no-tui` flag before P3. |
| P1-D4 | Client key | **One secret.** `client_key_env` names the env var the TUI reads its bearer from; the operator points it at the same secret as `control_token_env`. Saltnitor accepts only the control token in P1. |
| P1-D5 | `unwrap_used` deny (T1.3) vs. unwrap removal (T1.14) | File-level `#![expect(clippy::unwrap_used, reason = "removed by T1.14")]` in `main.rs` and `control_api.rs`; `expect` warns once unused, so it cannot linger. |
| P1-D6 | Invariant-scan false positives | `docs/specs/**` excluded; test inputs that must look like violations carry an inline `invariants: allow <rule> — <reason>` marker; the scan prints every marker. |
| P1-D7 | Scratch-branch proofs | `scratch/t1.3-todo` and `scratch/t1.5-violation` are pushed, run through CI via `workflow_dispatch`, their run URLs recorded, then deleted. Nothing else leaves `vnext`. |
| P1-D8 | Process ownership under `sudo` | A target counts as the operator's if its UID is in {effective UID, real UID, `SUDO_UID` when set}. |
| P1-D9 | Processes screen layout | Rows are `PID  user  name  RAM  VRAM`; the selected row's command and the `X` confirmation share the pane's bottom line (the inspector pane is too narrow for six columns). |
| P1-D10 | Logging before P3 | No `tracing` until T3.11. In P1 "the log" is the TUI log pane plus stderr; redaction applies where lines enter the pane and when the crash dump is written. |

Phase 0 decisions D1–D7 remain in force (machine-independent tests, read-only live system, push `vnext`
after each task, never push `master`, never force-push, no PR without asking, record/replay at every gate).

## 3. Workflow and the T1.0 hand-off

```
 this session                     fresh session (T1.0)                  operator
 ────────────                     ────────────────────                  ────────
 file CR-7, CR-8 ───────────────────────────────────────────────────▶ approve
 write T1.0 brief ─────▶ reads the brief + spec only
                         writes tests/acceptance/g1_*.rs,
                         the [[test]] "acceptance" target, G1 rows in gate.sh
                         records the red output
                         pushes branch p1/acceptance ─▶ PR into vnext ─▶ review + merge
 T1.1 … T1.16: one task → one commit → push → CI green   (CLAUDE.md §2 loop, unchanged)
 G1: gate.sh G1 → R0/R1 on the real router → fresh verifier → falsify 4 claims ─▶ verdict, tag v0.2.0
```

**Order.** CR-7 and CR-8 are filed and approved first, because both change what the T1.0 session writes.
Then the brief is written and committed. Then the operator opens the fresh session. Implementation
(T1.1 onward) starts only after the acceptance PR is merged into `vnext`.

**The T1.0 brief** lives at `docs/superpowers/briefs/g1-acceptance-brief.md` and contains only:

- *What to prove:* T1.0's list, G1 rows 2–3, the four claims to falsify, and the REQ IDs to read
  (REQ-PRX-002/003/006, REQ-SEC-001/005, REQ-CFG-003, REQ-PROC-003, REQ-TST-014/016).
- *Harness facts:* the binary is at `CARGO_BIN_EXE_saltnitor` and needs a pseudo-terminal; the fake
  runtime's scenario and recorder API; how to pick free ports and point the binary at a temporary config;
  `/v1/ensure/stream` is GET-only until P6; clap already exits 2 on unknown flags, so exit 2 alone does
  not prove REQ-CFG-003.
- *Rules:* black-box tests with `/// Verifies:` tags; the process-control API contract they test against
  is written into `tests/acceptance/README.md`; the acceptance target is `test = false` until G1; the work
  goes out as a PR into `vnext` with a `Protected-change:` trailer.

The brief says nothing about how P1 will be implemented.

**Dependency graph** (from `tasks.md`):

```
T1.0 acceptance tests (fresh session, red) ───────────────────────────────────────┐
T1.1 lockfile ─┬─ T1.2 hygiene ──────────────────────────────────────┐             │
               ├─ T1.4 release pipeline                              │             │
               └─ T1.3 CI green ─┬─ T1.5 invariant scan ─ T1.6 creds/paths ─┤      │
                                 ├─ T1.7 strict config ───────────┐  │             │
                                 ├─ T1.8 JSON errors ─┬─ T1.9 auth ─┴─ T1.10 secrets│
                                 │                    ├─ T1.11 streaming ─┬─ T1.12 cancel/limits
                                 │                    │                   └─ T1.15 interrogator
                                 │                    └─ T1.14 no panics
                                 └─ T1.13 kill by PID
                 T1.2, T1.6, T1.9, T1.11, T1.13, T1.15 ──▶ T1.16 docs ──▶ G1 ──▶ v0.2.0
```

## 4. Foundations (T1.1–T1.6)

```
push / PR → master | vnext   (+ workflow_dispatch for the scratch-branch proofs)
 ├─ fmt ──────── cargo fmt --all --check
 ├─ clippy ───── --all-targets --all-features --locked -D warnings
 │               todo / unimplemented / dbg_macro / unwrap_used = deny in [lints.clippy]
 ├─ test ─────── cargo test --all --locked          # Verifies: REQ-CI-003/AC1
 │   └─ counts ─ scripts/check-test-counts.sh ≥ evidence/test-baseline.txt
 ├─ build ────── cargo build --release --locked
 ├─ msrv ─────── Rust 1.95 (rust-version)
 ├─ deny ─────── cargo deny check (licenses, advisories, bans, sources)
 ├─ spec ─────── spec_lint.py + its self-test       # Verifies: REQ-DOC-006/AC2
 ├─ invariants ─ scripts/check-invariants.sh        (added by T1.5)
 └─ acceptance ─ cargo test --test acceptance       (informational until G1)
```

| Task | Design | Findings that shaped it |
|---|---|---|
| **T1.1** | Track `Cargo.lock`; `rust-version = "1.95"`; `rust-toolchain.toml` pins 1.95.0 with `rustfmt` and `clippy`. | `sysinfo 0.39` needs 1.95, so MSRV = current stable; the MSRV job matters once the pin moves. |
| **T1.2** | Untrack `.vscode/`, `.saltnitor_history`, the crash dump (local copies stay). Rename `Contributing_Guidelines.md` → `CONTRIBUTING.md`; add stub `SECURITY.md` and `CHANGELOG.md`; `.gitignore` gains those paths plus `.obsidian/` and `.claude/settings.local.json`. `legacy.zip` (`main/ui/app/events.rs`, May 2026): diff each file against every version in `git log --all -p -- src/`; identical → remove and note in CHANGELOG; unique → ask the operator, tag `legacy-archive`, then remove. | |
| **T1.3** | Three commits: ① `cargo fmt` only; ② the 11 clippy warning kinds fixed; ③ `ci.yml` replaces `rust.yml`, plus `[lints.clippy]`, `clippy.toml`, `deny.toml`, `scripts/check-test-counts.sh`. CI runs with read-only token permissions, cancels superseded runs, pins action versions (verified against current docs at plan time). P0 tests and snapshots stay byte-identical through ① and ②. | P1-D5 for `unwrap_used`. The workflow steps carry `# Verifies:` comments. |
| **T1.4** | `release.yml`: on published release and `workflow_dispatch`; `--locked` on the pinned toolchain; `SHA256SUMS`; upload via `gh release upload`; build-provenance attestation added. The dry run uploads a workflow artifact only. | Replaces archived `actions/upload-release-asset@v1` (REQ-CI-004/AC2). |
| **T1.5** | `scripts/check-invariants.sh` with five rules: kill-by-name in `src/`; user-home absolute paths; secret patterns; tracked runtime artifacts; `git`/`cmake`/`make` invoked from `src/`. Each failure prints rule, `file:line`, and the fix. `scripts/invariants-baseline.txt` seeds today's violations; the scan also fails on a listed entry that no longer occurs, so the file only shrinks. A shell self-test proves each rule fires. | P1-D6 for legitimate matches. |
| **T1.6** | Config v1 keys `router_ini` (path) and `client_key_env`. Tuner refuses to apply with a visible log line when `router_ini` is unset. Hot-swap and interrogator read the bearer from the named env var; no literal remains. `launch_router.sh` → `examples/external-mode/`, `test_control_api.sh` → `scripts/smoke_control_api.sh`, both with env tokens and placeholder paths. Also scrubs `/home/laz` from `CLAUDE.md`, the P0 design/plan docs, and the `record.rs` test string, so the task's Done-when grep is empty. Ratchet shrinks. | **Live impact:** the tuner stops applying until the operator adds `router_ini = "…"` to `config.toml`; exact line supplied. The router runs without `--api-key`, so hot-swap keeps working. |

## 5. Config, errors, auth, secrets (T1.7–T1.10)

```
startup (before raw mode, before any listener or child process)
 --config │ $XDG_CONFIG_HOME/saltnitor │ ~/.config/saltnitor ──▶ config_v1::load
     │ parse with deny_unknown_fields, tracking key path + line/col (serde_path_to_error + TOML span)
     │   └─ error ─▶ stderr: config.toml:42:13: profiles.x.ctx_size: expected integer, found string "32768k"
     │                ─▶ exit 2, no port bound, no child process
     ▼ validate: ports, URLs, ~ / $VAR / ${VAR} paths, secret sources (env │ 0600 file │ literal + warning)

request ─▶ [request-id] ─▶ [ONE auth layer: policy table] ─▶ handler ─▶ ApiError ─▶ {"error":{code,message,type,details,request_id}}
                           no/invalid bearer ─▶ 401 AUTH_REQUIRED
                           ?token= honoured only if allow_query_token, only on GET /v1/ensure/stream
                           route not in table ─▶ 404 ENDPOINT_NOT_SUPPORTED

every log line ─▶ Redactor (known secret values + `Bearer …` / `token=…` patterns) ─▶ TUI log pane, crash dump, stderr
```

- **T1.7 `src/config_v1.rs`.** Strict at every level; accepts exactly today's keys plus P1's additions
  (`router_ini`, `client_key_env`, `control_token_env`, `control_token_file`, `allow_query_token`,
  `[timeouts]`, `max_body_bytes`, `[process] term_grace_ms`, `schema_version`). Diagnostics use
  REQ-CFG-003/AC2's wording verbatim (`expected integer`, `found string "…"`); unknown keys suggest the
  closest known key (hand-written edit distance); a bad secret value prints as `found <redacted string>`.
  No file at the default path → defaults, logged; `--config` naming a missing file → exit 2. The
  `SUDO_USER` switch is removed (REQ-CFG-001/AC3). A **live-shape fixture** (the operator's config with
  its exact keys and tables, values replaced) proves the real config still loads.
- **T1.8 `src/error.rs`.** All 22 Appendix B codes; one `status()`, one `type`, one `IntoResponse`
  rendering the envelope; `MODEL_BUSY` adds `Retry-After`. Table test covers every code. Plain-text chat
  errors become envelopes; the CR-6 `pins_bd29_*` tests are rewritten to assert the fixed behaviour.
  `request_id` is `null` until T1.11 adds the ID layer. **`src/lib.rs` is created here** (CR-7), exposing
  `error`, `control_api`, and what they need; `main.rs` imports from it.
- **T1.9 `src/auth.rs`.** One route list builds both the axum router and the policy test, so a route
  cannot exist without a policy entry (REQ-SEC-001/AC3). Single middleware layer; `Bearer` only;
  `subtle` constant-time compare; failed-auth log at most once per second per peer, naming listener,
  peer, route — never the token; no CORS headers; `/healthz` → `200 {"status":"ok"}`. A proptest walks
  every route × {no credential, wrong token, valid token}. The 403 wrong-scope path is unreachable
  through real routes in P1 (admin scope is P3; `force` → 403 is P6); the middleware implements it
  generically and a unit test exercises it with an admin-only entry.
- **T1.10.** `control_token_env` / `control_token_file`; the file must be owned by the daemon user with
  no group/other bits, else exit 2; more than one source configured is also an error; literal
  `control_token` works with a deprecation warning. Redaction per P1-D10. Tests cover permission
  rejection, the warning, and absence of the token in captured log output and a crash dump.

## 6. Streaming proxy (T1.11–T1.12)

```
client ─POST /v1/chat/completions─▶ [request-id] ─▶ [auth] ─▶ proxy_stream::forward
 ① body ≤ max_body_bytes (32 MiB) ─────────────────────────── else 413 PAYLOAD_TOO_LARGE
 ② JSON object with a string "model"? (parse to check; forward the ORIGINAL bytes) ── else 400 REQUEST_INVALID
 ③ ensure(model): oracle → warm load, the existing path (MIG-007/AC1) ── 404 / 507 / 502 as JSON envelopes
 ④ upstream request: same body bytes
    − hop-by-hop headers (RFC 9110 §7.6.1 list + names in the client's Connection:)
    − Authorization / Cookie / Proxy-Authorization   + Bearer infer_bearer (if set)   + X-Request-Id
       connect > connect_ms, or no headers within first_byte_ms ─▶ 504 UPSTREAM_TIMEOUT
       reset or garbage before headers ───────────────────────────▶ 502 RUNTIME_UNHEALTHY
 ⑤ headers arrive ─▶ upstream status + end-to-end headers copied (4xx/5xx too), X-Request-Id added
 ⑥ upstream.bytes_stream() ─ each chunk as it arrives ─▶ Body::from_stream ─▶ client
       idle > idle_ms, or upstream dies mid-stream ─▶ end the response there, add nothing (no fake [DONE])
                                                      + log UPSTREAM_STREAM_ABORTED <request-id>
       client hangs up ─▶ body stream dropped ─▶ upstream connection closed (≤ cancel_propagation_ms)
```

- **Request IDs:** keep an incoming `X-Request-Id` matching `^[A-Za-z0-9._-]{1,128}$`, else generate a
  UUIDv7 (`uuid` crate). Outermost layer, so even a 401 carries it; echoed, forwarded, logged, and
  filled into the envelope.
- **Configurable limits:** `[timeouts] connect_ms / first_byte_ms / idle_ms` and `max_body_bytes`,
  §6 defaults (5 000 / 600 000 / 120 000 / 33 554 432). No total deadline for streams.
- **Unchanged until P6:** a request for B still evicts A mid-stream (BD-23).
- **Tests** (in-process, library + fake runtime): byte-exact proptest over random bodies (SSE comments,
  keep-alives, `data: [DONE]`, tool-call deltas, reasoning fields), random chunkings, random status;
  timing test per REQ-PRX-002/AC2 against the fake's send timestamp; T1.12's failure matrix (client drop
  → close seen ≤ 1 s and in-flight count restored; 4xx/5xx byte-exact; reset → 502; hang → 504 with short
  configured timeouts; mid-stream crash → truncated, no `[DONE]`, abort logged; 33 MiB → 413, 31 MiB
  passes; missing `model` → 400); `grep '\.(bytes|text|json)()' src/proxy_stream.rs` empty.
- **Fake runtime additions (additive, 39 existing tests unchanged):** raw byte chunks with per-chunk
  delay and send timestamps; prompt detection of a closed connection plus an in-flight counter.

## 7. PID-based process control (T1.13)

```
hw poller (1 s)
 ├─ sysinfo: every process ──────────────┐  one ProcessInfo per PID — no name dedupe
 └─ nvidia-smi --query-compute-apps=     ├─▶ Vec<ProcessInfo>{pid,name,memory_bytes,
      pid,process_name,used_memory ──────┘     gpu_memory_bytes: Option(+reason),command,start_time_ticks,uid}
                                                   │ Event::HardwareUpdate
 TUI inspectors (GPU view = rows with VRAM; CPU view = top RAM)   ▼
   [Up/Dn] select ─▶ process::Target::select(pid)  ← snapshot {pid, start_time, uid} + pidfd taken NOW
   [x] / [Delete] terminate ─▶ guard ─▶ SIGTERM ─▶ wait ≤ term_grace_ms ─▶ log "exited" | "still_running"
   [X] kill ───────▶ prompt "SIGKILL 4242 python3? [y/N]" ─▶ y ─▶ guard ─▶ SIGKILL ─▶ log

 guard (both paths, in this order):
   identity: /proc start_time + uid still equal the snapshot ── else PROCESS_CHANGED, nothing sent
   protected: PID 1 · saltnitor itself · the router's tree (systemctl MainPID + descendants)
              · a process not owned by the operator (P1-D8) ─── else PROCESS_PROTECTED + reason
```

- `src/process.rs` (library) owns listing, `Target::select`, `terminate`, `kill`; the TUI calls only
  those. No HTTP route in P1 (`/admin/*` is P3).
- Signals go through a pidfd (`rustix`, `process` feature: `pidfd_open` at selection, `pidfd_send_signal`
  later); `ENOSYS` falls back to `kill(pid)` guarded by the identity check.
- Router tree = `systemctl show -p MainPID <service_name>` plus descendants; if the unit cannot be
  queried, the existing `llama-server` name guard remains as fallback. Stopping the router stays Ctrl+K.
- Outcomes `exited` / `still_running`; refusals use `PROCESS_CHANGED` / `PROCESS_PROTECTED` from T1.8.
- Preflight drops `killall`. `nvidia-smi` stays optional; `[N/A]` per-process memory renders `n/a` with
  the reason, never `0`.
- **Tests** (`tests/process_control.rs`, real processes): two same-named `sleep`s, terminate one by PID,
  the other survives; a SIGTERM-ignoring child → `still_running` then `kill` → exited; wrong start_time →
  `PROCESS_CHANGED`; PID 1, own PID, foreign UID → `PROCESS_PROTECTED` with reason; `[PROP]` one row per
  PID for random tables with repeated names; the nvidia-smi parser on captured output incl. `[N/A]`.
- **Known gap until P2:** key handlers live inside `main.rs`'s loop, so no test presses `x` and watches
  the PID get signalled. Library tests prove targeting; rows and the confirm prompt are snapshot-tested;
  D3 at G1 demonstrates the keypress on the real TUI.
- The T1.0 author names the process API the acceptance test calls; T1.13 implements that contract. An
  impossible contract is a change request, never a quiet rename.

## 8. Error surfacing, interrogator, docs, gate (T1.14–T1.16, G1)

```
T1.14  bind :8765 fails ──▶ Event::Error{source,msg} ──▶ TUI log pane + status-line marker   (BD-15)
       journalctl spawn fails ──▶ same event, poller keeps running                      (BD-16)
       every unwrap/expect on I/O, parse, subprocess, channel paths ──▶ handled; the T1.3 `#![expect]` goes

T1.15  interrogator ──Bearer <client key>──▶ Saltnitor :8765 /v1/chat/completions (stream) ──▶ router
         TTFT client-side · final SSE chunk has `timings` ──▶ PP / TG t/s as measured
                          · no `timings`                  ──▶ `est.` (content chunks ÷ gen time) or `n/a`
         history ──▶ $XDG_STATE_HOME/saltnitor/history (fallback ~/.local/state), never CWD   (BD-21)

T1.16  README claims ──▶ evidence/claims.md ledger · CHANGELOG v0.2.0 (breaking changes) · SECURITY threat model
       tuner title "router.ini Tuner" (BD-30) ──▶ tuner snapshots change (CR-8)

G1     scripts/gate.sh G1 ──▶ rows 1–7 ──▶ R0/R1 label g1 vs captures/baseline (operator "go")
       ──▶ fresh verifier ──▶ 4 claims through the real binary ──▶ evidence/G1.md ──▶ verdict ──▶ tag v0.2.0 ──▶ PR (ask first)
```

- **T1.14.** `Event::Error` is the single path for background failures. A test occupies the control port
  first, starts the API through the library, and asserts the event. REQ-ERR-005/AC2 (exit on bind
  failure) is P3's; in P1 the TUI keeps running and reports.
- **T1.15.** The `k` toggle keeps gating the hot-swap's direct router call only; the interrogator always
  sends the client key to Saltnitor. Tests parse a recorded SSE transcript with and without `timings`;
  the deck snapshot shows `est.`/`n/a`.
- **T1.16.** `evidence/claims.md`: every README feature claim → test or gate row; unproven claims are
  rewritten or removed. CHANGELOG `v0.2.0` lists: auth enforced when a token is configured; `?token=`
  off; `SUDO_USER` switching gone; `router_ini` required for the tuner; `killall` gone; history moved;
  exit 2 on bad config; `Cargo.lock` tracked. `SECURITY.md`: threat model (tailnet peers, local users)
  and reporting.
- **G1 rows** are added to `gate.sh` by the T1.0 session (its Files list). Row 6 uses the CR-8 rule.
  `real-check.sh` learns to send the bearer and resolve the token like T1.10. Before R1 the operator
  edits the live `config.toml` once (`router_ini`; optionally `control_token_env`) — exact lines
  supplied, file never touched by the agent. Falsification: `curl -N` with timestamps against
  `bd02-slow-stream`; no bearer → 401 envelope; `saltnitor --config bad.toml` → exit 2 and the port
  free; two same-named `sleep`s, `x` on one in the TUI. Verifier: fresh-context agent with
  `tools/verifier-prompt.md`, the diff since G0, and the evidence.

## 9. Change requests to file before T1.0

- **CR-7 · library target and the "must compile" rule.** Affects T1.0 Done-when, T1.8 Files, T2.1
  (REQ-ARCH-002). T1.8 creates `src/lib.rs` exposing only P1's new modules plus `control_api` and its
  needs; `main.rs` imports from it; T2.1 changes from "create" to "extend". T1.0's Done-when becomes:
  the acceptance target is `test = false`; tests compile except those calling a P1 API the author names
  in `tests/acceptance/README.md`, which may fail to compile only for that reason.
- **CR-8 · G1 row 6 ("tests not weakened").** T1.13 (`x`/`X`, PID/user columns), T1.15 (PP/TG or
  `n/a`), T1.16 (tuner title) and the CR-6 `pins_bd28/29` rewrites intentionally change snapshots and
  assertions. New pass rule: every changed assertion or snapshot sits in a commit that states why
  (REQ-TST-007/AC2) and carries a `Protected-change:` trailer, and the operator approves the list in
  `evidence/G1.md`.

## 10. New crates (REQ-ARCH-008; all permissive, maintained, allowed in `deny.toml`)

| Crate | Used by | Why |
|---|---|---|
| `serde_path_to_error` | T1.7 | dotted key path in config diagnostics |
| `subtle` | T1.9 | constant-time token compare (REQ-SEC-004) |
| `uuid` (v7) | T1.11 | request IDs (REQ-PRX-009) |
| `rustix` (`process`) | T1.13 | `pidfd_open` / `pidfd_send_signal` (REQ-PROC-004/AC2) |
| `proptest`, `tempfile` (dev) | T1.9, T1.11, T1.13, T1.7/T1.10 | property tests; temp configs and key files |
| reqwest `stream`, tokio-stream `time` features | T1.11 | `bytes_stream()`, idle timeout |

Exact API names are checked against docs.rs when the plan is written.

## 11. Live-system impact (operator actions, in order)

1. **After T1.6:** add `router_ini = "<path to router.ini>"` to `~/.config/saltnitor/config.toml`, or
   the tuner refuses to apply. Optionally set `client_key_env`.
2. **After T1.10 (optional):** move the token to `control_token_env` or a `0600` `control_token_file`;
   the literal keeps working with a warning.
3. **Before G1's R1:** any tool calling `:8765` must send `Authorization: Bearer <control token>`;
   `?token=` no longer works unless `allow_query_token = true` is set deliberately.
4. **At G1:** approve the protected-path list, the CR-8 snapshot list, run the `[HW]`/`[HUMAN]` rows
   (R1 "go", D1–D3), write the verdict, then tag.

The agent reads but never modifies `~/ai-models/llama.cpp/`, the systemd unit, sudoers,
`~/.config/saltnitor/config.toml`, or `~/ai-runtimes/*`.

## 12. Constraints and risks

- **Protected paths** (`tests/acceptance/`, fixtures, snapshots, `gate.sh`, the spec) change only in
  commits with a `Protected-change:` trailer; the operator approves the list at G1 (CR-3).
- **Secrets and home paths** never enter commits, fixtures, logs, or captures; gate row 4b-style checks
  continue (`check-invariants.sh`).
- **Risk: T1.0 contract mismatch.** The fresh session may name a process API or config diagnostic shape
  the implementation cannot meet exactly. Mitigation: the brief points at REQ wording; mismatches become
  change requests, never test edits.
- **Risk: pty-driven acceptance tests are flaky in CI.** Mitigation: the acceptance job is informational
  until G1; the tests use generous, configurable waits and free-port allocation; a `script`-based fallback
  is documented in the brief.
- **Risk: `rustix` pidfd on CI kernels.** `ENOSYS` fallback path is itself tested by forcing the fallback.
- **Risk: live-router auth rollout.** Any client of the operator's that omits the bearer breaks at
  T1.9; the CHANGELOG and §11 list the change, and R1 exercises the header path before the verdict.
