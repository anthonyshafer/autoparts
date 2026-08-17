"""
Shared strategy logic for the 9/20/200 EMA reversal system.

Imported by both ema_analyzer.py (live read) and backtest.py (historical measurement)
so the rules that get REPORTED are the exact rules that get TESTED. No drift.

v2 filters layered on the core "gladiators -> boss" rule:
  Tier 1 (kill low-odds trades):
    - 200 EMA slope must be flat/rising  (avoid value traps)
    - volume/OBV must confirm the reclaim (avoid fakeouts)
    - broad market (SPY) above its own 200 EMA (longs win more in uptrends)
  Tier 2 (honest risk):
    - ATR-based stop (adapts to the stock's real volatility)
    - RSI(14) not overbought into the target

This is decision support computed from indicator rules — not a prediction oracle
and not licensed advice.
"""
from __future__ import annotations

from dataclasses import dataclass
import numpy as np
import pandas as pd


# ---------- indicators ----------

def ema(series: pd.Series, span: int) -> pd.Series:
    return series.ewm(span=span, adjust=False).mean()


def rsi(close: pd.Series, period: int = 14) -> pd.Series:
    delta = close.diff()
    gain = delta.clip(lower=0.0)
    loss = -delta.clip(upper=0.0)
    avg_gain = gain.ewm(alpha=1 / period, adjust=False).mean()
    avg_loss = loss.ewm(alpha=1 / period, adjust=False).mean()
    rs = avg_gain / avg_loss.replace(0.0, np.nan)
    out = 100 - (100 / (1 + rs))
    return out.fillna(50.0)


def atr(df: pd.DataFrame, period: int = 14) -> pd.Series:
    high, low, close = df["High"], df["Low"], df["Close"]
    prev_close = close.shift(1)
    tr = pd.concat(
        [(high - low), (high - prev_close).abs(), (low - prev_close).abs()],
        axis=1,
    ).max(axis=1)
    return tr.ewm(alpha=1 / period, adjust=False).mean()


def obv(close: pd.Series, volume: pd.Series) -> pd.Series:
    direction = np.sign(close.diff().fillna(0.0))
    return (direction * volume).cumsum()


def compute_indicators(df: pd.DataFrame) -> pd.DataFrame:
    df = df.copy()
    c = df["Close"]
    df["EMA9"] = ema(c, 9)
    df["EMA20"] = ema(c, 20)
    df["EMA200"] = ema(c, 200)
    df["RSI"] = rsi(c, 14)
    df["ATR"] = atr(df, 14)
    df["VOL_SMA20"] = df["Volume"].rolling(20).mean()
    df["OBV"] = obv(c, df["Volume"])
    df["OBV_SMA10"] = df["OBV"].rolling(10).mean()
    df["EMA200_20ago"] = df["EMA200"].shift(20)  # slope reference ~20 bars back
    return df


# ---------- signal ----------

@dataclass
class Signal:
    regime: str            # DISCOUNT / FAIR / PREMIUM
    reversal_confirmed: bool
    slope_ok: bool
    volume_ok: bool
    rsi_ok: bool
    market_ok: bool
    quality: int           # 0-5 checklist (how many filters passed)
    entry_ok: bool         # all hard gates true -> actionable long
    reasons: list[str]


def evaluate_row(row: pd.Series, market_ok: bool, fair_band: float = 0.005) -> Signal:
    """Decide the signal at a single bar close using only that row's indicators."""
    price = float(row["Close"])
    e9, e20, e200 = float(row["EMA9"]), float(row["EMA20"]), float(row["EMA200"])

    if price < e200 * (1 - fair_band):
        regime = "DISCOUNT"
    elif price > e200 * (1 + fair_band):
        regime = "PREMIUM"
    else:
        regime = "FAIR"

    reversal_confirmed = price > e9 and price > e20 and e9 > e20

    # Tier 1 / Tier 2 filters
    e200_ref = row.get("EMA200_20ago", np.nan)
    slope_ok = bool(pd.notna(e200_ref) and e200 >= e200_ref)  # flat-to-rising 200
    vol_sma = row.get("VOL_SMA20", np.nan)
    obv_sma = row.get("OBV_SMA10", np.nan)
    volume_ok = bool(
        (pd.notna(vol_sma) and float(row["Volume"]) >= vol_sma)
        or (pd.notna(obv_sma) and float(row["OBV"]) >= obv_sma)
    )
    rsi_ok = bool(pd.notna(row.get("RSI", np.nan)) and float(row["RSI"]) < 70.0)

    quality = sum([slope_ok, volume_ok, rsi_ok, market_ok, reversal_confirmed])

    # Hard gates = the CORE strategy: discounted + 9/20 reclaim + not overbought.
    # slope_ok and market_ok are RISK WARNINGS (they lower quality and flag value-trap /
    # risk-off), not blockers — requiring a rising 200 EMA contradicts "buy the discount"
    # (the 200 lags and is usually flat/falling at a genuine discount reclaim).
    entry_ok = regime == "DISCOUNT" and reversal_confirmed and rsi_ok

    reasons: list[str] = []
    reasons.append(f"regime={regime} (price {price:.2f} vs 200EMA {e200:.2f})")
    reasons.append(
        f"reversal {'CONFIRMED' if reversal_confirmed else 'not confirmed'} "
        f"(px>9 {price>e9}, px>20 {price>e20}, 9>20 {e9>e20})"
    )
    reasons.append(f"200-slope {'up/flat' if slope_ok else 'DOWN (value-trap risk)'}")
    reasons.append(f"volume/OBV {'confirms' if volume_ok else 'weak (fakeout risk)'}")
    reasons.append(f"RSI {float(row['RSI']):.0f} {'ok' if rsi_ok else 'OVERBOUGHT'}")
    reasons.append(f"market {'risk-on (SPY>200)' if market_ok else 'RISK-OFF (SPY<200)'}")

    return Signal(
        regime=regime, reversal_confirmed=reversal_confirmed, slope_ok=slope_ok,
        volume_ok=volume_ok, rsi_ok=rsi_ok, market_ok=market_ok,
        quality=quality, entry_ok=entry_ok, reasons=reasons,
    )


def atr_stop(entry: float, atr_val: float, mult: float = 2.0) -> float:
    return round(entry - mult * atr_val, 2)


def compute_stop(entry: float, atr_val: float, ema20: float,
                 recent_low: float | None = None, atr_mult: float = 2.0) -> float:
    """The ONE stop rule, shared by the live scan and the backtest so the edge that
    gets measured is the edge the tool tells you to risk. Tighter of an ATR stop and a
    structural stop (just under the 20 EMA / nearest swing low), floored just below entry.
    """
    stop_atr = entry - atr_mult * atr_val
    struct = ema20 if recent_low is None else min(ema20, recent_low)
    stop_struct = struct * 0.99
    return round(max(stop_atr, min(stop_struct, entry * 0.999)), 2)
