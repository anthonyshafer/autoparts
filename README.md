# Stocks — EMA Reversal System

A command-line tool that formalizes the **9 / 20 / 200 EMA reversal** strategy:
buy a stock while it's **discounted** (below its 200-week EMA) but only *after* price
**reclaims the 9 & 20 EMA** (the reversal is "locked in"), targeting the 200 EMA as the
take-profit / expected rejection.

> **Read this first.** This is *decision support* built from indicator rules — a
> disciplined checklist and level calculator. It is **not** a prediction oracle, **not**
> 95% accurate, and **not** licensed financial advice. The bundled backtest (below) shows
> the raw rules are roughly **break-even** historically, positive on some tickers and
> negative on others. Its value is enforcing discipline and computing your levels, not
> printing money. Log every real call in `journal/` so your *actual* accuracy is measured.

---

## What it does

Give it a ticker; it returns:

- **Verdict** — BUY / WATCH / WAIT / AVOID (plus risk flags)
- **Entry, Take-profit (200 EMA), Stop-loss (ATR-based)**
- **Rejection zones** and **support** levels
- **Upside %, R-multiple** (reward ÷ risk) and **position sizing** for your capital
- **Filter checklist** (5 checks) so you see *why* it's a go or no-go

---

## Install

### Windows app — single `.exe`, nothing to install (easiest)

Get a standalone `StockScanner.exe` (bundles Python + everything — no venv, no PATH setup):

- **Auto-built for you:** the GitHub Actions workflow (`.github/workflows/build-windows.yml`)
  compiles the `.exe` on GitHub's Windows machines. Go to the repo's **Actions** tab →
  latest "Build Windows App" run → download the **StockScanner-windows** artifact. Or tag
  a release (`git tag v1.0 && git push --tags`) to get a clean Releases download page.
- **Build it yourself once:** on any Windows PC with Python 3.10+, double-click
  **`build_windows.bat`** → produces `dist\StockScanner.exe`. Copy that single file to any
  Windows PC and double-click — the end user needs no Python at all.

The app is a simple window: type a ticker, pick weekly/daily, click **Scan** or **Backtest**.

### Windows (Python + command line)

1. Install **Python 3.10+** from <https://www.python.org/downloads/> — during install,
   tick **"Add python.exe to PATH."**
2. Double-click **`setup_windows.bat`** (one time — builds a local `.venv` and installs deps).
3. Double-click **`scan.bat`**, type a ticker (e.g. `BSX`), and read the result.

### Windows / Mac / Linux (command line)

```bash
# option A — pip + venv (works everywhere)
python -m venv .venv
# Windows:  .venv\Scripts\activate
# Mac/Lin:  source .venv/bin/activate
pip install -r requirements.txt
python tools/stocks.py scan BSX

# option B — uv (zero setup; installs deps automatically per run)
#   install uv once: https://docs.astral.sh/uv/getting-started/installation/
uv run tools/stocks.py scan BSX
```

On Windows without the venv activated, call the interpreter directly:
`.venv\Scripts\python.exe tools\stocks.py scan BSX`

---

## Command line

```
python tools/stocks.py scan     <TICKER> [--amount 50000] [--timeframe weekly|daily] [--json]
python tools/stocks.py backtest <TICKER...> [--timeframe weekly|daily] [--max-hold 52] [--json]
python tools/stocks.py watch    <TICKER...> [--timeframe weekly|daily]
```

| Command | What it does |
|---------|--------------|
| `scan` | Full read on one ticker: verdict, levels, filters, sizing. |
| `backtest` | Runs the rules over 10–15y of history and reports win-rate, avg R, expectancy. |
| `watch` | One-line verdict per ticker for a whole watchlist. |

Flags: `--amount` = capital to size against (default $50,000). `--timeframe weekly`
(200 EMA ≈ 200 weeks ≈ 4 years, the default) or `daily` (200 EMA ≈ 200 days).
`--json` for machine-readable output.

---

## Full worked example — `scan BSX`

```
$ python tools/stocks.py scan BSX --amount 50000

=== BSX — WATCH — wait for reclaim of 9/20 ===
[weekly]  price $51.83   regime DISCOUNT   setup quality 3/5

  EMA9 $48.34   EMA20 $53.67   EMA200 $71.41 <- target line
  RSI 40.3   ATR $4.24

  Filters:  slope FAIL   volume PASS   rsi PASS   market PASS   reversal FAIL

  Entry        $51.83
  Take-profit  $71.41  (+37.8%)  <- expected rejection at 200 EMA
  Stop-loss    $49.19  (-5.1%)  (ATR-based)
  R-multiple   7.42

  Rejection zones: $54.75, $55.38, $71.41
  Support:         $49.69, $48.35, $48.34, $44.35

  Sizing @ $50,000.00: 964 sh = $49,964.12  ->  TP +$18,875.12  |  stop -$2,544.96

  Why:
    - regime=DISCOUNT (price 51.83 vs 200EMA 71.41)
    - reversal not confirmed (px>9 True, px>20 False, 9>20 False)
    - 200-slope DOWN (value-trap risk)
    - volume/OBV confirms
    - RSI 40 ok
    - market risk-on (SPY>200)
```

**How to read this:** BSX is below its 200 EMA (discount ✅) and has reclaimed the 9 EMA,
**but not the 20** — so the reversal isn't confirmed and the verdict is **WATCH, not BUY**.
The trigger to re-check is a close back above the 20 EMA (**$53.67**). The 200-slope is
falling, which is flagged as *value-trap risk*. When/if it flips to BUY, the plan is
entry ~$51.83, target $71.41 (+37.8%), stop ~$49.19, for a 7.4R setup.

### Watchlist view

```
$ python tools/stocks.py watch BSX ABBV PFE JNJ KO INTC

TICKER   VERDICT                                      PRICE        TP      R  QUALITY
------------------------------------------------------------------------------------------
BSX      WATCH — wait for reclaim of 9/20             51.83     71.41   7.42  3/5
ABBV     AVOID — premium, price above the 200 line   249.46    178.10  -2.49  5/5
PFE      AVOID — premium, price above the 200 line    26.79     26.00  -0.45  4/5
...
```

---

## The strategy, precisely

| Piece | Rule |
|-------|------|
| **Regime** | `price < 200 EMA` → DISCOUNT · `≈ 200 EMA` → FAIR · `> 200 EMA` → PREMIUM |
| **Reversal (gate)** | `price > EMA9` **and** `price > EMA20` **and** `EMA9 > EMA20` |
| **Not overbought (gate)** | `RSI(14) < 70` |
| **Take-profit** | the 200 EMA (the "boss" / expected rejection) |
| **Stop-loss** | `entry − 2 × ATR(14)` (adapts to the stock's volatility) |
| **Risk flags** (not blockers) | 200-EMA slope down = value-trap risk · SPY below its 200 = risk-off market · weak volume = fakeout risk |

A **BUY** requires the three gates plus R ≥ 1.5. The risk flags lower "setup quality"
(x/5) and warn you, but don't veto the trade — because demanding a *rising* 200 EMA
contradicts the whole idea of buying a discount (the 200 lags and is usually flat or
falling at a genuine discount reclaim).

---

## Does it actually work? (bundled backtest — be honest)

Run it yourself: `python tools/stocks.py backtest BSX ABBV PFE JNJ NKE KO DIS INTC --timeframe daily`

Representative results across 8 large-caps (daily, ~5y), counting only actionable BUYs
(R ≥ 1.5) with the **same stop the live tool recommends**:

| Metric | Value | Reading |
|--------|-------|---------|
| Total trades | 44 | usable but modest |
| Portfolio avg | **+0.01 R / trade** | **essentially zero edge** |
| Per-ticker | mixed | a couple positive, several negative; most samples tiny |
| Weekly timeframe | 0–5 trades/ticker | too rare to be a system on its own |

**Takeaways (read these):**
- After aligning the backtest to the tool's *actual* stop and only counting the trades
  you'd really take, expectancy is **~0 R** — this rule set is **not a standalone edge.**
  Its value is discipline (levels, invalidation, sizing) and screening, not alpha.
- **Tuned in-sample, unvalidated out-of-sample.** The discount band and entry gates were
  chosen on these same tickers. A trustworthy edge number requires picking parameters on
  early years and measuring on held-out later years — **not yet done.** Treat every
  positive per-ticker figure as *in-sample* until that split exists.
- It's **ticker-dependent**: backtest *before* trusting it on a new name.
- Weekly is too rare to trade mechanically; it's a **screen**. Daily has more signals.
- Numbers here are **hypothetical** (past behavior of a rule set) and assume stop-first on
  ambiguous bars, exact fills, no slippage, no commissions. Real "accuracy" only exists
  once *you* have logged, resolved trades — that's what `journal/` is for.

---

## Data note

Prices come from Yahoo Finance (`yfinance`), which is free but occasionally has gaps,
splits, or stale values. If a level looks wrong versus your charting platform, trust your
chart and re-check the symbol. The tool is only as good as the data feeding it.

## Files

```
tools/strategy.py       shared indicators + signal logic (single source of truth)
tools/ema_analyzer.py   the scan report
tools/backtest.py       walk-forward backtester
tools/stocks.py         unified CLI (scan / backtest / watch)
requirements.txt        pip dependencies
setup_windows.bat       one-time Windows setup
scan.bat                double-click Windows launcher
journal/                YOUR logged calls — the real accuracy scoreboard
```

Not licensed advice. You decide and execute; the tool computes and warns.
