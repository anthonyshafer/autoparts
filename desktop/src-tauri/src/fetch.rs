// Yahoo data fetch — replaces the Python yfinance layer so the app needs no Python.
// Replicates yfinance auto_adjust=True: scale O/H/L by adjclose/close and set close=adjclose;
// volume unchanged. Same source as the Python tool, so downstream indicators/scan match.

use crate::engine::Ohlcv;
use yahoo_finance_api as yahoo;

fn interval_range(timeframe: &str) -> (&'static str, &'static str) {
    match timeframe {
        "daily" => ("1d", "3y"),
        _ => ("1wk", "10y"),
    }
}

async fn fetch_async(ticker: &str, timeframe: &str) -> Result<Ohlcv, String> {
    let (interval, range) = interval_range(timeframe);
    let provider = yahoo::YahooConnector::new().map_err(|e| format!("connector: {e:?}"))?;
    let resp = provider
        .get_quote_range(ticker, interval, range)
        .await
        .map_err(|e| format!("fetch {ticker}: {e:?}"))?;
    let quotes = resp.quotes().map_err(|e| format!("quotes: {e:?}"))?;
    let mut d = Ohlcv { open: vec![], high: vec![], low: vec![], close: vec![], volume: vec![] };
    for q in quotes {
        let ratio = if q.close != 0.0 { q.adjclose / q.close } else { 1.0 };
        d.open.push(q.open * ratio);
        d.high.push(q.high * ratio);
        d.low.push(q.low * ratio);
        d.close.push(q.adjclose);
        d.volume.push(q.volume as f64);
    }
    Ok(d)
}

/// Blocking fetch (spins a tokio runtime) so the sync scan/backtest paths can call it.
pub fn fetch(ticker: &str, timeframe: &str) -> Result<Ohlcv, String> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    rt.block_on(fetch_async(ticker, timeframe))
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
