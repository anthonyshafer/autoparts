# Swing-Feature Roadmap — Scope for Features 1–5

Scoping for the top-5 missing swing-trader features, mapped onto the current native-Rust +
Tauri architecture (fetch → engine → Tauri command → web UI). Each is either a **new
feature** or a **thin add** on the proven engine. Competitor context is in the companion
research report (see `docs/competitor-research-swing.md`).

**Current architecture recap (what we build on):**
- `desktop/src-tauri/src/fetch.rs` — async Yahoo fetch (tokio), auto-adjust, `Ohlcv` w/ timestamps.
- `desktop/src-tauri/src/engine.rs` — indicators, `scan_frame`, `backtest` (parity-proven).
- `desktop/src-tauri/src/main.rs` — Tauri commands (`scan`, `backtest`, `save_text`, …).
- `desktop/dist/index.html` — the whole UI (tabs, themes, Bloomberg skin).

Effort key: **S** ≈ half day · **M** ≈ 1 day · **L** ≈ 1–2 days.

---

## 1. Market-wide screener  ·  Effort: **L**  ·  Highest strategic value

**What:** run the scan across a *universe* of tickers and show every BUY/WATCH setup at once,
ranked — turning a one-ticker calculator into an actual swing *system*.

**How it maps:**
- **Rust — fan-out fetch+scan.** The async `fetch_async` already exists; add
  `screen(tickers: &[String], timeframe) -> Vec<ScanRow>` that runs fetches **concurrently
  with a capped semaphore** (~8–12 in flight to respect Yahoo rate limits), then `scan_frame`
  each, collecting a compact row: `{ticker, verdict, regime, price, entry, take_profit,
  stop_loss, r_multiple, setup_quality}`. Skip/report tickers with <60 bars or fetch errors.
- **Universe source.** Yahoo has no "all tickers" endpoint, so bundle curated lists as static
  JSON in the app: **S&P 500, Nasdaq-100, Dow 30**, plus the user's own watchlist (their open
  tabs / a saved list). Ship the lists in `desktop/dist/universes/`.
- **Tauri command** `screen(universe|tickers, timeframe)` → JSON array, with a **progress
  event** (emit `screen-progress {done, total}` so the UI shows a bar). **Cache** the last run
  with a timestamp; a full S&P-500 sweep is ~500 fetches → ~15–40s at concurrency 8–12.
- **UI — a "Screener" tab** (reuse the Saved-table pattern): sortable columns (verdict, ticker,
  R, regime, quality), **filters** (DISCOUNT only, BUY/WATCH only, min R), click a row → opens
  a full scan tab. A "Run" button + universe picker + progress bar.

**Risks:** Yahoo throttling on bulk (mitigate: concurrency cap + small jitter + result cache +
resumable progress). Curated lists go stale (refresh via a maintenance script, quarterly).

**Why first:** every competitor's core feature is a market screener; it's the single biggest
gap and the engine already does the hard part.

---

## 2. Price chart (candles + EMAs)  ·  Effort: **M**

**What:** draw candlesticks + the 9/20/200 EMA lines + a horizontal 200-EMA target line and
entry/stop/support/rejection markers for the scanned ticker. Swing traders need to *see* it.

**How it maps:**
- **Rust:** we already fetch OHLCV+ts and compute the EMAs. Add a `chart(ticker, timeframe)`
  command (or extend the scan payload) returning `{ts[], open[], high[], low[], close[],
  ema9[], ema20[], ema200[]}` plus the marker levels from the scan.
- **UI:** render with **lightweight-charts** (TradingView's free MIT lib, ~45KB) — but Tauri's
  CSP blocks CDNs, so **vendor it locally** (inline into `dist/`). Draw candles + 3 EMA line
  series + a priceline at the 200-EMA target and horizontal markers for entry/stop.
- Optional: weekly + daily side-by-side (two chart instances).

**Risks:** CSP/vendoring (must self-host the lib, no CDN). Perf is fine (lightweight-charts
handles thousands of bars). No engine changes → no parity risk.

---

## 3. Alerts / auto-watch  ·  Effort: **M**

**What:** monitor watchlist tickers in the background; fire a native notification when a WATCH
flips to BUY (price reclaims the 20-EMA trigger) or regime changes.

**How it maps:**
- **Rust:** a **tokio interval task** that re-scans the watchlist every N minutes (weekly
  strategy → hourly/daily is ample, not intraday), diffs each ticker's verdict against the
  last-known, and on a meaningful change emits a Tauri event.
- **Notifications:** add **`tauri-plugin-notification`** for native OS alerts; persist the
  watchlist + last verdicts (localStorage or a small file).
- **UI:** a "watch" toggle per ticker, a bell icon, and an alert log.

**Risks:** desktop app must be *running* to alert (it's not a server) — state that limitation.
Yahoo rate limits (watchlists are small, low risk).

---

## 4. Risk-based position sizing  ·  Effort: **S**  ·  Best ROI-per-effort

**What:** size by **risk % of account**, not just a $-amount. `risk$ = account × risk%`;
`shares = floor(risk$ / (entry − stop))`; `position$ = shares × entry`. Show the R already
baked in (target vs stop).

**How it maps:**
- Pure math on values the scan already returns (`entry`, `stop_loss`). Do it in the **frontend**
  (it has entry/stop) — add "Account $" and "Risk %" inputs; show risk-based shares + position
  $ alongside the current fixed-$ sizing. No Rust change needed.

**Risks:** none. Smallest change, immediately useful — do it alongside #2.

---

## 5. Earnings-date awareness  ·  Effort: **M**  ·  Data-source risk

**What:** flag if the next earnings date falls within the expected hold window — "⚠ earnings in
6 days." Holding a swing through earnings is a classic account-killer.

**How it maps:**
- **Data is the hard part.** `yahoo_finance_api` (Rust) doesn't expose earnings. Options:
  1. **Yahoo quoteSummary** `calendarEvents.earnings` via a small `reqwest` call — free but now
     needs crumb+cookie auth and is flaky.
  2. **Finnhub free tier** (`/calendar/earnings`) or **Financial Modeling Prep** — reliable,
     but needs a **free API key** (adds a config/env step and a `.env`-style setting).
- **Rust:** fetch the next earnings date, add `earnings_date` + `days_to_earnings` to the scan
  JSON.
- **UI:** a badge in the scan ("⚠ earnings in N days"), and flag if it lands before the target
  horizon.

**Risks:** the data source. **Recommendation:** use Finnhub free tier with an optional API key
(graceful "earnings unavailable" if unset) rather than fighting Yahoo's auth. FLAG that this is
the one feature needing an external key.

---

## Suggested build order

1. **#4 risk sizing** (S) + **#2 chart** (M) — quick, high-visibility wins, no engine risk.
2. **#1 screener** (L) — the strategic centerpiece.
3. **#3 alerts** (M) — natural once the screener/watchlist exists.
4. **#5 earnings** (M) — last, since it needs an external data source + a key.

## Research-informed reprioritization (see `docs/competitor-research-swing.md`)

The competitor pass (TrendSpider, TC2000, StockCharts, TradingView, Finviz, Trade Ideas)
sharpens the priority:

- **True table stakes we lack — present in ALL 6 competitors:** #1 screener, #2 charting,
  #3 alerts. These are the real gaps; build them first.
- **Common but NOT urgent — confirmed native in only 1–2 of 6:** #4 risk-% sizing (only
  TrendSpider, and even that unconfirmed in-workflow) and #5 earnings-awareness (only Finviz
  + TrendSpider). Keep them, but they're not parity emergencies. #4 is still worth doing
  first purely because it's an S-effort win.
- **Our moat — nobody has it:** the **calibration-tracked journal** + **single-strategy
  automated verdict**. No competitor ships a native journal, let alone one scoring predicted
  vs realized win-rate. **Add a 6th feature to protect/extend the moat:**

  **#6 — Auto-resolving journal + calibration report (Effort: M).** Auto-mark each logged call
  win/loss when price hits target/stop (re-fetch + check), then a report: *"your 65%-confidence
  calls resolved X% of the time"* (reliability curve). This is the differentiator the whole
  CLAUDE.md thesis is built on, and it's unclaimed territory across the entire competitive set.

**Revised build order:** #4 (S) → #2 (M) → **#1 screener (L)** → #3 (M) → **#6 calibration (M)**
→ #5 (M, needs an external earnings key). Do the table stakes to be competitive; do #6 to be
*different*.
