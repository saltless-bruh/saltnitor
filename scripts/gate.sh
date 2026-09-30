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
    if [[ $f == "$SPEC/tasks.md" ]]; then
      # Count non-checkbox changed lines; grep -c reads all input (grep -q + pipefail would
      # SIGPIPE the upstream grep and wrongly skip tasks.md).
      local other
      other=$(git diff -U0 "$1" HEAD -- "$f" | grep -E '^[-+][^-+]' \
        | grep -cvE '^[-+][[:space:]]*- \[[ x!]\] ' || true)
      [[ $other -eq 0 ]] && continue
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
