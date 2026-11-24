<p align="center"><img src="docs/logo.png" alt="Monolith" width="720"></p>

# MONOLITH

Monolith turns Java bytecode into a native program that runs **without a JVM**. It reads the class files of an
application and of the libraries it uses, analyses which code is reachable, translates that code to Rust, and builds
an executable. Nothing is decompiled and nothing is written by hand; the translation is done from the bytecode.

The first target is Minecraft Java Edition 26.3 (client).

## Status (honest)

| Area | State |
|---|---|
| Translation (bytecode to Rust), class and method coverage | works: 6 196 classes of the client closure, 0 untranslatable |
| Desktop game on Linux x86_64 (menu, world, input, sound) | runs in plain mode; headless tests pass. Headless run (Xvfb, software GL, 26.3 client): title, menu, create and load a world, walking, save and quit, and reload all pass in a scripted run, see [`docs/VERIFICATION.md`](docs/VERIFICATION.md). Multiplayer, Realms and long play are not verified |
| Known link gaps | 446 link-errors at translation time (CI log, 26.3 client). All are in optional dependency paths the game does not use on its boot path: log4j appenders (ZeroMQ, JMS, mail, Jackson, CSV, zstd), OSGi and netty logging, and the `ca.weblite` bridge of Minecraft's macOS-only code. Calls into them throw at run time. macOS is not a supported target. |
| Garbage collector | stop-the-world mark and sweep (pauses up to about 1 s at 500 MB live); concurrent mode (`MONO_GC_CONCURRENT=1`, off by default) passes the 120 s Linux boot after the SATB fixes: unbarriered reference overwrites (Unsafe put/CAS, VarHandle store/CAS, `recycle`) now log the old value. `MONO_GC_POISON=1` fills freed slots with `0xDD` for use-after-free checks |
| Sound | Linux ALSA (`libasound`, loaded at run time), Windows WASAPI; silent fallback when no device |
| Windows | window (Win32+WGL), input and sound back ends are written; CI runs the back-end smoke test and a full game boot (`.github/workflows/game.yml`). Not yet tested on real hardware |
| Android | a launcher APK exists; the native engine has not been tested on a device |
| Java class library overlays and JNA/Netty emulation | still in use; being replaced. Today: 243 Java files in `runtime/java_nolwjgl` and 56 in `classlib` ([`docs/WS7_ENVANTER_TR.md`](docs/WS7_ENVANTER_TR.md)), 259 JNI/JNA emulation natives ([`docs/JNI_ENVANTER_TR.md`](docs/JNI_ENVANTER_TR.md)); `MONO_COUNT_NATIVES=1` counts their calls at run time (no measurement of a real game yet) |
| Continuous integration | the GitHub Actions runs of this repository end in 3 to 4 seconds with a failure before any step runs, also on commits from before 2026-11-24 (no job log exists); the job exists but has no log, which points at the account or the runner (quota, billing, runner start) rather than at the workflow files, but that is not confirmed. No CI result can be relied on until it is fixed. `tools/release_check.py` runs the gates locally |

The project is not a finished product. Read [`docs/RELEASE.md`](docs/RELEASE.md) for what must pass before a release,
and [`docs/IMPL_SPEC.md`](docs/IMPL_SPEC.md) for the decisions and measurements.

## Quick start (Linux x86_64)

Requirements: Python 3.10 or newer, a JDK 25 (`MONO_JDK_HOME` points to it), rustc (stable), `jq` is not needed,
about 12 GB of RAM for the three-shard build, and about an hour for the build.

    python3 tools/fetch_client_jar.py --version 26.3 --out client.jar     # from Mojang's manifest, SHA-1 checked
    python3 tools/mcbuild_rust.py client.jar -o build/minecraft-26.3
    build/minecraft-26.3/run.sh      (Windows: build\minecraft-26.3\run.bat)

To run the same steps as the CI game job on your own machine (POSIX shells), with a log per step:

    sh tools/ci_local.sh --out build/ci-local        # fetch, build, 120 s boot, concurrent-GC boot on Linux

The first start needs a world: create one in Singleplayer. Options for the build: `--opt`, `--shards`, `--shard-jobs`, `--escape` and `--inline` (opt-in; the flags raised idle allocation in a measured run, see `docs/VERIFICATION.md`)
(use 1 on a machine with little memory). On a 4-core machine, `--shards 4 --shard-jobs 4` built a small test program
about 3.1x faster than one shard (measured, see `docs/BUILD_PERF.md`); the Minecraft defaults were not measured yet.

## Tests

    rustc --edition 2021 --test tests/rust/audio_main.rs -o /tmp/audio_tests && /tmp/audio_tests
    rustc --edition 2021 --test tests/rust/crash_main.rs -o /tmp/crash_tests && /tmp/crash_tests    # crash reporter
    python3 -m monolith.crc_tests          # and netty_tests, recycle_tests, stack_tests, patch_tests, patch_fp_tests (set MONO_JDK_HOME)
    python3 -m pyflakes monolith/*.py tools/*.py tests/*.py
    python3 tests/run_tests.py [Name ...]  # differential tests in tests/java (HotSpot against the translated program)
    python3 tools/release_check.py         # the gates of docs/RELEASE.md in one run

The differential tests compare the output of the translated program with HotSpot. The link errors of a transpile are
checked against the explained groups by `tools/check_link_errors.py` (`docs/LINK_ERRORS_TR.md`); the bytecode of every
patched method is pinned by a fingerprint (`tools/patch_fingerprints.py`, `patches/README.md`).

A crash (Rust panic, or SIGSEGV/SIGBUS/SIGILL/SIGFPE/SIGABRT on Linux) writes `mono-crash-<pid>.txt` into the working
directory, or into `MONO_CRASH_DIR`, with the thread, the message or fault address and a native backtrace.

## Repository layout

- `monolith/` — the translator (`rustgen.py`, `inline.py`, `escape.py`, `patches.py`, ...) and the Rust runtime (`monolith/rt/`).
- `patches/rust/` — Rust replacements for selected methods (DataFixer, tickChunk, bit storage, world height).
- `classlib/`, `runtime/` — Java overlays of parts of the class library (being replaced).
- `tools/` — build, packaging (`make_release.py`), measurement (`alloc_report.py`, `leak_soak.sh`, `leak_budget.py`).
- `docs/` — plans, specification, measurements, release procedure.

## Android (TouchCore launcher)

`android/touchcore/` is a native `arm64-v8a` launcher APK (`com.monolith.touchcore`) with a touch HUD and a demo
mode. It loads an engine library (`libminecraft_mono.so`) built with `python3 tools/mcbuild_rust.py --android <client.jar>`
(needs the Android NDK and `rustup target add aarch64-linux-android`). The APK is built with
`python3 tools/build_touchcore_apk.py`; that build passes with NDK 26.3 and the native core's host tests pass
(`docs/VERIFICATION.md`). The engine has not been run on a device yet.

## Launchers

| Platform | Start | Notes |
|---|---|---|
| Linux | `./launch.sh` | needs python3 >= 3.10 |
| Windows | `run.bat` in the build directory | back end written, verified by CI only |

The desktop launcher (`launcher/launcher.py`, standard library only) opens a local page in the browser; it starts the
`run.sh` that the build wrote. CLI: `./launch.sh --cli --play --username Steve`.

## Legal note

This repository contains only the translator, its runtime and original overlays of the class library. It does not
contain Minecraft binaries or code derived from them, and must not be used to distribute them (Minecraft EULA). Build
output stays in local directories; `.gitignore` excludes `*.jar`, `build/` and `out/`.
