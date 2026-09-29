# Change requests — saltnitor-vnext

CR-1 · 2026-09-28 · affects: T0.1, REQ-CI-001 (scope only) · found in: Phase 0 design
Problem: `.github/workflows/rust.yml` triggers only on `master`, so no P0 commit on `vnext` gets CI and the Definition of Done item "CI green on the pushed commit" cannot be met until T1.3. · Proposal: in T0.1, add `vnext` to the `push` and `pull_request` branch lists; change nothing else (T1.3 still owns the CI rework). · Impact: T0.1 Files gains `.github/workflows/rust.yml`.
Status: APPROVED (operator, 2026-09-28 — design D3)
Addendum 2026-09-29 (final review): the test step also needs `--workspace`, otherwise CI runs only the root crate and skips the fake runtime's 39 tests. Applied; APPROVED (operator, 2026-09-29).

CR-2 · 2026-09-28 · affects: T0.4, T0.3, T0.10, Appendix A evidence template · found in: Phase 0 design
Problem: controlled-environment tests alone cannot show that the upgraded system still works on the real machine. · Proposal: `fake-llama-server record` (R0: read-only GETs of `/health`, `/models`, `/v1/models`, `/props`, `/slots`, `/metrics`; sanitized; liveness + shape-drift table) and `replay_from` scenarios in T0.4; `scripts/real-check.sh` (R1, `[HW]`, operator "go" per run) in T0.10; captures under `tests/fixtures/captures/`; each gate's evidence reruns R0 + R1 and diffs against `captures/baseline/`. · Impact: T0.4, T0.10 scope; G0 needs the R1 baseline.
Status: APPROVED (operator, 2026-09-28 — design D6)

CR-3 · 2026-09-28 · affects: T0.10, REQ-TST-012/AC1 (means, not wording) · found in: Phase 0 design
Problem: required code-owner reviews on `vnext` block the agent's per-task pushes (design D4), and the operator is the only code owner. · Proposal: full protection (code-owner review) on `master`; `vnext` protected against force-push and deletion only. Protected-path changes on `vnext` go in commits with a `Protected-change:` trailer; `scripts/gate.sh` lists every protected-path change since the previous gate and the operator approves the list in `evidence/G<n>.md`; CODEOWNERS review applies to the `vnext → master` PR. · Impact: T0.10 text.
Status: APPROVED (operator, 2026-09-28 — design D7)

CR-4 · 2026-09-28 · affects: `docs/specs/vnext/tools/test_spec_lint.py` (spec tooling; no requirement) · found in: T0.1
Problem: `test_ticked_task_needs_progress_entry` seeded its defect by ticking the hard-coded task `T0.1` in a copy of the live `tasks.md`. Ticking T0.1 for real (T0.1 Done-when) removed the fixture text, so the self-test failed on every run after the first completed task. · Proposal (applied by the agent, pending approval): seed on the first still-unticked task and pre-write DONE lines for tasks already ticked; the `spec_lint` check under test is unchanged and still has to fail for the seeded task and pass once its DONE line exists. · Impact: none on requirements; the self-test keeps working through P11.
Status: APPROVED (operator, 2026-09-29)

CR-5 · 2026-09-28 · affects: BD register (REQ-MIG-006), REQ-ORC-001 · found in: T0.5
Problem: two footprint-heuristic errors not in BD-01…32: `parse_params_b` reads `0.5B` as 5 B (`src/control_api.rs:435`), and `parse_bpw` sends `Q4_0`/`Q4_1` to the 5.0 default (`src/control_api.rs:443`). Details in `baseline/DEFECTS.md` → Observations. · Proposal: add them to requirements.md §5 as BD-33 and BD-34, fixed by the oracle v2 work (REQ-ORC-001, GGUF metadata instead of names). · Impact: register grows to 34; no P0 test pins either behavior.
Status: APPROVED (operator, 2026-09-29) — applied in r2.3

CR-6 · 2026-09-29 · affects: REQ-MIG-002/AC1, REQ-MIG-002/AC3 · found in: G0 verification
Problem: AC1 requires the status and body of every control-API route to be pinned for success and error cases; AC3 forbids asserting a known defect as correct. Where the error status itself is the defect (BD-28 profile listing; BD-29 507-vs-503 and plain-text chat errors), no test can satisfy both. Six default-run tests currently pin such behavior, named `pins_bd28_*` / `pins_bd29_*`, each with a comment that the pin is not an endorsement. · Proposal: amend AC3 to "IF a behavior is a known defect (§5), THEN a characterization test that pins it SHALL be named `pins_bd<nn>_…`, SHALL state in a comment that it pins rather than endorses the behavior, and SHALL be updated by the task that fixes that defect; no other characterization test SHALL assert it." · Impact: no code change; the fixing tasks for BD-28/BD-29 must rewrite the named pins.
Status: APPROVED (operator, 2026-09-29) — applied in r2.4
