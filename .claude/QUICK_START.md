# Quick start
- Test: `export MONO_JDK_HOME=...temurin-25...; python3 tests/run_tests.py [Name...]` (15+ differential tests vs JVM)
- Transpile only: `python3 -m monolith.rustgen --cp ... --main ... -o out --shards 4` (no `--compile`; `MONO_VERBOSE=1` lists link errors, then `python3 tools/check_link_errors.py <stderr file>`)
- Full Minecraft: `./build.sh <client.jar> -o <dir> --opt 1 --shards 16 --shard-jobs 1` (`python3 tools/fetch_client_jar.py --version 26.3 --out client.jar` gets the jar; about an hour, 12 GB)
- Rust compile alone (generated shards, JDK 25): `python3 -m monolith.rustgen --cp <classes> --main <Main> -o <out> --shards 4 --shard-jobs 4 --opt 1 --compile`
- Run: `MC_GRAPHICS=opengl LIBGL_ALWAYS_SOFTWARE=1 xvfb-run -a -s "-screen 0 1280x720x24" ./run.sh`
- Missing natives per build: `<out>/report.json` -> issues.missing-native
