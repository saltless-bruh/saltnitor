#!/usr/bin/env bash
# Self-test: every rule must fire on a planted violation and stay quiet on the marker.
# Runs in a throwaway worktree seeded with a ratchet of whatever the tree already violates, so only
# the planted line can be "new". Verifies: REQ-CI-008/AC1, REQ-CI-008/AC2, REQ-CI-008/AC3
set -euo pipefail
ROOT="$(git rev-parse --show-toplevel)"
work=$(mktemp -d)
# shellcheck disable=SC2329 # invoked via trap
cleanup() { git -C "$ROOT" worktree remove -f "$work/wt" >/dev/null 2>&1 || true; rm -rf "$work"; git -C "$ROOT" worktree prune; }
trap cleanup EXIT
git -C "$ROOT" worktree add -q "$work/wt" HEAD
cd "$work/wt"
cp "$ROOT/scripts/check-invariants.sh" scripts/check-invariants.sh   # test the scanner as it is on disk
# Planted text is assembled from pieces so this file does not trip the scanner it tests.
home=/home; key=sk-
: > scripts/invariants-baseline.txt
scripts/check-invariants.sh | awk '/^RULE [a-z-]+ /{print $2 "\t" substr($3,1,length($3)-1)}' > scripts/invariants-baseline.txt || true
seed=$(cat scripts/invariants-baseline.txt)
fail=0
check() { # rule, planted text, file
  printf '%s\n' "$2" >> "$3"
  git add -f -- "$3"                  # the artifact rule scans tracked files
  if out=$(scripts/check-invariants.sh 2>&1); then echo "FAIL: $1 did not fire"; fail=1
  elif ! grep -q "^RULE $1 $3:" <<<"$out" || ! grep -q "  fix: " <<<"$out"; then echo "FAIL: $1 fired without rule id, location or remediation"; echo "$out"; fail=1
  else echo "ok: $1"; fi
  git reset -q -- "$3"; git checkout -q -- "$3" 2>/dev/null || rm -f "$3"
}
check kill-by-name 'let _ = Command::new("killall").arg("x");' src/events.rs
check home-path    "const P: &str = \"$home/someone/x\";" src/events.rs
check secret       "const K: &str = \"${key}abcdefghijklmnop\";" src/events.rs
check build-tool   'Command::new("cmake").arg("..");' src/events.rs
check artifact     'tracked' router.ini
# nested and editor-dir artifacts (REQ-REPO-002/AC1): the scan is not root-anchored
mkdir -p src/nested/benchmark-results .vscode
check artifact     'tracked' src/nested/saltnitor_crash_1.txt
check artifact     'tracked' src/nested/run.trace.csv
check artifact     'tracked' src/nested/dev.local.toml
check artifact     'tracked' src/nested/benchmark-results/r.json
check artifact     'tracked' .vscode/settings.json
# marker suppresses
printf '%s\n' "const P: &str = \"$home/someone/x\"; // invariants: allow home-path — sanitizer test input" >> src/events.rs
if scripts/check-invariants.sh >/dev/null 2>&1; then echo "ok: marker honoured"; else echo "FAIL: marker not honoured"; fail=1; fi
git checkout -q -- src/events.rs
# ratchet only shrinks: a listed entry that no longer occurs fails
printf '%s\nhome-path\tsrc/events.rs:9999\n' "$seed" > scripts/invariants-baseline.txt
if out=$(scripts/check-invariants.sh 2>&1); then echo "FAIL: stale ratchet entry accepted"; fail=1
elif grep -q '^RULE ratchet src/events.rs:9999' <<<"$out"; then echo "ok: stale ratchet entry rejected"
else echo "FAIL: stale ratchet entry rejected without a ratchet message"; echo "$out"; fail=1; fi
exit $fail
