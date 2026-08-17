#!/usr/bin/env bash
# Live integration parity: Rust end-to-end (--fetch-scan) vs the Python tool (stocks.py scan)
# on a basket of tickers. Both hit Yahoo, so verdict must match exactly and price levels to 2c.
# NOTE: uses live network data — run when markets data is reachable.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
BIN="desktop/src-tauri/target/debug/swingr"
[ -x "$BIN" ] || ( cd desktop/src-tauri && cargo build --bin swingr --quiet )

TICKERS=(${@:-BSX PFE SOFI UBER ZETA KO JNJ AAPL MSFT INTC})
FAIL=0
for T in "${TICKERS[@]}"; do
  R=$("$BIN" --fetch-scan "$T" weekly 2>/dev/null)
  P=$(uv run tools/stocks.py scan "$T" --json 2>/dev/null)
  if [ -z "$R" ] || [ -z "$P" ]; then echo "$T: FETCH ERROR"; FAIL=1; continue; fi
  python3 - "$R" "$P" <<'PY' || FAIL=1
import sys, json
r = json.loads(sys.argv[1]); p = json.loads(sys.argv[2])
ok = True; msgs = []
if r['verdict'] != p['verdict']:
    ok = False; msgs.append(f"verdict {r['verdict']!r}!={p['verdict']!r}")
for k in ['entry', 'ema200', 'stop_loss', 'take_profit', 'r_multiple', 'upside_pct', 'downside_pct']:
    rv, pv = r.get(k), p.get(k)
    if rv is None or pv is None:
        continue
    if abs(float(rv) - float(pv)) > 0.02:
        ok = False; msgs.append(f"{k} {rv}!={pv}")
print(f"{p['ticker']}: {'PASS' if ok else 'FAIL — ' + '; '.join(msgs)}  [{p['verdict'][:22]}]")
sys.exit(0 if ok else 1)
PY
done
echo ""
[ "$FAIL" -eq 0 ] && echo "LIVE SCAN PARITY: PASS" || { echo "LIVE SCAN PARITY: FAIL"; exit 1; }
