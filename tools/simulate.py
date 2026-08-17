# /// script
# requires-python = ">=3.10"
# dependencies = ["numpy>=1.24"]
# ///
"""
Portfolio Monte Carlo over the research dossier's bear/base/bull scenarios.

Runs two risk profiles on the 5-ticker dossier (SOFI, ZETA, BSX, UBER, PFE):
  - AGGRESSIVE  — fully invested, concentrated in high-upside names, ignores the
                  "only buy discounts" rule, optimistic scenario weighting.
  - CONSERVATIVE — only the discount setup (BSX) + income (PFE), big cash buffer,
                   pessimistic scenario weighting.

For each ticker we model the 12-month price as a TRIANGULAR distribution with
min = bear low, mode = base mid, max = bull high (from the dossier). Scenario
"probability weighting" is applied by skewing the mode via a profile bias, then we
draw N paths, size by the profile's allocation, and report the portfolio return
distribution: mean, median, P10/P90, probability of loss, best/worst.

THIS IS NOT A PREDICTION. It is a what-if over ASSUMPTIONS (the scenario ranges and
the allocation/bias you choose). Garbage-in applies: the output is only as good as the
dossier ranges and the assumed weights, both stated explicitly below. Not licensed advice.
"""
from __future__ import annotations

import argparse
import json
import numpy as np

# 12-month scenario anchors from docs/stock-dossier-2026-08-16.md (as of 2026-08-16).
# price = current; bear/base/bull = representative price midpoints of each range.
# div = extra total-return yield added on top of price return (PFE only).
DOSSIER = {
    # current/bear/base/bull from the dossier's scenario midpoints (analyst-informed).
    # beta = ASSUMED systematic risk. PFE 0.28 is the dossier's sourced figure; the others
    # are rough estimates (high-beta growth ~1.3-1.5, BSX ~0.9), NOT measured — treat as a
    # sensitivity knob, not fact.
    #          current   bear   base   bull    div    beta
    "BSX":  dict(price=51.83, bear=43.0, base=61.5, bull=84.5, div=0.0,   beta=0.90),
    "SOFI": dict(price=18.29, bear=13.0, base=20.0, bull=29.5, div=0.0,   beta=1.45),
    "ZETA": dict(price=29.05, bear=20.5, base=31.0, bull=41.0, div=0.0,   beta=1.55),
    "UBER": dict(price=75.95, bear=66.0, base=95.0, bull=132.5, div=0.0,  beta=1.25),
    "PFE":  dict(price=26.79, bear=23.5, base=28.5, bull=34.0, div=0.064, beta=0.28),
}

# Shared market shock (one draw per path, applied to every name scaled by its beta) so
# the names are CORRELATED and the left tail is realistic — not magically diversified away.
MARKET_VOL = 0.16   # ~annual stdev of the broad-market surprise

# Allocation of a fixed capital pool per profile (fractions; remainder = cash at 0% return).
# NOTE: both profiles use bias=0.0 so the comparison is APPLES-TO-APPLES — the only things
# that differ are allocation, cash, and beta exposure. (An earlier version skewed the modes
# optimistically for aggressive / pessimistically for conservative, which unfairly inflated
# the spread; that "optimism knob" was removed.)
PROFILES = {
    "AGGRESSIVE": dict(
        alloc={"UBER": 0.30, "ZETA": 0.25, "BSX": 0.20, "SOFI": 0.20, "PFE": 0.05},
        bias=0.0,
        note="Fully invested, concentrated in high-upside/high-beta names, ignores the "
             "discount-only rule. Same scenario weighting as conservative — differs only "
             "by allocation + cash + beta.",
    ),
    "CONSERVATIVE": dict(
        alloc={"BSX": 0.20, "PFE": 0.20, "UBER": 0.10},   # 50% cash
        bias=0.0,
        note="Only the discount setup (BSX) + income (PFE) + a value tilt (UBER), 50% cash. "
             "Same scenario weighting as aggressive — the lower risk comes from cash + "
             "lower-beta names, not from a pessimistic thumb on the scale.",
    ),
}


def biased_mode(d: dict, bias: float) -> float:
    """Move the triangular mode toward bull (bias>0) or bear (bias<0), clamped in-range."""
    base = d["base"]
    if bias >= 0:
        return base + bias * (d["bull"] - base)
    return base + bias * (base - d["bear"])


def simulate(profile_name: str, capital: float, n: int, seed: int) -> dict:
    p = PROFILES[profile_name]
    rng = np.random.default_rng(seed)
    alloc = p["alloc"]
    cash_frac = max(0.0, 1.0 - sum(alloc.values()))

    # one shared market shock per path -> correlates all names, adds a real fat tail
    market_shock = rng.normal(0.0, MARKET_VOL, n)

    port_returns = np.zeros(n)
    per_ticker = {}
    for tk, frac in alloc.items():
        d = DOSSIER[tk]
        mode = biased_mode(d, p["bias"])
        draws = rng.triangular(d["bear"], mode, d["bull"], n)
        # fundamental (scenario) return + systematic market move scaled by beta
        ret = (draws - d["price"]) / d["price"] + d["div"] + d["beta"] * market_shock
        port_returns += frac * ret
        per_ticker[tk] = dict(
            alloc_pct=round(frac * 100, 1),
            exp_return_pct=round(float(ret.mean()) * 100, 1),
            p10_pct=round(float(np.percentile(ret, 10)) * 100, 1),
            p90_pct=round(float(np.percentile(ret, 90)) * 100, 1),
        )
    # cash contributes 0 return but dilutes the portfolio

    end_value = capital * (1.0 + port_returns)
    pnl = end_value - capital
    return dict(
        profile=profile_name,
        note=p["note"],
        capital=capital,
        cash_pct=round(cash_frac * 100, 1),
        n=n,
        exp_return_pct=round(float(port_returns.mean()) * 100, 2),
        median_return_pct=round(float(np.median(port_returns)) * 100, 2),
        p10_return_pct=round(float(np.percentile(port_returns, 10)) * 100, 2),
        p90_return_pct=round(float(np.percentile(port_returns, 90)) * 100, 2),
        prob_loss_pct=round(float((port_returns < 0).mean()) * 100, 1),
        exp_pnl=round(float(pnl.mean()), 0),
        worst_1pct_pnl=round(float(np.percentile(pnl, 1)), 0),
        best_1pct_pnl=round(float(np.percentile(pnl, 99)), 0),
        per_ticker=per_ticker,
    )


def render(r: dict) -> str:
    L = [f"### {r['profile']} — ${r['capital']:,.0f}, {r['cash_pct']:.0f}% cash, {r['n']:,} paths",
         f"_{r['note']}_", ""]
    L.append(f"- **Expected 12-mo return: {r['exp_return_pct']:+.2f}%**  (median {r['median_return_pct']:+.2f}%)")
    L.append(f"- **Range (P10–P90): {r['p10_return_pct']:+.1f}% to {r['p90_return_pct']:+.1f}%**")
    L.append(f"- **Probability of a loss: {r['prob_loss_pct']:.0f}%**")
    L.append(f"- Expected P&L: ${r['exp_pnl']:,.0f}  |  bad case (1st pct): ${r['worst_1pct_pnl']:,.0f}  |  good case (99th pct): ${r['best_1pct_pnl']:,.0f}")
    L.append("")
    L.append("| Ticker | Alloc | Exp return | P10 | P90 |")
    L.append("|---|---|---|---|---|")
    for tk, d in r["per_ticker"].items():
        L.append(f"| {tk} | {d['alloc_pct']:.0f}% | {d['exp_return_pct']:+.1f}% | {d['p10_pct']:+.1f}% | {d['p90_pct']:+.1f}% |")
    return "\n".join(L)


def main() -> None:
    ap = argparse.ArgumentParser(description="Monte Carlo the dossier under two risk profiles.")
    ap.add_argument("--capital", type=float, default=50000)
    ap.add_argument("--n", type=int, default=50000)
    ap.add_argument("--seed", type=int, default=42)
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args()

    results = [simulate(name, args.capital, args.n, args.seed) for name in PROFILES]
    if args.json:
        print(json.dumps(results, indent=2, default=float))
    else:
        print("\n\n".join(render(r) for r in results))


if __name__ == "__main__":
    main()
