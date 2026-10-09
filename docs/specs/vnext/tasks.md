# Saltnitor vNext — Implementation Tasks

> **Spec:** `saltnitor-vnext` · **File:** `tasks.md` · **Revision:** r2 (post-audit) · **Date:** 2026-09-27
> **Contract:** `requirements.md` (same directory). Every task cites the requirement IDs it must satisfy; every gate row cites what it proves.
> **Baseline:** `saltless-bruh/saltnitor` @ `c89f278` · **Work branch:** `vnext`

## Quick map

| Section | Use |
|---|---|
| **Start here** | Session protocol, boundaries, and Definition of Done — read every session |
| **Phase map** | Where the project is; which gate is next |
| **P0 … P11** | Each phase has a brief (goal, entry, read-first, out of scope), tasks, and a gate. Load only the current phase |
| **Appendices** | Templates (evidence, progress, BLOCKED, change request), retired Appendix B, generated coverage index |

Extract one phase: `P=P4; sed -n "/<!-- phase:$P -->/,/<!-- \/phase:$P -->/p" tasks.md`.

---

## Start here

### Session protocol (every session)
1. `pwd`; `git status`; `git log --oneline -15`; read the last 20 lines of `docs/specs/vnext/PROGRESS.md`.
2. Run the Standard Verification (SV) below **before** changing anything. If it fails, fixing that is your first task — record it in PROGRESS.md.
3. Find the current phase: the first phase whose gate is not `PASSED` in `docs/specs/vnext/evidence/`.
4. Take the lowest-numbered unchecked task in that phase whose `Depends` are all `[x]` (the `Tn.0` gate task always comes first).
5. Read the task's requirement IDs (`grep -n -A12 "^#### REQ-…" requirements.md`) and the invariants in `requirements.md` §3.
6. Work that one task until its **Done when** holds. Then update the checkbox, commit, and PROGRESS.md (see Definition of Done). Stop and hand off if the context is getting long.

**Standard Verification (SV):**
```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all --locked
scripts/check-invariants.sh                      # from T1.5
python3 docs/specs/vnext/tools/spec_lint.py      # from T0.9
```

### Boundaries

| ✅ Always | ⚠️ Ask first (write a `BLOCKED:` entry and stop) | 🚫 Never |
|---|---|---|
| Run SV before and after each task | Adding a crate outside the policy (REQ-ARCH-008) | Edit `requirements.md`, gate definitions, `tests/acceptance/`, fixtures, snapshots, or `scripts/gate.sh` to make something pass |
| Cite `Task:` and `Refs:` in every commit | Any `[HW]` or `[HUMAN]` step | Delete, `#[ignore]`, or weaken a test; lower a threshold in `requirements.md` §6 |
| Put `/// Verifies: REQ-…/ACn` on tests | Changing a public API or config key not described by the task | Commit secrets; hardcode tokens or `/home/<user>` paths |
| Keep to the task's **Files** and **Not in scope** | Destructive git operations (rebase of shared history, force-push, tag deletion) | `killall`/`pkill`/kill by name; bind `0.0.0.0` or non-loopback without an explicit spec task |
| Record evidence (commands + output) | Anything an open question (`OQ-*`) blocks | `git pull`/build inside `~/ai-runtimes/*`; touch the production runtime from tests |
| Leave the app runnable at every commit | A requirement that looks wrong → change request (REQ-DOC-007) | `--no-verify`, disabling CI checks, deleting `Cargo.lock`, `#[allow]` without a justification comment |
| Prefer the real binary + fake runtime over mocks of your own code | | Special-case test inputs, stub features, or return placeholders to turn tests green |
| | | Mark a gate `PASSED`, or tick a `[HUMAN]` item yourself |

### Definition of Done (every task)
- [ ] Every cited AC — every AC of a requirement cited whole — is met by its verification method. Tests carry `Verifies:` tags.
- [ ] SV passes locally; CI is green on the pushed commit.
- [ ] No boundary violated; nothing in **Not in scope** touched.
- [ ] Docs and examples updated in the same commit when behavior changed (REQ-DOC-007/AC3).
- [ ] Commit: subject `T<p>.<n>: <summary>`; body has `Task: T<p>.<n>` and `Refs: <REQ IDs>`.
- [ ] Checkbox ticked, and a PROGRESS.md line appended (template in the appendix).

### Task format
`- [ ] **Tp.n — Title**` then: **Reqs** (IDs to satisfy; `REQ-X/ACn` limits the task to that AC) · **Depends** (task IDs) · **Read first** (source sections) · **Files** (expected touch set) · **Do** (approach; the design choices are yours within the requirements) · **Not in scope** · **Done when** (exact checks). Tags: `[P]` = can run in parallel with other `[P]` tasks of the same phase (disjoint files, no dependency) · `[HW]` reference host · `[HUMAN]` operator · `test-first` = write the failing test first.

### Standard gate procedure (every gate)
1. **Mechanical:** `scripts/gate.sh G<n>` runs every automated row and prints PASS/FAIL per row.
2. **Independent verification** (REQ-TST-013/AC1): a verifier other than the implementer — the operator, or a fresh-context reviewer agent given only `requirements.md`, the diff since the previous gate, and the evidence — confirms every row **and** every acceptance criterion listed by `spec_lint.py --list-phase P<n>`, and applies the anti-gaming rubric (REQ-TST-015).
3. **Runtime falsification** (REQ-TST-013/AC2): each claim listed under the gate is exercised through the real binary's CLI/HTTP/TUI surface and recorded as `NOT_FALSIFIED` or `FALSIFIED` (e.g., with the `runtime-feature-falsifier` skill).
4. **Evidence:** `docs/specs/vnext/evidence/G<n>.md` holds the commit SHA, CI run URL, outputs, verifier identity, rubric findings, and falsification results.
5. **Sign-off:** only the operator writes `Verdict: PASSED`, then tags the version where one is listed.

**If a gate fails:** do not start the next phase. Open fix tasks `T<p>.<n>-fix-k` in the current phase and repeat the procedure.

---

## Phase map

| Phase | Name | Version | BP / TP source | Gate |
|---|---|---|---|---|
| P0 | Baseline freeze & test harness | — | BP §27 Ph0 | G0 |
| P1 | Hardening | v0.2 | BP §4, §27 Ph1 | G1 |
| P2 | Module extraction | v0.2.x | BP §7, §27 Ph2 | G2 |
| P3 | Daemon / TUI / CLI | v0.3 | BP §6, §22, §27 Ph3 | G3 |
| P4 | Runtime backends & config v2 | v0.4 | BP §8–10, §27 Ph4; TP Ph1 | G4 |
| P5 | Telemetry v2 & Oracle v2 | v0.5-a | BP §11–12, §27 Ph5 | G5 |
| P6 | Residency scheduler | v0.5-b | BP §13 (DEC-02) | G6 |
| P7 | Benchmark laboratory | v0.5-c | BP §19–20, §27 Ph6; TP Ph3 | G7 |
| P8 | MoE / Codacus tuning lab | v0.5 | BP §16–18, §27 Ph7; TP Ph2–3 | G8 |
| P9 | Remote agent serving | v0.6 | BP §15, §27 Ph8; TP Ph5 | G9 |
| P10 | Flagship validation | v0.6.x | BP §31, §33; TP Ph4, Part 2 | G10 |
| P11 | v1.0 readiness | v1.0 | BP §28 | G11 |

---

<!-- phase:P0 -->
## Phase 0 — Baseline Freeze & Test Harness

**Goal:** pin today's behavior and build the test tooling every later phase relies on.
**Entry:** repository at `c89f278`. **Read first:** BP §3, §27 Ph0; `requirements.md` §0–§5.
**Out of scope:** any production behavior change. Only test modules, tools, docs, and fixtures may be added.

- [x] **T0.1 — Branch, spec scaffolding, CLAUDE.md**
  - **Reqs:** REQ-REPO-007, REQ-DOC-005, REQ-DOC-007
  - **Files:** `docs/specs/vnext/{requirements.md,tasks.md,PROGRESS.md,CHANGE_REQUESTS.md}`, `docs/specs/vnext/{sources,baseline,evidence,tools}/`, `CLAUDE.md`, `.github/workflows/rust.yml` (CR-1)
  - **Do:** create `vnext` from `master`; `git mv` both spec files and the rest of `specs/` into `docs/specs/vnext/`. Move both source documents into `docs/specs/vnext/sources/`: `specs/SALTNITOR_VNEXT_BLUEPRINT.md` and `docs/proposal/Technical Proposal — Saltnitor Qwen MoE Runtime, Tuning Laboratory, and Remote Agent Server.md`. Add empty PROGRESS and CHANGE_REQUESTS logs. Update every `specs/…` and `docs/proposal/…` path in the root `CLAUDE.md` to the new locations, and confirm it still meets REQ-DOC-005 (no `AGENTS.md`; r2.1) *(operator decisions, 2026-09-28)*. Add `vnext` to the `push`/`pull_request` branches of `rust.yml` (CR-1).
  - **Done when:** `git branch --show-current` prints `vnext`; `wc -l CLAUDE.md` ≤ 220; CLAUDE.md links (does not copy) the spec files; `AGENTS.md` does not exist; `specs/` and `docs/proposal/` no longer exist; `grep -nE "specs/(requirements|tasks|spec_lint|SALTNITOR)|docs/proposal" CLAUDE.md` finds only `docs/specs/vnext/` paths.

- [x] **T0.2 — Behavior inventory** `[P]`
  - **Reqs:** REQ-MIG-001
  - **Depends:** T0.1
  - **Files:** `docs/specs/vnext/baseline/BEHAVIOR.md`
  - **Do:** list CLI flags (`Cli`), config keys and defaults (`TomlConfig`, `ProfileMeta`), every keybinding, every route (method, auth, status codes, body shape), files read/written, and external commands — each with a `file:line` citation.
  - **Done when:** every route in `control_api::serve` and every `KeyCode` arm in `main.rs` is listed with a citation.

- [x] **T0.3 — Defect register** `[P]`
  - **Reqs:** REQ-MIG-006
  - **Depends:** T0.1
  - **Files:** `docs/specs/vnext/baseline/DEFECTS.md`
  - **Do:** re-verify every BD in the `requirements.md` §5 register (BD-01…BD-34 as of r2.3) at `c89f278`: confirm the evidence lines, add a one-line reproduction, leave `Fixed by:` empty. BD-32 needs a `/models` sample from the operator's real router `[HUMAN]` (or the upstream docs); mark it confirmed or disputed.
  - **Done when:** every register entry (34 as of r2.3) is confirmed or disputed with a reason.

- [x] **T0.4 — Fake runtime tool** · *test infrastructure*
  - **Reqs:** REQ-TST-011, REQ-TST-006
  - **Depends:** T0.1
  - **Files:** root `Cargo.toml` (`[workspace]`), `tools/fake-llama-server/**`, `tests/fixtures/captures/**` (CR-2)
  - **Do:** make the repo a workspace (root package unchanged, plus `tools/fake-llama-server` with `publish = false`). The fake serves `/health`, `/v1/models`, `/models` (with `status.value` objects), `/models/load|unload`, `/v1/chat/completions` (stream and non-stream), `/metrics`, `/slots`. It runs from a scenario file (chunks and delays, status codes, malformed responses, hang before headers, crash after N chunks, OOM when an arg exceeds a threshold). It prints fixture `--help`/`--version` selected by env. It records argv, env, and requests to JSONL, and records client-disconnect times. Build it as lib + bin: the library runs HTTP scenarios in-process for fast tests; the binary covers process-level behavior (argv, env, `--help`, crashes). The crate README documents how tests locate the binary. Add a `record` mode (R0: read-only GETs of `/health`, `/models`, `/v1/models`, `/props`, `/slots`, `/metrics` from a live router, saved sanitized, with a liveness/shape-drift table) and `replay_from` scenarios that serve a capture byte-for-byte (CR-2).
  - **Done when:** `cargo test -p fake-llama-server` covers every scenario feature.

- [x] **T0.5 — Characterization: pure helpers** `[P]`
  - **Reqs:** REQ-MIG-002
  - **Depends:** T0.1
  - **Files:** `#[cfg(test)]` modules in `src/main.rs`, `src/control_api.rs` (additions only)
  - **Do:** table-driven tests for `upsert_ini_section` (replace, append, comments kept, other sections untouched, missing section → `None`), `estimate_footprint`, `parse_params_b`, `parse_bpw`, and `Stage::from_outcome`.
  - **Done when:** the tests pass on unmodified code; `git diff c89f278 -- src` shows only test-module additions.

- [x] **T0.6 — Characterization: control API**
  - **Reqs:** REQ-MIG-002, REQ-TST-002, REQ-MIG-007/AC1, REQ-MIG-007/AC2
  - **Depends:** T0.4, T0.5
  - **Files:** `#[cfg(test)]` in `src/control_api.rs`, `tests/fixtures/scenarios/*.toml`
  - **Do:** characterize `/healthz`, `/v1/models`, `/v1/status`, `/v1/ensure` (already_resident, loaded, unknown profile, oracle reject), `/v1/ensure/stream` (stage order), and non-stream chat against the fake runtime. Record BD-02 buffering as timing evidence in `DEFECTS.md` — do not assert it.
  - **Done when:** the tests pass on baseline code; BD-02 has timing evidence.

- [x] **T0.7 — TUI visual baseline** `[P]`
  - **Reqs:** REQ-MIG-005, REQ-TUI-009, REQ-MIG-007/AC3, REQ-MIG-007/AC4
  - **Depends:** T0.1
  - **Files:** test module in `src/ui.rs`, `src/snapshots/*` (insta)
  - **Do:** use ratatui `TestBackend` with a fixed `App` fixture. Snapshot the dashboard, both bottom-deck modes, tuner pages 1–3, GPU/CPU inspectors, help, and search, plus the too-small refusal at 79×16 and 80×15.
  - **Done when:** the snapshots are committed and pass.

- [x] **T0.8 — Known-good router fixture** `[HUMAN]`
  - **Reqs:** REQ-MIG-004
  - **Depends:** T0.1
  - **Files:** `tests/fixtures/router/{known-good.ini,readme-example.ini}`
  - **Do:** the operator supplies the working `router.ini`; the agent sanitizes the paths and adds it plus the README example.
  - **Done when:** both fixtures exist and `grep -n "/home/" tests/fixtures/router/*` shows only placeholders.

- [x] **T0.9 — Spec lint installed**
  - **Reqs:** REQ-DOC-006/AC1, REQ-TST-016
  - **Depends:** T0.1
  - **Files:** `docs/specs/vnext/tools/spec_lint.py`, `docs/specs/vnext/tools/test_spec_lint.py` (both shipped with the spec pack)
  - **Do:** install both files. `--sync` regenerates the `Verify`/`Phase`/`Tasks` fields; `--tests --phase P0` checks `Verifies:` tags from G0 on, so tag the characterization tests as you write them. CI wiring happens in T1.3.
  - **Done when:** `python3 docs/specs/vnext/tools/spec_lint.py` exits 0, and `python3 -m unittest discover -s docs/specs/vnext/tools` passes.

- [x] **T0.10 — Protected acceptance scaffold** `[HUMAN]`
  - **Reqs:** REQ-TST-012, REQ-TST-007, REQ-TST-013, REQ-TST-015, REQ-MIG-003
  - **Depends:** T0.1
  - **Files:** `.github/CODEOWNERS`, `tests/acceptance/README.md`, `scripts/gate.sh`, `docs/specs/vnext/evidence/TEMPLATE.md`, `docs/specs/vnext/tools/verifier-prompt.md`, `scripts/real-check.sh` (CR-2)
  - **Do:** add CODEOWNERS (operator) for `docs/specs/**`, `tests/acceptance/**`, `tests/fixtures/**`, `scripts/gate.sh`, and `**/snapshots/**`. Write `scripts/gate.sh G<n>`, which runs a gate's automated rows and prints a PASS/FAIL table. The operator enables branch protection on `master` (reviews required from code owners) and protects `vnext` against force-push and deletion; `gate.sh` lists protected-path changes since the previous gate for operator approval (CR-3). Add `scripts/real-check.sh` (R1 real-world journey, `[HW]`, CR-2). Add the evidence template (Appendix A, including the anti-gaming rubric) and `verifier-prompt.md`: the instructions for a fresh-context reviewer, which gets only requirements, diff, and evidence, and is told to report gaps that affect correctness or stated requirements.
  - **Done when:** `scripts/gate.sh G0` runs; the operator confirms branch protection in `evidence/G0.md`.

- [x] **T0.11 — Test baseline record**
  - **Reqs:** REQ-CI-005/AC2
  - **Depends:** T0.5, T0.6, T0.7
  - **Files:** `docs/specs/vnext/evidence/test-baseline.txt`
  - **Done when:** per-suite counts from `cargo test -- --list` are recorded, all non-zero.

### Gate G0 — Baseline

| # | Check | Command / procedure | Pass condition | Reqs |
|---|---|---|---|---|
| 1 | Characterization + snapshots | `cargo test --all` | green; counts ≥ `test-baseline.txt` | REQ-MIG-002, REQ-MIG-005 |
| 2 | No behavior change | `git diff c89f278 -- src/` | only test modules and snapshots added | REQ-MIG-003 |
| 3 | Fake runtime | `cargo test -p fake-llama-server` | green | REQ-TST-011 |
| 4 | Inventory and defects | review `BEHAVIOR.md`, `DEFECTS.md` | complete per T0.2/T0.3 | REQ-MIG-001, REQ-MIG-006 |
| 5 | Protection | CODEOWNERS present; branch protection confirmed | yes `[HUMAN]` | REQ-TST-012 |
| 6 | Spec lint and traceability | `spec_lint.py --tests --phase P0`; `python3 -m unittest discover -s docs/specs/vnext/tools` | exit 0; self-test green | REQ-DOC-006, REQ-TST-016 |
| 7 | Smoke | `cargo run` against the fake runtime; `curl -s :8765/healthz` | TUI renders; 200 | REQ-MIG-003 |
| V | Independent verification and falsification | Standard gate procedure steps 2–3 | verifier ≠ implementer; rubric clean; every listed claim `NOT_FALSIFIED` | REQ-TST-013, REQ-TST-015 |

**Claims to falsify:** none (no product change). **Evidence:** `evidence/G0.md`.
<!-- /phase:P0 -->

---

<!-- phase:P1 -->
## Phase 1 — Hardening (v0.2)

**Goal:** fix the P0 correctness and safety problems of BP §4. No major new features.
**Entry:** G0 PASSED. **Read first:** BP §4, §24, §25, §29 "Hardening gate".
**Out of scope:** module extraction (P2), daemon split (P3), config v2 (P4), leases (P6).

- [x] **T1.0 — G1 acceptance tests (red)** · *independent author* `[HUMAN]` approval
  - **Reqs:** REQ-TST-012, REQ-TST-014, REQ-TST-016
  - **Files:** `tests/acceptance/g1_*.rs` (target `acceptance`), `scripts/gate.sh` (G1 rows)
  - **Do:** a session that will **not** implement P1 writes the acceptance tests for the G1 rows below, black-box against the built binary plus the fake runtime: first SSE chunk before completion; byte-exact bodies; cancellation; 401 on every protected route; query token refused by default; malformed config → exit 2 with the diagnostic, port free; terminating by PID through the process-control module API leaves a same-named process alive (the TUI path is demonstrated in D3). Also one compositional scenario: auth + streaming + request ID + cancellation in one flow. CI runs the `acceptance` target as informational until G1.
  - **Done when:** the acceptance target is declared `test = false`; the tests compile, except tests that call a Phase 1 API the author names in `tests/acceptance/README.md`, which may fail to compile only for that reason; each test fails for the right reason against the current code (record the failure output); and the operator approves them via CODEOWNERS review (CR-7).

- [x] **T1.1 — Lockfile and toolchain**
  - **Reqs:** REQ-REPO-001, REQ-REPO-008, REQ-REL-006
  - **Files:** `.gitignore`, `Cargo.lock`, `Cargo.toml` (`rust-version`), `rust-toolchain.toml`
  - **Done when:** `git ls-files Cargo.lock` is non-empty; `grep -n "Cargo.lock" .gitignore` finds nothing; `cargo build --locked` succeeds.

- [x] **T1.2 — Repository hygiene** `[P]`
  - **Reqs:** REQ-REPO-002, REQ-REPO-003, REQ-REPO-006, REQ-DOC-003
  - **Depends:** T1.1
  - **Files:** `.gitignore`, removed artifacts, `CONTRIBUTING.md`, `SECURITY.md`, `CHANGELOG.md`
  - **Do:** `git rm` `.saltnitor_history`, `crash_dump_20260511_163653.txt`, and `.vscode/`. For `legacy.zip`: unzip it to a temp dir and diff against history (`git log --all -p -- src/`). If there is unique source, ask the operator, then tag `legacy-archive`. Then remove the zip. Extend `.gitignore`; rename the contributing guide; add stub SECURITY/CHANGELOG.
  - **Done when:** `git ls-files | grep -E 'saltnitor_history|crash_dump_|legacy\.zip|\.vscode/'` is empty, and the legacy decision is in `CHANGELOG.md`.

- [x] **T1.3 — CI pipeline**
  - **Reqs:** REQ-CI-001, REQ-CI-002, REQ-CI-003, REQ-CI-005, REQ-CI-006, REQ-CI-007, REQ-DOC-006, REQ-ARCH-008, REQ-REPO-008
  - **Depends:** T1.1, T0.9
  - **Files:** `.github/workflows/ci.yml`, `Cargo.toml` `[lints.clippy]`, `clippy.toml`, `deny.toml`, `scripts/check-test-counts.sh`
  - **Do:** jobs for fmt, clippy, test, release build (all `--locked`) on `master`/`vnext`; an MSRV job; deny-level lints; a `hardware-tests` feature; the test-count check; `cargo deny`; `spec_lint.py` plus its self-test; the informational `acceptance` job. Workflow steps that implement a CI requirement carry a `# Verifies:` comment (e.g., `# Verifies: REQ-CI-003/AC1` on the protocol-test step, `# Verifies: REQ-DOC-006/AC2` on the lint step).
  - **Done when:** CI is green on `vnext`, and a scratch branch with `todo!()` fails CI (record the run URL).

- [x] **T1.4 — Release pipeline** `[P]`
  - **Reqs:** REQ-CI-004
  - **Depends:** T1.1
  - **Files:** `.github/workflows/release.yml`
  - **Done when:** a `workflow_dispatch` dry run produces the binary and `SHA256SUMS` without `actions/upload-release-asset@v1`.

- [x] **T1.5 — Invariant scan with ratchet**
  - **Reqs:** REQ-CI-008/AC1, REQ-CI-008/AC2, REQ-CI-008/AC3, REQ-REPO-004, REQ-REPO-005, REQ-PROC-006
  - **Depends:** T1.3
  - **Files:** `scripts/check-invariants.sh`, `scripts/invariants-baseline.txt`
  - **Do:** implement every REQ-CI-008/AC1 rule with remediation messages; seed the ratchet with today's violations; fail on anything new; wire it into CI.
  - **Done when:** CI runs it; a new violation on a scratch branch fails with a remediation message.

- [x] **T1.6 — Remove hardcoded credentials and paths**
  - **Reqs:** REQ-SEC-013, REQ-REPO-004, REQ-REPO-005, REQ-DOC-004
  - **Depends:** T1.5
  - **Files:** `src/main.rs`, `examples/external-mode/launch_router.sh` (moved), `scripts/smoke_control_api.sh` (moved)
  - **Do:** add v1 keys `router_ini` (path) and `client_key_env`. The tuner refuses to apply, with a visible log line, when `router_ini` is unset. The interrogator and hot-swap use the configured key. Scripts read tokens from env and use placeholder paths. Shrink the ratchet.
  - **Not in scope:** changing the tuner's write model (P4).
  - **Done when:** `git grep -n -e 'sk-saltnitor-2026' -e '/home/laz' -- ':!docs/specs'` is empty and the ratchet file has shrunk.

- [x] **T1.7 — Strict config loading (schema v1)** · *test-first*
  - **Reqs:** REQ-CFG-001, REQ-CFG-002, REQ-CFG-003, REQ-CFG-004, REQ-CFG-005/AC2, REQ-CFG-007, REQ-TST-001
  - **Depends:** T1.3
  - **Files:** `src/config_v1.rs` (new), `src/main.rs` (`load_config`), `tests/config_strict.rs`
  - **Do:** add `--config`; XDG resolution, logged; drop the `SUDO_USER` redirection. Parse with `deny_unknown_fields` and a path-aware deserializer (e.g., `serde_path_to_error` plus the TOML span) to print `file:line:col`, the key path, expected, and found. Accept `schema_version = 1`. Validate before any listener or task starts.
  - **Done when:** `cargo test --test config_strict` covers REQ-CFG-003/AC1–AC3 (runs the binary; asserts the port is free), unknown key, missing `--config` file, and missing default file.

- [x] **T1.8 — Error envelope and mapping** · *test-first*
  - **Reqs:** REQ-ERR-001, REQ-ERR-002, REQ-ERR-003, REQ-PRX-020
  - **Depends:** T1.3
  - **Files:** `src/error.rs` (new), `src/lib.rs` (new, CR-7), `src/control_api.rs`, `src/main.rs` (imports), `tests/error_mapping.rs`
  - **Do:** `ApiError { code, message, details, request_id }` covering the Appendix B codes; one `status()`; `type` strings; `IntoResponse`; replace plain-text errors; unknown `/v1/*` → `ENDPOINT_NOT_SUPPORTED`.
  - **Done when:** a table-driven test covers every Appendix B code; chat errors are JSON envelopes.

- [ ] **T1.9 — Authentication middleware and route policy** · *test-first*
  - **Reqs:** REQ-SEC-001, REQ-SEC-002/AC3, REQ-SEC-004, REQ-SEC-005, REQ-SEC-011, REQ-SEC-015, REQ-PRX-016, REQ-TST-008
  - **Depends:** T1.8
  - **Files:** `src/auth.rs` (new), `src/control_api.rs` (router), `tests/auth_policy.rs`
  - **Do:** a policy table (Appendix A subset) plus one default-deny middleware layer; bearer-only by default; `allow_query_token` compat for `GET /v1/ensure/stream`; constant-time compare (`subtle`); rate-limited failed-auth logs; no CORS headers.
  - **Done when:** `tests/auth_policy.rs` enumerates the router's routes and asserts each outcome (REQ-SEC-001/AC2–AC3, REQ-SEC-005/AC2).

- [ ] **T1.10 — Secret sourcing and redaction**
  - **Reqs:** REQ-SEC-003/AC1, REQ-SEC-003/AC2, REQ-SEC-003/AC4, REQ-SEC-006/AC1
  - **Depends:** T1.9, T1.7
  - **Files:** `src/auth.rs`, `src/config_v1.rs`, `tests/secrets.rs`
  - **Do:** `control_token_env` / `control_token_file` (reject files readable by group or others); the literal `control_token` keeps working with a deprecation warning; a redaction layer for logs and crash dumps.
  - **Done when:** tests cover file-permission rejection, the deprecation warning, and absence of token values in the captured log output and crash dump.

- [ ] **T1.11 — Streaming proxy core** · *test-first*
  - **Reqs:** REQ-PRX-001, REQ-PRX-002, REQ-PRX-003, REQ-PRX-004, REQ-PRX-005, REQ-PRX-009, REQ-SEC-012, REQ-MIG-007/AC1, REQ-TST-008
  - **Depends:** T0.4, T1.8
  - **Read first:** BP §4.2, §14
  - **Files:** `src/proxy_stream.rs` (new), `src/control_api.rs` (`h_chat`), `tests/proxy_streaming.rs`
  - **Do:** forward the original request bytes; strip client credentials and hop-by-hop headers; stream the upstream body through (`bytes_stream()` → `Body::from_stream`); preserve status and end-to-end headers; propagate or generate `X-Request-Id`.
  - **Not in scope:** timeouts and cancellation (T1.12); leases (P6).
  - **Done when:** `cargo test --test proxy_streaming` proves REQ-PRX-002/AC2, the REQ-PRX-003 property (random chunkings), header rules, credential stripping, and request IDs; `grep -nE '\.(bytes|text|json)\(\)' src/proxy_stream.rs` finds no call on an upstream response.

- [ ] **T1.12 — Cancellation, timeouts, failures, limits** · *test-first*
  - **Reqs:** REQ-PRX-006, REQ-PRX-008, REQ-PRX-010, REQ-PRX-011, REQ-PRX-017, REQ-PRX-018, REQ-TST-002
  - **Depends:** T1.11
  - **Files:** `src/proxy_stream.rs`, `tests/proxy_failures.rs`
  - **Done when:** tests prove: the fake runtime sees the close ≤ 1 s after a client drop; 4xx/5xx pass through byte-exact; a reset before headers → 502; a hang → 504; a mid-stream crash → truncated with no `[DONE]` plus a logged `UPSTREAM_STREAM_ABORTED`; 33 MiB → 413 while 31 MiB passes; a missing `model` → 400.

- [ ] **T1.13 — PID-based process control** · *test-first*
  - **Reqs:** REQ-PROC-001, REQ-PROC-002, REQ-PROC-003, REQ-PROC-004, REQ-PROC-005, REQ-PROC-006, REQ-PROC-007, REQ-TUI-010, REQ-TST-001, REQ-TST-008
  - **Depends:** T1.3
  - **Files:** `src/process.rs` (new), `src/main.rs` (inspectors), `src/ui.rs`, `tests/process_control.rs`
  - **Do:** `ProcessInfo` per PID; GPU apps queried with `pid`; SIGTERM → wait → report; SIGKILL as a separate confirmed action; identity check (start time + UID; pidfd where available); protected targets; `killall` removed from code and preflight; inspector keys `x`/`X` and help text updated.
  - **Done when:** the test kills one of two same-named `sleep` processes by PID and the other survives; identity-mismatch and protected-PID refusals are tested; the ratchet has no kill-by-name entries.

- [ ] **T1.14 — Error surfacing and panic removal**
  - **Reqs:** REQ-ERR-004/AC1, REQ-ERR-005/AC1
  - **Depends:** T1.8
  - **Files:** `src/main.rs`, `src/control_api.rs`
  - **Done when:** a test that occupies the control port beforehand sees a visible error event; no `unwrap`/`expect` remains on fallible paths (clippy `unwrap_used` denied); journal spawn failure → a status line, not a panic.

- [ ] **T1.15 — Interrogator truthfulness**
  - **Reqs:** REQ-TUI-006, REQ-TUI-007
  - **Depends:** T1.11
  - **Files:** `src/main.rs` (interrogator), `src/app.rs`, `src/ui.rs`
  - **Do:** send interrogator traffic through Saltnitor's own `/v1/chat/completions` (not the router port) with the configured client key, so it measures the real path; parse `timings` from the final SSE chunk.
  - **Done when:** a recorded SSE transcript with `timings` renders PP/TG from it; one without renders `n/a` or `est.`; history lives in `$XDG_STATE_HOME/saltnitor/history`.

- [ ] **T1.16 — README, CHANGELOG, SECURITY truthfulness**
  - **Reqs:** REQ-DOC-001, REQ-DOC-003
  - **Depends:** T1.2, T1.6, T1.9, T1.11, T1.13, T1.15
  - **Files:** `README.md`, `CHANGELOG.md`, `SECURITY.md`, `docs/specs/vnext/evidence/claims.md`, `src/ui.rs` (tuner title)
  - **Done when:** every README feature claim has a ledger row (claim → test/evidence); BD-22/BD-30 are fixed; breaking changes are listed.

### Gate G1 — Hardening (v0.2) · BP §29 "Hardening gate"

| # | Check | Command / procedure | Pass condition | Reqs |
|---|---|---|---|---|
| 1 | CI green | CI run on `vnext` HEAD | all jobs pass | REQ-CI-001, REQ-CI-005, REQ-CI-006, REQ-CI-007 |
| 2 | Acceptance suite | `cargo test --test acceptance` (G1 tests from T1.0, unmodified) | all pass | REQ-PRX-002, REQ-PRX-003, REQ-PRX-006, REQ-SEC-001, REQ-SEC-005, REQ-CFG-003, REQ-PROC-003 |
| 3 | Compositional scenario | `cargo test --test acceptance g1_compositional` | pass | REQ-TST-014 |
| 4 | Invariant scan | `scripts/check-invariants.sh` | pass **and** `invariants-baseline.txt` is empty | REQ-CI-008 |
| 5 | Repository hygiene | `git ls-files` checks from T1.1/T1.2 | pass | REQ-REPO-001, REQ-REPO-002, REQ-REPO-003 |
| 6 | Tests not weakened | `git diff <G0 commit> -- tests/ '**/snapshots/**'` | every changed assertion, expected value, or snapshot sits in a commit that states why and carries a `Protected-change:` trailer; the operator approves the list in `evidence/G1.md` (CR-8) | REQ-TST-007 |
| 7 | Traceability | `spec_lint.py --tests --phase P1` | every P1 MUST AC with `(T)` cited by a passing test | REQ-TST-016 |
| D1 | Real streaming | binary + fake runtime `slow-stream`; `curl -N` through :8765 with `ts` timestamps | chunks arrive incrementally | REQ-PRX-002 |
| D2 | Bad config | `saltnitor --config bad.toml; echo $?` | 2 and the full diagnostic | REQ-CFG-003 |
| D3 | Process terminate in TUI | terminate a dummy process by PID; `X` asks for confirmation | as specified | REQ-PROC-003 |
| V | Independent verification and falsification | Standard gate procedure steps 2–3 | verifier ≠ implementer; rubric clean; every listed claim `NOT_FALSIFIED` | REQ-TST-013, REQ-TST-015 |

**Claims to falsify:** "streams token-by-token through Saltnitor"; "unauthenticated inference is refused when auth is configured"; "a malformed config never starts the app"; "the process sniper targets exactly one PID". **Evidence:** `evidence/G1.md`. **Tag:** `v0.2.0`.
<!-- /phase:P1 -->

---

<!-- phase:P2 -->
## Phase 2 — Module Extraction (behavior-preserving)

**Goal:** same observable behavior, different internal structure (BP §27 Ph2).
**Entry:** G1 PASSED. **Read first:** BP §4.1, §7. **Rule:** no test assertion, expected value, or snapshot may change in this phase (REQ-ARCH-007).
**Out of scope:** new features, renamed config keys, changed routes.

- [ ] **T2.0 — G2 structural checks (red)** · *independent author* `[HUMAN]` approval
  - **Reqs:** REQ-TST-012, REQ-ARCH-001, REQ-ARCH-004, REQ-ARCH-007
  - **Files:** `scripts/gate.sh` (G2 rows), `tests/acceptance/g2_structure.rs`
  - **Do:** checks for `main.rs` line count and the clippy `too_many_lines` config, plus a script that compares, between the G1 commit and HEAD, the set of test names, each test's normalized body (assertions and expected values), and snapshot contents. It tolerates tests moving between files and changed `use` lines.
  - **Done when:** the checks fail on current code for the right reason, and the operator approves.

- [ ] **T2.1 — Library skeleton**
  - **Reqs:** REQ-ARCH-002, REQ-ARCH-003
  - **Files:** `src/lib.rs`, empty modules `config`, `proxy`, `model`, `telemetry`, `service`, `process`, `tui`, `error`
  - **Do:** extend `src/lib.rs` (created in T1.8, CR-7) with the empty modules listed in Files.
  - **Done when:** it builds and every test passes.

- [ ] **T2.2 — Extract configuration**
  - **Reqs:** REQ-ARCH-003, REQ-ARCH-007
  - **Depends:** T2.1
  - **Files:** `src/config/{mod,schema,loader,validation}.rs`, `src/main.rs`
  - **Done when:** `main.rs` holds no config types; tests are unchanged and green.

- [ ] **T2.3 — Extract proxy/API and oracle**
  - **Reqs:** REQ-ARCH-003, REQ-ARCH-007
  - **Depends:** T2.2
  - **Files:** `src/proxy/{mod,routes,auth,streaming,openai,health}.rs`, `src/model/oracle.rs`
  - **Done when:** `control_api.rs` is removed or a re-export shim; tests are unchanged and green.

- [ ] **T2.4 — Extract telemetry with async discipline**
  - **Reqs:** REQ-ARCH-003, REQ-ARCH-006, REQ-ARCH-007
  - **Depends:** T2.3
  - **Files:** `src/telemetry/{mod,system,gpu,nvidia_smi}.rs`
  - **Do:** run `nvidia-smi` via `tokio::process` or `spawn_blocking`; replace the `sh -c` port audit with a direct exec or `/proc/net/tcp` parsing.
  - **Done when:** `grep -rn "std::process::Command" src/telemetry` finds nothing on async paths; tests green.

- [ ] **T2.5 — Extract service control and journal streaming**
  - **Reqs:** REQ-ARCH-003, REQ-ARCH-007
  - **Depends:** T2.4
  - **Files:** `src/service/{mod,systemd,journal}.rs`
  - **Done when:** `main.rs` holds no `systemctl`/`journalctl` invocations; tests green.

- [ ] **T2.6 — Extract process control**
  - **Reqs:** REQ-ARCH-003, REQ-ARCH-007
  - **Depends:** T2.5
  - **Files:** `src/process/**`
  - **Done when:** the TUI calls the process module only through its API; tests green.

- [ ] **T2.7 — TUI decomposition**
  - **Reqs:** REQ-ARCH-003, REQ-ARCH-004, REQ-ARCH-007
  - **Depends:** T2.6
  - **Files:** `src/tui/{app,events,state}.rs`, `src/tui/screens/*`, `src/tui/widgets/*`
  - **Do:** one input handler per mode (inspectors, tuner, console, hot-swap, help, search, dashboard) returning actions; renderers split into screens and widgets.
  - **Done when:** snapshots are unchanged and green.

- [ ] **T2.8 — Thin `main.rs` and function-size limit**
  - **Reqs:** REQ-ARCH-001, REQ-ARCH-004
  - **Depends:** T2.7
  - **Files:** `src/main.rs`, `clippy.toml`, `Cargo.toml` `[lints]`
  - **Done when:** `wc -l < src/main.rs` ≤ 150; clippy passes with `too_many_lines` denied at threshold 100.

- [ ] **T2.9 — Structured internal errors**
  - **Reqs:** REQ-ERR-004/AC1, REQ-ERR-005/AC1
  - **Depends:** T2.7
  - **Done when:** `thiserror` enums exist per module; every remaining `let _ =` has a justification comment, listed in `evidence/G2.md`.

- [ ] **T2.10 — CONTRIBUTING architecture update** `[P]`
  - **Reqs:** REQ-DOC-003
  - **Depends:** T2.8
  - **Files:** `CONTRIBUTING.md`
  - **Done when:** CONTRIBUTING describes the module map and the SV commands.

### Gate G2 — Extraction

| # | Check | Command / procedure | Pass condition | Reqs |
|---|---|---|---|---|
| 1 | CI green | CI | pass | REQ-CI-001 |
| 2 | Structural checks | `scripts/gate.sh G2` (T2.0) | pass | REQ-ARCH-001, REQ-ARCH-004 |
| 3 | Behavior preserved | T2.0 diff script against the G1 commit | only `use`/path changes | REQ-ARCH-007, REQ-TST-007 |
| 4 | Async discipline | inspection + grep | no blocking calls on async paths | REQ-ARCH-006 |
| 5 | Traceability | `spec_lint.py --tests --phase P2` | pass | REQ-TST-016 |
| D1 | Smoke | TUI with the fake runtime: hot-swap, tuner apply (temp `router_ini`), interrogator stream | behaves as at G1 | REQ-MIG-003 |
| V | Independent verification and falsification | Standard gate procedure steps 2–3 | verifier ≠ implementer; rubric clean; every listed claim `NOT_FALSIFIED` | REQ-TST-013, REQ-TST-015 |

**Claims to falsify:** "behavior is unchanged from v0.2" (re-run the G1 falsification set). **Evidence:** `evidence/G2.md`.
<!-- /phase:P2 -->

---

<!-- phase:P3 -->
## Phase 3 — Daemon / TUI / CLI Split (v0.3)

**Goal:** the daemon owns state and the runtime; the TUI and CLI are clients (BP §6, §22).
**Entry:** G2 PASSED. **Read first:** BP §5, §6, §22; TP §34.
**Out of scope:** multiple runtimes and config v2 (P4). The managed runtime here is the single configured `llama-server` with the configured preset.

- [ ] **T3.0 — G3 acceptance tests (red)** · *independent author* `[HUMAN]` approval
  - **Reqs:** REQ-TST-012, REQ-TST-014
  - **Files:** `tests/acceptance/g3_*.rs`, `scripts/gate.sh` (G3 rows)
  - **Do:** black-box tests: the daemon serves with no TTY; the TUI quits mid-stream and the stream completes; `status --json` has `daemon_state`; SIGKILL of a client leaves the daemon unchanged; SIGKILL of the daemon leaves no runtime orphan within 2 s. Compositional: daemon + fake runtime + CLI + a streaming client + TUI detach in one flow.
  - **Done when:** red for the right reasons; operator-approved.

- [ ] **T3.1 — CLI skeleton and `config check`**
  - **Reqs:** REQ-CLI-001, REQ-CLI-003, REQ-CLI-004, REQ-CLI-005, REQ-CFG-013
  - **Files:** `src/cli/**`, `src/main.rs`
  - **Do:** clap subcommands `daemon`, `tui` (default), `status`, `config check`, plus placeholders that exit 1 with "not available in this version"; global `--json`; `NO_COLOR`.
  - **Done when:** `assert_cmd` tests cover `config check` exit codes 0/2 and a placeholder exiting 1.

- [ ] **T3.2 — Daemon core and state machine** · *test-first*
  - **Reqs:** REQ-DMN-001, REQ-DMN-002, REQ-DMN-007/AC1, REQ-ARCH-005, REQ-PRX-015, REQ-ERR-004/AC2, REQ-ERR-005/AC2
  - **Depends:** T3.1
  - **Files:** `src/daemon/{mod,server,state}.rs`
  - **Done when:** the daemon-state transition table (Appendix E) is fully tested; `/v1/status` is served from daemon state; `setsid saltnitor daemon </dev/null` serves `/healthz`; a daemon started with its port occupied exits non-zero; a background task killed by fault injection is reported in log and status and restarted where safe.

- [ ] **T3.3 — Local admin transport**
  - **Reqs:** REQ-DMN-003, REQ-SEC-007/AC1, REQ-SEC-008, REQ-SEC-010
  - **Depends:** T3.2
  - **Files:** `src/daemon/admin.rs`
  - **Do:** UDS at `$XDG_RUNTIME_DIR/saltnitor/saltnitor.sock` (dir 0700, socket 0600, `SO_PEERCRED` UID check), plus `/admin/*` on loopback with an admin token; refuse UID 0 without `--allow-root`.
  - **Done when:** tests cover the modes, peer-UID rejection (the predicate unit-tested where the environment prevents a real second UID), the admin token required on loopback, and an inference key rejected on `/admin/*`.

- [ ] **T3.4 — Event stream**
  - **Reqs:** REQ-DMN-004
  - **Depends:** T3.3
  - **Done when:** a test with a stalled consumer shows the daemon keeps serving and the consumer gets `lagged`.

- [ ] **T3.5 — Managed runtime process and external mode**
  - **Reqs:** REQ-SVC-001, REQ-SVC-002, REQ-SVC-003, REQ-DMN-011, REQ-SEC-009/AC1, REQ-RT-016, REQ-PROC-008, REQ-MIG-007/AC5
  - **Depends:** T3.2
  - **Files:** `src/runtime/process.rs`, `src/service/systemd.rs`
  - **Do:** add v1 keys `runtime_process_mode` (default `managed` only when `llama_server` is set, otherwise `external` so existing installs keep working) and `llama_server` (executable path); P4's migration maps both to `[runtimes.*]`. Managed mode spawns the configured `llama-server` (configured preset, loopback bind, own process group, `PR_SET_PDEATHSIG`, log capture, health wait); external mode keeps systemd control; emergency stop works in both modes; preflight depends on the mode.
  - **Done when:** fake-runtime tests cover no orphan after daemon SIGKILL (≤ 2 s), log capture, health timeout, and emergency stop reporting VRAM before/after (fake telemetry).

- [ ] **T3.6 — TUI as daemon client**
  - **Reqs:** REQ-TUI-001, REQ-TUI-002, REQ-TUI-005, REQ-TUI-006, REQ-TUI-008, REQ-TUI-009, REQ-DMN-006, REQ-ARCH-005, REQ-NFR-003, REQ-MIG-007/AC3, REQ-MIG-007/AC4, REQ-TUI-003, REQ-DMN-007/AC2
  - **Depends:** T3.4
  - **Files:** `src/tui/**`, `src/client/**`
  - **Do:** connect UDS → loopback; render from status plus the event stream; perform every action through the admin API; OFFLINE screen with retry; keep the visuals and bindings (REQ-MIG-007/AC3–AC4). Add F-key navigation for the screens that exist now: F1 Overview (today's dashboard), F6 Processes (today's inspectors as a screen), F7 Logs. Later screens appear in their own phases; no placeholder screens.
  - **Done when:** existing snapshots pass unchanged; the new OFFLINE snapshot is human-approved; the T3.0 "TUI quits mid-stream" test passes.

- [ ] **T3.7 — CLI commands for v0.3**
  - **Reqs:** REQ-CLI-001, REQ-CLI-002, REQ-CLI-003, REQ-CLI-006, REQ-SVC-005, REQ-DMN-007/AC2
  - **Depends:** T3.3
  - **Do:** `status`, `model list`, `profile list`, `service start|stop|restart`, `process list|term|kill`, each with `--json` and exit codes.
  - **Done when:** `assert_cmd` tests cover every command and exit codes 0, 2, 3 (daemon down: prints `OFFLINE`), and 4.

- [ ] **T3.8 — Single instance, shutdown, startup residency**
  - **Reqs:** REQ-DMN-008, REQ-DMN-009, REQ-DMN-012, REQ-PRX-012, REQ-NFR-004
  - **Depends:** T3.5
  - **Done when:** tests cover the second-instance refusal (prints PID and socket); SIGTERM drain (an in-flight fake stream completes inside the grace period, the socket is removed, the child is stopped); `preload_profile` loading at startup; startup ≤ `nfr.startup_ms`.

- [ ] **T3.9 — systemd units and operations doc (v0.3)** `[P]`
  - **Reqs:** REQ-SVC-004, REQ-DOC-002, REQ-DOC-004
  - **Depends:** T3.5
  - **Files:** `examples/systemd/*.service`, `docs/operations.md`
  - **Done when:** `systemd-analyze verify` passes on both unit files; the doc covers lingering and the user-unit ordering limitation.

- [ ] **T3.10 — Disconnect resilience**
  - **Reqs:** REQ-DMN-005, REQ-DMN-006
  - **Depends:** T3.6, T3.7
  - **Done when:** tests SIGKILL a TUI, a CLI, and an SSE client mid-stream; the stream completes and the daemon state is unchanged.

- [ ] **T3.11 — Structured logging**
  - **Reqs:** REQ-DMN-010
  - **Depends:** T3.2
  - **Files:** `src/daemon/logging.rs`, call sites across `src/daemon/**`, `Cargo.toml`
  - **Done when:** `tracing` is used throughout the daemon, and the `RUST_LOG` filter is honored (a test captures filtered output).

### Gate G3 — Daemon (v0.3) · BP §29 "Daemon gate"

| # | Check | Command / procedure | Pass condition | Reqs |
|---|---|---|---|---|
| 1 | CI green | CI | pass | REQ-CI-001 |
| 2 | Acceptance suite | `cargo test --test acceptance g3_` (unmodified) | pass: TUI exits while inference continues; daemon without a terminal; CLI inspects status; survives client disconnects; no orphans | REQ-DMN-001, REQ-DMN-005, REQ-DMN-006, REQ-CLI-001, REQ-SVC-002 |
| 3 | UDS security | T3.3 tests | pass | REQ-SEC-008 |
| 4 | Traceability | `spec_lint.py --tests --phase P3` | pass | REQ-TST-016 |
| D1 | Real host `[HW]` | user unit with a real llama-server: start; TUI attach/detach; `saltnitor status` over SSH | as specified | REQ-DMN-001, REQ-SVC-004, REQ-CLI-005 |
| V | Independent verification and falsification | Standard gate procedure steps 2–3 | verifier ≠ implementer; rubric clean; every listed claim `NOT_FALSIFIED` | REQ-TST-013, REQ-TST-015 |

**Claims to falsify:** "closing the TUI never stops inference"; "the daemon runs headless under systemd"; "the CLI works over SSH". **Evidence:** `evidence/G3.md`. **Tag:** `v0.3.0`.
<!-- /phase:P3 -->

---

<!-- phase:P4 -->
## Phase 4 — Runtime Backends & Config v2 (v0.4)

**Goal:** llama.cpp builds become interchangeable, capability-described backends, and profiles become the single source of truth (BP §8–10; TP Part 1).
**Entry:** G3 PASSED. **Read first:** BP §8–10, §16, §22; TP §3–10; `requirements.md` DEC-03/04/13/17, Appendices C–D.
**Out of scope:** scheduler leases (P6: use the interim in-flight counter), NVML (P5), MoE lab features (P8).

- [ ] **T4.0 — G4 acceptance tests (red)** · *independent author* `[HUMAN]` approval
  - **Reqs:** REQ-TST-012, REQ-TST-014
  - **Files:** `tests/acceptance/g4_*.rs`, `scripts/gate.sh` (G4 rows)
  - **Do:** black-box with two fake runtimes whose fixtures differ: exact capability sets; an unsupported setting → `RUNTIME_CAPABILITY_MISSING` with **no spawn**; switch runtime A→B→A via CLI with no source change; the runtime SHA shows in `runtime list --json`; promotion refused without a validation record; migration equivalence (REQ-CFG-010/AC2). Compositional: migrate the v1 config → start → stream through runtime A → switch to B → stream again.
  - **Done when:** red for the right reasons; operator-approved.

- [ ] **T4.1 — Config schema v2** · *test-first*
  - **Reqs:** REQ-CFG-005, REQ-CFG-006, REQ-CFG-007, REQ-CFG-008, REQ-CFG-009, REQ-SEC-002, REQ-SEC-003, REQ-SEC-014, REQ-RT-002, REQ-PROF-001, REQ-PROF-008, REQ-REM-001, REQ-TST-001
  - **Files:** `src/config/schema_v2.rs`, `src/config/validation.rs`, `tests/config_v2.rs`, `examples/config/config.toml`
  - **Done when:** tests cover every validation rule (duplicate IDs across both sources, non-loopback with auth off, literal secrets, undefined env var in a path, environment-unavailable marking), and `saltnitor config check examples/config/config.toml` exits 0.

- [ ] **T4.2 — Live reload**
  - **Reqs:** REQ-CFG-011, REQ-CLI-001
  - **Depends:** T4.1
  - **Done when:** tests show SIGHUP with an invalid config keeps the old config and reports an error, and a valid config applies with pending-restart items listed; `saltnitor config reload` takes the same path as SIGHUP.

- [ ] **T4.3 — Parameter registry**
  - **Reqs:** REQ-PROF-002, REQ-PROF-006
  - **Files:** `src/model/params.rs` (data table), `tests/param_registry.rs`
  - **Do:** encode Appendix D as data, including the default state of each optimization (e.g., async CPU on by default where the capability variant says so); add `derive_baseline()`.
  - **Done when:** property tests prove REQ-PROF-006/AC1 and AC3; a unit test proves AC2 with a synthetic default-on capability variant (T4.13 re-checks it against the real Codacus fixture).

- [ ] **T4.4 — RuntimeBackend, manager, topology, hermetic env**
  - **Reqs:** REQ-RT-001, REQ-RT-009, REQ-RT-016, REQ-RT-017, REQ-RT-020, REQ-RT-021, REQ-PROF-009
  - **Depends:** T4.1, T4.3
  - **Files:** `src/runtime/{mod,backend,manager,llama_cpp,env}.rs`
  - **Done when:** fake-runtime tests cover start → health → load → unload → stop; a unit-identity change restarts (new PID); the recorded env equals the allowlist + runtime env + `process_env`, and an ambient `LLAMA_ARG_CTX_SIZE` does **not** reach the runtime.

- [ ] **T4.5 — Preset generation, router hardening, single-model mode** · *test-first*
  - **Reqs:** REQ-RT-018, REQ-RT-022, REQ-RT-009
  - **Depends:** T4.4
  - **Files:** `src/runtime/preset.rs`, `tests/golden/presets/*.ini`
  - **Done when:** golden and determinism property tests pass; the fake runtime records `--models-max 1` and `--no-models-autoload`; a chat for an unloaded model is never auto-loaded by the runtime (the fake records no autoload); `status.value` mapping is tested; single-model argv includes `--alias`.

- [ ] **T4.6 — Config migration v1 → v2**
  - **Reqs:** REQ-CFG-010, REQ-CFG-012, REQ-TST-001, REQ-MIG-007/AC7
  - **Depends:** T4.1, T4.3, T4.5
  - **Files:** `src/config/migrate.rs`, `tests/config_migrate.rs`
  - **Do:** use the same conversion in memory when a v1 file is loaded (with the REQ-CFG-005/AC1 deprecation warning), so v1 configs keep working until v1.0 (REQ-MIG-007/AC7).
  - **Done when:** `known-good.ini` plus a v1 config migrate to a valid v2, and the generated preset is semantically equal to the original (the REQ-CFG-010/AC2 property over generated INIs as well); backups exist; the write is atomic; a v1 file still starts the daemon.

- [ ] **T4.7 — Capability discovery and upstream fixtures** `[HW]` capture
  - **Reqs:** REQ-RT-003, REQ-RT-004, REQ-RT-005, REQ-TST-001
  - **Depends:** T4.4
  - **Files:** `src/runtime/capability.rs`, `tests/fixtures/runtimes/upstream/<commit>/{help,version,models.json}`
  - **Do:** parse help/version, probe binary env names, probe the API after start, apply overrides with provenance. The operator captures the fixtures from the actual upstream build, including a `/models` sample.
  - **Done when:** fixture tests assert the exact capability sets and variants; an override shows provenance `override`.

- [ ] **T4.8 — Runtime fingerprint** `[P]`
  - **Reqs:** REQ-RT-006, REQ-RT-007
  - **Depends:** T4.7
  - **Files:** `src/runtime/fingerprint.rs`
  - **Done when:** fixture tests yield the expected fields, each with its source, and `null` for unknowable ones (T4.10 surfaces them in the CLI).

- [ ] **T4.9 — Profile validation and availability** · *test-first*
  - **Reqs:** REQ-RT-008, REQ-PROF-010, REQ-PRX-014, REQ-PROF-007, REQ-CFG-006, REQ-PRX-015
  - **Depends:** T4.7
  - **Files:** `src/runtime/validate.rs`, `src/proxy/openai.rs`
  - **Done when:** `moe_cache_capacity` on the upstream fixture → `RUNTIME_CAPABILITY_MISSING` with **no spawn** recorded; a sampler param in router mode → rejected (AC3); a missing model file is absent from `/v1/models` and listed in `/v1/status.unavailable`; a phantom upstream `default` model is not listed; `serve = false` → 404.

- [ ] **T4.10 — Runtime switching and scope-aware apply**
  - **Reqs:** REQ-RT-010, REQ-PROF-003, REQ-PROF-005, REQ-PROF-011, REQ-TUI-013, REQ-CLI-001, REQ-RT-006, REQ-PROF-006/AC1
  - **Depends:** T4.5, T4.8, T4.9
  - **Do:** `runtime list|use`, `profile show|apply|derive-baseline`; an interim in-flight request counter blocks switching while requests are active (admin force aside) until P6 replaces it with leases; PROCESS-scope change → restart, MODEL-scope → reload; TUI confirmation.
  - **Not in scope:** queueing (P6).
  - **Done when:** tests show a switch waits for an active fake stream; an env change gives a new PID; a model-scope change keeps the PID; `runtime list --json` shows each runtime's commit SHA and binary-hash prefix; `profile derive-baseline <id> --as <new>` writes the derived profile.

- [ ] **T4.11 — External mode completion**
  - **Reqs:** REQ-RT-019, REQ-SVC-003
  - **Depends:** T4.7
  - **Done when:** tests show external mode spawns nothing, reports switching/env control as unavailable with a reason, and fingerprints the configured binary.

- [ ] **T4.12 — Upstream backend parity**
  - **Reqs:** REQ-RT-014, REQ-MIG-003, REQ-MIG-007/AC1, REQ-MIG-007/AC2
  - **Depends:** T4.5, T4.9, T4.6
  - **Done when:** every P0–P3 characterization, protocol, and acceptance test passes through Backend 1 with no test edits.

- [ ] **T4.13 — Codacus backend and fixtures** `[HW]`
  - **Reqs:** REQ-RT-015, REQ-RT-004, REQ-MOE-011, REQ-PROF-006
  - **Depends:** T4.7
  - **Files:** `src/runtime/codacus.rs`, `tests/fixtures/runtimes/codacus/<commit>/*`
  - **Do:** the operator builds `thecodacus/llama.cpp` `perf` into `~/ai-runtimes/codacus-testing` and captures `--help`/`--version` for `llama-server`, `llama-bench`, and `llama-moe-trace`. The agent maps capabilities **only** from names present in these fixtures — including the async-default-on variant (`--no-sched-async-cpu`) and the cache capacity unit (slots) — and records confirmed and missing names under OQ-02 in `CHANGE_REQUESTS.md`.
  - **Done when:** fixture tests assert the Codacus capability set and variants, and REQ-PROF-006/AC2 holds against the real fixture (the baseline passes `--no-sched-async-cpu` if the fixture confirms it).

- [ ] **T4.14 — Channels, validation (smoke), promotion**
  - **Reqs:** REQ-RT-011, REQ-RT-012, REQ-RT-013, REQ-CLI-001
  - **Depends:** T4.12
  - **Files:** `src/runtime/channels.rs`, `examples/runtimes/build-codacus.sh` (operator-run)
  - **Done when:** `runtime validate <id>` and `runtime promote <id>` work from the CLI against the fake runtime; tests show promotion is refused without a record (`VALIDATION_REQUIRED`), refused after the binary hash changes, and otherwise succeeds atomically with a rollback pointer; `git grep -nE '"(git|cmake|make)"' src/` finds nothing.

- [ ] **T4.15 — Tuner rework and new screens**
  - **Reqs:** REQ-TUI-003, REQ-TUI-004, REQ-TUI-009, REQ-TUI-013, REQ-CFG-012, REQ-PROF-006
  - **Depends:** T4.10
  - **Files:** `src/tui/screens/{runtime,models,tuner}.rs`
  - **Do:** F2 Runtime, F3 Models, and F4 Tuner with pages Compute / Context-KV / Speculation / Server (MoE page in P8). Each control is bound to a registry param; unsupported controls are hidden or disabled with a reason; each shows its scope label. Writes go through the daemon to the profile TOML with `toml_edit` (atomic, comments kept). The "derive baseline" action is available.
  - **Done when:** the REQ-TUI-004/AC1 property test passes; the REQ-CFG-012/AC2 round-trip property passes; new snapshots are human-approved.

- [ ] **T4.16 — Invariant scan: fork/model literals**
  - **Reqs:** REQ-CI-008/AC4
  - **Depends:** T4.13
  - **Done when:** the allowlist covers only `src/runtime/codacus.rs`, the registry data, tests/fixtures, examples, and docs; CI is green.

- [ ] **T4.17 — Docs and examples (v0.4)** `[P]`
  - **Reqs:** REQ-DOC-002, REQ-DOC-004, REQ-PROF-004
  - **Depends:** T4.14
  - **Files:** `docs/configuration.md`, `docs/runtimes.md`, `docs/upgrade.md`, `examples/profiles/{qwen36-safe,qwen36-agent,qwen36-long}.toml`
  - **Done when:** every v2 key is documented; the examples validate; `git grep -in qwen36 src/` finds nothing.

### Gate G4 — Runtime (v0.4) · BP §29 "Runtime gate"; TP §39 Phase 1

| # | Check | Command / procedure | Pass condition | Reqs |
|---|---|---|---|---|
| 1 | CI green | CI | pass | REQ-CI-001 |
| 2 | Acceptance suite | `cargo test --test acceptance g4_` (unmodified) | pass: capabilities detected correctly; unsupported settings rejected before start; runtime SHA visible; switch without source edits (fakes); promotion guarded; migration lossless | REQ-RT-003, REQ-RT-005, REQ-RT-006, REQ-RT-008, REQ-RT-010, REQ-RT-013, REQ-CFG-010 |
| 3 | Hermetic env and router hardening | T4.4/T4.5 tests | pass | REQ-RT-021, REQ-RT-022 |
| 4 | No coupling | `scripts/check-invariants.sh` | pass | REQ-CI-008 |
| 5 | Traceability | `spec_lint.py --tests --phase P4` | pass | REQ-TST-016 |
| D1 | Upstream ↔ Codacus without source changes `[HW]` | on the reference host: `runtime use upstream` → request → `runtime use codacus-stable` → request | both serve; status shows the right SHA | REQ-RT-010 |
| D2 | Tuner applies for real `[HW]` | change `ctx_size` in F4 and apply; check `/props` or logs | the value is in effect | REQ-TUI-004 |
| V | Independent verification and falsification | Standard gate procedure steps 2–3 | verifier ≠ implementer; rubric clean; every listed claim `NOT_FALSIFIED` | REQ-TST-013, REQ-TST-015 |

**Claims to falsify:** "runtimes switch without editing source"; "unsupported settings never reach the runtime"; "the tuner's controls really apply". **Evidence:** `evidence/G4.md`. **Tag:** `v0.4.0`.
<!-- /phase:P4 -->

---

<!-- phase:P5 -->
## Phase 5 — Telemetry v2 & Oracle v2

**Goal:** accurate telemetry and a memory oracle that learns from observation (BP §11–12).
**Entry:** G4 PASSED. **Read first:** BP §11–12; `requirements.md` ORC-*, TEL-*.
**Out of scope:** leases and queueing (P6), the benchmark lab (P7).

- [ ] **T5.0 — G5 acceptance tests (red)** · *independent author* `[HUMAN]` approval
  - **Reqs:** REQ-TST-012, REQ-TST-014
  - **Files:** `tests/acceptance/g5_*.rs`, `scripts/gate.sh` (G5 rows)
  - **Do:** black-box, with GPU telemetry injected through a documented replay provider (`telemetry.gpu_provider = "replay:<file>"`). This is a real diagnostics feature, not a `cfg(test)` branch. Cover: observed config persists across a daemon restart (EXACT_OBSERVED); +2 048 MB external usage flips admission; confidence and risk appear in API, CLI, and TUI; predicted vs. actual is stored. Compositional: request → load → observed → restart → same request is `EXACT_OBSERVED` → external load injected → a larger profile is rejected with full details.
  - **Done when:** red for the right reasons; operator-approved.

- [ ] **T5.1 — Telemetry providers with NVML**
  - **Reqs:** REQ-TEL-001, REQ-TEL-002, REQ-TEL-003, REQ-TEL-004, REQ-TEL-009
  - **Files:** `src/telemetry/{mod,system,gpu,nvml,nvidia_smi,replay}.rs`
  - **Do:** provider traits; NVML (dynamic load) → nvidia-smi → `unavailable`; configurable cadences; the documented `replay:<file>` provider (REQ-TEL-001/AC2).
  - **Done when:** tests with fake providers cover fallback order and cadences; with NVML absent the daemon runs with GPU `unavailable`; a test asserts no periodic subprocess while the NVML provider is active.

- [ ] **T5.2 — Runtime attribution and per-PID GPU memory**
  - **Reqs:** REQ-TEL-006, REQ-PROC-007
  - **Depends:** T5.1
  - **Done when:** fake process-table/GPU tests split runtime-tree (including router children) vs. external usage correctly, and "attribution unknown" lowers confidence.

- [ ] **T5.3 — Runtime metrics provider** `[P]`
  - **Reqs:** REQ-TEL-005
  - **Depends:** T5.1
  - **Files:** `src/telemetry/llama_metrics.rs`, `tests/fixtures/metrics/*` (captured `[HW]`)
  - **Done when:** the parsers pass on real `/metrics` and `/slots` fixtures.

- [ ] **T5.4 — SQLite storage foundation**
  - **Reqs:** REQ-BEN-002
  - **Files:** `src/storage/mod.rs`, `src/storage/migrations/`
  - **Do:** one migration per file, named with the adding task's number so versions never collide (e.g., `V0504__init.sql`, `V0505__model_hashes.sql`), and discovered automatically, so tasks that add tables touch disjoint files.
  - **Done when:** tests cover migration from empty, re-open, and WAL mode.

- [ ] **T5.5 — Model hash cache**
  - **Reqs:** REQ-ORC-003
  - **Depends:** T5.4
  - **Files:** `src/model/hash_cache.rs`, `src/storage/migrations/V0505__model_hashes.sql`, daemon start-up wiring
  - **Done when:** tests show a cache hit, recompute on change, non-blocking progress, and a usable profile while hashing is pending.

- [ ] **T5.6 — Configuration hash** `[P]` · *test-first*
  - **Reqs:** REQ-ORC-002, REQ-TST-008
  - **Depends:** T4.3
  - **Files:** `src/model/config_hash.rs`, `tests/config_hash.rs`
  - **Done when:** REQ-ORC-002/AC2 property tests pass.

- [ ] **T5.7 — Peak windows and observed store**
  - **Reqs:** REQ-TEL-008, REQ-ORC-004, REQ-ORC-014, REQ-PROF-009, REQ-RT-007
  - **Depends:** T5.1, T5.4, T5.6
  - **Done when:** a load-only observation has `stable = false`, and a completed validation sequence on the fake runtime sets `stable = true`.

- [ ] **T5.8 — GGUF parser and estimator** · *test-first*
  - **Reqs:** REQ-ORC-007
  - **Depends:** T5.4
  - **Files:** `src/model/{gguf,estimate}.rs`, `tests/fixtures/gguf/*` (small synthetic files)
  - **Done when:** synthetic dense, MoE, and hybrid-attention GGUFs give hand-computed estimates; an unknown architecture → not estimable.

- [ ] **T5.9 — Prediction hierarchy** · *test-first*
  - **Reqs:** REQ-ORC-001, REQ-ORC-008, REQ-ORC-015
  - **Depends:** T5.7, T5.8
  - **Done when:** tests cover each tier, the refusal to extrapolate (REQ-ORC-001/AC2), and UNKNOWN → reject.

- [ ] **T5.10 — Admission, risk, details, force** · *test-first*
  - **Reqs:** REQ-ORC-005, REQ-ORC-006, REQ-ORC-010, REQ-ORC-011, REQ-ORC-012, REQ-TST-001
  - **Depends:** T5.2, T5.9
  - **Done when:** REQ-ORC-005/AC3–AC5 pass (AC5 as a property); the risk bands are tested; the rejection JSON has every field; force without admin → 403.

- [ ] **T5.11 — Predicted vs. actual and display**
  - **Reqs:** REQ-ORC-009, REQ-TUI-012, REQ-TUI-003, REQ-PRX-015
  - **Depends:** T5.7, T5.10
  - **Done when:** every load stores the prediction, measurement, and error %; the F3 snapshot shows predicted, available, headroom, confidence, and risk.

- [ ] **T5.12 — Known-failed configurations**
  - **Reqs:** REQ-ORC-013
  - **Depends:** T5.10
  - **Done when:** a fake OOM marks the hash; the next admission is rejected `known_failed`; a later stable run clears it.

- [ ] **T5.13 — Oracle accuracy evaluation** `[HW][HUMAN]`
  - **Reqs:** REQ-ORC-009
  - **Depends:** T5.11
  - **Do:** on the reference host, load ≥ 3 real profiles (one dense; the Qwen3.6 MoE at two contexts) and compare the GGUF_ESTIMATE predictions with the observed peaks, then repeat to get EXACT_OBSERVED.
  - **Done when:** the per-tier errors are in `evidence/G5.md` against §6 targets.

### Gate G5 — Oracle · BP §29 "Oracle gate"

| # | Check | Command / procedure | Pass condition | Reqs |
|---|---|---|---|---|
| 1 | CI green | CI | pass | REQ-CI-001 |
| 2 | Acceptance suite | `cargo test --test acceptance g5_` (unmodified) | pass: observed configs persisted; external VRAM changes admission; confidence exposed; predicted vs. actual measured | REQ-ORC-004, REQ-ORC-005, REQ-ORC-006, REQ-ORC-009 |
| 3 | Properties | `cargo test oracle::prop` | pass | REQ-ORC-002, REQ-ORC-005 |
| 4 | Traceability | `spec_lint.py --tests --phase P5` | pass | REQ-TST-016 |
| D1 | Real external load `[HW]` | hold ≥ 2 GB VRAM with another CUDA process; query admission of a borderline profile | the decision changes | REQ-ORC-005 |
| D2 | Accuracy `[HW]` | T5.13 | within targets, or revised targets accepted in writing | REQ-ORC-009 |
| V | Independent verification and falsification | Standard gate procedure steps 2–3 | verifier ≠ implementer; rubric clean; every listed claim `NOT_FALSIFIED` | REQ-TST-013, REQ-TST-015 |

**Claims to falsify:** "Saltnitor refuses loads that would OOM given current GPU usage"; "a second load of the same config is predicted from observation". **Evidence:** `evidence/G5.md`.
<!-- /phase:P5 -->

---

<!-- phase:P6 -->
## Phase 6 — Residency Scheduler & Request Lifecycle

**Goal:** safe multi-client residency — no swap ever terminates an active request (BP §13; DEC-02).
**Entry:** G5 PASSED. **Read first:** BP §13, §14, §26; `requirements.md` SCH-*, Appendix E.
**Out of scope:** remote listeners (P9), lab runners (P7 onward use the lab lease built here).

- [ ] **T6.0 — G6 acceptance tests (red)** · *independent author* `[HUMAN]` approval
  - **Reqs:** REQ-TST-012, REQ-TST-014
  - **Files:** `tests/acceptance/g6_*.rs`, `scripts/gate.sh` (G6 rows)
  - **Do:** black-box: the BP §26 residency scenario; reject policy → 503 + `Retry-After`; queue timeout; a lab lease rejects inference with reason `lab`; force needs admin. Compositional: two authenticated streaming clients on profile A + a client requesting B + one client cancelling mid-stream → A's streams stay byte-exact, B loads only after A drains, and the cancellation releases its lease.
  - **Done when:** red for the right reasons; operator-approved.

- [ ] **T6.1 — Pure scheduler core** · *test-first*
  - **Reqs:** REQ-SCH-012, REQ-SCH-001, REQ-SCH-002, REQ-SCH-004, REQ-SCH-005, REQ-TST-008, REQ-TST-001
  - **Files:** `src/scheduler/{mod,core,lease,queue}.rs`
  - **Done when:** property tests prove REQ-SCH-002/AC1 and REQ-SCH-004/AC1, AC4, plus batching (REQ-SCH-005).

- [ ] **T6.2 — Residency state machine**
  - **Reqs:** REQ-SCH-009, REQ-TST-001
  - **Depends:** T6.1
  - **Done when:** a full transition-table test passes, including router status mapping.

- [ ] **T6.3 — Proxy integration (admission pipeline)** · *test-first*
  - **Reqs:** REQ-PRX-007, REQ-SCH-002, REQ-SCH-010, REQ-SCH-011, REQ-ORC-010, REQ-MIG-007/AC1
  - **Depends:** T6.1, T6.2
  - **Do:** replace the P4 interim counter with leases; auth → validate → resolve → lease or queue → swap (drain → oracle at swap time → explicit load) → forward → release. The TUI uses the same path. A crash releases leases.
  - **Done when:** the residency scenario passes; the crash-mid-stream test releases leases; `grep` shows no TUI code calling the runtime directly; hot-swap by model id still matches the P0 characterization.

- [ ] **T6.4 — Policies and limits**
  - **Reqs:** REQ-SCH-003, REQ-SCH-004, REQ-SCH-006
  - **Depends:** T6.3
  - **Done when:** tests cover reject + `Retry-After`, `queue_full`, `queue_timeout`, and admin-only force.

- [ ] **T6.5 — Ensure endpoints on the scheduler**
  - **Reqs:** REQ-PRX-019, REQ-SEC-005, REQ-MIG-007/AC2
  - **Depends:** T6.3
  - **Done when:** the outcomes match P0 characterization (MIG-007/AC2); the `POST` stream emits the stage sequence; `force` without admin → 403.

- [ ] **T6.6 — Request identity, history, status, metrics**
  - **Reqs:** REQ-SCH-007, REQ-PRX-013, REQ-PRX-015, REQ-SEC-006
  - **Depends:** T6.3, T5.4
  - **Done when:** every field is recorded and no body content is stored; the REQ-PRX-013/AC2 tee property passes; `/v1/status` aggregates are present; `/metrics` is authenticated when enabled.

- [ ] **T6.7 — Lab lease**
  - **Reqs:** REQ-SCH-008
  - **Depends:** T6.3
  - **Done when:** tests show the lease waits for drain, rejects inference (`lab`), and restores the prior unit on release.

- [ ] **T6.8 — Overview and API screens: requests and queue** `[P]`
  - **Reqs:** REQ-TUI-003
  - **Depends:** T6.6
  - **Files:** `src/tui/screens/{overview,api}.rs` and their snapshots
  - **Done when:** F1 shows active/queued requests and F8 shows the request log (snapshots approved).

- [ ] **T6.9 — Mutation testing** `[P]`
  - **Reqs:** REQ-TST-010
  - **Depends:** T6.1
  - **Files:** `.cargo/mutants.toml`, `tests/scheduler_mutants.rs`, `docs/specs/vnext/evidence/mutants-P6.txt`
  - **Done when:** the `cargo mutants` score for the scheduler core and oracle arithmetic is recorded in `evidence/mutants-P6.txt` (linked from `evidence/G6.md`); tests that kill surviving mutants go in `tests/scheduler_mutants.rs`.

- [ ] **T6.10 — Proxy overhead** `[P]`
  - **Reqs:** REQ-NFR-001
  - **Depends:** T6.3
  - **Files:** `tests/proxy_overhead.rs`
  - **Done when:** a test measures p95 added TTFT ≤ `proxy_overhead_p95_ms` over 200 requests.

### Gate G6 — Scheduler

| # | Check | Command / procedure | Pass condition | Reqs |
|---|---|---|---|---|
| 1 | CI green | CI | pass | REQ-CI-001 |
| 2 | Acceptance suite | `cargo test --test acceptance g6_` (unmodified) | pass | REQ-SCH-002, REQ-SCH-003, REQ-SCH-004, REQ-SCH-008, REQ-SCH-010 |
| 3 | Properties | `cargo test scheduler::prop` | pass | REQ-SCH-012 |
| 4 | Mutation score | T6.9 | ≥ `sched.mutation_score_pct` (SHOULD) | REQ-TST-010 |
| 5 | Overhead | T6.10 | ≤ budget | REQ-NFR-001 |
| 6 | Traceability | `spec_lint.py --tests --phase P6` | pass | REQ-TST-016 |
| D1 | Two real clients `[HW]` | two `curl -N` streams to profile A plus a request for profile B | B waits; A completes intact; B loads and serves | REQ-SCH-002, REQ-SCH-010 |
| V | Independent verification and falsification | Standard gate procedure steps 2–3 | verifier ≠ implementer; rubric clean; every listed claim `NOT_FALSIFIED` | REQ-TST-013, REQ-TST-015 |

**Claims to falsify:** "a model swap never cuts off an active request"; "busy responses tell the client when to retry". **Evidence:** `evidence/G6.md`. **Tag:** `v0.5.0-alpha`.
<!-- /phase:P6 -->

---

<!-- phase:P7 -->
## Phase 7 — Benchmark Laboratory

**Goal:** reproducible, stored, comparable measurements (BP §19–20; TP §21–23).
**Entry:** G6 PASSED. **Read first:** BP §19–20; TP §21–23; `requirements.md` BEN-*, Appendix F.
**Out of scope:** MoE-specific workflows (P8).

- [ ] **T7.0 — G7 acceptance tests (red)** · *independent author* `[HUMAN]` approval
  - **Reqs:** REQ-TST-012, REQ-TST-014
  - **Files:** `tests/acceptance/g7_*.rs`, `scripts/gate.sh` (G7 rows)
  - **Do:** black-box against fakes: `bench rerun` gives identical argv/env; inserting a run without identity fails; `bench compare` output equals a golden file computed from seeded records; export→import round trip; drift flagged when the fake binary hash changes. Compositional: run → rerun → compare → export → import → compare again gives an identical result.
  - **Done when:** red for the right reasons; operator-approved.

- [ ] **T7.1 — Benchmark schema**
  - **Reqs:** REQ-BEN-002
  - **Done when:** migrations create every Appendix F table, with NOT NULL identity columns.

- [ ] **T7.2 — Run identity and resolved snapshot**
  - **Reqs:** REQ-BEN-003, REQ-PROF-009, REQ-RT-007
  - **Depends:** T7.1
  - **Done when:** inserting without any identity field fails (test); the resolved snapshot includes the effective env.

- [ ] **T7.3 — Microbenchmark runner**
  - **Reqs:** REQ-BEN-001
  - **Depends:** T7.2, T6.7
  - **Files:** `src/benchmark/runner.rs`, `tests/fixtures/llama-bench/*.json` (captured `[HW]`)
  - **Done when:** the runner takes a lab lease, runs the runtime's `llama-bench` with JSON output, and parser tests pass on real fixtures.

- [ ] **T7.4 — Controlled workload runner**
  - **Reqs:** REQ-BEN-001, REQ-BEN-010
  - **Depends:** T7.2
  - **Files:** `examples/workloads/{coding,reasoning,tool-calling}.json`, `src/benchmark/workloads.rs`
  - **Done when:** runs against the fake store timings-based metrics, and a malformed fake tool call is recorded as a failure.

- [ ] **T7.5 — Repetitions and noise**
  - **Reqs:** REQ-BEN-009
  - **Depends:** T7.3
  - **Done when:** fixed-sample tests verify the median, range, and "within noise" labeling.

- [ ] **T7.6 — Comparison engine and best-of**
  - **Reqs:** REQ-BEN-004, REQ-BEN-006, REQ-BEN-007
  - **Depends:** T7.5
  - **Done when:** the golden compare test and the REQ-BEN-006/AC3 antisymmetry property pass; incomparable runs are refused; no numeric literal feeds comparison output (inspection).

- [ ] **T7.7 — Rerun** `[P]`
  - **Reqs:** REQ-BEN-005
  - **Depends:** T7.6
  - **Files:** `src/benchmark/rerun.rs`, `tests/bench_rerun.rs`
  - **Not in scope:** the CLI verb (T7.14).
  - **Done when:** the fake records identical argv/env for the original and the rerun, and drift is flagged on a hash change.

- [ ] **T7.8 — Export and import** `[P]`
  - **Reqs:** REQ-BEN-008, REQ-TST-001
  - **Depends:** T7.6
  - **Files:** `src/benchmark/export.rs`, `tests/bench_export.rs`
  - **Not in scope:** the CLI verbs (T7.14).
  - **Done when:** the REQ-BEN-008/AC1 property passes.

- [ ] **T7.9 — Agent benchmark sessions**
  - **Reqs:** REQ-BEN-011, REQ-BEN-001
  - **Depends:** T6.6, T7.2
  - **Done when:** a scripted fake-agent session aggregates correctly, including client-imported fields.

- [ ] **T7.10 — Capture and replay** `[P]`
  - **Reqs:** REQ-BEN-012, REQ-SEC-006/AC2
  - **Depends:** T7.9
  - **Files:** `src/proxy/capture.rs` (plus one hook in the proxy pipeline), `src/benchmark/replay.rs`, `tests/bench_replay.rs`
  - **Done when:** capture is off by default, files are 0600, and replay reproduces the request sequence.

- [ ] **T7.11 — Crash records**
  - **Reqs:** REQ-BEN-014, REQ-MIG-007/AC6, REQ-RT-007
  - **Depends:** T7.2
  - **Files:** `src/runtime/crash.rs`, `src/daemon/admin.rs` (crash-dump route), the TUI Ctrl+D handler, `src/storage/migrations/V0711__crash_records.sql`
  - **Done when:** a fake crash yields a complete record, and Ctrl+D writes under the data dir.

- [ ] **T7.12 — Runtime validation comparison**
  - **Reqs:** REQ-BEN-016, REQ-RT-012
  - **Depends:** T7.6, T4.14
  - **Done when:** `runtime validate` includes the comparison and fails on a configured regression (fake data).

- [ ] **T7.13 — Promotion with evidence**
  - **Reqs:** REQ-BEN-015
  - **Depends:** T7.6
  - **Done when:** promotion without evidence run IDs and criteria is refused.

- [ ] **T7.14 — Benchmark screen and CLI**
  - **Reqs:** REQ-TUI-011, REQ-CLI-001, REQ-CLI-002
  - **Depends:** T7.6
  - **Done when:** F5 snapshots are approved; `bench run|compare|history|rerun|export|import` have CLI tests.

- [ ] **T7.15 — Hardware microbenchmark smoke** `[HW]`
  - **Reqs:** REQ-TST-005
  - **Depends:** T7.3
  - **Done when:** a real `bench run` on the reference host stores ≥ 3 repetitions with full identity.

### Gate G7 — Benchmark · BP §29 "Benchmark gate"

| # | Check | Command / procedure | Pass condition | Reqs |
|---|---|---|---|---|
| 1 | CI green | CI | pass | REQ-CI-001 |
| 2 | Acceptance suite | `cargo test --test acceptance g7_` (unmodified) | pass: rerun from stored configuration; results include runtime/model fingerprints; comparison generated from records | REQ-BEN-003, REQ-BEN-005, REQ-BEN-006, REQ-BEN-008 |
| 3 | Traceability | `spec_lint.py --tests --phase P7` | pass | REQ-TST-016 |
| D1 | Real run and rerun `[HW]` | `bench run` → `bench rerun` → `bench compare` on the reference host | both runs listed with deltas; identity matches | REQ-BEN-005, REQ-BEN-006 |
| V | Independent verification and falsification | Standard gate procedure steps 2–3 | verifier ≠ implementer; rubric clean; every listed claim `NOT_FALSIFIED` | REQ-TST-013, REQ-TST-015 |

**Claims to falsify:** "every benchmark can be rerun exactly from its record"; "comparison numbers come only from stored runs". **Evidence:** `evidence/G7.md`.
<!-- /phase:P7 -->

---

<!-- phase:P8 -->
## Phase 8 — MoE / Codacus Tuning Laboratory

**Goal:** first-class, capability-driven MoE experiments — routing traces, expert cache, capacity search, MTP, context sweeps, ladders (BP §16–18; TP §8, §14–20).
**Entry:** G7 PASSED; Codacus fixtures from T4.13 exist. **Read first:** TP §8–9, §14–20; `requirements.md` MOE-*, DEC-17, Appendices C–D.
**Out of scope:** the flagship ladder run on real hardware (P10).

- [ ] **T8.0 — G8 acceptance tests (red)** · *independent author* `[HUMAN]` approval
  - **Reqs:** REQ-TST-012, REQ-TST-014
  - **Files:** `tests/acceptance/g8_*.rs`, `scripts/gate.sh` (G8 rows)
  - **Do:** black-box against the Codacus-flavored fake: the capacity tuner yields `{96, 92, 88}` (REQ-MOE-005/AC3); a PROCESS-env toggle restarts the runtime (new PID) while a MODEL param reloads (same PID); a two-variable ladder step is refused; the MoE page is hidden on the upstream fixture; a mixed-model trace merge is refused; the baseline passes `--no-sched-async-cpu`. Compositional: ladder stage 0 → stage with cache → the MTP stage triggers capacity re-validation (REQ-MOE-012).
  - **Done when:** red for the right reasons; operator-approved.

- [ ] **T8.1 — MoE tuner page**
  - **Reqs:** REQ-MOE-001, REQ-TUI-004
  - **Done when:** snapshots show the page hidden on the upstream fixture and populated on the Codacus fixture; the REQ-TUI-004/AC1 property covers the MoE controls.

- [ ] **T8.2 — Process-level env semantics**
  - **Reqs:** REQ-MOE-002, REQ-PROF-011
  - **Depends:** T8.1
  - **Done when:** toggling host registration or prefetch, after confirmation, restarts the fake with the recorded env diff.

- [ ] **T8.3 — Failure classifier** · *test-first*
  - **Reqs:** REQ-MOE-006
  - **Files:** `src/runtime/failure.rs`, `tests/fixtures/failures/*.log` (real logs captured `[HW]` where possible)
  - **Done when:** CUDA-OOM, start-failure, and health-timeout fixtures classify correctly.

- [ ] **T8.4 — Routing-trace workflow**
  - **Reqs:** REQ-MOE-003, REQ-MOE-004, REQ-CLI-001
  - **Depends:** T8.3
  - **Files:** `src/benchmark/moe_trace.rs`
  - **Do:** run the trace tool per workload under a lab lease, with the invocation confirmed by the T4.13 fixtures (`MOE_TRACE_OUT=… llama-moe-trace -m … -n 512 -p …`); merge by the documented method (concatenation); store artifacts and metadata. If the fixture contradicts the README, follow the fixture and file a change request.
  - **Done when:** an end-to-end test with a fake trace tool produces per-workload files, a merged profile, and complete metadata; a mixed-model merge is refused; `saltnitor trace <profile>` drives the workflow.

- [ ] **T8.5 — Cache capacity tuner** · *test-first*
  - **Reqs:** REQ-MOE-005, REQ-ORC-014, REQ-CLI-001
  - **Depends:** T8.3, T6.7
  - **Files:** `src/benchmark/capacity_tuner.rs`
  - **Done when:** REQ-MOE-005/AC3 passes; the `tuner.min_free_vram_mb` check is enforced; every trial is stored; the safe profile is restored after each failure; nothing is auto-promoted; `saltnitor tune capacity <profile>` drives the tuner.

- [ ] **T8.6 — MTP workflow**
  - **Reqs:** REQ-MOE-007
  - **Depends:** T8.5
  - **Done when:** fake tests record TG/PP/TTFT/VRAM delta/acceptance and the equivalence check (identical and diverging cases); a profile whose cache is not stable is refused.

- [ ] **T8.7 — Context sweep** `[P]`
  - **Reqs:** REQ-MOE-008
  - **Depends:** T8.5
  - **Files:** `src/benchmark/context_sweep.rs`, `tests/context_sweep.rs`
  - **Done when:** a sweep over `sweep.contexts` stores every metric per step.

- [ ] **T8.8 — Ladder engine with re-validation**
  - **Reqs:** REQ-MOE-009, REQ-MOE-012, REQ-PROF-006
  - **Depends:** T8.4, T8.5, T8.6, T8.7
  - **Files:** `src/benchmark/ladder.rs`, `examples/ladders/qwen36-rtx3060.toml`
  - **Done when:** a two-variable step is refused without `--allow-multi`; stage 0 equals `derive-baseline` (including `--no-sched-async-cpu`); VRAM-shifting stages trigger capacity re-validation.

- [ ] **T8.9 — Fixture refresh and confinement**
  - **Reqs:** REQ-MOE-011, REQ-CI-008/AC4
  - **Depends:** T8.8
  - **Done when:** the Codacus fixtures match the build currently in `codacus-stable`, and the invariant scan passes.

- [ ] **T8.10 — Hardware verification** `[HW][HUMAN]`
  - **Reqs:** REQ-TST-005, REQ-MOE-003, REQ-MOE-005
  - **Depends:** T8.9
  - **Do:** on the reference host with Codacus `perf`: one real trace (coding workload) and a capacity search over a small range.
  - **Done when:** the artifacts and trials are stored with full identity; the results are in `evidence/G8.md`.

### Gate G8 — MoE lab · TP §39 Phases 2–3

| # | Check | Command / procedure | Pass condition | Reqs |
|---|---|---|---|---|
| 1 | CI green | CI | pass | REQ-CI-001 |
| 2 | Acceptance suite | `cargo test --test acceptance g8_` (unmodified) | pass | REQ-MOE-001, REQ-MOE-002, REQ-MOE-005, REQ-MOE-009, REQ-MOE-012, REQ-PROF-006 |
| 3 | Traceability | `spec_lint.py --tests --phase P8` | pass | REQ-TST-016 |
| D1 | Every important Codacus parameter configurable through Saltnitor `[HW]` | walk each confirmed Codacus capability in F4 → apply → confirm argv/env reached the runtime | all applicable | REQ-MOE-001, REQ-TUI-004 |
| D2 | Real trace and capacity search `[HW]` | T8.10 | artifacts stored | REQ-MOE-003, REQ-MOE-005 |
| V | Independent verification and falsification | Standard gate procedure steps 2–3 | verifier ≠ implementer; rubric clean; every listed claim `NOT_FALSIFIED` | REQ-TST-013, REQ-TST-015 |

**Claims to falsify:** "every Codacus optimization can be toggled from Saltnitor and is really applied"; "the capacity tuner never recommends a value that failed the 32K prompt". **Evidence:** `evidence/G8.md`. **Tag:** `v0.5.0`.
<!-- /phase:P8 -->

---

<!-- phase:P9 -->
## Phase 9 — Remote Agent Serving (v0.6)

**Goal:** Saltnitor is the only, authenticated, tailnet-reachable inference endpoint, and it recovers without physical access (BP §15; TP Part 3).
**Entry:** G8 PASSED. **Read first:** BP §15, §22; TP §25–38; `requirements.md` REM-*, SVC-*, DEC-10/11.
**Out of scope:** choosing the production profile (P10). Use any stable profile here (DEC-11).

- [ ] **T9.0 — G9 acceptance tests (red)** · *independent author* `[HUMAN]` approval
  - **Reqs:** REQ-TST-012, REQ-TST-014
  - **Files:** `tests/acceptance/g9_*.rs`, `scripts/gate.sh` (G9 rows)
  - **Do:** black-box with injected interface lists: listener modes; detection failure → DEGRADED with no listener; wildcard refused; a runtime bound off loopback → remote refused; recovery scenarios (crash → backoff → production reload; OOM → fallback + known-failed; crash loop → `CRASHED`). Compositional: on a non-loopback test listener — auth required, streaming works, `/admin` returns 404, and a forced runtime crash mid-session recovers.
  - **Done when:** red for the right reasons; operator-approved.

- [ ] **T9.1 — Listener manager** · *test-first*
  - **Reqs:** REQ-REM-001, REQ-REM-002, REQ-REM-003, REQ-PRX-015
  - **Files:** `src/daemon/listeners.rs`
  - **Done when:** tests cover every mode, detection failure, periodic re-detection, and wildcard refusal.

- [ ] **T9.2 — Remote auth, admin exclusion, key labels**
  - **Reqs:** REQ-SEC-002, REQ-SEC-007, REQ-SEC-011, REQ-SEC-014, REQ-REM-007
  - **Depends:** T9.1
  - **Done when:** on a non-loopback test listener, auth is required, `/admin/*` returns 404, and labels are recorded in `request_log`.

- [ ] **T9.3 — Upstream isolation and random upstream key**
  - **Reqs:** REQ-REM-005, REQ-SEC-009
  - **Depends:** T9.1
  - **Done when:** a fake runtime bound off loopback makes remote mode refuse; the upstream key never appears in logs or status (test).

- [ ] **T9.4 — Recovery supervisor** · *test-first*
  - **Reqs:** REQ-SVC-006, REQ-SVC-007, REQ-SVC-008, REQ-SCH-011, REQ-DMN-007
  - **Depends:** T9.1
  - **Files:** `src/service/recovery.rs`
  - **Done when:** fake-runtime tests cover crash → backoff → production reload; OOM → fallback + known-failed; crash loop → `CRASHED` with no further restarts; external mode re-probes after a systemd restart.

- [ ] **T9.5 — Boot integration and operations doc** `[P]`
  - **Reqs:** REQ-SVC-004, REQ-SVC-009, REQ-REM-010, REQ-DOC-002
  - **Files:** `examples/systemd/*`, `docs/operations.md`
  - **Done when:** the doc covers the boot chain, lingering, Tailscale ordering, BIOS restore-on-AC, SSH recovery, and a post-boot checklist.

- [ ] **T9.6 — Client configurations** `[HUMAN]`
  - **Reqs:** REQ-REM-008, REQ-DOC-004
  - **Files:** `examples/clients/opencode.json`, `examples/clients/deepseek-harness.settings.yaml`
  - **Do:** write both configs against each client's **current** documentation (re-verify the formats in REQ-REM-008 at implementation time), with env-based keys.
  - **Done when:** each config was used successfully in G9 D3/D4.

- [ ] **T9.7 — Remote/API screen (F8)** `[P]`
  - **Reqs:** REQ-TUI-003
  - **Depends:** T9.2
  - **Files:** `src/tui/screens/api.rs` and its snapshots
  - **Done when:** F8 shows listeners, auth state (never the key), clients, requests, and queue (snapshots approved).

- [ ] **T9.8 — Remote drills** `[HW][HUMAN]`
  - **Reqs:** REQ-REM-007, REQ-REM-009, REQ-TST-005, REQ-NFR-005
  - **Depends:** T9.1, T9.2, T9.3, T9.4, T9.5, T9.6, T9.7
  - **Do:** from a laptop off the home LAN (e.g., phone hotspot) over Tailscale: `/healthz`; `/v1/status`; an OpenCode session with streaming and tool calls; a swap while a stream is active; an induced crash and OOM; a `soak.gate_hours` daemon soak.
  - **Done when:** `evidence/G9.md` has each drill with timestamps and outputs.

### Gate G9 — Remote (v0.6) · BP §29 "Remote gate"; TP §39 Phase 5

| # | Check | Command / procedure | Pass condition | Reqs |
|---|---|---|---|---|
| 1 | CI green | CI | pass | REQ-CI-001 |
| 2 | Acceptance suite | `cargo test --test acceptance g9_` (unmodified) | pass | REQ-REM-002, REQ-REM-003, REQ-REM-005, REQ-SVC-006, REQ-SVC-007, REQ-SVC-008 |
| 3 | Traceability | `spec_lint.py --tests --phase P9` | pass | REQ-TST-016 |
| D1 | llama-server remains localhost-only `[HW]` | `ss -ltnp` on the host | runtime on 127.0.0.1 only; Saltnitor on 127.0.0.1 + 100.x only | REQ-REM-005 |
| D2 | Work laptop connects through Tailscale `[HUMAN]` | from outside the home network | connects; unauthenticated → 401 | REQ-SEC-002, REQ-REM-007 |
| D3 | OpenCode streams tokens correctly `[HUMAN]` | visible incremental output; the request log shows streaming TTFT | pass | REQ-REM-009, REQ-PRX-002 |
| D4 | Tool calls work `[HUMAN]` | OpenCode executes a tool call from the model | pass | REQ-REM-009 |
| D5 | Swap never terminates an active request `[HUMAN]` | during a long stream, request another profile from a second client | the first completes intact; the second is served afterwards | REQ-SCH-002 |
| D6 | Recovery without physical access `[HUMAN]` | `kill -9` the runtime; induce OOM with an oversized forced profile — all over SSH | recovers to production/fallback | REQ-SVC-006, REQ-SVC-007 |
| D7 | Soak `[HW]` | `soak.gate_hours` | no leak | REQ-NFR-005 |
| V | Independent verification and falsification | Standard gate procedure steps 2–3 | verifier ≠ implementer; rubric clean; every listed claim `NOT_FALSIFIED` | REQ-TST-013, REQ-TST-015 |

**Claims to falsify:** "only Saltnitor is reachable from the tailnet"; "a remote agent streams and uses tools normally"; "crashes recover without physical access". **Evidence:** `evidence/G9.md`. **Tag:** `v0.6.0`.
<!-- /phase:P9 -->

---

<!-- phase:P10 -->
## Phase 10 — Flagship Validation (Qwen3.6-35B-A3B on RTX 3060)

**Goal:** prove the definition of success (BP §33; TP §40) with stored evidence — one variable at a time, with the production profile chosen by agent performance.
**Entry:** G9 PASSED. **Read first:** TP §11–24; BP §31, §33. **Mode:** every task after T10.0 is `[HW][HUMAN]`. The agent prepares ladder files, commands, and generators, and the operator runs them on the reference host. Numbers are never edited by hand.

- [ ] **T10.0 — Validation runbook and report generator**
  - **Reqs:** REQ-VAL-003, REQ-BEN-006, REQ-BEN-007, REQ-CLI-001
  - **Files:** `docs/validation/RUNBOOK.md`, `src/benchmark/report.rs`, `examples/ladders/qwen36-rtx3060.toml`
  - **Do:** `saltnitor bench report --ladder <id>` renders `docs/validation/qwen36-rtx3060.md` from DB records only (comparison table, best-of, deltas vs. baseline with noise labels, negative results kept). The runbook gives exact commands per stage.
  - **Done when:** the generator passes golden tests on seeded records, and the runbook is operator-reviewed.

- [ ] **T10.1 — Stage 0 baseline** `[HW][HUMAN]`
  - **Reqs:** REQ-VAL-001, REQ-PROF-006, REQ-BEN-009
  - **Depends:** T10.0
  - **Done when:** the stage-0 runs (all optimizations off, including `--no-sched-async-cpu`; 32K; ≥ 3 repetitions) are stored.

- [ ] **T10.2 — Stages 1–2: host registration, then expert prefetch** `[HW][HUMAN]`
  - **Reqs:** REQ-VAL-002, REQ-MOE-002, REQ-MOE-009
  - **Depends:** T10.1
  - **Done when:** both stages are stored, each with exactly one change from the previous stage.

- [ ] **T10.3 — Stage 3: routing profile (merged coding + reasoning + tool-calling)** `[HW][HUMAN]`
  - **Reqs:** REQ-VAL-002, REQ-MOE-003
  - **Depends:** T10.2
  - **Done when:** the merged profile artifact is stored with metadata.

- [ ] **T10.4 — Stages 4–5: expert cache on; capacity search** `[HW][HUMAN]`
  - **Reqs:** REQ-VAL-002, REQ-MOE-005
  - **Depends:** T10.3
  - **Done when:** the max loadable, max prompt-stable, and recommended values are stored with every trial.

- [ ] **T10.5 — Stage 6: async CPU/GPU split, with capacity re-validation** `[HW][HUMAN]`
  - **Reqs:** REQ-VAL-002, REQ-MOE-012
  - **Depends:** T10.4
  - **Done when:** the stage runs and the re-validated capacity are stored.

- [ ] **T10.6 — Stage 7: MTP, with capacity re-validation** `[HW][HUMAN]`
  - **Reqs:** REQ-VAL-002, REQ-MOE-007, REQ-MOE-012
  - **Depends:** T10.5
  - **Done when:** the MTP metrics (including acceptance rate and the equivalence check) and the re-validated capacity are stored.

- [ ] **T10.7 — Stage 8: context sweep 16K → 64K** `[HW][HUMAN]`
  - **Reqs:** REQ-VAL-002, REQ-MOE-008
  - **Depends:** T10.6
  - **Done when:** every sweep step is stored.

- [ ] **T10.8 — Stage 9: agent benchmarks and production selection** `[HW][HUMAN]`
  - **Reqs:** REQ-VAL-008, REQ-BEN-011, REQ-BEN-015, REQ-BEN-007
  - **Depends:** T10.7
  - **Do:** run OpenCode and DeepSeek Harness on the same fixed repository tasks for the top candidate profiles; select by agent metrics (task success, wall time, failed tool calls, reloads), not TG alone; promote with evidence.
  - **Done when:** the promotion record cites the agent-benchmark run IDs and criteria.

- [ ] **T10.9 — Stability soak** `[HW][HUMAN]`
  - **Reqs:** REQ-VAL-004, REQ-NFR-005
  - **Depends:** T10.8
  - **Done when:** ≥ 20 agent tasks or ≥ 4 h of use with 0 OOMs and 0 unrecovered crashes are recorded.

- [ ] **T10.10 — Tool-call integrity** `[HW][HUMAN]`
  - **Reqs:** REQ-VAL-005, REQ-PRX-003
  - **Depends:** T10.8
  - **Done when:** both clients executed structured tool calls, byte-exactness was spot-checked from a capture, and the validity rate is recorded.

- [ ] **T10.11 — Remote operation and recovery with the production profile** `[HW][HUMAN]`
  - **Reqs:** REQ-VAL-006, REQ-VAL-007
  - **Depends:** T10.8
  - **Done when:** the G9 D2–D6 drills are repeated with the production profile and recorded.

- [ ] **T10.12 — Flagship demonstration and validation report** `[HW][HUMAN]`
  - **Reqs:** REQ-VAL-009, REQ-VAL-003
  - **Depends:** T10.9, T10.10, T10.11
  - **Done when:** a real repository task is completed end to end; `bench report` generates the validation report; every BP §33 item is linked to a record.

### Gate G10 — Flagship validation · BP §33; TP §40

| # | Check | Command / procedure | Pass condition | Reqs |
|---|---|---|---|---|
| 1 | Ladder complete | `saltnitor bench history --ladder <id> --json` | stages 0–9 present; ≥ 3 repetitions each; one variable per step or a recorded justification; re-validations present | REQ-VAL-001, REQ-VAL-002, REQ-MOE-012 |
| 2 | Honest improvement | the generated report | improvement or its absence stated with noise labels, from records only | REQ-VAL-003 |
| 3 | Stability | T10.9 evidence | 0 OOMs, 0 unrecovered crashes | REQ-VAL-004 |
| 4 | Tool calls | T10.10 evidence | both clients pass | REQ-VAL-005 |
| 5 | Remote and recovery | T10.11 evidence | pass | REQ-VAL-006, REQ-VAL-007 |
| 6 | Production selection | the promotion record | cites agent-benchmark evidence | REQ-VAL-008 |
| 7 | Flagship demo | T10.12 evidence | every BP §33 item recorded and linked | REQ-VAL-009 |
| 8 | CI green on the validated commit | CI | pass | REQ-CI-001 |
| 9 | Traceability | `spec_lint.py --tests --phase P10` | pass | REQ-TST-016 |
| V | Independent verification and falsification | Standard gate procedure steps 2–3 | verifier ≠ implementer; rubric clean; every listed claim `NOT_FALSIFIED` | REQ-TST-013, REQ-TST-015 |

**Claims to falsify:** "the production profile beats the baseline for agent work on this machine" (true or false, as measured); "the remote agent can complete a real task". **Evidence:** `evidence/G10.md` plus the generated report.
<!-- /phase:P10 -->

---

<!-- phase:P11 -->
## Phase 11 — v1.0 Readiness

**Goal:** a stable, documented, reproducible 1.0 (BP §28).
**Entry:** G10 PASSED.

- [ ] **T11.0 — Release checklist script**
  - **Reqs:** REQ-TST-012, REQ-REL-005, REQ-CLI-001
  - **Files:** `scripts/gate.sh` (G11 rows), `docs/release-checklist.md`
  - **Done when:** `scripts/gate.sh G11` runs every automated G11 row, including the CLI-surface check (every REQ-CLI-001/AC1 command answers `--help` with exit 0 and none still reports "not available in this version"); operator-approved.

- [ ] **T11.1 — Freeze schema and API; complete docs**
  - **Reqs:** REQ-REL-001, REQ-REL-002, REQ-DOC-002
  - **Depends:** T11.0
  - **Done when:** the API reference carries a version; the config reference lists every v2 key; CHANGELOG marks the freeze.

- [ ] **T11.2 — Upgrade path test**
  - **Reqs:** REQ-REL-003, REQ-CFG-010
  - **Depends:** T11.1
  - **Done when:** a test upgrades a v0.1 config and data layout to v1.0 by following `docs/upgrade.md`.

- [ ] **T11.3 — Reproducible release**
  - **Reqs:** REQ-REL-006, REQ-CI-004
  - **Depends:** T11.1
  - **Done when:** two independent release builds give identical checksums, or documented, bounded differences; provenance attestation is attached where supported.

- [ ] **T11.4 — Full regression, hardware suite, 72 h soak** `[HW]`
  - **Reqs:** REQ-REL-005, REQ-NFR-002, REQ-NFR-005, REQ-TST-005
  - **Depends:** T11.2, T11.3
  - **Done when:** all suites are green on the release candidate, and the `soak.release_hours` soak passes REQ-NFR-005.

- [ ] **T11.5 — Claims ledger verification**
  - **Reqs:** REQ-DOC-001
  - **Depends:** T11.1
  - **Done when:** every README claim is verified by a test or evidence, and a final runtime-falsification pass over the README comes back all `NOT_FALSIFIED`.

- [ ] **T11.6 — Sustained-use window** `[HUMAN]`
  - **Reqs:** REQ-REL-004
  - **Depends:** T11.4
  - **Done when:** ≥ `release.sustained_days` of daily use pass with no open P0/P1 defects (the issue list is linked).

### Gate G11 — v1.0 · BP §28

| # | Check | Command / procedure | Pass condition | Reqs |
|---|---|---|---|---|
| 1 | Stable config schema | migration tests plus the freeze note | pass | REQ-REL-001 |
| 2 | Stable daemon API | a versioned API reference is published | pass | REQ-REL-002 |
| 3 | Documented upgrade | T11.2 | pass | REQ-REL-003 |
| 4 | Sustained use | T11.6 | pass | REQ-REL-004 |
| 5 | Regression suite | T11.4 | green, including hardware | REQ-REL-005 |
| 6 | Reproducible artifacts | T11.3 | pass | REQ-REL-006 |
| 7 | Truthful README | T11.5 | every claim `NOT_FALSIFIED` | REQ-DOC-001 |
| 8 | CI green | CI on the release commit | pass | REQ-CI-001 |
| 9 | Full traceability | `spec_lint.py --tests` (every phase) | pass | REQ-TST-016, REQ-DOC-006 |
| 10 | CLI surface complete | `scripts/gate.sh G11` (T11.0) | every REQ-CLI-001/AC1 command present | REQ-CLI-001 |
| V | Independent verification and falsification | Standard gate procedure steps 2–3 | verifier ≠ implementer; rubric clean; every listed claim `NOT_FALSIFIED` | REQ-TST-013, REQ-TST-015 |

**Claims to falsify:** every feature claim in `README.md` (T11.5); "a v0.6 install upgrades to v1.0 by following `docs/upgrade.md`". **Evidence:** `evidence/G11.md`. **Tag:** `v1.0.0`.
<!-- /phase:P11 -->

---

## Appendix A — Templates

**Evidence file** (`docs/specs/vnext/evidence/G<n>.md`)
```markdown
# Gate G<n> — <name>
- Commit: <sha> · CI run: <url> · Date: <iso8601>
- Implementer(s): <agent/session ids> · Independent verifier: <who> (must differ)
| # | Check | Command / procedure | Result | Output / link |
|---|-------|---------------------|--------|---------------|
## Anti-gaming rubric (REQ-TST-015)
- [ ] no test/snapshot/fixture edits since previous gate (or approved: <link>)
- [ ] no cfg(test)/env-var branches in production code  - [ ] no outputs keyed to test inputs
- [ ] no weakened PartialEq/Debug impls  - [ ] no stubs/placeholders  - [ ] every #[allow] justified
## Runtime falsification (REQ-TST-013/AC2)
| Claim | Procedure through the real surface | NOT_FALSIFIED / FALSIFIED | Evidence |
## SHOULD items skipped (with justification)
## Verdict: PASSED / FAILED — signed: <operator>, <date>
```

**PROGRESS.md line**
```
2026-10-01 · T1.11 · a1b2c3d · DONE · streaming via bytes_stream; surprise: axum default body limit 2 MB also hit /v1/ensure → covered by T1.12
```

**BLOCKED entry** (inline under the task, and in PROGRESS.md)
```
[!] BLOCKED: <task> — <requirement/AC> cannot be met because <evidence>. Options: <A>/<B>. Need: <decision from operator>.
```

**Change request** (`docs/specs/vnext/CHANGE_REQUESTS.md`)
```
CR-<n> · <date> · affects: <REQ/AC or task IDs> · found in: <task>
Problem: <what is wrong/infeasible, with evidence> · Proposal: <new text> · Impact: <tasks/tests>
Status: OPEN | APPROVED (<operator>, <date>) | REJECTED
```

## Appendix B — Retired (r2.1)

The `AGENTS.md` skeleton was retired on 2026-09-28. The root `CLAUDE.md` is the agent guide (REQ-DOC-005, ≤ 220 lines); edit that file directly.

## Appendix C — Coverage index (generated by `spec_lint.py --sync`; do not edit by hand)

| Area | MUST (with task) | SHOULD (with task) | MAY | Cited by a gate row |
|---|---|---|---|---|
| MIG | 7 (7) | 0 (0) | 0 | 5 |
| REPO | 6 (6) | 2 (2) | 0 | 3 |
| CI | 7 (7) | 1 (1) | 0 | 5 |
| CFG | 12 (12) | 1 (1) | 0 | 2 |
| SEC | 12 (12) | 3 (3) | 0 | 4 |
| ERR | 5 (5) | 0 (0) | 0 | 0 |
| PRX | 20 (20) | 0 (0) | 0 | 3 |
| PROC | 8 (8) | 0 (0) | 0 | 1 |
| ARCH | 7 (7) | 1 (1) | 0 | 4 |
| DMN | 11 (11) | 1 (1) | 0 | 3 |
| CLI | 6 (6) | 0 (0) | 0 | 2 |
| TUI | 13 (13) | 0 (0) | 0 | 1 |
| RT | 22 (22) | 0 (0) | 0 | 8 |
| PROF | 11 (11) | 0 (0) | 0 | 1 |
| TEL | 7 (7) | 1 (1) | 0 | 0 |
| ORC | 15 (15) | 0 (0) | 0 | 5 |
| SCH | 11 (11) | 0 (0) | 1 | 6 |
| BEN | 14 (14) | 1 (1) | 0 | 4 |
| MOE | 11 (11) | 0 (0) | 0 | 6 |
| REM | 8 (8) | 0 (0) | 0 | 5 |
| SVC | 9 (9) | 0 (0) | 0 | 5 |
| TST | 12 (12) | 1 (1) | 0 | 8 |
| DOC | 7 (7) | 0 (0) | 0 | 2 |
| NFR | 2 (2) | 3 (3) | 0 | 2 |
| VAL | 9 (9) | 0 (0) | 0 | 9 |
| REL | 6 (6) | 0 (0) | 0 | 6 |
| **Total** | **258 (258)** | **15 (15)** | **1** | **100** |

Tasks: 150 · Gates: 12 · Per-requirement task links (with AC scopes): see the `Tasks:` field of each requirement.
