#!/usr/bin/env bash
# launch_router.sh — boots the llama.cpp NATIVE ROUTER for Saltnitor + Saltcode.
#
# Native-router mode: every model and its per-model flags live in router.ini as
# [sections]. There is NO router.env. Saltnitor's hot-swap deck loads a section by
# name; the Deep Tuner edits a section's flags in place.
#
# If you see ">>> Saltnitor native-router:" in journalctl you are running THIS file.
# If you see ">>> Booting with Saltnitor payload:" you are still on the OLD script.
#
# `exec` is deliberate: it REPLACES this shell with llama-server so systemd tracks the
# server as the unit's main process — required for Ctrl+K (systemctl stop) to kill it.
set -euo pipefail

ROUTER_INI="/home/laz/ai-models/llama.cpp/router.ini"
HOST="127.0.0.1"            # set 0.0.0.0 only if you need LAN access
PORT="8080"

# Locate llama-server. Export LLAMA_SERVER to override, or copy the exact path your
# OLD launch_router.sh used into the candidate list below.
LLAMA_SERVER="${LLAMA_SERVER:-}"
if [[ -z "$LLAMA_SERVER" || ! -x "$LLAMA_SERVER" ]]; then
  for c in \
    /home/laz/ai-models/llama.cpp/build/bin/llama-server \
    /home/laz/ai-models/llama.cpp/llama-server \
    /usr/local/bin/llama-server \
    "$(command -v llama-server 2>/dev/null || true)"; do
    if [[ -n "$c" && -x "$c" ]]; then LLAMA_SERVER="$c"; break; fi
  done
fi
if [[ -z "$LLAMA_SERVER" || ! -x "$LLAMA_SERVER" ]]; then
  echo "launch_router.sh: cannot find llama-server — set LLAMA_SERVER to its path" >&2
  exit 1
fi

echo ">>> Saltnitor native-router: $LLAMA_SERVER --models-preset $ROUTER_INI --models-max 1 --host $HOST --port $PORT"
exec "$LLAMA_SERVER" \
  --models-preset "$ROUTER_INI" \
  --models-max 1 \
  --host "$HOST" \
  --port "$PORT"
  # --api-key "sk-saltnitor-2026"   # uncomment to require a bearer on the router
                                    # (must then match config.toml infer_bearer)
