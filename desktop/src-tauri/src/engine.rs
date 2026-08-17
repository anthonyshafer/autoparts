// Native Rust port of the Python engine (tools/strategy.py indicators).
// Seeded to match pandas ewm(adjust=False) + NaN handling so outputs match the Python
// reference bar-for-bar. Verified by the parity harness in tests/parity/.

#[derive(Debug, Clone)]
pub struct Ohlcv {
    pub open: Vec<f64>,
    pub high: Vec<f64>,
    pub low: Vec<f64>,
    pub close: Vec<f64>,
    pub volume: Vec<f64>,
}

impl Ohlcv {
    pub fn len(&self) -> usize {
        self.close.len()
    }
    pub fn is_empty(&self) -> bool {
        self.close.is_empty()
    }
}

/// EWMA with adjust=False, seeded at the first element (pandas ewm(...).mean() default).
fn ewm_adjust_false(x: &[f64], alpha: f64) -> Vec<f64> {
    let mut out = Vec::with_capacity(x.len());
    if x.is_empty() {
        return out;
    }
    let mut y = x[0];
    out.push(y);
    for &v in &x[1..] {
        y = alpha * v + (1.0 - alpha) * y;
        out.push(y);
    }
    out
}

/// EMA by span (pandas ewm(span=span, adjust=False)); alpha = 2/(span+1).
pub fn ema(close: &[f64], span: usize) -> Vec<f64> {
    ewm_adjust_false(close, 2.0 / (span as f64 + 1.0))
}

/// Wilder RSI(period). Matches strategy.rsi: delta.diff, gain/loss, ewm(alpha=1/period,
/// adjust=False), rs = avg_gain/avg_loss (avg_loss==0 -> NaN -> RSI filled to 50).
pub fn rsi(close: &[f64], period: usize) -> Vec<f64> {
    let n = close.len();
    let mut out = vec![50.0_f64; n];
    if n < 2 {
        return out;
    }
    let alpha = 1.0 / period as f64;
    // deltas aligned to indices 1..n (delta[0] is NaN in pandas and dropped from the ewm seed)
    let mut gains = Vec::with_capacity(n - 1);
    let mut losses = Vec::with_capacity(n - 1);
    for i in 1..n {
        let d = close[i] - close[i - 1];
        gains.push(if d > 0.0 { d } else { 0.0 });
        losses.push(if d < 0.0 { -d } else { 0.0 });
    }
    let avg_gain = ewm_adjust_false(&gains, alpha);
    let avg_loss = ewm_adjust_false(&losses, alpha);
    for k in 0..avg_gain.len() {
        let g = avg_gain[k];
        let l = avg_loss[k];
        let val = if l == 0.0 {
            50.0 // rs = g/NaN -> NaN -> fillna(50)
        } else {
            let rs = g / l;
            100.0 - (100.0 / (1.0 + rs))
        };
        out[k + 1] = val; // aligned to close index k+1
    }
    out[0] = 50.0; // index 0 had NaN delta -> fillna(50)
    out
}

/// ATR(period). tr[0] = high-low (prev_close NaN dropped by pandas max skipna),
/// tr[i] = max(high-low, |high-prevclose|, |low-prevclose|), then ewm(alpha=1/period).
pub fn atr(d: &Ohlcv, period: usize) -> Vec<f64> {
    let n = d.len();
    let mut tr = Vec::with_capacity(n);
    for i in 0..n {
        let hl = d.high[i] - d.low[i];
        if i == 0 {
            tr.push(hl);
        } else {
            let pc = d.close[i - 1];
            let a = (d.high[i] - pc).abs();
            let b = (d.low[i] - pc).abs();
            tr.push(hl.max(a).max(b));
        }
    }
    ewm_adjust_false(&tr, 1.0 / period as f64)
}

/// OBV = cumsum(sign(close.diff().fillna(0)) * volume). diff[0]=NaN->0 -> obv[0]=0.
pub fn obv(close: &[f64], volume: &[f64]) -> Vec<f64> {
    let n = close.len();
    let mut out = Vec::with_capacity(n);
    let mut acc = 0.0;
    for i in 0..n {
        let dir = if i == 0 {
            0.0
        } else {
            let d = close[i] - close[i - 1];
            if d > 0.0 {
                1.0
            } else if d < 0.0 {
                -1.0
            } else {
                0.0
            }
        };
        acc += dir * volume[i];
        out.push(acc);
    }
    out
}

/// Simple moving average (pandas rolling(window).mean()); NaN until window-1.
/// Uses the SAME Kahan-compensated add/remove that pandas' roll_mean uses, so a flat
/// window yields the mean exactly (no streaming drift) — matching pandas bit-closely and
/// keeping exact `>=` comparisons (e.g. volume_ok on frozen OBV) consistent with Python.
pub fn sma(x: &[f64], window: usize) -> Vec<f64> {
    let n = x.len();
    let mut out = vec![f64::NAN; n];
    if window == 0 || n < window {
        return out;
    }
    let mut sum = 0.0_f64;
    let mut comp = 0.0_f64; // Kahan compensation
    let mut add = |val: f64, sum: &mut f64, comp: &mut f64| {
        let y = val - *comp;
        let t = *sum + y;
        *comp = (t - *sum) - y;
        *sum = t;
    };
    for i in 0..window {
        add(x[i], &mut sum, &mut comp);
    }
    out[window - 1] = sum / window as f64;
    for i in window..n {
        // remove x[i-window]
        let val = x[i - window];
        let y = -val - comp;
        let t = sum + y;
        comp = (t - sum) - y;
        sum = t;
        // add x[i]
        add(x[i], &mut sum, &mut comp);
        out[i] = sum / window as f64;
    }
    out
}

/// shift(k): out[i] = x[i-k], else NaN.
pub fn shift(x: &[f64], k: usize) -> Vec<f64> {
    let n = x.len();
    let mut out = vec![f64::NAN; n];
    for i in k..n {
        out[i] = x[i - k];
    }
    out
}

/// All indicators, matching tools/strategy.compute_indicators column-for-column.
pub struct Indicators {
    pub ema9: Vec<f64>,
    pub ema20: Vec<f64>,
    pub ema200: Vec<f64>,
    pub rsi: Vec<f64>,
    pub atr: Vec<f64>,
    pub vol_sma20: Vec<f64>,
    pub obv: Vec<f64>,
    pub obv_sma10: Vec<f64>,
    pub ema200_20ago: Vec<f64>,
}

pub fn compute_indicators(d: &Ohlcv) -> Indicators {
    let ema9 = ema(&d.close, 9);
    let ema20 = ema(&d.close, 20);
    let ema200 = ema(&d.close, 200);
    let rsi_v = rsi(&d.close, 14);
    let atr_v = atr(d, 14);
    let vol_sma20 = sma(&d.volume, 20);
    let obv_v = obv(&d.close, &d.volume);
    let obv_sma10 = sma(&obv_v, 10);
    let ema200_20ago = shift(&ema200, 20);
    Indicators {
        ema9,
        ema20,
        ema200,
        rsi: rsi_v,
        atr: atr_v,
        vol_sma20,
        obv: obv_v,
        obv_sma10,
        ema200_20ago,
    }
}

// ------------------------------------------------------------------------------------
// Scan port: swing levels, evaluate_row signal, compute_stop, and the full scan verdict
// (matches tools/strategy.evaluate_row + ema_analyzer.analyze_frame).
// ------------------------------------------------------------------------------------

/// Python round(x, nd): round-half-to-even (banker's rounding).
pub fn py_round(x: f64, nd: i32) -> f64 {
    if !x.is_finite() { return x; }
    let m = 10f64.powi(nd);
    let v = x * m;
    let f = v.floor();
    let diff = v - f;
    let r = if (diff - 0.5).abs() < 1e-9 {
        if (f as i64) % 2 == 0 { f } else { f + 1.0 }
    } else {
        v.round()
    };
    r / m
}
fn cents(x: f64) -> i64 { (x * 100.0).round() as i64 }
fn dedup_sort(mut v: Vec<f64>, ascending: bool) -> Vec<f64> {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v.dedup_by_key(|x| cents(*x));
    if !ascending { v.reverse(); }
    v
}

/// Local swing highs/lows (matches ema_analyzer._swing_levels, window=5).
pub fn swing_levels(d: &Ohlcv, window: usize) -> (Vec<f64>, Vec<f64>) {
    let (mut highs, mut lows) = (Vec::new(), Vec::new());
    let n = d.len();
    if n < 2 * window + 1 { return (highs, lows); }
    for i in window..(n - window) {
        let seg_h = &d.high[i - window..=i + window];
        let seg_l = &d.low[i - window..=i + window];
        let mx = seg_h.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let mn = seg_l.iter().cloned().fold(f64::INFINITY, f64::min);
        if d.high[i] == mx { highs.push(d.high[i]); }
        if d.low[i] == mn { lows.push(d.low[i]); }
    }
    (highs, lows)
}

pub struct ScanSignal {
    pub regime: String,
    pub reversal: bool,
    pub slope_ok: bool,
    pub volume_ok: bool,
    pub rsi_ok: bool,
    pub market_ok: bool,
    pub quality: u32,
    pub entry_ok: bool,
}

pub fn evaluate_last(d: &Ohlcv, ind: &Indicators, market_ok: bool, fair_band: f64) -> ScanSignal {
    let i = d.len() - 1;
    let price = d.close[i];
    let (e9, e20, e200) = (ind.ema9[i], ind.ema20[i], ind.ema200[i]);
    let regime = if price < e200 * (1.0 - fair_band) {
        "DISCOUNT"
    } else if price > e200 * (1.0 + fair_band) {
        "PREMIUM"
    } else {
        "FAIR"
    }.to_string();
    let reversal = price > e9 && price > e20 && e9 > e20;
    let e200_ref = ind.ema200_20ago[i];
    let slope_ok = e200_ref.is_finite() && e200 >= e200_ref;
    let vol_sma = ind.vol_sma20[i];
    let obv_sma = ind.obv_sma10[i];
    let volume_ok = (vol_sma.is_finite() && d.volume[i] >= vol_sma)
        || (obv_sma.is_finite() && ind.obv[i] >= obv_sma);
    let rsi_ok = ind.rsi[i].is_finite() && ind.rsi[i] < 70.0;
    let quality = slope_ok as u32 + volume_ok as u32 + rsi_ok as u32 + market_ok as u32 + reversal as u32;
    let entry_ok = regime == "DISCOUNT" && reversal && rsi_ok;
    ScanSignal { regime, reversal, slope_ok, volume_ok, rsi_ok, market_ok, quality, entry_ok }
}

pub fn compute_stop(entry: f64, atr_val: f64, ema20: f64, recent_low: Option<f64>, atr_mult: f64) -> f64 {
    let stop_atr = entry - atr_mult * atr_val;
    let struct_v = match recent_low {
        Some(l) => ema20.min(l),
        None => ema20,
    };
    let stop_struct = struct_v * 0.99;
    py_round(stop_atr.max(stop_struct.min(entry * 0.999)), 2)
}

/// Full scan output (the computed fields of ema_analyzer.Analysis; no fundamentals/name).
pub struct ScanResult {
    pub price: f64,
    pub ema9: f64,
    pub ema20: f64,
    pub ema200: f64,
    pub rsi: f64,
    pub atr: f64,
    pub regime: String,
    pub reversal_confirmed: bool,
    pub slope_ok: bool,
    pub volume_ok: bool,
    pub rsi_ok: bool,
    pub market_ok: bool,
    pub setup_quality: String,
    pub verdict: String,
    pub entry: f64,
    pub take_profit: f64,
    pub stop_loss: f64,
    pub rejection_zones: Vec<f64>,
    pub support: Vec<f64>,
    pub upside_pct: f64,
    pub downside_pct: f64,
    pub r_multiple: f64,
}

pub fn scan_frame(d: &Ohlcv, market_ok: bool, fair_band: f64) -> ScanResult {
    let ind = compute_indicators(d);
    let sig = evaluate_last(d, &ind, market_ok, fair_band);
    let i = d.len() - 1;
    let price = d.close[i];
    let (e9, e20, e200) = (ind.ema9[i], ind.ema20[i], ind.ema200[i]);
    let (rsi_v, atr_v) = (ind.rsi[i], ind.atr[i]);

    let (highs, lows) = swing_levels(d, 5);
    // rejection zones
    let rej: Vec<f64> = dedup_sort(
        highs.iter().filter(|&&v| price < v && v <= e200 * 1.02).map(|&v| py_round(v, 2)).collect(),
        true,
    );
    let mut rz = vec![py_round(e200, 2)];
    for &r in &rej {
        if (r - e200).abs() / e200 > 0.01 { rz.push(r); }
    }
    let rejection_zones = dedup_sort(rz, true);
    // support
    let mut below: Vec<f64> = lows.iter().cloned().filter(|&v| v < price).collect();
    below.sort_by(|a, b| b.partial_cmp(a).unwrap());
    below.truncate(3);
    let mut sup_src = vec![e20, e9];
    sup_src.extend(below.iter().cloned());
    let support = dedup_sort(
        sup_src.into_iter().filter(|&v| v < price).map(|v| py_round(v, 2)).collect(),
        false,
    );

    let entry = py_round(price, 2);
    let take_profit = py_round(e200, 2);
    let recent_low = below.first().cloned();
    let stop_loss = compute_stop(entry, atr_v, e20, recent_low, 2.0);
    let upside_pct = py_round((take_profit - entry) / entry * 100.0, 1);
    let downside_pct = py_round((stop_loss - entry) / entry * 100.0, 1);
    let risk = entry - stop_loss;
    let r_multiple = if risk > 0.0 { py_round((take_profit - entry) / risk, 2) } else { f64::NAN };

    let verdict = if sig.entry_ok && r_multiple >= 1.5 {
        "BUY".to_string()
    } else if sig.entry_ok {
        "BUY (thin R — size down)".to_string()
    } else if sig.regime == "DISCOUNT" && sig.reversal && !sig.market_ok {
        "HOLD FIRE — setup ok but market risk-off (SPY<200)".to_string()
    } else if sig.regime == "DISCOUNT" && sig.reversal && !sig.slope_ok {
        "CAUTION — reclaim ok but 200 sloping down (value-trap risk)".to_string()
    } else if sig.regime == "DISCOUNT" && !sig.reversal {
        "WATCH — wait for reclaim of 9/20".to_string()
    } else if sig.regime == "FAIR" {
        "WAIT — near fair value, poor risk/reward".to_string()
    } else {
        "AVOID — premium, price above the 200 line".to_string()
    };

    ScanResult {
        price: entry, ema9: py_round(e9, 2), ema20: py_round(e20, 2), ema200: py_round(e200, 2),
        rsi: py_round(rsi_v, 1), atr: py_round(atr_v, 2),
        regime: sig.regime, reversal_confirmed: sig.reversal, slope_ok: sig.slope_ok,
        volume_ok: sig.volume_ok, rsi_ok: sig.rsi_ok, market_ok: sig.market_ok,
        setup_quality: format!("{}/5", sig.quality), verdict, entry, take_profit, stop_loss,
        rejection_zones, support, upside_pct, downside_pct, r_multiple,
    }
}
