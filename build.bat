@echo off
rem Build a native Minecraft executable from a client.jar with the Rust backend.
rem   build.bat C:\path\to\client.jar              (desktop)
rem   build.bat C:\path\to\client.jar --android    (Android arm64)
rem Requirements: uv (manages Python and the environment itself; no venv needed), rustc (stable).
if "%~1"=="" (
  echo usage: build.bat C:\path\to\client.jar [options]   ^(see: build.bat --help^)
  exit /b 2
)
where rustc >nul 2>nul || (echo rustc not found: install a stable Rust toolchain & exit /b 1)
rem uv varsa onunla calistir (Python'u ve ortami kendisi yonetir)
where uv >nul 2>nul && (uv run --project "%~dp0." "%~dp0tools\mcbuild_rust.py" %* & exit /b)
rem uv yoksa eski yol: sistemdeki Python 3.10+
echo warning: uv not found, using the system Python. Install uv: powershell -ExecutionPolicy ByPass -c "irm https://astral.sh/uv/install.ps1 | iex" 1>&2
python "%~dp0tools\mcbuild_rust.py" %*
