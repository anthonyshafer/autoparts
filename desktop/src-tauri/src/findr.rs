// FINDR — bulk swing screener. Reuses the EXISTING fetch (fetch.rs) and scan (engine.rs)
// pipeline exactly as the single-ticker `scan` Tauri command does (see main.rs::scan_json):
// fetch::fetch(ticker, timeframe) -> engine::scan_frame(&d, market_ok, 0.005). This module adds
// NO new scan math — only universe sourcing, bounded-concurrency dispatch, and progress/hit
// streaming over that existing pipeline.
//
// ASSUMPTION: unlike scan_json (which calls fetch::market_ok(timeframe) — a full SPY fetch —
// on every single-ticker scan), FINDR computes market_ok ONCE per run and reuses it across all
// symbols. This is a deliberate deviation justified by this task's explicit rate-limit safety
// requirement (a full A-Z run is ~13k symbols; 13k redundant SPY fetches would multiply Yahoo
// load ~2x for no behavioral difference, since market_ok is timeframe-scoped, not
// symbol-scoped). engine::scan_frame — the reused scan math itself — is called identically to
// scan_json with the same `fair_band` (0.005) argument.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::engine;
use crate::fetch;

const CONCURRENCY: usize = 6;
const PROGRESS_EVERY: usize = 15;
const RETRY_BACKOFFS_MS: [u64; 2] = [400, 900];

const NASDAQ_LISTED_URL: &str = "https://www.nasdaqtrader.com/dynamic/SymDir/nasdaqlisted.txt";
const OTHER_LISTED_URL: &str = "https://www.nasdaqtrader.com/dynamic/SymDir/otherlisted.txt";
const FOOTER_MARKER: &str = "File Creation Time";

// Security-Name substrings (lowercased) that exclude a listing from "common stock".
const EXCLUDE_NAME_SUBSTRINGS: [&str; 8] = [
    "warrant",
    "right",
    " unit",
    "depositary",
    "preferred",
    "convertible",
    "debenture",
    " notes",
];

// ---- Process-lifetime caches ----

static UNIVERSE_CACHE: OnceLock<Mutex<Vec<(String, String)>>> = OnceLock::new();
static CANCEL_FLAGS: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> = OnceLock::new();

fn universe_cell() -> &'static Mutex<Vec<(String, String)>> {
    UNIVERSE_CACHE.get_or_init(|| Mutex::new(Vec::new()))
}

fn cancel_map() -> &'static Mutex<HashMap<String, Arc<AtomicBool>>> {
    CANCEL_FLAGS.get_or_init(|| Mutex::new(HashMap::new()))
}

// Poisoned-mutex recovery: a panic elsewhere in the process must not permanently brick the
// universe cache or cancel-flag map for the rest of the app's lifetime. The guarded data (a
// plain Vec/HashMap) carries no invariant that a foreign panic could have left inconsistent
// mid-mutation for our own single-statement critical sections, so recovering the poisoned
// guard is safe here.
fn lock_or_recover<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

// ---- Universe filtering ----

/// Matches ^[A-Z]{1,5}$ — 1 to 5 uppercase ASCII letters, nothing else. Drops preferreds,
/// warrants, when-issued, and other non-common-stock tickers that carry '.', '$', '-', digits.
fn is_common_stock_symbol(s: &str) -> bool {
    !s.is_empty() && s.len() <= 5 && s.chars().all(|c| c.is_ascii_uppercase())
}

fn name_excluded(name: &str) -> bool {
    let lower = name.to_lowercase();
    EXCLUDE_NAME_SUBSTRINGS
        .iter()
        .any(|needle| lower.contains(needle))
}

/// nasdaqlisted.txt: Symbol|Security Name|Market Category|Test Issue|Financial Status|
/// Round Lot Size|ETF|NextShares
fn parse_nasdaq_listed(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in text.lines() {
        if line.contains(FOOTER_MARKER) {
            continue;
        }
        let cols: Vec<&str> = line.split('|').collect();
        if cols.len() < 8 || cols[0] == "Symbol" {
            continue; // header or malformed row
        }
        let (symbol, name, test_issue, etf) = (
            cols[0].trim(),
            cols[1].trim(),
            cols[3].trim(),
            cols[6].trim(),
        );
        if test_issue == "Y" || etf == "Y" || !is_common_stock_symbol(symbol) || name_excluded(name)
        {
            continue;
        }
        out.push((symbol.to_string(), name.to_string()));
    }
    out
}

/// otherlisted.txt: ACT Symbol|Security Name|Exchange|CQS Symbol|ETF|Round Lot Size|
/// Test Issue|NASDAQ Symbol
fn parse_other_listed(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in text.lines() {
        if line.contains(FOOTER_MARKER) {
            continue;
        }
        let cols: Vec<&str> = line.split('|').collect();
        if cols.len() < 8 || cols[0] == "ACT Symbol" {
            continue; // header or malformed row
        }
        let (symbol, name, etf, test_issue) = (
            cols[0].trim(),
            cols[1].trim(),
            cols[4].trim(),
            cols[6].trim(),
        );
        if test_issue == "Y" || etf == "Y" || !is_common_stock_symbol(symbol) || name_excluded(name)
        {
            continue;
        }
        out.push((symbol.to_string(), name.to_string()));
    }
    out
}

async fn download_text(client: &reqwest::Client, url: &str) -> Result<String, String> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("GET {url} failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("GET {url} returned HTTP {}", resp.status()));
    }
    resp.text()
        .await
        .map_err(|e| format!("read body {url} failed: {e}"))
}

async fn build_universe() -> Result<Vec<(String, String)>, String> {
    let client = reqwest::Client::builder()
        .user_agent("swingr-findr/1.0")
        .build()
        .map_err(|e| format!("client build failed: {e}"))?;

    let nasdaq_res = download_text(&client, NASDAQ_LISTED_URL).await;
    let other_res = download_text(&client, OTHER_LISTED_URL).await;

    if let (Err(e1), Err(e2)) = (&nasdaq_res, &other_res) {
        return Err(format!(
            "FINDR universe download failed for both sources: nasdaqlisted={e1}, otherlisted={e2}"
        ));
    }

    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<(String, String)> = Vec::new();

    if let Ok(text) = nasdaq_res {
        for (sym, name) in parse_nasdaq_listed(&text) {
            if seen.insert(sym.clone()) {
                out.push((sym, name));
            }
        }
    }
    if let Ok(text) = other_res {
        for (sym, name) in parse_other_listed(&text) {
            if seen.insert(sym.clone()) {
                out.push((sym, name));
            }
        }
    }
    Ok(out)
}

/// Returns the cached, fully-parsed+filtered universe, downloading and populating the cache
/// on first call. Re-runs within the same process never re-download.
async fn universe() -> Result<Vec<(String, String)>, String> {
    {
        let cache = lock_or_recover(universe_cell());
        if !cache.is_empty() {
            return Ok(cache.clone());
        }
    }
    let fresh = build_universe().await?;
    if fresh.is_empty() {
        return Err(
            "FINDR universe is empty after filtering (both nasdaqlisted.txt and otherlisted.txt \
             downloads may have failed, and no cached universe is available)"
                .to_string(),
        );
    }
    let mut cache = lock_or_recover(universe_cell());
    if cache.is_empty() {
        *cache = fresh.clone();
    }
    Ok(cache.clone())
}

// ---- Scan dispatch (reuses fetch::fetch + engine::scan_frame exactly) ----

fn scan_one(symbol: &str, timeframe: &str, market_ok: bool) -> Result<engine::ScanResult, String> {
    let d = fetch::fetch(symbol, timeframe)?;
    if d.len() < 60 {
        return Err(format!(
            "only {} {} candles for {}",
            d.len(),
            timeframe,
            symbol
        ));
    }
    Ok(engine::scan_frame(&d, market_ok, 0.005))
}

/// Runs `scan_one` on the blocking-thread pool (it performs blocking network I/O via
/// `fetch::fetch`, which must never run inside a plain async task body).
async fn attempt_scan(
    symbol: String,
    timeframe: String,
    market_ok: bool,
) -> Result<engine::ScanResult, String> {
    tokio::task::spawn_blocking(move || scan_one(&symbol, &timeframe, market_ok))
        .await
        .map_err(|e| format!("blocking task join failed: {e}"))
        .and_then(|r| r)
}

#[derive(Clone, Serialize)]
struct FindrHit {
    run_id: String,
    symbol: String,
    name: String,
    verdict: String,
    r_multiple: Option<f64>,
    upside_pct: f64,
    price: f64,
    setup_quality: String,
}

#[tauri::command]
pub async fn findr(
    start: String,
    end: String,
    timeframe: String,
    run_id: String,
    app: AppHandle,
) -> Result<String, String> {
    let start_c = start
        .trim()
        .to_uppercase()
        .chars()
        .next()
        .ok_or_else(|| "start must be a single letter A-Z".to_string())?;
    let end_c = end
        .trim()
        .to_uppercase()
        .chars()
        .next()
        .ok_or_else(|| "end must be a single letter A-Z".to_string())?;
    if !start_c.is_ascii_uppercase() || !end_c.is_ascii_uppercase() {
        return Err("start/end must be A-Z letters".to_string());
    }
    let (lo, hi) = if start_c <= end_c {
        (start_c, end_c)
    } else {
        (end_c, start_c)
    };

    let tf = match timeframe.as_str() {
        "daily" => "daily".to_string(),
        _ => "weekly".to_string(),
    };

    let uni = universe().await?;
    let symbols: Vec<(String, String)> = uni
        .into_iter()
        .filter(|(sym, _)| sym.chars().next().is_some_and(|c| c >= lo && c <= hi))
        .collect();
    let total = symbols.len();

    let cancel_flag = Arc::new(AtomicBool::new(false));
    lock_or_recover(cancel_map()).insert(run_id.clone(), cancel_flag.clone());

    let _ = app.emit(
        "findr:progress",
        serde_json::json!({
            "run_id": run_id, "scanned": 0, "total": total,
            "found_count": 0, "current": serde_json::Value::Null,
        }),
    );

    // Computed once per run — see module-level ASSUMPTION note above.
    let market_ok = fetch::market_ok(&tf);

    let semaphore = Arc::new(Semaphore::new(CONCURRENCY));
    let scanned = Arc::new(AtomicUsize::new(0));
    let found_count = Arc::new(AtomicUsize::new(0));
    let hits: Arc<Mutex<Vec<FindrHit>>> = Arc::new(Mutex::new(Vec::new()));

    let mut set: JoinSet<()> = JoinSet::new();

    for (symbol, name) in symbols {
        let semaphore = semaphore.clone();
        let scanned = scanned.clone();
        let found_count = found_count.clone();
        let hits = hits.clone();
        let cancel_flag = cancel_flag.clone();
        let app = app.clone();
        let run_id = run_id.clone();
        let tf = tf.clone();

        set.spawn(async move {
            // Fast-path skip before consuming a permit.
            if cancel_flag.load(Ordering::Relaxed) {
                return;
            }
            let permit = match semaphore.acquire_owned().await {
                Ok(p) => p,
                Err(_) => return, // semaphore closed — treat as shutdown
            };
            // Re-check after acquiring the permit, per the cancellation contract.
            if cancel_flag.load(Ordering::Relaxed) {
                drop(permit);
                return;
            }

            // Initial attempt, then up to 2 retries with backoff (400ms, 900ms). Exhausting all
            // retries leaves `result` as None — the symbol still counts as scanned, not found.
            let mut result = attempt_scan(symbol.clone(), tf.clone(), market_ok)
                .await
                .ok();
            for &backoff_ms in RETRY_BACKOFFS_MS.iter() {
                if result.is_some() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                result = attempt_scan(symbol.clone(), tf.clone(), market_ok)
                    .await
                    .ok();
            }
            drop(permit);

            let n = scanned.fetch_add(1, Ordering::Relaxed) + 1;

            // Un-throttled per-symbol log line — fires once per symbol that actually finished
            // processing (success or fetch-error skip), independent of findr:hit and the
            // throttled findr:progress below.
            let (log_verdict, log_r_multiple, log_upside_pct): (String, Option<f64>, Option<f64>) =
                match &result {
                    Some(sr) => (
                        sr.verdict.clone(),
                        sr.r_multiple.is_finite().then_some(sr.r_multiple),
                        sr.upside_pct.is_finite().then_some(sr.upside_pct),
                    ),
                    None => ("SKIP (fetch error)".to_string(), None, None),
                };
            let _ = app.emit(
                "findr:log",
                serde_json::json!({
                    "run_id": run_id, "symbol": symbol, "verdict": log_verdict,
                    "r_multiple": log_r_multiple, "upside_pct": log_upside_pct,
                }),
            );

            if let Some(sr) = result {
                if sr.verdict.starts_with("BUY") {
                    let hit = FindrHit {
                        run_id: run_id.clone(),
                        symbol: symbol.clone(),
                        name,
                        verdict: sr.verdict,
                        r_multiple: sr.r_multiple.is_finite().then_some(sr.r_multiple),
                        upside_pct: sr.upside_pct,
                        price: sr.entry,
                        setup_quality: sr.setup_quality,
                    };
                    found_count.fetch_add(1, Ordering::Relaxed);
                    lock_or_recover(&hits).push(hit.clone());
                    let _ = app.emit("findr:hit", &hit);
                }
            }

            if n % PROGRESS_EVERY == 0 {
                let _ = app.emit(
                    "findr:progress",
                    serde_json::json!({
                        "run_id": run_id, "scanned": n, "total": total,
                        "found_count": found_count.load(Ordering::Relaxed), "current": symbol,
                    }),
                );
            }
        });
    }

    while set.join_next().await.is_some() {}

    let cancelled = cancel_flag.load(Ordering::Relaxed);
    let scanned_final = scanned.load(Ordering::Relaxed);

    let mut found: Vec<FindrHit> = lock_or_recover(&hits).clone();
    // Rank by r_multiple desc; missing/non-finite r_multiple sorts last.
    found.sort_by(|a, b| {
        let av = a.r_multiple.unwrap_or(f64::NEG_INFINITY);
        let bv = b.r_multiple.unwrap_or(f64::NEG_INFINITY);
        bv.partial_cmp(&av).unwrap_or(std::cmp::Ordering::Equal)
    });

    lock_or_recover(cancel_map()).remove(&run_id);

    let _ = app.emit(
        "findr:done",
        serde_json::json!({
            "run_id": run_id, "scanned": scanned_final, "total": total,
            "found_count": found.len(), "cancelled": cancelled,
        }),
    );

    let out = serde_json::json!({
        "run_id": run_id, "done": true, "cancelled": cancelled,
        "scanned": scanned_final, "total": total, "found": found,
    });
    Ok(out.to_string())
}

#[tauri::command]
pub fn findr_cancel(run_id: String) -> Result<(), String> {
    if let Some(flag) = lock_or_recover(cancel_map()).get(&run_id) {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}
