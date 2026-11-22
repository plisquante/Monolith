@echo off
rem Launcher for Windows. Opens the Monolith launcher UI (needs Python 3.10+).
where py >nul 2>nul && (py -3 "%~dp0launcher\launcher.py" %* & exit /b)
where python >nul 2>nul || (echo Python 3.10+ not found & exit /b 1)
python "%~dp0launcher\launcher.py" %*
