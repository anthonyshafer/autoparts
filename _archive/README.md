# _archive

Superseded artifacts, kept for reference only. Not part of the live build.

## tkinter-app/
The original **Tkinter desktop GUI** and its **PyInstaller packaging**, replaced by the
native **Rust + Tauri** app in `desktop/` (v1.4.0+). Contents:

- `gui.py` — the old Tkinter GUI (was `tools/gui.py`)
- `StockScanner.spec` — PyInstaller spec for the Tkinter app
- `build_macos.sh`, `build_windows.bat`, `setup_windows.bat` — PyInstaller build scripts
- `scan.bat`, `launch_gui.bat` — Windows launchers
- `requirements.txt` — pip deps for the Tkinter GUI / pip-based CLI install
- `build-apps.yml` — the old GitHub Actions workflow that built the Tkinter `.exe`/`.dmg`

**Still active (NOT archived):** the Python CLI engine in `tools/` (`strategy.py`,
`ema_analyzer.py`, `backtest.py`, `stocks.py`, `simulate.py`) remains the reference
implementation and the parity oracle for the Rust port — `tests/parity/` imports it.

Note: archiving `build-apps.yml` removes the old **Windows** build. The Tauri app currently
builds via `.github/workflows/build-desktop.yml` on **macOS only**; a Windows Tauri build is
not yet wired up.
