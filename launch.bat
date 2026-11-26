@echo off
rem Launcher for Windows. Opens the Monolith launcher UI (uv manages Python and the environment; no venv needed).
where uv >nul 2>nul && (uv run --project "%~dp0." "%~dp0launcher\launcher.py" %* & exit /b)
rem uv yoksa eski yol: Python 3.10+
echo warning: uv not found, using the system Python. Install uv: powershell -ExecutionPolicy ByPass -c "irm https://astral.sh/uv/install.ps1 | iex" 1>&2
where py >nul 2>nul && (py -3 "%~dp0launcher\launcher.py" %* & exit /b)
where python >nul 2>nul || (echo Python 3.10+ not found & exit /b 1)
python "%~dp0launcher\launcher.py" %*
