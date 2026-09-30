# Gate G<n> — <name>
- Commit: <sha> · CI run: <url> · Date: <iso8601>
- Implementer(s): <agent/session ids> · Independent verifier: <who> (must differ)

| # | Check | Command / procedure | Result | Output / link |
|---|-------|---------------------|--------|---------------|

## Real-world check (CR-2)
- R0 (read-only) vs `tests/fixtures/captures/baseline/r0`: <table from `fake-llama-server record … --baseline …`>
- R1 (`scripts/real-check.sh --label g<n> …`, operator "go" on <date>): <step table, diff vs baseline>

## Protected-path changes since the previous gate (CR-3)
<list printed by `scripts/gate.sh G<n>`> — approved: <operator>, <date>

## Anti-gaming rubric (REQ-TST-015)
- [ ] no test/snapshot/fixture edits since previous gate (or approved: <link>)
- [ ] no cfg(test)/env-var branches in production code  - [ ] no outputs keyed to test inputs
- [ ] no weakened PartialEq/Debug impls  - [ ] no stubs/placeholders  - [ ] every #[allow] justified

## Runtime falsification (REQ-TST-013/AC2)
| Claim | Procedure through the real surface | NOT_FALSIFIED / FALSIFIED | Evidence |

## SHOULD items skipped (with justification)

## Verdict: PASSED / FAILED — signed: <operator>, <date>
