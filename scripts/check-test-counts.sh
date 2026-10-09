#!/usr/bin/env bash
# Fail when any suite runs fewer tests than docs/specs/vnext/evidence/test-baseline.txt allows.
# Verifies: REQ-CI-005/AC1
set -euo pipefail
ROOT="$(git rev-parse --show-toplevel)"; cd "$ROOT"
BASE=docs/specs/vnext/evidence/test-baseline.txt
[[ -f $BASE ]] || { echo "check-test-counts: missing $BASE"; exit 2; }
LIST="$(mktemp)"; trap 'rm -f "$LIST"' EXIT
cargo test --workspace --locked --color never -- --list >"$LIST" 2>&1
python3 - "$BASE" "$LIST" <<'PY'
import re, sys
base = {}
for line in open(sys.argv[1], encoding="utf-8"):
    if "\t" in line and not line.startswith("#"):
        name, n = line.rstrip("\n").split("\t"); base[name] = int(n)
now, suite = {}, None
for line in open(sys.argv[2], encoding="utf-8"):
    m = re.match(r"\s*Running (\S+(?: \S+)?) \(\S*/deps/(.+?)-[0-9a-f]+\)", line)
    if m: suite = f"{m.group(2)} {m.group(1)}"; continue
    m = re.match(r"(\d+) tests?, \d+ benchmarks?$", line.strip())
    if m and suite: now[suite] = int(m.group(1)); suite = None
bad = [f"{s}: {now.get(s, 'missing')} < {n}" for s, n in base.items() if now.get(s, -1) < n]
print("\n".join(bad) or "test counts >= baseline for every suite")
sys.exit(1 if bad else 0)
PY
