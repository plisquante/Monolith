# Monolith: Rust back end

Monolith replaces the Zig back end. Same front end (class loading, reachability, indy desugaring, class library
overlay), Rust output. Entry point: `python3 -m monolith.rustgen --cp <dir> --main <Class> -o <out> [--run] [--strict]`.
Tests: `python3 monolith/run_tests.py` (each `monolith/tests/*.java` runs on HotSpot and as Rust; stdout and exit
code must be identical).

## Layout
- `monolith/rustgen.py`: bytecode -> Rust. Whole-world: the entry point pulls in every reachable class and method of the
  application, the overlay and the JDK. Control flow is `loop { match pc { .. } }`, exceptions are `Result` with `?`,
  handlers re-dispatch on the faulting pc.
- `monolith/rt/rt.rs`: object model (16-byte header, `Class` tables: vtable, itab, interfaces, field table), arrays,
  strings, class mirrors, class initialization with locking, throw hooks.
- `monolith/rt/natives.rs`: native methods of the overlay (`n_<class>__<method>`).
- `monolith/rt/vh.rs`: VarHandle access modes. `monolith/rt/sync.rs`: monitors, threads, park, sleep, interrupt.

## Verified (identical to HotSpot)
Arithmetic and control flow, objects, inheritance, interfaces and default methods, arrays, static init, exceptions
(catch, finally, nested, causes), real `String`/`StringBuilder`/`PrintStream` from the class library, indy string
concat, lambdas, records, enums, sealed switch, collections, streams, `String.format`, number parsing, threads,
`synchronized` (blocks and methods), wait/notify, interrupt. Hello world compiles 187 classes / 793 methods;
a collections/streams program 1308 classes / 6031 methods.

## Known gaps (the generator lists untranslatable code and missing natives on stderr; reaching them stops the
process with exit 70 and a message, nothing is silently skipped)
- Memory: nothing is ever freed (no GC, no regions yet). Not usable for a long gameplay session.
- Reflection: `Class.newField0`, `fields0`, `methods0`, annotations, constant pool, `getCallerClass` is approximated by a
  per-call-site hint (only for methods that call it). Blocks `ExecutorService`, `CompletableFuture`, `ConcurrentHashMap`
  merge paths (see `monolith/pending/Mono7.java`).
- Stack traces: `Throwable.backtrace0` returns null (no frames).
- Reference arrays share one runtime class: no `ArrayStoreException`, `checkcast` to `T[]` is loose.
- ServiceLoader provider table, MemorySegment VarHandles, JNI, NIO, files, sockets, zip, management natives.
- Graphics/audio layer (RHI, GL shim, SDL shim) and the Android launcher entry are not ported.
- Class loader, module and resource handling of the Zig runtime are not ported.

## Native surface still to port
613 native entry points exist in the Zig runtime; about 130 are done. Largest groups: `NativeRhi` 159, UnixNativeDispatcher 49,
`sun.nio.ch.Net` 40, Class 25, Unsafe 24 (most done), UnixFileDispatcherImpl 19, UnixFileSystem 14, zip 20.

## Android library (`--android`)
`python3 -m monolith.rustgen --cp <jars> --main <Class> --no-lwjgl --android --shards 32 --shard-jobs 3 --opt 2 -o <dir>`
builds `libminecraft_mono.so` for `aarch64-linux-android`: `mono_rt.rlib`, one object per shard, `libmc_main.rlib`
(`pub fn mono_run`), and a generated `link_stub.rs` (cdylib root) that exports `mono_android_entry(user, gameDir,
assetsDir, input)`. The entry starts the game on its own 256 MiB thread and returns at once, so the Android UI thread
never blocks. The TouchCore host (`libtouchcore_mono.so`) supplies EGL, input and the surface through the `tc_android_*`
symbols that `natives_rhi.rs` resolves. JNA needs libffi, which is built once as a static library from the official
release (checksum pinned). `tools/mcbuild_android.py` wraps jar, library and asset download plus this command.
