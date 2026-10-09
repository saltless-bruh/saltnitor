#!/usr/bin/env bash
# smoke_control_api.sh — smoke-test Saltnitor's control API + the llama.cpp router.
# Run on the box with Saltnitor up (cargo run) and the router started. No root needed.
# A manual tool, never a test. Exits non-zero if any step fails.
#
# Credentials (all optional, none are ever printed):
#   SALTNITOR_CONTROL_TOKEN  Bearer for the control API on :8765. REQUIRED when the daemon runs
#                            with a control token (config: control_token_env / control_token_file):
#                            /v1/status, /v1/models, /v1/chat/completions and /v1/ensure* need it.
#   LLAMA_API_KEY            the router's own --api-key (config: infer_bearer), for calls made
#                            straight to the router on :8080. Leave unset for an open router.
# `jq` is optional (falls back to raw output).
set -uo pipefail

CTRL="http://127.0.0.1:8765"     # Saltnitor control API  (config: control_port)
ROUTER="http://127.0.0.1:8080"   # llama.cpp router        (config: router_base)
CTOKEN="${SALTNITOR_CONTROL_TOKEN:-}"
IBEARER="${LLAMA_API_KEY:-}"

# Header arguments are built once so a missing credential means "no header", not "Bearer ".
CAUTH=(); [ -n "$CTOKEN" ] && CAUTH=(-H "Authorization: Bearer $CTOKEN")
IAUTH=(); [ -n "$IBEARER" ] && IAUTH=(-H "Authorization: Bearer $IBEARER")

FAILED=0
ok(){ printf '  \033[32mok\033[0m   %s\n' "$1"; }
no(){ printf '  \033[31mFAIL\033[0m %s\n' "$1"; FAILED=1; }
hr(){ printf '\n\033[1m-- %s --\033[0m\n' "$1"; }
pp(){ jq . 2>/dev/null || cat; }
[ -z "$CTOKEN" ] && printf 'note: SALTNITOR_CONTROL_TOKEN is unset; calls to :8765 that need a token will report 401\n'

hr "1) control API alive  (GET /healthz, no auth)"
if curl -fsS -m 5 "$CTRL/healthz" >/dev/null; then ok "control API responding on :8765"
else no "no answer on :8765 — is Saltnitor running?"; fi

hr "2) router alive + section names  (GET /v1/models on the router)"
if MODELS=$(curl -fsS -m 5 "${IAUTH[@]}" "$ROUTER/v1/models" 2>&1); then
  echo "$MODELS" | (jq -r '.data[].id' 2>/dev/null || echo "$MODELS")
  ok "ids above are your router.ini section names (expect A_STD / A_FOCUS / B)"
else no "router on :8080 did not answer ($MODELS) — start it:  sudo systemctl start llama-router"; fi

hr "3) oracle view  (GET /v1/status, Bearer)"
if STATUS=$(curl -fsS -m 5 "${CAUTH[@]}" "$CTRL/v1/status" 2>&1); then
  echo "$STATUS" | pp; ok "/v1/status answered"
else no "/v1/status failed ($STATUS)"; fi

hr "4) ensure Tier A resident  (POST /v1/ensure — blocking, oracle-gated, Bearer)"
if RESP=$(curl -fsS -m 120 -X POST "$CTRL/v1/ensure" "${CAUTH[@]}" \
    -H "Content-Type: application/json" -d '{"profile":"A_STD"}' 2>&1); then
  echo "$RESP" | pp
  if echo "$RESP" | grep -qiE 'loaded|already_resident|endpoint'; then ok "A_STD reported resident"
  else no "ensure did not report resident (body above)"; fi
else no "ensure failed ($RESP)"; fi

hr "5) real inference through the daemon  (POST /v1/chat/completions on :8765, model=A_STD, Bearer)"
if CHAT=$(curl -fsS -m 120 -X POST "$CTRL/v1/chat/completions" "${CAUTH[@]}" \
    -H "Content-Type: application/json" \
    -d '{"model":"A_STD","messages":[{"role":"user","content":"reply with the single word OK"}],"max_tokens":8}' 2>&1); then
  echo "$CHAT" | (jq -r '.choices[0].message.content' 2>/dev/null || echo "$CHAT"); ok "chat completion answered"
else no "chat completion failed ($CHAT)"; fi

hr "6) escalate to Tier B, watch the swap live  (GET /v1/ensure/stream, SSE, Bearer)"
echo "   expected stages: received -> oracle_ok -> loading -> done   (or oom/error)"
if STREAM=$(curl -fsS -N -m 180 "$CTRL/v1/ensure/stream?profile=B" "${CAUTH[@]}" 2>&1); then
  echo "$STREAM"
  if echo "$STREAM" | grep -q '"stage":"done"'; then ok "stream ended on a done event"
  else no "stream did not end on a done event (oom/error/truncated)"; fi
else no "ensure stream failed ($STREAM)"; fi

echo
if [ "$FAILED" -eq 0 ]; then
  printf '\033[1mDone.\033[0m All steps passed: the integration works end-to-end.\n'
  printf 'Tip: in another pane run  watch -n0.5 nvidia-smi  to watch VRAM swap as steps 4 and 6 fire.\n'
else
  printf '\033[1mFAILED.\033[0m At least one step failed (see FAIL lines above).\n'
fi
exit "$FAILED"
