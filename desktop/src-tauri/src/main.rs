// Swing R — Tauri (Rust) desktop app. Self-contained: the scan/backtest commands run
// the NATIVE Rust engine (engine.rs + fetch.rs), which is parity-proven against the Python
// tool. No Python at runtime — the app is a single standalone binary.

mod engine;
mod fetch;
mod findr;

use std::path::PathBuf;

// ---- Rust-native scan/backtest (no Python) — build the same JSON the frontend expects ----

fn ymd(ts: i64) -> String {
    use time::OffsetDateTime;
    match OffsetDateTime::from_unix_timestamp(ts) {
        Ok(dt) => format!("{:04}-{:02}-{:02}", dt.year(), dt.month() as u8, dt.day()),
        Err(_) => ts.to_string(),
    }
}

fn scan_json(ticker: &str, timeframe: &str) -> Result<String, String> {
    let d = fetch::fetch(ticker, timeframe)?;
    if d.len() < 60 {
        return Err(format!("Only {} {} candles for {}; need history for a 200 EMA.", d.len(), timeframe, ticker));
    }
    let mkt = fetch::market_ok(timeframe);
    let r = engine::scan_frame(&d, mkt, 0.005);
    let rmul = if r.r_multiple.is_finite() { serde_json::json!(r.r_multiple) } else { serde_json::Value::Null };
    let j = serde_json::json!({
        "ticker": ticker.to_uppercase(), "timeframe": timeframe,
        "verdict": r.verdict, "regime": r.regime, "price": r.price, "entry": r.entry,
        "ema9": r.ema9, "ema20": r.ema20, "ema200": r.ema200, "rsi": r.rsi, "atr": r.atr,
        "take_profit": r.take_profit, "stop_loss": r.stop_loss,
        "upside_pct": r.upside_pct, "downside_pct": r.downside_pct, "r_multiple": rmul,
        "rejection_zones": r.rejection_zones, "support": r.support,
        "setup_quality": r.setup_quality, "reversal_confirmed": r.reversal_confirmed,
        "slope_ok": r.slope_ok, "volume_ok": r.volume_ok, "rsi_ok": r.rsi_ok, "market_ok": r.market_ok,
        "reasons": r.reasons, "fundamentals": {}
    });
    Ok(j.to_string())
}

fn backtest_json(ticker: &str, timeframe: &str) -> Result<String, String> {
    let d = fetch::fetch_bt(ticker, timeframe)?;
    let r = engine::backtest(&d, 2.0, 52);
    let log: Vec<serde_json::Value> = r.log.iter().map(|t| {
        let ed = d.ts.get(t.entry_idx).map(|&x| ymd(x)).unwrap_or_else(|| t.entry_idx.to_string());
        let xd = d.ts.get(t.exit_idx).map(|&x| ymd(x)).unwrap_or_else(|| t.exit_idx.to_string());
        serde_json::json!({ "entry_date": ed, "exit_date": xd, "entry": t.entry, "exit": t.exit,
            "stop": t.stop, "target": t.target, "bars_held": t.bars_held, "outcome": t.outcome, "r": t.r })
    }).collect();
    let pf = if r.profit_factor.is_finite() { serde_json::json!(r.profit_factor) } else { serde_json::Value::Null };
    // Python stocks.py backtest --json returns a LIST; the frontend reads result[0].
    let obj = serde_json::json!({
        "ticker": ticker.to_uppercase(), "timeframe": timeframe, "bars": r.bars,
        "trades": r.trades, "wins": r.wins, "losses": r.losses, "timeouts": r.timeouts,
        "win_rate": r.win_rate, "avg_r": r.avg_r, "expectancy": r.avg_r, "profit_factor": pf,
        "total_r": r.total_r, "note": r.note, "trade_log": log
    });
    Ok(serde_json::Value::Array(vec![obj]).to_string())
}

// Chart series: last ~180 bars of OHLC + the 9/20/200 EMA lines + the scan marker levels.
fn chart_json(ticker: &str, timeframe: &str) -> Result<String, String> {
    let d = fetch::fetch(ticker, timeframe)?;
    if d.len() < 60 {
        return Err(format!("Only {} {} candles for {}.", d.len(), timeframe, ticker));
    }
    let ind = engine::compute_indicators(&d);
    let mkt = fetch::market_ok(timeframe);
    let r = engine::scan_frame(&d, mkt, 0.005);
    let n = d.len();
    let take = n.min(520); // return plenty of history; the UI viewport handles zoom/pan
    let s = n - take;
    let slice = |v: &[f64]| -> Vec<Option<f64>> {
        v[s..].iter().map(|&x| if x.is_finite() { Some(x) } else { None }).collect()
    };
    let j = serde_json::json!({
        "ticker": ticker.to_uppercase(), "timeframe": timeframe,
        "ts": &d.ts[s..], "open": &d.open[s..], "high": &d.high[s..],
        "low": &d.low[s..], "close": &d.close[s..], "volume": &d.volume[s..],
        "ema9": slice(&ind.ema9), "ema20": slice(&ind.ema20), "ema200": slice(&ind.ema200),
        "markers": { "take_profit": r.take_profit, "entry": r.entry, "stop_loss": r.stop_loss,
                     "rejection_zones": r.rejection_zones, "support": r.support }
    });
    Ok(j.to_string())
}

// ---- Fundamentals + News: read-only Yahoo Finance HTTP endpoints (no engine/fetch.rs
// involvement — independent of the OHLCV path). ----

const YAHOO_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
    (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

// Cached (cookie-jar Client, crumb) for the process lifetime — the crumb handshake is a
// 3-step round trip, so we do it once and reuse it across `fundamentals` calls rather than
// re-handshaking on every invocation.
static YAHOO_AUTH: std::sync::OnceLock<tokio::sync::Mutex<Option<(reqwest::Client, String)>>> =
    std::sync::OnceLock::new();

fn yahoo_auth_cell() -> &'static tokio::sync::Mutex<Option<(reqwest::Client, String)>> {
    YAHOO_AUTH.get_or_init(|| tokio::sync::Mutex::new(None))
}

// Step 1-3: prime the auth cookie, then read the crumb body. `fc.yahoo.com` may return a
// non-200 status; that is expected and does not indicate failure — it still sets the cookie.
async fn yahoo_handshake() -> Result<(reqwest::Client, String), String> {
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .user_agent(YAHOO_UA)
        .build()
        .map_err(|e| format!("client build failed: {e}"))?;

    let _ = client.get("https://fc.yahoo.com").send().await;

    let crumb_resp = client
        .get("https://query1.finance.yahoo.com/v1/test/getcrumb")
        .send()
        .await
        .map_err(|e| format!("getcrumb request failed: {e}"))?;
    let crumb = crumb_resp
        .text()
        .await
        .map_err(|e| format!("getcrumb body read failed: {e}"))?
        .trim()
        .to_string();
    if crumb.is_empty() {
        return Err("empty crumb from getcrumb".to_string());
    }
    Ok((client, crumb))
}

// Returns the cached (client, crumb), handshaking once on first call.
async fn yahoo_auth() -> Result<(reqwest::Client, String), String> {
    let mut guard = yahoo_auth_cell().lock().await;
    if let Some((client, crumb)) = guard.as_ref() {
        return Ok((client.clone(), crumb.clone()));
    }
    let (client, crumb) = yahoo_handshake().await?;
    *guard = Some((client.clone(), crumb.clone()));
    Ok((client, crumb))
}

async fn yahoo_invalidate_auth() {
    *yahoo_auth_cell().lock().await = None;
}

fn get_f64(v: &serde_json::Value, key: &str) -> Option<f64> {
    v.get(key).and_then(|x| x.as_f64())
}

// Step 4: v7 quote call. On an auth failure (crumb rejected), the error string is prefixed
// with "Unauthorized" so the caller can distinguish "retry the handshake" from a hard error.
async fn yahoo_fetch_quote(
    client: &reqwest::Client,
    crumb: &str,
    ticker: &str,
) -> Result<serde_json::Value, String> {
    let mut url = reqwest::Url::parse("https://query1.finance.yahoo.com/v7/finance/quote")
        .map_err(|e| format!("url parse failed: {e}"))?;
    url.query_pairs_mut()
        .append_pair("symbols", ticker)
        .append_pair("crumb", crumb);

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("quote request failed: {e}"))?;
    let status = resp.status();
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("quote body parse failed: {e}"))?;

    if let Some(err) = body.pointer("/quoteResponse/error") {
        if !err.is_null() {
            return Err(format!("Unauthorized: {err}"));
        }
    }
    if !status.is_success() {
        return Err(format!("Unauthorized: HTTP {status}"));
    }
    body.pointer("/quoteResponse/result/0")
        .cloned()
        .ok_or_else(|| "no quote result for ticker".to_string())
}

fn is_auth_error(e: &str) -> bool {
    e.starts_with("Unauthorized")
}

// Grounded, factual, advice-free one/two-sentence summary computed from the parsed fields.
fn fundamentals_read(
    price: Option<f64>,
    trailing_pe: Option<f64>,
    forward_pe: Option<f64>,
    eps_ttm: Option<f64>,
    low: Option<f64>,
    high: Option<f64>,
) -> String {
    let mut parts: Vec<String> = Vec::new();

    let unprofitable = match trailing_pe {
        Some(pe) => pe <= 0.0,
        None => matches!(eps_ttm, Some(e) if e <= 0.0),
    };

    if unprofitable {
        parts.push("Unprofitable on a trailing basis (negative/no earnings).".to_string());
    } else if let (Some(f), Some(t)) = (forward_pe, trailing_pe) {
        if f < t {
            parts.push(format!(
                "Forward P/E {f:.1} below trailing {t:.1} \u{2014} market expects earnings to grow."
            ));
        } else {
            parts.push(format!(
                "Forward P/E {f:.1} at/above trailing {t:.1} \u{2014} no earnings growth priced in."
            ));
        }
    }

    if let (Some(p), Some(l), Some(h)) = (price, low, high) {
        if h > l {
            let pct = (p - l) / (h - l);
            let pos = if pct < 0.33 {
                "lower third"
            } else if pct < 0.66 {
                "middle"
            } else {
                "upper third"
            };
            parts.push(format!("Trading in the {pos} of its 52-week range."));
        }
    }

    parts.join(" ")
}

async fn fundamentals_json(ticker: &str) -> Result<String, String> {
    let (client, crumb) = yahoo_auth().await?;
    let result = match yahoo_fetch_quote(&client, &crumb, ticker).await {
        Ok(v) => v,
        Err(e) if is_auth_error(&e) => {
            yahoo_invalidate_auth().await;
            let (client2, crumb2) = yahoo_auth().await?;
            yahoo_fetch_quote(&client2, &crumb2, ticker).await?
        }
        Err(e) => return Err(e),
    };

    let price = get_f64(&result, "regularMarketPrice");
    let trailing_pe = get_f64(&result, "trailingPE");
    let forward_pe = get_f64(&result, "forwardPE");
    let market_cap = get_f64(&result, "marketCap");
    let price_to_book = get_f64(&result, "priceToBook");
    let eps_ttm = get_f64(&result, "epsTrailingTwelveMonths");
    let eps_forward = get_f64(&result, "epsForward");
    let low = get_f64(&result, "fiftyTwoWeekLow");
    let high = get_f64(&result, "fiftyTwoWeekHigh");
    let read = fundamentals_read(price, trailing_pe, forward_pe, eps_ttm, low, high);

    let j = serde_json::json!({
        "ticker": ticker, "ok": true,
        "price": price, "trailingPE": trailing_pe, "forwardPE": forward_pe,
        "marketCap": market_cap, "priceToBook": price_to_book,
        "epsTtm": eps_ttm, "epsForward": eps_forward,
        "fiftyTwoWeekLow": low, "fiftyTwoWeekHigh": high,
        "read": read,
    });
    Ok(j.to_string())
}

#[tauri::command]
async fn fundamentals(ticker: String) -> Result<String, String> {
    let t = ticker.to_uppercase();
    match fundamentals_json(&t).await {
        Ok(json) => Ok(json),
        Err(e) => Ok(serde_json::json!({ "ticker": t, "ok": false, "error": e }).to_string()),
    }
}

async fn news_json(ticker: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .user_agent(YAHOO_UA)
        .build()
        .map_err(|e| format!("client build failed: {e}"))?;
    let mut url = reqwest::Url::parse("https://query1.finance.yahoo.com/v1/finance/search")
        .map_err(|e| format!("url parse failed: {e}"))?;
    url.query_pairs_mut()
        .append_pair("q", ticker)
        .append_pair("newsCount", "6")
        .append_pair("quotesCount", "1")
        .append_pair("enableFuzzyQuery", "false");

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("search request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("search body parse failed: {e}"))?;

    let sector = body
        .pointer("/quotes/0/sectorDisp")
        .or_else(|| body.pointer("/quotes/0/sector"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let industry = body
        .pointer("/quotes/0/industryDisp")
        .or_else(|| body.pointer("/quotes/0/industry"))
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let items: Vec<serde_json::Value> = body
        .get("news")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .take(6)
                .map(|n| {
                    serde_json::json!({
                        "title": n.get("title").and_then(|x| x.as_str()).unwrap_or(""),
                        "publisher": n.get("publisher").and_then(|x| x.as_str()).unwrap_or(""),
                        "link": n.get("link").and_then(|x| x.as_str()).unwrap_or(""),
                        "unixTime": n.get("providerPublishTime").and_then(|x| x.as_i64()),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let j = serde_json::json!({
        "ticker": ticker, "ok": true, "sector": sector, "industry": industry, "items": items,
    });
    Ok(j.to_string())
}

#[tauri::command]
async fn news(ticker: String) -> Result<String, String> {
    let t = ticker.to_uppercase();
    match news_json(&t).await {
        Ok(json) => Ok(json),
        Err(e) => Ok(serde_json::json!({ "ticker": t, "ok": false, "error": e }).to_string()),
    }
}

#[tauri::command]
fn scan(ticker: String, timeframe: String) -> Result<String, String> {
    scan_json(&ticker, &timeframe)
}

#[tauri::command]
fn chart(ticker: String, timeframe: String) -> Result<String, String> {
    chart_json(&ticker, &timeframe)
}

#[tauri::command]
fn backtest(ticker: String, timeframe: String) -> Result<String, String> {
    backtest_json(&ticker, &timeframe)
}

// Save plain, undecorated text to the user's Downloads or Desktop. Returns the full path.
#[tauri::command]
fn save_text(filename: String, contents: String, location: String) -> Result<String, String> {
    let home = std::env::var("HOME").map_err(|_| "no HOME env".to_string())?;
    let dir = match location.as_str() {
        "desktop" => PathBuf::from(&home).join("Desktop"),
        "downloads" => PathBuf::from(&home).join("Downloads"),
        // any absolute path (from Choose Folder) is used directly
        p if p.starts_with('/') => PathBuf::from(p),
        _ => PathBuf::from(&home).join("Downloads"),
    };
    // sanitize filename: strip path separators, allow .txt or .json, else default .txt
    let mut name = filename.replace(['/', '\\'], "_");
    let lower = name.to_lowercase();
    if !(lower.ends_with(".txt") || lower.ends_with(".json")) {
        name.push_str(".txt");
    }
    let path = dir.join(name);
    std::fs::write(&path, contents).map_err(|e| format!("could not write file: {e}"))?;
    Ok(path.to_string_lossy().into_owned())
}

// Open an external reference URL in the user's default browser.
// Only http/https is allowed, and the URL must parse as a simple absolute URL with no
// shell metacharacters, so this can never become a command-injection hole.
#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    let lower = url.to_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err("only http(s) URLs are allowed".into());
    }
    if url.chars().any(|c| c.is_whitespace() || c == '\'' || c == '"' || c == '`'
        || c == ';' || c == '|' || c == '&' || c == '$' || c == '<' || c == '>' || c == '\\')
    {
        return Err("URL contains illegal characters".into());
    }

    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = std::process::Command::new("open");
        c.arg(&url);
        c
    };
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", "", &url]);
        c
    };
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let mut cmd = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(&url);
        c
    };

    cmd.spawn().map_err(|e| format!("could not open browser: {e}"))?;
    Ok(())
}

// Native folder picker (macOS) via AppleScript; returns the chosen POSIX path, or an
// empty string if the user cancels.
#[tauri::command]
fn choose_folder() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        let script = "try\n set f to choose folder with prompt \"Choose export folder\"\n return POSIX path of f\non error number -128\n return \"\"\nend try";
        let out = std::process::Command::new("osascript")
            .arg("-e").arg(script)
            .output()
            .map_err(|e| format!("could not open folder picker: {e}"))?;
        return Ok(String::from_utf8_lossy(&out.stdout).trim().to_string());
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("folder picker is macOS-only in this build".into())
    }
}

// Parity harness: `swingr --parity <ohlcv.csv>` reads an OHLCV CSV (header:
// Open,High,Low,Close,Volume) and prints the LAST row's indicators as JSON, so it can be
// diffed against the Python reference (tools/strategy.compute_indicators).
fn run_parity(csv_path: &str) {
    let text = std::fs::read_to_string(csv_path).expect("read csv");
    let mut d = engine::Ohlcv { open: vec![], high: vec![], low: vec![], close: vec![], volume: vec![], ts: vec![] };
    for (i, line) in text.lines().enumerate() {
        if i == 0 && line.to_lowercase().contains("close") { continue; } // header
        let f: Vec<f64> = line.split(',').map(|s| s.trim().parse::<f64>().unwrap_or(f64::NAN)).collect();
        if f.len() < 5 { continue; }
        d.open.push(f[0]); d.high.push(f[1]); d.low.push(f[2]); d.close.push(f[3]); d.volume.push(f[4]);
    }
    let ind = engine::compute_indicators(&d);
    // Emit EVERY bar as CSV so the harness can diff per-bar (not just the last row).
    let g = |x: f64| if x.is_finite() { format!("{:.6}", x) } else { String::new() };
    println!("idx,close,ema9,ema20,ema200,rsi,atr,vol_sma20,obv,obv_sma10,ema200_20ago");
    for i in 0..d.len() {
        println!("{},{:.6},{},{},{},{},{},{},{},{},{}",
            i, d.close[i], g(ind.ema9[i]), g(ind.ema20[i]), g(ind.ema200[i]), g(ind.rsi[i]),
            g(ind.atr[i]), g(ind.vol_sma20[i]), g(ind.obv[i]), g(ind.obv_sma10[i]), g(ind.ema200_20ago[i]));
    }
}

// Scan parity: `swingr --scan-parity <ohlcv.csv> <0|1 market_ok>` prints the full
// scan result JSON to diff against ema_analyzer.analyze_frame.
fn run_scan_parity(csv_path: &str, market_ok: bool) {
    let text = std::fs::read_to_string(csv_path).expect("read csv");
    let mut d = engine::Ohlcv { open: vec![], high: vec![], low: vec![], close: vec![], volume: vec![], ts: vec![] };
    for (i, line) in text.lines().enumerate() {
        if i == 0 && line.to_lowercase().contains("close") { continue; }
        let f: Vec<f64> = line.split(',').map(|s| s.trim().parse::<f64>().unwrap_or(f64::NAN)).collect();
        if f.len() < 5 { continue; }
        d.open.push(f[0]); d.high.push(f[1]); d.low.push(f[2]); d.close.push(f[3]); d.volume.push(f[4]);
    }
    let r = engine::scan_frame(&d, market_ok, 0.005);
    let jf = |x: f64| if x.is_finite() { format!("{:.4}", x) } else { "null".to_string() };
    let arr = |v: &Vec<f64>| { let s: Vec<String> = v.iter().map(|x| format!("{:.2}", x)).collect(); format!("[{}]", s.join(",")) };
    let jb = |b: bool| if b { "true" } else { "false" };
    let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    println!(
        "{{\"regime\":\"{}\",\"verdict\":\"{}\",\"reversal_confirmed\":{},\"slope_ok\":{},\"volume_ok\":{},\"rsi_ok\":{},\"market_ok\":{},\"setup_quality\":\"{}\",\"price\":{},\"ema9\":{},\"ema20\":{},\"ema200\":{},\"rsi\":{},\"atr\":{},\"entry\":{},\"take_profit\":{},\"stop_loss\":{},\"upside_pct\":{},\"downside_pct\":{},\"r_multiple\":{},\"rejection_zones\":{},\"support\":{}}}",
        esc(&r.regime), esc(&r.verdict), jb(r.reversal_confirmed), jb(r.slope_ok), jb(r.volume_ok),
        jb(r.rsi_ok), jb(r.market_ok), r.setup_quality, jf(r.price), jf(r.ema9), jf(r.ema20),
        jf(r.ema200), jf(r.rsi), jf(r.atr), jf(r.entry), jf(r.take_profit), jf(r.stop_loss),
        jf(r.upside_pct), jf(r.downside_pct), jf(r.r_multiple), arr(&r.rejection_zones), arr(&r.support),
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(pos) = args.iter().position(|a| a == "--parity") {
        let csv = args.get(pos + 1).expect("--parity needs a csv path");
        run_parity(csv);
        return;
    }
    if let Some(pos) = args.iter().position(|a| a == "--scan-parity") {
        let csv = args.get(pos + 1).expect("--scan-parity needs a csv path");
        let mkt = args.get(pos + 2).map(|s| s == "1").unwrap_or(true);
        run_scan_parity(csv, mkt);
        return;
    }
    // Backtest parity: `swingr --backtest-parity <ohlcv.csv> [atr_mult] [max_hold]`
    if let Some(pos) = args.iter().position(|a| a == "--backtest-parity") {
        let csv = args.get(pos + 1).expect("--backtest-parity needs a csv path");
        let atr_mult = args.get(pos + 2).and_then(|s| s.parse().ok()).unwrap_or(2.0);
        let max_hold = args.get(pos + 3).and_then(|s| s.parse().ok()).unwrap_or(52);
        let text = std::fs::read_to_string(csv).expect("read csv");
        let mut d = engine::Ohlcv { open: vec![], high: vec![], low: vec![], close: vec![], volume: vec![], ts: vec![] };
        for (i, line) in text.lines().enumerate() {
            if i == 0 && line.to_lowercase().contains("close") { continue; }
            let f: Vec<f64> = line.split(',').map(|s| s.trim().parse::<f64>().unwrap_or(f64::NAN)).collect();
            if f.len() < 5 { continue; }
            d.open.push(f[0]); d.high.push(f[1]); d.low.push(f[2]); d.close.push(f[3]); d.volume.push(f[4]);
        }
        let r = engine::backtest(&d, atr_mult, max_hold);
        let pf = if r.profit_factor.is_finite() { format!("{:.2}", r.profit_factor) } else { "null".to_string() };
        let trades: Vec<String> = r.log.iter().map(|t| format!(
            "{{\"entry_idx\":{},\"exit_idx\":{},\"entry\":{:.2},\"exit\":{:.2},\"stop\":{:.2},\"target\":{:.2},\"bars_held\":{},\"outcome\":\"{}\",\"r\":{:.2}}}",
            t.entry_idx, t.exit_idx, t.entry, t.exit, t.stop, t.target, t.bars_held, t.outcome, t.r)).collect();
        println!(
            "{{\"bars\":{},\"trades\":{},\"wins\":{},\"losses\":{},\"timeouts\":{},\"win_rate\":{:.1},\"avg_r\":{:.2},\"profit_factor\":{},\"total_r\":{:.2},\"note\":\"{}\",\"log\":[{}]}}",
            r.bars, r.trades, r.wins, r.losses, r.timeouts, r.win_rate, r.avg_r, pf, r.total_r,
            r.note.replace('"', "\\\""), trades.join(","),
        );
        return;
    }
    // Exercise the exact Tauri command bodies from the CLI (for parity tests vs stocks.py).
    if let Some(pos) = args.iter().position(|a| a == "--scan-json") {
        let t = args.get(pos + 1).expect("ticker");
        let tf = args.get(pos + 2).map(|s| s.as_str()).unwrap_or("weekly");
        match scan_json(t, tf) { Ok(s) => println!("{s}"), Err(e) => { eprintln!("{e}"); std::process::exit(1); } }
        return;
    }
    if let Some(pos) = args.iter().position(|a| a == "--bt-json") {
        let t = args.get(pos + 1).expect("ticker");
        let tf = args.get(pos + 2).map(|s| s.as_str()).unwrap_or("weekly");
        match backtest_json(t, tf) { Ok(s) => println!("{s}"), Err(e) => { eprintln!("{e}"); std::process::exit(1); } }
        return;
    }
    if let Some(pos) = args.iter().position(|a| a == "--chart-json") {
        let t = args.get(pos + 1).expect("ticker");
        let tf = args.get(pos + 2).map(|s| s.as_str()).unwrap_or("weekly");
        match chart_json(t, tf) { Ok(s) => println!("{s}"), Err(e) => { eprintln!("{e}"); std::process::exit(1); } }
        return;
    }
    // Live fetch + scan (Rust end-to-end): `swingr --fetch-scan <TICKER> <weekly|daily>`
    if let Some(pos) = args.iter().position(|a| a == "--fetch-scan") {
        let ticker = args.get(pos + 1).expect("--fetch-scan needs a ticker");
        let tf = args.get(pos + 2).map(|s| s.as_str()).unwrap_or("weekly");
        match fetch::fetch(ticker, tf) {
            Ok(d) if d.len() >= 60 => {
                let mkt = fetch::market_ok(tf);
                let r = engine::scan_frame(&d, mkt, 0.005);
                let jf = |x: f64| if x.is_finite() { format!("{:.4}", x) } else { "null".to_string() };
                let arr = |v: &Vec<f64>| { let s: Vec<String> = v.iter().map(|x| format!("{:.2}", x)).collect(); format!("[{}]", s.join(",")) };
                let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
                println!(
                    "{{\"ticker\":\"{}\",\"regime\":\"{}\",\"verdict\":\"{}\",\"setup_quality\":\"{}\",\"entry\":{},\"take_profit\":{},\"stop_loss\":{},\"upside_pct\":{},\"downside_pct\":{},\"r_multiple\":{},\"ema9\":{},\"ema20\":{},\"ema200\":{},\"rsi\":{},\"atr\":{},\"rejection_zones\":{},\"support\":{},\"bars\":{}}}",
                    ticker.to_uppercase(), esc(&r.regime), esc(&r.verdict), r.setup_quality, jf(r.entry),
                    jf(r.take_profit), jf(r.stop_loss), jf(r.upside_pct), jf(r.downside_pct), jf(r.r_multiple),
                    jf(r.ema9), jf(r.ema20), jf(r.ema200), jf(r.rsi), jf(r.atr), arr(&r.rejection_zones), arr(&r.support), d.len(),
                );
            }
            Ok(d) => eprintln!("only {} bars for {ticker}", d.len()),
            Err(e) => eprintln!("{e}"),
        }
        return;
    }
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            scan, chart, backtest, save_text, open_url, choose_folder, fundamentals, news,
            findr::findr, findr::findr_cancel
        ])
        .run(tauri::generate_context!())
        .expect("error while running Swing R");
}
