# /// script
# requires-python = ">=3.10"
# dependencies = ["pandas>=2.0","numpy>=1.24","yfinance>=0.2.40"]
# ///
"""Reference scan dumper: reads an OHLCV csv, runs the SHARED ema_analyzer.analyze_frame
(no network), and prints the scan result JSON — ground truth for the Rust --scan-parity.
Usage: py_scan_dump.py <ohlcv.csv> <0|1 market_ok>"""
import json
import os
import sys

import pandas as pd

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "..", "tools"))
from ema_analyzer import analyze_frame  # noqa: E402

csv = sys.argv[1]
market_ok = (len(sys.argv) > 2 and sys.argv[2] == "1")
a = analyze_frame(pd.read_csv(csv), market_ok=market_ok, fair_band=0.005, ticker="TEST", timeframe="weekly")


def num(x):
    return None if x is None or (isinstance(x, float) and pd.isna(x)) else float(x)


print(json.dumps({
    "regime": a.regime, "verdict": a.verdict,
    "reversal_confirmed": a.reversal_confirmed, "slope_ok": a.slope_ok,
    "volume_ok": a.volume_ok, "rsi_ok": a.rsi_ok, "market_ok": a.market_ok,
    "setup_quality": a.setup_quality, "price": num(a.price),
    "ema9": num(a.ema9), "ema20": num(a.ema20), "ema200": num(a.ema200),
    "rsi": num(a.rsi), "atr": num(a.atr), "entry": num(a.entry),
    "take_profit": num(a.take_profit), "stop_loss": num(a.stop_loss),
    "upside_pct": num(a.upside_pct), "downside_pct": num(a.downside_pct),
    "r_multiple": num(a.r_multiple),
    "rejection_zones": [round(float(x), 2) for x in a.rejection_zones],
    "support": [round(float(x), 2) for x in a.support],
}))
