# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""Per-bar, per-field parity check of Rust vs Python indicator CSVs.
Usage: check.py <rust.csv> <py.csv>  -> exit 0 if every bar/field within tol, else 1."""
import csv
import sys

ABS_TOL = 2e-6   # both sides print %.6f from near-identical raw floats; allow last-digit rounding
REL_TOL = 1e-6

def load(p):
    with open(p) as f:
        return list(csv.DictReader(f))

rust, py = load(sys.argv[1]), load(sys.argv[2])
fields = ["ema9", "ema20", "ema200", "rsi", "atr", "vol_sma20", "obv", "obv_sma10", "ema200_20ago"]

if len(rust) != len(py):
    print(f"ROW COUNT mismatch: rust={len(rust)} py={len(py)}")
    sys.exit(1)

maxdiff = {f: 0.0 for f in fields}
fails = []
for i, (r, p) in enumerate(zip(rust, py)):
    for f in fields:
        rv, pv = r[f].strip(), p[f].strip()
        if rv == "" and pv == "":
            continue
        if rv == "" or pv == "":
            fails.append((i, f, rv or "null", pv or "null", "one-null"))
            continue
        d = abs(float(rv) - float(pv))
        maxdiff[f] = max(maxdiff[f], d)
        if d > max(ABS_TOL, REL_TOL * abs(float(pv))):
            fails.append((i, f, rv, pv, f"{d:.2e}"))

print(f"bars compared: {len(rust)}")
print("max per-field diff:  " + "  ".join(f"{f}={maxdiff[f]:.1e}" for f in fields))
if fails:
    print(f"FAILS: {len(fails)} (showing first 8)")
    for row in fails[:8]:
        print(f"  bar {row[0]} {row[1]}: rust={row[2]} py={row[3]} diff={row[4]}")
    print("PARITY: FAIL")
    sys.exit(1)
print("PARITY: PASS (all bars, all fields)")
