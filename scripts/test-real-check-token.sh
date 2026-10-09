#!/usr/bin/env bash
# Self-test for scripts/real-check.sh token resolution (T1.10 order: control_token literal, else
# the env var named by control_token_env, else the contents of control_token_file). Uses throwaway
# config files and a PATH whose curl records any call, so nothing is contacted and the live config
# is never read. The script is only ever run with --print-token-source, never the real check.
# Supports the G1 R1 procedure; it proves no spec AC (the daemon-side AC tests live in tests/secrets.rs).
set -euo pipefail
ROOT="$(git rev-parse --show-toplevel)"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir "$work/bin"
cat >"$work/bin/curl" <<STUB
#!/usr/bin/env bash
echo called >>"$work/curl-called"
exit 1
STUB
chmod +x "$work/bin/curl"
secret="tok-$$-not-a-real-secret"
fail=0

run() { # config-file [env assignments...] -> sets out, rc
  local cfg=$1; shift
  set +e
  out=$(env "$@" PATH="$work/bin:$PATH" "$ROOT/scripts/real-check.sh" --print-token-source --config "$cfg" 2>&1)
  rc=$?
  set -e
}
expect() { # name, wanted-rc, wanted-output-substring
  if [[ $rc != "$2" || $out != *"$3"* ]]; then
    echo "FAIL: $1 (rc=$rc, wanted $2 / '$3'): $out"; fail=1
  elif [[ $out == *"$secret"* ]]; then
    echo "FAIL: $1 printed the token value"; fail=1
  else echo "ok: $1"; fi
}

printf 'control_port = 1\ncontrol_token = "%s"\n' "$secret" >"$work/literal.toml"
run "$work/literal.toml" X=1
expect literal-token 0 "token source: literal"

printf 'control_token_env = "RC_TEST_TOKEN"\n' >"$work/env.toml"
run "$work/env.toml" "RC_TEST_TOKEN=$secret"
expect env-token 0 "token source: env"
run "$work/env.toml" X=1
expect env-unset-is-an-error 2 "control_token_env"

printf '%s\n' "$secret" >"$work/token"
printf 'control_token_file = "%s"\n' "$work/token" >"$work/file.toml"
run "$work/file.toml" X=1
expect file-token 0 "token source: file"

: >"$work/empty"
printf 'control_token_file = "%s"\n' "$work/empty" >"$work/emptyfile.toml"
run "$work/emptyfile.toml" X=1
expect empty-file-is-an-error 2 "control_token_file"

printf 'control_port = 1\n' >"$work/none.toml"
run "$work/none.toml" X=1
expect no-token 0 "token source: none"

printf 'control_token = "a"\ncontrol_token_env = "RC_TEST_TOKEN"\n' >"$work/two.toml"
run "$work/two.toml" "RC_TEST_TOKEN=$secret"
expect two-sources-is-an-error 2 "exactly one"

if [[ -e $work/curl-called ]]; then echo "FAIL: curl was invoked"; fail=1; else echo "ok: nothing contacted"; fi
exit "$fail"
