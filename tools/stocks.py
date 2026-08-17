# /// script
# requires-python = ">=3.10"
# dependencies = [
#   "yfinance>=0.2.40",
#   "pandas>=2.0",
#   "numpy>=1.24",
# ]
# ///
"""
stocks — one command line for the EMA reversal system.

Subcommands:
  scan      one-ticker live read (verdict, entry/TP/stop, filters, sizing)
  backtest  measure the rules over history (win-rate, avg R, expectancy)
  watch     scan a list of tickers and print a one-line verdict for each

Examples:
  python tools/stocks.py scan BSX
  python tools/stocks.py scan BSX --amount 50000 --timeframe weekly
  python tools/stocks.py backtest BSX ABBV PFE --timeframe daily
  python tools/stocks.py watch BSX ABBV PFE JNJ KO

Decision support from indicator rules — not a prediction oracle, not licensed advice.
"""
from __future__ import annotations

import argparse
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ema_analyzer as ea  # noqa: E402
import backtest as bt  # noqa: E402


def cmd_scan(args) -> None:
    a = ea.analyze(args.ticker, timeframe=args.timeframe)
    if args.json:
        import json
        from dataclasses import asdict
        print(json.dumps(asdict(a), indent=2, default=float))
    else:
        print(ea.render(a, args.amount))


def cmd_backtest(args) -> None:
    results = [bt.backtest(t, args.timeframe, args.atr_mult, args.max_hold) for t in args.tickers]
    if args.json:
        import json
        from dataclasses import asdict
        print(json.dumps([asdict(r) for r in results], indent=2, default=float))
        return
    print("\n\n".join(bt.render(r) for r in results))
    if len(results) > 1:
        import numpy as np
        agg = [t["r"] for r in results for t in r.trade_log]
        if agg:
            print(f"\n=== PORTFOLIO: {len(agg)} trades  avg {np.mean(agg):+.2f}R  "
                  f"total {sum(agg):+.1f}R ===")


def cmd_watch(args) -> None:
    print(f"{'TICKER':8s} {'VERDICT':40s} {'PRICE':>9s} {'TP':>9s} {'R':>6s}  QUALITY")
    print("-" * 90)
    for t in args.tickers:
        try:
            a = ea.analyze(t, timeframe=args.timeframe)
            print(f"{a.ticker:8s} {a.verdict[:40]:40s} {a.entry:9.2f} {a.take_profit:9.2f} "
                  f"{a.r_multiple:6.2f}  {a.setup_quality}")
        except SystemExit as e:
            print(f"{t.upper():8s} {'ERROR: ' + str(e):40s}")
    print("-" * 90)
    print("Decision support from indicator rules — not a prediction, not advice. Log calls in journal/.")


def main() -> None:
    p = argparse.ArgumentParser(prog="stocks", description="EMA reversal system CLI.")
    sub = p.add_subparsers(dest="cmd", required=True)

    s = sub.add_parser("scan", help="one-ticker live read")
    s.add_argument("ticker")
    s.add_argument("--amount", type=float, default=50000)
    s.add_argument("--timeframe", choices=["weekly", "daily"], default="weekly")
    s.add_argument("--json", action="store_true")
    s.set_defaults(func=cmd_scan)

    b = sub.add_parser("backtest", help="measure the rules over history")
    b.add_argument("tickers", nargs="+")
    b.add_argument("--timeframe", choices=["weekly", "daily"], default="weekly")
    b.add_argument("--atr-mult", type=float, default=2.0)
    b.add_argument("--max-hold", type=int, default=52)
    b.add_argument("--json", action="store_true")
    b.set_defaults(func=cmd_backtest)

    w = sub.add_parser("watch", help="one-line verdict for a list of tickers")
    w.add_argument("tickers", nargs="+")
    w.add_argument("--timeframe", choices=["weekly", "daily"], default="weekly")
    w.set_defaults(func=cmd_watch)

    args = p.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
