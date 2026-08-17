"""
StockScanner — a desktop GUI for the EMA reversal (swing/position) system.

Uses Tkinter (ships with Python, no extra GUI dependency). Wraps the same
strategy.evaluate_row() logic the CLI and backtest use, so the app, the command line,
and the backtest can never silently disagree.

Themes: Amber (Bloomberg), Green phosphor, Dark modern, Light — switchable live from the
Theme dropdown. Defaults to Amber.

Run from source:   python tools/gui.py
Packaged app:      double-click StockScanner.exe / StockScanner.app
Headless check:    StockScanner --selftest

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


APP_TITLE = "StockScanner — EMA Swing System"
DISCLAIMER = ("Decision support from indicator rules. Not a prediction, not 95% accurate, "
              "not licensed advice. You decide and execute.")

# ---- Themes -------------------------------------------------------------------
# Each theme: bg, fg (normal text), accent (headers/labels), field bg, up/down colors,
# and a warn color for WATCH/CAUTION. mono = use a monospace font (terminal feel).
THEMES = {
    "Amber (Bloomberg)": dict(
        bg="#0a0a0a", panel="#141210", fg="#ffb000", accent="#ff8c00",
        field="#1c1810", field_fg="#ffcc44", up="#2ecc40", down="#ff4136",
        warn="#ffdc00", btn="#2a2416", mono=True,
    ),
    "Green phosphor": dict(
        bg="#001100", panel="#001a00", fg="#33ff66", accent="#00ff41",
        field="#002200", field_fg="#66ff88", up="#00ff41", down="#ff5555",
        warn="#aaff00", btn="#003300", mono=True,
    ),
    "Dark modern": dict(
        bg="#1e222a", panel="#252b36", fg="#e6e6e6", accent="#4aa3ff",
        field="#2c3340", field_fg="#ffffff", up="#26de81", down="#fc5c65",
        warn="#fed330", btn="#323a48", mono=False,
    ),
    "Light": dict(
        bg="#f4f4f4", panel="#ffffff", fg="#1a1a1a", accent="#0057b8",
        field="#ffffff", field_fg="#000000", up="#0a8a2a", down="#c0271a",
        warn="#b8860b", btn="#e6e6e6", mono=False,
    ),
}
DEFAULT_THEME = "Amber (Bloomberg)"


class App(tk.Tk):
    def __init__(self) -> None:
        super().__init__()
        self.title(APP_TITLE)
        self.geometry("860x660")
        self.minsize(720, 540)
        self._q: queue.Queue = queue.Queue()
        self._theme_name = DEFAULT_THEME
        self._build()
        self._apply_theme(self._theme_name)
        self.after(120, self._drain)

    # ---- layout ----
    def _build(self) -> None:
        self.style = ttk.Style(self)
        try:
            self.style.theme_use("clam")   # 'clam' honors bg/fg colors reliably cross-OS
        except tk.TclError:
            pass

        self.top = ttk.Frame(self, padding=10)
        self.top.pack(fill="x")

        self.lbl_ticker = ttk.Label(self.top, text="TICKER")
        self.lbl_ticker.grid(row=0, column=0, sticky="w")
        self.ticker = ttk.Entry(self.top, width=10)
        self.ticker.grid(row=0, column=1, padx=(4, 14))
        self.ticker.insert(0, "BSX")
        self.ticker.bind("<Return>", lambda e: self._run_scan())

        self.lbl_tf = ttk.Label(self.top, text="TIMEFRAME")
        self.lbl_tf.grid(row=0, column=2, sticky="w")
        self.timeframe = ttk.Combobox(self.top, width=8, values=["weekly", "daily"], state="readonly")
        self.timeframe.set("weekly")
        self.timeframe.grid(row=0, column=3, padx=(4, 14))

        self.lbl_cap = ttk.Label(self.top, text="CAPITAL $")
        self.lbl_cap.grid(row=0, column=4, sticky="w")
        self.amount = ttk.Entry(self.top, width=10)
        self.amount.grid(row=0, column=5, padx=(4, 14))
        self.amount.insert(0, "50000")

        self.scan_btn = ttk.Button(self.top, text="SCAN", command=self._run_scan)
        self.scan_btn.grid(row=0, column=6, padx=4)
        self.bt_btn = ttk.Button(self.top, text="BACKTEST", command=self._run_backtest)
        self.bt_btn.grid(row=0, column=7, padx=4)

        # theme picker
        self.lbl_theme = ttk.Label(self.top, text="THEME")
        self.lbl_theme.grid(row=0, column=8, sticky="e", padx=(14, 2))
        self.theme_box = ttk.Combobox(self.top, width=16, values=list(THEMES), state="readonly")
        self.theme_box.set(self._theme_name)
        self.theme_box.grid(row=0, column=9, padx=(0, 2))
        self.theme_box.bind("<<ComboboxSelected>>", lambda e: self._apply_theme(self.theme_box.get()))

        self.status = ttk.Label(self, text="Ready.", anchor="w", padding=(10, 2))
        self.status.pack(fill="x")

        self.out = tk.Text(self, wrap="word", borderwidth=0, padx=12, pady=10)
        self.out.pack(fill="both", expand=True, padx=10, pady=(4, 6))
        self.scroll = ttk.Scrollbar(self.out, command=self.out.yview)
        self.out.configure(yscrollcommand=self.scroll.set, state="disabled")
        self.scroll.pack(side="right", fill="y")

        self.foot = ttk.Label(self, text=DISCLAIMER, anchor="w", padding=(10, 4),
                              wraplength=820, justify="left")
        self.foot.pack(fill="x")

    # ---- theming ----
    def _apply_theme(self, name: str) -> None:
        t = THEMES[name]
        self._theme_name = name
        self._t = t
        mono = ("Courier New", 12) if t["mono"] else ("Segoe UI", 11)
        mono_b = ("Courier New", 12, "bold") if t["mono"] else ("Segoe UI", 11, "bold")

        self.configure(bg=t["bg"])
        self.style.configure(".", background=t["bg"], foreground=t["fg"], font=mono)
        self.style.configure("TFrame", background=t["bg"])
        self.style.configure("TLabel", background=t["bg"], foreground=t["accent"], font=mono_b)
        self.style.configure("TButton", background=t["btn"], foreground=t["fg"],
                             font=mono_b, borderwidth=1, focuscolor=t["accent"])
        self.style.map("TButton", background=[("active", t["accent"])],
                       foreground=[("active", t["bg"])])
        self.style.configure("TEntry", fieldbackground=t["field"], foreground=t["field_fg"],
                             insertcolor=t["fg"])
        self.style.configure("TCombobox", fieldbackground=t["field"], background=t["btn"],
                             foreground=t["field_fg"], arrowcolor=t["fg"])
        self.style.map("TCombobox", fieldbackground=[("readonly", t["field"])],
                       foreground=[("readonly", t["field_fg"])])
        # status + footer are TLabels but want plain fg (not accent)
        for w in (self.status, self.foot):
            w.configure(background=t["bg"], foreground=t["fg"], font=mono)

        self.out.configure(bg=t["panel"], fg=t["fg"], insertbackground=t["fg"],
                           font=("Courier New", 12) if t["mono"] else ("Menlo", 12))
        # text tags for colored output
        self.out.tag_configure("up", foreground=t["up"])
        self.out.tag_configure("down", foreground=t["down"])
        self.out.tag_configure("warn", foreground=t["warn"])
        self.out.tag_configure("accent", foreground=t["accent"],
                               font=(mono[0], 13, "bold"))
        self.out.tag_configure("dim", foreground=t["field_fg"])

    # ---- worker plumbing ----
    def _busy(self, on: bool, msg: str = "") -> None:
        state = "disabled" if on else "normal"
        self.scan_btn.configure(state=state)
        self.bt_btn.configure(state=state)
        if msg:
            self.status.configure(text=msg)

    def _write(self, text: str) -> None:
        self.out.configure(state="normal")
        self.out.delete("1.0", "end")
        for line in text.splitlines():
            tag = self._tag_for(line)
            self.out.insert("end", line + "\n", tag)
        self.out.configure(state="disabled")

    def _tag_for(self, line: str) -> str:
        low = line.lower()
        if line.strip().startswith("==="):
            return "accent"
        if "buy" in low and "avoid" not in low:
            return "up"
        if "avoid" in low:
            return "down"
        if "watch" in low or "caution" in low or "hold fire" in low or "wait" in low:
            return "warn"
        if any(k in low for k in ("take-profit", "+", "win", "target")) and "-" not in line[:4]:
            pass
        if "stop-loss" in low or "loss" in low:
            return "down"
        if "take-profit" in low:
            return "up"
        return ""

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
        sym = self.ticker.get().strip().upper()
        if not sym:
            messagebox.showwarning("StockScanner", "Enter a ticker (e.g. BSX).")
            return
        tf, amt = self.timeframe.get(), self._amount()
        self._busy(True, f"Scanning {sym} ({tf})…")

        def work() -> None:
            try:
                a = ea.analyze(sym, timeframe=tf)
                self._q.put(("result", ea.render(a, amt)))
            except SystemExit as e:
                self._q.put(("error", str(e)))
            except Exception as e:
                self._q.put(("error", f"{type(e).__name__}: {e}"))

        threading.Thread(target=work, daemon=True).start()

    def _run_backtest(self) -> None:
        sym = self.ticker.get().strip().upper()
        if not sym:
            messagebox.showwarning("StockScanner", "Enter a ticker (e.g. BSX).")
            return
        tf = self.timeframe.get()
        max_hold = 52 if tf == "weekly" else 120
        self._busy(True, f"Backtesting {sym} ({tf})… this can take a moment.")

        def work() -> None:
            try:
                r = bt.backtest(sym, timeframe=tf, max_hold=max_hold)
                self._q.put(("result", bt.render(r)))
            except SystemExit as e:
                self._q.put(("error", str(e)))
            except Exception as e:
                self._q.put(("error", f"{type(e).__name__}: {e}"))

        threading.Thread(target=work, daemon=True).start()


def _selftest() -> int:
    """Headless check that the FROZEN bundle runs with no Python/venv: exercises the
    bundled deps (pandas/numpy/strategy) without opening a window or hitting the network.
    """
    import pandas as pd
    import numpy as np
    from strategy import compute_indicators, evaluate_row
    n = 260
    base = np.linspace(10, 20, n)
    df = pd.DataFrame({
        "Open": base, "High": base + 0.5, "Low": base - 0.5,
        "Close": base, "Volume": np.full(n, 1_000_000.0),
    })
    d = compute_indicators(df)
    evaluate_row(d.iloc[-1], market_ok=True)
    print("StockScanner selftest OK")
    return 0


def main() -> None:
    if "--selftest" in sys.argv:
        raise SystemExit(_selftest())
    App().mainloop()


if __name__ == "__main__":
    main()
