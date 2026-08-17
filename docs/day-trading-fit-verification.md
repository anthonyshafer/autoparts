# Verification — Purpose Fit & Predictive Accuracy

**Date:** 2026-08-16 · **Method:** direct backtest measurement + code review.
**Confirmed purpose:** **SWING / POSITION trading** (not day trading).

## Verdict: correctly built for its purpose ✅

> **UCEF cross-check:** scored **12% fit "as a day-trading tool"** — and noted the tool's own
> code/docs *already self-describe as weekly*, so the day-trading label was an external
> mislabel, not a flaw. Read the inverse: as the **swing/position tool it actually is**, it's
> correctly architected (clean shared strategy module, no-lookahead backtest, ATR stops).

The engine is built on the **200-week EMA** (≈ 4 years) with a target at that line — a
multi-week-to-multi-month horizon. That is exactly a **swing/position trend-following**
design, which is the tool's confirmed intended use. No mismatch: it does what it's for.

## Predictive character (stated straight)

This is a **trend-following** system, and its measured behavior matches that by design:

| Timeframe test | Trades | Avg R | Win-rate |
|---|---|---|---|
| Daily, short holds (5–10 bars) | 53–71 | +0.15 to +0.35 R | low (0–33%) |
| Daily, full swing hold | 44 | ~+0.0 R (R≥1.5 gated) | mixed |

**Read:** trend-followers run a **low win-rate with large winners** — a few trades that reach
the far target pay for many small losers. That is the correct, expected fingerprint for a
swing system aiming at the 200-week mean. It is **not** a high-hit-rate signal and shouldn't
be used like one. The edge on its native horizon is thin/in-sample — the value is
**disciplined entries, invalidation, and sizing**, not directional precision.

## What it is / isn't

- ✅ **Is:** a swing/position screen — find discounted names (below the 200-week EMA) that
  have reclaimed the 9/20, then hold toward the 200-EMA target with an ATR stop.
- ❌ **Isn't:** a day-trading system. No intraday timeframes, no session-close exit, no
  VWAP/opening-range logic — and it **shouldn't** have them, because that's not the goal.
  (If day trading were ever wanted, it would be a separate intraday build, out of scope here.)

## Bottom line

Accurate label = **swing/position, trend-following — correctly purposed.** Predictive
accuracy is honest: low win-rate, winner-driven, thin in-sample edge. Use it as a
**disciplined swing screen**, size to the invalidation, and log calls to measure the real
edge over time.
