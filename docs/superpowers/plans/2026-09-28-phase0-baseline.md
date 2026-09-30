# Phase 0 — Baseline Freeze & Test Harness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Pin today's Saltnitor behavior into tests, build the scenario-driven fake runtime and the real-world record/replay check, and assemble gate G0 — with zero production-behavior change.

**Architecture:** Characterization tests are `#[cfg(test)]` modules appended to existing `src/` files (the crate is binary-only). A new workspace member `tools/fake-llama-server` (lib + bin) plays llama-server from a data-driven `Scenario`; the same crate records a live router read-only (R0) and replays captures. Gate tooling (`scripts/gate.sh`, doc checkers, CODEOWNERS) makes G0 mechanically checkable.

**Tech Stack:** Rust 2024 · tokio 1.52 · axum 0.8.9 · reqwest 0.13.3 · serde/serde_json/toml · ratatui 0.30 `TestBackend` · insta 1.48 · Python 3.12 (stdlib only) · bash · GitHub Actions.

**API check:** every external API the plan's code calls was verified against the docs.rs pages for the exact `Cargo.lock` versions, indexed in context-mode as `docs:<crate>-<version>:<item>` (search with `ctx_search(source: "docs:")`): `TestBackend::{new, buffer}` and `Buffer`'s `Debug` style runs (ratatui 0.30.0), `assert_snapshot!` + `INSTA_UPDATE` modes (insta 1.48.0), `Body::from_stream`, `Router::fallback`, `body::to_bytes` (axum 0.8.9), `Response::chunk`, `RequestBuilder::bearer_auth` (reqwest 0.13.3), `ReceiverStream` (tokio-stream 0.1.18). Re-check there before changing any of these calls.

**Spec:** `docs/superpowers/specs/2026-09-28-phase0-baseline-design.md` (read it with this plan). Contract: `docs/specs/vnext/tasks.md` Phase 0 block + cited requirements (paths valid after Task 1; before that, `specs/`).

## Global Constraints

- Baseline commit `c89f278`; all work on branch `vnext`; never commit on or push `master`; never force-push; no PR without asking.
- `git diff c89f278 -- src/` may only show appended `#[cfg(test)]` modules and new files under `src/snapshots/` (REQ-MIG-003). Every existing byte of every `src/*.rs` file stays identical.
- Never run `cargo fmt` or `cargo clippy --fix` on the root crate (SV is red at baseline until T1.3). New code must add **no** clippy warnings and must be rustfmt-clean.
- No test touches the live router, the network beyond loopback, or modifies the GPU. Tests may run `nvidia-smi` read-only (design D1).
- Fakes only at process/network/hardware/clock boundaries (REQ-TST-006). Saltnitor code under test runs its real logic.
- Async code in the fake never holds a `std::sync::Mutex` guard across `.await` and never blocks the runtime (`std::thread::sleep`, blocking I/O). Blocking calls in `#[tokio::test]` bodies are allowed only on a *separate process* (reading a child's stdout, `child.wait()`).
- A characterization test never asserts a known defect as correct (REQ-MIG-002/AC3): BD-02 (buffering), BD-03 (chat/status/models are open without token), BD-32 (status-object shape) are measured by `#[ignore]` probes only.
- Every test that proves an AC carries `/// Verifies: REQ-…/ACn` on the line(s) above `#[test]`/`#[tokio::test]`.
- Live system is read-only: `~/ai-models/llama.cpp/`, `/etc/systemd/system/llama-router.service`, `~/.config/saltnitor/config.toml`, `~/ai-runtimes/*`.
- Committed fixtures contain no `/home/<user>/` paths, no bearer tokens, no prompt text.
- New crates: `insta = "1.48.0"` (dev) only. The fake reuses crate versions already in `Cargo.lock`.
- Commit format: `T0.n: summary` + blank line + `Task: T0.n` + `Refs: <IDs>` + blank line + `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Push `vnext` after each task that passes close-out.
- From Task 14 (T0.10) on, a commit touching protected paths other than `PROGRESS.md` or task checkboxes adds the trailer `Protected-change: <paths>` (CR-3).

## Review Focus

1. **Secrets and private text leaking into committed files** — recorder JSONL, R0 captures, the sanitized `router.ini`. Expected: no bearer token, no `/home/<user>/`, no prompt/content text is ever written. Pinned by: Task 5 `recorder_strips_authorization`, Task 6 `argv_and_allowlisted_env_are_recorded`, Task 7 `capture_redacts_home_prompts_and_bearer`, Task 12 grep check, gate row 4b.
2. **Tests silently depending on the host** — GPU present or not, `.saltnitor_history` in the CWD, ports in use, the operator's running Saltnitor on 8765. Expected: identical results on the reference host and on GPU-less CI. Pinned by: Task 10 rig uses zero reserves + extreme estimates and a free-port retry; Task 11 fixture overwrites history; CI (CR-1) runs every commit on ubuntu without a GPU; Task 16 smoke uses port 18765.
3. **Characterization tests enshrining defects** — BD-02, BD-03, BD-32. Expected: no default-run test asserts them. Pinned by: the "Do not assert" list in Task 10 and the `#[ignore]` probes; verifier checks it (Task 16).
4. **Vacuous characterization tests** — a test that passes whatever the code does. Expected: every group fails when the production behavior changes. Pinned by: the mutation step in Tasks 9, 10, 11 (break a line → watch the test fail → restore → prefix check).
5. **Accidental live-system mutation** — R0 pointed at the live router must never load/unload/chat. Expected: only `GET` requests. Pinned by: Task 7 `capture_issues_only_get_requests`; R1 refuses to run without `--yes`.

## Skills applied

`rust-engineer` (idiomatic async Rust, validation commands), `test-driven-development` + `writing-good-tests.md` (red-green for new code; name the break; literal expectations; no mock assertions), `test-master` (error paths, independent tests, no order dependence), `verification-before-completion` (evidence before claims; revert-must-fail pattern), `cli-developer` (fake binary: `--help`/`--version`, stderr for diagnostics, non-interactive), `python-pro` (typed stdlib scripts, pathlib, unittest), `devops-engineer` (CI trigger, CODEOWNERS, branch protection, gate script).

Found with `find-skills` and applied (read from source, not installed): **`gfargo/tui-design-skill`** (installed globally as plugin `tui-design@tui-design-marketplace`; ratatui reference: insta + `TestBackend` snapshots capture *text only* → Task 11 snapshots the buffer's `Debug` form, which also records colors/modifiers), **`wshobson/agents@bash-defensive-patterns`** (`set -Eeuo pipefail`, `command -v` preflight, input validation, `mktemp` + `trap` cleanup → Tasks 14, 15), **`wshobson/agents@rust-async-patterns`** (no blocking and no lock held across `.await` in async code → fake engine and async tests). Considered, not applied to P0: `trailofbits/skills@mutation-testing` (built around the mewt/muton tools; P0's scripted revert-must-fail check covers the need without a new tool), `trailofbits/skills@rust-review` (installed globally as plugin `rust-review@trailofbits`; security audit of unsafe/FFI/concurrency — for the verifier from P1 gates on, where auth and the streaming proxy change), `trailofbits/skills@property-based-testing` (P0 has no `[PROP]` ACs).

**Characterization ≠ TDD.** New code (fake runtime, doc checkers, gate script) is written test-first. Characterization tests pin *existing* code, so they pass on first run by design; their "red" step is a **mutation check**: temporarily break the pinned production line, watch the test fail for the expected reason, restore the line, and re-run the prefix check.

## File map

| Path | Responsibility | Task |
|---|---|---|
| `docs/specs/vnext/**` | spec pack (moved), PROGRESS, CHANGE_REQUESTS, baseline docs, evidence | 1–3, 8, 12–16 |
| `docs/specs/vnext/tools/check_baseline_docs.py` + `test_check_baseline_docs.py` | completeness checks for BEHAVIOR.md / DEFECTS.md | 2 |
| `.github/workflows/rust.yml` | add `vnext` trigger (CR-1) | 1 |
| `Cargo.toml` | `[workspace]`, dev-deps `fake-llama-server`, `insta` | 4, 11 |
| `tools/fake-llama-server/src/scenario.rs` | `Scenario`, `Fault`, `ModelsShape`, TOML + builders | 4 |
| `tools/fake-llama-server/src/recorder.rs` | JSONL/in-memory event recorder | 4 |
| `tools/fake-llama-server/src/engine.rs` | axum fallback router, model state, faults, streaming | 4–5 |
| `tools/fake-llama-server/src/lib.rs` | `spawn`, `serve_on`, `Handle` | 4 |
| `tools/fake-llama-server/src/record.rs` | R0 capture, drift, sanitize/redact, replay lookup, `record` CLI | 7 |
| `tools/fake-llama-server/src/main.rs` | llama-server-like binary: argv/env record, help/version fixtures, OOM, crash exit | 6 |
| `tools/fake-llama-server/tests/{http,process,record}.rs` | fake's own tests | 4–7 |
| `tests/fixtures/scenarios/*.toml` | scenarios used by control-API tests | 10 |
| `tests/fixtures/captures/baseline/{r0,r1.json}` | real-router baseline | 8, 15 |
| `tests/fixtures/router/*.ini` | sanitized router presets | 12 |
| `src/main.rs`, `src/control_api.rs`, `src/ui.rs` (appended test modules), `src/snapshots/*.snap` | characterization | 9–11 |
| `scripts/gate.sh`, `scripts/real-check.sh`, `.github/CODEOWNERS` | gate + R1 + protection | 14, 15 |

## Close-out procedure (run at the end of every task; referenced as **Close-out**)

Run heavy commands through `ctx_execute` (context-mode), concurrency 1.

1. Tests: `cargo test --workspace --locked 2>&1; echo EXIT=$?` → `EXIT=0`. (If `Cargo.toml` changed this task, run `cargo build --workspace` once first to refresh the local, git-ignored `Cargo.lock`.)
2. Spec lint: `python3 docs/specs/vnext/tools/spec_lint.py; echo EXIT=$?` → `spec_lint: OK`, `EXIT=0`.
3. Prefix check (no production change):
   ```bash
   python3 - <<'PY'
   import pathlib, subprocess, sys
   base = "c89f278"
   bad = []
   for f in subprocess.check_output(["git", "ls-tree", "-r", "--name-only", base, "src"], text=True).split():
       old = subprocess.check_output(["git", "show", f"{base}:{f}"])
       new = pathlib.Path(f).read_bytes()
       if not new.startswith(old):
           bad.append(f"{f}: existing bytes changed")
       elif new[len(old):].strip() and not new[len(old):].lstrip().startswith(b"#[cfg(test)]"):
           bad.append(f"{f}: appended text is not a #[cfg(test)] module")
   tracked = subprocess.check_output(["git", "ls-tree", "-r", "--name-only", base, "src"], text=True).split()
   for f in subprocess.check_output(["git", "ls-files", "--cached", "--others", "--exclude-standard", "src"], text=True).split():
       if f not in tracked and not f.startswith("src/snapshots/"):
           bad.append(f"{f}: new file outside src/snapshots/")
   print("\n".join(bad) or "prefix check: OK"); sys.exit(1 if bad else 0)
   PY
   ```
4. No new lint debt:
   ```bash
   cargo clippy --workspace --all-targets --locked --message-format short 2>&1 \
     | grep -E '^(src|tools)/[^:]+:[0-9]+:[0-9]+: (warning|error)' \
     | sed -E 's/:[0-9]+:[0-9]+:/:/' | sort -u > /tmp/claude-1000/clippy-now.txt
   comm -13 docs/specs/vnext/baseline/clippy-baseline.txt /tmp/claude-1000/clippy-now.txt
   ```
   Expected: no output. Format: `cargo fmt -p fake-llama-server --check` (when the crate exists; on a diff run `cargo fmt -p fake-llama-server` — new crate, allowed) and, for each `src/*.rs` with an appended module, check only the appended part (pass `fix` as the first argument to rewrite it in place — the baseline bytes are never touched):
   ```bash
   python3 - check <<'PY'
   import pathlib, subprocess, sys
   fix, rc = sys.argv[1:] == ["fix"], 0
   for f in ["src/main.rs", "src/control_api.rs", "src/ui.rs"]:
       old = subprocess.check_output(["git", "show", f"c89f278:{f}"])
       src = pathlib.Path(f)
       tail = src.read_bytes()[len(old):]
       if not tail.strip():
           continue
       tmp = pathlib.Path("/tmp/claude-1000") / (src.stem + "_tests.rs")
       tmp.write_bytes(tail.lstrip())
       if fix:
           subprocess.check_call(["rustfmt", "--edition", "2024", str(tmp)])
           src.write_bytes(old + b"\n" + tmp.read_bytes())
       else:
           rc |= subprocess.call(["rustfmt", "--edition", "2024", "--check", str(tmp)])
   sys.exit(rc)
   PY
   ```
   Expected: exit 0. On a diff, re-run with `python3 - fix <<'PY' … PY` (same body), then re-run the check and the prefix check.
5. PROGRESS line, format `YYYY-MM-DD · T0.n · <short sha> · DONE · <one-line note>`. Order: stage the work → `git commit` → `git rev-parse --short HEAD` → append the PROGRESS line with that sha → `git commit --amend --no-edit` (the commit is not pushed yet, so amending is safe; the amended sha differs by design — the line records the work commit before the bookkeeping amend).
6. Tick the task box in `docs/specs/vnext/tasks.md` only when its **Done when** holds (in the same amend).
7. `git push -u origin vnext` then check CI: `gh run list --branch vnext --limit 1 --json status,conclusion,headSha`. Record the run URL in the PROGRESS note of the next task if it was still running.

---

### Task 1: T0.1 — Branch, spec scaffolding, CLAUDE.md, change requests

**Files:**
- Move: `specs/{requirements.md,tasks.md,SPEC_AUDIT_REPORT.md}` → `docs/specs/vnext/`; `specs/{spec_lint.py,test_spec_lint.py}` → `docs/specs/vnext/tools/`; `specs/SALTNITOR_VNEXT_BLUEPRINT.md` and `docs/proposal/Technical Proposal — Saltnitor Qwen MoE Runtime, Tuning Laboratory, and Remote Agent Server.md` → `docs/specs/vnext/sources/`
- Create: `docs/specs/vnext/{PROGRESS.md,CHANGE_REQUESTS.md}`, `docs/specs/vnext/{baseline,evidence}/.gitkeep`, `docs/specs/vnext/baseline/clippy-baseline.txt`
- Modify: `CLAUDE.md`, `docs/specs/vnext/tasks.md` (T0.1, T0.4, T0.10 text), `docs/specs/vnext/requirements.md` (revision line, Appendix I), `.github/workflows/rust.yml`
- Commit also: `docs/superpowers/specs/2026-09-28-phase0-baseline-design.md`, `docs/superpowers/plans/2026-09-28-phase0-baseline.md`

**Interfaces:**
- Produces: all spec paths under `docs/specs/vnext/`; `docs/specs/vnext/baseline/clippy-baseline.txt` (sorted unique `path: level: message` lines) used by every Close-out.

- [ ] **Step 1: Preconditions**

Run: `git status --short; git rev-parse --short HEAD; git branch --show-current`
Expected: HEAD `c89f278`, branch `master`; untracked `CLAUDE.md`, `specs/`, `docs/`; `.vscode/web-timemanager.json` modified and `.obsidian/` untracked (both stay out of every commit).

- [ ] **Step 2: Create the branch and move the pack** (the files are untracked, so `mv` + `git add` is the equivalent of the task's `git mv`)

```bash
git switch -c vnext
mkdir -p docs/specs/vnext/{sources,baseline,evidence,tools}
mv specs/requirements.md specs/tasks.md specs/SPEC_AUDIT_REPORT.md docs/specs/vnext/
mv specs/spec_lint.py specs/test_spec_lint.py docs/specs/vnext/tools/
mv specs/SALTNITOR_VNEXT_BLUEPRINT.md docs/specs/vnext/sources/
mv "docs/proposal/Technical Proposal — Saltnitor Qwen MoE Runtime, Tuning Laboratory, and Remote Agent Server.md" docs/specs/vnext/sources/
rm -rf specs/__pycache__ && rmdir specs docs/proposal
touch docs/specs/vnext/baseline/.gitkeep docs/specs/vnext/evidence/.gitkeep
```

- [ ] **Step 3: Verify the lint still finds its files from the new location**

Run: `python3 docs/specs/vnext/tools/spec_lint.py; python3 -m unittest discover -s docs/specs/vnext/tools`
Expected: `spec_lint: OK`; unittest `OK`.

- [ ] **Step 4: Write `docs/specs/vnext/PROGRESS.md`**

```markdown
# PROGRESS — saltnitor-vnext

One line per completed task (Appendix A format): `date · task · sha · DONE|BLOCKED · note`.
```

- [ ] **Step 5: Write `docs/specs/vnext/CHANGE_REQUESTS.md`**

```markdown
# Change requests — saltnitor-vnext

CR-1 · 2026-09-28 · affects: T0.1, REQ-CI-001 (scope only) · found in: Phase 0 design
Problem: `.github/workflows/rust.yml` triggers only on `master`, so no P0 commit on `vnext` gets CI and the Definition of Done item "CI green on the pushed commit" cannot be met until T1.3. · Proposal: in T0.1, add `vnext` to the `push` and `pull_request` branch lists; change nothing else (T1.3 still owns the CI rework). · Impact: T0.1 Files gains `.github/workflows/rust.yml`.
Status: APPROVED (operator, 2026-09-28 — design D3)

CR-2 · 2026-09-28 · affects: T0.4, T0.3, T0.10, Appendix A evidence template · found in: Phase 0 design
Problem: controlled-environment tests alone cannot show that the upgraded system still works on the real machine. · Proposal: `fake-llama-server record` (R0: read-only GETs of `/health`, `/models`, `/v1/models`, `/props`, `/slots`, `/metrics`; sanitized; liveness + shape-drift table) and `replay_from` scenarios in T0.4; `scripts/real-check.sh` (R1, `[HW]`, operator "go" per run) in T0.10; captures under `tests/fixtures/captures/`; each gate's evidence reruns R0 + R1 and diffs against `captures/baseline/`. · Impact: T0.4, T0.10 scope; G0 needs the R1 baseline.
Status: APPROVED (operator, 2026-09-28 — design D6)

CR-3 · 2026-09-28 · affects: T0.10, REQ-TST-012/AC1 (means, not wording) · found in: Phase 0 design
Problem: required code-owner reviews on `vnext` block the agent's per-task pushes (design D4), and the operator is the only code owner. · Proposal: full protection (code-owner review) on `master`; `vnext` protected against force-push and deletion only. Protected-path changes on `vnext` go in commits with a `Protected-change:` trailer; `scripts/gate.sh` lists every protected-path change since the previous gate and the operator approves the list in `evidence/G<n>.md`; CODEOWNERS review applies to the `vnext → master` PR. · Impact: T0.10 text.
Status: APPROVED (operator, 2026-09-28 — design D7)
```

- [ ] **Step 6: Amend `tasks.md` in place for CR-1…CR-3** (exact edits with the Edit tool)

In T0.1, **Files** line: append `, `.github/workflows/rust.yml` (CR-1)` before the line end. **Do** line: append ` Add `vnext` to the `push`/`pull_request` branches of `rust.yml` (CR-1).`

In T0.4, **Files** line: append `, `tests/fixtures/captures/**` (CR-2)`. **Do** line: append ` Add a `record` mode (R0: read-only GETs of `/health`, `/models`, `/v1/models`, `/props`, `/slots`, `/metrics` from a live router, saved sanitized, with a liveness/shape-drift table) and `replay_from` scenarios that serve a capture byte-for-byte (CR-2).`

In T0.10, **Files** line: append `, `scripts/real-check.sh` (CR-2)`. Replace the sentence `The operator enables branch protection on `vnext`/`master` (reviews required from code owners).` with `The operator enables branch protection on `master` (reviews required from code owners) and protects `vnext` against force-push and deletion; `gate.sh` lists protected-path changes since the previous gate for operator approval (CR-3). Add `scripts/real-check.sh` (R1 real-world journey, `[HW]`, CR-2).`

- [ ] **Step 7: Amend `requirements.md`**

Line 3: `**Revision:** r2 (post-audit) · **Date:** 2026-09-27` → `**Revision:** r2.2 · **Date:** 2026-09-28`.
Append to the Appendix I table:

```markdown
| r2.2 | 2026-09-28 | CR-1…CR-3 approved (Phase 0 design): `vnext` CI trigger in T0.1; R0 record/replay in T0.4 and R1 `scripts/real-check.sh` in T0.10 with captures under `tests/fixtures/captures/`; branch protection = full on `master`, force-push/deletion guard on `vnext`, protected-path changes listed per gate. REQ-TST-012/AC1 wording unchanged. |
```

Run: `python3 docs/specs/vnext/tools/spec_lint.py` → `spec_lint: OK`.

- [ ] **Step 8: Rewrite CLAUDE.md paths** (asserts each old string exists exactly once)

```bash
python3 - <<'PY'
import pathlib, sys
p = pathlib.Path("CLAUDE.md"); t = p.read_text()
pairs = [
  ("**Spec:** `saltnitor-vnext` r2 ·", "**Spec:** `saltnitor-vnext` r2.2 ·"),
  ("| `specs/requirements.md` |", "| `docs/specs/vnext/requirements.md` |"),
  ("| `specs/tasks.md` |", "| `docs/specs/vnext/tasks.md` |"),
  ("| `specs/SALTNITOR_VNEXT_BLUEPRINT.md` —", "| `docs/specs/vnext/sources/SALTNITOR_VNEXT_BLUEPRINT.md` —"),
  ("| `docs/proposal/Technical Proposal", "| `docs/specs/vnext/sources/Technical Proposal"),
  ("| `specs/spec_lint.py` |", "| `docs/specs/vnext/tools/spec_lint.py` |"),
  ("| `specs/SPEC_AUDIT_REPORT.md` |", "| `docs/specs/vnext/SPEC_AUDIT_REPORT.md` |"),
  ("T0.1 moves the pack to `docs/specs/vnext/`, with tools in `tools/` and BP/TP in `sources/`, and rewrites the paths in this file to match. Until T0.1 is `[x]`, the paths above are correct. After that, use only `docs/specs/vnext/`.",
   "The pack lives in `docs/specs/vnext/`: tools in `tools/`, BP/TP in `sources/`, baseline docs in `baseline/`, gate evidence in `evidence/`, R0/R1 captures in `tests/fixtures/captures/`."),
  ('grep -n -A12 "^#### REQ-PRX-002" specs/requirements.md', 'grep -n -A12 "^#### REQ-PRX-002" docs/specs/vnext/requirements.md'),
  ('/p" specs/tasks.md', '/p" docs/specs/vnext/tasks.md'),
  ("python3 specs/spec_lint.py ", "python3 docs/specs/vnext/tools/spec_lint.py "),
  ("python3 -m unittest discover -s specs ", "python3 -m unittest discover -s docs/specs/vnext/tools "),
]
for old, new in pairs:
    n = t.count(old)
    if n != 1: sys.exit(f"expected 1 occurrence, found {n}: {old!r}")
    t = t.replace(old, new)
p.write_text(t)
PY
grep -nE "specs/(requirements|tasks|spec_lint|SALTNITOR)|docs/proposal" CLAUDE.md; wc -l CLAUDE.md
```
Expected: every grep hit contains `docs/specs/vnext/`; line count ≤ 220. If a pair reports a count ≠ 1, open CLAUDE.md, find the actual text, and fix that pair — never force the replacement.

- [ ] **Step 9: CR-1 — add the `vnext` trigger**

In `.github/workflows/rust.yml` replace both `branches: [ "master" ]` with `branches: [ "master", "vnext" ]`.
Run: `git diff --stat .github/workflows/rust.yml` → `1 file changed, 2 insertions(+), 2 deletions(-)`.

- [ ] **Step 10: Capture the clippy baseline**

```bash
cargo clippy --workspace --all-targets --locked --message-format short 2>&1 \
  | grep -E '^(src|tools)/[^:]+:[0-9]+:[0-9]+: (warning|error)' \
  | sed -E 's/:[0-9]+:[0-9]+:/:/' | sort -u > docs/specs/vnext/baseline/clippy-baseline.txt
wc -l docs/specs/vnext/baseline/clippy-baseline.txt
```
Expected: a non-zero count (the 32 known findings collapse to fewer unique lines once line numbers are stripped).

- [ ] **Step 11: Done-when checks**

Run: `git branch --show-current; test ! -e AGENTS.md && echo no-agents; test ! -e specs && test ! -e docs/proposal && echo moved; wc -l < CLAUDE.md`
Expected: `vnext`, `no-agents`, `moved`, a number ≤ 220.

- [ ] **Step 12: Commit and push**

```bash
git add CLAUDE.md docs/specs docs/superpowers .github/workflows/rust.yml
git commit -F - <<'MSG'
T0.1: move spec pack to docs/specs/vnext, file CR-1..3, add vnext CI trigger

Task: T0.1
Refs: REQ-REPO-007, REQ-DOC-005, REQ-DOC-007

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
MSG
```
Then Close-out steps 5–7 (PROGRESS line + tick T0.1 + amend + push). Expected CI: a run appears for `vnext` and passes (`cargo build`, `cargo test` with 0 tests).

---

### Task 2: T0.2 — Behavior inventory + completeness checker

**Files:**
- Create: `docs/specs/vnext/tools/check_baseline_docs.py`, `docs/specs/vnext/tools/test_check_baseline_docs.py`, `docs/specs/vnext/baseline/BEHAVIOR.md`

**Interfaces:**
- Produces: `check_baseline_docs.py inventory` (exit 0 when every route in `control_api::serve` and every `KeyCode::…` in `main.rs` at `c89f278` is cited) and `check_baseline_docs.py defects [--allow-blocked]` (used in Task 3, 8, 14). Python API: `routes(src) -> set[str]`, `keycodes(src) -> set[str]`, `missing_citations(doc, tokens) -> list[str]`, `check_defects(doc, allow_blocked, line_exists) -> list[str]`.

- [ ] **Step 1: Write the failing tests** — `docs/specs/vnext/tools/test_check_baseline_docs.py`

```python
"""Tests for check_baseline_docs.py (Phase 0 baseline-document completeness)."""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import check_baseline_docs as cbd  # noqa: E402

SRC_ROUTES = '''
    let app = Router::new()
        .route("/v1/ensure", post(h_ensure))
        .route("/healthz", get(h_health))
'''
SRC_KEYS = '''
    match key.code {
        KeyCode::Char('q') => {}
        KeyCode::Tab => {}
        KeyCode::F(5) => {}
    }
'''


class InventoryTest(unittest.TestCase):
    def test_routes_are_method_and_path(self):
        self.assertEqual(cbd.routes(SRC_ROUTES), {"POST /v1/ensure", "GET /healthz"})

    def test_keycodes_keep_their_payload(self):
        self.assertEqual(cbd.keycodes(SRC_KEYS), {"KeyCode::Char('q')", "KeyCode::Tab", "KeyCode::F(5)"})

    def test_token_counts_only_on_a_line_with_a_citation(self):
        doc = "| `GET /healthz` | 200 | src/control_api.rs:409 |\n| `POST /v1/ensure` | no citation |\n"
        self.assertEqual(cbd.missing_citations(doc, {"GET /healthz", "POST /v1/ensure"}), ["POST /v1/ensure"])


def entry(bd, status="confirmed — reproduced", evidence="`src/main.rs:75`"):
    return (f"### {bd} — title\n- **Evidence (c89f278):** {evidence}\n- **Repro:** run it\n"
            f"- **Status:** {status}\n- **Fixed by:**\n\n")


class DefectsTest(unittest.TestCase):
    def full(self, **over):
        return "".join(entry(f"BD-{i:02d}", **over.get(i, {})) for i in range(1, 33))

    def test_complete_register_passes(self):
        self.assertEqual(cbd.check_defects(self.full(), False, lambda p, n: True), [])

    def test_missing_entry_is_reported(self):
        doc = self.full().replace("### BD-07 — title", "### BD-99 — title")
        errs = cbd.check_defects(doc, False, lambda p, n: True)
        self.assertIn("BD-07: missing entry", errs)
        self.assertIn("BD-99: not in the BD-01…BD-32 register", errs)

    def test_citation_must_exist_at_baseline(self):
        errs = cbd.check_defects(self.full(), False, lambda p, n: n < 75)
        self.assertTrue(any(e.startswith("BD-01: src/main.rs:75 does not exist") for e in errs), errs)

    def test_blocked_fails_unless_allowed(self):
        doc = self.full(**{32: {"status": "BLOCKED — needs R0 capture"}})
        self.assertEqual(cbd.check_defects(doc, False, lambda p, n: True), ["BD-32: still BLOCKED"])
        self.assertEqual(cbd.check_defects(doc, True, lambda p, n: True), [])

    def test_git_command_is_valid_evidence(self):
        doc = self.full(**{9: {"evidence": "`git ls-tree -r --name-only c89f278 | grep legacy.zip`"}})
        self.assertEqual(cbd.check_defects(doc, False, lambda p, n: True), [])

    def test_status_needs_a_reason(self):
        doc = self.full(**{3: {"status": "confirmed"}})
        self.assertIn("BD-03: Status must be 'confirmed|disputed|BLOCKED — <reason>'",
                      cbd.check_defects(doc, False, lambda p, n: True))


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run to verify it fails**

Run: `python3 -m unittest discover -s docs/specs/vnext/tools -p 'test_check_*.py'`
Expected: `ModuleNotFoundError: No module named 'check_baseline_docs'`.

- [ ] **Step 3: Implement** — `docs/specs/vnext/tools/check_baseline_docs.py`

```python
#!/usr/bin/env python3
"""Completeness checks for the Phase 0 baseline documents.

  check_baseline_docs.py inventory                 BEHAVIOR.md cites every route and KeyCode arm
  check_baseline_docs.py defects [--allow-blocked] DEFECTS.md has 32 complete entries

Source files are read at the baseline commit, so appended test modules never count.
"""
from __future__ import annotations

import argparse
import functools
import re
import subprocess
import sys
from pathlib import Path
from typing import Callable

BASELINE = "c89f278"
ROUTE_RE = re.compile(r'\.route\(\s*"([^"]+)"\s*,\s*(get|post|put|delete|patch)\(')
KEY_RE = re.compile(r"KeyCode::(Char\('(?:\\.|[^'])'\)|F\(\d+\)|[A-Za-z]+)")
CITE_RE = re.compile(r"src/[\w/]+\.rs:\d+")
BD_HEAD = re.compile(r"^### (BD-\d{2}) — .+$", re.M)
PATH_LINE = re.compile(r"`?([\w./-]+\.[\w]+|\.gitignore):(\d+)")


def routes(src: str) -> set[str]:
    return {f"{m.group(2).upper()} {m.group(1)}" for m in ROUTE_RE.finditer(src)}


def keycodes(src: str) -> set[str]:
    return {f"KeyCode::{m.group(1)}" for m in KEY_RE.finditer(src)}


def missing_citations(doc: str, tokens: set[str]) -> list[str]:
    cited = [line for line in doc.splitlines() if CITE_RE.search(line)]
    return sorted(t for t in tokens if not any(f"`{t}`" in line for line in cited))


def check_defects(doc: str, allow_blocked: bool, line_exists: Callable[[str, int], bool]) -> list[str]:
    parts = BD_HEAD.split(doc)
    entries = dict(zip(parts[1::2], parts[2::2]))
    expected = [f"BD-{i:02d}" for i in range(1, 33)]
    errors: list[str] = []
    for bd in expected:
        body = entries.get(bd)
        if body is None:
            errors.append(f"{bd}: missing entry")
            continue
        ev = re.search(r"^- \*\*Evidence \(c89f278\):\*\* (.+)$", body, re.M)
        if not ev:
            errors.append(f"{bd}: missing '- **Evidence (c89f278):**' line")
        else:
            cites = PATH_LINE.findall(ev.group(1))
            if not cites and "`git " not in ev.group(1):
                errors.append(f"{bd}: Evidence needs a `path:line` citation or a `git …` command")
            for path, line in cites:
                if not line_exists(path, int(line)):
                    errors.append(f"{bd}: {path}:{line} does not exist at {BASELINE}")
        if not re.search(r"^- \*\*Repro:\*\* \S", body, re.M):
            errors.append(f"{bd}: missing Repro")
        st = re.search(r"^- \*\*Status:\*\* (confirmed|disputed|BLOCKED) — \S", body, re.M)
        if not st:
            errors.append(f"{bd}: Status must be 'confirmed|disputed|BLOCKED — <reason>'")
        elif st.group(1) == "BLOCKED" and not allow_blocked:
            errors.append(f"{bd}: still BLOCKED")
        if not re.search(r"^- \*\*Fixed by:\*\*", body, re.M):
            errors.append(f"{bd}: missing 'Fixed by:' line")
    errors += [f"{bd}: not in the BD-01…BD-32 register" for bd in sorted(set(entries) - set(expected))]
    return errors


@functools.lru_cache(maxsize=None)
def _baseline_lines(path: str) -> int:
    try:
        text = subprocess.check_output(["git", "show", f"{BASELINE}:{path}"], stderr=subprocess.DEVNULL)
    except subprocess.CalledProcessError:
        return 0
    return text.count(b"\n") + (0 if text.endswith(b"\n") else 1)


def _git_show(path: str) -> str:
    return subprocess.check_output(["git", "show", f"{BASELINE}:{path}"], text=True)


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("inventory")
    d = sub.add_parser("defects")
    d.add_argument("--allow-blocked", action="store_true")
    a = ap.parse_args(argv)
    root = Path(subprocess.check_output(["git", "rev-parse", "--show-toplevel"], text=True).strip())
    base = root / "docs/specs/vnext/baseline"
    if a.cmd == "inventory":
        tokens = routes(_git_show("src/control_api.rs")) | keycodes(_git_show("src/main.rs"))
        missing = missing_citations((base / "BEHAVIOR.md").read_text(encoding="utf-8"), tokens)
        for t in missing:
            print(f"BEHAVIOR.md: `{t}` is not cited on a line with a src/…:line citation")
        print(f"inventory: {len(tokens) - len(missing)}/{len(tokens)} cited")
        return 1 if missing else 0
    errors = check_defects((base / "DEFECTS.md").read_text(encoding="utf-8"), a.allow_blocked,
                           lambda p, n: 0 < n <= _baseline_lines(p))
    print("\n".join(errors) or "defects: 32/32 complete")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `python3 -m unittest discover -s docs/specs/vnext/tools`
Expected: all tests `OK` (spec_lint self-test + the 9 new ones).

- [ ] **Step 5: List what BEHAVIOR.md must cover**

Run: `python3 docs/specs/vnext/tools/check_baseline_docs.py inventory` (fails — BEHAVIOR.md absent → FileNotFoundError is fine). Then list the tokens:
```bash
python3 -c "
import sys; sys.path.insert(0,'docs/specs/vnext/tools'); import check_baseline_docs as c
print(sorted(c.routes(c._git_show('src/control_api.rs')))); print(sorted(c.keycodes(c._git_show('src/main.rs'))))"
```
Expected: 6 routes (`GET /healthz`, `GET /v1/ensure/stream`, `GET /v1/models`, `GET /v1/status`, `POST /v1/chat/completions`, `POST /v1/ensure`) and every `KeyCode` token.

- [ ] **Step 6: Write `docs/specs/vnext/baseline/BEHAVIOR.md`**

Read `src/main.rs`, `src/control_api.rs`, `src/app.rs`, `src/ui.rs` at `c89f278` (`git show c89f278:<path>`; use `ctx_execute_file`/`grep -n`, not whole-file reads). Structure — every row cites `src/<file>.rs:<line>`:

```markdown
# Behavior inventory — Saltnitor @ c89f278

Scope: observable behavior before vNext. Line numbers are at c89f278. REQ-MIG-001.

## CLI flags (`Cli`, src/main.rs:29)
| Flag | Type | Default / precedence | Citation |
|---|---|---|---|
| `--port`, `-p` | u16 | CLI > `port` in config > 8080 | src/main.rs:31, src/main.rs:150 |
| `--host` | string | CLI > config > `127.0.0.1` | src/main.rs:34, src/main.rs:151 |
| `--service-name`, `-s` | string | CLI > config > `llama-router` | src/main.rs:37, src/main.rs:155 |

## Config keys (`TomlConfig`, `ProfileMeta`)
| Key | Type | Default | Used by | Citation |
(one row per field of TomlConfig and ProfileMeta; include the load path rule: `$SUDO_USER` → `/home/$SUDO_USER/.config/saltnitor/config.toml`, else `$HOME/.config/…`; parse errors → defaults, BD-01)

## Keybindings
| Key token | Context (mode/popup) | Action | Citation |
| `KeyCode::Char('q')` | … | … | src/main.rs:NNN |
(one row per KeyCode token from Step 5 — a token used in several contexts gets one row per context)

## Control-API routes
| Route | Auth | Status codes | Body shape | Citation |
| `POST /v1/ensure` | Bearer if `control_token` set | 200 loaded/already_resident, 400, 401, 503, 507 | EnsureResponse {…} | src/control_api.rs:404, src/control_api.rs:283 |
(all 6 routes)

## Files read / written
| Path | R/W | When | Citation |
(config.toml, .saltnitor_history (CWD), router.ini hardcoded path, crash dumps, /proc/meminfo)

## External commands
| Command | Purpose | Sync/async | Citation |
(nvidia-smi ×n, journalctl -u <svc> -f, ss, systemctl via sudo -n, killall, sh -c command -v …)
```

- [ ] **Step 7: Verify completeness**

Run: `python3 docs/specs/vnext/tools/check_baseline_docs.py inventory`
Expected: `inventory: N/N cited`, exit 0.

- [ ] **Step 8: Commit** — Close-out 1–4, then:

```bash
git add docs/specs/vnext/tools/check_baseline_docs.py docs/specs/vnext/tools/test_check_baseline_docs.py docs/specs/vnext/baseline/BEHAVIOR.md
git commit -F - <<'MSG'
T0.2: behavior inventory with route/keybinding completeness check

Task: T0.2
Refs: REQ-MIG-001

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
MSG
```
Close-out 5–7.

---

### Task 3: T0.3 — Defect register (BD-01…BD-31; BD-32 blocked)

**Files:**
- Create: `docs/specs/vnext/baseline/DEFECTS.md`

**Interfaces:**
- Consumes: `check_baseline_docs.py defects [--allow-blocked]` (Task 2).
- Produces: `DEFECTS.md` entries `### BD-nn — <title>` that Tasks 8 and 10 update (BD-32 status, BD-02 timing).

- [ ] **Step 1: Get the register source**

Run: `grep -nE '^\| *BD-[0-9]+' docs/specs/vnext/requirements.md` (use `ctx_execute`). Each row gives title, cited lines, and the fixing requirement.

- [ ] **Step 2: Re-verify each defect at `c89f278`**

For each BD, open the cited lines with `git show c89f278:<path> | sed -n '<a>,<b>p'` and confirm the defect is visible there. Where the register cites a bare filename (`main.rs:75`), write the repo path (`src/main.rs:75`) — the checker verifies the line exists at the baseline.

- [ ] **Step 3: Write `DEFECTS.md`** — one entry per BD in this exact shape:

```markdown
# Defect register — Saltnitor @ c89f278

Re-verified 2026-09-28. `Fixed by:` is filled when the fixing task lands. REQ-MIG-006.

### BD-01 — Malformed config silently becomes defaults
- **Evidence (c89f278):** `src/main.rs:75` — `toml::from_str(&content).unwrap_or_default()`
- **Repro:** write `port = "x"` to a temp `$HOME/.config/saltnitor/config.toml`, run `HOME=<tmp> cargo run` → starts on 8080 with no diagnostic.
- **Status:** confirmed — the parse error is discarded on the cited line.
- **Fixed by:**

### BD-02 — The proxy buffers the whole upstream body
- **Evidence (c89f278):** `src/control_api.rs:388` — `match resp.bytes().await {`
- **Repro:** `cargo test bd02_first_byte_timing -- --ignored --nocapture` (added in T0.6) prints first-byte times direct vs through Saltnitor.
- **Status:** confirmed — the whole body is awaited before the response is built; timing evidence appended by T0.6.
- **Fixed by:**
```

For BD-32 write `- **Status:** BLOCKED — needs a real `/v1/models` sample; captured by R0 after T0.4 (Task 8).` Keep the `Repro` line: `fake-llama-server record --upstream http://127.0.0.1:8080 --out tests/fixtures/captures/baseline/r0`. A defect you cannot see at the cited lines is `disputed — <what the code actually does>`.

- [ ] **Step 4: Check**

Run: `python3 docs/specs/vnext/tools/check_baseline_docs.py defects --allow-blocked` → `defects: 32/32 complete`, exit 0.
Run: `python3 docs/specs/vnext/tools/check_baseline_docs.py defects` → exactly `BD-32: still BLOCKED`, exit 1 (expected until Task 8).

- [ ] **Step 5: Commit** — Close-out 1–4; **do not tick T0.3** (it ticks in Task 8). Append to PROGRESS: `2026-09-28 · T0.3 · <sha> · BLOCKED · BD-01…31 verified; BD-32 needs R0 capture (T0.4)`. Also add under T0.3 in tasks.md: `  - [!] BLOCKED: T0.3 — BD-32 needs a real /v1/models sample. Options: R0 capture after T0.4 (planned, Task 8). Need: nothing — resolves in Task 8.`

```bash
git add docs/specs/vnext/baseline/DEFECTS.md
git commit -F - <<'MSG'
T0.3: defect register BD-01..31 re-verified; BD-32 pending R0 capture

Task: T0.3
Refs: REQ-MIG-006

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
MSG
```
Close-out 5 (PROGRESS/BLOCKED lines via amend) and 7.

---

### Task 4: T0.4 (1/5) — Workspace, fake crate core: scenario, recorder, models, load/unload, health, metrics, slots

**Files:**
- Modify: `Cargo.toml` (root)
- Create: `tools/fake-llama-server/Cargo.toml`, `tools/fake-llama-server/src/{lib.rs,scenario.rs,recorder.rs,engine.rs,record.rs}`, `tools/fake-llama-server/tests/http.rs`

**Interfaces:**
- Produces (used by Tasks 5–10):
  - `fake_llama_server::spawn(Scenario) -> Handle` (async; binds `127.0.0.1:0`)
  - `Handle { pub addr: SocketAddr, pub recorder: Recorder }`, `Handle::base_url(&self) -> String`, `Handle::loaded(&self) -> Vec<String>`
  - `serve_on(TcpListener, Scenario, Recorder, CrashMode) -> io::Result<()>`; `CrashMode::{DropConnection, ExitProcess}`
  - `Scenario { models: Vec<ModelSpec>, models_shape: ModelsShape, max_loaded: Option<usize>, chat_chunks: Option<Vec<String>>, routes: HashMap<String, Fault>, oom: Option<OomRule>, replay_from: Option<PathBuf> }`; builders `with_model(&str, bool)`, `with_shape(ModelsShape)`, `with_max_loaded(usize)`, `with_fault(&str, Fault)`, `with_chat_chunks(&[&str])`, `with_replay(impl Into<PathBuf>)`; `Scenario::from_toml(&str)`, `Scenario::from_file(&Path)`
  - `Fault::{Status{code,body}, Chunks{items,delay_ms}, HangBeforeHeaders, CrashAfter{chunks}, Malformed}` — route key format `"METHOD /path"`
  - `ModelsShape::{StatusObject (default), Legacy}`
  - `Recorder::{default(), to_file(&Path), record(&str, Value), events(), of_kind(&str)}`
  - `record::replay_file(&Path, &str) -> Option<(u16, String, Vec<u8>)>` (stub returning `None` in this task; implemented in Task 7)

- [ ] **Step 1: Root workspace** — append to root `Cargo.toml`:

```toml

[workspace]
members = ["tools/fake-llama-server"]

[dev-dependencies]
fake-llama-server = { path = "tools/fake-llama-server" }
```

- [ ] **Step 2: `tools/fake-llama-server/Cargo.toml`**

```toml
[package]
name = "fake-llama-server"
version = "0.0.0"
edition = "2024"
publish = false
description = "Scenario-driven stand-in for llama-server, for Saltnitor tests only (REQ-TST-011)"

[dependencies]
axum = "0.8"
tokio = { version = "1.52.3", features = ["full"] }
tokio-stream = "0.1"
serde = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.149"
toml = "1.1.2"
reqwest = { version = "0.13.3", features = ["json"] }
```

- [ ] **Step 3: Write the failing tests** — `tools/fake-llama-server/tests/http.rs`

```rust
//! HTTP behavior of the in-process fake (REQ-TST-011).
use fake_llama_server::{spawn, ModelsShape, Scenario};
use serde_json::{json, Value};

fn ab() -> Scenario {
    Scenario::default().with_model("A", true).with_model("B", false)
}

async fn get(url: String) -> (u16, Value) {
    let r = reqwest::get(url).await.expect("request");
    let s = r.status().as_u16();
    (s, r.json().await.unwrap_or(Value::Null))
}

async fn post(url: String, body: Value) -> (u16, Value) {
    let r = reqwest::Client::new().post(url).json(&body).send().await.expect("request");
    let s = r.status().as_u16();
    (s, r.json().await.unwrap_or(Value::Null))
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn health_reports_ok() {
    let f = spawn(Scenario::default()).await;
    assert_eq!(get(format!("{}/health", f.base_url())).await, (200, json!({"status": "ok"})));
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn models_use_status_objects_by_default_on_both_paths() {
    let f = spawn(ab()).await;
    let want = json!({"object": "list", "data": [
        {"id": "A", "object": "model", "owned_by": "llamacpp", "status": {"value": "loaded"}},
        {"id": "B", "object": "model", "owned_by": "llamacpp", "status": {"value": "unloaded"}}
    ]});
    assert_eq!(get(format!("{}/models", f.base_url())).await, (200, want.clone()));
    assert_eq!(get(format!("{}/v1/models", f.base_url())).await, (200, want));
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn legacy_shape_uses_a_status_string() {
    let f = spawn(ab().with_shape(ModelsShape::Legacy)).await;
    let (_, v) = get(format!("{}/v1/models", f.base_url())).await;
    assert_eq!(v["data"][0]["status"], json!("loaded"));
    assert_eq!(v["data"][1]["status"], json!("unloaded"));
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn load_and_unload_change_what_models_reports() {
    let f = spawn(ab()).await;
    assert_eq!(post(format!("{}/models/load", f.base_url()), json!({"model": "B"})).await, (200, json!({"success": true})));
    assert_eq!(f.loaded(), vec!["A", "B"]);
    assert_eq!(post(format!("{}/models/unload", f.base_url()), json!({"model": "A"})).await.0, 200);
    assert_eq!(f.loaded(), vec!["B"]);
    let (_, v) = get(format!("{}/models", f.base_url())).await;
    assert_eq!(v["data"][0]["status"]["value"], json!("unloaded"));
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn max_loaded_evicts_the_oldest_model() {
    let f = spawn(ab().with_max_loaded(1)).await;
    post(format!("{}/models/load", f.base_url()), json!({"model": "B"})).await;
    assert_eq!(f.loaded(), vec!["B"]);
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn load_of_unknown_model_is_404_and_changes_nothing() {
    let f = spawn(ab()).await;
    assert_eq!(post(format!("{}/models/load", f.base_url()), json!({"model": "Z"})).await.0, 404);
    assert_eq!(post(format!("{}/models/load", f.base_url()), json!({})).await.0, 400);
    assert_eq!(f.loaded(), vec!["A"]);
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn metrics_and_slots_are_served() {
    let f = spawn(ab()).await;
    let r = reqwest::get(format!("{}/metrics", f.base_url())).await.unwrap();
    assert!(r.headers()["content-type"].to_str().unwrap().starts_with("text/plain"));
    let text = r.text().await.unwrap();
    assert!(text.contains("llamacpp:requests_processing 0"), "{text}");
    assert!(text.contains("llamacpp:models_loaded 1"), "{text}");
    assert_eq!(get(format!("{}/slots", f.base_url())).await, (200, json!([{"id": 0, "is_processing": false}])));
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn unknown_route_is_404() {
    let f = spawn(ab()).await;
    assert_eq!(get(format!("{}/nope", f.base_url())).await.0, 404);
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn recorder_strips_authorization() {
    let f = spawn(ab()).await;
    reqwest::Client::new()
        .post(format!("{}/models/load", f.base_url()))
        .bearer_auth("sk-secret")
        .body(r#"{"model":"B"}"#)
        .send().await.unwrap();
    let reqs = f.recorder.of_kind("request");
    let last = reqs.last().unwrap();
    assert_eq!(last["method"], "POST");
    assert_eq!(last["path"], "/models/load");
    assert_eq!(last["body"], r#"{"model":"B"}"#);
    assert_eq!(last["has_authorization"], true);
    assert!(last["headers"].get("authorization").is_none());
    assert!(!serde_json::to_string(&f.recorder.events()).unwrap().contains("sk-secret"));
}
```

- [ ] **Step 4: Run to verify it fails**

Run: `cargo build --workspace` (refreshes the local lockfile) then `cargo test -p fake-llama-server --test http`
Expected: compile error — crate `fake_llama_server` has no `spawn`/`Scenario` (files missing).

- [ ] **Step 5: `src/scenario.rs`**

```rust
//! Scenario: the data a test (or `FAKE_SCENARIO` TOML) gives the fake runtime.
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    #[serde(default)]
    pub models_shape: ModelsShape,
    /// Like llama-server `--models-max`: loading beyond this evicts the oldest.
    #[serde(default)]
    pub max_loaded: Option<usize>,
    /// Content pieces of the default chat reply (default: "Hello", " from", " fake").
    #[serde(default)]
    pub chat_chunks: Option<Vec<String>>,
    /// Directory of an R0 capture; GET endpoints found there are served from it.
    #[serde(default)]
    pub replay_from: Option<PathBuf>,
    #[serde(default)]
    pub oom: Option<OomRule>,
    #[serde(default)]
    pub models: Vec<ModelSpec>,
    /// Faults keyed by `"METHOD /path"`, e.g. `"POST /v1/chat/completions"`.
    #[serde(default)]
    pub routes: HashMap<String, Fault>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelSpec {
    pub id: String,
    #[serde(default)]
    pub loaded: bool,
}

/// How `/models` and `/v1/models` report load state.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelsShape {
    /// Current upstream router: `"status": {"value": "loaded"}`.
    #[default]
    StatusObject,
    /// What Saltnitor's `router_loaded()` reads at c89f278: `"status": "loaded"`.
    Legacy,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Fault {
    /// Reply with this status and body (JSON content type when the body starts with `{`).
    Status { code: u16, #[serde(default)] body: String },
    /// Stream these pieces with a delay before each (chat: content deltas as SSE).
    Chunks { items: Vec<String>, delay_ms: u64 },
    /// Accept the request and never send headers.
    HangBeforeHeaders,
    /// Send this many pieces, then crash (drop the connection, or exit the binary).
    CrashAfter { chunks: usize },
    /// 200 with a truncated JSON body or broken SSE framing.
    Malformed,
}

/// Binary only: exit at startup with llama.cpp-style OOM text when `arg`'s value > `gt`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OomRule {
    pub arg: String,
    pub gt: u64,
}

impl Scenario {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }

    /// Parse a scenario file; a relative `replay_from` is resolved against the file's directory.
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let mut s = Self::from_toml(&text)?;
        if let Some(r) = s.replay_from.as_ref().filter(|r| r.is_relative()) {
            s.replay_from = Some(path.parent().unwrap_or(Path::new(".")).join(r));
        }
        Ok(s)
    }

    pub fn with_model(mut self, id: &str, loaded: bool) -> Self {
        self.models.push(ModelSpec { id: id.to_string(), loaded });
        self
    }

    pub fn with_shape(mut self, shape: ModelsShape) -> Self {
        self.models_shape = shape;
        self
    }

    pub fn with_max_loaded(mut self, n: usize) -> Self {
        self.max_loaded = Some(n);
        self
    }

    pub fn with_fault(mut self, route: &str, fault: Fault) -> Self {
        self.routes.insert(route.to_string(), fault);
        self
    }

    pub fn with_chat_chunks(mut self, pieces: &[&str]) -> Self {
        self.chat_chunks = Some(pieces.iter().map(|p| p.to_string()).collect());
        self
    }

    pub fn with_replay(mut self, dir: impl Into<PathBuf>) -> Self {
        self.replay_from = Some(dir.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies: REQ-TST-011/AC1
    #[test]
    fn toml_scenario_parses_every_field() {
        let s = Scenario::from_toml(
            r#"
models_shape = "legacy"
max_loaded = 1
chat_chunks = ["a", "b"]
[oom]
arg = "--ctx-size"
gt = 65536
[[models]]
id = "A"
loaded = true
[routes."GET /health"]
kind = "status"
code = 500
body = "down"
[routes."POST /v1/chat/completions"]
kind = "chunks"
items = ["x"]
delay_ms = 10
"#,
        )
        .unwrap();
        assert_eq!(s.models_shape, ModelsShape::Legacy);
        assert_eq!(s.max_loaded, Some(1));
        assert_eq!(s.chat_chunks, Some(vec!["a".to_string(), "b".to_string()]));
        assert_eq!(s.oom, Some(OomRule { arg: "--ctx-size".into(), gt: 65536 }));
        assert_eq!(s.models[0].id, "A");
        assert!(s.models[0].loaded);
        assert_eq!(s.routes["GET /health"], Fault::Status { code: 500, body: "down".into() });
        assert_eq!(s.routes["POST /v1/chat/completions"], Fault::Chunks { items: vec!["x".into()], delay_ms: 10 });
    }

    /// Verifies: REQ-TST-011/AC1
    #[test]
    fn unknown_keys_are_rejected() {
        assert!(Scenario::from_toml("modelz = []").is_err());
    }

    /// Verifies: REQ-TST-011/AC1
    #[test]
    fn relative_replay_dir_resolves_against_the_scenario_file() {
        let dir = std::env::temp_dir().join(format!("fake-scn-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("s.toml");
        std::fs::write(&file, "replay_from = \"cap\"\n").unwrap();
        assert_eq!(Scenario::from_file(&file).unwrap().replay_from, Some(dir.join("cap")));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
```

- [ ] **Step 6: `src/recorder.rs`**

```rust
//! Event recorder: argv, env, requests, disconnects, crashes — in memory and optionally JSONL.
//! Never stores the Authorization header value.
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use axum::http::{HeaderMap, header};
use serde_json::{Map, Value, json};

#[derive(Clone)]
pub struct Recorder {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    t0: Instant,
    events: Vec<Value>,
    file: Option<std::fs::File>,
}

impl Default for Recorder {
    fn default() -> Self {
        Self { inner: Arc::new(Mutex::new(Inner { t0: Instant::now(), events: Vec::new(), file: None })) }
    }
}

impl Recorder {
    /// Also append every event as one JSON line to `path`.
    pub fn to_file(path: &Path) -> std::io::Result<Self> {
        let file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
        let r = Self::default();
        r.inner.lock().expect("recorder lock").file = Some(file);
        Ok(r)
    }

    /// Record `data` (an object) with `kind` and milliseconds since start (`t_ms`).
    pub fn record(&self, kind: &str, mut data: Value) {
        let mut g = self.inner.lock().expect("recorder lock");
        let t_ms = g.t0.elapsed().as_millis() as u64;
        if let Value::Object(m) = &mut data {
            m.insert("kind".into(), json!(kind));
            m.insert("t_ms".into(), json!(t_ms));
        }
        if let Some(f) = g.file.as_mut() {
            let _ = writeln!(f, "{data}");
            let _ = f.flush();
        }
        g.events.push(data);
    }

    pub(crate) fn request(&self, method: &str, path: &str, query: &str, headers: &HeaderMap, body: &[u8]) {
        let kept: Map<String, Value> = headers
            .iter()
            .filter(|(k, _)| *k != header::AUTHORIZATION)
            .map(|(k, v)| (k.to_string(), json!(v.to_str().unwrap_or(""))))
            .collect();
        self.record(
            "request",
            json!({
                "method": method, "path": path, "query": query, "headers": kept,
                "has_authorization": headers.contains_key(header::AUTHORIZATION),
                "body": String::from_utf8_lossy(body),
            }),
        );
    }

    pub(crate) fn disconnect(&self, path: &str) {
        self.record("disconnect", json!({ "path": path }));
    }

    pub fn events(&self) -> Vec<Value> {
        self.inner.lock().expect("recorder lock").events.clone()
    }

    pub fn of_kind(&self, kind: &str) -> Vec<Value> {
        self.events().into_iter().filter(|e| e["kind"] == kind).collect()
    }
}
```

- [ ] **Step 7: `src/engine.rs`** (chat and streaming faults arrive in Task 5: until then `/v1/chat/completions` is an unknown route (404) and non-`Status` faults answer an explicit 501)

```rust
//! The fake llama-server HTTP surface: one fallback handler dispatching on (method, path).
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::response::Response;
use serde_json::{Value, json};

use crate::recorder::Recorder;
use crate::scenario::{Fault, ModelsShape, Scenario};

/// What a `CrashAfter` fault does once its chunks are sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrashMode {
    /// In-process: abort the HTTP connection.
    DropConnection,
    /// Binary: exit the process with status 101.
    ExitProcess,
}

pub(crate) struct Shared {
    pub(crate) scenario: Scenario,
    pub(crate) loaded: Mutex<Vec<String>>,
    pub(crate) recorder: Recorder,
    pub(crate) crash: CrashMode,
}

impl Shared {
    pub(crate) fn new(scenario: Scenario, recorder: Recorder, crash: CrashMode) -> Arc<Self> {
        let loaded = scenario.models.iter().filter(|m| m.loaded).map(|m| m.id.clone()).collect();
        Arc::new(Self { scenario, loaded: Mutex::new(loaded), recorder, crash })
    }

    pub(crate) fn load(&self, id: &str) {
        let mut l = self.loaded.lock().expect("loaded lock");
        if l.iter().any(|m| m == id) {
            return;
        }
        l.push(id.to_string());
        if let Some(max) = self.scenario.max_loaded {
            while l.len() > max {
                l.remove(0);
            }
        }
    }

    fn unload(&self, id: &str) {
        self.loaded.lock().expect("loaded lock").retain(|m| m != id);
    }

    pub(crate) fn known(&self, id: &str) -> bool {
        self.scenario.models.is_empty() || self.scenario.models.iter().any(|m| m.id == id)
    }
}

pub(crate) fn router(shared: Arc<Shared>) -> Router {
    Router::new().fallback(handle).with_state(shared)
}

async fn handle(State(st): State<Arc<Shared>>, req: Request) -> Response {
    let method = req.method().as_str().to_string();
    let path = req.uri().path().to_string();
    let query = req.uri().query().unwrap_or("").to_string();
    let headers = req.headers().clone();
    let body = axum::body::to_bytes(req.into_body(), usize::MAX).await.unwrap_or_default();
    st.recorder.request(&method, &path, &query, &headers, &body);
    let json: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);

    if let Some(fault) = st.scenario.routes.get(&format!("{method} {path}")).cloned() {
        return apply_fault(&st, fault, &method, &path, &json).await;
    }
    if method == "GET" {
        if let Some(resp) = replay(&st, &path) {
            return resp;
        }
    }
    match (method.as_str(), path.as_str()) {
        ("GET", "/health") => json_resp(200, json!({"status": "ok"})),
        ("GET", "/models") | ("GET", "/v1/models") => json_resp(200, models_body(&st)),
        ("POST", "/models/load") => load_unload(&st, &json, true),
        ("POST", "/models/unload") => load_unload(&st, &json, false),
        ("GET", "/metrics") => text_resp(200, "text/plain; version=0.0.4", metrics_body(&st)),
        ("GET", "/slots") => json_resp(200, json!([{"id": 0, "is_processing": false}])),
        _ => not_found(),
    }
}

async fn apply_fault(st: &Arc<Shared>, fault: Fault, _method: &str, _path: &str, _json: &Value) -> Response {
    match fault {
        Fault::Status { code, body } => {
            let ct = if body.trim_start().starts_with('{') { "application/json" } else { "text/plain" };
            text_resp(code, ct, body)
        }
        other => {
            // Streaming faults are implemented in Task 5; until then they are an explicit 501,
            // and no test in this task depends on them.
            let _ = st;
            text_resp(501, "text/plain", format!("fault not implemented yet: {other:?}"))
        }
    }
}

fn replay(st: &Shared, path: &str) -> Option<Response> {
    let dir = st.scenario.replay_from.as_ref()?;
    let (status, ct, bytes) = crate::record::replay_file(dir, path)?;
    Some(
        Response::builder()
            .status(StatusCode::from_u16(status).unwrap_or(StatusCode::OK))
            .header(header::CONTENT_TYPE, ct)
            .body(Body::from(bytes))
            .expect("valid replay response"),
    )
}

fn models_body(st: &Shared) -> Value {
    let loaded = st.loaded.lock().expect("loaded lock").clone();
    let data: Vec<Value> = st
        .scenario
        .models
        .iter()
        .map(|m| {
            let state = if loaded.contains(&m.id) { "loaded" } else { "unloaded" };
            let status = match st.scenario.models_shape {
                ModelsShape::StatusObject => json!({ "value": state }),
                ModelsShape::Legacy => json!(state),
            };
            json!({"id": m.id, "object": "model", "owned_by": "llamacpp", "status": status})
        })
        .collect();
    json!({"object": "list", "data": data})
}

fn load_unload(st: &Shared, body: &Value, load: bool) -> Response {
    let Some(id) = body["model"].as_str() else {
        return json_resp(400, error_body(400, "missing 'model'", "invalid_request_error"));
    };
    if !st.known(id) {
        return json_resp(404, error_body(404, &format!("model '{id}' not found"), "not_found_error"));
    }
    if load { st.load(id) } else { st.unload(id) }
    json_resp(200, json!({"success": true}))
}

fn metrics_body(st: &Shared) -> String {
    let n = st.loaded.lock().expect("loaded lock").len();
    format!(
        "# HELP llamacpp:requests_processing Number of requests processing.\n\
         # TYPE llamacpp:requests_processing gauge\n\
         llamacpp:requests_processing 0\n\
         llamacpp:models_loaded {n}\n"
    )
}

pub(crate) fn error_body(code: u16, message: &str, kind: &str) -> Value {
    json!({"error": {"code": code, "message": message, "type": kind}})
}

fn not_found() -> Response {
    json_resp(404, error_body(404, "File Not Found", "not_found_error"))
}

pub(crate) fn json_resp(code: u16, v: Value) -> Response {
    text_resp(code, "application/json", v.to_string())
}

pub(crate) fn text_resp(code: u16, content_type: &str, body: String) -> Response {
    Response::builder()
        .status(StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        .header(header::CONTENT_TYPE, content_type)
        .body(Body::from(body))
        .expect("valid response parts")
}
```

Note on the 501 branch: it is an honest, labeled "not implemented" response for faults that land in the very next task (Task 5 replaces `apply_fault` wholesale). It is not reachable by any test in this task and is gone before T0.4 is ticked.

- [ ] **Step 8: `src/record.rs` (replay lookup only; capture lands in Task 7)**

```rust
//! R0 record/replay (CR-2). This task provides the replay lookup the engine calls;
//! Task 7 adds capture, drift, sanitizing and the `record` CLI.
use std::path::Path;

/// Recorded (status, content type, body) for `path` in the capture at `dir`, if any.
pub fn replay_file(_dir: &Path, _path: &str) -> Option<(u16, String, Vec<u8>)> {
    None
}
```

- [ ] **Step 9: `src/lib.rs`**

```rust
//! Scenario-driven stand-in for llama-server, for Saltnitor's tests (REQ-TST-011).
//!
//! * In-process: [`spawn`] a [`Scenario`] on a loopback port and read what it saw from
//!   [`Handle::recorder`].
//! * Process-level: the `fake-llama-server` binary (see README.md).
//! * Real-world check: `fake-llama-server record` captures a live router read-only.
mod engine;
pub mod record;
mod recorder;
mod scenario;

use std::net::SocketAddr;
use std::sync::Arc;

pub use engine::CrashMode;
pub use recorder::Recorder;
pub use scenario::{Fault, ModelSpec, ModelsShape, OomRule, Scenario};

/// A running in-process fake. Dropping it stops the server.
pub struct Handle {
    pub addr: SocketAddr,
    pub recorder: Recorder,
    shared: Arc<engine::Shared>,
    task: tokio::task::JoinHandle<()>,
}

impl Handle {
    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Currently loaded model ids, oldest first.
    pub fn loaded(&self) -> Vec<String> {
        self.shared.loaded.lock().expect("loaded lock").clone()
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Serve `scenario` on a fresh `127.0.0.1` port.
pub async fn spawn(scenario: Scenario) -> Handle {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind loopback");
    let addr = listener.local_addr().expect("local addr");
    let recorder = Recorder::default();
    let shared = engine::Shared::new(scenario, recorder.clone(), CrashMode::DropConnection);
    let app = engine::router(shared.clone());
    let task = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Handle { addr, recorder, shared, task }
}

/// Serve on an existing listener until the process ends (used by the binary).
pub async fn serve_on(
    listener: tokio::net::TcpListener,
    scenario: Scenario,
    recorder: Recorder,
    crash: CrashMode,
) -> std::io::Result<()> {
    axum::serve(listener, engine::router(engine::Shared::new(scenario, recorder, crash))).await
}
```

- [ ] **Step 10: Run tests to verify they pass**

Run: `cargo test -p fake-llama-server`
Expected: 9 http tests + 3 scenario unit tests pass.

- [ ] **Step 11: Commit** — Close-out 1–4 (fmt: `cargo fmt -p fake-llama-server --check`; if it reports diffs, run `cargo fmt -p fake-llama-server` — the fake crate is new code, formatting it is allowed), then:

```bash
git add Cargo.toml tools/fake-llama-server
git commit -F - <<'MSG'
T0.4 (1/5): fake-llama-server core — scenario, recorder, model state

Task: T0.4
Refs: REQ-TST-011, REQ-TST-006

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
MSG
git push -u origin vnext
```
(No PROGRESS line or tick yet — T0.4 completes in Task 7.)

---

### Task 5: T0.4 (2/5) — Chat completions and streaming faults

**Files:**
- Modify: `tools/fake-llama-server/src/engine.rs` (add chat, replace `apply_fault`)
- Modify: `tools/fake-llama-server/tests/http.rs` (append tests)

**Interfaces:**
- Produces: `POST /v1/chat/completions` — unknown model → 400; missing model → 400; known model → autoload (respects `max_loaded`); `stream:false` → `chat.completion` JSON with content = concatenated chunks; `stream:true` → SSE `chat.completion.chunk` events then `data: [DONE]`. All five `Fault`s behave as documented in `scenario.rs`. Client disconnects are recorded as `kind = "disconnect"`.

- [ ] **Step 1: Append failing tests** to `tests/http.rs`

```rust
use std::time::{Duration, Instant};
use fake_llama_server::Fault;

fn chat_url(f: &fake_llama_server::Handle) -> String {
    format!("{}/v1/chat/completions", f.base_url())
}

async fn read_all(resp: &mut reqwest::Response) -> (String, Option<reqwest::Error>) {
    let mut text = String::new();
    loop {
        match resp.chunk().await {
            Ok(Some(c)) => text.push_str(&String::from_utf8_lossy(&c)),
            Ok(None) => return (text, None),
            Err(e) => return (text, Some(e)),
        }
    }
}

fn deltas(sse: &str) -> String {
    sse.lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter(|d| *d != "[DONE]")
        .map(|d| serde_json::from_str::<Value>(d).unwrap()["choices"][0]["delta"]["content"].as_str().unwrap().to_string())
        .collect()
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn chat_non_stream_autoloads_and_evicts() {
    let f = spawn(ab().with_max_loaded(1)).await;
    let (s, v) = post(chat_url(&f), json!({"model": "B", "messages": [{"role": "user", "content": "hi"}]})).await;
    assert_eq!(s, 200);
    assert_eq!(v["object"], "chat.completion");
    assert_eq!(v["model"], "B");
    assert_eq!(v["choices"][0]["message"]["content"], "Hello from fake");
    assert_eq!(f.loaded(), vec!["B"]);
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn chat_rejects_missing_or_unknown_model() {
    let f = spawn(ab()).await;
    assert_eq!(post(chat_url(&f), json!({"messages": []})).await.0, 400);
    let (s, v) = post(chat_url(&f), json!({"model": "Z", "messages": []})).await;
    assert_eq!(s, 400);
    assert_eq!(v["error"]["message"], "model 'Z' not found");
    assert_eq!(f.loaded(), vec!["A"]);
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn chat_stream_sends_sse_chunks_then_done() {
    let f = spawn(ab()).await;
    let mut r = reqwest::Client::new().post(chat_url(&f))
        .json(&json!({"model": "A", "stream": true, "messages": []})).send().await.unwrap();
    assert_eq!(r.headers()["content-type"], "text/event-stream");
    let (text, err) = read_all(&mut r).await;
    assert!(err.is_none());
    assert_eq!(text.matches("chat.completion.chunk").count(), 3);
    assert_eq!(deltas(&text), "Hello from fake");
    assert!(text.ends_with("data: [DONE]\n\n"), "{text}");
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn chunks_fault_streams_with_scripted_timing() {
    let f = spawn(ab().with_fault("POST /v1/chat/completions",
        Fault::Chunks { items: vec!["a".into(), "b".into(), "c".into()], delay_ms: 150 })).await;
    let t0 = Instant::now();
    let mut r = reqwest::Client::new().post(chat_url(&f))
        .json(&json!({"model": "A", "stream": true, "messages": []})).send().await.unwrap();
    let mut first = None;
    let mut text = String::new();
    while let Some(c) = r.chunk().await.unwrap() {
        first.get_or_insert(t0.elapsed());
        text.push_str(&String::from_utf8_lossy(&c));
    }
    let (first, total) = (first.unwrap(), t0.elapsed());
    assert_eq!(deltas(&text), "abc");
    assert!(first >= Duration::from_millis(140), "first={first:?}");
    assert!(total >= Duration::from_millis(430), "total={total:?}");
    assert!(total - first >= Duration::from_millis(250), "streamed, not buffered: first={first:?} total={total:?}");
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn chunks_fault_on_non_stream_chat_waits_then_answers() {
    let f = spawn(ab().with_fault("POST /v1/chat/completions",
        Fault::Chunks { items: vec!["x".into(), "y".into()], delay_ms: 100 })).await;
    let t0 = Instant::now();
    let (s, v) = post(chat_url(&f), json!({"model": "A", "messages": []})).await;
    assert_eq!((s, v["choices"][0]["message"]["content"].as_str()), (200, Some("xy")));
    assert!(t0.elapsed() >= Duration::from_millis(190));
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn status_fault_overrides_a_route() {
    let f = spawn(ab().with_fault("GET /health", Fault::Status { code: 503, body: r#"{"error":"busy"}"#.into() })).await;
    let r = reqwest::get(format!("{}/health", f.base_url())).await.unwrap();
    assert_eq!(r.status(), 503);
    assert_eq!(r.headers()["content-type"], "application/json");
    assert_eq!(r.text().await.unwrap(), r#"{"error":"busy"}"#);
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn hang_before_headers_times_the_client_out() {
    let f = spawn(ab().with_fault("GET /health", Fault::HangBeforeHeaders)).await;
    let err = reqwest::Client::new().get(format!("{}/health", f.base_url()))
        .timeout(Duration::from_millis(300)).send().await.unwrap_err();
    assert!(err.is_timeout(), "{err}");
    assert_eq!(f.recorder.of_kind("request").last().unwrap()["path"], "/health");
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn crash_after_drops_the_connection_mid_stream() {
    let f = spawn(ab().with_fault("POST /v1/chat/completions", Fault::CrashAfter { chunks: 1 })).await;
    let mut r = reqwest::Client::new().post(chat_url(&f))
        .json(&json!({"model": "A", "stream": true, "messages": []})).send().await.unwrap();
    let (text, err) = read_all(&mut r).await;
    assert!(err.is_some(), "stream must end in an error, got clean end: {text}");
    assert_eq!(text.matches("chat.completion.chunk").count(), 1, "{text}");
    assert!(!text.contains("[DONE]"));
    assert_eq!(f.recorder.of_kind("crash").len(), 1);
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn malformed_fault_breaks_json_and_sse() {
    let f = spawn(ab().with_fault("POST /v1/chat/completions", Fault::Malformed)).await;
    let r = reqwest::Client::new().post(chat_url(&f)).json(&json!({"model": "A", "messages": []})).send().await.unwrap();
    assert_eq!(r.status(), 200);
    assert!(serde_json::from_str::<Value>(&r.text().await.unwrap()).is_err());
    let r = reqwest::Client::new().post(chat_url(&f)).json(&json!({"model": "A", "stream": true, "messages": []})).send().await.unwrap();
    let text = r.text().await.unwrap();
    let data = text.strip_prefix("data: ").unwrap().trim_end();
    assert!(serde_json::from_str::<Value>(data).is_err());
    assert!(!text.contains("[DONE]"));
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn client_disconnect_is_recorded() {
    let items = (0..10).map(|i| i.to_string()).collect();
    let f = spawn(ab().with_fault("POST /v1/chat/completions", Fault::Chunks { items, delay_ms: 100 })).await;
    let mut r = reqwest::Client::new().post(chat_url(&f))
        .json(&json!({"model": "A", "stream": true, "messages": []})).send().await.unwrap();
    r.chunk().await.unwrap();
    drop(r);
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let d = f.recorder.of_kind("disconnect");
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0]["path"], "/v1/chat/completions");
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p fake-llama-server --test http`
Expected: the 10 new tests FAIL (chat routes 404; faults 501).

- [ ] **Step 3: Implement** — in `engine.rs`, add imports and replace `apply_fault`; add chat route and helpers.

Add to the imports:
```rust
use std::time::Duration;

use axum::body::Bytes;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
```

Add the chat arm to the `match` in `handle` (before `_ => not_found()`):
```rust
        ("POST", "/v1/chat/completions") => chat(&st, &json),
```

Replace the whole `apply_fault` function with:
```rust
async fn apply_fault(st: &Arc<Shared>, fault: Fault, method: &str, path: &str, json: &Value) -> Response {
    let is_chat = method == "POST" && path == "/v1/chat/completions";
    let stream = json["stream"].as_bool().unwrap_or(false);
    let model = json["model"].as_str().unwrap_or("fake").to_string();
    match fault {
        Fault::Status { code, body } => {
            let ct = if body.trim_start().starts_with('{') { "application/json" } else { "text/plain" };
            text_resp(code, ct, body)
        }
        Fault::HangBeforeHeaders => std::future::pending::<Response>().await,
        Fault::Chunks { items, delay_ms } if is_chat && stream => {
            st.load(&model);
            stream_body(st.clone(), path, sse_pieces(&model, &items, true), delay_ms, None, "text/event-stream")
        }
        Fault::Chunks { items, delay_ms } if is_chat => {
            st.load(&model);
            tokio::time::sleep(Duration::from_millis(delay_ms * items.len() as u64)).await;
            json_resp(200, completion(&model, &items.concat()))
        }
        Fault::Chunks { items, delay_ms } => {
            stream_body(st.clone(), path, items, delay_ms, None, "application/octet-stream")
        }
        Fault::CrashAfter { chunks } => {
            let items = chat_items(st);
            let (pieces, ct) = if is_chat {
                (sse_pieces(&model, &items, false), "text/event-stream")
            } else {
                (items, "application/octet-stream")
            };
            stream_body(st.clone(), path, pieces, 0, Some(chunks), ct)
        }
        Fault::Malformed if is_chat && stream => {
            text_resp(200, "text/event-stream", "data: {\"choices\":[{\"delta\":{\"content\":\"x\"\n\n".into())
        }
        Fault::Malformed => text_resp(200, "application/json", "{\"object\":\"list\",\"data\":[".into()),
    }
}

fn chat(st: &Arc<Shared>, body: &Value) -> Response {
    let Some(model) = body["model"].as_str() else {
        return json_resp(400, error_body(400, "missing 'model'", "invalid_request_error"));
    };
    if !st.known(model) {
        return json_resp(400, error_body(400, &format!("model '{model}' not found"), "invalid_request_error"));
    }
    st.load(model);
    let items = chat_items(st);
    if body["stream"].as_bool().unwrap_or(false) {
        stream_body(st.clone(), "/v1/chat/completions", sse_pieces(model, &items, true), 0, None, "text/event-stream")
    } else {
        json_resp(200, completion(model, &items.concat()))
    }
}

fn chat_items(st: &Shared) -> Vec<String> {
    st.scenario.chat_chunks.clone().unwrap_or_else(|| vec!["Hello".into(), " from".into(), " fake".into()])
}

fn completion(model: &str, content: &str) -> Value {
    json!({
        "id": "chatcmpl-fake", "object": "chat.completion", "model": model,
        "choices": [{"index": 0, "message": {"role": "assistant", "content": content}, "finish_reason": "stop"}],
        "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
    })
}

fn sse_pieces(model: &str, items: &[String], done: bool) -> Vec<String> {
    let mut out: Vec<String> = items
        .iter()
        .map(|c| {
            let ev = json!({
                "id": "chatcmpl-fake", "object": "chat.completion.chunk", "model": model,
                "choices": [{"index": 0, "delta": {"content": c}, "finish_reason": null}]
            });
            format!("data: {ev}\n\n")
        })
        .collect();
    if done {
        out.push("data: [DONE]\n\n".into());
    }
    out
}

/// Stream `pieces` (sleeping `delay_ms` before each); crash after `crash_after` pieces.
/// A failed send means the client went away: record a disconnect and stop.
fn stream_body(
    st: Arc<Shared>,
    path: &str,
    pieces: Vec<String>,
    delay_ms: u64,
    crash_after: Option<usize>,
    content_type: &str,
) -> Response {
    let (tx, rx) = mpsc::channel::<Result<Bytes, std::io::Error>>(1);
    let path = path.to_string();
    tokio::spawn(async move {
        for (i, piece) in pieces.into_iter().enumerate() {
            if crash_after == Some(i) {
                return crash(&st, &tx).await;
            }
            if delay_ms > 0 {
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }
            if tx.send(Ok(Bytes::from(piece))).await.is_err() {
                st.recorder.disconnect(&path);
                return;
            }
        }
        if crash_after.is_some() {
            crash(&st, &tx).await;
        }
    });
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .body(Body::from_stream(ReceiverStream::new(rx)))
        .expect("valid streaming response")
}

async fn crash(st: &Shared, tx: &mpsc::Sender<Result<Bytes, std::io::Error>>) {
    st.recorder.record("crash", json!({}));
    tokio::time::sleep(Duration::from_millis(50)).await; // let already-sent pieces flush
    match st.crash {
        CrashMode::DropConnection => {
            let _ = tx.send(Err(std::io::Error::other("fake-llama-server: simulated crash"))).await;
        }
        CrashMode::ExitProcess => std::process::exit(101),
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p fake-llama-server` (run twice to catch timing flakiness)
Expected: all pass both times.

- [ ] **Step 5: Commit** — Close-out 1–4, then `git add tools/fake-llama-server && git commit` with subject `T0.4 (2/5): fake chat completions, SSE and fault scripting`, same `Task:`/`Refs:`/trailer as Task 4; push.

---

### Task 6: T0.4 (3/5) — The binary: argv/env recording, help/version fixtures, OOM, crash exit, README

**Files:**
- Create: `tools/fake-llama-server/src/main.rs`, `tools/fake-llama-server/fixtures/{help-default.txt,version-default.txt}`, `tools/fake-llama-server/tests/process.rs`, `tools/fake-llama-server/README.md`

**Interfaces:**
- Produces: binary `fake-llama-server` — llama-server-style argv (`--host`, `--port`, anything else recorded and ignored); env `FAKE_SCENARIO` (TOML path), `FAKE_RECORD` (JSONL path), `FAKE_HELP_FIXTURE`, `FAKE_VERSION_FIXTURE`; prints `FAKE_LISTENING <addr>` on stdout once bound; exit 1 on OOM rule, 2 on bad scenario/record file, 101 on `CrashAfter`. Subcommand `record` (Task 7).

- [ ] **Step 1: Fixtures**

`fixtures/help-default.txt`:
```
usage: fake-llama-server [options]

A scenario-driven stand-in for llama-server (tests only).

  -h,    --help        print this help and exit
  --version            print version and exit
  --host HOST          bind address (default: 127.0.0.1)
  --port PORT          bind port (default: 8080; 0 = any free port)

Environment: FAKE_SCENARIO, FAKE_RECORD, FAKE_HELP_FIXTURE, FAKE_VERSION_FIXTURE
```
`fixtures/version-default.txt`:
```
version: 0 (fake-llama-server)
built with rustc for testing only
```

- [ ] **Step 2: Failing tests** — `tests/process.rs`

```rust
//! Process-level behavior of the fake binary (REQ-TST-011).
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use serde_json::Value;

const BIN: &str = env!("CARGO_BIN_EXE_fake-llama-server");

fn tmpdir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("fake-llama-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Start the binary on a free port; returns the child and its base URL.
fn start(cmd: &mut Command) -> (Child, String) {
    let mut child = cmd.args(["--port", "0"]).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.as_mut().unwrap()).read_line(&mut line).unwrap();
    let addr = line.trim().strip_prefix("FAKE_LISTENING ").unwrap_or_else(|| panic!("unexpected: {line:?}")).to_string();
    (child, format!("http://{addr}"))
}

fn jsonl(path: &PathBuf) -> Vec<Value> {
    std::fs::read_to_string(path).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

/// Verifies: REQ-TST-011/AC1
#[test]
fn help_and_version_print_default_fixtures() {
    let out = Command::new(BIN).arg("--help").output().unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8(out.stdout).unwrap(), include_str!("../fixtures/help-default.txt"));
    let out = Command::new(BIN).arg("--version").output().unwrap();
    assert_eq!(String::from_utf8(out.stdout).unwrap(), include_str!("../fixtures/version-default.txt"));
}

/// Verifies: REQ-TST-011/AC1
#[test]
fn help_fixture_is_selected_by_env() {
    let d = tmpdir("help");
    std::fs::write(d.join("h.txt"), "usage: llama-server [build 1234]\n  --n-cpu-moe N\n").unwrap();
    let out = Command::new(BIN).arg("--help").env("FAKE_HELP_FIXTURE", d.join("h.txt")).output().unwrap();
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "usage: llama-server [build 1234]\n  --n-cpu-moe N\n");
    let out = Command::new(BIN).arg("--help").env("FAKE_HELP_FIXTURE", d.join("missing.txt")).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn argv_and_allowlisted_env_are_recorded() {
    let d = tmpdir("argv");
    let rec = d.join("rec.jsonl");
    let (mut child, base) = start(
        Command::new(BIN)
            .args(["--models-preset", "x.ini", "-ngl", "99"])
            .env("FAKE_RECORD", &rec)
            .env("LLAMA_ARG_THREADS", "7")
            .env("SECRET_TOKEN", "hunter2"),
    );
    assert_eq!(reqwest::get(format!("{base}/health")).await.unwrap().status(), 200);
    child.kill().unwrap();
    child.wait().unwrap();
    let ev = jsonl(&rec);
    let argv = &ev.iter().find(|e| e["kind"] == "argv").unwrap()["argv"];
    assert!(argv.as_array().unwrap().iter().any(|a| a == "-ngl"));
    let env = &ev.iter().find(|e| e["kind"] == "env").unwrap()["env"];
    assert_eq!(env["LLAMA_ARG_THREADS"], "7");
    assert!(env.get("SECRET_TOKEN").is_none());
    assert!(!std::fs::read_to_string(&rec).unwrap().contains("hunter2"));
    assert!(ev.iter().any(|e| e["kind"] == "request" && e["path"] == "/health"));
}

/// Verifies: REQ-TST-011/AC1
#[test]
fn oom_rule_exits_when_the_argument_is_too_large() {
    let d = tmpdir("oom");
    std::fs::write(d.join("s.toml"), "[oom]\narg = \"--ctx-size\"\ngt = 65536\n").unwrap();
    let out = Command::new(BIN).args(["--ctx-size", "131072", "--port", "0"])
        .env("FAKE_SCENARIO", d.join("s.toml")).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("cudaMalloc failed: out of memory"));
    let (mut child, _) = start(Command::new(BIN).args(["--ctx-size", "4096"]).env("FAKE_SCENARIO", d.join("s.toml")));
    child.kill().unwrap();
    child.wait().unwrap();
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn crash_after_exits_the_process() {
    let d = tmpdir("crash");
    std::fs::write(d.join("s.toml"),
        "[routes.\"POST /v1/chat/completions\"]\nkind = \"crash_after\"\nchunks = 1\n").unwrap();
    let (mut child, base) = start(Command::new(BIN).env("FAKE_SCENARIO", d.join("s.toml")));
    let mut r = reqwest::Client::new().post(format!("{base}/v1/chat/completions"))
        .body(r#"{"model":"A","stream":true,"messages":[]}"#).send().await.unwrap();
    while let Ok(Some(_)) = r.chunk().await {}
    assert_eq!(child.wait().unwrap().code(), Some(101));
}

/// Verifies: REQ-TST-011/AC1
#[test]
fn bad_scenario_exits_2_with_a_diagnostic() {
    let d = tmpdir("bad");
    std::fs::write(d.join("s.toml"), "models = [").unwrap();
    let out = Command::new(BIN).args(["--port", "0"]).env("FAKE_SCENARIO", d.join("s.toml")).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("bad scenario"));
}
```

- [ ] **Step 3: Run to verify they fail**

Run: `cargo test -p fake-llama-server --test process`
Expected: compile error — `CARGO_BIN_EXE_fake-llama-server` not defined (no binary yet).

- [ ] **Step 4: `src/main.rs`**

```rust
//! `fake-llama-server` binary: behaves like llama-server for process-level tests.
use std::io::Write;
use std::path::Path;

use fake_llama_server::{CrashMode, Recorder, Scenario, record, serve_on};
use serde_json::{Map, Value, json};

const DEFAULT_HELP: &str = include_str!("../fixtures/help-default.txt");
const DEFAULT_VERSION: &str = include_str!("../fixtures/version-default.txt");
/// Only these environment variables are recorded, so secrets never reach the JSONL.
const ENV_PREFIXES: &[&str] = &["LLAMA_", "GGML_", "CUDA_", "HIP_", "FAKE_"];

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("record") {
        std::process::exit(record::cli(&args[2..]).await);
    }
    if args.iter().skip(1).any(|a| a == "--help" || a == "-h") {
        return print_fixture("FAKE_HELP_FIXTURE", DEFAULT_HELP);
    }
    if args.iter().skip(1).any(|a| a == "--version") {
        return print_fixture("FAKE_VERSION_FIXTURE", DEFAULT_VERSION);
    }
    let scenario = match std::env::var_os("FAKE_SCENARIO") {
        Some(p) => Scenario::from_file(Path::new(&p)).unwrap_or_else(|e| fail(2, &format!("bad scenario: {e}"))),
        None => Scenario::default(),
    };
    let recorder = match std::env::var_os("FAKE_RECORD") {
        Some(p) => Recorder::to_file(Path::new(&p)).unwrap_or_else(|e| fail(2, &format!("cannot open record file: {e}"))),
        None => Recorder::default(),
    };
    recorder.record("argv", json!({ "argv": args }));
    let env: Map<String, Value> = std::env::vars()
        .filter(|(k, _)| ENV_PREFIXES.iter().any(|p| k.starts_with(p)))
        .map(|(k, v)| (k, json!(v)))
        .collect();
    recorder.record("env", json!({ "env": env }));

    if let Some(rule) = &scenario.oom {
        if let Some(v) = arg_value(&args, &rule.arg).and_then(|v| v.parse::<u64>().ok()) {
            if v > rule.gt {
                recorder.record("oom", json!({ "arg": rule.arg, "value": v }));
                eprintln!("ggml_backend_cuda_buffer_type_alloc_buffer: allocating {v} MiB on device 0: cudaMalloc failed: out of memory");
                eprintln!("llama_init_from_model: failed to initialize the context: failed to allocate buffer");
                std::process::exit(1);
            }
        }
    }

    let host = arg_value(&args, "--host").unwrap_or_else(|| "127.0.0.1".into());
    let port = arg_value(&args, "--port").unwrap_or_else(|| "8080".into());
    let listener = tokio::net::TcpListener::bind(format!("{host}:{port}"))
        .await
        .unwrap_or_else(|e| fail(1, &format!("bind {host}:{port}: {e}")));
    println!("FAKE_LISTENING {}", listener.local_addr().expect("local addr"));
    let _ = std::io::stdout().flush();
    if let Err(e) = serve_on(listener, scenario, recorder, CrashMode::ExitProcess).await {
        fail(1, &e.to_string());
    }
}

/// Value of `--name V` or `--name=V`.
fn arg_value(args: &[String], name: &str) -> Option<String> {
    let eq = format!("{name}=");
    args.iter().enumerate().find_map(|(i, a)| {
        if a == name { args.get(i + 1).cloned() } else { a.strip_prefix(&eq).map(str::to_string) }
    })
}

fn print_fixture(var: &str, default: &str) {
    match std::env::var_os(var).map(std::fs::read_to_string) {
        Some(Ok(text)) => print!("{text}"),
        Some(Err(e)) => fail(2, &format!("{var}: {e}")),
        None => print!("{default}"),
    }
}

fn fail(code: i32, msg: &str) -> ! {
    eprintln!("fake-llama-server: {msg}");
    std::process::exit(code)
}

#[cfg(test)]
mod tests {
    use super::arg_value;

    fn a(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    /// Verifies: REQ-TST-011/AC1
    #[test]
    fn arg_value_reads_space_and_equals_forms_only_for_the_exact_flag() {
        assert_eq!(arg_value(&a(&["x", "--port", "9"]), "--port").as_deref(), Some("9"));
        assert_eq!(arg_value(&a(&["x", "--port=9"]), "--port").as_deref(), Some("9"));
        assert_eq!(arg_value(&a(&["x", "--port"]), "--port"), None);
        assert_eq!(arg_value(&a(&["x", "--portal", "9"]), "--port"), None);
    }
}
```

`record::cli` does not exist yet — add it to `src/record.rs` now as the real argument check that Task 7 extends:
```rust
/// `fake-llama-server record …` entry point; returns the process exit code.
pub async fn cli(args: &[String]) -> i32 {
    eprintln!("record: capture is added in the next commit (args: {args:?})");
    2
}
```
(Exit 2 = usage error. Task 7 replaces the body; no test in this task calls it.)

- [ ] **Step 5: README** — `tools/fake-llama-server/README.md`

```markdown
# fake-llama-server

Test-only stand-in for `llama-server` (REQ-TST-011). Never released (`publish = false`).

## In-process (fast HTTP tests)

    let fake = fake_llama_server::spawn(Scenario::default().with_model("A", true)).await;
    // point the code under test at fake.base_url(); inspect fake.recorder / fake.loaded()

## As a process (argv, env, --help, crashes, OOM)

Inside this crate's tests: `env!("CARGO_BIN_EXE_fake-llama-server")`.
From other crates' tests: run `cargo build -p fake-llama-server` first, then use
`<target-dir>/debug/fake-llama-server` (target dir = `$CARGO_TARGET_DIR` or `<workspace>/target`).

    FAKE_SCENARIO=path/to/scenario.toml   scenario (see src/scenario.rs for every field)
    FAKE_RECORD=path/to/events.jsonl      append argv/env/request/disconnect/crash events
    FAKE_HELP_FIXTURE / FAKE_VERSION_FIXTURE   text printed for --help / --version

Prints `FAKE_LISTENING <addr>` on stdout once bound (`--port 0` picks a free port).
Exit codes: 1 OOM rule or bind failure · 2 bad scenario/record/fixture file · 101 `crash_after`.
Only env vars starting with LLAMA_, GGML_, CUDA_, HIP_, FAKE_ are recorded; the
Authorization header value is never recorded.

## Scenario TOML

    models_shape = "status_object"   # or "legacy"
    max_loaded = 1
    [[models]]
    id = "A"
    loaded = true
    [routes."POST /v1/chat/completions"]
    kind = "chunks"            # status | chunks | hang_before_headers | crash_after | malformed
    items = ["a", "b"]
    delay_ms = 200

## Record / replay (real-world check, CR-2)

    fake-llama-server record --upstream http://127.0.0.1:8080 --out DIR [--baseline DIR] [--bearer-env VAR]

Read-only GETs only. Saves sanitized bodies + manifest.json and prints a LIVE/DOWN/DEGRADED
and SHAPE-OK/SHAPE-DRIFT table. A scenario with `replay_from = "DIR"` serves those bodies.
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p fake-llama-server`
Expected: all pass (http, process, scenario/unit, bin unit).

- [ ] **Step 7: Commit** — Close-out 1–4; subject `T0.4 (3/5): fake-llama-server binary — argv/env record, fixtures, OOM, crash`; push.

---

### Task 7: T0.4 (4/5) — R0 record, drift, sanitizing, replay; T0.4 done

**Files:**
- Modify: `tools/fake-llama-server/src/record.rs` (full implementation)
- Create: `tools/fake-llama-server/tests/record.rs`

**Interfaces:**
- Produces: `record::{ENDPOINTS, Liveness, Shape, EndpointRecord, Manifest, CaptureOptions, capture, key_paths, sanitize_home, redact, render_table, replay_file, cli}`.
  - `capture(&CaptureOptions) -> Result<Manifest, String>` writes `<out>/<file>` per non-404 endpoint + `<out>/manifest.json`.
  - `cli` exit codes: 0 `/health` LIVE · 1 otherwise · 2 usage/IO error.

- [ ] **Step 1: Failing tests** — `tests/record.rs`

```rust
//! R0 capture and replay (CR-2; REQ-TST-011).
use std::path::PathBuf;

use fake_llama_server::record::{CaptureOptions, Liveness, Shape, capture};
use fake_llama_server::{ModelsShape, Scenario, spawn};

fn tmpdir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("fake-rec-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn opts(upstream: String, out: PathBuf) -> CaptureOptions {
    CaptureOptions { upstream, out, baseline: None, bearer: None, slow_ms: 2000 }
}

fn liveness(m: &fake_llama_server::record::Manifest, path: &str) -> Liveness {
    m.endpoints.iter().find(|e| e.path == path).unwrap().liveness
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn capture_saves_bodies_manifest_and_liveness() {
    let f = spawn(Scenario::default().with_model("A", true)).await;
    let out = tmpdir("basic");
    let m = capture(&opts(f.base_url(), out.clone())).await.unwrap();
    for p in ["/health", "/models", "/v1/models", "/slots", "/metrics"] {
        assert_eq!(liveness(&m, p), Liveness::Live, "{p}");
    }
    assert_eq!(liveness(&m, "/props"), Liveness::Absent);
    for file in ["health.json", "models.json", "v1_models.json", "slots.json", "metrics.txt", "manifest.json"] {
        assert!(out.join(file).exists(), "{file}");
    }
    assert!(!out.join("props.json").exists());
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn capture_issues_only_get_requests() {
    let f = spawn(Scenario::default().with_model("A", true)).await;
    capture(&opts(f.base_url(), tmpdir("get"))).await.unwrap();
    let methods: Vec<String> = f.recorder.of_kind("request").iter().map(|r| r["method"].as_str().unwrap().to_string()).collect();
    assert_eq!(methods.len(), 6);
    assert!(methods.iter().all(|m| m == "GET"), "{methods:?}");
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn capture_redacts_home_prompts_and_bearer() {
    let f = spawn(Scenario::default().with_model("/home/alice/models/q.gguf", true)).await;
    let out = tmpdir("redact");
    let mut o = opts(f.base_url(), out.clone());
    o.bearer = Some("sk-live-token".into());
    capture(&o).await.unwrap();
    assert_eq!(f.recorder.of_kind("request")[0]["has_authorization"], true);
    let mut all = String::new();
    for e in std::fs::read_dir(&out).unwrap() {
        all.push_str(&std::fs::read_to_string(e.unwrap().path()).unwrap());
    }
    assert!(all.contains("${HOME}/models/q.gguf"));
    assert!(!all.contains("/home/alice"));
    assert!(!all.contains("sk-live-token"));
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn drift_against_a_baseline_names_the_changed_key_paths() {
    let base = tmpdir("drift-base");
    let now = tmpdir("drift-now");
    let a = spawn(Scenario::default().with_model("A", true)).await;
    capture(&opts(a.base_url(), base.clone())).await.unwrap();
    let b = spawn(Scenario::default().with_model("A", true).with_shape(ModelsShape::Legacy)).await;
    let mut o = opts(b.base_url(), now);
    o.baseline = Some(base);
    let m = capture(&o).await.unwrap();
    let models = m.endpoints.iter().find(|e| e.path == "/models").unwrap();
    assert_eq!(models.shape, Shape::Drift { added: vec![], removed: vec!["data[].status.value".into()] });
    let health = m.endpoints.iter().find(|e| e.path == "/health").unwrap();
    assert_eq!(health.shape, Shape::Ok);
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn down_upstream_is_reported_not_raised() {
    let m = capture(&opts("http://127.0.0.1:1".into(), tmpdir("down"))).await.unwrap();
    assert!(m.endpoints.iter().all(|e| e.liveness == Liveness::Down));
}

/// Verifies: REQ-TST-011/AC1
#[tokio::test]
async fn replay_serves_captured_bytes_and_falls_back_for_the_rest() {
    let src = spawn(Scenario::default().with_model("A", true).with_shape(ModelsShape::Legacy)).await;
    let cap = tmpdir("replay");
    capture(&opts(src.base_url(), cap.clone())).await.unwrap();
    drop(src);
    let f = spawn(Scenario::default().with_replay(cap.clone())).await;
    let body = reqwest::get(format!("{}/models", f.base_url())).await.unwrap().bytes().await.unwrap();
    assert_eq!(body.as_ref(), std::fs::read(cap.join("models.json")).unwrap().as_slice());
    assert_eq!(reqwest::get(format!("{}/props", f.base_url())).await.unwrap().status(), 404);
    let empty = tmpdir("replay-empty");
    let g = spawn(Scenario::default().with_replay(empty)).await;
    assert_eq!(reqwest::get(format!("{}/health", g.base_url())).await.unwrap().status(), 200);
}
```

Append to `tests/process.rs`:
```rust
/// Verifies: REQ-TST-011/AC1
#[test]
fn record_cli_prints_a_liveness_table_and_exit_code() {
    let d = tmpdir("reccli");
    let (mut child, base) = start(&mut Command::new(BIN));
    let out = Command::new(BIN).args(["record", "--upstream", &base, "--out"]).arg(d.join("cap")).output().unwrap();
    child.kill().unwrap();
    child.wait().unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    let table = String::from_utf8(out.stdout).unwrap();
    assert!(table.lines().any(|l| l.starts_with("/health") && l.contains("LIVE")), "{table}");
    let out = Command::new(BIN).args(["record", "--upstream", "http://127.0.0.1:1", "--out"]).arg(d.join("down")).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let out = Command::new(BIN).args(["record"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p fake-llama-server --test record`
Expected: compile errors — `CaptureOptions`, `capture`, `Liveness`, `Shape` not found.

- [ ] **Step 3: Implement `src/record.rs`** (replaces the whole file)

```rust
//! R0 record/replay (CR-2): read-only capture of a live llama-server router, shape drift
//! against a baseline, and replay lookup for scenarios with `replay_from`.
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Endpoints R0 reads, and the file each is saved as. GET only — never load/unload/chat.
pub const ENDPOINTS: &[(&str, &str)] = &[
    ("/health", "health.json"),
    ("/models", "models.json"),
    ("/v1/models", "v1_models.json"),
    ("/props", "props.json"),
    ("/slots", "slots.json"),
    ("/metrics", "metrics.txt"),
];
/// String fields that may carry user text; their values are replaced before saving.
const REDACT_KEYS: &[&str] = &["prompt", "content", "text", "generated", "generated_text"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Liveness {
    Live,
    Degraded,
    Down,
    Absent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Shape {
    Ok,
    Drift { added: Vec<String>, removed: Vec<String> },
    NoBaseline,
    NotJson,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EndpointRecord {
    pub path: String,
    pub file: Option<String>,
    pub status: Option<u16>,
    pub latency_ms: u64,
    pub content_type: Option<String>,
    pub liveness: Liveness,
    pub shape: Shape,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub captured_unix: u64,
    pub upstream: String,
    pub endpoints: Vec<EndpointRecord>,
}

pub struct CaptureOptions {
    pub upstream: String,
    pub out: PathBuf,
    pub baseline: Option<PathBuf>,
    /// Sent as `Authorization: Bearer`; never written anywhere.
    pub bearer: Option<String>,
    pub slow_ms: u64,
}

pub async fn capture(opts: &CaptureOptions) -> Result<Manifest, String> {
    std::fs::create_dir_all(&opts.out).map_err(|e| format!("create {}: {e}", opts.out.display()))?;
    let http = reqwest::Client::builder().timeout(Duration::from_secs(10)).build().map_err(|e| e.to_string())?;
    let mut endpoints = Vec::new();
    for (path, file) in ENDPOINTS {
        let mut rb = http.get(format!("{}{}", opts.upstream.trim_end_matches('/'), path));
        if let Some(t) = &opts.bearer {
            rb = rb.bearer_auth(t);
        }
        let t0 = Instant::now();
        let rec = match rb.send().await {
            Err(_) => EndpointRecord {
                path: (*path).into(), file: None, status: None, latency_ms: t0.elapsed().as_millis() as u64,
                content_type: None, liveness: Liveness::Down, shape: Shape::NotJson,
            },
            Ok(resp) => {
                let status = resp.status().as_u16();
                let content_type = resp.headers().get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok()).map(str::to_string);
                let body = resp.bytes().await.map_err(|e| format!("{path}: {e}"))?;
                let latency_ms = t0.elapsed().as_millis() as u64;
                if status == 404 {
                    EndpointRecord { path: (*path).into(), file: None, status: Some(404), latency_ms,
                        content_type, liveness: Liveness::Absent, shape: Shape::NotJson }
                } else {
                    let text = sanitize_home(&String::from_utf8_lossy(&body));
                    let saved = match serde_json::from_str::<Value>(&text) {
                        Ok(mut v) => {
                            redact(&mut v);
                            serde_json::to_string_pretty(&v).map_err(|e| e.to_string())? + "\n"
                        }
                        Err(_) => text,
                    };
                    std::fs::write(opts.out.join(file), &saved).map_err(|e| format!("write {file}: {e}"))?;
                    let liveness = if (200..300).contains(&status) && latency_ms <= opts.slow_ms {
                        Liveness::Live
                    } else {
                        Liveness::Degraded
                    };
                    let shape = shape_vs_baseline(&saved, opts.baseline.as_deref(), file);
                    EndpointRecord { path: (*path).into(), file: Some((*file).into()), status: Some(status),
                        latency_ms, content_type, liveness, shape }
                }
            }
        };
        endpoints.push(rec);
    }
    let captured_unix = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let m = Manifest { captured_unix, upstream: opts.upstream.clone(), endpoints };
    let text = serde_json::to_string_pretty(&m).map_err(|e| e.to_string())? + "\n";
    std::fs::write(opts.out.join("manifest.json"), text).map_err(|e| format!("write manifest: {e}"))?;
    Ok(m)
}

fn shape_vs_baseline(text: &str, baseline: Option<&Path>, file: &str) -> Shape {
    let Ok(cur) = serde_json::from_str::<Value>(text) else { return Shape::NotJson };
    let Some(dir) = baseline else { return Shape::NoBaseline };
    let Some(prev) = std::fs::read_to_string(dir.join(file)).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok())
    else {
        return Shape::NoBaseline;
    };
    let (now, before) = (key_paths(&cur), key_paths(&prev));
    let added: Vec<String> = now.difference(&before).cloned().collect();
    let removed: Vec<String> = before.difference(&now).cloned().collect();
    if added.is_empty() && removed.is_empty() { Shape::Ok } else { Shape::Drift { added, removed } }
}

/// Every key path in `v`; array elements are merged under `name[]`.
pub fn key_paths(v: &Value) -> BTreeSet<String> {
    fn walk(v: &Value, at: String, out: &mut BTreeSet<String>) {
        match v {
            Value::Object(m) => {
                if !at.is_empty() {
                    out.insert(at.clone());
                }
                for (k, x) in m {
                    walk(x, if at.is_empty() { k.clone() } else { format!("{at}.{k}") }, out);
                }
            }
            Value::Array(a) => {
                let p = format!("{at}[]");
                out.insert(p.clone());
                for x in a {
                    walk(x, p.clone(), out);
                }
            }
            _ => {
                out.insert(at);
            }
        }
    }
    let mut out = BTreeSet::new();
    walk(v, String::new(), &mut out);
    out
}

/// Replace `/home/<user>/` with `${HOME}/`.
pub fn sanitize_home(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("/home/") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 6..];
        match after.find('/') {
            Some(j) if j > 0 && !after[..j].contains(|c: char| c == '"' || c.is_whitespace()) => {
                out.push_str("${HOME}/");
                rest = &after[j + 1..];
            }
            _ => {
                out.push_str("/home/");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Replace string values of user-text keys (prompts, generated text) with `<redacted>`.
pub fn redact(v: &mut Value) {
    match v {
        Value::Object(m) => {
            for (k, x) in m.iter_mut() {
                if x.is_string() && REDACT_KEYS.contains(&k.as_str()) {
                    *x = Value::String("<redacted>".into());
                } else {
                    redact(x);
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(redact),
        _ => {}
    }
}

pub fn render_table(m: &Manifest) -> String {
    let mut s = format!("{:<12} {:>6} {:>7}  {:<9} {}\n", "ENDPOINT", "STATUS", "MS", "LIVENESS", "SHAPE");
    for e in &m.endpoints {
        let status = e.status.map(|c| c.to_string()).unwrap_or_else(|| "-".into());
        let live = match e.liveness {
            Liveness::Live => "LIVE",
            Liveness::Degraded => "DEGRADED",
            Liveness::Down => "DOWN",
            Liveness::Absent => "absent",
        };
        let shape = match &e.shape {
            Shape::Ok => "SHAPE-OK".to_string(),
            Shape::Drift { added, removed } => format!("SHAPE-DRIFT +{added:?} -{removed:?}"),
            Shape::NoBaseline => "no-baseline".into(),
            Shape::NotJson => "-".into(),
        };
        s.push_str(&format!("{:<12} {:>6} {:>7}  {:<9} {}\n", e.path, status, e.latency_ms, live, shape));
    }
    s
}

/// Recorded (status, content type, body) for `path` in the capture at `dir`, if any.
pub fn replay_file(dir: &Path, path: &str) -> Option<(u16, String, Vec<u8>)> {
    let text = std::fs::read_to_string(dir.join("manifest.json")).ok()?;
    let m: Manifest = serde_json::from_str(&text).ok()?;
    let e = m.endpoints.into_iter().find(|e| e.path == path)?;
    let file = e.file?;
    let bytes = std::fs::read(dir.join(&file)).ok()?;
    let ct = e.content_type.unwrap_or_else(|| {
        if file.ends_with(".txt") { "text/plain".into() } else { "application/json".into() }
    });
    Some((e.status.unwrap_or(200), ct, bytes))
}

const USAGE: &str =
    "usage: fake-llama-server record --out DIR [--upstream URL] [--baseline DIR] [--bearer-env VAR] [--slow-ms N]";

/// `fake-llama-server record …`; exit 0 when /health is LIVE, 1 otherwise, 2 on usage/IO errors.
pub async fn cli(args: &[String]) -> i32 {
    let mut upstream = "http://127.0.0.1:8080".to_string();
    let (mut out, mut baseline, mut bearer_env, mut slow_ms) = (None, None, None, 2000u64);
    let mut i = 0;
    while i < args.len() {
        match (args[i].as_str(), args.get(i + 1).cloned()) {
            ("--upstream", Some(v)) => upstream = v,
            ("--out", Some(v)) => out = Some(PathBuf::from(v)),
            ("--baseline", Some(v)) => baseline = Some(PathBuf::from(v)),
            ("--bearer-env", Some(v)) => bearer_env = Some(v),
            ("--slow-ms", Some(v)) => match v.parse() {
                Ok(n) => slow_ms = n,
                Err(_) => {
                    eprintln!("record: --slow-ms needs a number\n{USAGE}");
                    return 2;
                }
            },
            (other, _) => {
                eprintln!("record: unknown or incomplete argument '{other}'\n{USAGE}");
                return 2;
            }
        }
        i += 2;
    }
    let Some(out) = out else {
        eprintln!("record: --out is required\n{USAGE}");
        return 2;
    };
    let bearer = match bearer_env {
        Some(name) => match std::env::var(&name) {
            Ok(t) if !t.is_empty() => Some(t),
            Ok(_) => None,
            Err(_) => {
                eprintln!("record: environment variable {name} is not set");
                return 2;
            }
        },
        None => None,
    };
    match capture(&CaptureOptions { upstream, out, baseline, bearer, slow_ms }).await {
        Ok(m) => {
            print!("{}", render_table(&m));
            let live = m.endpoints.iter().any(|e| e.path == "/health" && e.liveness == Liveness::Live);
            if live { 0 } else { 1 }
        }
        Err(e) => {
            eprintln!("record: {e}");
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Verifies: REQ-TST-011/AC1
    #[test]
    fn key_paths_merge_array_elements() {
        let got: Vec<String> = key_paths(&json!({"a": {"b": [{"c": 1}, {"c": 2, "d": null}]}})).into_iter().collect();
        assert_eq!(got, vec!["a", "a.b[]", "a.b[].c", "a.b[].d"]);
    }

    /// Verifies: REQ-TST-011/AC1
    #[test]
    fn sanitize_home_rewrites_only_real_home_paths() {
        assert_eq!(
            sanitize_home(r#"path /home/laz/ai-models/x and /home/ alone and "/home/bob/q""#),
            r#"path ${HOME}/ai-models/x and /home/ alone and "${HOME}/q""#
        );
    }

    /// Verifies: REQ-TST-011/AC1
    #[test]
    fn redact_replaces_user_text_but_keeps_shape() {
        let mut v = json!({"slots": [{"id": 0, "prompt": "secret plan", "n_ctx": 4096, "content": ["x"]}]});
        redact(&mut v);
        assert_eq!(v, json!({"slots": [{"id": 0, "prompt": "<redacted>", "n_ctx": 4096, "content": ["x"]}]}));
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p fake-llama-server` (twice)
Expected: all pass. Also confirm no `501`/"not implemented" text remains: `grep -rn "not implemented" tools/fake-llama-server/src` → no output.

- [ ] **Step 5: Tick T0.4 and commit** — Close-out 1–4; subject `T0.4 (4/5): R0 record with drift, sanitizing and replay; fake runtime complete`; PROGRESS line `… · T0.4 · <sha> · DONE · fake-llama-server lib+bin, faults, recorder, R0 record/replay (CR-2)`; tick `T0.4`; amend; push.

---

### Task 8: T0.4 (5/5) / T0.3 close — R0 baseline capture of the live router, BD-32

**Files:**
- Create: `tests/fixtures/captures/baseline/r0/*` (from the live router, read-only)
- Modify: `docs/specs/vnext/baseline/DEFECTS.md` (BD-32), `docs/specs/vnext/tasks.md` (tick T0.3, remove its BLOCKED line)

- [ ] **Step 1: Is the router up?** (read-only GET)

Run via `ctx_execute`: `curl -s -o /dev/null -w '%{http_code}\n' --max-time 3 http://127.0.0.1:8080/health`
Expected: `200`. If not 200: write `[!] BLOCKED: T0.3 — BD-32 needs the live router up for R0. Need: operator starts llama-router.` in PROGRESS and stop this task (continue with Task 9; return here later).

- [ ] **Step 2: Capture** (bearer read from the live config in-process; never echoed)

```bash
cargo build --locked -p fake-llama-server
RC_BEARER="$(python3 -c "import tomllib,os;print(tomllib.load(open(os.path.expanduser('~/.config/saltnitor/config.toml'),'rb')).get('infer_bearer') or '')")" \
  target/debug/fake-llama-server record --upstream http://127.0.0.1:8080 \
  --out tests/fixtures/captures/baseline/r0 --bearer-env RC_BEARER
```
Expected: table with `/health LIVE`; exit 0.

- [ ] **Step 3: Check the capture is clean**

Run: `grep -rnE '/home/[a-z]|sk-|Bearer' tests/fixtures/captures/ ; echo "grep-exit=$?"`
Expected: no matches, `grep-exit=1`. Inspect `v1_models.json` for the model entry shape (via `ctx_execute_file`, print only `data[0]` keys and `status`).

- [ ] **Step 4: Decide BD-32**

`router_loaded()` (src/control_api.rs:101–103) treats a model as loaded only if `loaded == true` or `state == "loaded"` or `status == "loaded"` (string). If the captured `v1_models.json` has `"status": {"value": "loaded"}` (an object) → BD-32 **confirmed**. If it has a string `status`/`state`/boolean `loaded` → **disputed**. Update the BD-32 entry:
```markdown
- **Evidence (c89f278):** `src/control_api.rs:101` — string/bool checks only; live sample `tests/fixtures/captures/baseline/r0/v1_models.json`
- **Repro:** `cargo test bd32_status_object_not_recognised -- --ignored --nocapture` (T0.6) and the R0 capture above.
- **Status:** confirmed — the live router reports `"status": {"value": "loaded"}` (captured 2026-09-28), which none of the three checks matches.
```
(Adjust the Status text to what the capture actually shows.)

- [ ] **Step 5: Verify, tick, commit**

Run: `python3 docs/specs/vnext/tools/check_baseline_docs.py defects` → `defects: 32/32 complete`.
Remove T0.3's `[!] BLOCKED` line; tick T0.3. Close-out 1–4, then:
```bash
git add tests/fixtures/captures/baseline/r0 docs/specs/vnext/baseline/DEFECTS.md docs/specs/vnext/tasks.md
git commit -F - <<'MSG'
T0.3: close BD-32 from the R0 baseline capture of the live router

Task: T0.3
Refs: REQ-MIG-006, REQ-TST-011

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
MSG
```
PROGRESS `… · T0.3 · <sha> · DONE · BD-32 <confirmed|disputed> from live /v1/models`; amend; push.

---

### Task 9: T0.5 — Characterization: pure helpers

**Files:**
- Modify (append only): `src/main.rs`, `src/control_api.rs`

**Interfaces:**
- Consumes: private fns `upsert_ini_section` (main.rs:110), `estimate_footprint`/`parse_params_b`/`parse_bpw` (control_api.rs:423–449), `Stage::from_outcome` (control_api.rs:220) — reachable from `super::*` in an appended test module.

- [ ] **Step 1: Append to `src/main.rs`** (after the last line; start with a blank line)

```rust

#[cfg(test)]
mod tests {
    use super::upsert_ini_section;

    fn kv(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[test]
    fn upsert_replaces_appends_and_keeps_comments_and_other_sections() {
        let ini = "[*]\nctx-size = 1\n\n[A]\nmodel = /m/a.gguf\nctx-size = 4096\n; ctx-size = 2\n# note\n\n[B]\nctx-size = 1\n";
        let got = upsert_ini_section(ini, "A", &kv(&[("ctx-size", "8192"), ("n-gpu-layers", "99")]));
        assert_eq!(
            got.as_deref(),
            Some("[*]\nctx-size = 1\n\n[A]\nmodel = /m/a.gguf\nctx-size = 8192\n; ctx-size = 2\n# note\n\nn-gpu-layers = 99\n[B]\nctx-size = 1\n")
        );
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[test]
    fn upsert_matches_keys_without_spaces_and_normalizes_the_line() {
        assert_eq!(upsert_ini_section("[A]\nctx-size=4096\n", "A", &kv(&[("ctx-size", "1")])).as_deref(), Some("[A]\nctx-size = 1\n"));
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[test]
    fn upsert_in_the_last_section_appends_and_ends_with_newline() {
        assert_eq!(upsert_ini_section("[A]\nmodel = x", "A", &kv(&[("n-gpu-layers", "99")])).as_deref(), Some("[A]\nmodel = x\nn-gpu-layers = 99\n"));
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[test]
    fn upsert_matches_a_header_with_surrounding_whitespace() {
        assert_eq!(upsert_ini_section("  [A]  \nk = 1\n", "A", &kv(&[("k", "2")])).as_deref(), Some("  [A]  \nk = 2\n"));
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[test]
    fn upsert_missing_section_is_none() {
        assert_eq!(upsert_ini_section("[A]\nk = 1\n", "B", &kv(&[("k", "2")])), None);
    }
}
```

- [ ] **Step 2: Append to `src/control_api.rs`** — the `tests` module (Task 10 appends more tests **inside this same module**, so keep its closing brace last)

```rust

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn meta(model: &str, offload: bool, v: Option<f64>, r: Option<f64>) -> ProfileMeta {
        ProfileMeta { model: model.into(), offload, est_vram_gb: v, est_ram_gb: r }
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[test]
    fn parse_params_b_reads_the_first_size_token() {
        for (name, want) in [
            ("Qwen3-30B-A3B-Q4_K_M.gguf", Some(30.0)),
            ("llama-2-7b-chat.Q8_0.gguf", Some(7.0)),
            ("gemma-3-270M.gguf", None),
            ("mistral.gguf", None),
            ("model-9000B.gguf", None),
        ] {
            assert_eq!(parse_params_b(name), want, "{name}");
        }
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[test]
    fn parse_bpw_maps_quant_names() {
        for (name, want) in [
            ("m-Q2_K.gguf", 2.6), ("m-Q3_K_M.gguf", 3.5), ("m-IQ3_XXS.gguf", 3.5), ("m-IQ4_XS.gguf", 4.3),
            ("m-Q4_K_M.gguf", 4.85), ("m-IQ4_NL.gguf", 4.85), ("m-Q5_K_M.gguf", 5.5), ("m-Q5_0.gguf", 5.5),
            ("m-Q6_K.gguf", 6.6), ("m-Q8_0.gguf", 8.5), ("m-F16.gguf", 16.0), ("m-BF16.gguf", 16.0),
            ("m.gguf", 5.0),
        ] {
            assert_eq!(parse_bpw(name), want, "{name}");
        }
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[test]
    fn estimate_footprint_overrides_and_heuristics() {
        assert_eq!(estimate_footprint(&meta("x-30B-Q4_K_M.gguf", true, Some(3.0), Some(4.0))), (3.0, 4.0));
        assert_eq!(estimate_footprint(&meta("llama-7b-Q8_0.gguf", false, None, None)), (8.9, 0.0));
        assert_eq!(estimate_footprint(&meta("Qwen3-30B-A3B-Q4_K_M.gguf", true, None, None)), (4.7, 15.5));
        assert_eq!(estimate_footprint(&meta("Qwen3-30B-A3B-Q4_K_M.gguf", true, Some(3.0), None)), (5.7, 15.5));
        assert_eq!(estimate_footprint(&meta("mystery.gguf", false, None, None)), (6.5, 0.0));
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[test]
    fn stage_from_outcome_builds_the_terminal_frame() {
        let v = |o| serde_json::to_value(Stage::from_outcome(o)).unwrap();
        assert_eq!(
            v(EnsureOutcome::Loaded { model: "A".into(), endpoint: "http://x/v1".into(), load_ms: 1234, vram_estimate_gb: Some(4.7) }),
            json!({"stage": "done", "status": "loaded", "model": "A", "endpoint": "http://x/v1", "load_ms": 1234, "vram_estimate_gb": 4.7})
        );
        assert_eq!(
            v(EnsureOutcome::AlreadyResident { model: "A".into(), endpoint: "http://x/v1".into() }),
            json!({"stage": "done", "status": "already_resident", "model": "A", "endpoint": "http://x/v1", "load_ms": null, "vram_estimate_gb": null})
        );
        assert_eq!(
            v(EnsureOutcome::Oom(OomInfo { need_vram_gb: 1.0, total_vram_gb: 2.0, need_ram_gb: 3.0, total_ram_gb: 4.0 })),
            json!({"stage": "oom", "detail": "need ~1.0GB VRAM (have 2.0) / ~3.0GB RAM (have 4.0); pass force=true"})
        );
        assert_eq!(v(EnsureOutcome::Bad("x".into())), json!({"stage": "error", "detail": "x"}));
        assert_eq!(v(EnsureOutcome::Err("y".into())), json!({"stage": "error", "detail": "y"}));
    }
}
```

Not pinned on purpose (possible defects, not in the BD register): `parse_params_b("Qwen2.5-0.5B")` returns `Some(5.0)` because `.` becomes a space; `parse_bpw("m-Q4_0.gguf")` returns the 5.0 default. Record both under a new `## Observations (not in the BD register)` heading at the end of `DEFECTS.md` with citations `src/control_api.rs:435` and `src/control_api.rs:444`, and file `CR-4` proposing them as BD-33/BD-34 (operator decides).

- [ ] **Step 3: Run**

Run: `cargo test --locked -p saltnitor tests::`
Expected: 9 tests pass.

- [ ] **Step 4: Mutation check (each must FAIL, then restore)**

```bash
mutate() { python3 -c 'import sys,pathlib; p=pathlib.Path(sys.argv[1]); t=p.read_text(); assert t.count(sys.argv[2])>=1, "pattern not found"; p.write_text(t.replace(sys.argv[2], sys.argv[3], 1))' "$@"; }
mutate src/control_api.rs 'up.contains("IQ4") { 4.85 }' 'up.contains("IQ4") { 4.8 }'
cargo test --locked -p saltnitor parse_bpw 2>&1 | grep -E 'test result|FAILED|panicked'   # expect FAILED (m-Q4_K_M.gguf)
mutate src/control_api.rs 'up.contains("IQ4") { 4.8 }' 'up.contains("IQ4") { 4.85 }'
mutate src/main.rs 'out.push(format!("{} = {}", k, v));' 'out.push(format!("{}={}", k, v));'
cargo test --locked -p saltnitor upsert 2>&1 | grep -E 'test result|FAILED|panicked'      # expect FAILED (replace/append tests)
mutate src/main.rs 'out.push(format!("{}={}", k, v));' 'out.push(format!("{} = {}", k, v));'
```
`mutate` replaces the first occurrence only (the replace-in-loop line comes before the append-remaining line in `upsert_ini_section`). Then run Close-out 3 — it must print `prefix check: OK`, proving every restore was byte-exact.

- [ ] **Step 5: Commit** — Close-out 1–4; subject `T0.5: characterize ini upsert, footprint heuristics and stage frames`, `Refs: REQ-MIG-002`; PROGRESS; tick T0.5; amend; push.

---

### Task 10: T0.6 — Characterization: control API against the fake

**Files:**
- Modify (append inside the existing `tests` module of `src/control_api.rs`)
- Create: `tests/fixtures/scenarios/control-api-legacy.toml`, `tests/fixtures/scenarios/bd02-slow-stream.toml`
- Modify: `docs/specs/vnext/baseline/DEFECTS.md` (BD-02 timing evidence)

**Interfaces:**
- Consumes: `fake_llama_server::{spawn, Scenario, Fault, Handle}` (Tasks 4–7); `ControlApi::new(profiles, router_base, infer_bearer, control_token, reserve_vram_gb, reserve_ram_gb, tx)`; `serve(Arc<ControlApi>, SocketAddr)`; `crate::events::Event::ActiveModelSet(String)`.

**Do not assert (REQ-MIG-002/AC3):** chat/status/models reachable without a token (BD-03); `status_object` shape treated as not loaded (BD-32); full buffering of chat (BD-02); exact VRAM/RAM numbers from `/v1/status` (host-dependent, D1).

- [ ] **Step 1: Scenario fixtures**

`tests/fixtures/scenarios/control-api-legacy.toml`:
```toml
# Control-API characterization. The router reports the legacy string status that
# router_loaded() understands at c89f278. BD-32 (status objects) is not pinned here.
models_shape = "legacy"
max_loaded = 1

[[models]]
id = "A"
loaded = true

[[models]]
id = "B"
loaded = false
```
`tests/fixtures/scenarios/bd02-slow-stream.toml`:
```toml
# BD-02 evidence probe: five SSE chunks, 200 ms apart, model A resident.
models_shape = "legacy"
max_loaded = 1

[[models]]
id = "A"
loaded = true

[routes."POST /v1/chat/completions"]
kind = "chunks"
items = ["t1", "t2", "t3", "t4", "t5"]
delay_ms = 200
```

- [ ] **Step 2: Append the rig and tests** inside `mod tests` of `src/control_api.rs` (before its closing `}`)

```rust
    use fake_llama_server::{Fault, Scenario};
    use serde_json::Value;

    const SCENARIOS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/scenarios");

    fn scenario(name: &str) -> Scenario {
        Scenario::from_file(std::path::Path::new(&format!("{SCENARIOS}/{name}"))).expect("scenario fixture")
    }
    /// Zero estimates + zero reserves: the oracle passes on any host, GPU or not (design D1).
    fn fits() -> ProfileMeta {
        meta("m-7b-Q4_K_M.gguf", false, Some(0.0), Some(0.0))
    }
    /// An estimate no machine has: the oracle rejects on any host (design D1).
    fn never_fits() -> ProfileMeta {
        meta("m-7b-Q4_K_M.gguf", false, Some(1.0e6), Some(0.0))
    }

    struct Rig {
        base: String,
        fake: fake_llama_server::Handle,
        events: mpsc::Receiver<Event>,
        http: reqwest::Client,
    }

    impl Rig {
        fn url(&self, path: &str) -> String {
            format!("{}{}", self.base, path)
        }
        fn endpoint(&self) -> String {
            format!("{}/v1", self.fake.base_url())
        }
        fn upstream_chats(&self) -> Vec<Value> {
            self.fake.recorder.of_kind("request").into_iter().filter(|r| r["path"] == "/v1/chat/completions").collect()
        }
        async fn post_json(&self, path: &str, body: Value) -> (u16, Value) {
            let r = self.http.post(self.url(path)).json(&body).send().await.unwrap();
            let s = r.status().as_u16();
            (s, r.json().await.unwrap_or(Value::Null))
        }
        async fn post_raw(&self, path: &str, body: &'static str) -> (u16, String, String) {
            let r = self.http.post(self.url(path)).header("Content-Type", "application/json").body(body).send().await.unwrap();
            let s = r.status().as_u16();
            let ct = r.headers().get("content-type").map(|v| v.to_str().unwrap().to_string()).unwrap_or_default();
            (s, ct, r.text().await.unwrap())
        }
    }

    /// Real ControlApi + real `serve()` route table on a free loopback port, in front of the fake.
    async fn rig(scenario: Scenario, profiles: &[(&str, ProfileMeta)], token: Option<&str>) -> Rig {
        let fake = fake_llama_server::spawn(scenario).await;
        let (tx, events) = mpsc::channel(256);
        let profiles = profiles.iter().map(|(k, p)| (k.to_string(), p.clone())).collect();
        let api = Arc::new(ControlApi::new(profiles, fake.base_url(), None, token.map(str::to_string), 0.0, 0.0, tx));
        let http = reqwest::Client::new();
        for _ in 0..20 {
            let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
            let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
            let task = tokio::spawn(serve(api.clone(), addr));
            let base = format!("http://{addr}");
            for _ in 0..50 {
                if let Ok(r) = http.get(format!("{base}/healthz")).send().await {
                    if r.status() == 200 {
                        return Rig { base, fake, events, http };
                    }
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            task.abort(); // port was taken between bind and serve; try another
        }
        panic!("control API did not come up");
    }

    fn stages(sse: &str) -> Vec<Value> {
        sse.lines().filter_map(|l| l.strip_prefix("data: ")).map(|d| serde_json::from_str(d).unwrap()).collect()
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2, REQ-TST-002/AC1
    #[tokio::test]
    async fn healthz_is_200() {
        let r = rig(scenario("control-api-legacy.toml"), &[("A", fits())], None).await;
        assert_eq!(r.http.get(r.url("/healthz")).send().await.unwrap().status(), 200);
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2, REQ-TST-002/AC1
    #[tokio::test]
    async fn models_lists_profile_ids_openai_style() {
        let r = rig(scenario("control-api-legacy.toml"), &[("A", fits()), ("B", fits())], None).await;
        let v: Value = r.http.get(r.url("/v1/models")).send().await.unwrap().json().await.unwrap();
        assert_eq!(v["object"], "list");
        let mut ids: Vec<&str> = v["data"].as_array().unwrap().iter().map(|m| m["id"].as_str().unwrap()).collect();
        ids.sort();
        assert_eq!(ids, ["A", "B"]);
        assert!(v["data"].as_array().unwrap().iter().all(|m| m["object"] == "model" && m["owned_by"] == "saltnitor"));
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[tokio::test]
    async fn status_reports_resident_models_and_numeric_telemetry() {
        let r = rig(scenario("control-api-legacy.toml"), &[("A", fits()), ("B", fits())], None).await;
        let v: Value = r.http.get(r.url("/v1/status")).send().await.unwrap().json().await.unwrap();
        assert_eq!(v["resident_models"], json!(["A"]));
        assert_eq!(v["endpoint"], json!(r.endpoint()));
        let mut profiles: Vec<&str> = v["profiles"].as_array().unwrap().iter().map(|p| p.as_str().unwrap()).collect();
        profiles.sort();
        assert_eq!(profiles, ["A", "B"]);
        for k in ["vram_used_gb", "vram_total_gb", "ram_free_gb"] {
            assert!(v[k].is_f64(), "{k} = {}", v[k]);
        }
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2, REQ-MIG-007/AC2
    #[tokio::test]
    async fn ensure_already_resident_skips_the_warm_load() {
        let r = rig(scenario("control-api-legacy.toml"), &[("A", fits())], None).await;
        let (s, v) = r.post_json("/v1/ensure", json!({"profile": "A"})).await;
        assert_eq!((s, v), (200, json!({"status": "already_resident", "model": "A", "endpoint": r.endpoint()})));
        assert!(r.upstream_chats().is_empty());
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2, REQ-MIG-007/AC2
    #[tokio::test]
    async fn ensure_loads_with_a_one_token_warm_request() {
        let mut r = rig(scenario("control-api-legacy.toml"), &[("A", fits()), ("B", fits())], None).await;
        let (s, v) = r.post_json("/v1/ensure", json!({"profile": "B"})).await;
        assert_eq!(s, 200);
        assert_eq!((v["status"].as_str(), v["model"].as_str()), (Some("loaded"), Some("B")));
        assert_eq!(v["endpoint"], json!(r.endpoint()));
        assert!(v["load_ms"].is_u64());
        assert_eq!(v["vram_estimate_gb"], json!(0.0));
        assert!(v.get("detail").is_none());
        let chats = r.upstream_chats();
        assert_eq!(chats.len(), 1);
        let warm: Value = serde_json::from_str(chats[0]["body"].as_str().unwrap()).unwrap();
        assert_eq!(warm, json!({"model": "B", "messages": [{"role": "user", "content": "warmup"}], "max_tokens": 1}));
        assert_eq!(r.fake.loaded(), vec!["B"]);
        let mut active = vec![];
        while let Ok(e) = r.events.try_recv() {
            if let Event::ActiveModelSet(m) = e {
                active.push(m);
            }
        }
        assert_eq!(active, ["B"]);
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[tokio::test]
    async fn ensure_unknown_profile_is_400() {
        let r = rig(scenario("control-api-legacy.toml"), &[("A", fits())], None).await;
        let (s, v) = r.post_json("/v1/ensure", json!({"profile": "Z"})).await;
        assert_eq!((s, v), (400, json!({"status": "bad_request", "model": "", "endpoint": r.endpoint(), "detail": "unknown profile 'Z'"})));
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2, REQ-MIG-007/AC2
    #[tokio::test]
    async fn ensure_oracle_rejects_without_loading() {
        let r = rig(scenario("control-api-legacy.toml"), &[("B", never_fits())], None).await;
        let (s, v) = r.post_json("/v1/ensure", json!({"profile": "B"})).await;
        assert_eq!(s, 507);
        assert_eq!((v["status"].as_str(), v["model"].as_str()), (Some("oom_rejected"), Some("")));
        assert_eq!(v["vram_estimate_gb"], json!(1.0e6));
        assert!(v["detail"].as_str().unwrap().starts_with("need ~1000000.0GB VRAM (have "), "{v}");
        assert!(r.upstream_chats().is_empty());
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[tokio::test]
    async fn ensure_force_bypasses_the_oracle() {
        let r = rig(scenario("control-api-legacy.toml"), &[("B", never_fits())], None).await;
        let (s, v) = r.post_json("/v1/ensure", json!({"profile": "B", "force": true})).await;
        assert_eq!((s, v["status"].as_str()), (200, Some("loaded")));
        assert_eq!(v["vram_estimate_gb"], json!(1.0e6));
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2, REQ-TST-002/AC1
    #[tokio::test]
    async fn ensure_reports_router_failure_as_503() {
        let sc = scenario("control-api-legacy.toml")
            .with_fault("POST /v1/chat/completions", Fault::Status { code: 500, body: "boom".into() });
        let r = rig(sc, &[("B", fits())], None).await;
        let (s, v) = r.post_json("/v1/ensure", json!({"profile": "B"})).await;
        assert_eq!(s, 503);
        assert_eq!(v["status"], "error");
        assert_eq!(v["detail"], "router load failed: router returned 500 Internal Server Error");
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2, REQ-TST-002/AC1
    #[tokio::test]
    async fn ensure_requires_the_bearer_when_a_token_is_set() {
        let r = rig(scenario("control-api-legacy.toml"), &[("A", fits())], Some("t0k")).await;
        let (s, v) = r.post_json("/v1/ensure", json!({"profile": "A"})).await;
        assert_eq!((s, v), (401, json!({"status": "error", "model": "", "endpoint": r.endpoint(), "detail": "bad token"})));
        let ok = r.http.post(r.url("/v1/ensure")).bearer_auth("t0k").json(&json!({"profile": "A"})).send().await.unwrap();
        assert_eq!(ok.status(), 200);
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[tokio::test]
    async fn ensure_stream_emits_stages_in_order() {
        let r = rig(scenario("control-api-legacy.toml"), &[("A", fits()), ("B", fits())], None).await;
        let text = r.http.get(r.url("/v1/ensure/stream?profile=B")).send().await.unwrap().text().await.unwrap();
        let st = stages(&text);
        let names: Vec<&str> = st.iter().map(|s| s["stage"].as_str().unwrap()).collect();
        assert_eq!(names, ["received", "oracle_ok", "loading", "done"]);
        assert_eq!(st[0], json!({"stage": "received", "profile": "B", "model": "B"}));
        assert_eq!(st[3]["status"], "loaded");
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[tokio::test]
    async fn ensure_stream_accepts_the_query_token() {
        let r = rig(scenario("control-api-legacy.toml"), &[("A", fits())], Some("t0k")).await;
        let ok = r.http.get(r.url("/v1/ensure/stream?profile=A&token=t0k")).send().await.unwrap();
        assert_eq!(ok.status(), 200);
        assert_eq!(stages(&ok.text().await.unwrap()).last().unwrap()["status"], "already_resident");
        let bad = r.http.get(r.url("/v1/ensure/stream?profile=A&token=nope")).send().await.unwrap();
        assert_eq!((bad.status().as_u16(), bad.text().await.unwrap()), (401, "bad token".to_string()));
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2, REQ-MIG-007/AC1, REQ-TST-002/AC1
    #[tokio::test]
    async fn chat_forwards_the_body_unchanged_after_ensure() {
        let r = rig(scenario("control-api-legacy.toml"), &[("A", fits())], None).await;
        const BODY: &str = r#"{"model":"A","messages":[{"role":"user","content":"hi"}]}"#;
        let (s, _, text) = r.post_raw("/v1/chat/completions", BODY).await;
        assert_eq!(s, 200);
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["choices"][0]["message"]["content"], "Hello from fake");
        let chats = r.upstream_chats();
        assert_eq!(chats.len(), 1);
        assert_eq!(chats[0]["body"], BODY);
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2, REQ-MIG-007/AC1
    #[tokio::test]
    async fn chat_hot_swaps_a_non_resident_model_first() {
        let r = rig(scenario("control-api-legacy.toml"), &[("A", fits()), ("B", fits())], None).await;
        const BODY: &str = r#"{"model":"B","messages":[{"role":"user","content":"hi"}]}"#;
        assert_eq!(r.post_raw("/v1/chat/completions", BODY).await.0, 200);
        let chats = r.upstream_chats();
        assert_eq!(chats.len(), 2);
        let warm: Value = serde_json::from_str(chats[0]["body"].as_str().unwrap()).unwrap();
        assert_eq!(warm["max_tokens"], 1);
        assert_eq!(chats[1]["body"], BODY);
        assert_eq!(r.fake.loaded(), vec!["B"]);
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[tokio::test]
    async fn chat_without_or_with_unknown_model_is_rejected() {
        let r = rig(scenario("control-api-legacy.toml"), &[("A", fits())], None).await;
        let (s, _, text) = r.post_raw("/v1/chat/completions", r#"{"messages":[]}"#).await;
        assert_eq!((s, text.as_str()), (400, "missing 'model' in request body"));
        let (s, _, text) = r.post_raw("/v1/chat/completions", r#"{"model":"Z","messages":[]}"#).await;
        assert_eq!((s, text.as_str()), (404, "unknown model 'Z': unknown profile 'Z'"));
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2, REQ-TST-002/AC1
    #[tokio::test]
    async fn chat_passes_upstream_5xx_through() {
        let sc = scenario("control-api-legacy.toml")
            .with_fault("POST /v1/chat/completions", Fault::Status { code: 503, body: r#"{"error":"busy"}"#.into() });
        let r = rig(sc, &[("A", fits())], None).await;
        let (s, ct, text) = r.post_raw("/v1/chat/completions", r#"{"model":"A","messages":[]}"#).await;
        assert_eq!((s, ct.as_str(), text.as_str()), (503, "application/json", r#"{"error":"busy"}"#));
    }

    /// Verifies: REQ-MIG-002/AC1, REQ-MIG-002/AC2
    #[tokio::test]
    async fn chat_oracle_reject_is_503() {
        let r = rig(scenario("control-api-legacy.toml"), &[("B", never_fits())], None).await;
        let (s, _, text) = r.post_raw("/v1/chat/completions", r#"{"model":"B","messages":[]}"#).await;
        assert_eq!(s, 503);
        assert!(text.starts_with("saltnitor oracle: 'B' needs ~1000000.0GB VRAM"), "{text}");
    }

    /// BD-02 evidence probe — measures, asserts nothing about buffering (REQ-MIG-002/AC3).
    #[tokio::test]
    #[ignore = "BD-02 evidence probe: cargo test bd02_first_byte_timing -- --ignored --nocapture"]
    async fn bd02_first_byte_timing() {
        async fn first_byte(http: &reqwest::Client, url: String) -> (u16, u128, u128) {
            let t0 = std::time::Instant::now();
            let mut resp = http.post(url).header("Content-Type", "application/json")
                .body(r#"{"model":"A","stream":true,"messages":[]}"#).send().await.unwrap();
            let status = resp.status().as_u16();
            let mut first = None;
            while let Some(_c) = resp.chunk().await.unwrap() {
                first.get_or_insert(t0.elapsed().as_millis());
            }
            (status, first.unwrap_or(0), t0.elapsed().as_millis())
        }
        let r = rig(scenario("bd02-slow-stream.toml"), &[("A", fits())], None).await;
        let direct = first_byte(&r.http, format!("{}/v1/chat/completions", r.fake.base_url())).await;
        let via = first_byte(&r.http, r.url("/v1/chat/completions")).await;
        println!("BD-02 direct: status={} first_byte_ms={} total_ms={}", direct.0, direct.1, direct.2);
        println!("BD-02 via saltnitor: status={} first_byte_ms={} total_ms={}", via.0, via.1, via.2);
        assert_eq!((direct.0, via.0), (200, 200));
    }

    /// BD-32 evidence probe — shows what status reports for the current upstream shape.
    #[tokio::test]
    #[ignore = "BD-32 evidence probe: cargo test bd32_status_object_not_recognised -- --ignored --nocapture"]
    async fn bd32_status_object_not_recognised() {
        let sc = Scenario::default().with_model("A", true); // default = status objects
        let r = rig(sc, &[("A", fits())], None).await;
        let v: Value = r.http.get(r.url("/v1/status")).send().await.unwrap().json().await.unwrap();
        println!("BD-32: fake reports A loaded (status object); saltnitor resident_models = {}", v["resident_models"]);
    }
```

- [ ] **Step 3: Run**

Run: `cargo test --locked -p saltnitor control_api::tests` → all non-ignored pass (run twice).

- [ ] **Step 4: Mutation check** (each must FAIL, then restore with the reverse `mutate`; `mutate` as defined in Task 9 Step 4)

1. `mutate src/control_api.rs '(StatusCode::INSUFFICIENT_STORAGE, Json' '(StatusCode::OK, Json'` → `cargo test --locked -p saltnitor ensure_oracle_rejects_without_loading` FAILS.
2. `mutate src/control_api.rs '.header("Content-Type", "application/json").body(body);' '.header("Content-Type", "application/json").body(Bytes::from_static(b"{}"));'` → `chat_forwards_the_body_unchanged_after_ensure` FAILS.
3. `mutate src/control_api.rs 'sink.emit(Stage::Loading { model: req.profile.clone() }).await;' 'sink.emit(Stage::OracleOk { vram_estimate_gb: 0.0 }).await;'` → `ensure_stream_emits_stages_in_order` FAILS.

After restoring all three, Close-out 3 must print `prefix check: OK`.

- [ ] **Step 5: BD-02 evidence**

Run: `cargo test --locked -p saltnitor bd02_first_byte_timing -- --ignored --nocapture` and `… bd32_status_object_not_recognised -- --ignored --nocapture`.
Append to the BD-02 entry in DEFECTS.md: `- **Timing (2026-09-28, fake, 5×200 ms chunks):** direct first byte <d> ms, through Saltnitor <v> ms, total <t> ms.` (numbers from the output). Append the printed BD-32 line to the BD-32 entry as `- **Probe:** …`.

- [ ] **Step 6: Commit** — Close-out 1–4; add `src/control_api.rs tests/fixtures/scenarios docs/specs/vnext/baseline/DEFECTS.md`; subject `T0.6: characterize control API routes against the fake runtime`; `Refs: REQ-MIG-002, REQ-TST-002, REQ-MIG-007`; PROGRESS; tick T0.6; amend; push.

---

### Task 11: T0.7 — TUI visual baseline

**Files:**
- Modify: `Cargo.toml` (dev-dep `insta = "1.48.0"`), append test module to `src/ui.rs`
- Create: `src/snapshots/*.snap` (11 files)

- [ ] **Step 1: Add insta** — in root `Cargo.toml` `[dev-dependencies]` add `insta = "1.48.0"`; run `cargo build --workspace --tests`.

- [ ] **Step 2: Append to `src/ui.rs`**

```rust

#[cfg(test)]
mod tests {
    use super::draw;
    use crate::app::App;
    use ratatui::{Terminal, backend::TestBackend};

    /// Fixed data; every host-dependent field is overwritten (history is read from the CWD in App::new).
    fn fixture() -> App {
        let mut app = App::new(
            "Fixture CPU 8-Core".into(), 16, 32.0, "Fixture GPU 12GB".into(), 12.0, true,
            "127.0.0.1".into(), 8080, "llama-router".into(), 33, 8192,
        );
        app.console_history = Vec::new();
        app.history_index = 0;
        app.vram_used = 6.5;
        app.ram_used = 12.25;
        app.cpu_history = (0..100).map(|i| (i * 7 % 100) as u64).collect();
        app.cpu_cores = (0..16).map(|i| (i * 6) as f32).collect();
        app.gpu_temp = 55;
        app.gpu_power = "120W".into();
        app.gpu_util = "40%".into();
        app.vram_util = "54%".into();
        app.gpu_fan = "30%".into();
        app.gpu_clocks = "1800 MHz".into();
        app.gpu_processes = vec![("llama-server".into(), 6.1)];
        app.sys_processes = vec![("llama-server".into(), 9.5), ("saltnitor".into(), 0.1)];
        app.swap_used = 0.5;
        app.swap_total = 8.0;
        app.sys_uptime = 3661;
        app.port_status = "Port 8080: LISTENING".into();
        app.available_models = vec!["A_STD".into(), "A_FOCUS".into(), "B".into()];
        app.active_model = "A_STD".into();
        app.hot_swap_state.select(Some(0));
        for line in ["router started", "model A_STD loaded", "request served"] {
            app.add_log(line.to_string());
        }
        app
    }

    fn render(app: &mut App, w: u16, h: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| draw(f, app)).unwrap();
        // Debug form = text lines + style runs (fg/bg/modifier), so color-only meaning is pinned too;
        // the Display form would capture text only.
        format!("{:?}", t.backend().buffer())
    }

    /// Verifies: REQ-MIG-005/AC1, REQ-TUI-009/AC1, REQ-MIG-007/AC3
    #[test]
    fn dashboard_with_interrogator_deck() {
        insta::assert_snapshot!("dashboard", render(&mut fixture(), 100, 30));
    }

    /// Verifies: REQ-MIG-005/AC1, REQ-TUI-009/AC1, REQ-MIG-007/AC3
    #[test]
    fn hot_swap_deck() {
        let mut app = fixture();
        app.bottom_tab_mode = 1;
        insta::assert_snapshot!("deck_hot_swap", render(&mut app, 100, 30));
    }

    /// Verifies: REQ-MIG-005/AC1, REQ-TUI-009/AC1, REQ-MIG-007/AC3
    #[test]
    fn tuner_pages() {
        for page in 0..3 {
            let mut app = fixture();
            app.show_tuner = true;
            app.tuner_page = page;
            let name = format!("tuner_page_{}", page + 1);
            insta::assert_snapshot!(name.as_str(), render(&mut app, 100, 30));
        }
    }

    /// Verifies: REQ-MIG-005/AC1, REQ-TUI-009/AC1, REQ-MIG-007/AC3
    #[test]
    fn gpu_and_cpu_inspectors() {
        let mut app = fixture();
        app.show_gpu_inspector = true;
        insta::assert_snapshot!("gpu_inspector", render(&mut app, 100, 30));
        let mut app = fixture();
        app.show_sys_inspector = true;
        insta::assert_snapshot!("cpu_inspector", render(&mut app, 100, 30));
    }

    /// Verifies: REQ-MIG-005/AC1, REQ-TUI-009/AC1, REQ-MIG-007/AC3
    #[test]
    fn help_and_search() {
        let mut app = fixture();
        app.show_help = true;
        insta::assert_snapshot!("help", render(&mut app, 100, 30));
        let mut app = fixture();
        app.is_searching = true;
        app.search_query = "loaded".into();
        insta::assert_snapshot!("search", render(&mut app, 100, 30));
    }

    /// Verifies: REQ-MIG-005/AC1, REQ-TUI-009/AC1, REQ-MIG-007/AC4
    #[test]
    fn too_small_terminal_is_refused() {
        insta::assert_snapshot!("too_small_79x16", render(&mut fixture(), 79, 16));
        insta::assert_snapshot!("too_small_80x15", render(&mut fixture(), 80, 15));
    }
}
```

- [ ] **Step 3: Create snapshots, then review them**

Run: `INSTA_UPDATE=unseen cargo test --locked -p saltnitor ui::tests` (`unseen` writes only snapshots that do not exist yet; a changed existing snapshot goes to `.snap.new` for review instead of being overwritten — insta 1.48 docs)
Then read each of the 11 files in `src/snapshots/` (`saltnitor__ui__tests__*.snap`). Check: every snapshot has both a `content: [` block and a `styles: [` block (text **and** colors are pinned); the refusal snapshots' styles show `fg: Red` with `BOLD | RAPID_BLINK`; the dashboard shows " API Interrogator "; the hot-swap deck lists `A_STD`, `A_FOCUS`, `B`; tuner titles show `[Page 1/3]`…`[Page 3/3]`; both refusal snapshots contain `TERMINAL FOOTPRINT TOO SMALL`; no snapshot contains this machine's CPU/GPU name or a real history line.

- [ ] **Step 4: Snapshots are stable**

Run: `INSTA_UPDATE=no cargo test --locked -p saltnitor ui::tests` twice → pass; `ls src/snapshots/*.snap.new 2>/dev/null` → nothing.

- [ ] **Step 5: Mutation check** — change `" API Interrogator "` (src/ui.rs:222) to `" API Interrogatorx "` → `INSTA_UPDATE=no cargo test -p saltnitor dashboard_with_interrogator_deck` FAILS; restore; delete any `.snap.new`; prefix check OK.

- [ ] **Step 6: Commit** — Close-out 1–4; add `Cargo.toml src/ui.rs src/snapshots`; subject `T0.7: TUI snapshot baseline for every view and the size refusal`; `Refs: REQ-MIG-005, REQ-TUI-009, REQ-MIG-007`; body line: `New crate: insta 1.48 (Apache-2.0), named by REQ-TUI-009 for snapshot tests.`; PROGRESS; tick T0.7; amend; push. Snapshot approval (REQ-TUI-009/AC1) is the operator's review of this commit's diff — note it in the PROGRESS line.

---

### Task 12: T0.8 — Router fixtures

**Files:**
- Create: `tests/fixtures/router/known-good.ini`, `tests/fixtures/router/readme-example.ini`

- [ ] **Step 1: Sanitize the live router.ini** (read-only source)

```bash
mkdir -p tests/fixtures/router
python3 - <<'PY'
import pathlib, re
src = pathlib.Path.home() / "ai-models/llama.cpp/router.ini"
out = []
for line in src.read_text().splitlines():
    line = re.sub(r"/home/[^/\s]+/ai-models/", "${MODELS_DIR}/", line)
    line = re.sub(r"/home/[^/\s]+/", "${HOME}/", line)
    if re.match(r"\s*(api[-_]key|hf[-_]token|bearer)\s*=", line, re.I):
        line = re.sub(r"=.*", "= <redacted>", line)
    out.append(line)
header = "; Sanitized copy of the operator's working router.ini (c89f278 era). REQ-MIG-004.\n; ${MODELS_DIR} = the models root; ${HOME} = the user's home.\n"
pathlib.Path("tests/fixtures/router/known-good.ini").write_text(header + "\n".join(out) + "\n")
PY
```

- [ ] **Step 2: README example**

```bash
python3 - <<'PY'
import pathlib, re
t = pathlib.Path("README.md").read_text()
m = re.search(r"```ini\n(.*?)```", t, re.S)
assert m, "README has no ```ini block"
pathlib.Path("tests/fixtures/router/readme-example.ini").write_text(
    "; router.ini example from README.md at c89f278 (REQ-MIG-004).\n" + m.group(1))
PY
```

- [ ] **Step 3: Done-when check**

Run: `grep -n "/home/" tests/fixtures/router/*; echo "exit=$?"; grep -niE 'sk-|api[-_]key *= *[^<]' tests/fixtures/router/*; echo "exit=$?"`
Expected: both `exit=1` (no matches). Compare section names: `grep -n '^\[' tests/fixtures/router/known-good.ini` shows `[*]`, `[A_STD]`, `[A_FOCUS]`, `[B]`.

- [ ] **Step 4: Commit** — Close-out 1–2; subject `T0.8: sanitized known-good and README router fixtures`; `Refs: REQ-MIG-004`; PROGRESS; tick T0.8 (the `[HUMAN]` part — supplying the file — was replaced by the agent's read-only copy per design D2; the operator approves via this diff; say so in PROGRESS); amend; push.

---

### Task 13: T0.9 — Spec lint installed and traceability green

**Files:**
- Modify: `docs/specs/vnext/requirements.md`, `docs/specs/vnext/tasks.md` (only via `--sync`, if it changes anything)

- [ ] **Step 1: Sync generated fields**

Run: `python3 docs/specs/vnext/tools/spec_lint.py --sync && git diff --stat docs/specs/vnext`
Expected: no diff, or only regenerated `Verify`/`Phase`/`Tasks` fields. Review any diff line by line.

- [ ] **Step 2: Traceability for P0**

Run: `python3 docs/specs/vnext/tools/spec_lint.py --tests --phase P0; echo EXIT=$?`
Expected: `spec_lint: OK`, `EXIT=0` — every due P0 MUST (T) AC (REQ-MIG-002/AC1–2, REQ-MIG-005/AC1, REQ-MIG-007/AC1–4, REQ-TST-002/AC1, REQ-TST-011/AC1, REQ-TUI-009/AC1) is cited by a tag. If one is missing, the fix is a real test that proves it — never a tag on an unrelated test.

- [ ] **Step 3: Self-test**

Run: `python3 -m unittest discover -s docs/specs/vnext/tools` → `OK`.

- [ ] **Step 4: Commit** — subject `T0.9: spec lint installed; P0 traceability passes`; `Refs: REQ-DOC-006, REQ-TST-016`; PROGRESS; tick T0.9; amend; push. (If Step 1 produced no diff, the commit contains only the PROGRESS line and the tick.)

---

### Task 14: T0.10 — CODEOWNERS, gate script, evidence template, verifier prompt

**Files:**
- Create: `.github/CODEOWNERS`, `tests/acceptance/README.md`, `scripts/gate.sh`, `docs/specs/vnext/evidence/TEMPLATE.md`, `docs/specs/vnext/tools/verifier-prompt.md`

**Interfaces:**
- Produces: `scripts/gate.sh G0` (exit 0 when every automated row passes; prints a Markdown table, failing-row logs, and the protected-change list since `c89f278`); `scripts/gate.sh counts` (per-suite `name<TAB>count` lines, used by Task 16 and row 1).

- [ ] **Step 1: `.github/CODEOWNERS`**

```
# Protected paths (REQ-TST-012, CR-3). Changes need the operator's review on the vnext → master PR,
# and are listed per gate by scripts/gate.sh for approval in docs/specs/vnext/evidence/G<n>.md.
/docs/specs/         @saltless-bruh
/tests/acceptance/   @saltless-bruh
/tests/fixtures/     @saltless-bruh
/scripts/gate.sh     @saltless-bruh
**/snapshots/**      @saltless-bruh
/.github/CODEOWNERS  @saltless-bruh
```

- [ ] **Step 2: `tests/acceptance/README.md`**

```markdown
# Acceptance tests (protected)

Gate acceptance tests live here, one file per gate (`g1_*.rs`, …), written by a session that does
not implement that phase (REQ-TST-012, REQ-TST-014). Implementing agents treat this directory as
read-only; changes go through a change request (REQ-DOC-007). Phase 0 has no acceptance tests —
G0 is verified by `scripts/gate.sh G0` and the evidence file.
```

- [ ] **Step 3: `scripts/gate.sh`** (`chmod +x`)

```bash
#!/usr/bin/env bash
# Run a gate's automated rows and print a PASS/FAIL table (REQ-TST-012; CODEOWNERS-protected).
#   scripts/gate.sh G0       run gate G0
#   scripts/gate.sh counts   per-suite test counts (source of evidence/test-baseline.txt)
# Manual rows are printed as MANUAL and are never passed by this script.
# No `-e`: a failing row must not stop the gate — every row runs and is reported.
set -uo pipefail
for cmd in git cargo python3; do
  command -v "$cmd" >/dev/null || { printf 'gate.sh: missing dependency %s\n' "$cmd" >&2; exit 2; }
done

ROOT="$(git rev-parse --show-toplevel)" || exit 2
cd "$ROOT" || exit 2
SPEC=docs/specs/vnext
TOOLS=$SPEC/tools
LOGDIR="$(mktemp -d)"
trap 'rm -rf "$LOGDIR"' EXIT
ROWS=()
FAILED=0

add_row() { ROWS+=("| $1 | $2 | $3 | $4 |"); if [[ $3 == FAIL ]]; then FAILED=1; fi; }

run_row() { # id, check, command...
  local id=$1 check=$2
  shift 2
  if "$@" >"$LOGDIR/$id.log" 2>&1; then add_row "$id" "$check" PASS "\`$*\`"
  else add_row "$id" "$check" FAIL "\`$*\` (log below)"; fi
}

suite_counts() {
  cargo test --workspace --locked -- --list 2>&1 | python3 -c '
import re, sys
suite = None
for line in sys.stdin:
    m = re.match(r"\s*Running (\S+(?: \S+)?) \(\S*/deps/(.+?)-[0-9a-f]+\)", line)
    if m:
        suite = f"{m.group(2)} {m.group(1)}"
        continue
    m = re.match(r"(\d+) tests?, \d+ benchmarks?$", line.strip())
    if m and suite:
        print(f"{suite}\t{m.group(1)}")
        suite = None
'
}

# shellcheck disable=SC2329  # invoked indirectly: passed by name to run_row
row_tests_and_counts() {
  cargo test --workspace --locked || return 1
  local base=$SPEC/evidence/test-baseline.txt
  if [[ ! -f $base ]]; then echo "no $base yet (T0.11)"; return 0; fi
  suite_counts >"$LOGDIR/counts.now"
  python3 - "$base" "$LOGDIR/counts.now" <<'PY'
import sys
def load(p):
    out = {}
    for line in open(p, encoding="utf-8"):
        if "\t" in line and not line.startswith("#"):
            name, n = line.rstrip("\n").split("\t")
            out[name] = int(n)
    return out
base, now = load(sys.argv[1]), load(sys.argv[2])
bad = [f"{s}: {now.get(s, 'missing')} < {n}" for s, n in base.items() if now.get(s, -1) < n]
print("\n".join(bad) or "suite counts >= baseline")
sys.exit(1 if bad else 0)
PY
}

# shellcheck disable=SC2329  # invoked indirectly: passed by name to run_row
row_no_behavior_change() {
  python3 - <<'PY'
import pathlib, subprocess, sys
base = "c89f278"
tracked = subprocess.check_output(["git", "ls-tree", "-r", "--name-only", base, "src"], text=True).split()
bad = []
for f in tracked:
    old = subprocess.check_output(["git", "show", f"{base}:{f}"])
    new = pathlib.Path(f).read_bytes()
    tail = new[len(old):]
    if not new.startswith(old):
        bad.append(f"{f}: existing bytes changed")
    elif tail.strip() and not tail.lstrip().startswith(b"#[cfg(test)]"):
        bad.append(f"{f}: appended text is not a #[cfg(test)] module")
for f in subprocess.check_output(["git", "ls-files", "--cached", "--others", "--exclude-standard", "src"], text=True).split():
    if f not in tracked and not f.startswith("src/snapshots/"):
        bad.append(f"{f}: new file outside src/snapshots/")
print("\n".join(bad) or "only test modules and snapshots added under src/")
sys.exit(1 if bad else 0)
PY
}

# shellcheck disable=SC2329  # invoked indirectly: passed by name to run_row
row_fixtures_clean() {
  ! grep -rnE '/home/[a-z_][a-z0-9_-]*/|sk-[A-Za-z0-9]|Bearer [A-Za-z0-9]' tests/fixtures
}

protected_changes() { # since $1
  git diff --name-only "$1" HEAD -- docs/specs tests/acceptance tests/fixtures scripts/gate.sh \
      .github/CODEOWNERS ':(glob)**/snapshots/**' | while read -r f; do
    [[ $f == "$SPEC/PROGRESS.md" ]] && continue
    if [[ $f == "$SPEC/tasks.md" ]] && ! git diff -U0 "$1" HEAD -- "$f" \
        | grep -E '^[-+][^-+]' | grep -qvE '^[-+][[:space:]]*- \[[ x!]\] '; then
      continue
    fi
    echo "- $f"
  done
}

gate_g0() {
  run_row 1 "Characterization + snapshots; counts ≥ baseline" row_tests_and_counts
  run_row 2 "No behavior change under src/" row_no_behavior_change
  run_row 3 "Fake runtime" cargo test -p fake-llama-server --locked
  run_row 4a "Inventory + defect register complete" \
    bash -c "python3 $TOOLS/check_baseline_docs.py inventory && python3 $TOOLS/check_baseline_docs.py defects"
  run_row 4b "Fixtures sanitized (no home paths, tokens)" row_fixtures_clean
  add_row 4 "Inventory and defects review" MANUAL "review $SPEC/baseline/{BEHAVIOR,DEFECTS}.md"
  add_row 5 "Protection" MANUAL "CODEOWNERS present: $([[ -f .github/CODEOWNERS ]] && echo yes || echo NO); branch protection confirmed by operator"
  run_row 6 "Spec lint + traceability + self-test" \
    bash -c "python3 $TOOLS/spec_lint.py --tests --phase P0 && python3 -m unittest discover -s $TOOLS"
  add_row 7 "Smoke (TUI renders; /healthz 200) + R0/R1 vs baseline" MANUAL "see evidence/G0.md"
  add_row V "Independent verification and falsification" MANUAL "verifier ≠ implementer; rubric clean"
  PROTECTED_SINCE=c89f278
}

case "${1:-}" in
  counts) suite_counts; exit $? ;;
  G0) gate_g0 ;;
  *) echo "usage: scripts/gate.sh G0 | counts   (no rows defined for '${1:-}')" >&2; exit 2 ;;
esac

echo "## Gate $1 — automated rows at $(git rev-parse --short HEAD)"
echo
echo "| # | Check | Result | Command / procedure |"
echo "|---|---|---|---|"
printf '%s\n' "${ROWS[@]}"
for log in "$LOGDIR"/*.log; do
  id=$(basename "$log" .log)
  if printf '%s\n' "${ROWS[@]}" | grep -q "^| $id | .* | FAIL |"; then
    echo; echo "### Row $id output (last 40 lines)"; echo '~~~'; tail -n 40 "$log"; echo '~~~'
  fi
done
echo; echo "### Protected-path changes since $PROTECTED_SINCE (operator approves in evidence)"
protected_changes "$PROTECTED_SINCE"
exit $FAILED
```

- [ ] **Step 4: Test the gate script against real states**

Run: `scripts/gate.sh G0; echo EXIT=$?` → rows 1, 2, 3, 4a, 4b, 6 PASS; 4, 5, 7, V MANUAL; `EXIT=0` (row 1 notes "no test-baseline.txt yet").
Break row 2 deliberately: `echo '// x' >> src/app.rs; scripts/gate.sh G0 | grep '^| 2 '` → `FAIL`; then `git checkout -- src/app.rs` (app.rs has no test module, so restoring the whole file is safe); re-run → `PASS`.
Break row 4b: `echo 'path=/home/laz/x' > tests/fixtures/leak.txt; scripts/gate.sh G0 | grep '^| 4b '` → `FAIL`; `rm tests/fixtures/leak.txt`.
Run: `scripts/gate.sh counts` → one line per suite, all counts > 0. `scripts/gate.sh G9; echo $?` → usage message, `2`.
Run: `shellcheck -x scripts/gate.sh` → no output, exit 0. Fix findings in the script; a `# shellcheck disable=SCxxxx` needs a one-line reason on the same comment.

- [ ] **Step 5: `docs/specs/vnext/evidence/TEMPLATE.md`**

```markdown
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
```

- [ ] **Step 6: `docs/specs/vnext/tools/verifier-prompt.md`**

```markdown
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
```

- [ ] **Step 7: Operator actions (`[HUMAN]` — do not perform)**

Add to PROGRESS and to the task: `[!] BLOCKED: T0.10 — branch protection must be enabled by the operator.` with these commands for the operator to run (or the GitHub UI equivalent):
```bash
gh api -X PUT repos/saltless-bruh/saltnitor/branches/master/protection --input - <<'JSON'
{"required_status_checks": null, "enforce_admins": false,
 "required_pull_request_reviews": {"require_code_owner_reviews": true, "required_approving_review_count": 1},
 "restrictions": null, "allow_force_pushes": false, "allow_deletions": false}
JSON
gh api -X PUT repos/saltless-bruh/saltnitor/branches/vnext/protection --input - <<'JSON'
{"required_status_checks": null, "enforce_admins": false, "required_pull_request_reviews": null,
 "restrictions": null, "allow_force_pushes": false, "allow_deletions": false}
JSON
```
Note for the operator: as the only code owner, you cannot approve your own PR; `enforce_admins: false` lets you merge `vnext → master` as admin after reviewing.

- [ ] **Step 8: Commit** — Close-out 1–4; add the five files; subject `T0.10: CODEOWNERS, gate script, evidence template, verifier prompt`; `Refs: REQ-TST-012, REQ-TST-007, REQ-TST-013, REQ-TST-015, REQ-MIG-003`; trailer `Protected-change: .github/CODEOWNERS, scripts/gate.sh, docs/specs/vnext/evidence/TEMPLATE.md, docs/specs/vnext/tools/verifier-prompt.md`; PROGRESS (with the BLOCKED line for protection); **tick T0.10 only after the operator confirms protection** (Task 16 records it); amend; push.

---

### Task 15: CR-2 — R1 real-world journey script + baseline run `[HW]`

**Files:**
- Create: `scripts/real-check.sh`, `tests/fixtures/captures/baseline/r1.json` (from the operator-approved run)

**Interfaces:**
- Consumes: `target/release/saltnitor`, `target/release/fake-llama-server record`, live config at `~/.config/saltnitor/config.toml` (read-only).
- Produces: `tests/fixtures/captures/<label>/r1.json` = `{"label", "captured_unix", "a", "b", "steps": [{"step", "http", "outcome", "load_ms", "ttfb_ms", "total_ms", "chunks", "done", "keys"}]}`; exit 0 all steps 2xx · 3 router down · 4 control API not up · 5 a step failed · 2 usage.

- [ ] **Step 1: `scripts/real-check.sh`** (`chmod +x`)

```bash
#!/usr/bin/env bash
# R1 real-world check (CR-2). [HW]: loads and evicts models on the real GPU.
# Runs a fixed journey through Saltnitor against the LIVE router and records statuses,
# timings and response key paths — never generated text. Run only after the operator says go.
#
#   scripts/real-check.sh --label NAME --a PROFILE --b PROFILE --yes
#       [--router URL] [--control URL] [--config PATH]
# Requires: bash ≥ 4.4, curl, python3 ≥ 3.11 (tomllib), cargo, script (util-linux), ps.
set -Eeuo pipefail
for cmd in curl python3 cargo script ps; do
  command -v "$cmd" >/dev/null || { printf 'real-check: missing dependency %s\n' "$cmd" >&2; exit 2; }
done
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
LABEL="" A="" B="" YES=0
ROUTER="http://127.0.0.1:8080" CONTROL="http://127.0.0.1:8765"
CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}/saltnitor/config.toml"
while [[ $# -gt 0 ]]; do
  case $1 in
    --label) LABEL=$2; shift 2 ;;
    --a) A=$2; shift 2 ;;
    --b) B=$2; shift 2 ;;
    --router) ROUTER=$2; shift 2 ;;
    --control) CONTROL=$2; shift 2 ;;
    --config) CONFIG=$2; shift 2 ;;
    --yes) YES=1; shift ;;
    *) echo "real-check: unknown argument $1" >&2; exit 2 ;;
  esac
done
[[ -n $LABEL && -n $A && -n $B ]] || { echo "real-check: --label, --a and --b are required" >&2; exit 2; }
[[ $LABEL =~ ^[a-z0-9][a-z0-9_-]*$ ]] || { echo "real-check: --label must match [a-z0-9_-] (it becomes a directory name)" >&2; exit 2; }
[[ $A =~ ^[A-Za-z0-9_.-]+$ && $B =~ ^[A-Za-z0-9_.-]+$ ]] || { echo "real-check: profile names must match [A-Za-z0-9_.-]" >&2; exit 2; }
[[ -r $CONFIG ]] || { echo "real-check: cannot read $CONFIG" >&2; exit 2; }
[[ $YES == 1 ]] || { echo "real-check: this loads and evicts models on the GPU; pass --yes once the operator has said go" >&2; exit 2; }
OUT="tests/fixtures/captures/$LABEL"
mkdir -p "$OUT"
WORK="$(mktemp -d)"

# Secrets are read from the live config into this process only; never printed or written.
eval "$(python3 - "$CONFIG" <<'PY'
import shlex, sys, tomllib
c = tomllib.load(open(sys.argv[1], "rb"))
print(f"export RC_CONTROL_TOKEN={shlex.quote(c.get('control_token') or '')}")
print(f"export RC_INFER_BEARER={shlex.quote(c.get('infer_bearer') or '')}")
PY
)"

auth_router=(); [[ -n $RC_INFER_BEARER ]] && auth_router=(-H "Authorization: Bearer $RC_INFER_BEARER")
auth_control=(); [[ -n $RC_CONTROL_TOKEN ]] && auth_control=(-H "Authorization: Bearer $RC_CONTROL_TOKEN")

curl -fsS -o /dev/null --max-time 5 "${auth_router[@]}" "$ROUTER/health" \
  || { echo "real-check: router $ROUTER is not up — BLOCKED" >&2; exit 3; }

cargo build --release --locked -p saltnitor -p fake-llama-server >/dev/null

# R0 alongside (read-only). The P0 baseline r0 is captured once (Task 8) and not overwritten.
if [[ $LABEL != baseline || ! -d $OUT/r0 ]]; then
  base_args=(); [[ $LABEL != baseline && -d tests/fixtures/captures/baseline/r0 ]] \
    && base_args=(--baseline tests/fixtures/captures/baseline/r0)
  RC_BEARER="$RC_INFER_BEARER" target/release/fake-llama-server record --upstream "$ROUTER" \
    --out "$OUT/r0" --bearer-env RC_BEARER "${base_args[@]}" || true
fi

STARTED_PID=""
cleanup() {
  if [[ -n $STARTED_PID ]]; then
    local child
    child=$(ps -o pid= --ppid "$STARTED_PID" | tr -d ' ' || true)
    [[ -n $child ]] && kill "$child" 2>/dev/null || true
    kill "$STARTED_PID" 2>/dev/null || true
  fi
  rm -rf "$WORK"
}
trap cleanup EXIT

if curl -fsS -o /dev/null --max-time 2 "$CONTROL/healthz"; then
  echo "real-check: using the running Saltnitor at $CONTROL"
else
  script -qfec "$ROOT/target/release/saltnitor" /dev/null </dev/null >/dev/null 2>&1 &
  STARTED_PID=$!
  for _ in $(seq 1 100); do
    curl -fsS -o /dev/null --max-time 1 "$CONTROL/healthz" && break
    sleep 0.2
  done
  curl -fsS -o /dev/null --max-time 2 "$CONTROL/healthz" \
    || { echo "real-check: Saltnitor control API did not come up at $CONTROL" >&2; exit 4; }
fi

step() { # name, method, path, body(optional) -> $WORK/<n>.{body,meta}
  local n=$1 method=$2 path=$3 body=${4:-}
  local args=(-sS -o "$WORK/$n.body" -w '%{http_code} %{time_starttransfer} %{time_total}' --max-time 600 "${auth_control[@]}")
  [[ -n $body ]] && args+=(-H 'Content-Type: application/json' -d "$body")
  curl "${args[@]}" -X "$method" "$CONTROL$path" >"$WORK/$n.meta" || echo "000 0 0" >"$WORK/$n.meta"
}

step 1 POST /v1/ensure "{\"profile\":\"$A\"}"
step 2 POST /v1/chat/completions "{\"model\":\"$A\",\"stream\":true,\"max_tokens\":16,\"messages\":[{\"role\":\"user\",\"content\":\"Reply with one short word.\"}]}"
step 3 POST /v1/ensure "{\"profile\":\"$B\"}"
step 4 GET /v1/status

python3 - "$WORK" "$OUT/r1.json" "$LABEL" "$A" "$B" <<'PY'
import json, pathlib, sys, time
work, out, label, a, b = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2]), *sys.argv[3:6]
def keys(v, at=""):
    s = set()
    if isinstance(v, dict):
        if at: s.add(at)
        for k, x in v.items(): s |= keys(x, f"{at}.{k}" if at else k)
    elif isinstance(v, list):
        s.add(at + "[]")
        for x in v: s |= keys(x, at + "[]")
    else:
        s.add(at)
    return s
names = {1: f"ensure {a}", 2: f"chat stream {a}", 3: f"ensure {b}", 4: "status"}
steps, ok = [], True
for n, name in names.items():
    code, ttfb, total = (work / f"{n}.meta").read_text().split()
    body = (work / f"{n}.body").read_text(errors="replace") if (work / f"{n}.body").exists() else ""
    rec = {"step": name, "http": int(code), "ttfb_ms": round(float(ttfb) * 1000), "total_ms": round(float(total) * 1000)}
    if n == 2:
        data = [l[6:] for l in body.splitlines() if l.startswith("data: ")]
        rec |= {"chunks": sum(d != "[DONE]" for d in data), "done": "[DONE]" in data}
    else:
        try:
            v = json.loads(body)
            rec |= {"outcome": v.get("status"), "load_ms": v.get("load_ms"), "keys": sorted(keys(v))}
        except json.JSONDecodeError:
            rec |= {"outcome": None, "keys": []}
    ok &= 200 <= rec["http"] < 300
    steps.append(rec)
out.write_text(json.dumps({"label": label, "captured_unix": int(time.time()), "a": a, "b": b, "steps": steps}, indent=2) + "\n")
base = pathlib.Path("tests/fixtures/captures/baseline/r1.json")
prev = {s["step"]: s for s in json.loads(base.read_text())["steps"]} if base.exists() and label != "baseline" else {}
print(f"{'STEP':<22} {'HTTP':>4} {'OUTCOME':<17} {'TTFB ms':>8} {'TOTAL ms':>9}  vs baseline")
for s in steps:
    p = prev.get(s["step"])
    cmp = "-" if not p else f"http {p['http']}→{s['http']}, ttfb {p['ttfb_ms']}→{s['ttfb_ms']}" + (
        "" if p.get("keys") == s.get("keys") else ", KEYS CHANGED")
    print(f"{s['step']:<22} {s['http']:>4} {str(s.get('outcome') or ('done' if s.get('done') else '-')):<17} {s['ttfb_ms']:>8} {s['total_ms']:>9}  {cmp}")
sys.exit(0 if ok else 5)
PY
```

- [ ] **Step 2: Dry checks (no GPU effect)**

Run: `scripts/real-check.sh --label x --a A_STD --b B; echo $?` → refuses without `--yes`, `2`.
Run: `scripts/real-check.sh --label ../x --a A_STD --b B --yes; echo $?` → label rejected, `2` (checked before any network or build step).
Run: `bash -n scripts/real-check.sh && shellcheck -x scripts/real-check.sh` → no output, exit 0 (operator installed shellcheck 2026-09-28). `grep -nE 'qwen|codacus' -i scripts/real-check.sh` → nothing (INV-11).

- [ ] **Step 3: Ask the operator for "go"** — message: "R1 will ensure A_STD, stream one 16-token reply, swap to B, and read status on the real GPU (loads/evicts models, ~1–3 min). Say go when the GPU is free." **Stop until the operator says go.**

- [ ] **Step 4: Run (after "go")**

Run: `scripts/real-check.sh --label baseline --a A_STD --b B --yes`
Expected: 4 steps with HTTP 2xx; `ensure A_STD` outcome `loaded` or `already_resident`; chat `done` true; `ensure B` `loaded`. Then `grep -rnE '/home/[a-z]|sk-|Bearer ' tests/fixtures/captures/baseline/r1.json` → nothing.

- [ ] **Step 5: Commit** — add `scripts/real-check.sh tests/fixtures/captures/baseline/r1.json`; subject `CR-2: R1 real-world journey script and P0 baseline run`; `Task: T0.10`; `Refs: REQ-MIG-003, REQ-TST-013`; trailer `Protected-change: tests/fixtures/captures/baseline/r1.json`; PROGRESS `… · CR-2 · <sha> · DONE · R1 baseline: <one-line table summary>`; amend; push.

---

### Task 16: T0.11 + G0 — Test baseline, smoke, evidence, verification

**Files:**
- Create: `docs/specs/vnext/evidence/test-baseline.txt`, `docs/specs/vnext/evidence/G0.md`

- [ ] **Step 1: T0.11 test baseline**

```bash
{ echo "# Per-suite minimum test counts (REQ-CI-005/AC2). Only ever raised. Generated by scripts/gate.sh counts at $(git rev-parse --short HEAD)."
  scripts/gate.sh counts; } > docs/specs/vnext/evidence/test-baseline.txt
cat docs/specs/vnext/evidence/test-baseline.txt
```
Expected: suites `saltnitor unittests src/main.rs`, `fake_llama_server unittests src/lib.rs`, `fake_llama_server unittests src/main.rs`, `http tests/http.rs`, `process tests/process.rs`, `record tests/record.rs`, all counts > 0. Commit: subject `T0.11: record per-suite test baseline`; `Refs: REQ-CI-005`; trailer `Protected-change: docs/specs/vnext/evidence/test-baseline.txt`; PROGRESS; tick T0.11; amend; push.

- [ ] **Step 2: Smoke (row 7)** — Saltnitor against the fake, isolated from the live config and the operator's instance

```bash
cargo build --locked --workspace
ROOT=$(git rev-parse --show-toplevel); T=$(mktemp -d); mkdir -p "$T/.config/saltnitor"
target/debug/fake-llama-server --port 0 > "$T/fake.out" 2>&1 & FAKE=$!
sleep 0.5; FPORT=$(sed -n 's/^FAKE_LISTENING 127.0.0.1://p' "$T/fake.out")
cat > "$T/.config/saltnitor/config.toml" <<EOF
control_port = 18765
router_base = "http://127.0.0.1:$FPORT"
[profiles.A]
model = "m-7b-Q4_K_M.gguf"
est_vram_gb = 0.0
est_ram_gb = 0.0
EOF
( cd "$T" && HOME="$T" script -qfec "timeout 8 $ROOT/target/debug/saltnitor --port $FPORT" "$T/tui.log" </dev/null >/dev/null 2>&1 ) & SMOKE=$!
sleep 3; curl -s -o /dev/null -w 'healthz=%{http_code}\n' http://127.0.0.1:18765/healthz
wait $SMOKE; kill $FAKE
sed -E 's/\x1b\[[0-9;?]*[a-zA-Z]//g' "$T/tui.log" | grep -c 'API Interrogator'
```
Expected: `healthz=200`; grep count ≥ 1 (TUI rendered). Run via `ctx_execute`; paste the two result lines into G0.md row 7.

- [ ] **Step 3: R0 rerun vs baseline** (read-only)

Run the Task 8 Step 2 command with `--out /tmp/claude-1000/g0-r0 --baseline tests/fixtures/captures/baseline/r0` → table with `SHAPE-OK` on every JSON endpoint; paste into G0.md. (R1 baseline from Task 15 is the G0 R1 entry — no second GPU run needed at G0.)

- [ ] **Step 4: Gate script**

Run: `scripts/gate.sh G0 > /tmp/claude-1000/g0.md; echo EXIT=$?` → `EXIT=0`; all automated rows PASS.

- [ ] **Step 5: Write `docs/specs/vnext/evidence/G0.md`** from `TEMPLATE.md`: commit sha, CI run URL (`gh run list --branch vnext --limit 1 --json url,conclusion`), the gate table (automated rows from Step 4 + manual rows with evidence), R0/R1 section, the protected-change list, rubric boxes left **unchecked** for the verifier, `Claims to falsify: none (no product change)`, and `## Verdict:` left blank.

- [ ] **Step 6: Independent verification (row V)** — ask the operator: "G0 needs a verifier who is not me. Options: (a) I launch a fresh-context reviewer agent with only `requirements.md`, `git diff c89f278..HEAD`, `G0.md` and `verifier-prompt.md`; (b) you review." **Wait for the answer.** If (a): dispatch one `general-purpose` agent whose prompt is the contents of `docs/specs/vnext/tools/verifier-prompt.md` with `<n>` = 0 and the three inputs as file paths; paste its report verbatim into G0.md under the rubric; fix any correctness gap it finds in a new commit and re-run Steps 4–5.

- [ ] **Step 7: Operator sign-off items** — in G0.md mark as pending operator: branch protection confirmation (row 5, Task 14 Step 7), snapshot approval (REQ-TUI-009/AC1), protected-change list approval, `Verdict`. Tick T0.10 only when the operator confirms protection.

- [ ] **Step 8: Commit** — add `docs/specs/vnext/evidence/G0.md`; subject `G0: baseline gate evidence`; `Task: G0`; `Refs: REQ-MIG-002, REQ-MIG-003, REQ-MIG-005, REQ-TST-011, REQ-TST-012, REQ-DOC-006, REQ-TST-016, REQ-TST-013, REQ-TST-015`; trailer `Protected-change: docs/specs/vnext/evidence/G0.md`; PROGRESS `… · G0 · <sha> · DONE · evidence ready; awaiting operator verdict`; amend; push. Then report to the operator with the gate table and the list of items needing their signature.
