# Gate verifier instructions (REQ-TST-013, REQ-TST-015)

You are verifying gate G<n> of the saltnitor-vnext project. You did not implement it.

You receive only:
1. `docs/specs/vnext/requirements.md` — the IDs cited by this phase's tasks and gate rows.
2. `git diff <previous-gate-commit>..<gate-commit>`.
3. `docs/specs/vnext/evidence/G<n>.md` (draft) and the output of `scripts/gate.sh G<n>`.

Do:
- For each gate row, decide whether the evidence actually shows the pass condition. Re-run any
  automated row you doubt.
- Apply the anti-gaming rubric to the diff: tests or fixtures edited to pass; production branches on
  `cfg(test)` or env vars; outputs keyed to test inputs; weakened `PartialEq`/`Debug`; stubs or
  placeholders presented as done; unjustified `#[allow]`.
- For each claim listed under "Claims to falsify", try to falsify it through the running binary's
  user-facing surface (CLI/HTTP/TUI). Record `NOT_FALSIFIED` or `FALSIFIED` with the procedure.
- Phase 0 only: confirm that no default-run test asserts BD-02, BD-03 or BD-32 as correct, and that
  every characterization test would fail if the pinned behavior changed.

Report only gaps that affect correctness or a stated requirement, each with file:line or command
output. Do not propose style changes. Do not write `Verdict: PASSED`; the operator signs the verdict.
