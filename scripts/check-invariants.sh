#!/usr/bin/env bash
# Invariant scan (REQ-CI-008/AC1): fails on new violations; pre-existing ones live in the ratchet
# file scripts/invariants-baseline.txt (rule<TAB>file:line), which may only shrink (AC3).
# Each failure prints the rule, the location, and how to fix it (AC2).
# Exempt one line with: invariants: allow <rule> — <reason>
set -uo pipefail
ROOT="$(git rev-parse --show-toplevel)"; cd "$ROOT" || exit 1
BASELINE=scripts/invariants-baseline.txt
declare -A FIX=(
  [kill-by-name]="signal an exact PID through src/process.rs (INV-09); never killall/pkill"
  [home-path]="read the path from config (router_ini, XDG dirs) or an env var; never a user's home (INV-16)"
  [secret]="load the key from <key>_env / <key>_file (REQ-SEC-003); never a literal"
  [build-tool]="runtime channels are built by the operator; src/ never runs git/cmake/make (INV-12)"
  [artifact]="untrack it (git rm --cached) and add it to .gitignore (REQ-REPO-002)"
)
hits=()   # "rule<TAB>file:line<TAB>message"
scan() { # rule, grep -E pattern, pathspec...
  local rule=$1 pat=$2; shift 2
  while IFS= read -r line; do
    [[ $line == *"invariants: allow $rule"* ]] && { echo "marker: $line"; continue; }
    hits+=("$rule"$'\t'"${line%%:*}:$(cut -d: -f2 <<<"$line")"$'\t'"$(cut -d: -f3- <<<"$line")")
  done < <(git grep -nE "$pat" -- "$@" 2>/dev/null || true)
}
scan kill-by-name '\b(killall|pkill)\b' 'src/' 'tools/*/src/'
scan home-path    '/home/[a-z_][a-z0-9_-]*/' ':!docs/specs' ':!*.md' ':!tests/fixtures/captures'
# tests/ is out of scope: the protected acceptance suite carries placeholder bearer tokens.
scan secret       '(sk-[A-Za-z0-9]{8,}|Bearer [A-Za-z0-9._-]{12,}|api[_-]?key *= *"[^"$<]{8,}")' 'src/' 'scripts/' 'examples/' '.github/' 'tools/*/src/' 'README.md'
scan build-tool   'Command::new\("(git|cmake|make)"\)' 'src/'
for f in $(git ls-files -- 'router.ini' '*.gguf' 'target/' 'saltnitor_crash_*' '.saltnitor_history' 'crash_dump_*'); do
  hits+=("artifact"$'\t'"$f:1"$'\t'"tracked runtime artifact")
done
# ratchet
declare -A base=(); while IFS=$'\t' read -r r loc; do [[ -n $r && $r != \#* ]] && base["$r"$'\t'"$loc"]=1; done < "$BASELINE"
declare -A seen=()
status=0
for h in "${hits[@]}"; do
  IFS=$'\t' read -r rule loc msg <<<"$h"; key="$rule"$'\t'"$loc"; seen["$key"]=1
  [[ -n ${base[$key]:-} ]] && continue
  printf 'RULE %s %s: %s\n  fix: %s\n' "$rule" "$loc" "$msg" "${FIX[$rule]}"; status=1
done
for key in "${!base[@]}"; do
  if [[ -z ${seen[$key]:-} ]]; then
    IFS=$'\t' read -r rule loc <<<"$key"
    printf 'RULE ratchet %s: baseline entry no longer occurs (%s)\n  fix: delete the line from %s — the ratchet only shrinks (REQ-CI-008/AC3)\n' "$loc" "$rule" "$BASELINE"; status=1
  fi
done
[[ $status -eq 0 ]] && echo "invariants: clean (${#base[@]} ratcheted)"
exit $status
