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

/// Simple moving average (pandas rolling(window).mean()); NaN (f64::NAN) until window-1.
pub fn sma(x: &[f64], window: usize) -> Vec<f64> {
    let n = x.len();
    let mut out = vec![f64::NAN; n];
    if window == 0 || n < window {
        return out;
    }
    let mut sum: f64 = x[..window].iter().sum();
    out[window - 1] = sum / window as f64;
    for i in window..n {
        sum += x[i] - x[i - window];
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
