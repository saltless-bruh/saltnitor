#!/usr/bin/env bash
# test_control_api.sh — smoke-test Saltnitor's control API + the llama.cpp router.
# Run on the box with Saltnitor up (cargo run) and the router started. No root needed.
# Adjust the four vars if you changed config.toml. `jq` is optional (falls back to raw).

CTRL="http://127.0.0.1:8765"     # Saltnitor control API  (config: control_port)
ROUTER="http://127.0.0.1:8080"   # llama.cpp router        (config: router_base)
CTOKEN="sk-saltcode-local"       # config: control_token   (Bearer for /v1/ensure*)
IBEARER="sk-saltnitor-2026"      # config: infer_bearer    (router api-key, only if set)

ok(){ printf '  \033[32mok\033[0m   %s\n' "$1"; }
no(){ printf '  \033[31mFAIL\033[0m %s\n' "$1"; }
hr(){ printf '\n\033[1m-- %s --\033[0m\n' "$1"; }
pp(){ jq . 2>/dev/null || cat; }

hr "1) control API alive  (GET /healthz, no auth)"
if curl -fsS -m 5 "$CTRL/healthz" >/dev/null; then ok "control API responding on :8765"
else no "no answer on :8765 — is Saltnitor running?"; fi

hr "2) router alive + section names  (GET /v1/models, no auth)"
MODELS=$(curl -fsS -m 5 "$ROUTER/v1/models" 2>/dev/null)
if [ -n "$MODELS" ]; then
  echo "$MODELS" | (jq -r '.data[].id' 2>/dev/null || echo "$MODELS")
  ok "ids above are your router.ini section names (expect A_STD / A_FOCUS / B)"
else no "no answer on :8080 — start it:  sudo systemctl start llama-router"; fi

hr "3) oracle view  (GET /v1/status, no auth)"
curl -fsS -m 5 "$CTRL/v1/status" | pp; echo

hr "4) ensure Tier A resident  (POST /v1/ensure — blocking, oracle-gated, Bearer)"
RESP=$(curl -fsS -m 120 -X POST "$CTRL/v1/ensure" \
  -H "Authorization: Bearer $CTOKEN" -H "Content-Type: application/json" \
  -d '{"profile":"A_STD"}' 2>/dev/null)
echo "$RESP" | pp
echo "$RESP" | grep -qiE 'ready|resident|endpoint' && ok "A_STD reported resident" \
  || no "ensure did not report ready (body above)"

hr "5) real inference routes to it  (POST /v1/chat/completions, model=A_STD)"
curl -fsS -m 120 -X POST "$ROUTER/v1/chat/completions" \
  -H "Authorization: Bearer $IBEARER" -H "Content-Type: application/json" \
  -d '{"model":"A_STD","messages":[{"role":"user","content":"reply with the single word OK"}],"max_tokens":8}' \
  | (jq -r '.choices[0].message.content' 2>/dev/null || cat); echo

hr "6) escalate to Tier B, watch the swap live  (GET /v1/ensure/stream, SSE, Bearer)"
echo "   expected stages: received -> oracle_ok -> loading -> done   (or oom/error)"
curl -fsS -N -m 180 "$CTRL/v1/ensure/stream?profile=B" -H "Authorization: Bearer $CTOKEN"
echo

printf '\n\033[1mDone.\033[0m If 1-5 pass and 6 ends on a done event, the integration works end-to-end.\n'
printf 'Tip: in another pane run  watch -n0.5 nvidia-smi  to watch VRAM swap as steps 4 and 6 fire.\n'
