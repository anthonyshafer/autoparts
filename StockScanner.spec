# -*- mode: python ; coding: utf-8 -*-
# PyInstaller spec for StockScanner — one-file, windowed (GUI) Windows build.
# Bundles Python + yfinance/pandas/numpy so the resulting .exe needs nothing installed.
from PyInstaller.utils.hooks import collect_all

datas, binaries, hiddenimports = [], [], []
# yfinance and curl_cffi pull data files / compiled bits PyInstaller can miss.
for pkg in ("yfinance", "curl_cffi"):
    d, b, h = collect_all(pkg)
    datas += d
    binaries += b
    hiddenimports += h

hiddenimports += [
    "pandas", "numpy",
    "tkinter", "tkinter.ttk", "tkinter.scrolledtext", "tkinter.messagebox",
]

a = Analysis(
    ["tools/gui.py"],
    pathex=["tools"],
    binaries=binaries,
    datas=datas,
    hiddenimports=hiddenimports,
    hookspath=[],
    runtime_hooks=[],
    excludes=["matplotlib", "PyQt5", "PySide2", "IPython", "pytest"],
    noarchive=False,
)
pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,
    [],
    name="StockScanner",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    console=False,          # windowed GUI, no console box
    disable_windowed_traceback=False,
)   # one-file build: all binaries/datas passed into EXE, no COLLECT step

