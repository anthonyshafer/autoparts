// StockScanner — Tauri (Rust) desktop app. Self-contained: the scan/backtest commands run
// the NATIVE Rust engine (engine.rs + fetch.rs), which is parity-proven against the Python
// tool. No Python at runtime — the app is a single standalone binary.

mod engine;
mod fetch;

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

#[tauri::command]
fn scan(ticker: String, timeframe: String) -> Result<String, String> {
    scan_json(&ticker, &timeframe)
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
    // sanitize filename: strip path separators, force a .txt extension
    let mut name = filename.replace(['/', '\\'], "_");
    if !name.to_lowercase().ends_with(".txt") {
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

// Parity harness: `stockscanner --parity <ohlcv.csv>` reads an OHLCV CSV (header:
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

// Scan parity: `stockscanner --scan-parity <ohlcv.csv> <0|1 market_ok>` prints the full
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
    // Backtest parity: `stockscanner --backtest-parity <ohlcv.csv> [atr_mult] [max_hold]`
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
    // Live fetch + scan (Rust end-to-end): `stockscanner --fetch-scan <TICKER> <weekly|daily>`
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
        .invoke_handler(tauri::generate_handler![scan, backtest, save_text, open_url, choose_folder])
        .run(tauri::generate_context!())
        .expect("error while running StockScanner");
}
