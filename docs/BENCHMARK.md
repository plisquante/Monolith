# Benchmarks

> **Tarihî ölçümler (Zig dönemi).** Aşağıdaki sayılar Zig arka ucu ve `runtime/*.zig` modülleri içindir; bugünkü Rust
> çalışma zamanı ve TouchCore çekirdeği (`android/touchcore/native`) için yeniden ölçülmedi. Güncel yapı süresi ölçümü:
> [`BUILD_PERF.md`](BUILD_PERF.md).

## 1. GC-Free Client Core Subsystems (`-O ReleaseFast`, `aarch64` 10-core Linux)

Measured directly with `python3 tools/microbench/run_client_core_bench.py` (`tools/microbench/client_core_bench.zig`, Zig 0.15.1 `-O ReleaseFast`) on this host (`aarch64`, 10 cores, ~7.4 GB RAM) after replacing STW mark-and-sweep GC and external LWJGL dependencies with the 4-subsystem Client Core:

| Subsystem / Benchmark | Workload | Measured Result | Throughput / Frame Cost |
|---|---|---|---|
| `ArenaAllocator` (`runtime/mem_pool.zig`) | `2,000,000` x `64 B` allocs across `200` frames (`resetRetainingCapacity`) | `23.42 ms` total | **`11.71 ns/op` (`85.4 M ops/s`)**, `0.0 ms` STW pause |
| `SlabPool` 3-epoch + OS unmap (`runtime/mem_pool.zig`) | `64,000` x `64 B` slots (`refillBatch` + `retireDeferred` + `advanceAndReclaim`) | `4.63 ms` total, `4000 KiB` reclaimed | **`72.30 ns/op`**, **`0.0 ms` STW pause** |
| `WorkerPool` (`runtime/thread_pool.zig`) | `250` batches x `64` tasks (`16,000` tasks, 4 workers, `WaitGroup`, batch lock + post-unlock broadcast) | `36.73 ms` total (was `3987.72 ms`) | **`146.9 us` per 64-task batch (108.5x faster)**, `0%` idle spin |
| World Render 3-Stage Cull (`runtime/world_render.zig`) | `256` chunk sections (`16x16x16`), `1920x1080`, FOV `70°`, `200` frames | `127 / 256` visible sections survive | **`40.1 us/frame` (`0.040 ms/frame`)** |
| GPU-Driven Instancing (`runtime/gpu_instancing.zig` + `runtime/rhi.zig`) | `2,048` entities/particles (`64B`/`32B`), `8` meshes, frustum+Hi-Z+pixel cull, opaque/translucent ordered, `200` frames | `1,499 / 2,048` survive in `7` draw batches | **`287.0 us/frame` (`0.287 ms/frame`)** |

### Java -> Zig End-to-End & Differential Suite (`27/27 PASS`, RTA Closure & S24 FE Optimization Pass)

- **RTA Closure & Build Speedup (`aarch64` S24 FE)**:
  - `PatchTarget`: `230` classes / `855` methods (down from `5,902` classes / `31,968` methods), `build 14.7s, native 0.06s`.
  - `JnaProxyFold`: `523` classes / `2,543` methods, `build 21.9s` (down from `624.8s` — **28.5x faster**), `native 0.05s`.
  - `Threads`: `build 28.4s` (down from `527.4s` — **18.6x faster**), `native 0.54s` (down from `100.45s` — **186x faster**).
  - `ClientCoreE2E`: `1,092` classes / `5,932` methods (`0 missing-native, 0 link-error, 0 unsupported-varhandle`), `build 37.9s` (down from `587.1s` — **15.5x faster**), `native 0.15s`.
  - Full 27-test differential suite (`python3 tests/run_tests.py`): **27/27 PASS** (`Arith` 13.4s, `IfaceInit` 12.6s, `Intrinsics` 13.3s, `LazyLeaf` 13.7s, `PatchTarget` 14.7s, `GcStress` 18.6s, `Exceptions` 18.8s, `Jni` 19.3s, `Misc` 21.6s, `Oop` 21.8s, `JnaProxyFold` 21.9s, `MHandles` 24.4s, `Strings` 25.0s, `StackWalk` 25.2s, `ModuleInfo` 26.5s, `Reflect` 26.8s, `Annot` 28.0s, `Threads` 28.4s, `ClientCoreE2E` 37.9s, `IoZip` 37.9s, `OptSuite` 41.9s, `Collections1` 43.3s, `Lambdas` 44.3s, `Platform` 45.7s, `NioFiles` 71.1s, `NetIo` 76.2s, `HttpFetch` 132.4s).

Re-run benchmarks locally:

    python3 tools/microbench/run_client_core_bench.py
    python3 tests/run_client_core_e2e.py -O Debug --shards 16 --shard-jobs 1

For full-client hardware GPU/power comparison via `tools/bench.py` (drop `LIBGL_ALWAYS_SOFTWARE` and `SDL_VIDEO_FORCE_EGL`):

    python3 tools/bench.py --target jvm    --client-jar client.jar --libs LIBDIR --java JAVA25 --mcout OUT --out r/jvm
    python3 tools/bench.py --target native --mcout OUT --out r/native --no-lwjgl --graphics auto

---

## 2. Historical Baseline: Minecraft 26.3 vs Vanilla on HotSpot (Pre-GC-Removal, 4 vCPU Xeon)

Machine: cloud VM, 4 vCPU Intel Xeon @ 2.10 GHz, 15 GiB RAM, **no GPU** (no /dev/dri, no NVIDIA/AMD
device). Rendering: Mesa 25.2.8 llvmpipe (software OpenGL 4.5 core), Xvfb, window 854x480, via
`SDL_VIDEO_FORCE_EGL=1 LIBGL_ALWAYS_SOFTWARE=1`. Both targets started with the same arguments into the
same saved world (`--quickPlaySingleplayer "New World"`) from the same pristine copy of the game dir.
Harness: `tools/bench.py` (samples /proc each second; F3 screenshots for fps/tick). One run per target.

*Note: All rows below were captured **before** removing the stop-the-world GC and **before** introducing the custom Zero-LWJGL RHI, vanilla-safe culling, and GPU-driven instancing.*

Targets: vanilla = `client.jar` + libraries on OpenJDK 25.0.5 (G1, -Xmx4g, JIT). Native = monolith build,
ReleaseFast with the self-hosted Zig backend (`--backend native`); the LLVM ReleaseFast build of the full
game needs more than the 15 GiB of this machine and was killed by the OOM killer, so the optimized
native build could not be measured here.

| metric | vanilla (HotSpot) | native (monolith, self-hosted backend) |
|---|---|---|
| launch to window | 13 s | 215 s |
| launch to integrated server opened world | 10.8 s | 197 s |
| launch to world playable ("Loading terrain" gone) | 21.7 s | 645.6 s |
| CPU seconds spent before the world is playable | ~50 | ~1464 |
| avg CPU after world ready (of one core / of 4 cores) | 239 % / 60 % | 373 % / 93 % |
| RSS avg / peak after start | 2255 / 2297 MiB | 7108 / 7803 MiB |
| threads (max) | 56 | 33 |
| FPS (F3, software GL) | 17 | 1 |
| integrated server tick (F3, budget 50 ms) | 7.6 ms | 315 ms (246-390 across runs) |
| region data saved in the window | 4 files / 12.2 MiB | 1 file / 0.01 MiB |
| GPU utilization / VRAM / power | n/a (no GPU) | n/a (no GPU) |

Caveats: one run each; FPS and tick are single F3 readings (not a time series); the player ended in
different places (vanilla inside terrain, native in tree foliage), so rendering load differs; software
rendering shares the same 4 cores with the game (llvmpipe threads), which hurts both. Chunk-loading speed is
only represented by time-to-playable and by the saved region data; no chunks/s counter was recorded.

## GC share (native, same run conditions)

With `MONO_GC_VERBOSE=1` the native build reports 9 collections before the world is playable, 22.7 s of
total stop-the-world pause, i.e. about 3.5 % of the 657 s start-to-world time. Raising the minimum
collection trigger from 64 MiB to 256 MiB (`MONO_GC_MIN_MB`) did not change time-to-world (646 s before,
657 s after: noise) or RSS (7.1 GiB average). Garbage collection is therefore not the bottleneck; the gap to
HotSpot (21.7 s) is execution speed of the generated code (self-hosted Zig backend, no inlining or register
allocation across calls, shadow-stack frames) plus a larger heap footprint (live data 1.5 GiB at the 9th GC).
Memory breakdown at the title screen: RssAnon 3.8 GiB (heap), RssFile 0.9 GiB (executable pages).

## Sharded LLVM ReleaseFast build (same machine, same conditions)

`--shards 32` splits the program into 32 separately compiled objects, which lets the LLVM ReleaseFast
build fit in this 15 GiB machine (peak 5 GB with 2 shards in parallel; the single-unit build was killed
above 15 GB). Same benchmark, same world:

| metric | vanilla (HotSpot) | native, self-hosted backend | native, 32 shards LLVM ReleaseFast |
|---|---|---|---|
| executable size | (JVM) | 5.6 GB | 1.35 GB |
| launch to window | 13 s | 215 s | 27.5 s |
| launch to world playable | 21.7 s | 645.6 s | 43.0 s |
| FPS (F3, llvmpipe) | 17 | 1 | 4 |
| integrated server tick (budget 50 ms) | 7.6 ms | 315 ms | 49 ms |
| RSS average | 2255 MiB | 7108 MiB | 7395 MiB |
| avg CPU after world ready | 239 % of a core | 373 % | 350 % |
| GC pause until world ready | n/a | 22.7 s | 18.7 s (15 collections) |

Still behind vanilla in start-up time (2x), server tick (6x) and memory (3x live data: 1.8 GiB vs
~0.9 GiB); FPS is dominated by software rendering on this machine and cannot be compared fairly.

## Successive optimizations on the sharded LLVM build (same machine, one run each; noise about +-2 s)

| build | window | world playable | GC pause until playable | RSS avg | F3 server tick (single reading) |
|---|---|---|---|---|---|
| 32 shards, serial GC | 27.5 s | 43.0 s | 18.7 s | 7395 MiB | 49 ms |
| + parallel mark and sweep | 26.8 s | 45.1 s | 6.5 s | 7045 MiB | not read |
| + per-site itable/checkcast caches, dense shadow-stack frames, GC worker backoff | 23.6 s | 42.3 s | 6.4 s | 7274 MiB | 72 ms |

Reading: the parallel GC cut the pause by 3x but did not shorten the start-up: the process is CPU bound
(about 376 % of 4 cores, same as before), so the pause was not on the critical path. The per-site caches
and dense frames gave about 6 % on time to world playable, within what one run can distinguish from noise.
The gap to vanilla (21.7 s to playable, 17 fps, 7.6 ms tick, 2.3 GiB RSS on this machine) is still open.
Sampling with gdb during start-up shows the remaining cost in the runtime hot paths and in the generated
code itself, not in the GC.

## Optimization progression, round 2 (same machine, sharded LLVM ReleaseFast, llvmpipe)

| Build | World ready | RSS avg | GC pause (12 GCs) |
|---|---|---|---|
| lazy-frame leaf methods | 42.3 s | 6603 MiB | 4.5 s |
| + JDK intrinsics (Math/Integer/Long/Float/Double statics inline), SimpleBitStorage patch | 36.5 s | 6660 MiB | 4.0 s |
| + inlined null/bounds-check helpers | 33.5 s | 6832 MiB | 3.8 s |
| + inline small-object zeroing, more intrinsics, class lookup by hash table | 33.3 s | 6843 MiB | 4.5 s |
| vanilla (HotSpot, same machine, earlier run) | 21.7 s | 2255 MiB | n/a |

`MONO_PATCH_VERIFY=1` run of the same build: no mismatch in `SimpleBitStorage.get/cellIndex` (set returns void and is
not self-checked). One run per row; the lazy-frame change alone moved world-ready by 0.7 s (noise).
Profile (gdb sampling, 60 samples, busy threads only) before this round: chunk generation dominates
(NoiseBasedChunkGenerator buildTerrain/doFill, MaterialSystem.buildSurface, BiomeManager) plus small accessors
called through the shard symbol table (51% of all class references cross a shard boundary, so LLVM cannot inline
them). A clustering of classes by reference counts did not reduce that share in an offline simulation and was dropped.

The last row changed nothing measurable (33.5 s to 33.3 s, run-to-run noise is about 1 s). Start-up phases of the last build: session lock 16.1 s (vanilla 10.8 s), window +2.2 s (vanilla +2.2 s), world 15.0 s (vanilla 8.7 s).

## Call-cost microbenchmark (tools/microbench/CallBench.java)

Sharded (3 shards) LLVM ReleaseFast vs HotSpot 25, same machine, one run: recursive `fib(40)`, 4e8 interface calls
(two receiver classes), 8e8 field set plus getter iterations.

| | HotSpot | Monolith | ratio |
|---|---|---|---|
| fib(40) | 401 ms | 1066 ms | 2.7x |
| interface call loop | 309 ms | 1304 ms | 4.2x |
| setter + getter loop | 294 ms | 2000 ms | 6.8x |

The getter loop is already inlined by monolith, yet costs ~2.5 ns per iteration. The disassembly shows the
accumulator and the line counter living in stack memory (a read-modify-write chain per iteration), the
shadow-stack line store, the reference slot copies through `R[]` and the safepoint poll. This is the structural
cost of the generated code shape (one labeled switch per method, reference slots in a memory array, frames) and the
reason a profile of the game is flat: no single method dominates, every method is 2-7x slower than HotSpot's
compiled code. Patching individual kernels cannot close that; changing the code shape can (structured loops with
register-resident locals, reference slots without a shadow stack via conservative stack scanning, cross-shard inlining).

Line-store-on-throw change (`rt.nnF` & co): world ready 33.7 s (a first run right after the build measured 42.1 s, a cold-cache outlier; run-to-run noise is about 1 s otherwise), i.e. no measurable gain in the game although the call-cost microbenchmark improved by about 20% when the line stores were removed by hand.

Round 3 (inline checkcast/itable fast paths, world height patches for Level/ChunkAccess/LevelReader getMinY, Holder.value, Level.dimensionType): world ready 30.8 s (session lock 16.1 s, window 18.3 s), RSS 6959 MiB; `MONO_PATCH_VERIFY=1` run: 0 mismatches. Vanilla: 21.7 s.
