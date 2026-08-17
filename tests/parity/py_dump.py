# /// script
# requires-python = ">=3.10"
# dependencies = ["pandas>=2.0","numpy>=1.24"]
# ///
"""Reference dumper: reads an OHLCV csv, runs tools/strategy.compute_indicators, and prints
EVERY bar's indicators as CSV — the ground truth the Rust --parity output is diffed against.
Usage: py_dump.py <ohlcv.csv>"""
import os
import sys

import pandas as pd

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "..", "tools"))
from strategy import compute_indicators  # noqa: E402

csv = sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(__file__), "ohlcv.csv")
d = compute_indicators(pd.read_csv(csv))
cols = ["EMA9", "EMA20", "EMA200", "RSI", "ATR", "VOL_SMA20", "OBV", "OBV_SMA10", "EMA200_20ago"]


def f(x):
    return "" if pd.isna(x) else f"{float(x):.6f}"


print("idx,close,ema9,ema20,ema200,rsi,atr,vol_sma20,obv,obv_sma10,ema200_20ago")
for i in range(len(d)):
    r = d.iloc[i]
    print(",".join([str(i), f"{float(r['Close']):.6f}"] + [f(r[c]) for c in cols]))
