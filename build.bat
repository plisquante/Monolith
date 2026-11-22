@echo off
rem Build a native Minecraft executable from a client.jar.
rem   build.bat C:\path\to\client.jar [--graphics vulkan^|opengl] [-O ReleaseFast] [--out DIR]
rem Requirements: Python 3.10+, zig 0.15.1 (pip install ziglang==0.15.1).
if "%~1"=="" (
  echo usage: build.bat C:\path\to\client.jar [options]   ^(see: python tools\mcbuild.py --help^)
  exit /b 2
)
where zig >nul 2>nul || (echo zig not found: pip install ziglang==0.15.1 & exit /b 1)
python "%~dp0tools\mcbuild.py" %*
