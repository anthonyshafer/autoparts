# /// script
# requires-python = ">=3.10"
# dependencies = ["pandas>=2.0","numpy>=1.24","yfinance>=0.2.40"]
# ///
"""Reference backtest dumper: reads an OHLCV csv, runs the SHARED backtest_frame (no
network, market=all-True since it doesn't affect trades), prints BacktestResult JSON.
Usage: py_bt_dump.py <ohlcv.csv> [atr_mult] [max_hold]"""
import json
import math
import os
import sys

import pandas as pd

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "..", "tools"))
from strategy import compute_indicators  # noqa: E402
from backtest import backtest_frame  # noqa: E402

csv = sys.argv[1]
atr_mult = float(sys.argv[2]) if len(sys.argv) > 2 else 2.0
max_hold = int(sys.argv[3]) if len(sys.argv) > 3 else 52
df = compute_indicators(pd.read_csv(csv))
r = backtest_frame(df, [True] * len(df), atr_mult, max_hold)

log = [{
    "entry_idx": int(t["entry_date"]), "exit_idx": int(t["exit_date"]),
    "entry": t["entry"], "exit": t["exit"], "stop": t["stop"], "target": t["target"],
    "bars_held": t["bars_held"], "outcome": t["outcome"], "r": t["r"],
} for t in r.trade_log]

print(json.dumps({
    "bars": r.bars, "trades": r.trades, "wins": r.wins, "losses": r.losses,
    "timeouts": r.timeouts, "win_rate": r.win_rate, "avg_r": r.avg_r,
    "profit_factor": (None if math.isinf(r.profit_factor) else r.profit_factor),
    "total_r": r.total_r, "note": r.note, "log": log,
}))
