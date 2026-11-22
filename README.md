<p align="center"><img src="docs/logo.png" alt="Minecraft Zig Edition" width="720"></p>

# J2ZIG

`j2zig` is an ahead-of-time **Java bytecode → Zig transpiler** with a zero-STW runtime
(`ArenaAllocator` + 3-epoch `SlabPool`, wake-on-demand `WorkerPool`, custom Zero-LWJGL RHI supporting
Vulkan 1.1+ and OpenGL 3.3 Core, vanilla-safe world culling, and GPU-driven instancing) that turns a
set of JAR files into a native executable which runs **without a JVM**.

It is a real transpiler: it reads `.class` files, analyses the bytecode and emits Zig
for every reachable method. Nothing is rewritten by hand and nothing is decompiled.

```
python3 -m j2zig build --cp app.jar:lib.jar --main com.example.Main -o out/   # -> out/Main
python3 tools/mcbuild.py client.jar --no-lwjgl --graphics auto                 # Minecraft client transpile via RHI (experimental — see below)
python3 tests/run_client_core_e2e.py -O Debug --shards 16 --shard-jobs 1       # Unified 4-subsystem Java+Zig E2E
python3 tools/microbench/run_client_core_bench.py                              # ReleaseFast Client Core microbench
python3 tests/run_tests.py                                                     # JVM vs native diff tests
```

Requirements: Python ≥ 3.10; a JDK 21+ with `javac` used as the class-library source
(Temurin 25.0.5 tested — `J2Z_JDK_HOME`; on JDK 24+ there are no jmods, the runtime image is
extracted with `jimage` automatically); Zig 0.15.1 (`pip install ziglang==0.15.1`, make sure a
`zig` binary is on PATH). Per-task machine requirements (this repo does not build Minecraft on
a small machine): see [`docs/HARDWARE.md`](docs/HARDWARE.md).

> **`--no-lwjgl` caveat**: the transpiled vanilla client still calls `org.lwjgl.*` in its render path,
> so stripping LWJGL binaries does not by itself produce a playable zero-LWJGL game. The RHI bring-up
> is opt-in (`J2Z_RHI=1`, `J2Z_RHI_STATS=1`) and the bridge (`j2z.rhi.NativeRhi`) is compiled into every
> build automatically. A playable zero-LWJGL client needs a Java-side GL→RHI shim on a machine with the
> client jar + GPU — see `docs/STATUS.md` §15. The launcher passes the backend as
> `-Dj2z.graphics.backend=<api>` (safe for the vanilla argument parser).

## Status

* Complete pipeline: class files → RTA → Zig → native executable; verified by differential
  tests that compare the JVM's and the native program's stdout, stderr and exit status, plus
  unified Client Core E2E checks (see [`docs/STATUS.md`](docs/STATUS.md) and [`docs/BENCHMARK.md`](docs/BENCHMARK.md)).
* The design is described in [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).
* Client core subsystems completed and E2E-verified (`0 missing-native`, `0 link-error`, `0 unsupported-varhandle`
  across the JDK 25 client baseline): GC-free `ArenaAllocator` + `SlabPool`, wake-on-demand `WorkerPool`,
  Zero-LWJGL RHI (Vulkan 1.1+ with OpenGL 3.3 Core fallback for legacy integrated GPUs), vanilla-safe
  FOV/Portal/Hi-Z/LOD world culling, and GPU-driven non-block instancing.

## TouchCore For Minecraft Zig Edition (Android APK Launcher)

`android/touchcore/` contains a native `arm64-v8a` Android launcher APK (`com.j2zig.touchcore`)
powered by J2ZIG Client Core v2 (`mem_pool.zig`, `thread_pool.zig`, `rhi.zig`, `world_render.zig`,
`gpu_instancing.zig`), featuring:

- Player username & skin-avatar profile selector
- MJLauncher-style frosted glass touch HUD with vector icons, connected `+` D-Pad & Analog stick,
  swipeable attack/use look, virtual cursor (`FARE`), soft keyboard (`KB`), center crosshair,
  isometric 9-slot hotbar, and full visual layout editor (`ControlsEditorActivity`)
- Vulkan 1.1+ / OpenGL ES 3.0+ RHI settings, resolution scale, render distance, FPS cap, FOV,
  explicit Demo Mode vs external `libminecraft_j2z.so` (`j2z_android_entry`) loader without silent fallback
- Shared-library Android build pipeline: `python3 tools/mcbuild.py build-lib <client.jar> -dynamic --target aarch64-linux-android.26`
  and `python3 -m j2zig build-lib -dynamic --no-lwjgl --target aarch64-linux-android.26 ...`

Prebuilt signed APK: [`releases/TouchCore-For-Minecraft-Zig-Edition.apk`](releases/TouchCore-For-Minecraft-Zig-Edition.apk)  
Reproducible CLI build script: `python3 tools/build_touchcore_apk.py`

## Launchers (all platforms)

| Platform | Start | Notes |
|---|---|---|
| Linux | `./launch.sh` | needs python3 >= 3.10 |
| macOS | `./launch.sh` or double-click `launch.command` | sets `DYLD_LIBRARY_PATH` for natives |
| Windows | `launch.bat` | uses `py -3` or `python` |
| Android | `releases/TouchCore-For-Minecraft-Zig-Edition.apk` | see TouchCore section |

The desktop launcher (`launcher/launcher.py`, stdlib only) opens a local UI in the browser: build, username,
graphics API (auto/vulkan/opengl), game dir. It starts the `run.sh`/`run.bat` that `tools/mcbuild.py` generates in
`build/minecraft-*`. CLI: `./launch.sh --cli --play --username Steve --graphics vulkan`.

## Legal note

This repository contains only the transpiler, its runtime and an original class-library
overlay. It does not contain, and must not be used to distribute, Minecraft binaries or code
derived from them (Minecraft EULA). Generated output stays in local build directories
(`.gitignore` excludes `*.jar` and `out/`).
