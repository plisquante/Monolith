@echo off
rem Build a native Minecraft executable from a client.jar with the Rust backend.
rem   build.bat C:\path\to\client.jar              (desktop)
rem   build.bat C:\path\to\client.jar --android    (Android arm64)
rem Requirements: Python 3.10+, rustc (stable).
if "%~1"=="" (
  echo usage: build.bat C:\path\to\client.jar [options]   ^(see: python tools\mcbuild_rust.py --help^)
  exit /b 2
)
where rustc >nul 2>nul || (echo rustc not found: install a stable Rust toolchain & exit /b 1)
python "%~dp0tools\mcbuild_rust.py" %*
