// StockScanner — Tauri (Rust) desktop shell over the existing Python engine.
//
// The Rust side is deliberately thin: it locates the repo's Python CLI (tools/stocks.py)
// and invokes it with --json, then hands the JSON straight to the web UI, which renders a
// Bloomberg-style terminal skin. This keeps 100% of the trading logic in the audited Python
// engine (one source of truth) while giving a real web-tech UI.
//
// mac-only POC: assumes `python3` (or a venv) with the deps is available on PATH.

mod engine;
mod fetch;

use std::path::PathBuf;
use std::process::Command;

// Resolve the repo root: the packaged app sets STOCKSCANNER_ROOT; in dev we walk up from
// the executable / current dir to find tools/stocks.py.
fn repo_root() -> Option<PathBuf> {
    // 1. runtime env var
    if let Ok(env_root) = std::env::var("STOCKSCANNER_ROOT") {
        let p = PathBuf::from(env_root);
        if p.join("tools/stocks.py").exists() {
            return Some(p);
        }
    }
    // 2. persisted config file (~/.config/stockscanner/root) — written once, works for
    //    any copy of the app regardless of where it's moved.
    if let Ok(home) = std::env::var("HOME") {
        let cfg = PathBuf::from(&home).join(".config/stockscanner/root");
        if let Ok(s) = std::fs::read_to_string(&cfg) {
            let p = PathBuf::from(s.trim());
            if p.join("tools/stocks.py").exists() {
                return Some(p);
            }
        }
    }
    // 3. compile-time default baked in at build (set SS_DEFAULT_ROOT when building)
    if let Some(def) = option_env!("SS_DEFAULT_ROOT") {
        let p = PathBuf::from(def);
        if p.join("tools/stocks.py").exists() {
            return Some(p);
        }
    }
    // dev: start from CWD and the exe dir, walk up looking for tools/stocks.py
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.to_path_buf());
        }
    }
    for start in candidates {
        let mut dir = Some(start.as_path());
        while let Some(d) = dir {
            if d.join("tools/stocks.py").exists() {
                return Some(d.to_path_buf());
            }
            dir = d.parent();
        }
    }
    None
}

fn python_bin(root: &PathBuf) -> String {
    // Prefer a local venv if present, else system python3.
    for cand in [".venv_gui/bin/python", ".venv/bin/python"] {
        if root.join(cand).exists() {
            return root.join(cand).to_string_lossy().into_owned();
        }
    }
    "python3".to_string()
}

// Run tools/stocks.py <args...> --json and return stdout (JSON) or an error string.
fn run_cli(args: &[&str]) -> Result<String, String> {
    let root = repo_root().ok_or_else(|| {
        "Could not locate tools/stocks.py. Set STOCKSCANNER_ROOT to the repo path.".to_string()
    })?;
    let py = python_bin(&root);
    let output = Command::new(&py)
        .arg("tools/stocks.py")
        .args(args)
        .arg("--json")
        .current_dir(&root)
        .output()
        .map_err(|e| format!("failed to launch {py}: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[tauri::command]
fn scan(ticker: String, timeframe: String) -> Result<String, String> {
    run_cli(&["scan", &ticker, "--timeframe", &timeframe])
}

#[tauri::command]
fn backtest(ticker: String, timeframe: String) -> Result<String, String> {
    run_cli(&["backtest", &ticker, "--timeframe", &timeframe])
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
    let mut d = engine::Ohlcv { open: vec![], high: vec![], low: vec![], close: vec![], volume: vec![] };
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
    let mut d = engine::Ohlcv { open: vec![], high: vec![], low: vec![], close: vec![], volume: vec![] };
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
