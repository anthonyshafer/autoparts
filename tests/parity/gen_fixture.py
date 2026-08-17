# /// script
# requires-python = ">=3.10"
# dependencies = ["numpy>=1.24"]
# ///
"""Deterministic OHLCV fixtures (no network) for Rust<->Python parity.
Writes several CSVs into tests/parity/ that exercise edge branches the smooth fixture misses:
  ohlcv.csv   — main: trend + sine (general path)
  flat.csv    — all closes equal (OBV dir=0, RSI avg_loss=0)
  rising.csv  — strictly increasing (avg_loss stays 0 → RSI div-by-zero path)
  tailflat.csv— flat run at the end (OBV frozen, obv_sma10==obv)
  short16.csv — only 16 bars (undecayed EWM seed transient)
"""
import os
import numpy as np

HERE = os.path.dirname(__file__)


def write(name, o, h, l, c, v):
    with open(os.path.join(HERE, name), "w") as f:
        f.write("Open,High,Low,Close,Volume\n")
        for i in range(len(c)):
            f.write(f"{o[i]:.6f},{h[i]:.6f},{l[i]:.6f},{c[i]:.6f},{v[i]:.6f}\n")
    print(f"wrote {name} ({len(c)} rows)")


# main
n = 320
t = np.arange(n)
c = 50.0 + 0.05 * t + 8.0 * np.sin(t / 11.0) + 3.0 * np.sin(t / 3.3)
write("ohlcv.csv", c - 0.2 * np.sin(t / 4.0), c + 0.6 + 0.4 * np.abs(np.sin(t / 5.0)),
      c - 0.6 - 0.4 * np.abs(np.cos(t / 7.0)), c, 1_000_000.0 + 200_000.0 * np.abs(np.sin(t / 9.0)))

# flat: every close identical
n = 260
c = np.full(n, 100.0)
write("flat.csv", c, c + 0.5, c - 0.5, c, np.full(n, 1_000_000.0))

# rising: strictly increasing (no down bar ever)
c = 50.0 + 0.3 * np.arange(n)
write("rising.csv", c - 0.1, c + 0.4, c - 0.4, c, np.full(n, 900_000.0))

# tailflat: normal then 25 flat closes at the end
t = np.arange(n)
c = 60.0 + 5.0 * np.sin(t / 8.0) + 0.04 * t
c[-25:] = c[-25]
write("tailflat.csv", c - 0.2, c + 0.5, c - 0.5, c, 1_000_000.0 + 100_000.0 * np.sin(t / 6.0))

# short16: undecayed EWM seed transient
n = 16
t = np.arange(n)
c = 100.0 + np.sin(t) + 0.5 * t
write("short16.csv", c - 0.1, c + 0.3, c - 0.3, c, np.full(n, 500_000.0))
