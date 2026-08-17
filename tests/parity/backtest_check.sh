#!/usr/bin/env bash
# Backtest parity: Rust engine::backtest vs Python backtest_frame on IDENTICAL fetched data,
# across tickers / timeframes / (atr_mult,max_hold) — including boundary + high-trade-count
# cases. Fetches each ticker once to CSV so it's deterministic given the data.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
BIN="desktop/src-tauri/target/debug/stockscanner"
[ -x "$BIN" ] || ( cd desktop/src-tauri && cargo build --bin stockscanner --quiet )

# ticker timeframe atr_mult max_hold
CASES=(
  "BSX weekly 2.0 52" "PFE weekly 2.0 52" "INTC weekly 2.0 52" "NKE weekly 2.0 52"
  "AMC daily 2.0 52" "AMC daily 3.0 26" "MU daily 1.5 104" "MU daily 2.0 52" "PLUG daily 2.0 52"
)
FAIL=0
for c in "${CASES[@]}"; do
  read -r T TF AM MH <<<"$c"
  uv run tests/parity/fetch_csv.py "$T" "$TF" /tmp/bt_$T.csv >/dev/null 2>&1
  "$BIN" --backtest-parity /tmp/bt_$T.csv "$AM" "$MH" > /tmp/bt_r.json 2>/dev/null
  uv run tests/parity/py_bt_dump.py /tmp/bt_$T.csv "$AM" "$MH" > /tmp/bt_p.json 2>/dev/null
  if uv run tests/parity/check_bt.py /tmp/bt_r.json /tmp/bt_p.json >/tmp/bt_o.txt 2>/dev/null; then
    echo "$c: PASS ($(grep -o 'trades=[0-9]*' /tmp/bt_o.txt))"
  else echo "$c: FAIL"; grep -A8 'FAIL' /tmp/bt_o.txt; FAIL=1; fi
done
echo ""
[ "$FAIL" -eq 0 ] && echo "BACKTEST PARITY: ALL PASS" || { echo "BACKTEST PARITY: FAIL"; exit 1; }
