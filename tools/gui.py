"""
StockScanner — a tiny desktop GUI for the EMA reversal system.

Uses Tkinter (ships with Python, no extra GUI dependency). Wraps the same
strategy.evaluate_row() logic the CLI and backtest use, so the app, the command line,
and the backtest can never silently disagree.

Run from source:   python tools/gui.py
Packaged (.exe):   double-click StockScanner.exe  (see build_windows.bat / CI)

Decision support from indicator rules — not a prediction oracle, not licensed advice.
"""
from __future__ import annotations

import os
import sys
import threading
import queue

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import tkinter as tk
from tkinter import ttk, scrolledtext, messagebox

import ema_analyzer as ea
import backtest as bt


APP_TITLE = "StockScanner — EMA Reversal System"
DISCLAIMER = ("Decision support from indicator rules. Not a prediction, not 95% accurate, "
              "not licensed advice. You decide and execute.")


class App(tk.Tk):
    def __init__(self) -> None:
        super().__init__()
        self.title(APP_TITLE)
        self.geometry("820x640")
        self.minsize(700, 520)
        self._q: queue.Queue = queue.Queue()
        self._build()
        self.after(120, self._drain)

    def _build(self) -> None:
        top = ttk.Frame(self, padding=10)
        top.pack(fill="x")

        ttk.Label(top, text="Ticker:").grid(row=0, column=0, sticky="w")
        self.ticker = ttk.Entry(top, width=12)
        self.ticker.grid(row=0, column=1, padx=(4, 14))
        self.ticker.insert(0, "BSX")
        self.ticker.bind("<Return>", lambda e: self._run_scan())

        ttk.Label(top, text="Timeframe:").grid(row=0, column=2, sticky="w")
        self.timeframe = ttk.Combobox(top, width=8, values=["weekly", "daily"], state="readonly")
        self.timeframe.set("weekly")
        self.timeframe.grid(row=0, column=3, padx=(4, 14))

        ttk.Label(top, text="Capital $:").grid(row=0, column=4, sticky="w")
        self.amount = ttk.Entry(top, width=12)
        self.amount.grid(row=0, column=5, padx=(4, 14))
        self.amount.insert(0, "50000")

        self.scan_btn = ttk.Button(top, text="Scan", command=self._run_scan)
        self.scan_btn.grid(row=0, column=6, padx=4)
        self.bt_btn = ttk.Button(top, text="Backtest", command=self._run_backtest)
        self.bt_btn.grid(row=0, column=7, padx=4)

        self.status = ttk.Label(self, text="Ready.", anchor="w", padding=(10, 2))
        self.status.pack(fill="x")

        self.out = scrolledtext.ScrolledText(self, wrap="word", font=("Consolas", 11))
        self.out.pack(fill="both", expand=True, padx=10, pady=(4, 6))
        self.out.configure(state="disabled")

        foot = ttk.Label(self, text=DISCLAIMER, anchor="w", padding=(10, 4),
                         foreground="#888", wraplength=780, justify="left")
        foot.pack(fill="x")

    # --- worker plumbing (keep the UI responsive during network fetches) ---

    def _busy(self, on: bool, msg: str = "") -> None:
        state = "disabled" if on else "normal"
        self.scan_btn.configure(state=state)
        self.bt_btn.configure(state=state)
        if msg:
            self.status.configure(text=msg)

    def _write(self, text: str) -> None:
        self.out.configure(state="normal")
        self.out.delete("1.0", "end")
        self.out.insert("1.0", text)
        self.out.configure(state="disabled")

    def _drain(self) -> None:
        try:
            while True:
                kind, payload = self._q.get_nowait()
                if kind == "result":
                    self._write(payload)
                    self._busy(False, "Done.")
                elif kind == "error":
                    self._busy(False, "Error.")
                    messagebox.showerror("StockScanner", payload)
        except queue.Empty:
            pass
        self.after(120, self._drain)

    def _amount(self) -> float:
        try:
            return float(self.amount.get().replace(",", "").replace("$", "").strip())
        except ValueError:
            return 50000.0

    def _run_scan(self) -> None:
        tk_sym = self.ticker.get().strip().upper()
        if not tk_sym:
            messagebox.showwarning("StockScanner", "Enter a ticker (e.g. BSX).")
            return
        tf = self.timeframe.get()
        amt = self._amount()
        self._busy(True, f"Scanning {tk_sym} ({tf})…")

        def work() -> None:
            try:
                a = ea.analyze(tk_sym, timeframe=tf)
                self._q.put(("result", ea.render(a, amt)))
            except SystemExit as e:
                self._q.put(("error", str(e)))
            except Exception as e:  # network / data errors
                self._q.put(("error", f"{type(e).__name__}: {e}"))

        threading.Thread(target=work, daemon=True).start()

    def _run_backtest(self) -> None:
        tk_sym = self.ticker.get().strip().upper()
        if not tk_sym:
            messagebox.showwarning("StockScanner", "Enter a ticker (e.g. BSX).")
            return
        tf = self.timeframe.get()
        max_hold = 52 if tf == "weekly" else 120
        self._busy(True, f"Backtesting {tk_sym} ({tf})… this can take a moment.")

        def work() -> None:
            try:
                r = bt.backtest(tk_sym, timeframe=tf, max_hold=max_hold)
                self._q.put(("result", bt.render(r)))
            except SystemExit as e:
                self._q.put(("error", str(e)))
            except Exception as e:
                self._q.put(("error", f"{type(e).__name__}: {e}"))

        threading.Thread(target=work, daemon=True).start()


def main() -> None:
    App().mainloop()


if __name__ == "__main__":
    main()
