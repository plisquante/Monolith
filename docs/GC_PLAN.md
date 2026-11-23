# Memory architecture plan: from stop-the-world GC to an idle collector, then to no collector

Goal (owner's words): GC at zero, zero leaks, never a visible drop. Route: make the collector invisible first
(always-on, small, paced), shrink what it has to do (less garbage, owned data), then switch it off only when a
measured leak budget says it is safe. Nothing below is implemented yet unless it says "done".

## 0. Definition of done

| Metric | Today | Target (idle collector) | End state (collector off) |
|---|---|---|---|
| Longest GC pause on the game thread | not measured (STW over whole heap) | < 1 ms (root snapshot only) | none |
| Allocation into the managed heap, steady state | 36.5 MiB/s (prof4, before inline/escape build) | < 5 MiB/s | residual only |
| Barrier cost on the mutator | none | < 3 % frame time | none |
| Collector CPU | bursts | <= 10 % of one core, throttled | 0 |
| Leak budget: live managed bytes growth at steady state | unknown | flat after warm-up | < 1 MiB/min over a 60 min soak |

Targets, not results. Every row gets a measuring tool in phase 0 and is checked on every build after that.

## 1. Where we are (facts from the code)

* `monolith/rt/gc.rs`: segregated slab allocator (64 KiB slabs, 14 size classes 16..2048 B, large objects in a map),
  thread-local active slabs, stop-the-world mark-sweep, conservative stack and register scan, precise heap marking
  through `ref_offsets`, trigger at 32 MiB (`MONO_GC_MB`), pinned refs, registered static slots.
* Done: frame (stack) allocation of non-escaping objects, scope-bound iterator recycling, DataFixer patch (live heap -61 %),
  inliner + iteration-local escape analysis, `tickChunk` patch, allocation profiler with steady-state diff.
* Not done: any incremental or concurrent collection, any write barrier, owned (manually released) objects.

## 2. The four tiers

Every allocation ends up in exactly one tier. A tier is chosen by proof (analysis) or by a patch (declared owner), never by guess.

1. **Frame**: does not outlive the method. Stack slot. Free at return. (done, growing with inline/escape coverage)
2. **Region**: lives until the end of a known scope (tick, frame, packet). Bump arena, freed in one step. (arena exists; scope
   markers for tick/frame to be added; verified by the escape-violation checker)
3. **Owned**: has one owner with a defined release point (chunk, section, block entity, entity, GPU-side wrappers). Allocated
   outside the sweep set, traced through (it holds references to managed objects) but never swept; freed by `rt::release`
   when the owner dies. Patches declare ownership; debug mode poisons and quarantines released objects to catch use-after-release.
4. **Managed**: the residue. Collected by the idle collector until the leak budget lets us drop it.

Tiers 1-3 shrink tier 4. Tier 4 is the safety net, and it stays compiled (feature `gc`) until a soak test passes.

## 3. The idle collector (tier 4)

Like an engine at idle: always turning over a little, throttled, gives more throttle only when memory pressure rises.

* **Concurrent marking on a background thread** with snapshot-at-the-beginning (SATB). The game threads never mark.
  * Start of a cycle: one short stop (roots only): stacks and registers (conservative, as today), registered statics, pinned refs.
    This is the only pause; it is proportional to stack depth, not heap size.
  * Marking: atomic mark bits (epoch parity bit in the object flags, so no clear pass); worklist in the marker thread.
  * Allocation during a cycle is born marked.
  * Write barrier (SATB): while a cycle is active, a reference store pushes the overwritten reference into a thread-local buffer.
    Outside a cycle the barrier is one load of a global flag and a not-taken branch.
  * End of marking: short handshake to drain the thread-local buffers (no stack rescan needed under SATB).
* **Lazy sweeping**: no sweep pause. A slab is swept when an allocator refills from it (and by the marker thread when idle).
  Empty slabs go back to the empty pool; size-class free lists are rebuilt per slab.
* **Pacing**: a cycle starts when managed live bytes exceed `trigger = max(floor, live_after_last * 1.25)`. The marker runs with a duty cycle
  (sleep/yield between work chunks) targeting <= 10 % of one core. If allocation outruns marking (heap above `soft_limit`), the
  duty cycle rises, then mutators assist in bounded chunks (<= 200 us per allocation slow path). At `hard_limit` a full stop-the-world collection
  runs once, logs loudly, and counts as a failure of the leak budget.
* **Thread model**: Game threads register as today. The marker needs each thread's stack range only for the start snapshot.
* **No moving, no compaction.** Slabs keep size classes; fragmentation is watched by a metric (slab occupancy histogram), not fixed by copying.
* Generational collection is not planned. It needs a second barrier and a remembered set; the profile after tiers 1-3 decides whether it is ever worth it.

## 4. Codegen and runtime impact of the barrier

The translator owns every reference store, so the barrier is emitted, not patched in afterwards.

* Emitted at: `putfield`/`aastore` of references, `putstatic` of references, bulk copies (`System.arraycopy`, `Arrays.copyOf` family, `clone`) as a range barrier.
* Runtime/native/patch code must not store references with raw writes. `rt::set_ref`/`rt::set_ref_array` become the only API and carry the barrier. A grep-based
  check in `monolith/patch_tests.py` fails the build if a patch or native writes a reference field raw.
* Stores into frame/region objects and into objects born in the current cycle skip the barrier (analysis result), and constructors writing into a fresh object skip it.
* Because the barrier changes generated code, it is batched with other codegen changes into one full regeneration (section 9).

## 5. Owned memory (tier 3)

* API: `rt::alloc_owned(class)`, `rt::release(ref)`; owner types in patches (`owner: "chunk"` in the manifest binding) so release points are declared next to the patch.
* Candidate owners in order of expected payoff (measure first, do not guess): chunk sections and palettes (`PalettedContainer`, `BitStorage`, light data),
  chunk/section maps, entity lists, block entities, packet buffers, mesh buffers, texture/atlas data.
* Release points: chunk unload, entity removal, section rebuild, connection close.
* Safety: `MONO_OWNED_CHECK=1` poisons released memory and keeps it in quarantine for N frames; any access to a poisoned object aborts with the allocation site.
  Releasing an object still referenced elsewhere is the main risk; the check mode and a reachability audit (a debug full mark that reports owned objects still reachable after release) cover it.

## 6. Garbage reduction backlog (tiers 1-2), order from the profile

1. Finish measuring the inline + escape + `tickChunk` build (in progress). Compare class by class with prof4.
2. Value-like temporaries still heap-allocated: `Vec3`, `AABB`, `BlockPos` factories beyond `tickChunk`, `MutableBlockPos` copies, `VoxelShape` chains.
3. Strings: `StringBuilder`, `char[]`/`byte[]` growth (`Arrays.copyOf`), `String.format`/`concat`; replace hot paths by Rust patches that write into a reused region buffer.
4. Lambdas and captured closures at hot call sites: allocate in the frame when non-escaping (analysis extension), reuse non-capturing instances.
5. fastutil `clone`, collection resizing, stream pipelines on tick paths; DFU leftovers.
6. After each item: new steady-state profile, update `docs/ALLOC_BASELINE.md`.

## 7. Verification and tooling (built before the collector)

* **Pause/heap monitor** (phase 0): per-collection pause, longest pause, bytes allocated per second per tier, live bytes over time, slab occupancy. Output joins the profile JSON.
* **Leak budget tool**: scripted play (`tools/headless_play.sh`), samples live managed bytes every 10 s after warm-up, fits a slope, fails above budget. `MONO_GC=off` runs the same script with the collector disabled to show what tiers 1-3 alone achieve.
* **Collector tests** (fast, no Minecraft): small bytecode programs in `monolith/tests/gc/`: linked structures mutated while a cycle runs, multi-thread allocation storms, finalization-free cycles, large arrays, static roots, deep stacks. Run with a tiny trigger so cycles are constant.
* **Verify mode** `MONO_GC_VERIFY=1`: after each concurrent cycle, a stop-the-world full mark runs and the two live sets are compared; any object the STW mark reaches but the concurrent sweep freed aborts with the object's class and allocation site.
* **Region/frame checker**: `MONO_ARENA_CHECK=abort` stays on in all game runs.

## 8. Phases and exit criteria

| Phase | Work | Exit criterion |
|---|---|---|
| 0 | Pause/heap monitor, leak-budget tool, `MONO_GC=off` switch, baseline on the current build | numbers recorded in `docs/ALLOC_BASELINE.md` |
| 1 | Validate inline + escape + `tickChunk` build in the game | no arena violation, no crash in a 10 min run, alloc rate vs prof4 documented |
| 2 | Collector tests + verify mode scaffolding; tier-4 internals refactored (epoch mark bit, lazy sweep) on the existing STW collector | tests green, no behaviour change in game |
| 3 | Write barrier in codegen, runtime API, native/patch audit; SATB concurrent marker, pacing, assists, hard limit | verify mode clean for a 30 min run, pause < 1 ms, barrier cost < 3 % |
| 4 | Garbage backlog (section 6), region scopes for tick/frame | managed steady-state allocation < 5 MiB/s |
| 5 | Owned memory for the top owners from the profile | live managed heap and collector CPU drop measurably per owner |
| 6 | `MONO_GC=off` soak (60 min scripted play) | residual growth < 1 MiB/min; if met, collector off by default, kept behind feature `gc` as net; hard-limit event = fail |
| 7 | Platforms on the final runtime: Android `.so`, Windows/macOS windowing | each runs the leak script |

## 9. Build logistics (this decides the order)

A full Minecraft regeneration is ~10 min of analysis plus ~55 min of compile (32 shards, 2 parallel jobs); a runtime change invalidates all shard objects unless
`--reuse-objs` is valid (internals only, same ABI and same generated code). So:

* Collector internals (phase 2, most of phase 3's marker) are developed against the test programs and the small test builds, not the full game.
* Everything that changes generated code (barrier, region scope markers, owned-alloc calls, new patches) is collected and applied in as few full regenerations as possible.
* Never run generation, compile and game tests at the same time (memory limit).

## 10. Decisions fixed by this plan

* Collector design: concurrent SATB marking on a background thread, lazy non-moving sweep, no generational scheme for now.
* Pause budget: only the root snapshot; everything else is concurrent or amortized.
* Ownership is declared in patches, enforced in debug by poisoning, and never inferred silently.
* The collector is removed from the default path only after a passed soak; it stays in the tree behind a feature as the net.
* Order: measure, then shrink garbage, then make the net invisible; the barrier and marker go in one regeneration.

## 11. Risks

* A missed barrier (raw reference store in a native or patch) loses live objects under SATB: mitigated by the single store API, the grep check, and verify mode.
* Conservative stack scanning keeps some garbage alive: bounded, visible in the leak tool.
* Owned release bugs corrupt memory: poisoning/quarantine mode and the reachability audit; ship owned types one at a time.
* Fragmentation without compaction: slab occupancy metric; size-class tuning from the profile.
* Android: marker thread priority and big.LITTLE scheduling need measuring on a device (phase 7).

## Pre-registered leak gate (fixed before the measurement)

- Scenario: open the saved world, stand still (no walking, no new chunks), 60 minutes (`tools/leak_soak.sh --minutes 60`).
- Warm-up: the first 30 minutes are ignored (the world around the player settles in this window, measured in the first 60-minute soak).
- Gate: least-squares slope of live managed bytes over minutes 30-60, from `tools/leak_budget.py --warmup-s 1800 --budget-mb-per-min 1`.
- Pass: slope <= 1 MiB/min. Fail: anything above. The window is not changed after the run.
