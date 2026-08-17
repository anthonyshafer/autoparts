@echo off
REM Launch the StockScanner GUI from source (Windows, dev use).
REM Uses the local venv if present; falls back to system Python.
setlocal
cd /d "%~dp0"
set PY=python
if exist ".venv\Scripts\python.exe" set PY=.venv\Scripts\python.exe
"%PY%" tools\gui.py
if errorlevel 1 pause
