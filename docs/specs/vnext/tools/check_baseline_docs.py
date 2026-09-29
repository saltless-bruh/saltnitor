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
REGISTER_ROW = re.compile(r"^\| *(BD-\d{2}) *\|", re.M)


def routes(src: str) -> set[str]:
    return {f"{m.group(2).upper()} {m.group(1)}" for m in ROUTE_RE.finditer(src)}


def keycodes(src: str) -> set[str]:
    return {f"KeyCode::{m.group(1)}" for m in KEY_RE.finditer(src)}


def missing_citations(doc: str, tokens: set[str]) -> list[str]:
    cited = [line for line in doc.splitlines() if CITE_RE.search(line)]
    return sorted(t for t in tokens if not any(f"`{t}`" in line for line in cited))


def register_ids(req_text: str) -> list[str]:
    """BD ids listed as rows of the defect table in requirements.md §5, in order."""
    return REGISTER_ROW.findall(req_text)


def check_defects(doc: str, allow_blocked: bool, line_exists: Callable[[str, int], bool],
                  expected: list[str] | None = None) -> list[str]:
    parts = BD_HEAD.split(doc)
    entries = dict(zip(parts[1::2], parts[2::2]))
    if expected is None:
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
    errors += [f"{bd}: not in the requirements.md §5 register" for bd in sorted(set(entries) - set(expected))]
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
    expected = register_ids((root / "docs/specs/vnext/requirements.md").read_text(encoding="utf-8"))
    errors = check_defects((base / "DEFECTS.md").read_text(encoding="utf-8"), a.allow_blocked,
                           lambda p, n: 0 < n <= _baseline_lines(p), expected)
    print("\n".join(errors) or f"defects: {len(expected)}/{len(expected)} complete")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
