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
| Runtime unit tests | `rustc --edition 2021 --test tests/rust/audio_main.rs` and `tests/rust/crash_main.rs` | all pass (audio 12, crash reporter 5) |
| Native differential tests | `python3 -m monolith.crc_tests`, `netty_tests`, `recycle_tests`, `stack_tests`, `patch_tests` (MONO_JDK_HOME set) | all `ALL PASSED` |
| Link errors explained | `MONO_VERBOSE=1 python3 -m monolith.rustgen ... 2> t.err`, then `python3 tools/check_link_errors.py t.err` (`docs/LINK_ERRORS_TR.md`) | 0 unexplained |
| GC stress, both modes | `tests/run_tests.py GcStress Oop Threads Lambdas Strings Collections1` with `MONO_GC_MB=1 MONO_GC_POISON=1 MONO_ARENA_CHECK=abort`, plain and `MONO_GC_CONCURRENT=1` | same output as HotSpot, no panic, six of six in each mode |
| Game start, plain | `tools/headless_play.sh build --seconds 60` | game running, no crash report, no `Sound engine ERROR`, no `FFI types not initialized` |
| Game start, concurrent GC | `MONO_GC_CONCURRENT=1 tools/headless_play.sh ...` | same as plain (one headless run reached the title and stayed up for 120 s, see `docs/VERIFICATION.md`; repeat runs needed) |
| Leak gate | `tools/leak_soak.sh build --minutes 60`, then `tools/leak_budget.py --warmup-s 1800 --budget-mb-per-min 1` | slope <= 1 MiB/min (one pass with the collector on: slope -0.054 MiB/min, see `docs/VERIFICATION.md`; needs repetition; the collector-off run was stopped at 13.3 GB resident after 10 min, see `docs/VERIFICATION.md`) |

## 3. Package

    python3 tools/make_release.py build/minecraft-26.3 --out dist --name Monolith-Linux-x86_64 --version <x.y.z>

Produces `dist/<name>.tar.gz`, `dist/<name>.sha256` and `dist/<name>.manifest.json` (version, source commit, SHA-256 of
every file). The game's own files (saves, options, logs, crash reports) are never packaged.

## 4. Known blockers (a release is not allowed while these stand)

- Concurrent GC: an earlier build crashed at pre-bootstrap (ClassCastException in DataFixers). In the current build two
  headless runs with `MONO_GC_CONCURRENT=1` loaded a saved world, walked, saved and quit, with no ClassCastException,
  panic or fatal line (`docs/VERIFICATION.md`). Two runs are not enough: the 60 min leak soak is needed before concurrent
  mode can be the default. Until then concurrent mode stays off by default, and the release uses stop-the-world collection.
- The stop-the-world pause reaches about 1.1 s with a 500 MB live heap (measured, plain mode).
- The collector cannot be switched off yet: with `MONO_GC=off` the game reached 13.3 GB resident after 10 min standing
  still on a 16 GB machine (measured, stopped). The profile shows about 19 MB/s of short-lived garbage at idle, not a
  leak (`docs/VERIFICATION.md`). Tiers 1-3 of `GC_PLAN.md` have to shrink allocation first.
- Android audio and window back ends are not tested on a device. The Windows back end exists and its smoke test
  passes on CI runners; the game boots there in CI (`.github/workflows/game.yml`); no hardware run yet.
- Java overlays (`classlib/`, `runtime/java_nolwjgl/`) and the JNA/Netty emulation are still in place.
- Android device testing has not been done.
- The leak gate passed in two runs: the earlier one and 2026-11-24 (+0.180 MiB/min after a 30 minute warm-up, 60 minute soak, plain GC, `docs/STATUS.md` §22). Both are idle-world soaks; walking, other worlds and concurrent GC with the soak are not covered. The same profile has a mean stop-the-world pause of 1.1 s (longest 1.8 s), which stays a blocker for a good player experience.

## 5. Platforms

Only Linux x86_64 is packaged so far. Windows builds are produced by `tools/mcbuild_rust.py` on their own
systems (run.bat, prog.exe); packaging and device tests for them, and Android, are separate releases
(docs/MASTER_PLAN_TR.md, milestones M3 and M4).
