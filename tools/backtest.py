# /// script
# requires-python = ">=3.10"
# dependencies = [
#   "yfinance>=0.2.40",
#   "pandas>=2.0",
#   "numpy>=1.24",
# ]
# ///
"""
Walk-forward backtest for the 9/20/200 EMA reversal system.

Measures the strategy instead of guessing at it. For each ticker it:
  1. builds the same indicators + signal the live analyzer uses (from strategy.py),
  2. enters at the bar close when entry_ok fires (and flat),
  3. sets target = 200 EMA snapshot, stop = 2*ATR below entry,
  4. walks forward bar-by-bar until stop, target, or max-hold,
  5. reports win-rate, avg R, expectancy (avg R per trade), profit factor.

No lookahead: the decision at bar i uses only indicators through bar i; fills happen
on bar i's close and exits on bars i+1..  Results are HYPOTHETICAL — past behavior of
a rule set, not a promise. Small trade counts are noise; the report flags that.

Usage:
  uv run tools/backtest.py BSX
  uv run tools/backtest.py BSX ABBV PFE --timeframe weekly --max-hold 52
  uv run tools/backtest.py BSX --json
"""
from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass, asdict, field

import os

import numpy as np
import pandas as pd
import yfinance as yf

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from strategy import compute_indicators, evaluate_row, compute_stop  # noqa: E402


def _fetch(ticker: str, interval: str, period: str) -> pd.DataFrame:
    d = yf.download(ticker, period=period, interval=interval, auto_adjust=True,
                    progress=False, multi_level_index=False)
    if d is None or d.empty:
        raise SystemExit(f"No data for {ticker!r}.")
    return d.dropna()


def _market_series(timeframe: str, index: pd.Index) -> pd.Series:
    """Boolean per-date: SPY above its own 200 EMA, aligned to the stock's dates."""
    try:
        interval = {"weekly": "1wk", "daily": "1d"}[timeframe]
        spy = _fetch("SPY", interval, "15y" if timeframe == "weekly" else "5y")
        spy_ema = spy["Close"].ewm(span=200, adjust=False).mean()
        ok = (spy["Close"] >= spy_ema)
        return ok.reindex(index, method="ffill").fillna(False)
    except Exception:
        return pd.Series(True, index=index)  # unknown -> permissive


@dataclass
class Trade:
    entry_date: str
    exit_date: str
    entry: float
    exit: float
    stop: float
    target: float
    bars_held: int
    outcome: str   # WIN / LOSS / TIMEOUT
    r: float


@dataclass
class BacktestResult:
    ticker: str
    timeframe: str
    bars: int
    trades: int
    wins: int
    losses: int
    timeouts: int
    win_rate: float
    avg_r: float
    expectancy: float      # same as avg_r; kept explicit
    profit_factor: float
    total_r: float
    note: str
    trade_log: list = field(default_factory=list)


def backtest(ticker: str, timeframe: str = "weekly", atr_mult: float = 2.0,
             max_hold: int = 52) -> BacktestResult:
    interval = {"weekly": "1wk", "daily": "1d"}[timeframe]
    period = "15y" if timeframe == "weekly" else "5y"
    df = compute_indicators(_fetch(ticker, interval, period))
    market = _market_series(timeframe, df.index)

    start = 200  # need the 200 EMA warmed
    trades: list[Trade] = []
    i = start
    n = len(df)
    while i < n - 1:
        row = df.iloc[i]
        if pd.isna(row["EMA200"]) or pd.isna(row["ATR"]):
            i += 1
            continue
        sig = evaluate_row(row, market_ok=bool(market.iloc[i]))
        if not sig.entry_ok:
            i += 1
            continue

        entry = float(row["Close"])
        target = float(row["EMA200"])
        # same stop rule the live tool recommends (shared compute_stop), so the measured
        # edge matches the traded rule. recent_low = min low of the prior 10 bars.
        recent_low = float(df["Low"].iloc[max(0, i - 10):i].min()) if i > 0 else None
        stop = compute_stop(entry, float(row["ATR"]), float(row["EMA20"]),
                            recent_low=recent_low, atr_mult=atr_mult)
        if stop <= 0 or target <= entry:
            i += 1
            continue
        risk = entry - stop
        # only count trades a user acting on a "BUY" would take: R >= 1.5
        if (target - entry) / risk < 1.5:
            i += 1
            continue

        # walk forward
        exit_price = float(df.iloc[min(i + max_hold, n - 1)]["Close"])
        exit_idx = min(i + max_hold, n - 1)
        outcome = "TIMEOUT"
        for j in range(i + 1, min(i + max_hold + 1, n)):
            bar = df.iloc[j]
            hit_stop = float(bar["Low"]) <= stop
            hit_tgt = float(bar["High"]) >= target
            if hit_stop and hit_tgt:      # ambiguous bar -> assume stop first (conservative)
                exit_price, exit_idx, outcome = stop, j, "LOSS"
                break
            if hit_stop:
                exit_price, exit_idx, outcome = stop, j, "LOSS"
                break
            if hit_tgt:
                exit_price, exit_idx, outcome = target, j, "WIN"
                break
            exit_price, exit_idx = float(bar["Close"]), j

        r = (exit_price - entry) / risk
        trades.append(Trade(
            entry_date=str(df.index[i].date()), exit_date=str(df.index[exit_idx].date()),
            entry=round(entry, 2), exit=round(exit_price, 2), stop=round(stop, 2),
            target=round(target, 2), bars_held=exit_idx - i, outcome=outcome, r=round(r, 2),
        ))
        i = exit_idx + 1  # no overlapping positions

    wins = sum(1 for t in trades if t.outcome == "WIN")
    losses = sum(1 for t in trades if t.outcome == "LOSS")
    timeouts = sum(1 for t in trades if t.outcome == "TIMEOUT")
    rs = [t.r for t in trades]
    total_r = round(sum(rs), 2)
    avg_r = round(np.mean(rs), 2) if rs else 0.0
    win_rate = round(100 * wins / len(trades), 1) if trades else 0.0
    gross_win = sum(r for r in rs if r > 0)
    gross_loss = -sum(r for r in rs if r < 0)
    profit_factor = round(gross_win / gross_loss, 2) if gross_loss > 0 else float("inf")

    if len(trades) < 10:
        note = f"ONLY {len(trades)} trades — statistically thin; treat as anecdote, not edge."
    elif expectancy_ok(avg_r):
        note = "positive expectancy over a usable sample — but hypothetical, keep logging live."
    else:
        note = "negative/weak expectancy — this rule set did not pay on this ticker historically."

    return BacktestResult(
        ticker=ticker.upper(), timeframe=timeframe, bars=n, trades=len(trades),
        wins=wins, losses=losses, timeouts=timeouts, win_rate=win_rate, avg_r=avg_r,
        expectancy=avg_r, profit_factor=profit_factor, total_r=total_r, note=note,
        trade_log=[asdict(t) for t in trades],
    )


def expectancy_ok(avg_r: float) -> bool:
    return avg_r > 0.1


def render(r: BacktestResult) -> str:
    L = []
    L.append(f"=== BACKTEST {r.ticker} [{r.timeframe}] — {r.bars} bars ===")
    L.append(f"  trades {r.trades}   wins {r.wins}  losses {r.losses}  timeouts {r.timeouts}")
    L.append(f"  win-rate {r.win_rate}%   avg R {r.avg_r}   expectancy {r.expectancy} R/trade")
    L.append(f"  profit factor {r.profit_factor}   total {r.total_r} R")
    L.append(f"  >> {r.note}")
    if r.trade_log:
        L.append("  last trades:")
        for t in r.trade_log[-5:]:
            L.append(f"    {t['entry_date']} -> {t['exit_date']}  {t['outcome']:7s} "
                     f"entry {t['entry']} exit {t['exit']}  {t['r']:+.2f}R")
    return "\n".join(L)


def main() -> None:
    p = argparse.ArgumentParser(description="Walk-forward backtest of the EMA reversal system.")
    p.add_argument("tickers", nargs="+")
    p.add_argument("--timeframe", choices=["weekly", "daily"], default="weekly")
    p.add_argument("--atr-mult", type=float, default=2.0)
    p.add_argument("--max-hold", type=int, default=52)
    p.add_argument("--json", action="store_true")
    args = p.parse_args()

    results = [backtest(t, args.timeframe, args.atr_mult, args.max_hold) for t in args.tickers]
    if args.json:
        # JSON has no Infinity/NaN literal; emit null for non-finite so JS can parse it.
        def _san(o):
            if isinstance(o, float):
                return o if np.isfinite(o) else None
            if isinstance(o, dict):
                return {k: _san(v) for k, v in o.items()}
            if isinstance(o, list):
                return [_san(v) for v in o]
            return o
        print(json.dumps([_san(asdict(r)) for r in results], indent=2, default=float,
                         allow_nan=False))
    else:
        print("\n\n".join(render(r) for r in results))
        if len(results) > 1:
            agg = [t["r"] for r in results for t in r.trade_log]
            if agg:
                print(f"\n=== PORTFOLIO: {len(agg)} trades  avg {np.mean(agg):+.2f}R  "
                      f"total {sum(agg):+.1f}R ===")


if __name__ == "__main__":
    main()
