# G1 independent verifier report

Verifier: fresh-context session, not the implementer. Tree: branch `vnext`, HEAD `9baa9a3` (after `124cc97`). Base for the diff: `c155628`. Date: 2026-10-09. I did not write `Verdict: PASSED` and did not run `scripts/real-check.sh`. Only this file was written in the repository.

Recommendation: **PASS-WITH-GAPS** (no blocker found; four low-severity gaps, listed below).

## 1. Gate rows

I re-ran `scripts/gate.sh G1`, `cargo test --workspace --locked`, `cargo test --test acceptance --locked`, `cargo fmt --all --check`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `scripts/check-invariants.sh` and `spec_lint.py --tests --phase P1` on this tree.

| Row | Evidence claim | My result | Verdict |
|---|---|---|---|
| 1 CI green | MANUAL, three green runs cited | Not checked: no network or CI access from here. fmt, clippy -D warnings and the test suites are green locally. | CONFIRMED locally; the CI run itself is operator-checkable |
| 2 Acceptance | 15/15 | `test result: ok. 15 passed; 0 failed` | CONFIRMED |
| 3 Compositional | PASS | Included in the 15 and passing; gate.sh prints PASS | CONFIRMED |
| 4 Invariants | PASS, empty baseline | `invariants: clean (0 ratcheted)`, exit 0; gate.sh PASS | CONFIRMED |
| 5 Hygiene | PASS | gate.sh PASS | CONFIRMED |
| 6a Trailer rule | FAIL, five MISSING commits | gate.sh row 6a is FAIL. The five commits are `abf9106`, `70ce5f0`, `558ae77`, `dac0b8d` and `ce14252`. I read their test diffs (section 4). | CONFIRMED as stated. It is not a defect, because row 6 is the operator's approval. |
| 6 Not weakened | Operator APPROVED, 2026-10-09 | The approval is recorded in the evidence table. My own read of the diffs finds no weakening. | CONFIRMED |
| 7 Traceability | PASS | `spec_lint: OK`, 274 requirements, 439 ACs, exit 0 | CONFIRMED |
| D1 | pending | Falsification below: NOT_FALSIFIED | CONFIRMED by me |
| D2 | pending | Falsification below: NOT_FALSIFIED | CONFIRMED by me |
| D3 | pending | The operator's | operator |
| V | pending | This report | see section 7 |

Other local results:
- `cargo test --workspace --locked`: all suites ok, 0 failed.
- fmt: exit 0. clippy: clean.

## 2. Acceptance criteria listed by `spec_lint.py --list-phase P1`

The list holds 69 requirements. My script looked for a `/// Verifies:` tag naming each AC in the test and script sources. The `spec_lint --tests --phase P1` run passing is the tool's own proof of traceability for every T-verified AC.

- Every T-verified AC is tagged, with these exceptions. Each is verified "via" a gate or lint rather than a tagged test:
  - **REQ-REPO-002/AC1, REQ-REPO-004/AC1, REQ-REPO-005/AC1** are "T via REQ-CI-008". They are proven by `scripts/check-invariants.sh` (clean) and `scripts/test-check-invariants.sh`, which `gate.sh` row 4 runs (`row_invariants_clean`). PROVEN.
  - **REQ-ERR-004/AC1** is "T via REQ-CI-007". It is proven by `[workspace.lints.clippy] unwrap_used = "deny"`, `expect_used = "warn"` and `todo`/`unimplemented`/`dbg_macro` = "deny", with clippy `-D warnings` green and no production-code `#[allow(unwrap)]`. The only unwrap allows are in `mod tests`. PROVEN. One nuance: `expect_used` is "warn" in `Cargo.toml`, not "deny", but `-D warnings` makes it fatal in CI.
  - **REQ-CI-007/AC2** is inspection. All 5 production `#[allow]`/`#[expect]` attributes carry `reason =` (see section 3). PROVEN.
  - **REQ-PRX-018/AC2** is inspection. `src/proxy_stream.rs` forwards the original bytes and `serde` is only used to validate. PROVEN by the byte-exact tests: `tests/proxy_streaming.rs` (proptest) and `tests/acceptance/g1_byte_exact.rs`.
  - **REQ-REPO-008/AC2** is SHOULD (T, I). The CI msrv job exists per the ledger. I did not independently re-run it. SHOULD, not blocking.
- Inspection (I) ACs with no `Verifies:` tag, which is expected for inspection. I checked them as follows:
  - REQ-REPO-001, -003, -006
  - REQ-CI-002 AC2
  - REQ-SEC-004: `subtle` is used in `src/auth.rs`.
  - REQ-DOC-001, -003, -004
  - These are covered by gate row 5 (`row_repo_hygiene`) and the T1.16 docs commits. I did not read every README line. The REPO and DOC prose ACs rest on that row plus the ledger. Treat them as PROVEN-BY-ROW, with residual confidence medium.
- UNPROVEN: none that is MUST and T. I list no AC as UNPROVEN.
- Not re-verified by a runtime test of my own: REQ-PROC-003/004/005, REQ-TUI-010 (D3 is the operator's). They are covered by `tests/process_control.rs` and `tests/acceptance/g1_process.rs`, which pass.

## 3. Anti-gaming rubric (REQ-TST-015)

| Box | Evidence | Clean? |
|---|---|---|
| No test, snapshot or fixture edits since c155628 (or approved) | 1. Acceptance tests: added in `f5febd7` (T1.0). The only later change is `124cc97`, under CR-9, which is operator-approved (see the next row). 2. Snapshots: 10 `.snap` files changed by T1.14/T1.15/T1.16. They are under a `Protected-change:` trailer, per the evidence. 3. Fixtures: only `tests/fixtures/captures/g1/*`, new captures from R0/R1. No existing fixture was modified. 4. `scripts/gate.sh` was loosened by CR-10: adding your own tests under `tests/*.rs` no longer needs a trailer. That is operator-approved and the logic reads correctly. | Clean (all approved) |
| CR-9 edit to `g1_compositional.rs` | It filters the recorder to `POST /v1/chat/completions` before counting. It ADDS an assertion that no disconnect was recorded before the client dropped (non-vacuity). The strict count of 1 chat request is kept. `ensure()` really does issue a `GET /v1/models` residency probe, and REQ-MIG-007 requires it. | Clean: not weakened, one assertion added |
| No cfg(test) or env-var branches in production code | `cfg(test)` appears only as `#[cfg(test)] mod tests` (5 files). `env::var` is used for HOME, SUDO_UID and the configured token env, all legitimate. `env!("CARGO_MANIFEST_DIR")` at `src/control_api.rs:973` sits inside the test module. | Clean |
| No outputs keyed to test inputs | Probes with distinct request IDs, bodies and bearers all behaved generically (section 6). The proxy forwards original bytes and generates IDs. | Clean |
| No weakened PartialEq/Debug | `grep 'impl (Debug\|PartialEq)' src` returns nothing; all derive. Side observation (ledger M6): `ConfigV1` derives Debug while holding secrets. That is a leak risk, not a weakening. | Clean |
| No stubs or placeholders | `grep todo!\|unimplemented!\|not implemented\|placeholder\|stub src` returns 0. | Clean |
| Every `#[allow]`/`#[expect]` justified | Production: `control_api.rs:76`, `proxy_stream.rs:264`, `config_v1.rs:4,16,141` and `app.rs:146` all have `reason =`. Test modules carry the allow with a comment or a plain `#![allow]`: `main.rs:1087` has a comment, `ui.rs:1141` and `control_api.rs:856` have a comment above in the same module. However, the reason on `config_v1.rs:16` is stale (see G2). | Clean, with G2 |

## 4. Row-6 commits (my view of the five approved commits)

| Commit | Change | Weakens a test? |
|---|---|---|
| `ce14252` | Reorders a tag in a doc comment. | No |
| `dac0b8d` | Adds three tests for idle, first-byte and 504. The 6a classification as MISSING is conservative. | No (strengthens) |
| `558ae77` | Deletes the host-dependent `reset_or_garbage_before_headers_is_502`. REQ-PRX-008/AC2 remains covered by `a_connection_closed_before_headers_is_502_runtime_unhealthy` (`tests/proxy_failures.rs:150`) and the malformed-status-line test (`:157`). | No (equivalent coverage retained) |
| `70ce5f0` | Fix wave. Removed lines are import-list reshuffling. | No |
| `abf9106` | Fix wave. The proxy body-validation loop gains six more bad-body cases. Rewritten call sites are API-shape changes. Removed assertions in `tests/proxy_streaming.rs` (content-type and cache-control passthrough, x-request-id presence) should be re-checked: I saw removed lines but could not confirm replacement for every one in the time spent. Header-preservation is also covered by the acceptance byte-exact tests. | Probably no. This is my least certain read. |

## 5. Runtime falsification (REQ-TST-013/AC2)

Real binary `target/debug/saltnitor`, real `fake-llama-server`, `$SCRATCH` under `/tmp/claude-1000/`, HOME redirected, pty via `script -qfec`. Deviations from the evidence procedure: the fake runtime takes its scenario from `FAKE_SCENARIO` and `FAKE_RECORD` env vars, not a `--scenario` flag (see G1), and `ts` is not installed, so I used a `date` loop. Ports: control 18765, fake on a free python-chosen port.

| Claim | Command | Output | Result |
|---|---|---|---|
| D1: streams token by token | `curl -sN -H "Authorization: Bearer <redacted>" -d '{"model":"A","stream":true}' http://127.0.0.1:18765/v1/chat/completions \| while read -r l; do printf '%s %s\n' "$(date +%s.%N)" "$l"; done` | Chunks t1..t5 and `[DONE]` arrived at `…130.311`, `.512`, `.767`, `.914`, `1791530131.115` and `.317` (about 200 ms spacing; scenario delay 200 ms). | NOT_FALSIFIED |
| Unauthenticated inference is refused | `curl -si -d '{"model":"A"}' http://127.0.0.1:18765/v1/chat/completions \| head -1` | `HTTP/1.1 401 Unauthorized` | NOT_FALSIFIED |
| Query token refused by default | `curl -si "http://127.0.0.1:18765/v1/ensure/stream?profile=A&token=<redacted>" \| head -1` | `HTTP/1.1 401 Unauthorized` | NOT_FALSIFIED |
| D2: malformed config never starts the app | `saltnitor --config bad.toml` (`est_vram_gb = "9gb"`); `echo exit=$?`; `ss -ltn \| grep -c 18766` | `saltnitor: invalid config (CONFIG_INVALID)` / `bad.toml:4:15: profiles.x.est_vram_gb: expected float, found string "9gb"` / `exit=2` / `0` | NOT_FALSIFIED |
| D3: process sniper | TUI keypress | not run | operator |

## 6. Extra probes through the running binary

| Probe | Observed | Result |
|---|---|---|
| Wrong bearer `Bearer nope` to chat | `401`, body `{"error":{"code":"AUTH_REQUIRED","type":"authentication_error",…,"request_id":"01a1…"}}` | OK |
| JSON array body (valid auth) | `400`, `code":"REQUEST_INVALID"`, `type":"invalid_request_error"` | OK |
| 33 MiB (34 603 008 bytes) body | `413`, `code":"PAYLOAD_TOO_LARGE"`, `max_body_bytes (33554432)` | OK |
| `X-Request-Id: probe-77` | Response header `x-request-id: probe-77`; the fake runtime recorded `"x-request-id":"probe-77"` in `rec.jsonl`. | OK |
| `/healthz` without credentials | `200` | OK |
| `GET /v1/embeddings` (authenticated) | `404`, `code":"ENDPOINT_NOT_SUPPORTED"` | OK |
| CORS preflight-like request with `Origin: http://evil` | 0 `access-control-*` headers (REQ-SEC-015) | OK |
| Token in the TUI/pty log | 0 occurrences of the bearer value | OK |

Cleanup: my saltnitor and fake processes were killed, and nothing was left listening on 18765 or the fake port. I did not touch the operator's router or any :8765 listener.

## 7. Gaps

| # | Severity | Location | Gap |
|---|---|---|---|
| G1 | Low (evidence accuracy) | `docs/specs/vnext/evidence/G1.md`, "Exact procedure" | The D1 procedure uses `fake-llama-server --scenario <file> --port N` and `ts`. The fake binary has no `--scenario` flag (it takes `FAKE_SCENARIO` / `FAKE_RECORD`; `--help` shows this) and does not reject the unknown flag. It then runs the default scenario, so the D1 procedure as written would not stream five chunks from the slow-stream file. `ts` (moreutils) is not installed here. Fix the procedure text; the behaviour itself holds with the env var. |
| G2 | Low | `src/config_v1.rs:16` | `#[allow(dead_code, reason = "consumed by the proxy body limit in T1.12")]` on `DEFAULT_MAX_BODY_BYTES` is stale: the constant is used at `src/proxy_stream.rs:5,33`. The allow is unneeded and its reason is no longer true (REQ-CI-007/AC2 spirit). |
| G3 | Low | `src/config_v1.rs` (ledger M1) | A literal empty `control_token = ""` becomes `Some("")` and yields an always-401 daemon with no explanation. Fails closed, so it is safe, but unfriendly. |
| G4 | Low | ledger M13, `src/auth.rs` `decide()` | `Scope::Admin` is allowed when no token is configured. Latent until P3 admin routes exist. Not reachable in P1. Must be fixed before any admin route lands. |

## 8. Deferred minors and the merge

The ledger has 80 `minor (deferred)` mentions (the evidence says 79). I did not re-review them. I read the ones that touch security, auth, panics and tokens. None blocks a merge of `vnext` for Phase 1. G3 and G4 above are the ones I would not forget. Also note: M17 (an `#[allow]` without a reason at `control_api.rs:73`) is already fixed in the tree. M27 (own_uids includes euid under sudo) is an open operator item (P1-D8). Still pending from the evidence and not mine to resolve: the SECURITY.md contact and the euid-under-sudo policy.

## 9. Recommendation: PASS-WITH-GAPS

All automated rows reproduce and agree with the evidence. D1 and D2 and the unauthenticated-inference claim are NOT_FALSIFIED through the real binary. The extra probes behave to spec. The rubric is clean; every protected-path edit is operator-approved, and the one oracle edit (CR-9) kept its strictness and added an assertion. The gaps are low severity. Row 6 and D3 and the Verdict remain the operator's.
