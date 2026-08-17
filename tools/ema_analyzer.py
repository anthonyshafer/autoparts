# /// script
# requires-python = ">=3.10"
# dependencies = [
#   "yfinance>=0.2.40",
#   "pandas>=2.0",
#   "numpy>=1.24",
# ]
# ///
"""
EMA reversal analyzer v2 — 9/20/200 with Tier-1/Tier-2 filters.

Strategy (default timeframe = weekly, 200 EMA ~= 200 weeks ~= 4 years):
  buy discounted (below 200 EMA) AFTER price reclaims the 9 & 20 EMA, targeting the
  200 EMA (the "boss" / expected rejection). v2 adds filters that remove low-odds
  trades: 200-EMA slope, volume/OBV confirmation, market regime (SPY), RSI, ATR stop.

Decision support from indicator rules — not a prediction oracle, not licensed advice.
Nothing here places trades. Log every call in journal/ so accuracy is MEASURED.

Usage:
  uv run tools/ema_analyzer.py BSX
  uv run tools/ema_analyzer.py BSX --amount 50000 --timeframe weekly
  uv run tools/ema_analyzer.py BSX --json
"""
from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass, asdict

import os

import numpy as np
import pandas as pd
import yfinance as yf

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from strategy import compute_indicators, evaluate_row, compute_stop  # noqa: E402


def _fetch(ticker: str, interval: str, period: str) -> pd.DataFrame:
    data = yf.download(
        ticker, period=period, interval=interval,
        auto_adjust=True, progress=False, multi_level_index=False,
    )
    if data is None or data.empty:
        raise SystemExit(f"No data returned for {ticker!r}. Check the symbol.")
    return data.dropna()


def _swing_levels(df: pd.DataFrame, window: int = 5) -> tuple[list[float], list[float]]:
    highs, lows = [], []
    h, l = df["High"].values, df["Low"].values
    for i in range(window, len(df) - window):
        if h[i] == h[i - window : i + window + 1].max():
            highs.append(float(h[i]))
        if l[i] == l[i - window : i + window + 1].min():
            lows.append(float(l[i]))
    return highs, lows


def _market_ok(timeframe: str) -> bool:
    """SPY above its own 200 EMA on the same timeframe = risk-on."""
    try:
        interval = {"weekly": "1wk", "daily": "1d"}[timeframe]
        spy = _fetch("SPY", interval, "10y" if timeframe == "weekly" else "3y")
        spy["EMA200"] = spy["Close"].ewm(span=200, adjust=False).mean()
        last = spy.iloc[-1]
        return bool(last["Close"] >= last["EMA200"])
    except Exception:
        return True  # unknown -> don't block, but note it


@dataclass
class Analysis:
    ticker: str
    timeframe: str
    price: float
    ema9: float
    ema20: float
    ema200: float
    rsi: float
    atr: float
    regime: str
    reversal_confirmed: bool
    slope_ok: bool
    volume_ok: bool
    rsi_ok: bool
    market_ok: bool
    setup_quality: str        # "N/5"
    verdict: str
    entry: float
    take_profit: float
    rejection_zones: list[float]
    support: list[float]
    stop_loss: float
    upside_pct: float
    downside_pct: float
    r_multiple: float
    reasons: list[str]
    fundamentals: dict
    disclaimer: str = (
        "Decision support from indicator rules — not a prediction, not 95% accurate, "
        "not licensed advice. Log the call in journal/ so accuracy is measured."
    )


def analyze(ticker: str, timeframe: str = "weekly", fair_band: float = 0.005) -> Analysis:
    interval = {"weekly": "1wk", "daily": "1d"}[timeframe]
    period = "10y" if timeframe == "weekly" else "3y"
    data = _fetch(ticker, interval, period)
    if len(data) < 60:
        raise SystemExit(
            f"Only {len(data)} {timeframe} candles for {ticker!r}; need history for a 200 EMA."
        )

    market_ok = _market_ok(timeframe)
    a = analyze_frame(data, market_ok, fair_band=fair_band, ticker=ticker, timeframe=timeframe)

    fundamentals = {}
    try:
        info = yf.Ticker(ticker).info
        for k, label in [
            ("shortName", "name"), ("marketCap", "market_cap"), ("trailingPE", "pe"),
            ("targetMeanPrice", "analyst_target"), ("fiftyTwoWeekLow", "52w_low"),
            ("fiftyTwoWeekHigh", "52w_high"),
        ]:
            if info.get(k) is not None:
                fundamentals[label] = info[k]
    except Exception:
        pass
    a.fundamentals = fundamentals
    return a


def analyze_frame(data, market_ok: bool, fair_band: float = 0.005,
                  ticker: str = "", timeframe: str = "weekly") -> Analysis:
    """Compute the full Analysis from an OHLCV frame — no network. Shared by analyze()
    (the live tool) and the Rust-parity dumper, so both exercise identical logic."""
    data = compute_indicators(data)
    last = data.iloc[-1]
    sig = evaluate_row(last, market_ok=market_ok, fair_band=fair_band)

    price = float(last["Close"])
    e9, e20, e200 = float(last["EMA9"]), float(last["EMA20"]), float(last["EMA200"])
    rsi_v, atr_v = float(last["RSI"]), float(last["ATR"])

    highs, lows = _swing_levels(data)
    rej = sorted({round(v, 2) for v in highs if price < v <= e200 * 1.02})
    rejection_zones = sorted(set([round(e200, 2)] + [r for r in rej if abs(r - e200) / e200 > 0.01]))
    below_lows = sorted([v for v in lows if v < price], reverse=True)[:3]
    support = sorted({round(v, 2) for v in ([e20, e9] + below_lows) if v < price}, reverse=True)

    entry = round(price, 2)
    take_profit = round(e200, 2)
    # shared stop rule (same one the backtest measures) — tighter of ATR and structural
    recent_low = below_lows[0] if below_lows else None
    stop_loss = compute_stop(entry, atr_v, e20, recent_low=recent_low, atr_mult=2.0)

    upside_pct = round((take_profit - entry) / entry * 100, 1)
    downside_pct = round((stop_loss - entry) / entry * 100, 1)
    risk = entry - stop_loss
    r_multiple = round((take_profit - entry) / risk, 2) if risk > 0 else float("nan")

    # verdict incorporates the filters
    if sig.entry_ok and r_multiple >= 1.5:
        verdict = "BUY"
    elif sig.entry_ok:
        verdict = "BUY (thin R — size down)"
    elif sig.regime == "DISCOUNT" and sig.reversal_confirmed and not sig.market_ok:
        verdict = "HOLD FIRE — setup ok but market risk-off (SPY<200)"
    elif sig.regime == "DISCOUNT" and sig.reversal_confirmed and not sig.slope_ok:
        verdict = "CAUTION — reclaim ok but 200 sloping down (value-trap risk)"
    elif sig.regime == "DISCOUNT" and not sig.reversal_confirmed:
        verdict = "WATCH — wait for reclaim of 9/20"
    elif sig.regime == "FAIR":
        verdict = "WAIT — near fair value, poor risk/reward"
    else:
        verdict = "AVOID — premium, price above the 200 line"

    return Analysis(
        ticker=ticker.upper(), timeframe=timeframe, price=entry,
        ema9=round(e9, 2), ema20=round(e20, 2), ema200=round(e200, 2),
        rsi=round(rsi_v, 1), atr=round(atr_v, 2),
        regime=sig.regime, reversal_confirmed=sig.reversal_confirmed,
        slope_ok=sig.slope_ok, volume_ok=sig.volume_ok, rsi_ok=sig.rsi_ok,
        market_ok=sig.market_ok, setup_quality=f"{sig.quality}/5",
        verdict=verdict, entry=entry, take_profit=take_profit,
        rejection_zones=rejection_zones, support=support, stop_loss=stop_loss,
        upside_pct=upside_pct, downside_pct=downside_pct, r_multiple=r_multiple,
        reasons=sig.reasons, fundamentals={},
    )


def _money(x: float) -> str:
    return f"${x:,.2f}"


def render(a: Analysis, amount: float) -> str:
    shares = int(amount // a.entry) if a.entry > 0 else 0
    cost = shares * a.entry
    profit = shares * (a.take_profit - a.entry)
    risk_val = shares * (a.entry - a.stop_loss)
    L = []
    L.append(f"=== {a.ticker} — {a.verdict} ===")
    L.append(f"[{a.timeframe}]  price {_money(a.price)}   regime {a.regime}   setup quality {a.setup_quality}")
    L.append("")
    L.append(f"  EMA9 {_money(a.ema9)}   EMA20 {_money(a.ema20)}   EMA200 {_money(a.ema200)} <- target line")
    L.append(f"  RSI {a.rsi}   ATR {_money(a.atr)}")
    L.append("")
    L.append(f"  Filters:  slope {'PASS' if a.slope_ok else 'FAIL'}   "
             f"volume {'PASS' if a.volume_ok else 'FAIL'}   "
             f"rsi {'PASS' if a.rsi_ok else 'FAIL'}   "
             f"market {'PASS' if a.market_ok else 'FAIL'}   "
             f"reversal {'PASS' if a.reversal_confirmed else 'FAIL'}")
    L.append("")
    L.append(f"  Entry        {_money(a.entry)}")
    L.append(f"  Take-profit  {_money(a.take_profit)}  ({a.upside_pct:+.1f}%)  <- expected rejection at 200 EMA")
    L.append(f"  Stop-loss    {_money(a.stop_loss)}  ({a.downside_pct:+.1f}%)  (ATR-based)")
    L.append(f"  R-multiple   {a.r_multiple}")
    L.append("")
    L.append(f"  Rejection zones: {', '.join(_money(z) for z in a.rejection_zones) or 'n/a'}")
    L.append(f"  Support:         {', '.join(_money(z) for z in a.support) or 'n/a'}")
    L.append("")
    L.append(f"  Sizing @ {_money(amount)}: {shares} sh = {_money(cost)}  ->  "
             f"TP +{_money(profit)}  |  stop -{_money(risk_val)}")
    L.append("")
    L.append("  Why:")
    for r in a.reasons:
        L.append(f"    - {r}")
    if a.fundamentals:
        f = a.fundamentals
        bits = []
        if "name" in f: bits.append(str(f["name"]))
        if "pe" in f: bits.append(f"PE {f['pe']:.1f}")
        if "analyst_target" in f: bits.append(f"analyst tgt {_money(f['analyst_target'])}")
        if "52w_low" in f and "52w_high" in f:
            bits.append(f"52w {_money(f['52w_low'])}-{_money(f['52w_high'])}")
        L.append("")
        L.append("  Context: " + "  |  ".join(bits))
    L.append("")
    L.append("  (Decision support from your EMA rules — not a prediction, not advice. Log it in journal/.)")
    return "\n".join(L)


def main() -> None:
    p = argparse.ArgumentParser(description="EMA reversal analyzer v2 (9/20/200 + filters).")
    p.add_argument("ticker")
    p.add_argument("--amount", type=float, default=50000)
    p.add_argument("--timeframe", choices=["weekly", "daily"], default="weekly")
    p.add_argument("--json", action="store_true")
    args = p.parse_args()
    a = analyze(args.ticker, timeframe=args.timeframe)
    if args.json:
        # JSON has no Infinity/NaN literal; emit null for non-finite (e.g. an infinite PE)
        # so the desktop UI can JSON.parse the output.
        def _san(o):
            if isinstance(o, float):
                return o if np.isfinite(o) else None
            if isinstance(o, dict):
                return {k: _san(v) for k, v in o.items()}
            if isinstance(o, list):
                return [_san(v) for v in o]
            return o
        print(json.dumps(_san(asdict(a)), indent=2, default=float, allow_nan=False))
    else:
        print(render(a, args.amount))


if __name__ == "__main__":
    main()
