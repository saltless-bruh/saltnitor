#!/usr/bin/env python3
"""Self-test for spec_lint.py: the shipped spec pack passes, and each seeded defect fails.

Run:  python3 -m unittest discover -s docs/specs/vnext/tools
Verifies: REQ-DOC-006/AC1, REQ-TST-016/AC2
"""
import os
import re
import shutil
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
LINT = os.path.join(HERE, "spec_lint.py")


def read(path):
    with open(path, encoding="utf-8") as f:
        return f.read()


def write(path, text):
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)


def find_spec(name):
    for d in (os.path.join(HERE, ".."), HERE):
        p = os.path.join(d, name)
        if os.path.exists(p):
            return os.path.normpath(p)
    raise unittest.SkipTest(f"{name} not found beside tools/")


class SpecLintTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix="spec_lint_")
        self.req = os.path.join(self.tmp, "requirements.md")
        self.tasks = os.path.join(self.tmp, "tasks.md")
        self.progress = os.path.join(self.tmp, "PROGRESS.md")
        self.src = os.path.join(self.tmp, "src")
        os.mkdir(self.src)
        shutil.copy(find_spec("requirements.md"), self.req)
        shutil.copy(find_spec("tasks.md"), self.tasks)

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    # helpers ---------------------------------------------------------------
    def run_lint(self, *extra):
        p = subprocess.run([sys.executable, LINT, "--req", self.req, "--tasks", self.tasks,
                            "--progress", self.progress, *extra], capture_output=True, text=True)
        return p.returncode, p.stdout + p.stderr

    def edit(self, path, old, new):
        text = read(path)
        self.assertIn(old, text, f"fixture text not found: {old!r}")
        write(path, text.replace(old, new, 1))

    def assert_fails_with(self, needle, *extra):
        code, out = self.run_lint(*extra)
        self.assertEqual(code, 1, out)
        self.assertIn(needle, out)

    # the shipped pack ----------------------------------------------------------
    def test_shipped_pack_passes(self):
        code, out = self.run_lint()
        self.assertEqual(code, 0, out)

    def test_sync_is_a_fixed_point(self):
        before = read(self.req), read(self.tasks)
        code, out = self.run_lint("--sync")
        self.assertEqual(code, 0, out)
        after = read(self.req), read(self.tasks)
        self.assertEqual(before, after)

    # seeded defects ------------------------------------------------------------
    def test_undefined_id(self):
        self.edit(self.tasks, "  - **Reqs:** REQ-MIG-001\n", "  - **Reqs:** REQ-MIG-001, REQ-ZZZ-999\n")
        self.assert_fails_with("undefined REQ-ZZZ-999")

    def test_retired_id(self):
        self.edit(self.tasks, "  - **Reqs:** REQ-MIG-001\n", "  - **Reqs:** REQ-MIG-001, REQ-TST-003\n")
        self.assert_fails_with("references retired REQ-TST-003")

    def test_missing_ac(self):
        self.edit(self.tasks, "  - **Reqs:** REQ-MIG-001\n", "  - **Reqs:** REQ-MIG-001, REQ-MIG-001/AC9\n")
        self.assert_fails_with("REQ-MIG-001/AC9 — that AC does not exist")

    def test_uncovered_must_ac(self):
        self.edit(self.tasks, "REQ-MIG-007/AC6", "REQ-MIG-007/AC5")
        self.assert_fails_with("REQ-MIG-007/AC6: covered by no task")

    def test_ac_without_verification_method(self):
        self.edit(self.req, "now with the semantics of REQ-BEN-014. (T)", "now with the semantics of REQ-BEN-014.")
        self.assert_fails_with("REQ-MIG-007/AC6: no verification method")

    def test_later_phase_dependency(self):
        self.edit(self.tasks, "  - **Reqs:** REQ-MIG-001\n  - **Depends:** T0.1\n",
                  "  - **Reqs:** REQ-MIG-001\n  - **Depends:** T0.1, T11.1\n")
        self.assert_fails_with("T0.2: depends on later-phase task T11.1")

    def test_dependency_cycle(self):
        self.edit(self.tasks, "  - **Reqs:** REQ-REPO-007, REQ-DOC-005, REQ-DOC-007\n",
                  "  - **Reqs:** REQ-REPO-007, REQ-DOC-005, REQ-DOC-007\n  - **Depends:** T0.2\n")
        self.assert_fails_with("dependency cycle")

    def test_parallel_task_without_files(self):
        self.edit(self.tasks, "- [ ] **T2.9 — Structured internal errors**", "- [ ] **T2.9 — Structured internal errors** `[P]`")
        self.assert_fails_with("T2.9: [P] task has no Files line")

    def test_parallel_tasks_sharing_files(self):
        self.edit(self.tasks, "  - **Files:** `docs/specs/vnext/baseline/DEFECTS.md`",
                  "  - **Files:** `docs/specs/vnext/baseline/BEHAVIOR.md`")
        self.assert_fails_with("[P] tasks share")

    def test_gate_cites_requirement_due_later(self):
        self.edit(self.tasks, "| TUI renders; 200 | REQ-MIG-003 |", "| TUI renders; 200 | REQ-REL-001 |")
        self.assert_fails_with("G0: cites REQ-REL-001, which is first due in P11")

    def test_stale_generated_fields(self):
        text = read(self.req)
        text, n = re.subn(r"(#### REQ-MIG-001 — .*\n> MUST · Verify: [^·]+· Phase: )P0", r"\1P9", text, count=1)
        self.assertEqual(n, 1)
        write(self.req, text)
        self.assert_fails_with("stale")

    def test_requirement_without_source(self):
        self.edit(self.req, "### 7.2 Repository (REPO)",
                  "#### REQ-ZZZ-001 — Probe\n> MUST · Verify: T · Phase: P0 · Tasks: T0.1\n"
                  "- **AC1** The probe SHALL exist. (T)\n\n### 7.2 Repository (REPO)")
        self.edit(self.tasks, "  - **Reqs:** REQ-REPO-007, REQ-DOC-005, REQ-DOC-007\n",
                  "  - **Reqs:** REQ-REPO-007, REQ-DOC-005, REQ-DOC-007, REQ-ZZZ-001\n")
        self.assert_fails_with("REQ-ZZZ-001: no source")

    def test_ticked_task_needs_progress_entry(self):
        self.edit(self.tasks, "- [ ] **T0.1 —", "- [x] **T0.1 —")
        write(self.progress, "2026-10-01 T0.2 started\n")
        self.assert_fails_with("T0.1: ticked [x] but no 'DONE' line")
        write(self.progress, "2026-10-01 T0.1 DONE abc1234\n")
        code, out = self.run_lint()
        self.assertEqual(code, 0, out)

    def test_verifies_tags(self):
        code, out = self.run_lint("--tests", "--phase", "P0", "--src", self.src)
        self.assertEqual(code, 1, out)
        due = sorted(set(re.findall(r"(REQ-[A-Z]+-\d{3}/AC\d+): no test tagged", out)))
        self.assertTrue(due, out)
        self.assertNotIn("REQ-MIG-007/AC6", due)                  # due in P7, not P0
        write(os.path.join(self.src, "notes.md"), "Verifies: " + ", ".join(due) + "\n")  # Markdown never counts
        code, out = self.run_lint("--tests", "--phase", "P0", "--src", self.src)
        self.assertEqual(code, 1, out)
        write(os.path.join(self.src, "baseline_tests.rs"),
              "".join(f"/// Verifies: {ref}\n#[test] fn t{i}() {{}}\n" for i, ref in enumerate(due)))
        code, out = self.run_lint("--tests", "--phase", "P0", "--src", self.src)
        self.assertEqual(code, 0, out)


if __name__ == "__main__":
    unittest.main()
