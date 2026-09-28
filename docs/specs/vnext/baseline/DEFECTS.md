# Defect register — Saltnitor @ c89f278

Re-verified 2026-09-28 against `c89f278` (every cited line was read at that commit). `Fixed by:` is filled when the fixing task lands. REQ-MIG-006.
Status meanings: `confirmed` — the defect is visible at the cited lines; `disputed` — it is not; `BLOCKED` — evidence still needed.

### BD-01 — Malformed config silently becomes defaults
- **Evidence (c89f278):** `src/main.rs:75` — `toml::from_str(&content).unwrap_or_default()`
- **Repro:** write `port = "x"` to `<tmp>/.config/saltnitor/config.toml`, run `HOME=<tmp> cargo run` → starts with port 8080 and no diagnostic.
- **Status:** confirmed — the parse error is discarded by `unwrap_or_default()` on the cited line.
- **Fixed by:**

### BD-02 — The proxy buffers the whole upstream body
- **Evidence (c89f278):** `src/control_api.rs:388` — `match resp.bytes().await {`
- **Repro:** `cargo test bd02_first_byte_timing -- --ignored --nocapture` (added in T0.6) prints first-byte times direct vs through Saltnitor.
- **Status:** confirmed — the full body is awaited before the response is built; timing evidence appended by T0.6.
- **Fixed by:**

### BD-03 — Auth is only checked on the ensure routes
- **Evidence (c89f278):** `src/control_api.rs:286`, `src/control_api.rs:318` — `auth_ok` is called only in `h_ensure` / `h_ensure_stream`
- **Repro:** set `control_token`, then `curl -s -o /dev/null -w '%{http_code}' 127.0.0.1:8765/v1/status` → 200 without a bearer.
- **Status:** confirmed — `h_status`, `h_models`, `h_chat` never call `auth_ok`.
- **Fixed by:**

### BD-04 — `?token=` is always accepted on `/v1/ensure/stream`
- **Evidence (c89f278):** `src/control_api.rs:241`, `src/control_api.rs:319` — `req.token.as_deref() == Some(t.as_str())`
- **Repro:** `curl -N '127.0.0.1:8765/v1/ensure/stream?profile=A&token=<control_token>'` → 200 with no header; there is no switch to refuse query tokens.
- **Status:** confirmed — the query token is compared unconditionally.
- **Fixed by:**

### BD-05 — Process Sniper kills by name; PIDs are dropped
- **Evidence (c89f278):** `src/main.rs:468`, `src/main.rs:494`, `src/main.rs:284` — `killall -9 <name>`; process list deduplicated by name via `seen_names`
- **Repro:** run two processes named `sleep`, open the CPU inspector (`c`), select one, press `x` → both are killed.
- **Status:** confirmed — only the name reaches `killall`.
- **Fixed by:**

### BD-06 — Hardcoded router.ini path
- **Evidence (c89f278):** `src/main.rs:578`, `launch_router.sh:15` — `/home/laz/ai-models/llama.cpp/router.ini`
- **Repro:** run as any user other than `laz`, open the tuner, press Enter → `TUNER ERROR: cannot read /home/laz/ai-models/llama.cpp/router.ini`.
- **Status:** confirmed — the path is a `const` literal.
- **Fixed by:**

### BD-07 — Hardcoded bearer token
- **Evidence (c89f278):** `src/main.rs:660`, `src/main.rs:748` — `"Bearer sk-saltnitor-2026"`; also `launch_router.sh:42`, `test_control_api.sh:9`
- **Repro:** `git grep -n sk-saltnitor-2026 c89f278` lists four files.
- **Status:** confirmed — the token is a literal in source and scripts.
- **Fixed by:**

### BD-08 — `.gitignore` ignores `Cargo.lock`
- **Evidence (c89f278):** `.gitignore:2` — `Cargo.lock`
- **Repro:** `git ls-files Cargo.lock` prints nothing; CI resolves dependencies fresh on every run.
- **Status:** confirmed — the lockfile is ignored.
- **Fixed by:**

### BD-09 — Committed artifacts
- **Evidence (c89f278):** `git ls-tree -r --name-only c89f278 | grep -E 'saltnitor_history|crash_dump|legacy.zip|.vscode/settings.json'`
- **Repro:** the command above lists `.saltnitor_history`, `.vscode/settings.json`, `crash_dump_20260511_163653.txt`, `legacy.zip`.
- **Status:** confirmed — all four are tracked.
- **Fixed by:**

### BD-10 — CI only builds and runs zero tests
- **Evidence (c89f278):** `.github/workflows/rust.yml:20`, `.github/workflows/rust.yml:22` — `cargo build --verbose`, `cargo test --verbose`
- **Repro:** `cargo test` at c89f278 → `running 0 tests`.
- **Status:** confirmed — no fmt/clippy/audit steps and no tests (T0.5–T0.7 add the first tests).
- **Fixed by:**

### BD-11 — Release uses an archived action and builds unlocked
- **Evidence (c89f278):** `.github/workflows/release.yml:16`, `.github/workflows/release.yml:18` — `cargo build --release`; `actions/upload-release-asset@v1`
- **Repro:** read the workflow; no `--locked`, and the upload action is archived upstream.
- **Status:** confirmed — both lines as described.
- **Fixed by:**

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
- **Fixed by:**

### BD-16 — Panicking `unwrap`/`expect` in background tasks
- **Evidence (c89f278):** `src/main.rs:242`, `src/main.rs:243`, `src/main.rs:364`, `src/main.rs:366` — `event::poll(..).unwrap()`, `.expect("Failed to spawn journalctl")`
- **Repro:** run with `journalctl` unavailable after preflight (e.g. PATH changed) → the log task panics.
- **Status:** confirmed — the calls panic on error.
- **Fixed by:**

### BD-17 — Some tuner controls are never applied
- **Evidence (c89f278):** `src/main.rs:554`, `src/main.rs:571` — the `kv` list written to router.ini
- **Repro:** change draft max/min/model, ctx-shift, metrics or api-key in the tuner, press Enter → router.ini section unchanged for those keys.
- **Status:** confirmed — those fields never enter `kv`.
- **Fixed by:**

### BD-18 — Interrogator throughput numbers are not measured
- **Evidence (c89f278):** `src/main.rs:755`, `src/main.rs:784` — `eval_tps: 0.0`; `gen_tps = total_tokens / total_time_s` where tokens are SSE content chunks and time includes TTFT
- **Repro:** fire any prompt from the interrogator → eval t/s always shows 0.0.
- **Status:** confirmed — `eval_tps` is the literal `0.0`.
- **Fixed by:**

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
- **Fixed by:**

### BD-22 — README drift
- **Evidence (c89f278):** `README.md:23` — tuner "generates a native Linux `router.env`"; `README.md:134` — links `integrations/INTEGRATION.md`
- **Repro:** `git ls-tree -r --name-only c89f278 | grep -c '^integrations/'` → 0; the tuner writes `router.ini` (`src/main.rs:578`).
- **Status:** confirmed — the README describes a file format and a directory that do not exist.
- **Fixed by:**

### BD-23 — No lease: a request for B evicts A mid-stream
- **Evidence (c89f278):** `src/control_api.rs:157` — `ensure_lock` is held only for the ensure itself
- **Repro:** stream a long chat on A through `/v1/chat/completions`, then POST chat for B → the router evicts A while A still streams.
- **Status:** confirmed — nothing holds A resident after its ensure returns.
- **Fixed by:**

### BD-24 — 2 MB body limit on chat
- **Evidence (c89f278):** `src/control_api.rs:355` — `body: Bytes` extractor with axum's default body limit
- **Repro:** POST a 3 MB chat body to `/v1/chat/completions` → 413 before the handler runs.
- **Status:** confirmed — no `DefaultBodyLimit` override on the router.
- **Fixed by:**

### BD-25 — The tuner edits the live runtime config in place
- **Evidence (c89f278):** `src/main.rs:577`, `src/main.rs:584` — writes router.ini, then `sudo -n systemctl restart`
- **Repro:** tuner Enter → the live router.ini changes and the service restarts.
- **Status:** confirmed — no generated preset; the runtime's own file is rewritten.
- **Fixed by:**

### BD-26 — Preflight requires systemd tools in every mode
- **Evidence (c89f278):** `src/main.rs:82`, `src/main.rs:83` — `["journalctl", "ss", "systemctl", "killall"]`
- **Repro:** run on a machine without `killall` → exit 1 before the TUI starts, even if no sniper action is ever used.
- **Status:** confirmed — unconditional check.
- **Fixed by:**

### BD-27 — Port auditor builds a shell command string
- **Evidence (c89f278):** `src/main.rs:406`, `src/main.rs:407` — `format!("ss -lptn 'sport = :{}'", port_e)` run via `sh -c`
- **Repro:** read the task; the port is a `u16`, so today only formatting risk, but the pattern is shell-string construction.
- **Status:** confirmed — `sh -c` with a formatted string.
- **Fixed by:**

### BD-28 — `/v1/models` lists every configured profile
- **Evidence (c89f278):** `src/control_api.rs:336` — built from `api.profiles.keys()` only
- **Repro:** configure a profile whose model file is missing → it is still listed.
- **Status:** confirmed — no availability check.
- **Fixed by:**

### BD-29 — Inconsistent OOM status and plain-text chat errors
- **Evidence (c89f278):** `src/control_api.rs:299`, `src/control_api.rs:369` — 507 on `/v1/ensure`, 503 on chat
- **Repro:** oracle reject via both routes → 507 JSON vs 503 plain text.
- **Status:** confirmed — see T0.6 tests `ensure_oracle_rejects_without_loading` and `chat_oracle_reject_is_503`.
- **Fixed by:**

### BD-30 — Tuner title says router.env
- **Evidence (c89f278):** `src/ui.rs:355` — `" Deep router.env Tuner [Page {}/3] "`
- **Repro:** open the tuner (`t`); compare with the file it writes (`router.ini`).
- **Status:** confirmed — title literal.
- **Fixed by:**

### BD-31 — Control-API address hardcoded to loopback
- **Evidence (c89f278):** `src/main.rs:231` — `SocketAddr::from(([127, 0, 0, 1], control_port))`
- **Repro:** no config key changes the bind address.
- **Status:** confirmed — literal address.
- **Fixed by:**

### BD-32 — Resident fast path may never fire with status objects
- **Evidence (c89f278):** `src/control_api.rs:101` — checks `loaded` (bool), `state == "loaded"`, `status == "loaded"` (string) only; live sample `tests/fixtures/captures/baseline/r0/v1_models.json` (captured 2026-09-28)
- **Repro:** `fake-llama-server record --upstream http://127.0.0.1:8080 --out tests/fixtures/captures/baseline/r0`, then inspect `v1_models.json` for the shape of `status`.
- **Status:** confirmed — the live router reports `status` as an object (`{"value": "unloaded", "args": […], "preset": "…", "failed": true, "exit_code": 10}`), so `m["status"].as_str()` is always `None`; none of the three checks can match and the resident fast path never fires.
- **Observation (live system, 2026-09-28, not a Saltnitor defect):** all 5 router models were `unloaded` with `"failed": true, "exit_code": 10` at capture time — the router's last load attempt failed for every model. Reported to the operator.
- **Fixed by:**

## Observations (not in the BD register)

Found while writing the T0.5 characterization tests. Not pinned by any test; proposed as BD-33/BD-34 in CR-5 (operator decides).

- **`parse_params_b` misreads decimal sizes** — `src/control_api.rs:435` replaces `.` with a space before splitting, so `Qwen2.5-0.5B-Instruct.gguf` yields `Some(5.0)` (the `5B` of `0.5B`), a 10× over-estimate.
- **`parse_bpw` has no `Q4_0`/`Q4_1` case** — `src/control_api.rs:443`: `m-Q4_0.gguf` matches none of the arms and falls through to the 5.0 default (~4.5 bpw actual).
