@echo off
REM ============================================================================
REM  Build StockScanner.exe — a standalone Windows app (no venv for end users).
REM  Run this ONCE on a Windows PC that has Python 3.10+ installed.
REM  Output: dist\StockScanner.exe  (double-clickable, no Python needed to run it)
REM ============================================================================
setlocal
cd /d "%~dp0"

echo [1/3] Installing build tooling and dependencies ...
python -m pip install --upgrade pip pyinstaller >nul 2>&1
python -m pip install -r requirements.txt
if errorlevel 1 (
  echo.
  echo Could not install dependencies. Is Python 3.10+ installed and on PATH?
  echo Get it from https://www.python.org/downloads/ (tick "Add python.exe to PATH").
  pause
  exit /b 1
)

echo [2/3] Building the executable with PyInstaller ...
python -m PyInstaller --noconfirm --clean StockScanner.spec
if errorlevel 1 (
  echo Build failed. See messages above.
  pause
  exit /b 1
)

echo [3/3] Done.
echo.
echo Your app is here:  dist\StockScanner.exe
echo Double-click it to run. You can copy that single .exe to any Windows PC.
pause
