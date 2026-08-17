// StockScanner — Tauri (Rust) desktop shell over the existing Python engine.
//
// The Rust side is deliberately thin: it locates the repo's Python CLI (tools/stocks.py)
// and invokes it with --json, then hands the JSON straight to the web UI, which renders a
// Bloomberg-style terminal skin. This keeps 100% of the trading logic in the audited Python
// engine (one source of truth) while giving a real web-tech UI.
//
// mac-only POC: assumes `python3` (or a venv) with the deps is available on PATH.

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

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![scan, backtest, save_text, open_url])
        .run(tauri::generate_context!())
        .expect("error while running StockScanner");
}
