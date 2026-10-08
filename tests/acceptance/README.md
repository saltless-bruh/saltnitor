# Acceptance tests (protected)

Gate acceptance tests live here, one file per gate (`g1_*.rs`, …), written by a session that does
not implement that phase (REQ-TST-012, REQ-TST-014). Implementing agents treat this directory as
read-only; changes go through a change request (REQ-DOC-007). Phase 0 has no acceptance tests —
G0 is verified by `scripts/gate.sh G0` and the evidence file.

## Declaration and invocation
The suite is one cargo test target, `acceptance` (`tests/acceptance/main.rs`, `test = false`),
so plain `cargo test` never runs it while it is red. Run it with `cargo test --test acceptance`
(informational in CI until G1; required at G1). G1 rows 2–3 call it.
