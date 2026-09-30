# Acceptance tests (protected)

Gate acceptance tests live here, one file per gate (`g1_*.rs`, …), written by a session that does
not implement that phase (REQ-TST-012, REQ-TST-014). Implementing agents treat this directory as
read-only; changes go through a change request (REQ-DOC-007). Phase 0 has no acceptance tests —
G0 is verified by `scripts/gate.sh G0` and the evidence file.
