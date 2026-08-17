#!/usr/bin/env bash
# Full parity suite: build the Rust binary, then diff Rust vs Python indicators PER BAR
# across every fixture (main + edge cases). Exit non-zero if any fixture diverges.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
BIN="desktop/src-tauri/target/debug/stockscanner"

echo "== building rust binary =="
( cd desktop/src-tauri && cargo build --bin stockscanner --quiet )
echo "== generating fixtures =="
uv run tests/parity/gen_fixture.py >/dev/null

FIX=(ohlcv flat rising tailflat short16)
FAIL=0
for f in "${FIX[@]}"; do
  csv="tests/parity/$f.csv"
  "$BIN" --parity "$csv" > "/tmp/rust_$f.csv"
  uv run tests/parity/py_dump.py "$csv" > "/tmp/py_$f.csv" 2>/dev/null
  echo "---- fixture: $f ----"
  if uv run tests/parity/check.py "/tmp/rust_$f.csv" "/tmp/py_$f.csv"; then :; else FAIL=1; fi
done

echo ""
if [ "$FAIL" -eq 0 ]; then echo "ALL FIXTURES: PARITY PASS"; else echo "SOME FIXTURES FAILED"; exit 1; fi
