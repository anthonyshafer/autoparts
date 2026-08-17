#!/usr/bin/env bash
# ============================================================================
#  Build StockScanner.app — a standalone macOS app (no venv for end users).
#  Run this once on a Mac with Python 3.10+.
#  Output: dist/StockScanner.app  (double-clickable; drag to /Applications)
# ============================================================================
set -euo pipefail
cd "$(dirname "$0")"

echo "[1/3] Installing build tooling and dependencies ..."
python3 -m pip install --upgrade pip pyinstaller >/dev/null
python3 -m pip install -r requirements.txt

echo "[2/3] Building the app with PyInstaller ..."
python3 -m PyInstaller --noconfirm --clean StockScanner.spec

echo "[3/3] Done."
echo
echo "Your app is here:  dist/StockScanner.app"
echo "Double-click it, or drag it to /Applications."
echo
echo "Note: on first launch macOS Gatekeeper may block an unsigned app."
echo "If so: right-click the app -> Open -> Open, OR run:"
echo "  xattr -dr com.apple.quarantine dist/StockScanner.app"
