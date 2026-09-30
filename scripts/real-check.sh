#!/usr/bin/env bash
# R1 real-world check (CR-2). [HW]: loads and evicts models on the real GPU.
# Runs a fixed journey through Saltnitor against the LIVE router and records statuses,
# timings and response key paths — never generated text. Run only after the operator says go.
#
#   scripts/real-check.sh --label NAME --a PROFILE --b PROFILE --yes
#       [--router URL] [--control URL] [--config PATH]
# Requires: bash ≥ 4.4, curl, python3 ≥ 3.11 (tomllib), cargo, script (util-linux), ps.
set -Eeuo pipefail
for cmd in curl python3 cargo script ps; do
  command -v "$cmd" >/dev/null || { printf 'real-check: missing dependency %s\n' "$cmd" >&2; exit 2; }
done
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
LABEL="" A="" B="" YES=0
ROUTER="http://127.0.0.1:8080" CONTROL="http://127.0.0.1:8765"
CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}/saltnitor/config.toml"
while [[ $# -gt 0 ]]; do
  case $1 in
    --label) LABEL=$2; shift 2 ;;
    --a) A=$2; shift 2 ;;
    --b) B=$2; shift 2 ;;
    --router) ROUTER=$2; shift 2 ;;
    --control) CONTROL=$2; shift 2 ;;
    --config) CONFIG=$2; shift 2 ;;
    --yes) YES=1; shift ;;
    *) echo "real-check: unknown argument $1" >&2; exit 2 ;;
  esac
done
[[ -n $LABEL && -n $A && -n $B ]] || { echo "real-check: --label, --a and --b are required" >&2; exit 2; }
[[ $LABEL =~ ^[a-z0-9][a-z0-9_-]*$ ]] || { echo "real-check: --label must match [a-z0-9_-] (it becomes a directory name)" >&2; exit 2; }
[[ $A =~ ^[A-Za-z0-9_.-]+$ && $B =~ ^[A-Za-z0-9_.-]+$ ]] || { echo "real-check: profile names must match [A-Za-z0-9_.-]" >&2; exit 2; }
[[ -r $CONFIG ]] || { echo "real-check: cannot read $CONFIG" >&2; exit 2; }
[[ $YES == 1 ]] || { echo "real-check: this loads and evicts models on the GPU; pass --yes once the operator has said go" >&2; exit 2; }
OUT="tests/fixtures/captures/$LABEL"
mkdir -p "$OUT"
WORK="$(mktemp -d)"

# Secrets are read from the live config into this process only; never printed or written.
eval "$(python3 - "$CONFIG" <<'PY'
import shlex, sys, tomllib
c = tomllib.load(open(sys.argv[1], "rb"))
print(f"export RC_CONTROL_TOKEN={shlex.quote(c.get('control_token') or '')}")
print(f"export RC_INFER_BEARER={shlex.quote(c.get('infer_bearer') or '')}")
PY
)"

auth_router=(); [[ -n $RC_INFER_BEARER ]] && auth_router=(-H "Authorization: Bearer $RC_INFER_BEARER")
auth_control=(); [[ -n $RC_CONTROL_TOKEN ]] && auth_control=(-H "Authorization: Bearer $RC_CONTROL_TOKEN")

curl -fsS -o /dev/null --max-time 5 "${auth_router[@]}" "$ROUTER/health" \
  || { echo "real-check: router $ROUTER is not up — BLOCKED" >&2; exit 3; }

cargo build --release --locked -p saltnitor -p fake-llama-server >/dev/null

# R0 alongside (read-only). The P0 baseline r0 is captured once (Task 8) and not overwritten.
if [[ $LABEL != baseline || ! -d $OUT/r0 ]]; then
  base_args=(); [[ $LABEL != baseline && -d tests/fixtures/captures/baseline/r0 ]] \
    && base_args=(--baseline tests/fixtures/captures/baseline/r0)
  RC_BEARER="$RC_INFER_BEARER" target/release/fake-llama-server record --upstream "$ROUTER" \
    --out "$OUT/r0" --bearer-env RC_BEARER "${base_args[@]}" || true
fi

STARTED_PID=""
cleanup() {
  if [[ -n $STARTED_PID ]]; then
    local child
    child=$(ps -o pid= --ppid "$STARTED_PID" | tr -d ' ' || true)
    [[ -n $child ]] && kill "$child" 2>/dev/null || true
    kill "$STARTED_PID" 2>/dev/null || true
  fi
  rm -rf "$WORK"
}
trap cleanup EXIT

if curl -fsS -o /dev/null --max-time 2 "$CONTROL/healthz"; then
  echo "real-check: using the running Saltnitor at $CONTROL"
else
  script -qfec "$ROOT/target/release/saltnitor" /dev/null </dev/null >/dev/null 2>&1 &
  STARTED_PID=$!
  for _ in $(seq 1 100); do
    curl -fsS -o /dev/null --max-time 1 "$CONTROL/healthz" && break
    sleep 0.2
  done
  curl -fsS -o /dev/null --max-time 2 "$CONTROL/healthz" \
    || { echo "real-check: Saltnitor control API did not come up at $CONTROL" >&2; exit 4; }
fi

step() { # name, method, path, body(optional) -> $WORK/<n>.{body,meta}
  local n=$1 method=$2 path=$3 body=${4:-}
  local args=(-sS -o "$WORK/$n.body" -w '%{http_code} %{time_starttransfer} %{time_total}' --max-time 600 "${auth_control[@]}")
  [[ -n $body ]] && args+=(-H 'Content-Type: application/json' -d "$body")
  curl "${args[@]}" -X "$method" "$CONTROL$path" >"$WORK/$n.meta" || echo "000 0 0" >"$WORK/$n.meta"
}

step 1 POST /v1/ensure "{\"profile\":\"$A\"}"
step 2 POST /v1/chat/completions "{\"model\":\"$A\",\"stream\":true,\"max_tokens\":16,\"messages\":[{\"role\":\"user\",\"content\":\"Reply with one short word.\"}]}"
step 3 POST /v1/ensure "{\"profile\":\"$B\"}"
step 4 GET /v1/status

python3 - "$WORK" "$OUT/r1.json" "$LABEL" "$A" "$B" <<'PY'
import json, pathlib, sys, time
work, out, label, a, b = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2]), *sys.argv[3:6]
def keys(v, at=""):
    s = set()
    if isinstance(v, dict):
        if at: s.add(at)
        for k, x in v.items(): s |= keys(x, f"{at}.{k}" if at else k)
    elif isinstance(v, list):
        s.add(at + "[]")
        for x in v: s |= keys(x, at + "[]")
    else:
        s.add(at)
    return s
names = {1: f"ensure {a}", 2: f"chat stream {a}", 3: f"ensure {b}", 4: "status"}
steps, ok = [], True
for n, name in names.items():
    code, ttfb, total = (work / f"{n}.meta").read_text().split()
    body = (work / f"{n}.body").read_text(errors="replace") if (work / f"{n}.body").exists() else ""
    rec = {"step": name, "http": int(code), "ttfb_ms": round(float(ttfb) * 1000), "total_ms": round(float(total) * 1000)}
    if n == 2:
        data = [l[6:] for l in body.splitlines() if l.startswith("data: ")]
        rec |= {"chunks": sum(d != "[DONE]" for d in data), "done": "[DONE]" in data}
    else:
        try:
            v = json.loads(body)
            rec |= {"outcome": v.get("status"), "load_ms": v.get("load_ms"), "keys": sorted(keys(v))}
        except json.JSONDecodeError:
            rec |= {"outcome": None, "keys": []}
    ok &= 200 <= rec["http"] < 300
    steps.append(rec)
out.write_text(json.dumps({"label": label, "captured_unix": int(time.time()), "a": a, "b": b, "steps": steps}, indent=2) + "\n")
base = pathlib.Path("tests/fixtures/captures/baseline/r1.json")
prev = {s["step"]: s for s in json.loads(base.read_text())["steps"]} if base.exists() and label != "baseline" else {}
print(f"{'STEP':<22} {'HTTP':>4} {'OUTCOME':<17} {'TTFB ms':>8} {'TOTAL ms':>9}  vs baseline")
for s in steps:
    p = prev.get(s["step"])
    cmp = "-" if not p else f"http {p['http']}→{s['http']}, ttfb {p['ttfb_ms']}→{s['ttfb_ms']}" + (
        "" if p.get("keys") == s.get("keys") else ", KEYS CHANGED")
    print(f"{s['step']:<22} {s['http']:>4} {str(s.get('outcome') or ('done' if s.get('done') else '-')):<17} {s['ttfb_ms']:>8} {s['total_ms']:>9}  {cmp}")
sys.exit(0 if ok else 5)
PY
