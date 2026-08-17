# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""Compare Rust backtest JSON vs Python backtest JSON: summary stats + per-trade log.
Usage: check_bt.py <rust.json> <py.json>"""
import json
import sys

rust = json.load(open(sys.argv[1]))
py = json.load(open(sys.argv[2]))
fails = []

for k in ["bars", "trades", "wins", "losses", "timeouts", "note"]:
    if rust.get(k) != py.get(k):
        fails.append(f"{k}: rust={rust.get(k)} py={py.get(k)}")
for k in ["win_rate", "avg_r", "total_r"]:
    if abs(float(rust.get(k, 0)) - float(py.get(k, 0))) > 1e-9:
        fails.append(f"{k}: rust={rust.get(k)} py={py.get(k)}")
# profit_factor: both null, or both finite & equal
rp, pp = rust.get("profit_factor"), py.get("profit_factor")
if (rp is None) != (pp is None) or (rp is not None and abs(float(rp) - float(pp)) > 1e-9):
    fails.append(f"profit_factor: rust={rp} py={pp}")

rl, pl = rust.get("log", []), py.get("log", [])
if len(rl) != len(pl):
    fails.append(f"log length: rust={len(rl)} py={len(pl)}")
else:
    for i, (rt, pt) in enumerate(zip(rl, pl)):
        for k in ["entry_idx", "exit_idx", "bars_held", "outcome"]:
            if rt.get(k) != pt.get(k):
                fails.append(f"trade {i} {k}: rust={rt.get(k)} py={pt.get(k)}")
        for k in ["entry", "exit", "stop", "target", "r"]:
            if abs(float(rt.get(k, 0)) - float(pt.get(k, 0))) > 1e-9:
                fails.append(f"trade {i} {k}: rust={rt.get(k)} py={pt.get(k)}")

print(f"trades={py.get('trades')} wins={py.get('wins')} losses={py.get('losses')} "
      f"timeouts={py.get('timeouts')} avg_r={py.get('avg_r')} pf={py.get('profit_factor')}")
if fails:
    print(f"BACKTEST PARITY: FAIL ({len(fails)})")
    for f in fails[:12]:
        print("  " + f)
    sys.exit(1)
print("BACKTEST PARITY: PASS")
