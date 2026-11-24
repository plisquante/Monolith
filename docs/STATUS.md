# Status

Last updated: 2026-11-15. Toolchain: rustc (stable), OpenJDK 25 (Temurin 25.0.5) class library (JDK 21 also supported).

> **Kronolojik kayıt.** Bu dosyadaki "Zig" ifadeleri, Zig arka ucunun kaldırılmasından önceki durumu anlatır (arka uç
> kaldırıldı; güncel durum `README.md` ve `.claude/ARCHITECTURE_MAP.md`'de). Güncel plan: `MASTER_PLAN_TR.md`.

## 1. Differential test results

`python3 tests/run_tests.py` compiles each `tests/java/*.java` with javac, runs it on the JVM
and as a monolith native executable, and requires identical stdout, first stderr line and exit
code. Last run: **10/10 passed against the JDK 25 class library** (Debug builds, i.e. with Zig safety checks enabled).

| test | covers |
|---|---|
| `Arith` | int/long/float/double arithmetic edge cases, conversions, NaN/±0/∞, 2 000 random doubles/floats through `toString`/`parseDouble`, `StrictMath` (fdlibm), `Integer`/`Long` bit operations, `Math.*Exact` |
| `Strings` | concat (indy), `StringBuilder`, search/replace/split/regex, case mapping incl. Unicode, `intern`, string `switch`, text blocks, charsets, patterns |
| `Oop` | inheritance, abstract/default/static interface methods, `super` calls, field hiding, inner/local/anonymous classes, generics + bridges, records, enums with bodies, `EnumMap/EnumSet`, sealed types, pattern `switch` (`typeSwitch`), init order |
| `Exceptions` | try/catch/finally semantics, runtime exceptions with HotSpot messages, try-with-resources + suppressed, `ExceptionInInitializerError` → `NoClassDefFoundError`, deep recursion, `StackOverflowError`, stack traces with line numbers, uncaught-exception handlers |
| `Lambdas` | all lambda/method-reference shapes (static, bound, unbound, constructor, array constructor), captures, boxing/widening adaptation, serializable lambdas (`altMetafactory`), `Comparator` combinators, `Optional` |
| `Collections1` | `ArrayList/LinkedList/HashMap/LinkedHashMap/TreeMap/HashSet/TreeSet/ArrayDeque/PriorityQueue/BitSet`, `Arrays`, `Collections`, streams and collectors, `Random` |
| `Threads` | `synchronized`, `wait/notify`, atomics, `ReentrantLock`/`Condition` (AQS via Unsafe + park/unpark), `ConcurrentHashMap`, `ExecutorService`, `CountDownLatch`, `CompletableFuture`, interrupts, `volatile`, `ThreadLocal`, daemon / non-daemon shutdown |
| `GcStress` | allocation churn on 1 and 4 threads (stop-the-world GC with safepoints), large arrays, `HashMap` growth, weak references |
| `Misc` | `tableswitch/lookupswitch`, labeled loops, multi-dim arrays, `arraycopy` overlap, varargs, overloads, boxing caches, `String.format`, `System.exit` status |
| `IoZip` | `FileInputStream/FileOutputStream/RandomAccessFile/File`, readers/writers, `CRC32/Adler32`, `Deflater/Inflater`, `GZIP*Stream`, `Zip*Stream` |

## 2. Minecraft Java Edition 26.3 (client) analysis

Inputs: `client.jar` (11 383 classes, class file version 69 / Java 25) + 66 Linux libraries.
Nothing derived from these files is stored in this repository.

| metric | value |
|---|---|
| classes in client.jar | 11 383 (1 793 interfaces), 98 739 methods, 1.91 M instructions |
| `jsr/ret` (unsupported) | 0 |
| invokedynamic sites | 24 014 lambda, 5 941 record `ObjectMethods`, 2 201 string concat, 79 pattern switch — all handled by `indy.py` |
| reflection call sites (client.jar) | 30 |
| libraries | 31 789 classes, 3 903 native methods (LWJGL OpenGL alone: 2 278) |
| **RTA closure from `net.minecraft.client.main.Main`** | **53 996 classes loaded, 38 885 instantiated, 240 868 reachable methods, 3.93 M instructions** (50 s) |
| Zig generated for the whole closure | 53 977 modules, 542 MB of Zig, **0 code generation errors** (108 s) |
| reachable native methods | 781, of which 682 not yet implemented |
| link errors / missing classes | 137 / 20 — mostly JDK 22–25 APIs (`java.lang.classfile`, `Math.powExact`, …) because the build used JDK 21's `java.base`; Minecraft 26.3 requires Java 25 |
| unsupported `MethodHandle.invoke*` sites | 36 |

Missing native methods by area: `org/lwjgl/opengl` 115, `sun/nio/ch` 95, `sun/nio/fs` 59,
`com/sun/jna` 56, `org/lwjgl/system/libffi` 39, `java/io` 38 (implemented since), netty
epoll/kqueue/unix 70, `com/mojang/jtracy` 20, `java/util/zip` 17 (implemented since), …

## 3. What is needed before Minecraft can start natively

1. **JDK 25 `java.base`** as class-library input (download a JDK 25 jmod; the pipeline is version agnostic).
2. **JNI bridge for third-party native libraries** (LWJGL, JNA, netty-epoll, jtracy, lz4):
   generate `extern` declarations for `Java_<class>_<method>` symbols and link the libraries
   shipped in the `natives-linux` jars, with a `JNIEnv` function table implemented in Zig
   (LWJGL mostly passes primitives/addresses; `GetDirectBufferAddress`, string and callback
   functions are the main JNIEnv entry points it uses).
3. **NIO natives** (`sun.nio.ch` file channels / selectors, `sun.nio.fs` UnixNativeDispatcher):
   Minecraft reads its resources and worlds through `java.nio.file`.
4. **Reflection metadata** for the classes reached through `Class.forName`/`newInstance`
   (Gson type adapters, `ServiceLoader`, log4j plugins) — a build-time configuration plus
   generated constructor/field accessors.
5. **MethodHandles** used by libraries (36 sites): direct-method-handle objects backed by
   generated adapter functions.
6. **Build scaling**: Zig compiles the program as one LLVM module; 240 k methods require
   splitting the program into separately compiled units (C-ABI boundaries between packages)
   to keep optimized builds tractable. Debug builds use Zig's fast self-hosted backend.
7. Resource access: the class loader overlay serves `getResourceAsStream` from an extracted
   resource directory (`MONO_RESOURCES`); the assets must be extracted at build time.

## 4. Coverage: transpiling every method (not only reachable ones)

`python3 tests/transpile_all.py <archives>` runs the front end and the Zig generator over
every method of every class:

| input | classes | methods | result |
|---|---|---|---|
| JDK 21 `java.base.jmod` | 7 555 | 62 235 | 62 235 ok, 0 failures |
| Minecraft 26.3 `client.jar` + 66 libraries | 43 034 | 546 163 | 546 163 ok, 0 failures (138 classes unloadable: optional dependencies such as Kafka/OSGi/JFR missing from the class path) |

## 5. Performance (first measurements, optimization phase started)

`GcStress` (≈44 M short-lived objects on 1 + 4 threads, 300 k-entry `HashMap`, weak refs),
ReleaseFast, 4 cores:

| | time | peak RSS |
|---|---|---|
| HotSpot (OpenJDK 21, default G1) | 1.0 s | 640–830 MB |
| monolith, system allocator per object | 8.6–10.6 s | 710 MB |
| monolith, segregated-fit allocator | 5.9–6.9 s | 238 MB |

Applied: segregated-fit thread-local allocator, CHA devirtualization, elided class-init
checks, in-place sweep, `volatile`-only atomics. Evaluated next steps, in expected order of
impact:

1. **Shadow-stack cost**: every call pushes a frame (TLS access, stack-limit check, safepoint
   poll) and zero-initializes the `R` reference array; references live in memory instead of
   registers. Replacing the shadow stack with conservative stack scanning over the
   segregated-fit heap (object-start lookup by block/size class) lets LLVM keep references in
   registers and removes the per-call bookkeeping. Leaf methods that cannot allocate, call or
   throw need no frame at all.
2. **Generational / bump allocation** for the nursery (most objects die young).
3. **Inline caches** for megamorphic-looking interface calls; itable lookups are linear.
4. **SIMD**: `Arrays.fill/equals/mismatch`, `String` scans and `System.arraycopy` are natural
   `@Vector` candidates (HotSpot uses intrinsics there).
5. **Parallel marking** on the stopped world (threads are already registered/parked).
6. **Build scaling**: split the program into separately compiled units so LLVM can optimize
   the 240 k-method Minecraft closure in parallel.

## 6. Known semantic differences (documented, intentional)

* Locale data: only Locale.US / ROOT symbols (`DecimalFormatSymbols` overlay); CLDR data is not embedded.
* `NullPointerException` messages are not "helpful" (HotSpot computes them from bytecode at run time).
* `Object.hashCode()` identity hashes differ from HotSpot's (unspecified by the JLS).
* Finalizers (`Object.finalize`) never run; soft references are cleared like weak references.
* `System.gc()` is a no-op unless the program sets `-Dmono.explicit.gc` (Minecraft calls it every tick; a collection
  stops the game for about half a second). With the property, it runs a complete collection before it returns, and
  WeakReferences are cleared as in HotSpot (Rust backend, both GC modes; `tests/java/GcStress.java` sets the property).
  The Zig backend's `GcStress` still keeps one WeakReference referent alive: its conservative stack scan finds a stale
  reference to it. Open.
* `Class.getDeclared*` reflection, dynamic class definition, hidden classes, method handles beyond
  VarHandles and the desugared invokedynamic bootstraps are not available.

## 7. Minecraft 26.3 native run (milestone)

The natively compiled client (Debug build, JDK 25 closure, no JVM) starts, creates an OpenGL
context through LWJGL/SDL3, loads resources and renders the title/first-run screen under
Xvfb with Mesa llvmpipe. Reproduce headless:

    Xvfb :97 -screen 0 1280x720x24+32 +extension GLX +render &
    SDL_VIDEO_FORCE_EGL=1 DISPLAY=:97 LIBGL_ALWAYS_SOFTWARE=1 MC_GRAPHICS=opengl ./run.sh

`SDL_VIDEO_FORCE_EGL=1` (needs libegl1/libegl-mesa0) is only required on Xvfb: Minecraft asks for an
sRGB framebuffer, which Xvfb's GLX does not offer; real X servers and Wayland do not need it.
Expected log noise without network/audio: Mojang services unavailable, OpenAL device, narrator (flite).
Vulkan backend is selected only where the driver exposes VK_KHR_surface.

## 8. Build modes and measured numbers (Minecraft 26.3 closure, 61 k Zig files, 904 MB of source)

| build | zig time | peak compiler RSS | title screen (Xvfb, llvmpipe) |
|---|---|---|---|
| Debug (self-hosted backend, -fstrip) | ~3 min | ~10 GB | ~5 min |
| ReleaseFast, self-hosted backend (`--backend native`) | ~3.3 min | ~7 GB | ~3.4 min |
| ReleaseFast, LLVM | killed by the OOM killer at >15 GB after 4.3 min | >15 GB | n/a |
| ReleaseSmall, LLVM | killed by the OOM killer at ~13 GB after 4.2 min | >13 GB | n/a |

LLVM optimizes the whole program as one module; a 16 GB machine cannot hold it. `--backend auto`
(default) therefore picks the LLVM build only when the machine has enough RAM (about 4 GiB per
10 000 generated files times 1.6) and falls back to the self-hosted backend, also when the LLVM
build is killed. Small programs (tests) always use LLVM: `GcStress` ReleaseFast runs in 2.8 s
(HotSpot 0.94 s, previous monolith baseline 6.4 s).

Optimizations applied this round: leaf methods (no throw/alloc/call/loop) skip the shadow-stack
frame; allocation counter batched per thread; tiny runtime helpers are force-inlined only on the
self-hosted backend (forcing them under LLVM was 3.5x slower).

Why not shard into separately compiled objects: Zig functions returning error unions cannot be
exported across objects (`export` requires a C calling convention), and virtual calls go through
vtables of Zig-ABI function pointers. Sharding needs a C-compatible method ABI (exceptions as a
thread-local flag instead of error unions) and weak-linked runtime globals; that is a codegen
redesign, not a build-script change.

## 9. Gameplay milestone

The natively compiled Minecraft 26.3 client (no JVM) now: starts, reaches the title screen, opens
Singleplayer, creates a world (integrated server over netty local channels), loads it and renders
the world in-game under Xvfb/llvmpipe (OpenGL 4.5 core, Mesa 25.2): terrain, trees, mobs, HUD,
tutorial toasts, F3 overlay, keyboard/mouse input. Driven headless with xdotool (see section 7).

Fixed on the way (all with differential tests where the behaviour is JDK-level): interface
initialization with default methods, unaligned Unsafe access, http/https URL handlers, named
boot modules (`java.base` closed to deep reflection but exporting `java.*`/`javax.*`,
`jdk.unsupported` opening `sun.misc`), JNI `RegisterNatives`, reflective netty channel and cleaner
classes, JNA structure types.

Measured in-game on this 4-core, llvmpipe-only machine with the self-hosted backend: about 1 fps and
integrated-server ticks of ~390 ms (vs the 50 ms budget). Both the software GL rasterizer and
unoptimized generated code contribute; a real GPU removes the first, LLVM Release removes much of
the second (needs a machine with enough RAM, see section 8).

## 10. Patch system (patches/)

Hand-written Zig replaces hot methods without storing any Mojang code: `patches/manifest.json` names a method
(class, method, descriptor) and a Zig function in `patches/*.zig`. The transpiler keeps the translated original as
`<name>_orig` and emits a wrapper; `MONO_PATCH_VERIFY=1` runs both and aborts on a bitwise difference for primitive
returns. Works unsharded and sharded (patch files are copied into every shard; their `P.C_*` references are
added to the shard facades). Verified by tests/java/PatchTarget (test-only patches in the manifest, class absent
from Minecraft builds). Not yet verified: the mismatch abort path, the sharded copy path. See patches/README.md.

## 11. Performance work log (this branch)

Measured on the same 4 vCPU machine, sharded LLVM ReleaseFast, llvmpipe, world ready (vanilla HotSpot: 21.7 s):
43.0 s (start of the performance work) -> 42.3 (lazy-frame leaves) -> 36.5 (JDK intrinsics, SimpleBitStorage patch)
-> 33.5 (inline null/bounds checks) -> 33.3 (small-object zeroing, class lookup hash) -> 33.7 (line stored only on the
throwing path) -> ~31-33 (world height patches, inline checkcast/itable). Run-to-run noise is about 1-2 s.

Tools: `MONO_PROF=<file>` (in-process 1 kHz CPU sampling), `tools/prof_report.py`, `tools/bench.py`,
`tools/microbench/CallBench.java`, `MONO_PATCH_VERIFY=1`.

Findings: the game profile is flat (no method above ~3%); call-heavy code is 2.7x to 6.8x slower than HotSpot
(docs/BENCHMARK.md); 140k futex calls in 15 s are visible during world generation (one hot lock word, not yet
identified); memory management (allocation, zeroing, GC mark and sweep) is about 10% of CPU samples; software
rendering threads (llvmpipe) take about 7%.

Open structural options, none started: structured loops with register-resident locals instead of one labeled
switch per method; reference slots without the `R[]` memory array (conservative stack scanning, which risks
weak-reference timing differences); cross-shard inlining (needs fewer, larger shards or LTO, both limited by the
15 GiB build memory); a smaller object header and lower RSS (6.5-7 GiB vs 2.3 GiB for vanilla).

## 12. GC-free client core, Zero-LWJGL RHI, world culling & GPU instancing

1. **GC-free memory (`runtime/mem_pool.zig`, `runtime/thread_pool.zig`, `runtime/rt.zig`)**:
   - Eliminated stop-the-world mark-and-sweep GC (`collect`, `markAndSweep`, `traceParallel`, `sweepBlocks`), STW barriers, and loop safepoint overhead (`rt.safepoint()` is now a wait-free no-op).
   - Replaced with lock-free per-thread bump `ArenaAllocator` (`pushArenaScope` / `popArenaScope`, `beginFrame` / `endFrame`) + segregated `SlabPool` with 3-epoch quiescent deferred reclamation and a low-power wake-on-demand `WorkerPool`.
   - Closed JDK 25 `MethodHandleNatives` (`registerNatives`, `getNamedCon`, `init`, `expand`), `NativeImageBuffer.getNativeMap`, `MethodHandles.Lookup.IMPL_LOOKUP`/`checkAccess`, `MethodHandles` VarHandle combinators, `VarHandle` (`VarForm`, `AccessType`, `AccessMode`), `Class.getRecordComponents`, and Foreign `MemorySegment` `VarHandle` lowering (`runtime/varhandle.zig`, `monolith/program.py`, `monolith/zigen.py`).
2. **Custom RHI (`runtime/rhi.zig`, `runtime/rhi_vk.zig`, `runtime/rhi_gl.zig`, `runtime/rhi_bridge.zig`, `runtime/java/mono/rhi/NativeRhi.java`, `tools/mcbuild.py`)**:
   - Dynamic Vulkan 1.1+ backend (`libvulkan.so.1` probe, `vkCmdDrawIndexed` / `vkCmdDrawIndexedIndirect` command dispatch) with automatic fallback to strict OpenGL 3.3 Core (`rhi_gl.zig`: triple-buffered `GL_TEXTURE_BUFFER` TBO instancing + `glMapBufferRange` unsynchronized streaming + `glDrawElementsInstancedBaseVertex` for Intel HD 5500) and headless command-verification mode, plus `--no-lwjgl` and `--graphics auto|vulkan|opengl` support in `tools/mcbuild.py`.
3. **Vanilla-safe world render pipeline (`runtime/world_render.zig`)**:
   - Immutable `SectionSnapshot` culling leaves vanilla tick/redstone/physics/server logic untouched: 6-plane Gribb-Hartmann frustum culling, 6-face CaveCulling portal BFS (`computeSectionPortals`), conservative `HiZPyramid` occlusion, and viewport pixel-coverage LOD culling (`cullAndScheduleWorld`).
4. **GPU-driven non-block instancing (`runtime/gpu_instancing.zig`)**:
   - Unified 64-byte `GpuInstance` / 32-byte `GpuParticle` compaction (`InstanceBatcher`) across `BlockDisplay`, `ItemDisplay`, `TextDisplay`, mobs, players, dropped items, chests, signs, and particles into RHI indirect/instanced draw batches.
5. **End-to-end verification (`tests/run_client_core_e2e.py`)**:
   - `python3 tests/run_client_core_e2e.py -O Debug --shards 16 --shard-jobs 1`: passed in 594.6s (Zig unit tests `mem_pool` 2/2, `thread_pool` 1/1, `rhi` 3/3, `world_render` 4/4, `gpu_instancing` 6/6, 4-subsystem Zig E2E 8/8, `javac` + `monolith` 5901 classes / 31959 methods / 300 natives with `0 missing-native`, `0 link-error`, `0 unsupported-varhandle`, and native Java-to-Zig E2E run `JAVA_CLIENT_CORE_E2E_OK backend=3 vis=2 inst=6 batches=4 dtMs=5`).
   - Final `-O ReleaseFast` verification (`python3 tests/run_client_core_e2e.py -O ReleaseFast --shards 32 --shard-jobs 1`): passed all checks in 2398.9s (`JAVA_CLIENT_CORE_E2E_OK backend=3 vis=2 inst=6 batches=4 dtMs=2`, `0 missing-native`, `0 link-error`, `0 unsupported-varhandle`).
   - Post-GC-removal multithreaded JVM differential regression (`MONO_TEST_SHARDS=16 python3 tests/run_tests.py -O Debug Threads`): `PASS` (`13 lines; jvm 0.78s, native 100.75s, build 534.6s`).
   - Measured `-O ReleaseFast` subsystem microbenchmarks (`tools/microbench/run_client_core_bench.py`): `ArenaAllocator` `6.68 ns/op` (`149.7 M ops/s`), `SlabPool` 3-epoch `30.08 ns/op` (`0.0 ms` STW pause), `WorldRender` 256-section 3-stage cull `30.0 us/frame`, `InstanceBatcher` 2048-entity/particle cull+batch `264.4 us/frame`.

## 13. Verification on constrained hardware (2 vCPU / 1.9 GiB, 2026-11-16)

This checkout was audited and partially verified on a machine far smaller than the one behind sections 5–12 (2 vCPU Intel Xeon @ 2.60 GHz, 1.94 GiB RAM, no swap, 25 GB disk). See [`HARDWARE.md`](HARDWARE.md) for the full task-by-task table. Results, honestly:

* Verified here: `python3 -m py_compile` on all Python sources (clean), `pyflakes` on all Python sources (clean), `zig ast-check` on `runtime/*.zig` (clean), `bash -n` on all shell scripts (clean), `.claude/hooks/guard.py --self-test` (36/36) and its stdin/exit-2 protocol, `tests/run_tests.py --min-avail-gib` refusal path (exits 3, instant), and one full differential test: **Arith PASS** (JVM vs native identical output; 2 228 lines; jvm 0.16 s, native 0.58 s; zig Debug self-hosted build 495.0 s; load average ~10 during the build, zig peak RSS ~745 MB).
* During the Arith build the machine was effectively saturated (interactive commands timed out; kswapd active, `MemAvailable` bottomed at ~700 MB). No OOM kill occurred — the self-hosted backend plus `-fstrip` kept it alive. This is exactly the failure mode `docs/HARDWARE.md` and the new memory floors (`MONO_TEST_MIN_AVAIL_GIB`, `MONO_MIN_AVAIL_GIB`, guard.py heavy-job floor) now refuse instead of risking.
* **Not runnable on this box, not run**: the rest of the differential suite, the Client Core E2E, microbenchmarks, the sharded Minecraft build, and the full-program Zig compile. All claims in sections 5–12 come from the larger machine and were not re-measured here. The "10/10 passed" result in section 1 likewise predates this audit.
* Fixed in this audit: missing `.claude/hooks/guard.py` (implemented, self-tested), `tests/run_tests.py` had no memory guard (now refuses below a floor and stops the suite), docs referenced a nonexistent `docs/learnings/` and a stale jmod requirement (fixed), unused Python imports/variables and a placeholder f-string (removed; `pyflakes` is clean), `tests/__pycache__` and stale build trees (removed).

## 14. Gap audit & hardening pass (2026-11-16, second session)

Repository-wide audit for *açıklar* (latent gaps, not features): security, portability, correctness. All
fixes verified with lightweight checks only; the heavy gates (differential suite, Client Core E2E, sharded
Minecraft build) remain hardware-blocked on this box — see §13.

Fixed and verified in this pass:

* **tools/mcbuild.py — network-fetched JDK was extracted without any integrity check.** Now:
  SHA-256 of the archive is verified *before* extraction (expected digest from the Adoptium asset's
  `.sha256.txt` companion; the v3 `/checksums` API returns 404 and is not used); mismatch → archive is
  deleted and the run aborts. `MONO_JDK_NO_CHECKSUM=1` is a documented bypass. Tar extraction uses
  `filter="data"` on Python ≥ 3.12, otherwise realpath-containment + symlink/hardlink target checks;
  zip extraction enforces realpath containment (`_safe_extract_zip`). `extract_resources()` (client-JAR /
  native resources) had a zip-slip: entry names are validated against `dest` and copied by streaming.
  `fetch()` deletes partial downloads on failure; the version manifest is cached (no `.tmp` race);
  `ensure_jdk()` locates the installed JDK via `<install>/bin/javac` instead of recursion; the
  native-library scan now covers `.so` / `.dylib` / `.dll`; `read_jar_version()` fails with a clear
  message instead of a raw traceback; all network helpers send `User-Agent: monolith-mcbuild/<version>`
  (the default python-urllib UA is rejected 403 by Adoptium's CDN).
  **Verified end-to-end on this box:** `ensure_jdk(25)` fresh download → "JDK SHA-256 verified" →
  Temurin 25.0.5+8 (javac 25.0.5) under `~/.cache/monolith/temurin-25/`.
* **monolith/classpath.py — `jdk_home()` could not find the mcbuild-installed JDK** (only `MONO_JDK_HOME` /
  `JAVA_HOME` / `/opt/jdk25` / `/usr/lib/jvm` were probed), so a `mcbuild ensure-jdk` install was
  invisible to `monolith build`. Now also globs `~/.cache/monolith/temurin-*/**`. module-info parse failures
  now warn on stderr instead of failing silently.
* **monolith/__main__.py — auto backend selection.** The sharded path always forced LLVM (Debug builds
  OOM); it now mirrors the unsharded policy (self-hosted for Debug/ReleaseSafe, or when the machine
  provably cannot fit LLVM for optimizing builds). The unsharded path no longer pins Debug to LLVM
  either; both paths route through the same `_mem_total_gib()` estimate. The `fcntl` classlib lock is
  now optional (`msvcrt.locking` fallback, else proceed — outputs are content-addressed/immutable).
* **runtime/mem_pool.zig — arena `alloc` returned misaligned pointers for `align > 4096`** (now returns
  null per `Allocator` contract); removed the redundant `madvise(MADV_DONTNEED)` branch whose both paths
  freed the chunk identically. `zig ast-check` clean; `zig test -lc -O Debug` 2/2 pass.
* **monolith/program.py** — dead `synchronized` no-op removed. **tools/bench.py** — `SIGKILL` no longer
  assumed (falls back to `SIGTERM`). **tests/run_client_core_e2e.py** — no longer hardcodes one
  machine's JDK path (derives env → `classpath.jdk_home()` → `~/.cache/monolith/temurin-*`); adds a
  per-step timeout (`MONO_E2E_TIMEOUT`, default 10800 s).
* **Repo portability** — hardcoded author-machine paths eliminated: `.claude/COMMON_MISTAKES.md` and
  (paths made machine independent: `/root/...` → `$HOME/...`, `MCOUT` falls back to the repo-local
  `out/` tree instead of a `/tmp/claude-0/...` path).
* **Hygiene sweep** — secrets patterns (PEM blocks, AWS/GitHub/Slack tokens, JWTs, hardcoded
  passwords): 0 hits. No hardcoded user-machine paths remain.
* **New `.github/workflows/ci.yml`** — syntax/lint, `zig ast-check`, JSON validity, guard self-test,
  JDK 25 install, and one differential test (Arith). The heavy gates are deliberately not in CI; see
  `docs/HARDWARE.md`. Authored but NOT yet run on GitHub (no local runner here).

Explicitly NOT done / residual gaps (need the big machine, GitHub CI, or human review):

* Differential suite beyond Arith, Client Core E2E, sharded Minecraft build:
  not runnable on this box (§13). An attempt this session to re-run Arith was **aborted by the agent**
  when `MemAvailable` fell to ~34 MB during the zig Debug compile (zig peak RSS ~1.5 GB); the process
  was killed to protect the interactive session and ~726 MB of stale build output in `tests/build/` was
  removed. §13's 745 MB peak could not be reproduced under current conditions — heavy verification
  belongs on a bigger machine or in CI.
* Patch verification beyond primitive returns (see patches/README.md); the sharded patch-copy path
  remains untested.
* `.claude/hooks/guard.py` was not wired into a `.claude/settings.json` then (left as a user decision);
  it was wired on explicit user request in the following pass — see §15 (item 7).
* `tests/run_client_core_e2e.py` changes are syntax/lint-verified only (`bash -n`-class checks), not
  executed — the test needs the documented big-machine profile.


## 15. Architecture & integration gap pass (2026-11-16, third session)

Goal: find the code that exists only in synthetic tests/isolated modules but is not reachable from the
real client flow (`net.minecraft.client.*` — render/culling/instancing/RHI/memory lifecycle), and wire it in.
All analysis was done against this checkout; verification is transpile-only + static checks (1.94 GiB box —
no full builds). Nothing below claims in-game correctness; see "Not done".

### Integration gaps found (and what was done about each)

1. **The entire Zero-LWJGL RHI bridge was unreachable from any real build.** `mono.rhi.NativeRhi` (Java JNI
   facade of `runtime/rhi_bridge.zig`) was compiled only inside `tests/run_client_core_e2e.py`; neither
   `monolith build` nor `mcbuild.py --no-lwjgl` produced it, so `mono.rhi.NativeRhi` could never be linked from a
   client — the RHI/culling/instancing stack was dead code on the real path.
   **Fix:** `monolith/classpath.py: builtin_classes()` compiles `runtime/java/**/*.java` into a content-keyed
   cache (`~/.cache/monolith/builtin-<sha12>`, locked, stamped, `MONO_NO_BUILTIN=1` disables) and `cmd_build`
   prepends it to the application class path (before the JDK overlay, so our natives always resolve).
   The E2E harness now javacs `ClientCoreE2E.java` against it (no more hand-javac of the bridge).
   **Verified:** fresh compile + cache-hit (~6 ms); `javap` shows all bridge natives; transpile-only of
   `ClientCoreE2E` yields 5901 classes / 31959 methods / 300 natives with **0 missing-native / 0 link-error**
   (identical counts to the pre-change E2E ledger), `builtin_rhi: true` in report.json.
2. **Manifest patches that match nothing were silently dropped** — a typo'd patch entry is invisible
   forever. **Fix:** `monolith build` now reports them as `issues["unused-patch"]` with the likely cause.
   **Verified:** transpile-only of `PatchTarget` prints exactly the 9 non-matching mojmap patches; the
   3 used PatchTarget patches are not listed.
3. **The generated launcher crashed the vanilla client by design**: `run.sh`/`run.bat` passed
   `--graphicsBackend <api>` — vanilla `net.minecraft.client.main.Main` rejects unknown arguments.
   **Fix:** the backend is passed as `-Dmono.graphics.backend=<api>` (ignored by vanilla; read by any
   mono-aware code). The launcher's argument list is now exactly the vanilla one.
4. **Arena-scope bridge failure modes were silent.** `NativeRhi.pushArenaScope()` dropped scopes beyond
   depth 16 and `popArenaScope()` underflowed to a no-op — both hide lifecycle bugs in a long-lived client.
   **Fix:** both now throw `IllegalArgumentException` with a clear message. `submitSectionSnapshot` is now
   capped at 65536 submissions per reset with a `droppedSections` counter (the one unbounded growth path
   in the bridge); `shutdownClientRhi()` performs a final zero-STW `rt.gc()` epoch reclaim.
5. **No observability for long-lived-object bloat.** Nothing in the runtime answered "how big is the slab
   pool / the frame arena right now?", so a slowly leaking client was undebuggable from Java.
   **Fix:** six new bridge natives, exposed as `NativeRhi.arenaBytes()` (calling thread's transient arena),
   `slabCommittedBytes()`, `slabReclaimedBytes()`, `frameEpoch()`, `threadCount()`, `droppedSections()`.
   Note (important, by design): the runtime cannot safely free *unreachable* SlabPool objects — nothing can
   prove them dead without a GC, and `retireObject()` has zero callers. Transient Java work must run inside
   `NativeRhi.pushArenaScope()/popArenaScope()` (O(1) rewind). Epochs advance only in `rt.beginFrame()`;
   advancing on every nested scope would reclaim too early and was deliberately NOT wired.
6. **No client session lifecycle hooks existed** — the bridge could not be brought up/down with the game.
   **Fix:** `patches/client_frame.zig` (+2 manifest entries): `net/minecraft/client/main/Main.main` and
   `net/minecraft/client/Minecraft.run` are bracketed with orig-fallback (init RHI before, shutdown after,
   stats printout with `MONO_RHI_STATS=1`). **Opt-in:** unless `MONO_RHI=1` is set, the hooks are one getenv
   and call `orig` untouched; RHI init failure degrades to a printed warning, never an abort. On 26.x
   (unobfuscated production names, the mcbuild default) the `Main.main([Ljava/lang/String;)V` entry matches;
   on mapping-name mismatch the entry is inert and is now reported as `unused-patch`.
7. **`.claude/hooks/guard.py` was never wired** — nothing ran it automatically.
   **Fix:** `.claude/settings.json` created: `PreToolUse` on `Bash|Write|Edit|NotebookEdit` runs
   `python3 "$CLAUDE_PROJECT_DIR/.claude/hooks/guard.py"` (fail-closed). Verified: self-test passes; the
   exact settings.json command allows `git status` (exit 0), blocks `git push --force` (exit 2), blocks
   writes to `out/` (exit 2), allows normal writes (exit 0).

### What is still NOT wired (and why — honestly)

* **In-game render-loop injection** (`LevelRenderer`/`GameRenderer` → `mono.rhi.NativeRhi`, feeding
  `SectionSnapshot`s and instances per frame): requires the actual client jar in front of a human or a
  Mojang-mapped client, and a GPU machine to smoke-test. Blind patching of those signatures from this box
  would ship unverifiable code — worse than useless. The bridge API, the session hooks and the stats are
  the scaffolding for exactly that work.
* **`--no-lwjgl` end-to-end in-game**: even with the bridge reachable, the vanilla client calls
  `org.lwjgl.*` directly (fully reachable from the render path); those classes are stripped by
  `--no-lwjgl`, and RTA turns unreachable/missing references into `NoSuchMethod` throws. A working
  zero-LWJGL client needs a Java-side GL→RHI shim (or a GL-ABI shim in Zig routing to `rhi.zig`), i.e.
  re-implementing the client's GL calls — a project of its own, on a machine with the jar + GPU.
  The current `--no-lwjgl` flag is therefore documented as *transpile-only* scaffolding, not a runnable game.
* **Android surface layer**: none exists anywhere in this repository (no ANativeActivity/JNI glue, no GL/EGL
  or Vulkan surface creation). Implementing one blind would be pure fabrication; documented as absent.
  A real port needs ndk + a device/emulator + a window/context shim for `rhi_vk.zig`/`rhi_gl.zig`.
* **RHI frames are never produced by vanilla**: nothing in the client calls `NativeRhi.beginFrame`, so
  epoch reclamation and `frameEpoch()` stay flat in a vanilla session (the session hooks bracket the
  client lifetime only). Stats natives report that state truthfully; they do not fake activity.
* **Full builds** (suite, Client Core E2E, sharded Minecraft build): not runnable on this box (§13/§14).
  `tests/run_client_core_e2e.py` was simplified to the builtin bridge and is pyflakes-clean; it has NOT
  been executed.

### Verification done here

`py_compile` + `pyflakes` clean (all touched Python); `bash -n` clean; `zig ast-check` clean
(`runtime/*.zig`, `patches/*.zig`); full `zig test -lc -O Debug` of the patch + runtime graph:
**11/11 pass** (4 new `client_frame` lifecycle tests + all 7 pre-existing runtime tests);
4-subsystem Client Core E2E (`tests/e2e_client_core_test.zig`, Debug): **8/8 pass**;
`zig build-obj` of `rhi_bridge.zig` and `client_frame.zig` against the real output layout: clean; JSON valid
(`patches/manifest.json`, `.claude/settings.json`, `report.json`); `guard.py --self-test` PASS;
`builtin_classes()` fresh build + cache hit; transpile-only smokes: `PatchTarget` (unused-patch list OK,
built-in bridge linked) and `ClientCoreE2E` (`0 missing-native/link-error`, `builtin_rhi: true`);
kill-switch smoke with `MONO_NO_BUILTIN=1` (`builtin_rhi: false` in the report; analysis aborts with a
single loud `link-error: class not found: mono/rhi/NativeRhi` — the failure is explicit, never silent).
No native code was executed beyond the Zig unit tests — the transpiled outputs were analysed, not run.

### Arithmetic differential test on this box (JVM vs native, 1 GiB ceiling)

Task: run `tests/run_tests.py Arith` live with a hard 1 GiB memory ceiling so the box cannot be
OOM-killed, and show JVM vs native side by side. What was actually done and measured:

* cgroup v2 is read-only and `systemd-run --scope`/`--user` are denied on this box, so
  `memory.max` cannot be set; `/usr/bin/prlimit` exists. `RLIMIT_AS` (address space) is the only
  hard ceiling available per-process, and it degrades into clean allocation failures instead of
  OOM-killer picks. `RLIMIT_RSS` is a no-op on modern Linux.
* First attempt capped the *whole harness* at 1 GiB (`MONO_TEST_BUILD_AS_GIB=1`): it failed in
  `tools/gen_jla.py` because `javap` cannot start with <=1 GiB of address space (HotSpot reserves
  >1 GiB virtually). Wrapping the JVM itself in the same ceiling is not viable on this JDK.
* The ceiling was therefore applied to every `zig` invocation only (`MONO_ZIG_AS_GIB=1`,
  implemented as `preexec_fn` `RLIMIT_AS` in `monolith/__main__.py`, all 6 zig call sites; harness
  `tests/run_tests.py` reverted to no cap).
* With the cap, the transpiler (~40 s, 5907 classes / 31964 methods / 288 natives) and every
  sharded `zig build-obj` compiled fine; the **self-hosted link step** (`zig build-exe` of
  shard0 + all shard objects) reproducibly died with `error: OutOfMemory` under the 1 GiB ceiling,
  both Debug and `ReleaseSmall`, with 8 and 16 shards. GNU ld links the 474-640 MiB of objects in
  ~2 s but rejects zig's `extern threadlocal` references (NOTYPE UND) against the STT_TLS
  definitions in `shared.o`.
* Fix implemented: `MONO_ZIG_LD=cc` in `_compile_sharded` links with the system C toolchain instead
  of zig's self-hosted linker, after `tools/fix_tls_syms.py` retypes those four undefined symbols
  (`cur`, `processing_refs`, `throw_depth`, `attached`) to `STT_TLS` in the generated objects
  (layout unchanged; one `st_info` byte each). Off by default; zig link is still the default path.
* Live result (final run, output saved): `PASS Arith [2228 lines; jvm 0.13s, native 0.41s, build 127.0s]`
  — `1/1 passed`, `MemAvailable` 1560 MB before and 1524 MB after (nothing crashed). A standalone
  `prlimit --as=1G` run of the native binary and `diff` against the JVM stdout are byte-identical
  (0 lines), both stderr empty.
* Not done / not claimed: the 1 GiB ceiling measures *address space*, not RSS — the box's
  ~1.5 GiB availability was never at risk; the ceiling protects the machine, it is not a proof
  that zig's linker "fits" in 1 GiB (it does not — hence the GNU-ld path). The unsharded
  `zig build-exe` path and LLVM remain impossible under the ceiling. All other differential tests
  were not run. `tests/build` was deleted after the run.

## 16. Bugfix, memory-reclamation, register-return & RHI hardening pass (2026-11-16)

Audited and resolved the latent bugs and bottlenecks exposed after merging the v2 audit snapshot:

* **SlabPool OS memory reclamation & arena safety (`runtime/mem_pool.zig`, `runtime/rt.zig`)**:
  - Replaced unlinked 256 KiB slabs (which could never shrink back to the OS) with `128 KiB`-aligned
    `SlabBlock`s carrying an inline 128B header (`(ptr & ~(SLAB_SIZE - 1))` O(1) lookup): when all slots
    in a slab return or retire across 3 epochs, excess empty slabs above `MAX_RETAINED_EMPTY_SLABS` (4 = 512 KiB)
    are unmapped directly back to the OS (`madvise(MADV_DONTNEED)` on the warm reserve; zero-retention RSS
    bloat). Verified by unit test `"SlabPool returns completely empty slabs across epochs back to the OS"`.
  - Tuned per-thread cache refill batch (`max 32`) and `scratch_arena` retention (`max 2` chunks = 512 KiB).
  - Added `arenaSuspend`/`arenaResume` around `rt.intern`, `rt.classMirror`, and `rt.initClass` (`<clinit>`)
    so process-global objects never allocate inside transient frame/arena scopes.
  - Added arithmetic-overflow guard, `0xAA` debug safety poison on rewound arena slices, oversized base chunk
    release on `resetRetainingCapacity`, and LIFO `vtFree` accounting fixes in `ArenaAllocator`.
* **Register-return ABI (`rt.Res`) & dead shadow-array elimination (`runtime/rt.zig`, `monolith/zigen.py`, `monolith/shard.py`)**:
  - Implemented register-packed `rt.Res(T)` returns (`u64`/`u128` via `toR`/`unR`) across `_r` wrappers,
    vtables, itables, reflection `reflectInvoke`, `Class.enumValues0`, `MemorySegment.address()` `VarHandle`
    lowering, and cross-shard facades.
  - Eliminated the dead write-only `var R: [K]rt.Ref` stack array and escaping `.refs = &R` pointer from
    compiled Java methods (`sv`/`lv` use register-promotable locals `s*a`/`l*a`; `F.refs = null` by default).
  - Removed duplicate `invokestatic` check in `_is_leaf2` (`monolith/zigen.py`).
* **RHI, GL 3.3 Core, World Culling, GPU Instancing, Render-Loop Hooks & `--no-lwjgl` (`runtime/rhi*.zig`, `patches/`, `runtime/java_nolwjgl/`, `tools/mcbuild.py`)**:
  - `runtime/rhi_gl.zig`: fixed synthetic pipeline binding (`gl_default_prog` GLSL 330 shader + required GL 3.3
    Core VAO `gl_default_vao`), `uFirstInstance` uniform fallback when `has_base_instance == false` (Intel HD
    5500), zero-offset orphan+unsync TBO upload (`size_bytes`), `eglGetProcAddress` fallback, and lazy
    `tryAttachGlContext()`.
  - `runtime/world_render.zig`: fixed CaveCulling portal BFS so diamond paths accumulate every entry face
    (regressed test passed); `runtime/gpu_instancing.zig`: ordered opaque draws before translucent
    (`particle`/`text_display`) and excluded unregistered meshes from GPU staging uploads and survivor stats.
  - `runtime/rhi_bridge.zig` & `runtime/java/mono/rhi/NativeRhi.java`: added bridge-owned `frame_arena`, explicit
    validation/OOM errors, `submitSectionSnapshotEx`, `visibleSectionCount`/`visibleSectionField`, and
    GL command shims; added per-tick/render lifecycle hooks in `patches/client_frame.zig` and `patches/level_renderer.zig`.
  - Added opt-in Zero-LWJGL shim (`runtime/java_nolwjgl/org/lwjgl/...`) wired cleanly via `--no-lwjgl` in
    `monolith/__main__.py` and `tools/mcbuild.py` (fixed `--no-lwjgl` launcher props).
* **JNA Udev Proxy Folding & `String.coder` (`monolith/program.py`, `classlib/src/java/lang/String.java`, `tests/java/JnaProxyFold.java`)**:
  - Fixed `_fold_proxy` (`monolith/program.py:637`) to stop at array `aastore` boundaries and filter to interfaces
    only (fixing `Proxy.newProxyInstance(..., ifaces, ...)` when trailed by `Class.getClassLoader()`).
  - Hardened `_fold_jna_proxy` for deeper windows and trailing option maps (`Udev extends Library`).
  - Added `private final byte coder = UTF16;` (0 extra bytes in existing alignment padding) in `String.java`.
* **Verified live here (`aarch64` 10-core Linux, 7.4 GB RAM):**
  - `ClientCoreE2E` `-O Debug`: **6/6** `mem_pool`, **1/1** `thread_pool`, **7/7** `rhi`, **9/9** `world_render`,
    **12/12** `gpu_instancing`, **14/14** 4-subsystem E2E, `1,092` classes / `5,932` methods (`0 missing-native,
    0 link-error, 0 unsupported-varhandle`), build time reduced from `587.1s` to `37.9s` (**15.5x faster**).
  - Full 27-test differential JVM suite (`tests/run_tests.py`, `-O Debug`): **27/27 PASS** (`Arith`, `Strings`,
    `Oop`, `Exceptions`, `Lambdas`, `Collections1`, `Threads`, `GcStress`, `Misc`, `IoZip`, `NioFiles`, `NetIo`,
    `HttpFetch`, `IfaceInit`, `LazyLeaf`, `Intrinsics`, `PatchTarget`, `Reflect`, `MHandles`, `Annot`,
    `StackWalk`, `ModuleInfo`, `Platform`, `Jni`, `OptSuite`, `JnaProxyFold`, `ClientCoreE2E`).
  - `tools/microbench/run_client_core_bench.py` (`-O ReleaseFast`): `arena_alloc_64B` `11.71 ns/op` (`85.4 Mops/s`),
    `slab_epoch_64B` `72.30 ns/op` (`stw_pause_ms=0.0`), `worker_pool` `146.9 us/batch` (`36.73 ms` total, **108.5x faster**
    than `3987.72 ms`), `world_cull_256` `40.1 us/frame`, `gpu_instancing_2048` `1499/2048` in `7` batches (`287.0 us/frame`).

## 17. Deep system audit, RTA closure reduction & S24 FE stability pass (2026-11-16)

* **RTA closure bloat elimination (`5,902` -> `230` classes on base tests; 15x–40x faster builds)**:
  - `classlib/src/java/lang/Module.java`, `Class.java`, `System.java`: decoupled `BootModule` / `UnsupportedModule`
    from `ModuleDescriptor.newOpenModule(...)` and eager `ModuleLayer.empty()` in `System.<clinit>`; removed
    `java.lang.classfile.*`, `ModuleBootstrap`, `ClassLoaders`, `BuiltinClassLoader`, `ServicesCatalog`, and
    `URL.openStream()` (`JarVerifier` -> `ProviderConfig` -> 1,500+ SSL/JCA/Crypto classes) from `Module.java`;
    added dynamic host `os.arch` native (`J_java_lang_System__arch0`: `aarch64` / `amd64`).
  - `classlib/src/java/lang/String.java`: implemented `String.format` / `String.formatted` directly without
    `java.util.Formatter` or `BigDecimal` (eliminating `Formatter`/`Locale`/`Calendar`/`java.time`/`Pattern`/`ICU`
    closure bloat from JDK `.toString()` and error paths while passing all `Misc` and `Oop` format tests).
  - Added lightweight overlays `classlib/src/jdk/internal/loader/ClassLoaders.java`,
    `classlib/src/jdk/internal/util/ClassFileDumper.java`, `classlib/src/jdk/internal/util/ReferencedKeyMap.java`,
    `classlib/src/java/util/stream/Tripwire.java`, and
    `classlib/src/sun/reflect/generics/reflectiveObjects/ParameterizedTypeImpl.java`; updated `tools/gen_jla.py`
    and `monolith/classpath.py` (`services()` scoped to application sources `[:boot_start]`).
* **Runtime, reflection, VarHandle & JNI fixes**:
  - `runtime/thread_pool.zig`: batched queue insertion under a single mutex lock with post-unlock `cv.broadcast()`
    and single `wg.add(count)` (`15,950.9 us/batch` -> `146.9 us/batch`, **108.5x speedup**).
  - `runtime/varhandle.zig`: fixed unaligned `MemorySegment` `getSeg`/`setSeg` (`*align(1)`) to prevent alignment
    panics in `Debug`/`ReleaseSafe`.
  - `classlib/src/jdk/internal/reflect/MonoFieldAccessor.java`: routed `isVolatile` fields through
    `Unsafe.get*Volatile` / `put*Volatile`.
  - `runtime/natives.zig`: added maximally-specific default interface method lookup in `findTarget`.
  - `runtime/jni.zig` & `runtime/jni_call.zig`: added static-flag check on interface methods in `findMethod` and
    implemented `DefineClass`, `FromReflectedField`, `ToReflectedMethod`, `ToReflectedField`, and `GetModule`
    (0 unimplemented JNIEnv slots).
  - `monolith/zigen.py`: removed dead `_shadow_refs` and `_compact_refs` helpers.
* **Mobile / Termux LMKD protection (`monolith/__main__.py`, `tests/run_tests.py`, `tests/run_client_core_e2e.py`)**:
  - Capped child `zig`/`javac`/`java` CPU affinity (`sched_setaffinity({0, 1})`, `nice(10)`), JVM heap (`-Xmx192m`/
    `-Xmx256m -XX:+UseSerialGC`), automatic sharding above 450 classes, and `cc` linking on `<4.5 GiB` available RAM.

## 18. TouchCore For Monolith, Android `build-lib` Pipeline & GLES3/GLFW Surface Bridge (2026-11-16)

* **Android `libminecraft_mono.so` Production Pipeline (`monolith/__main__.py`, `tools/mcbuild.py`, `runtime/rt.zig`, `monolith/zigen.py`, `monolith/shard.py`)**:
  - Added `build-lib` subcommand and `-dynamic` / `--dynamic` + `--target aarch64-linux-android.26` support to both
    `monolith` and `tools/mcbuild.py` (using Termux/NDK clang + `ld.lld -shared -z max-page-size=16384` +
    `libclang_rt.builtins-aarch64-android.a` for `__emutls_get_address`).
  - Exported `mono_android_entry(...)` from generated `main.zig` (sharded & unsharded) calling `rt.runMainAsync`,
    spawning the JVM main thread with full stack (`default_stack_size * 4`) and detaching immediately (`rc = 0` in <=1 ms)
    so the Android UI thread never blocks.
  - Fixed Android `aarch64` pointer-tagging / TBI (`bit 63 = 1`) panics by converting pointer-to-`i64` casts from
    `@intCast` to `@bitCast` across `monolith/zigen.py` (`mirrorSet`/`mirrorGet`), `rt.zig` (`eetop`), `natives.zig`,
    `varhandle.zig`, `io.zig`, and `nio.zig`; fixed cross-shard `getModule` `rt.unR` type handling in `runtime/jni_call.zig`
    and added `gl_shim.zig` to `shard.py` `RUNTIME_FILES`.
* **GL Path Decision & Direct GL → `libGLESv3.so` Dispatch (`runtime/gl_shim.zig`, `runtime/rhi_gl.zig`, `runtime/rhi_bridge.zig`, `runtime/java/mono/rhi/NativeRhi.java`, `runtime/java_nolwjgl/`)**:
  - Chose direct native dispatch to Android `libGLESv3.so` with GL 3.3 Core → GLES 3.0 compatibility shims
    (GLSL `#version 150`/`330 core`/`4xx` -> `#version 300 es`, `glClearDepth`->`glClearDepthf`,
    `glDepthRange`->`glDepthRangef`, safe `glPolygonMode` no-op) instead of re-implementing the entire stateful GL
    pipeline in Java `NativeRhi`.
  - Updated `runtime/rhi_gl.zig` with Android `libGLESv3.so` + `libEGL.so` probe fallback and `ES 3.00` shaders,
    and expanded `runtime/java_nolwjgl/org/lwjgl/` (`GLFW`, callback interfaces, `GL11C`, `GL15C`/`GL15`, `GL20C`/`GL20`, `GL30C`/`GL30`).
  - Wired `GLFW.glfwCreateWindow`, `glfwMakeContextCurrent`, `glfwSwapBuffers`, and `glfwPollEvents` to TouchCore's
    `ANativeWindow` + `EGL` + `g_tc_input` via `tc_android_has_window`, `tc_android_init_egl`, `tc_android_swap_buffers`,
    `tc_android_get_size`, and `tc_android_poll_snapshot` (`libtouchcore_mono.so`).
* **MJLauncher-Style TouchCore APK (`android/touchcore/`, `releases/Monolith.apk`)**:
  - Fixed startup/pre-game crash hazards: expanded `VkPhysicalDeviceProperties.limits_pad` to `[560]u8` (`runtime/rhi_vk.zig`)
    to match the 824-byte Vulkan ABI struct, moved 131 KB portal scratch off the pthread stack (`touchcore_core.zig`),
    added `android:extractNativeLibs="true"`, `gl_ready` + EGL depth/ES2 fallback + 4 MB render-thread stack (`touchcore_android.c`),
    and deferred `mono_android_entry` off the UI thread until `ANativeWindow` attaches.
  - Upgraded UI & controls (`HudRenderer.java`, `TouchOverlayView.java`, `ControlsEditorActivity.java`, `GameActivity.java`,
    `LauncherActivity.java`): frosted glass vector-icon buttons, connected `+` D-Pad & Analog Stick, swipeable attack/use
    camera drag, virtual cursor (`FARE`), soft keyboard (`KB`), center crosshair, isometric 9-slot hotbar, visual HUD
    editor with landscape preview + fine nudge pad, explicit **"DEMO MODU (Zig RHI Voxel)"** labeling, and explicit
    error dialog when an external `libminecraft_mono.so` fails (no silent fallback to Demo Mode).
* **Verified live on this ARM64 Android phone:**
  - `python3 tools/build_touchcore_apk.py`: built [`releases/Monolith.apk`](../releases/Monolith.apk) (`0.98 MiB`), `apksigner verify` PASS (v2/v3), `zipalign -c -p 4` PASS, `nm -D` confirms all `Java_com_monolith_touchcore_NativeBridge_*` & `tc_android_*` symbols.
  - Bionic PIE native tests via `/system/bin/linker64`: verified `libtouchcore_mono.so` (`tc_zig_init`, 10 ticks, `tc_zig_shutdown`), single-shard & 4-shard `--no-lwjgl` `-dynamic` `libminecraft_mono.so` (`mono_android_entry` returned `0` in `1 ms`, `NO_SURFACE_EXPLICIT_THROW=true`, `NativeRhi.pollOsInput` reading `KEY_W=1` across `.so` boundary, `GL11C` GL→GLES3 dispatch, `EXIT=0`).
  - Regression suites: `tests/run_tests.py Arith IoZip` PASS; `tests/run_client_core_e2e.py` ALL 4 E2E CHECKS PASS (`mem_pool 6/6`, `thread_pool 1/1`, `rhi 7/7`, `world_render 9/9`, `gpu_instancing 12/12`, `4-Subsystem E2E 14/14`, `ClientCoreE2E` native `JAVA_CLIENT_CORE_E2E_OK`).

## 19. TouchCore Mojang Downloader, Expanded `--no-lwjgl` Stack & Honest Android `client.jar` Status

* **Official Mojang Version/Asset Downloader (`android/touchcore/src/com/monolith/touchcore/MojangDownloader.java`, `LauncherActivity.java`)**:
  - Fetches `https://piston-meta.mojang.com/mc/game/version_manifest_v2.json` directly from Mojang CDNs (no bundled Mojang game files or jars inside the APK).
  - Downloads the selected release `<id>.json`, `client.jar` (size-checked) -> `<filesDir>/monolith/versions/<id>/client.jar`, and `<assetIndex.id>.json` -> `<assetsDir>/indexes/<assetIndex.id>.json` on a background thread. *(Live HTTPS download from Mojang CDNs has not been tested inside the headless PRoot environment.)*
* **Expanded `--no-lwjgl` Client Shim Coverage (`runtime/java_nolwjgl/org/lwjgl/`)**:
  - `system/`: Added `MemoryStack` (try-with-resources frame allocator), `Pointer`, `Configuration`, `Callback`.
  - `stb/`: Added `STBImage` (`stbi_info_from_memory`, `stbi_load_from_memory` real PNG chunk + `java.util.zip.Inflater` RGBA decoder + filter reconstruction, `stbi_set_flip_vertically_on_load`, `stbi_image_free`).
  - `opengl/`: Added `GLCapabilities` (`OpenGL11`..`OpenGL33`, ARB flags returned by `GL.createCapabilities()`), `GL31C`/`GL31`, `GL32C`/`GL32`, `GL33C`/`GL33`, plus `GL30C` mipmap/renderbuffer/framebuffer helpers.
  - `glfw/`: Added `Callbacks.glfwFreeCallbacks`, `GLFWVidMode`, `glfwGetPrimaryMonitor`, `glfwGetVideoMode`, `GLFWErrorCallbackI`, `GLFWCharCallbackI`, `GLFWScrollCallbackI`, `GLFWWindowFocusCallbackI`, `glfwSetClipboardString`/`glfwGetClipboardString`, `glfwSwapInterval`.
  - `openal/`: Added minimal `ALC`, `ALC10`, `AL`, `AL10`, `ALCCapabilities`, `ALCapabilities` stubs.
* **Android Multiarch C Header Fix (`monolith/__main__.py`)**:
  - Added `_android_include_flags()` passing `/data/data/com.termux/files/usr/include/aarch64-linux-android` (`asm/types.h`) to `zig build-obj` and `clang` so `@cImport(@cInclude("zlib.h"))` in `runtime/io.zig` compiles cleanly when building Android `--target aarch64-linux-android.26` `-dynamic` libraries using `java.util.zip.*` / `STBImage`.
* **Verified after these updates**:
  - `python3 tools/build_touchcore_apk.py`: built [`releases/Monolith.apk`](../releases/Monolith.apk) (`0.98 MiB`, 13 Java files, 107 D8 classes), `apksigner verify` PASS (v2/v3), `zipalign -c -p 4` PASS.
  - 4-shard Android `aarch64-linux-android.26` `-dynamic --no-lwjgl` build (`FullNoLwjglTest` -> `libminecraft_mono.so`, `26233 KiB`) executed directly on Android Bionic `/system/bin/linker64`: `mono_android_entry` returned `0` in `0.82 ms`, `STBIMAGE_PNG_OK 2x2 RGBA`, `GL_CAPS_OK OpenGL33=true`, `OPENAL_OK buf=1 src=2`, `GLFW_SURFACE_CHECK_OK vm=1280x720` (plus explicit `UnsupportedOperationException` without attached OS surface), `FULL_NOLWJL_BIONIC_OK`, `Bionic exit=0`.
  - `zig test -lc runtime/rhi.zig`: `All 7 tests passed.`
  - `python3 tests/run_tests.py Arith IoZip`: `2/2 passed`.
* **Remaining blockers before vanilla Mojang `client.jar` runs 100% playable on phone**:
  - Generating an Android `libminecraft_mono.so` from a full Mojang `client.jar` has **not** yet been run (`client.jar` is not present locally, and AOT compiling the ~61k-file Vanilla closure with LLVM requires ~7–15 GiB RAM per `§8`, while `monolith/__main__.py` restricts `--backend native` to `x86_64/AMD64`, meaning large vanilla AOT `.so` builds must be cross-compiled from a high-RAM host or split further).
  - Real `client.jar` gameplay end-to-end on a live Android SurfaceFlinger GPU/audio session (Blaze3D/Minecraft shader uniform compatibility under GLES3, keyboard/touch gameplay loop inside `GameActivity`, and remaining unimplemented JDK/third-party natives tracked in `§2–3`) has not yet been executed on-device.

## 20. Full MC `26.3` `--no-lwjgl` SDL3/GLES3/OpenAL Parity, Escape Analysis & SHA-1 Downloader

* **Real Minecraft `26.3` (`26.3-client.jar`, 41,483,720 bytes + 42 libraries) Empirical Gap Closure**:
  - Added full LWJGL 3.4 SDL3 surface/event/input layer (`runtime/java_nolwjgl/org/lwjgl/sdl/` — `SDLVideo`, `SDLEvents`, `SDLKeyboard`, `SDLMouse`, `SDL_Event` + sub-events, `SDLInit`, `SDLTimer`, `SDLClipboard`, `SDLStdinc`, `SDLSurface`, `SDLPixels`, `SDLMessageBox`, `SDLHints`, `SDLLog`, `SDLKeycode`, `SDLScancode`, `SDLVulkan`) wired to `NativeRhi` (`ANativeWindow` + `g_tc_input`).
  - Expanded OpenGL 3.3 Core -> GLES3 & OpenAL bridge (`runtime/gl_shim.zig`, `runtime/al_shim.zig`, `runtime/rhi_bridge.zig`, `runtime/java/mono/rhi/NativeRhi.java`, `monolith/shard.py`): added FBO blit, buffer copy/range mapping, samplers, queries, `glVertexAttribDivisor`, `glFenceSync`/`glDeleteSync`/`glClientWaitSync`, `glDrawBuffer`, `ARBDirectStateAccess` DSA->bind adapters, `ARBVertexAttribBinding` state emulation onto `GL33C`, dynamic `libopenal.so.1`/`libopenal.so` audio probing with clean fallback, real `STBImageResize` bilinear RGBA scaling, `STBImageWrite` Deflater PNG encoding + `STBIWriteCallback.getData`, `StructBuffer.get()`, `Pointer.Default`, and full `org/lwjgl/vulkan/`, `util/vma/`, `util/shaderc/`, `util/spvc/`, `util/freetype/` (`FT_Init_FreeType = -1` bitmap font fallback) link closure.
  - Fixed `MethodHandle.invokeBasic` signature-polymorphic lowering (`monolith/program.py`, `classlib/src/java/lang/invoke/MethodHandle.java`) and added `net/minecraft/client/renderer/LevelRenderer.resetLevelRenderData()V` to `patches/manifest.json`.
  - RTA verification against `26.3-client.jar` (`61,280` loaded classes, `278,867` reachable methods) cut `org/lwjgl/*` link errors from **`345` -> `0`** (all patches including `resetLevelRenderData` matched `true`; `91` non-LWJGL link errors remain, mostly optional Log4j appenders/`NarratorMac` + `9` `MethodHandle`/`Lookup` internals, alongside `103` JDK natives and `257` third-party natives).
* **Intra-Method Escape Analysis & Auto-`ArenaScope` (`monolith/zigen.py`, `runtime/rt.zig`)**:
  - Added `rt.arenaMark`, `rt.arenaRestore`, `rt.newObjectArena`, and `rt.newArrayArena` in `runtime/rt.zig` plus forward DAG dataflow escape analysis (`_analyze_local_allocs`, `_safe_ea_callee`) in `monolith/zigen.py` to bump-allocate proven loop-free non-escaping `new`/`newarray`/`anewarray` objects on the thread's `scratch_arena` and rewind at method exit.
* **TouchCore SHA-1 Library + Asset Object Downloader (`android/touchcore/src/com/monolith/touchcore/MojangDownloader.java`)**:
  - Upgraded downloader with Linux OS rule filtering, 4-thread `ExecutorService`, and SHA-1 + size verification for `client.jar`, non-native library JARs, `assetIndex`, and individual `resources.download.minecraft.net` asset objects. *(Live HTTPS download still untested inside offline PRoot.)*
* **Verified after edits**:
  - `python3 -m py_compile` / AST checks / `.claude/hooks/guard.py --self-test` / `zig ast-check`: PASS; all `276` Java files in `runtime/java/` + `runtime/java_nolwjgl/` compiled cleanly with JDK 25 `javac` (`rc=0`).
  - `tests/run_client_core_e2e.py`: `ALL 4 CLIENT CORE E2E CHECKS PASSED (Debug)` (`mem_pool 6/6`, `thread_pool 1/1`, `rhi 7/7`, `world_render 9/9`, `gpu_instancing 12/12`, `4-Subsystem E2E 14/14`, `JAVA_CLIENT_CORE_E2E_OK`, `0 missing-native`, `0 link-error`, `0 unsupported-varhandle`).
  - 4-shard Android ARM64 `-dynamic --no-lwjgl` Bionic `/system/bin/linker64` test (`BionicSdlCheck`, `17438 KiB` `.so`): emitted `rt.arenaMark`/`newObjectArena`/`newArrayArena`, printed `HOST_ENTRY_RC=0`, `EA_ARENA_OK=1002502500`, `SDL_NO_SURFACE_GUARD_OK=true`, `GL_SHIM_OK=true`, `OPENAL_SHIM_OK=true`, `STB_PNG_OK=true`, `FREETYPE_FALLBACK_OK=-1`, `BIONIC_SDL_E2E_PASS`, `BIONIC_EXIT=0`.
  - Differential tests (`tests/run_tests.py`): `Arith` (`1/1 PASS`), `MHandles` (`1/1 PASS`); `GcStress` matched `churn`/`big arrays`/`threads`/`map 200000 262501012965` while `WeakReference` returned `weak cleared=false` under GC-free `SlabPool`.
  - TouchCore APK (`python3 tools/build_touchcore_apk.py`): built [`releases/Monolith.apk`](../releases/Monolith.apk) (`0.98 MiB`, 13 Java files, 109 D8 classes), `zipalign -c -p 4` PASS, `apksigner verify` v2/v3 PASS.
* **Remaining external/hardware blockers**:
  - Full `26.3-client.jar` LLVM `.so` compilation (~`7–15 GiB` RAM) must run on GitHub Actions ([`build-so.yml`](../.github/workflows/build-so.yml)) or an `x86_64` host; live on-device `SurfaceFlinger` rendering + shader compatibility + HTTPS Mojang downloads await testing on the physical phone with the compiled `.so` loaded into TouchCore APK.


## 2026-11-18: Android launcher and engine verification

- Environment: Zig 0.15.2, Android NDK r27c, JDK 25.0.5, Android SDK platform 34 / build-tools 34.0.0, 4 cores, 15 GiB RAM.
- Full `libminecraft_mono.so` (Minecraft 26.3, ReleaseFast, `aarch64-linux-android.26`, 16 shards) built: 437 MB stripped, 16 KB page aligned, `mono_android_entry` exported.
  One shard needs up to 7.5 GiB, so the machine builds one shard at a time. An interrupted build resumes with the `--only-shards K,K` option of the Minecraft build tool.
- Crash causes fixed: `exit()` racing HWUI/GPU-driver destructors (now `exit_group` on Android); engine stdio invisible (now forwarded to logcat tag `MonolithEngine`);
  glibc-only `__errno_location`, `copy_file_range` and `posix_spawn*` imports that made `dlopen` fail on bionic (errno via `std.c._errno`, raw syscall, weak imports);
  EGL context destroyed on every Surface change (the context is kept, only the window surface is rebound).
- Launcher: the duplicate hotbar and crosshair drawn over the game HUD are gone; touch slots map onto the real hotbar.
- Differential tests: 26 of 27 default tests pass. `GcStress` (WeakReference not cleared after `System.gc()`) is open. `ClientCoreE2E` has its own runner.
- D8 8.2.2 (build-tools 34) crashes on JDK 25 class files: use R8 9.5.33 as `~/.cache/monolith/android-sdk/r8.jar`.
- Not verified here: running the `.so` on a device or emulator.

### 2026-11-19: engine start-up verified on x86_64 with a desktop mock of the Android host

- `tools/mock_android_host.c` stands in for `libtouchcore_mono.so` (EGL pbuffer, GLES 3 through Mesa): window, GL context and the
  whole `Minecraft.<init>` path run without a phone. Debug engine start-up reaches shader loading.
- Fixed on the way: STBImage failure reason (null, not "ok"), `ModuleLayer.boot()`, JNI `GetEnv`/`AttachCurrentThread` for threads that
  already have a ThreadState (JNA deadlock), `class$` forName helper in the RTA, VarHandle ordinary methods kept, NumberFormat without
  locale providers, no-LWJGL Vulkan/GL/memory stubs the game reflects on.
- **Open (blocks rendering): shader compilation.** Minecraft 26.3 compiles GLSL to SPIR-V with shaderc and, for the GL backend, translates it
  with SPIRV-Cross (`renderpearl` `GlslCompiler.compileToSpv`). `runtime/java_nolwjgl/org/lwjgl/util/shaderc` and `.../spvc` are stubs that
  report "shaderc disabled in --no-lwjgl build", so the first critical pipeline (`minecraft:pipeline/gui`) fails and the game ends with a crash report.
  Needed: shaderc (glslang, SPIRV-Tools; sources ship with the NDK under `sources/third_party/shaderc`) and SPIRV-Cross compiled for aarch64,
  Java natives for the calls `renderpearl` makes, GLSL ES 3.x output, and linking into `libminecraft_mono.so`.

## 21. Release-gate verification on this machine (2026-11-24)

- Environment: 4 vCPU, 16 GiB RAM, JDK 25.0.5.1+2 (`~/.cache/monolith/temurin-25`), rustc/cargo, Python 3.13. No device, no GPU, no display.
- Passed (commands as in `docs/RELEASE.md` section 2 where listed):
  - `pyflakes monolith tools tests/run_tests.py`: no output. `compileall` on `monolith tools tests`: exit 0.
  - `rustc --test tests/rust/audio_main.rs`: 12/12.
  - `monolith.crc_tests`, `netty_tests`, `recycle_tests`, `patch_tests`, `stack_tests`: `ALL PASSED` each.
  - `tests/run_tests.py GcStress Oop Threads Lambdas Strings Collections1` with `MONO_GC_MB=1 MONO_GC_POISON=1 MONO_ARENA_CHECK=abort`: 6/6, plain and `MONO_GC_CONCURRENT=1`.
  - `tests/run_tests.py Arith`: 1/1.
  - `tools/rt_typecheck.py`: PASS.
  - `android/touchcore/native` `cargo test`: 29/29.
- Correction: `GcStress` is no longer open. §18 and §16 lines about it are historical; this run passed both GC modes.
- Not run here (hardware or time limit): full Minecraft build, `tools/headless_play.sh` game start, 60 min leak soak, Android device run, Client Core E2E.

## 22. Code changes of 2026-11-24 and what is still unverified

Done, with the check that was run:
- `monolith/rt/crash.rs`: panic hook and Linux fatal-signal handlers (SEGV, BUS, ILL, FPE, ABRT) write `mono-crash-<pid>.txt` (dir: `MONO_CRASH_DIR`). Replaces the SIGSEGV-only stderr handler in `rt.rs`. `tests/rust/crash_main.rs`: 5/5 (real panic and real SIGSEGV in child processes). `tools/rt_typecheck.py`: PASS.
- `tools/check_link_errors.py` and `docs/LINK_ERRORS_TR.md`: all 446 link errors of the Minecraft 26.3 transpile fall into five explained groups; a new unexplained one fails the gate (positive and negative case run).
- Patch fingerprints (`monolith/patches.py`, `tools/patch_fingerprints.py`): all 10 patches pin their method body; `monolith.patch_fp_tests` 9/9.
- WS7: 35 unused Java overlay files deleted from `runtime/java_nolwjgl` (277 to 242); the remaining tree compiles with the same `javac` call the pipeline uses (295 classes). `docs/WS7_ENVANTER_TR.md`.
- `docs/JNI_ENVANTER_TR.md`: 259 JNI/JNA emulation natives by group; the Tracy natives are not called in normal play (bytecode checked).
- Zig leftovers: dead `touchcore_core.zig` removed, stale `.zig` file references in comments point at the Rust modules.
- Runtime bugs fixed (`monolith/rt/sync.rs`, `gc.rs`, `rt.rs`), each with a test that ran: a lost interrupt in `Object.wait` (`WaitInterrupt`, 3000 rounds); a lock-order deadlock between `interrupt` and a timed `wait`; stale interrupt flags and park permits inherited by a new `Thread` at a reused address; `holdsLock` and `notify` creating monitors; monitors of dead objects never released (now dropped by the collector between marking and sweeping, `MonitorChurn`); `MONO_TRACE_EX` and `MONO_TRACE_PARK` read on every exception or monitor wait instead of once. Both GC modes with `MONO_GC_POISON=1 MONO_ARENA_CHECK=abort`, 12 repeats each: no failure.
- `rustgen.py`: the compiled-object cache `/tmp/monolith_shard_cache` had no limit and reached 42 GB (the disk was full in the middle of the differential suite, 14 tests failed with `No space left on device`, not from code). Now capped at 8 GiB, oldest first (`MONO_SHARD_CACHE_GB`); `monolith.cache_tests` 7/7. After the cap the 14 tests were re-run: 14/14.
- Differential suite `tests/run_tests.py`: 27 passed, 3 skipped (`ClientCoreE2E`, `HttpFetch`, `Jni`), each skip with its measured reason. `NetIo` runs and passes (its skip was out of date).
- `MONO_COUNT_NATIVES` (`monolith/rt/ncount.rs`): the meter for the JNI/JNA = 0 goal; checked on a small program that calls JNA-style natives (7 calls, 3 natives counted correctly); `tests/rust/ncount_main.rs`.
- Docs: `docs/KULLANICI_REHBERI_TR.md` (settings checked against the runtime code), `docs/PLAN_4_GUN_TR.md`, `docs/JNI_ENVANTER_TR.md`.

Not verified (needs a full Minecraft pipeline run and a game start, left to the end on request):
- That the transpile after the overlay deletion still reports 446 link errors with 0 unexplained and 0 missing natives.
- That a game started from the new runtime (crash reporter installed at `init_hooks`) behaves as before.
- That the pipeline accepts the 10 fingerprints (the tool and `validate()` use the same function on the same classes; the pipeline itself was not run after the change).
- The 16-shard desktop run was stopped at 7 of 16 shards, on request, to free the machine for code work.
