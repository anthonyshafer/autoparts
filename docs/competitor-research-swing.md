# Competitor Research — Swing/Position Trading Tools vs Swing R

**As-of:** 2026-08-17. SaaS pricing changes often — treat all $ figures as point-in-time
snapshots. Claims sourced from official pricing pages where fetched; unverifiable items are
flagged CANNOT VERIFY rather than guessed.

Our app: free, local, native-Rust desktop tool running ONE strategy (9/20/200-week EMA
reversal) with an automated verdict (entry/target/stop/R), a walk-forward backtest, a
Monte-Carlo scenario simulator, and a manual calibration journal. No screener, charts,
alerts, risk-% sizing, or earnings-awareness.

## Feature matrix

| | Screener | Charting | Backtest (true engine) | Alerts | Risk-% sizing (native) | Earnings-aware | Journal / calibration | Single-strategy verdict | Price |
|---|---|---|---|---|---|---|---|---|---|
| **TrendSpider** (nearest) | Yes | Yes (multi-TF) | Yes | Yes (dynamic) | Calculator only (CANNOT VERIFY in-workflow) | Yes | No | No | $54–$399+/mo, no free platform tier |
| **TC2000** | Yes (EasyScan) | Yes | **No** (scan-validation only) | Yes (≤1,000) | No | CANNOT VERIFY | No | No | $24.99–$99.99/mo |
| **StockCharts** | Yes (Scan Workbench) | Yes | CANNOT VERIFY | Yes (≤500) | No | CANNOT VERIFY | No | No | $19.95–$49.95/mo |
| **TradingView** | Yes | Yes | Yes (Pine Strategy Tester) | Yes (≤1,000) | Community scripts only | CANNOT VERIFY native | **No** (confirmed absent) | No | Free tier + $12.95–$199.95/mo |
| **Finviz Elite** | Yes (70+ filters) | Basic | Screener-criteria only | Yes | No | **Yes** (filter + calendar) | No | No | $39.50/mo (~$24.96 annual) |
| **Trade Ideas** | Yes (Holly AI) | Yes | Yes (day-trade) | Yes | CANNOT VERIFY | CANNOT VERIFY | No | No (paid curated picks) | CANNOT VERIFY (~$89–$254/mo) |
| **Swing R (ours)** | **No** | **No** | Yes (walk-forward) | **No** | **No** | **No** | **Yes** (manual) | **Yes** | **Free, local** |

## Table stakes we lack (evidence: present in all 6)
1. **Market-wide screener** — all 6. The single most consistent feature in the category.
2. **Price charting** — all 6 (even screener-first Finviz ships basic charts).
3. **Alerts** — all 6 (from 3 on TradingView free to 1,000+ on top tiers).

Backtesting is present in 4/6 (we already have it → a strength, not a gap).

## Common but NOT urgent (confirmed in only 1–2 of 6)
- **Risk-% position sizing** — TrendSpider ships a *standalone* calculator (in-workflow
  integration unconfirmed); no other competitor confirmed native. Worth having, not a parity
  emergency.
- **Earnings-date awareness** — confirmed native only in Finviz (filter + calendar) and
  TrendSpider. Common, not universal.

## Where we're genuinely differentiated (nobody matches)
- **Calibration-tracked journal** — no researched competitor has a native journal at all;
  TradingView explicitly lacks one (confirmed May 2026) and users bolt on TraderSync etc.
  A journal built to score *predicted vs realized* win-rate is unclaimed.
- **Single fixed strategy with an automated verdict** — every competitor is a build-your-own
  toolkit (custom scans/scripts/bots). An opinionated one-strategy verdict engine is a
  different product category, not just a feature.
- **Free + local + full-featured + no subscription** — TradingView/Finviz have free tiers,
  but they're cloud, capped, upsell teasers. No competitor offers a fully-featured, no-account,
  no-subscription *local desktop* tool.

## The open gap to OWN
**Calibration-first single-strategy automation.** Every competitor sells infinite
configurability; none close the loop — pick one strategy, force a probability at the time of
the call, and score that probability against real outcomes. TrendSpider's Strategy Bots come
closest to "automate a backtested strategy forward" but show no evidence of calibration
scoring. That's our moat — lean into it (auto-resolve journal + a calibration report), not
just chase table stakes.

## CANNOT VERIFY (do not rely on without a direct check)
- Trade Ideas current tier names/prices (sources materially conflicted).
- TrendSpider position-size calculator wired into the in-app scan/backtest workflow.
- StockCharts backtesting (not found on official pages; absence ≠ confirmed absent).
- TC2000 / StockCharts / TradingView earnings-awareness & native sizing at the first-class level.

**Sources:** [TrendSpider pricing](https://trendspider.com/pricing/) · [TC2000 pricing](https://www.tc2000.com/Pricing) · [StockCharts pricing](https://stockcharts.com/pricing/) · [TradingView pricing](https://www.tradingview.com/pricing/) · [Finviz screener](https://finviz.com/screener?v=340) · [Finviz earnings calendar](https://finviz.com/calendar/earnings) · [Liberated Stock Trader — TC2000 review](https://www.liberatedstocktrader.com/tc2000-review/) · [Take Profit Trader — TradingView review 2026](https://takeprofitapp.com/en/learn/tradingview-review-2026) · [StockBrokers.com — Finviz review](https://www.stockbrokers.com/review/tools/finviz)
