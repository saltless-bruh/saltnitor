# Defect register — Saltnitor @ c89f278

Re-verified 2026-09-28 against `c89f278`; BD-33/34 added 2026-09-29 (CR-5, r2.3) (every cited line was read at that commit). `Fixed by:` is filled when the fixing task lands. REQ-MIG-006.
Status meanings: `confirmed` — the defect is visible at the cited lines; `disputed` — it is not; `BLOCKED` — evidence still needed.

### BD-01 — Malformed config silently becomes defaults
- **Evidence (c89f278):** `src/main.rs:75` — `toml::from_str(&content).unwrap_or_default()`
- **Repro:** write `port = "x"` to `<tmp>/.config/saltnitor/config.toml`, run `HOME=<tmp> cargo run` → starts with port 8080 and no diagnostic.
- **Status:** confirmed — the parse error is discarded by `unwrap_or_default()` on the cited line.
- **Fixed by:** T1.7 (`9cc518b`) — strict loader; `tests/config_strict.rs::type_error_exits_2_with_the_diagnostic_and_leaves_the_port_free`

### BD-02 — The proxy buffers the whole upstream body
- **Evidence (c89f278):** `src/control_api.rs:388` — `match resp.bytes().await {`
- **Repro:** `cargo test bd02_first_byte_timing -- --ignored --nocapture` (added in T0.6) prints first-byte times direct vs through Saltnitor.
- **Status:** confirmed — the full body is awaited before the response is built; timing evidence appended by T0.6.
- **Timing (2026-09-28, fake runtime, 5 SSE chunks 200 ms apart, two runs identical):** direct first byte 201 ms / total 1207 ms; through Saltnitor first byte 1208 ms / total 1208 ms — the client receives nothing until the stream has ended.
- **Fixed by:** T1.11 (`c63da04`) — `src/proxy_stream.rs`; `tests/proxy_streaming.rs::first_chunk_is_forwarded_before_the_second_is_sent`

### BD-03 — Auth is only checked on the ensure routes
- **Evidence (c89f278):** `src/control_api.rs:286`, `src/control_api.rs:318` — `auth_ok` is called only in `h_ensure` / `h_ensure_stream`
- **Repro:** set `control_token`, then `curl -s -o /dev/null -w '%{http_code}' 127.0.0.1:8765/v1/status` → 200 without a bearer.
- **Status:** confirmed — `h_status`, `h_models`, `h_chat` never call `auth_ok`.
- **Fixed by:** T1.9 (`bcc122d`) — auth middleware + POLICY table; `tests/auth_policy.rs::every_route_gets_the_policy_outcome`

### BD-04 — `?token=` is always accepted on `/v1/ensure/stream`
- **Evidence (c89f278):** `src/control_api.rs:241`, `src/control_api.rs:319` — `req.token.as_deref() == Some(t.as_str())`
- **Repro:** `cargo test bd04_query_token_is_always_accepted -- --ignored --nocapture` → HTTP 200 with a `?token=` and no header; there is no switch to refuse query tokens.
- **Status:** confirmed — the query token is compared unconditionally.
- **Fixed by:** T1.9 (`bcc122d`) — `allow_query_token` default off; `tests/auth_policy.rs::query_token_is_ignored_by_default`

### BD-05 — Process Sniper kills by name; PIDs are dropped
- **Evidence (c89f278):** `src/main.rs:468`, `src/main.rs:494`, `src/main.rs:284` — `killall -9 <name>`; process list deduplicated by name via `seen_names`
- **Repro:** run two processes named `sleep`, open the CPU inspector (`c`), select one, press `x` → both are killed.
- **Status:** confirmed — only the name reaches `killall`.
- **Fixed by:** T1.13 (`27cd25b`) — PID/pidfd control in `src/process.rs`; `tests/process_control.rs::terminating_one_of_two_same_named_processes_leaves_the_other_alive`

### BD-06 — Hardcoded router.ini path
- **Evidence (c89f278):** `src/main.rs:578`, `launch_router.sh:15` — `/home/laz/ai-models/llama.cpp/router.ini`
- **Repro:** run as any user other than `laz`, open the tuner, press Enter → `TUNER ERROR: cannot read /home/laz/ai-models/llama.cpp/router.ini`.
- **Status:** confirmed — the path is a `const` literal.
- **Fixed by:** T1.6 (`b35c6e5`) — `router_ini` config key; the tuner refuses to apply without it

### BD-07 — Hardcoded bearer token
- **Evidence (c89f278):** `src/main.rs:660`, `src/main.rs:748` — `"Bearer sk-saltnitor-2026"`; also `launch_router.sh:42`, `test_control_api.sh:9`
- **Repro:** `git grep -n sk-saltnitor-2026 c89f278` lists four files.
- **Status:** confirmed — the token is a literal in source and scripts.
- **Fixed by:** T1.6 (`b35c6e5`) — literal key removed; `client_key_env` / `control_token_env`; `scripts/check-invariants.sh` secret rule

### BD-08 — `.gitignore` ignores `Cargo.lock`
- **Evidence (c89f278):** `.gitignore:2` — `Cargo.lock`
- **Repro:** `git ls-files Cargo.lock` prints nothing; CI resolves dependencies fresh on every run.
- **Status:** confirmed — the lockfile is ignored.
- **Fixed by:** T1.1 (`44fc4fa`) — `Cargo.lock` tracked; CI builds with `--locked`

### BD-09 — Committed artifacts
- **Evidence (c89f278):** `git ls-tree -r --name-only c89f278 | grep -E 'saltnitor_history|crash_dump|legacy.zip|.vscode/settings.json'`
- **Repro:** the command above lists `.saltnitor_history`, `.vscode/settings.json`, `crash_dump_20260511_163653.txt`, `legacy.zip`.
- **Status:** confirmed — all four are tracked.
- **Fixed by:** T1.2 (`ee64dbf`) — local artefacts untracked and ignored, `legacy.zip` dropped (`git ls-files` lists none of the four)

### BD-10 — CI only builds and runs zero tests
- **Evidence (c89f278):** `.github/workflows/rust.yml:20`, `.github/workflows/rust.yml:22` — `cargo build --verbose`, `cargo test --verbose`
- **Repro:** `cargo test` at c89f278 → `running 0 tests`.
- **Status:** confirmed — no fmt/clippy/audit steps and no tests (T0.5–T0.7 add the first tests).
- **Fixed by:** T1.3 (`dea52f1`) — `.github/workflows/ci.yml` runs `cargo test --all --locked` plus the test-count gate

### BD-11 — Release uses an archived action and builds unlocked
- **Evidence (c89f278):** `.github/workflows/release.yml:16`, `.github/workflows/release.yml:18` — `cargo build --release`; `actions/upload-release-asset@v1`
- **Repro:** read the workflow; no `--locked`, and the upload action is archived upstream.
- **Status:** confirmed — both lines as described.
- **Fixed by:** T1.4 (`fea0b15`) — `.github/workflows/release.yml` builds `--locked` and uploads with `gh release upload`

### BD-12 — `main()` is ~850 lines
- **Evidence (c89f278):** `src/main.rs:143`, `src/main.rs:993` — `#[tokio::main]` … closing brace
- **Repro:** `git show c89f278:src/main.rs | sed -n "143p;993p"` → `#[tokio::main]` … `}`: `main` spans lines 144–993.
- **Status:** confirmed — one function holds startup, all tasks and all key handling.
- **Fixed by:**

### BD-13 — The oracle ignores external VRAM use
- **Evidence (c89f278):** `src/control_api.rs:136`, `src/control_api.rs:142` — `need + reserve <= total`
- **Repro:** with another process holding most of the VRAM, `POST /v1/ensure` for a profile that fits the *total* → `loaded` attempt instead of a reject.
- **Status:** confirmed — only `memory.total` is compared; `memory.used` is not consulted.
- **Fixed by:**

### BD-14 — The footprint is a filename heuristic only
- **Evidence (c89f278):** `src/control_api.rs:423` — `estimate_footprint` from `parse_params_b` / `parse_bpw`
- **Repro:** a profile whose file name has no size token gets the 8 B default (see T0.5 tests).
- **Status:** confirmed — no GGUF metadata is read.
- **Fixed by:**

### BD-15 — Control-API bind failure is invisible
- **Evidence (c89f278):** `src/control_api.rs:411`, `src/control_api.rs:413` — `eprintln!("[control_api] bind {} failed: {}", addr, e)`
- **Repro:** occupy port 8765, start Saltnitor → the TUI starts, the message is written behind the alternate screen, the API is simply absent.
- **Status:** confirmed — the error is printed to stderr only.
- **Fixed by:** T1.14 (`297086d`) — bind failure surfaced; `tests/error_surfacing.rs::occupied_control_port_produces_an_error_event`

### BD-16 — Panicking `unwrap`/`expect` in background tasks
- **Evidence (c89f278):** `src/main.rs:242`, `src/main.rs:243`, `src/main.rs:364`, `src/main.rs:366` — `event::poll(..).unwrap()`, `.expect("Failed to spawn journalctl")`
- **Repro:** run with `journalctl` unavailable after preflight (e.g. PATH changed) → the log task panics.
- **Status:** confirmed — the calls panic on error.
- **Fixed by:** T1.14 (`297086d`) — `unwrap_used` denied crate-wide, no exceptions

### BD-17 — Some tuner controls are never applied
- **Evidence (c89f278):** `src/main.rs:554`, `src/main.rs:571` — the `kv` list written to router.ini
- **Repro:** change draft max/min/model, ctx-shift, metrics or api-key in the tuner, press Enter → router.ini section unchanged for those keys.
- **Status:** confirmed — those fields never enter `kv`.
- **Fixed by:**

### BD-18 — Interrogator throughput numbers are not measured
- **Evidence (c89f278):** `src/main.rs:755`, `src/main.rs:784` — `eval_tps: 0.0`; `gen_tps = total_tokens / total_time_s` where tokens are SSE content chunks and time includes TTFT
- **Repro:** fire any prompt from the interrogator → eval t/s always shows 0.0.
- **Status:** confirmed — `eval_tps` is the literal `0.0`.
- **Fixed by:** T1.15 (`6455835`) — rates from runtime `timings`, else `est.`/`n/a`; `src/interrogate.rs::tests::no_timings_means_estimated_or_not_available_never_zero`

### BD-19 — TUI hot-swap bypasses the oracle
- **Evidence (c89f278):** `src/main.rs:622` — hot-swap Enter guesses NGL from the name and POSTs to the router directly
- **Repro:** select an oversized model in the Hot-Swap deck → a load is attempted with no oracle check.
- **Status:** confirmed — the control API is not involved in this path.
- **Fixed by:**

### BD-20 — Blocking `nvidia-smi` inside an async task
- **Evidence (c89f278):** `src/main.rs:308`, `src/main.rs:329` — `std::process::Command::new("nvidia-smi")` in the hardware poller
- **Repro:** read the task: two blocking process spawns per 1 s loop on a tokio worker.
- **Status:** confirmed — `std::process::Command`, not `tokio::process`.
- **Fixed by:**

### BD-21 — History file relative to CWD
- **Evidence (c89f278):** `src/app.rs:118`, `src/main.rs:986` — `".saltnitor_history"`
- **Repro:** start Saltnitor from two different directories → two separate histories; under `sudo`, a root-owned file.
- **Status:** confirmed — relative path on both read and write.
- **Fixed by:** T1.15 (`6455835`) — history under `$XDG_STATE_HOME/saltnitor/history`; `src/interrogate.rs::tests::history_lives_under_xdg_state_home`

### BD-22 — README drift
- **Evidence (c89f278):** `README.md:23` — tuner "generates a native Linux `router.env`"; `README.md:134` — links `integrations/INTEGRATION.md`
- **Repro:** `git ls-tree -r --name-only c89f278 | grep -c '^integrations/'` → 0; the tuner writes `router.ini` (`src/main.rs:578`).
- **Status:** confirmed — the README describes a file format and a directory that do not exist.
- **Fixed by:** T1.16 — README rewritten against `docs/specs/vnext/evidence/claims.md`; `router.env` and `integrations/INTEGRATION.md` claims removed

### BD-23 — No lease: a request for B evicts A mid-stream
- **Evidence (c89f278):** `src/control_api.rs:157` — `ensure_lock` is held only for the ensure itself
- **Repro:** stream a long chat on A through `/v1/chat/completions`, then POST chat for B → the router evicts A while A still streams.
- **Status:** confirmed — nothing holds A resident after its ensure returns.
- **Fixed by:**

### BD-24 — 2 MB body limit on chat
- **Evidence (c89f278):** `src/control_api.rs:355` — `body: Bytes` extractor with axum's default body limit
- **Repro:** POST a 3 MB chat body to `/v1/chat/completions` → 413 before the handler runs.
- **Status:** confirmed — no `DefaultBodyLimit` override on the router.
- **Fixed by:** T1.12 (`4278528`) — 32 MiB `max_body_bytes`; `tests/proxy_failures.rs::thirty_three_mib_is_413_and_thirty_one_mib_passes`

### BD-25 — The tuner edits the live runtime config in place
- **Evidence (c89f278):** `src/main.rs:577`, `src/main.rs:584` — writes router.ini, then `sudo -n systemctl restart`
- **Repro:** tuner Enter → the live router.ini changes and the service restarts.
- **Status:** confirmed — no generated preset; the runtime's own file is rewritten.
- **Fixed by:**

### BD-26 — Preflight requires systemd tools in every mode
- **Evidence (c89f278):** `src/main.rs:82`, `src/main.rs:83` — `["journalctl", "ss", "systemctl", "killall"]`
- **Repro:** run on a machine without `killall` → exit 1 before the TUI starts, even if no sniper action is ever used.
- **Status:** confirmed — unconditional check.
- **Fixed by:** T1.13 (`27cd25b`) — `killall` dropped from the preflight command list

### BD-27 — Port auditor builds a shell command string
- **Evidence (c89f278):** `src/main.rs:406`, `src/main.rs:407` — `format!("ss -lptn 'sport = :{}'", port_e)` run via `sh -c`
- **Repro:** read the task; the port is a `u16`, so today only formatting risk, but the pattern is shell-string construction.
- **Status:** confirmed — `sh -c` with a formatted string.
- **Fixed by:**

### BD-28 — `/v1/models` lists every configured profile
- **Evidence (c89f278):** `src/control_api.rs:336` — built from `api.profiles.keys()` only
- **Repro:** configure a profile whose model file is missing → it is still listed.
- **Status:** confirmed — no availability check; pinned (not endorsed) by T0.6 test `pins_bd28_models_lists_every_configured_profile`.
- **Fixed by:**

### BD-29 — Inconsistent OOM status and plain-text chat errors
- **Evidence (c89f278):** `src/control_api.rs:299`, `src/control_api.rs:369` — 507 on `/v1/ensure`, 503 on chat
- **Repro:** oracle reject via both routes → 507 JSON vs 503 plain text.
- **Status:** confirmed — pinned (not endorsed) by T0.6 tests `pins_bd29_ensure_oracle_reject_is_507_json`, `pins_bd29_chat_oracle_reject_is_503_plain_text`, `pins_bd29_chat_errors_are_plain_text`, `pins_bd29_chat_load_failure_is_502_plain_text`, `pins_bd29_chat_upstream_body_failure_is_502_plain_text`.
- **Fixed by:** T1.8 (`bf2370a`) — `ApiError` envelopes; `src/control_api.rs::tests::ensure_oracle_reject_is_507_envelope`, `::chat_oracle_reject_is_507_envelope`

### BD-30 — Tuner title says router.env
- **Evidence (c89f278):** `src/ui.rs:355` — `" Deep router.env Tuner [Page {}/3] "`
- **Repro:** open the tuner (`t`); compare with the file it writes (`router.ini`).
- **Status:** confirmed — title literal.
- **Fixed by:** T1.16 — tuner title is `router.ini Tuner`; `src/snapshots/saltnitor__ui__tests__tuner_page_{1,2,3}.snap`

### BD-31 — Control-API address hardcoded to loopback
- **Evidence (c89f278):** `src/main.rs:231` — `SocketAddr::from(([127, 0, 0, 1], control_port))`
- **Repro:** no config key changes the bind address.
- **Status:** confirmed — literal address.
- **Fixed by:**

### BD-32 — Resident fast path may never fire with status objects
- **Evidence (c89f278):** `src/control_api.rs:101` — checks `loaded` (bool), `state == "loaded"`, `status == "loaded"` (string) only; live sample `tests/fixtures/captures/baseline/r0/v1_models.json` (captured 2026-09-28)
- **Repro:** `fake-llama-server record --upstream http://127.0.0.1:8080 --out tests/fixtures/captures/baseline/r0`, then inspect `v1_models.json` for the shape of `status`.
- **Status:** confirmed — the live router reports `status` as an object (`{"value": "unloaded", "args": […], "preset": "…", "failed": true, "exit_code": 10}`), so `m["status"].as_str()` is always `None`; none of the three checks can match and the resident fast path never fires.
- **Probe (T0.6, `bd32_status_object_not_recognised`):** fake router reports A loaded in status-object form; Saltnitor `/v1/status` → `resident_models = []`.
- **Observation (live system, corrected 2026-09-29, not a Saltnitor defect):** every router model shows `"failed": true, "exit_code": 10` right after the router starts, with no load attempted. Cause: an upstream llama.cpp bug at build b9105 — the `server_model_meta` initializer in `tools/server/server-models.cpp:340` omits the `loaded_info` field, so `DEFAULT_STOP_TIMEOUT` (10) lands in `exit_code` and `is_failed()` is true until the model is first loaded (`load()` resets it, line 482). Cosmetic: loads are not affected.
- **Fixed by:**

### BD-33 — `parse_params_b` misreads decimal sizes
- **Evidence (c89f278):** `src/control_api.rs:435` — `name.to_uppercase().replace(['-', '_', '.'], " ")`
- **Repro:** `parse_params_b("Qwen2.5-0.5B-Instruct.gguf")` → `Some(5.0)`: after `.` becomes a space the tokens are `0` and `5B`, so a 0.5 B model is estimated as 5 B.
- **Status:** confirmed — found while writing the T0.5 characterization tests; added by CR-5 (r2.3). Deliberately not pinned by any test.
- **Fixed by:**

### BD-34 — `parse_bpw` has no `Q4_0`/`Q4_1` case
- **Evidence (c89f278):** `src/control_api.rs:443`, `src/control_api.rs:449` — no arm matches `Q4_0`; the chain ends in `else { 5.0 }`
- **Repro:** `parse_bpw("m-Q4_0.gguf")` → `5.0` (actual ≈ 4.5 bpw), so Q4_0/Q4_1 models are over-estimated.
- **Status:** confirmed — found while writing the T0.5 characterization tests; added by CR-5 (r2.3). Deliberately not pinned by any test.
- **Fixed by:**
