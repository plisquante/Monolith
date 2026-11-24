---
name: monolith-build
description: Build, test, and run monolith. Use when running tests, transpiling, compiling Zig, building Minecraft, or the Android TouchCore launcher.
---

# monolith-build

## Environment
- JDK 25 required: `export MONO_JDK_HOME=$HOME/.cache/monolith/temurin-25/jdk-25.0.5+8`. JAVA_HOME may be 21 and silently give wrong output.
- Memory: Zig compile of Minecraft needs ~10 GB. Never run it with the test suite. One heavy job at a time (`.claude/hooks/guard.py` enforces it).
- Low RAM floors: `MONO_MIN_AVAIL_GIB`, `MONO_TEST_MIN_AVAIL_GIB`. Per-task needs: `docs/HARDWARE.md`.

## Commands
- Tests (differential vs JVM): `python3 tests/run_tests.py [Name...]`
- Transpile only: `python3 -m monolith build --cp ... --main ... -o out --no-compile`
- Full Minecraft: `./build.sh <client.jar> --out <dir> -O Debug --graphics opengl --skip-assets`
- Rust compile alone: `python3 -m monolith.rustgen --cp <classes> --main <Main> -o <out> --shards 4 --shard-jobs 4 --opt 1 --compile`
- Run: `MC_GRAPHICS=opengl LIBGL_ALWAYS_SOFTWARE=1 xvfb-run -a -s "-screen 0 1280x720x24" ./run.sh`
- Missing natives: `<out>/report.json` key `issues.missing-native`
- Android launcher APK: `tools/build_touchcore_apk.py`; sources `android/touchcore`; CI `.github/workflows/build-so.yml`.

## Gotchas
- Field mangling: `_` -> `_1`, `$` -> `_2` (`monolith/world.py:mangle`).
- Natives: `n_<class>__<method>` in `monolith/rt/natives*.rs`; `_` in method names becomes `_1`.
- Generated `<out>/src` is rebuilt every transpile. Fix the generator.
- Default graphics is vulkan. Under Xvfb use `--graphics opengl` plus `LIBGL_ALWAYS_SOFTWARE=1`.
- Complex `rm` one-liners get blocked. Edit via Python.
- Never commit jar, `mc/`, `out/`, `build/`, `tests/build`.
- Behavioral equivalence vs HotSpot first, optimize after.

## Android launcher APK (verified setup)
- Needs the Android NDK (tested: 26.3.11579264), `rustup target add aarch64-linux-android`, Android SDK (platforms;android-34, build-tools;34.0.0), JDK 25. Unset `_JAVA_OPTIONS` and `JAVA_TOOL_OPTIONS`: they print to stderr and break differential tests.
- D8 8.2.2 from build-tools 34 crashes on JDK 25 class files. Put R8 9.5.33 (maven.google.com com/android/tools/r8) at `~/.cache/monolith/android-sdk/r8.jar` and `android.jar` next to it; add build-tools to PATH for aapt, zipalign, apksigner.
- `ANDROID_NDK_HOME=<ndk> python3 tools/build_touchcore_apk.py --out <apk>`; check with `apksigner verify`.
