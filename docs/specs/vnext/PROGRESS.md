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
