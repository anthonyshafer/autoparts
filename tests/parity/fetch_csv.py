# /// script
# requires-python = ">=3.10"
# dependencies = ["pandas>=2.0","numpy>=1.24","yfinance>=0.2.40"]
# ///
"""Fetch a ticker's OHLCV once (yfinance, same params as backtest.py) and save to CSV, so
Rust and Python run the backtest on IDENTICAL data — deterministic parity, no live timing.
Usage: fetch_csv.py <TICKER> <weekly|daily> <out.csv>"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "..", "tools"))
from backtest import _fetch  # noqa: E402

ticker, tf, out = sys.argv[1], sys.argv[2], sys.argv[3]
interval = {"weekly": "1wk", "daily": "1d"}[tf]
period = "15y" if tf == "weekly" else "5y"
d = _fetch(ticker, interval, period)
d = d[["Open", "High", "Low", "Close", "Volume"]]
d.to_csv(out, index=False)
print(f"wrote {out} ({len(d)} rows)")
