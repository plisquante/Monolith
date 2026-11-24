# Quick start
- Test: `export MONO_JDK_HOME=...temurin-25...; python3 tests/run_tests.py [Name...]` (15+ differential tests vs JVM)
- Transpile only: `python3 -m monolith build --cp ... --main ... -o out --no-compile`
- Full Minecraft: `./build.sh <client.jar> --out <dir> -O Debug --graphics opengl --skip-assets`
- Rust compile alone (generated shards, JDK 25): `python3 -m monolith.rustgen --cp <classes> --main <Main> -o <out> --shards 4 --shard-jobs 4 --opt 1 --compile`
- Run: `MC_GRAPHICS=opengl LIBGL_ALWAYS_SOFTWARE=1 xvfb-run -a -s "-screen 0 1280x720x24" ./run.sh`
- Missing natives per build: `<out>/report.json` -> issues.missing-native
