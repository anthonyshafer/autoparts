@echo off
REM One-time setup on Windows: creates a local venv and installs dependencies.
REM Requires Python 3.10+ installed and on PATH (https://www.python.org/downloads/).
setlocal
cd /d "%~dp0"

echo Creating virtual environment in .venv ...
python -m venv .venv
if errorlevel 1 (
  echo.
  echo Could not create venv. Is Python 3.10+ installed and on PATH?
  echo Download it from https://www.python.org/downloads/ and check "Add python.exe to PATH".
  pause
  exit /b 1
)

echo Installing dependencies ...
".venv\Scripts\python.exe" -m pip install --upgrade pip
".venv\Scripts\python.exe" -m pip install -r requirements.txt
if errorlevel 1 (
  echo Dependency install failed. See messages above.
  pause
  exit /b 1
)

echo.
echo Done. You can now double-click scan.bat, or run:
echo   .venv\Scripts\python.exe tools\stocks.py scan BSX
pause
