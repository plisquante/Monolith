# GC removal: inventory

Status: analysis only. No behaviour changed. The tracing collector is still `monolith/rt/gc.rs` (about 1490 lines),
reached through `monolith/rt/heap.rs`.

## Where objects are allocated (runtime, GC heap entry points)

Call sites of `alloc_obj`, `alloc_arena`, `alloc_recyclable`, `new_str`, `new_array`, `new_obj_array`, `alloc_zeroed`:

| file | sites |
|---|---|
| `rt.rs` | 28 (object and array construction, string literals, class mirrors) |
| `natives_sys.rs` | 22 (process, time, system properties) |
| `gc.rs` | 19 (the collector itself) |
| `natives_net.rs` | 18 |
| `natives_reflect.rs` | 15 |
| `natives_io.rs` | 14 |
| `natives.rs` | 10 |
| `natives_jni.rs` | 8 |
| `natives_rhi.rs` | 5 |
| `natives_nio.rs` | 2 |
| total | 141 |

Most natives allocate a Java `String` or an array for a return value. Those values are owned by the caller
and die with it, which is the easy case for an ownership model. The hard cases are objects that are stored in
other objects or in statics.

## Roots the collector scans

- Static slots: `register_static_slot` (rt.rs:632), one per Java static field of reference type.
- Array classes: `register_array_class` (rt.rs:724), so their element classes stay alive.
- Pinned references: `pin_ref` (rt.rs:615 for class statics, rt.rs:1314 for a shadow-stack exception that
  must survive until `init_failure` is reported).
- Thread stacks: scanned conservatively between `register_thread` and `unregister_thread` (gc.rs:424, 451);
  thread objects are set with `set_thread_obj` (rt.rs:1024, sync.rs:259).

## What a replacement has to decide

1. Ownership of Java objects: who frees an object that is referenced from a field and no longer from any stack.
   Options: reference counting with `Arc`-like handles (cycles leak unless broken explicitly), an arena per
   class-loader lifetime (never freed, but bounded by program size), or handles into a slab with explicit
   release at the points the Java semantics name (`WeakReference` clearing, `Cleaner`, `finalize` is not supported).
2. Stack roots: the conservative stack scan goes away only if every reference held in a Rust local is owned by a
   handle type, which the generated code does not do today.
3. Escape analysis (`--escape`) keeps non-escaping objects in frame slots (`rt::stack_obj`), and non-escaping arrays in
   the method's arena (`rt::new_array_arena`, `rt::new_obj_array_arena`, released by `rt::ArenaGuard` when the method
   returns). Neither reaches the collector. Loop allocations whose object survives one iteration but not the method
   are not put in the arena yet: an arena lives until the method returns, so a long loop would keep every iteration's
   object alive. Those need a per-iteration or per-scope reset first.

## Measured: generated code for `Stack1` with `--escape --inline`

Counted in the emitted Rust (static sites, not runtime counts):

| allocation | sites | route |
|---|---|---|
| `rt::stack_obj` | 78 | frame slot, never reaches the collector |
| `rt::new_array_arena` | 14 | method arena, released at return |
| `rt::new_obj_array_arena` | 38 | method arena, released at return |
| `rt::alloc` | 4972 | collected heap |
| `rt::new_array` | 977 | collected heap |
| `rt::new_obj_array` | 930 | collected heap |
| `rt::alloc_recyclable` | 18 | collected heap, recycled per class |

Counts from the generated `Stack1` output (`--escape --inline`, `--opt 1`, one run). The earlier note that the
arena is not emitted was wrong for arrays: 52 arena sites are already emitted. Objects have no arena route: the
`new` path emits `stack_obj` or the heap, never `alloc_arena`.

About 98% of emitted allocation sites (6897 of 7027) are still on the collected heap, even with escape analysis on. The generator message
under `--escape` reports 223 frame or arena sites (it counts all analysed methods, including ones the output does
not reach). The loop region candidates on the closure without `--inline` were 4 of 1513 classified sites; they
have not been counted with `--inline` yet. Escaping sites are the bulk, so the ownership model for escaping
objects is the real work.

## Next steps (not started)

- Measure how many escaping allocations a typical Minecraft-sized program makes (`--escape` statistics). The
  `--escape` message now prints the frame and arena site count (it was a count of everything not escaping, which
  included loop sites that are not frame-local).
- Count the loop region candidates (survive an iteration, not the method) with `--inline` on Stack1 and on a
  Minecraft-sized program, before deciding whether a per-iteration reset is worth building.
- Choose the ownership model for escaping objects and write it down before changing `gc.rs`.
- Replace the collector behind `heap.rs` one allocation class at a time, each with `tests/run_tests.py` green.
