# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""Compare Rust scan JSON vs Python scan JSON: exact match on strings/bools/arrays,
numeric tolerance on floats. Usage: check_scan.py <rust.json> <py.json>"""
import json
import sys

rust = json.load(open(sys.argv[1]))
py = json.load(open(sys.argv[2]))

STR = ["regime", "verdict", "setup_quality"]
BOOL = ["reversal_confirmed", "slope_ok", "volume_ok", "rsi_ok", "market_ok"]
NUM = ["price", "ema9", "ema20", "ema200", "rsi", "atr", "entry", "take_profit",
       "stop_loss", "upside_pct", "downside_pct", "r_multiple"]
ARR = ["rejection_zones", "support"]
TOL = 1e-4

fails = []
for k in STR + BOOL:
    if rust.get(k) != py.get(k):
        fails.append((k, rust.get(k), py.get(k)))
for k in NUM:
    r, p = rust.get(k), py.get(k)
    if r is None and p is None:
        continue
    if r is None or p is None or abs(float(r) - float(p)) > TOL:
        fails.append((k, r, p))
for k in ARR:
    r = [round(float(x), 2) for x in (rust.get(k) or [])]
    p = [round(float(x), 2) for x in (py.get(k) or [])]
    if r != p:
        fails.append((k, r, p))

for k in STR + BOOL + NUM + ARR:
    mark = "FAIL" if any(f[0] == k for f in fails) else "ok"
    print(f"  {k:20} rust={str(rust.get(k))[:34]:34} py={str(py.get(k))[:34]:34} {mark}")
if fails:
    print(f"\nSCAN PARITY: FAIL ({len(fails)} fields)")
    sys.exit(1)
print("\nSCAN PARITY: PASS")
