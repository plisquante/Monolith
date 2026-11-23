# Release procedure (Monolith)

A release is a tested build of one Minecraft version (26.3) with its gates passed and its evidence recorded. Nothing
is released from a build that did not pass every gate below.

## 1. Build

    python3 tools/mcbuild_rust.py <client.jar> -o build/minecraft-26.3     # desktop, Linux x86_64

Needs rustc, the JDK 25 (`MONO_JDK_HOME` or downloaded by the tool), about 12 GB of RAM for 3 shards, and about 65 minutes.
Use `--shard-jobs 1` on a machine with less memory. Never reuse objects after a runtime body change (`--reuse-objs`
is only for additive changes; see docs/IMPL_SPEC.md section 7).

## 2. Gates (all must pass)

| gate | how | pass |
|---|---|---|
| Lint | `pyflakes monolith/*.py tools/*.py tests/*.py` | no output |
| Runtime unit tests | `rustc --test tests/rust/audio_main.rs` | all pass |
| Native differential tests | `python3 -m monolith.crc_tests`, `netty_tests`, `recycle_tests`, `stack_tests`, `patch_tests` (MONO_JDK_HOME set) | all `ALL PASSED` |
| GC stress, both modes | `tests/java/GcConcurrentStress` through the runtime, plain and `MONO_GC_CONCURRENT=1` | same total as HotSpot, no panic, six of six |
| Game start, plain | `tools/headless_play.sh build --seconds 60` | game running, no crash report, no `Sound engine ERROR`, no `FFI types not initialized` |
| Game start, concurrent GC | `MONO_GC_CONCURRENT=1 tools/headless_play.sh ...` | same as plain (currently FAILS, see section 4) |
| Leak gate | `tools/leak_soak.sh build --minutes 60`, then `tools/leak_budget.py --warmup-s 1800 --budget-mb-per-min 1` | slope <= 1 MiB/min (one pass so far, needs repetition) |

## 3. Package

    python3 tools/make_release.py build/minecraft-26.3 --out dist --name Monolith-Linux-x86_64 --version <x.y.z>

Produces `dist/<name>.tar.gz`, `dist/<name>.sha256` and `dist/<name>.manifest.json` (version, source commit, SHA-256 of
every file). The game's own files (saves, options, logs, crash reports) are never packaged.

## 4. Known blockers (a release is not allowed while these stand)

- Concurrent GC crashes the game at pre-bootstrap (ClassCastException in DataFixers). Barriers were added for arraycopy,
  Unsafe, VarHandle, reflection and patches; the crash remains in the last measured build. Until it is fixed the
  concurrent mode stays off by default, and the release uses stop-the-world collection.
- The stop-the-world pause reaches about 1.1 s with a 500 MB live heap (measured, plain mode).
- Android audio and window back ends are not tested on a device. Windows and macOS back ends exist and their smoke test
  passes on CI runners; the game boots there in CI (`.github/workflows/game.yml`); no hardware run yet.
- Java overlays (`classlib/`, `runtime/java_nolwjgl/`) and the JNA/Netty emulation are still in place.
- Android device testing has not been done.
- The leak gate passed in one run only.

## 5. Platforms

Only Linux x86_64 is packaged so far. Windows and macOS builds are produced by `tools/mcbuild_rust.py` on their own
systems (run.bat, prog.exe); packaging and device tests for them, and Android, are separate releases
(docs/MASTER_PLAN_TR.md, milestones M3 and M4).
