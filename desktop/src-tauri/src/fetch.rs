// Yahoo data fetch — replaces the Python yfinance layer so the app needs no Python.
// Replicates yfinance auto_adjust=True: scale O/H/L by adjclose/close and set close=adjclose;
// volume unchanged. Same source as the Python tool, so downstream indicators/scan match.

use crate::engine::Ohlcv;
use yahoo_finance_api as yahoo;

/// scan periods (ema_analyzer._fetch): weekly 10y / daily 3y.
fn scan_interval_range(timeframe: &str) -> (&'static str, &'static str) {
    match timeframe {
        "daily" => ("1d", "3y"),
        _ => ("1wk", "10y"),
    }
}

/// backtest periods (backtest._fetch): weekly 15y / daily 5y.
fn bt_interval_range(timeframe: &str) -> (&'static str, &'static str) {
    match timeframe {
        "daily" => ("1d", "5y"),
        _ => ("1wk", "15y"),
    }
}

async fn fetch_async(ticker: &str, interval: &str, range: &str) -> Result<Ohlcv, String> {
    let provider = yahoo::YahooConnector::new().map_err(|e| format!("connector: {e:?}"))?;
    let resp = provider
        .get_quote_range(ticker, interval, range)
        .await
        .map_err(|e| format!("fetch {ticker}: {e:?}"))?;
    let quotes = resp.quotes().map_err(|e| format!("quotes: {e:?}"))?;
    let mut d = Ohlcv::default();
    for q in quotes {
        // Match Python _fetch's .dropna(): skip any bar with a non-finite price/volume.
        if ![q.open, q.high, q.low, q.close, q.adjclose].iter().all(|v| v.is_finite())
            || !(q.volume as f64).is_finite()
        {
            continue;
        }
        let ratio = if q.close != 0.0 { q.adjclose / q.close } else { 1.0 };
        d.open.push(q.open * ratio);
        d.high.push(q.high * ratio);
        d.low.push(q.low * ratio);
        d.close.push(q.adjclose);
        d.volume.push(q.volume as f64);
        d.ts.push(q.timestamp as i64);
    }
    Ok(d)
}

fn block_fetch(ticker: &str, interval: &str, range: &str) -> Result<Ohlcv, String> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    rt.block_on(fetch_async(ticker, interval, range))
}

/// Fetch for the live scan (scan periods).
pub fn fetch(ticker: &str, timeframe: &str) -> Result<Ohlcv, String> {
    let (interval, range) = scan_interval_range(timeframe);
    block_fetch(ticker, interval, range)
}

/// Fetch for the backtest (longer periods).
pub fn fetch_bt(ticker: &str, timeframe: &str) -> Result<Ohlcv, String> {
    let (interval, range) = bt_interval_range(timeframe);
    block_fetch(ticker, interval, range)
}

/// market_ok = SPY's last close >= its 200-EMA on the same timeframe (matches _market_ok).
/// Unknown/fetch failure -> true (permissive), same as the Python fallback.
pub fn market_ok(timeframe: &str) -> bool {
    match fetch("SPY", timeframe) {
        Ok(d) if !d.close.is_empty() => {
            let ema200 = crate::engine::ema(&d.close, 200);
            let i = d.close.len() - 1;
            d.close[i] >= ema200[i]
        }
        _ => true,
    }
}

// ---- Async-native variants (for callers ALREADY on a tokio runtime, e.g. FINDR's async
// command). block_fetch()'s `Runtime::new().block_on()` PANICS ("Cannot start a runtime from
// within a runtime") when called on a runtime worker thread, so async callers MUST use these. ----

/// Async scan fetch — same data as `fetch`, safe to call from an async context.
pub async fn fetch_scan_async(ticker: &str, timeframe: &str) -> Result<Ohlcv, String> {
    let (interval, range) = scan_interval_range(timeframe);
    fetch_async(ticker, interval, range).await
}

/// Async `market_ok` — same rule as the sync version, safe to call from an async context.
pub async fn market_ok_async(timeframe: &str) -> bool {
    match fetch_scan_async("SPY", timeframe).await {
        Ok(d) if !d.close.is_empty() => {
            let ema200 = crate::engine::ema(&d.close, 200);
            let i = d.close.len() - 1;
            d.close[i] >= ema200[i]
        }
        _ => true,
    }
}
