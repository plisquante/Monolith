# Quick start
- Test: `export J2Z_JDK_HOME=...temurin-25...; python3 tests/run_tests.py [Name...]` (15+ differential tests vs JVM)
- Transpile only: `python3 -m j2zig build --cp ... --main ... -o out --no-compile`
- Full Minecraft: `./build.sh <client.jar> --out <dir> -O Debug --graphics opengl --skip-assets`
- Compile alone: `cd <out> && zig build-exe src/main.zig -ODebug -femit-bin=minecraft --name minecraft -lc -lz`
- Run: `MC_GRAPHICS=opengl LIBGL_ALWAYS_SOFTWARE=1 xvfb-run -a -s "-screen 0 1280x720x24" ./run.sh`
- Missing natives per build: `<out>/report.json` -> issues.missing-native
