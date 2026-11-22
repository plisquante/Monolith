<p align="center"><img src="docs/logo.png" alt="Monolith" width="720"></p>

# MONOLITH

Monolith is an ahead-of-time **Java bytecode → native compiler** (Rust back end; the older Zig back end is still in the tree) with a zero-STW runtime
(`ArenaAllocator` + 3-epoch `SlabPool`, wake-on-demand `WorkerPool`, custom Zero-LWJGL RHI supporting
Vulkan 1.1+ and OpenGL 3.3 Core, vanilla-safe world culling, and GPU-driven instancing) that turns a
set of JAR files into a native executable which runs **without a JVM**.

It reads `.class` files, analyses the bytecode and emits code
for every reachable method. Nothing is rewritten by hand and nothing is decompiled.

```
python3 -m monolith.rustgen --cp app.jar --main com.example.Main -o out/ --run   # Rust back end (docs/MONOLITH.md)
python3 tools/mcbuild_rust.py --android client.jar                                      # Minecraft -> libminecraft_mono.so (Android arm64)
python3 -m monolith build --cp app.jar:lib.jar --main com.example.Main -o out/   # -> out/Main
python3 tools/mcbuild.py client.jar --no-lwjgl --graphics auto                 # Minecraft client transpile via RHI (experimental — see below)
python3 tests/run_client_core_e2e.py -O Debug --shards 16 --shard-jobs 1       # Unified 4-subsystem Java+Zig E2E
python3 tools/microbench/run_client_core_bench.py                              # ReleaseFast Client Core microbench
python3 tests/run_tests.py                                                     # JVM vs native diff tests
```

Requirements: Python ≥ 3.10; a JDK 21+ with `javac` used as the class-library source
(Temurin 25.0.5 tested — `MONO_JDK_HOME`; on JDK 24+ there are no jmods, the runtime image is
extracted with `jimage` automatically); Zig 0.15.1 (`pip install ziglang==0.15.1`, make sure a
`zig` binary is on PATH). Per-task machine requirements (this repo does not build Minecraft on
a small machine): see [`docs/HARDWARE.md`](docs/HARDWARE.md).

> **`--no-lwjgl` caveat**: the transpiled vanilla client still calls `org.lwjgl.*` in its render path,
> so stripping LWJGL binaries does not by itself produce a playable zero-LWJGL game. The RHI bring-up
> is opt-in (`MONO_RHI=1`, `MONO_RHI_STATS=1`) and the bridge (`mono.rhi.NativeRhi`) is compiled into every
> build automatically. A playable zero-LWJGL client needs a Java-side GL→RHI shim on a machine with the
> client jar + GPU — see `docs/STATUS.md` §15. The launcher passes the backend as
> `-Dmono.graphics.backend=<api>` (safe for the vanilla argument parser).

## Status

* Complete pipeline: class files → RTA → Zig → native executable; verified by differential
  tests that compare the JVM's and the native program's stdout, stderr and exit status, plus
  unified Client Core E2E checks (see [`docs/STATUS.md`](docs/STATUS.md) and [`docs/BENCHMARK.md`](docs/BENCHMARK.md)).
* The design is described in [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).
* Client core subsystems completed and E2E-verified (`0 missing-native`, `0 link-error`, `0 unsupported-varhandle`
  across the JDK 25 client baseline): GC-free `ArenaAllocator` + `SlabPool`, wake-on-demand `WorkerPool`,
  Zero-LWJGL RHI (Vulkan 1.1+ with OpenGL 3.3 Core fallback for legacy integrated GPUs), vanilla-safe
  FOV/Portal/Hi-Z/LOD world culling, and GPU-driven non-block instancing.

## TouchCore For Monolith (Android APK Launcher)

`android/touchcore/` contains a native `arm64-v8a` Android launcher APK (`com.monolith.touchcore`)
powered by MONOLITH Client Core v2 (`mem_pool.zig`, `thread_pool.zig`, `rhi.zig`, `world_render.zig`,
`gpu_instancing.zig`), featuring:

- Player username & skin-avatar profile selector
- MJLauncher-style frosted glass touch HUD with vector icons, connected `+` D-Pad & Analog stick,
  swipeable attack/use look, virtual cursor (`FARE`), soft keyboard (`KB`), center crosshair,
  isometric 9-slot hotbar, and full visual layout editor (`ControlsEditorActivity`)
- Vulkan 1.1+ / OpenGL ES 3.0+ RHI settings, resolution scale, render distance, FPS cap, FOV,
  explicit Demo Mode vs external `libminecraft_mono.so` (`mono_android_entry`) loader without silent fallback
- Engine library for the app (`libminecraft_mono.so`): `python3 tools/mcbuild_rust.py --android <client.jar>` (Rust back end, needs the Android NDK and
  `rustup target add aarch64-linux-android`; the same pipeline is `python3 -m monolith.rustgen --android`).
  The older Zig route: `python3 tools/mcbuild.py build-lib <client.jar> -dynamic --target aarch64-linux-android.26`

Build the APK (output `releases/Monolith.apk`): : `python3 tools/build_touchcore_apk.py`

## Launchers (all platforms)

| Platform | Start | Notes |
|---|---|---|
| Linux | `./launch.sh` | needs python3 >= 3.10 |
| macOS | `./launch.sh` or double-click `launch.command` | sets `DYLD_LIBRARY_PATH` for natives |
| Windows | `launch.bat` | uses `py -3` or `python` |
| Android | `releases/Monolith.apk` (build it with `tools/build_touchcore_apk.py`) | see TouchCore section |

The desktop launcher (`launcher/launcher.py`, stdlib only) opens a local UI in the browser: build, username,
graphics API (auto/vulkan/opengl), game dir. It starts the `run.sh`/`run.bat` that `tools/mcbuild.py` generates in
`build/minecraft-*`. CLI: `./launch.sh --cli --play --username Steve --graphics vulkan`.

## Legal note

This repository contains only the transpiler, its runtime and an original class-library
overlay. It does not contain, and must not be used to distribute, Minecraft binaries or code
derived from them (Minecraft EULA). Generated output stays in local build directories
(`.gitignore` excludes `*.jar` and `out/`).
