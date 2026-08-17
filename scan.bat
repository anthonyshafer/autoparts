@echo off
REM Double-click friendly launcher for a single-ticker scan (Windows).
REM Prompts for a ticker, then runs the scan using the local venv if present.
setlocal
cd /d "%~dp0"

set PY=python
if exist ".venv\Scripts\python.exe" set PY=.venv\Scripts\python.exe

set /p TICKER=Enter ticker (e.g. BSX):
if "%TICKER%"=="" (
  echo No ticker entered. Exiting.
  pause
  exit /b 1
)

set /p AMOUNT=Capital to size against [50000]:
if "%AMOUNT%"=="" set AMOUNT=50000

"%PY%" tools\stocks.py scan %TICKER% --amount %AMOUNT% --timeframe weekly
echo.
pause
