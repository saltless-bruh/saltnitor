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
