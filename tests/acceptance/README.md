# Acceptance tests (protected)

Gate acceptance tests live here, one file per gate (`g1_*.rs`, …), written by a session that does
not implement that phase (REQ-TST-012, REQ-TST-014). Implementing agents treat this directory as
read-only; changes go through a change request (REQ-DOC-007). Phase 0 has no acceptance tests —
G0 is verified by `scripts/gate.sh G0` and the evidence file.

## Declaration and invocation
The suite is one cargo test target, `acceptance` (`tests/acceptance/main.rs`, `test = false`),
so plain `cargo test` never runs it while it is red. Run it with `cargo test --test acceptance`
(informational in CI until G1; required at G1). G1 rows 2–3 call it.

## Layout (G1, T1.0)
| File | Proves |
|---|---|
| `main.rs` | target root; declares the modules and the `acceptance_without_process_api` cfg |
| `harness.rs` | pty launch (`portable-pty`), throw-away `$HOME`, free ports, fake runtime, chunk timing |
| `g1_streaming.rs` | first SSE chunk before the last one is sent (REQ-PRX-002) |
| `g1_byte_exact.rs` | downstream bytes == upstream bytes, incl. a `[PROP]` case over random bodies (REQ-PRX-003) |
| `g1_cancel.rs` | client drop → upstream `disconnect` within `cancel_propagation_ms` (REQ-PRX-006) |
| `g1_auth.rs` | 401 envelope on every protected route, bearer accepted, `?token=` refused (REQ-SEC-001, -002/AC3, -005) |
| `g1_config.rs` | malformed config → exit 2, full diagnostic, port free (REQ-CFG-003, -004/AC1) |
| `g1_process.rs` | terminate by PID leaves a same-named process alive (REQ-PROC-003/AC1, AC3) |
| `g1_compositional.rs` | auth + request ID + streaming + cancellation in one flow (REQ-TST-014) |

Run everything: `cargo test --test acceptance`. Until the process-control API exists, build without
`g1_process` with
`RUSTFLAGS="--cfg acceptance_without_process_api --check-cfg cfg(acceptance_without_process_api)" cargo test --test acceptance`.

### Harness decisions (interpretations of the brief)
- **Config reaches the binary through `$HOME`**, not `--config`, for every server test: the harness writes
  `$HOME/.config/saltnitor/config.toml` in a private temp dir and unsets `XDG_CONFIG_HOME`/`SUDO_USER`.
  Today's clap rejects `--config` (exit 2), so passing it would make every test fail for the wrong reason.
  Only the `g1_config` tests pass `--config <file>`, where the brief's trap applies and the stderr text is asserted.
- **Protected routes** checked without a bearer: `GET /v1/models`, `GET /v1/status`, `POST /v1/chat/completions`,
  `POST /v1/ensure`, `GET /v1/ensure/stream`. `/v1/ensure/stream` is GET-only until P6, so POST is not sent.
  Each must answer 401 with `Content-Type: application/json` and `{"error":{"code":"AUTH_REQUIRED","type":"authentication_error",…}}`.
- **"line and column"** (REQ-CFG-003/AC2) is asserted as `<path>:<line>:<digits>`; the column may point at the key or the value.
- **"no child process shall exist"** (REQ-CFG-003/AC3) is not asserted beyond the port being free; the binary spawns nothing before the TUI.
- **Byte-exactness** uses two upstreams: the fake's chat route for realistic SSE deltas and `data: [DONE]`, and a raw
  HTTP/1.1 upstream inside the test (`RawUpstream`) for SSE comments, keep-alives, tool-call/reasoning deltas and random
  bytes with random chunk boundaries, because the fake's chat route cannot emit comment lines. The raw upstream answers
  `GET /v1/models` with `A` loaded so the proxy's ensure step treats the model as resident.
- **Cancellation budget** is `cancel_propagation_ms` (1 000 ms, §6) plus one upstream piece gap (100 ms; the fake only
  notices a dropped downstream at its next send). The "active-request count returns to its prior value" half of
  REQ-PRX-006/AC1 has no observable surface in P1 and is left to the implementer's unit tests.
- **Timing assertions** use wall-clock gaps of 2 000 ms (streaming) and 300 ms (compositional) so a buffering proxy fails
  by seconds, not by the `stream_forward_budget_ms` CI tolerance.

## Process-control API contract (REQ-PROC-003; used by `g1_process.rs`)
`g1_process.rs` is the one module that calls library code. It expects, from the `saltnitor` library crate (CR-7, T1.8):

```rust
pub mod process {
    /// A PID pinned to the identity the operator saw (REQ-PROC-004: start time + UID).
    pub struct Target { /* private */ }

    impl Target {
        /// Snapshot `pid` now. Err if the PID does not exist or cannot be read.
        pub fn snapshot(pid: u32) -> Result<Target, ProcessError>;
    }

    /// Result of a graceful terminate (REQ-PROC-003/AC1).
    #[derive(Debug, PartialEq, Eq)]
    pub enum Outcome { Exited, StillRunning }

    /// Send SIGTERM to exactly `target`'s PID (after re-checking its identity), wait up to
    /// `grace`, and report. `Exited` also covers a zombie not yet reaped by its parent
    /// (the test's `sleep` children are reaped by the test only afterwards).
    pub fn terminate(target: &Target, grace: std::time::Duration) -> Result<Outcome, ProcessError>;

    /// Any error type implementing `std::fmt::Debug` (the test only `expect`s on it).
    pub struct ProcessError;
}
```

The test spawns two `sleep 300` children, calls `Target::snapshot(victim_pid)` then
`terminate(&target, Duration::from_secs(5))`, and asserts `Outcome::Exited`, that the victim's exit status
is signal 15 (SIGTERM), and that the other `sleep` is still running 500 ms later. `SIGKILL` (AC2) is out of
this test's scope; the TUI path is demonstrated in G1 row D3. Re-exporting under different module paths or
adding fields/variants is fine as long as these names resolve; renaming them requires a change request.

## G1 red run (against `a743d65`, the vnext HEAD at T1.0)
Run 1 — plain `cargo test --test acceptance 2>&1`: the target does not compile, only because the
process-control API above does not exist yet (the single compile failure the T1.0 "Done when" allows):

```
error[E0433]: cannot find module or crate `saltnitor` in this scope
  --> tests/acceptance/g1_process.rs:11:5
   |
11 | use saltnitor::process::{Outcome, Target, terminate};
   |     ^^^^^^^^^ use of unresolved module or unlinked crate `saltnitor`
   |
   = help: if you wanted to use a crate named `saltnitor`, use `cargo add saltnitor` to add it to your `Cargo.toml`

For more information about this error, try `rustc --explain E0433`.
error: could not compile `saltnitor` (test "acceptance") due to 1 previous error
```

Run 2 — the same suite without `g1_process`
(`RUSTFLAGS="--cfg acceptance_without_process_api --check-cfg cfg(acceptance_without_process_api)" cargo test --test acceptance -- --test-threads=4 2>&1`;
long event dumps truncated):

```
running 14 tests
test g1_auth::a_valid_bearer_is_accepted ... ok
test g1_auth::healthz_answers_200_without_credentials ... ok
test g1_auth::every_protected_route_returns_401_envelope_with_a_wrong_bearer ... FAILED
test g1_auth::every_protected_route_returns_401_envelope_without_a_bearer ... FAILED
test g1_auth::query_token_on_ensure_stream_is_refused_by_default ... FAILED
test g1_byte_exact::non_streaming_body_through_the_proxy_equals_the_upstream_body ... ok
test g1_byte_exact::any_upstream_body_with_any_chunking_is_forwarded_byte_exact ... ok
test g1_byte_exact::realistic_sse_with_comments_and_tool_calls_is_forwarded_byte_exact ... ok
test g1_config::type_error_in_a_profile_exits_2_with_the_full_diagnostic_and_a_free_port ... FAILED
test g1_config::unknown_profile_key_exits_2_with_the_diagnostic_and_a_free_port ... FAILED
test g1_byte_exact::streaming_body_through_the_proxy_equals_the_upstream_body ... ok
test g1_streaming::first_sse_chunk_reaches_the_client_before_upstream_sends_the_last ... FAILED
test g1_compositional::auth_then_request_id_then_streaming_then_cancellation_in_one_flow ... FAILED
test g1_cancel::dropping_the_client_mid_stream_closes_the_upstream_request_within_budget ... FAILED
---- g1_auth::every_protected_route_returns_401_envelope_with_a_wrong_bearer stdout ----
thread 'g1_auth::every_protected_route_returns_401_envelope_with_a_wrong_bearer' (109699) panicked at tests/acceptance/g1_auth.rs:123:5:
GET /v1/models: status 200 OK, expected 401 (body "{\"data\":[{\"id\":\"A\",\"object\":\"model\",\"owned_by\":\"saltnitor\"}],\"object\":\"list\"}")
GET /v1/models: error.code is not AUTH_REQUIRED in "{\"data\":[{\"id\":\"A\",\"object\":\"model\",\"owned_by\":\"saltnitor\"}],\"object\":\"list\"}"
GET /v1/models: error.type is not authentication_error in "{\"data\":[{\"id\":\"A\",\"object\":\"model\",\"owned_by\":\"saltnitor\"}],\"object\":\"list\"}"
GET /v1/status: status 200 OK, expected 401 (body "{\"resident_models\":[\"A\"],\"endpoint\":\"http://127.0.0.1:34519/v1\",\"vram_used_gb\":0.4,\"vram_total_gb\":12.0,\"ram_free_gb\":24.0,\"profiles\":[\"A\"]}")
GET /v1/status: error.code is not AUTH_REQUIRED in "{\"resident_models\":[\"A\"],\"endpoint\":\"http://127.0.0.1:34519/v1\",\"vram_used_gb\":0.4,\"vram_total_gb\":12.0,\"ram_free_gb\":24.0,\"profiles\":[\"A\"]}"
GET /v1/status: error.type is not authentication_error in "{\"resident_models\":[\"A\"],\"endpoint\":\"http://127.0.0.1:34519/v1\",\"vram_used_gb\":0.4,\"vram_total_gb\":12.0,\"ram_free_gb\":24.0,\"profiles\":[\"A\"]}"
POST /v1/chat/completions: status 200 OK, expected 401 (body "{\"choices\":[{\"finish_reason\":\"stop\",\"index\":0,\"message\":{\"content\":\"x\",\"role\":\"assistant\"}}],\"id\":\"chatcmpl-fake\",\"model\":\"A\",\"object\":\"chat.completi …
POST /v1/chat/completions: error.code is not AUTH_REQUIRED in "{\"choices\":[{\"finish_reason\":\"stop\",\"index\":0,\"message\":{\"content\":\"x\",\"role\":\"assistant\"}}],\"id\":\"chatcmpl-fake\",\"model\":\"A\",\"object\":\"chat.complet …
POST /v1/chat/completions: error.type is not authentication_error in "{\"choices\":[{\"finish_reason\":\"stop\",\"index\":0,\"message\":{\"content\":\"x\",\"role\":\"assistant\"}}],\"id\":\"chatcmpl-fake\",\"model\":\"A\",\"object\":\"chat. …
POST /v1/ensure: error.code is not AUTH_REQUIRED in "{\"status\":\"error\",\"model\":\"\",\"endpoint\":\"http://127.0.0.1:34519/v1\",\"detail\":\"bad token\"}"
POST /v1/ensure: error.type is not authentication_error in "{\"status\":\"error\",\"model\":\"\",\"endpoint\":\"http://127.0.0.1:34519/v1\",\"detail\":\"bad token\"}"
GET /v1/ensure/stream: content-type "text/plain; charset=utf-8", expected application/json
GET /v1/ensure/stream: body is not JSON: "bad token"
---- g1_auth::every_protected_route_returns_401_envelope_without_a_bearer stdout ----
thread 'g1_auth::every_protected_route_returns_401_envelope_without_a_bearer' (109700) panicked at tests/acceptance/g1_auth.rs:99:5:
GET /v1/models: status 200 OK, expected 401 (body "{\"data\":[{\"id\":\"A\",\"object\":\"model\",\"owned_by\":\"saltnitor\"}],\"object\":\"list\"}")
GET /v1/models: error.code is not AUTH_REQUIRED in "{\"data\":[{\"id\":\"A\",\"object\":\"model\",\"owned_by\":\"saltnitor\"}],\"object\":\"list\"}"
GET /v1/models: error.type is not authentication_error in "{\"data\":[{\"id\":\"A\",\"object\":\"model\",\"owned_by\":\"saltnitor\"}],\"object\":\"list\"}"
GET /v1/status: status 200 OK, expected 401 (body "{\"resident_models\":[\"A\"],\"endpoint\":\"http://127.0.0.1:45305/v1\",\"vram_used_gb\":0.4,\"vram_total_gb\":12.0,\"ram_free_gb\":24.0,\"profiles\":[\"A\"]}")
GET /v1/status: error.code is not AUTH_REQUIRED in "{\"resident_models\":[\"A\"],\"endpoint\":\"http://127.0.0.1:45305/v1\",\"vram_used_gb\":0.4,\"vram_total_gb\":12.0,\"ram_free_gb\":24.0,\"profiles\":[\"A\"]}"
GET /v1/status: error.type is not authentication_error in "{\"resident_models\":[\"A\"],\"endpoint\":\"http://127.0.0.1:45305/v1\",\"vram_used_gb\":0.4,\"vram_total_gb\":12.0,\"ram_free_gb\":24.0,\"profiles\":[\"A\"]}"
POST /v1/chat/completions: status 200 OK, expected 401 (body "{\"choices\":[{\"finish_reason\":\"stop\",\"index\":0,\"message\":{\"content\":\"x\",\"role\":\"assistant\"}}],\"id\":\"chatcmpl-fake\",\"model\":\"A\",\"object\":\"chat.completi …
POST /v1/chat/completions: error.code is not AUTH_REQUIRED in "{\"choices\":[{\"finish_reason\":\"stop\",\"index\":0,\"message\":{\"content\":\"x\",\"role\":\"assistant\"}}],\"id\":\"chatcmpl-fake\",\"model\":\"A\",\"object\":\"chat.complet …
POST /v1/chat/completions: error.type is not authentication_error in "{\"choices\":[{\"finish_reason\":\"stop\",\"index\":0,\"message\":{\"content\":\"x\",\"role\":\"assistant\"}}],\"id\":\"chatcmpl-fake\",\"model\":\"A\",\"object\":\"chat. …
POST /v1/ensure: error.code is not AUTH_REQUIRED in "{\"status\":\"error\",\"model\":\"\",\"endpoint\":\"http://127.0.0.1:45305/v1\",\"detail\":\"bad token\"}"
POST /v1/ensure: error.type is not authentication_error in "{\"status\":\"error\",\"model\":\"\",\"endpoint\":\"http://127.0.0.1:45305/v1\",\"detail\":\"bad token\"}"
GET /v1/ensure/stream: content-type "text/plain; charset=utf-8", expected application/json
GET /v1/ensure/stream: body is not JSON: "bad token"
---- g1_auth::query_token_on_ensure_stream_is_refused_by_default stdout ----
thread 'g1_auth::query_token_on_ensure_stream_is_refused_by_default' (109799) panicked at tests/acceptance/g1_auth.rs:158:5:
GET /v1/ensure/stream?token=: status 200 OK, expected 401 (body "data: {\"stage\":\"received\",\"profile\":\"A\",\"model\":\"A\"}\n\ndata: {\"stage\":\"done\",\"status\":\"already_resident\",\"model\":\"A\",\"endpoint\":\"http://127.0.0.1:3 …
GET /v1/ensure/stream?token=: content-type "text/event-stream", expected application/json
GET /v1/ensure/stream?token=: body is not JSON: "data: {\"stage\":\"received\",\"profile\":\"A\",\"model\":\"A\"}\n\ndata: {\"stage\":\"done\",\"status\":\"already_resident\",\"model\":\"A\",\"endpoint\":\"http://127.0.0.1:39517/v1\",\"load …
---- g1_config::type_error_in_a_profile_exits_2_with_the_full_diagnostic_and_a_free_port stdout ----
thread 'g1_config::type_error_in_a_profile_exits_2_with_the_full_diagnostic_and_a_free_port' (109937) panicked at tests/acceptance/g1_config.rs:76:5:
stderr lacks `/tmp/saltnitor-acceptance-109697-11-badcfg-type/config.toml:7:<col>`
stderr lacks "profiles.x.est_vram_gb"
stderr lacks "expected float"
stderr lacks "found string \"9gb\""
--- stderr ---
error: unexpected argument '--config' found
Usage: saltnitor [OPTIONS]
For more information, try '--help'.
---- g1_config::unknown_profile_key_exits_2_with_the_diagnostic_and_a_free_port stdout ----
thread 'g1_config::unknown_profile_key_exits_2_with_the_diagnostic_and_a_free_port' (109951) panicked at tests/acceptance/g1_config.rs:118:5:
stderr lacks `/tmp/saltnitor-acceptance-109697-12-badcfg-unknown/config.toml:7:<col>`
stderr lacks "profiles.x.ctx_size"
stderr lacks "unknown key"
--- stderr ---
error: unexpected argument '--config' found
Usage: saltnitor [OPTIONS]
For more information, try '--help'.
---- g1_streaming::first_sse_chunk_reaches_the_client_before_upstream_sends_the_last stdout ----
thread 'g1_streaming::first_sse_chunk_reaches_the_client_before_upstream_sends_the_last' (109963) panicked at tests/acceptance/g1_streaming.rs:36:5:
first read already contains the terminal frame — the body was buffered: "data: {\"choices\":[{\"delta\":{\"content\":\"A\"},\"finish_reason\":null,\"index\":0}],\"id\":\"chatcmpl-fake\",\"model\":\"A\",\"object\":\"chat.completion.chunk\"}\ …
---- g1_compositional::auth_then_request_id_then_streaming_then_cancellation_in_one_flow stdout ----
thread 'g1_compositional::auth_then_request_id_then_streaming_then_cancellation_in_one_flow' (109922) panicked at tests/acceptance/g1_compositional.rs:42:5:
assertion `left == right` failed: step 1: status
  left: 200
 right: 401
---- g1_cancel::dropping_the_client_mid_stream_closes_the_upstream_request_within_budget stdout ----
thread 'g1_cancel::dropping_the_client_mid_stream_closes_the_upstream_request_within_budget' (109907) panicked at tests/acceptance/g1_cancel.rs:41:5:
no `disconnect` recorded by the fake within 1.1s of the client dropping; events: [Object {"body": String(""), "has_authorization": Bool(false), "headers": Object {"accept": String("*/*"), "host": String("127.0.0.1:39221")}, "kind": String(" …
    g1_auth::every_protected_route_returns_401_envelope_with_a_wrong_bearer
    g1_auth::every_protected_route_returns_401_envelope_without_a_bearer
    g1_auth::query_token_on_ensure_stream_is_refused_by_default
    g1_cancel::dropping_the_client_mid_stream_closes_the_upstream_request_within_budget
    g1_compositional::auth_then_request_id_then_streaming_then_cancellation_in_one_flow
    g1_config::type_error_in_a_profile_exits_2_with_the_full_diagnostic_and_a_free_port
    g1_config::unknown_profile_key_exits_2_with_the_diagnostic_and_a_free_port
    g1_streaming::first_sse_chunk_reaches_the_client_before_upstream_sends_the_last
test result: FAILED. 6 passed; 8 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.36s
error: test failed, to rerun pass `--test acceptance`
```

Why each result is what it is today:
| Test | Today | Reason |
|---|---|---|
| `g1_auth::healthz_answers_200_without_credentials` | pass | harness proof: binary starts in the pty, reads the temp `$HOME` config, binds the chosen port |
| `g1_auth::a_valid_bearer_is_accepted` | pass | `/v1/models` is open today, so a bearer is trivially "accepted" |
| `g1_byte_exact::*` (4) | pass | the baseline forwards the buffered body unmodified (BD-02 buffers, but does not alter bytes) |
| `g1_auth::every_protected_route_returns_401_envelope_without_a_bearer` | red | `/v1/models` and `/v1/status` answer 200 without credentials (BD-03) |
| `g1_auth::every_protected_route_returns_401_envelope_with_a_wrong_bearer` | red | same routes ignore the header entirely |
| `g1_auth::query_token_on_ensure_stream_is_refused_by_default` | red | `?token=` is accepted and the SSE stream starts (BD-04) |
| `g1_config::*` (2) | red | clap exits 2 on the unknown `--config` flag; stderr has no `<file>:<line>:<col>`, key path, or expected/found text (BD-01) |
| `g1_streaming::first_sse_chunk_…` | red | the first read already contains `[DONE]`: the body was buffered (BD-02) |
| `g1_cancel::dropping_the_client_…` | red | the fake records no `disconnect` within 1.1 s; the proxy keeps draining the upstream |
| `g1_compositional::…` | red | fails at step 1: unauthenticated `POST /v1/chat/completions` answers 200 |
| `g1_process::…` | does not compile | `saltnitor::process` does not exist (T1.8 lib.rs, T1.13 module) |
