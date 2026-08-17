---
name: ema-scan
description: Run the 9/20/200 EMA reversal analysis on a stock ticker. Use when the user gives a ticker and wants a buy/wait/avoid read with entry, take-profit, rejection zones, support, stop-loss, upside %, R-multiple, and position sizing. Triggers on "scan TICKER", "run EMA on TICKER", "is TICKER a buy", "analyze TICKER".
---

# EMA Scan

Runs `tools/ema_analyzer.py` — formalizes the "little gladiators -> boss" strategy
(price reclaims 9/20 EMA while below the 200 EMA, hunting the 200 as target/rejection).

## Run it

```
uv run tools/ema_analyzer.py <TICKER> --amount <CAPITAL> --timeframe weekly
```

Add `--json` for machine-readable output. `--timeframe daily` for a shorter-term read.

## After running

1. Lead with the verdict line and the key numbers (entry / TP / stop / R).
2. If the read disagrees with the operator's mental model, say so explicitly.
3. Offer to log the call in `journal/` (that's the accuracy scoreboard).

## What the verdict means

- **BUY** — discounted (below 200 EMA), reversal confirmed (price > 9 & 20, 9>20), R >= 1.5.
- **BUY (thin R)** — same but reward/risk under 1.5; size down or wait for a better entry.
- **WATCH** — discounted but 9/20 not yet reclaimed; set an alert, no entry yet.
- **WAIT** — near fair value; poor risk/reward.
- **AVOID** — premium; price already above the 200 line.

Decision support only — not a prediction, not licensed advice.
