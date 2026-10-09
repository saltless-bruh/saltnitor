# PROGRESS — saltnitor-vnext

One line per completed task (Appendix A format): `date · task · sha · DONE|BLOCKED · note`.
2026-09-28 · T0.1 · c7e18ed · DONE · spec pack moved to docs/specs/vnext; CR-1..3 filed and applied (r2.2); vnext CI trigger; clippy baseline (11 unique lines)
2026-09-28 · T0.2 · d55f1d8 · DONE · BEHAVIOR.md: 6 routes + 25 KeyCode tokens cited (check_baseline_docs.py inventory 31/31)
2026-09-28 · T0.3 · 89d2f4f · BLOCKED · BD-01…31 confirmed at c89f278; BD-32 needs the R0 capture of the live router (plan Task 8, after T0.4)
2026-09-28 · T0.4 · fd74253 · DONE · fake-llama-server lib+bin: scenario/faults/recorder, llama-style binary, R0 record/replay (CR-2); 39 tests
2026-09-28 · T0.3 · 73e496b · DONE · BD-32 confirmed from live /v1/models (status is an object); live router: all 5 models failed to load (exit_code 10) at capture time
2026-09-28 · T0.5 · fb14adf · DONE · 9 characterization tests (upsert, footprint, params/bpw, stage frames); mutation-checked; 2 parser observations → CR-5
2026-09-28 · T0.6 · c49bdd0 · DONE · 17 route tests on the real serve() vs fake; mutation-checked; BD-02 timing (1208 ms vs 201 ms first byte) and BD-32 probe recorded
2026-09-28 · T0.7 · 091e912 · DONE · 11 insta snapshots (text + styles), mutation-checked (title text and refusal color); snapshot approval = operator review of this commit (REQ-TUI-009/AC1)
2026-09-28 · T0.8 · 90810b5 · DONE · known-good.ini = read-only sanitized copy of the live router.ini (design D2 replaces the [HUMAN] hand-over; operator approves via this diff); readme-example.ini from README
2026-09-28 · T0.9 · 0c8ce4b · DONE · --sync is a fixed point; --tests --phase P0 exit 0 (all 10 due P0 T-ACs tagged); self-test 25/25
2026-09-28 · T0.10 · ed6420b · BLOCKED · files added; scripts/gate.sh G0 → all automated rows PASS; [HUMAN] branch protection pending (master: code-owner review; vnext: no force-push/deletion — CR-3)
2026-09-28 · T0.11 · c94c126 · DONE · 6 suites, all non-zero: saltnitor 34, fake lib 6, fake bin 1, http 19, process 7, record 6
2026-09-29 · G0 · 17bb1ca · BLOCKED · evidence drafted: all automated rows PASS, smoke PASS, R0 SHAPE-OK; pending operator: R1 [HW] run (router loads fail, exit_code 10), branch protection, verifier choice, protected-path approval, verdict
2026-09-29 · CR-4/CR-5/CR-1+ · f90a8f9 · DONE · operator approved; r2.3: BD-33/34 added (register 34/34); router 'failed exit 10' re-diagnosed as upstream b9105 cosmetic bug
2026-09-29 · G0 · b70afff · BLOCKED · verifier gaps 1–5 addressed (gate.sh protected list, 3 new pins, 36 citations, CR-6); remaining: R1 on Codacus runtime, T0.10 tick, CR-6, verdict
2026-09-29 · T0.10 · 574f46c · DONE · operator confirmed branch protection (master: code-owner review; vnext: no force-push/deletion); CR-6 approved (r2.4)
2026-09-29 · G0 · 574f46c · READY · R1 PASS on Codacus runtime; all rows confirmed; awaiting operator verdict
2026-09-29 · G0 · c155628 · PASSED · operator verdict signed (laz); Phase 0 complete — next: vnext→master PR review, then P1
2026-10-09 · T1.0 · f5febd7 · DONE · 15 tests, red for missing auth middleware/envelope (BD-03/04), buffered proxy (BD-02), config fallback to defaults (BD-01), no cancellation propagation, no saltnitor::process API (g1_process does not compile); approval = PR review
2026-10-09 · T1.1 · 22e918c · DONE · Cargo.lock tracked (BD-08 fixed); rust-version 1.95; toolchain pinned
2026-10-09 · T1.2 · ff13456 · DONE · untracked local artefacts, dropped legacy.zip (all 4 files matched history), CONTRIBUTING rename, SECURITY/CHANGELOG stubs
2026-10-09 · T1.3 · 9689c57 · DONE · fmt+clippy clean (~30 clippy errors, not the 11 in baseline; cargo fix + manual), ci.yml replaces rust.yml, deny/test-count gates; todo!() probe failed CI: https://github.com/saltless-bruh/saltnitor/actions/runs/37879968092
2026-10-09 · T1.4 · 85f8b80 · DONE · release.yml: locked build, SHA256SUMS, attestation, gh release upload; dry run https://github.com/saltless-bruh/saltnitor/actions/runs/37880510590 (first push had flow-mapping YAML error, fixed)
