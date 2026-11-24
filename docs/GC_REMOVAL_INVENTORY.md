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
3. Escape analysis (`--escape`) keeps non-escaping objects in frame slots (`rt::stack_obj`). Those never reach the
   collector. The runtime has a scope arena (`rt::alloc_arena`), but the generator does not emit calls to it
   (checked: `alloc_arena` appears in `rustgen.py` only in help text). The remaining work is the escaping allocations.

## Measured: generated code for `Stack1` with `--escape --inline`

Counted in the emitted Rust (static sites, not runtime counts):

| allocation | sites | route |
|---|---|---|
| `rt::stack_obj` | 77 | frame slot, never reaches the collector |
| `rt::alloc` | 4959 | collected heap |
| `rt::new_array` | 979 | collected heap |
| `rt::new_obj_array` | 945 | collected heap |
| `rt::alloc_recyclable` | 18 | collected heap, recycled per class |
| `alloc_arena` (generated) | 0 | scope arena is not emitted by the generator yet |

About 98% of allocation sites are still on the collected heap, even with escape analysis on. The scope arena
exists in the runtime (`rt::alloc_arena`) but the generator does not use it. The first real GC-removal step is
to make the generator emit arena allocations for every site the escape analysis already proves short-lived.
Escape analysis currently marks only 77 sites as frame-local; the rest escape, so the ownership model is the
real work.

## Next steps (not started)

- Measure how many escaping allocations a typical Minecraft-sized program makes (`--escape` statistics).
- Choose the ownership model for escaping objects and write it down before changing `gc.rs`.
- Replace the collector behind `heap.rs` one allocation class at a time, each with `tests/run_tests.py` green.
