# j2zig — Java bytecode → Zig transpiler: architecture

```
 app.jar + libs ─┐
 classlib overlay ├─► ClassPath ─► World (hierarchy, JVMS resolution, vtables/itables)
 java.base.jmod ─┘                   │
                                     ▼
                     Program: decode + invokedynamic desugaring + RTA
                                     │  (reachable methods, instantiated classes)
                                     ▼
                     ZigGen: one Zig module per class + classes.zig + main.zig
                                     │
 runtime/*.zig (GC-free pool, RHI, world culling, instancing, threads) ┴─► zig build-exe ─► native executable (no JVM)
```

## Front end (`j2zig/`)

| module | role |
|---|---|
| `classfile.py` | class file parser, versions 45–69 (Java 1.1 … 25): constant pool, fields, methods, Code, exception tables, BootstrapMethods, Signature (generics metadata), InnerClasses, NestHost/Members, Record, PermittedSubclasses, LineNumberTable |
| `bytecode.py` | decoder for all 202 opcodes (wide, tableswitch/lookupswitch normalised); `jsr/ret` are decoded and reported as unsupported |
| `classpath.py` | dirs, JARs (multi-release aware), JDK `.jmod` files |
| `world.py` | closed-world class hierarchy: JVMS §5.4.3 field/method resolution, §5.4.6 selection, maximally-specific default methods, vtable layout (with miranda/default slots), itable contents, prefix-compatible instance layouts |
| `indy.py` | **invokedynamic desugaring** into synthetic bytecode classes: `LambdaMetafactory.metafactory/altMetafactory` (captures, bridges, marker interfaces, boxing/unboxing/widening adaptation), `StringConcatFactory` (recipes, constants), `ObjectMethods.bootstrap` (record `toString/hashCode/equals` with the JDK's exact formulas and comparison order), `SwitchBootstraps.typeSwitch/enumSwitch`. Unknown bootstraps are reported and throw `BootstrapMethodError` like a JVM. |
| `program.py` | instruction preparation (indy rewrite, folding of disabled `assert`s) and **Rapid Type Analysis** with a `why`-chain for every reachable method |
| `zigen.py` | Zig code generation (below) |
| `analyze.py` | JAR statistics: versions, opcode histogram, indy bootstraps, reflection/Unsafe/VarHandle use, natives |

## Code generation scheme

* **Types**: `int/short/char/byte/boolean → i32` on the operand stack, fields keep their storage width
  (`i8/u16/i16`), `long → i64`, `float/double → f32/f64`, references → `rt.Ref = ?*rt.Object`.
* **Operand stack & locals**: abstract interpretation over the CFG computes the stack shape at every
  block entry; each `(depth, kind)` slot and `(local, kind)` pair becomes a Zig variable.
  Reference slots live in the per-call GC frame array `R` (precise roots).
* **Control flow**: basic blocks are prongs of a labeled `switch`; jumps are `continue :sw <pc>`
  (Zig compiles this to direct branches). Straight-line methods are emitted without the switch.
* **Arithmetic** follows JVMS exactly: wrapping `+% -% *%`, `idiv/irem` with `/ by zero` and
  `MIN_VALUE / -1`, masked shifts, saturating `f2i/d2l`, NaN-aware `fcmpl/fcmpg`.
* **Exceptions**: every method returns `rt.E!T`; the thrown object is pending in the thread
  state. Throwing operations inside a try-range use `catch continue :sw D`, where dispatch
  prong `D` tests the handlers in exception-table order. Runtime checks (null, bounds,
  casts, array stores, negative sizes, division) throw the same exception classes and
  messages as HotSpot.
* **Dispatch**: `invokevirtual` → vtable slot; `invokeinterface` → per-class itable;
  `invokespecial/static` → direct. Calls are devirtualized when the target is private/final,
  the class is final, or **class hierarchy analysis** over the instantiated classes proves a
  single possible target (sound in the closed world; any late change to the program triggers
  a full regeneration).
* **Liveness**: reference operand-stack slots consumed by a call are cleared afterwards, so
  stale slots do not keep garbage (or weakly referenced objects) alive.
* **Class initialization**: JVMS §5.5 (per-class state, recursive requests by the initializing
  thread, waiting threads, `ExceptionInInitializerError` wrapping, `NoClassDefFoundError` with
  the JDK 21 cause chain). Checks are elided where the JVMS guarantees initialization.
* **Synchronization**: `monitorenter/exit` and `synchronized` methods use a lazily allocated,
  reentrant per-object monitor (mutex + condition); `wait/notify/notifyAll` included.
* **volatile** fields use sequentially consistent atomics.
* **VarHandle**: `findVarHandle/findStaticVarHandle/arrayElementVarHandle` produce
  (kind, offset) descriptors; signature-polymorphic calls (`get`, `set*`, `compareAndSet`,
  `compareAndExchange*`, `getAndSet*`, `getAndAdd*`, `getAndBitwise*`) are lowered to atomic
  memory operations in `runtime/varhandle.zig` using the call-site descriptor.
* **Line numbers**: `F.line` is updated before instructions that can throw, so stack traces
  carry `File.java:NN` like the JVM.

## Runtime (`runtime/`)

* `mem_pool.zig` & `thread_pool.zig` — lock-free page-backed `ArenaAllocator` (`ArenaMark`,
  `madvise(MADV_DONTNEED)`), segregated `SlabPool` (`16..32768` B) with quiescent 3-epoch deferred
  reclamation (zero STW pauses), and condition-variable `WorkerPool` + `WaitGroup` (`0%` idle CPU spin).
* `rt.zig` — object header, arrays, class metadata, allocation via `SlabPool`/`ArenaAllocator`,
  wait-free `safepoint()` no-op, frame scoping (`beginFrame`/`endFrame`/`pushArenaScope`/`popArenaScope`),
  OS threads (`std.Thread`), and stack-overflow guard (`StackOverflowError`).
* `rhi.zig`, `rhi_vk.zig`, `rhi_gl.zig`, `rhi_bridge.zig` — Zero-LWJGL custom RHI (`auto` ->
  Vulkan 1.1+ -> strict OpenGL 3.3 Core TBO + `glMapBufferRange` for legacy integrated GPUs like
  Intel HD 5500 -> Headless command verification) plus `NativeRhi.java` JNI/native entry points.
* `world_render.zig` & `gpu_instancing.zig` — vanilla-safe `SectionSnapshot` 3-stage culling
  (6-plane FOV frustum, 6-face CaveCulling portal BFS, conservative `HiZPyramid`, pixel-coverage LOD)
  and unified `64B` `GpuInstance` / `32B` `GpuParticle` non-block instancing across display entities,
  mobs, players, items, chests, signs, and particles.
* `natives.zig` — native methods of the class library (Object, Class, System, Runtime,
  Thread, Throwable backtraces, Float/Double bits, reflect.Array, `MethodHandleNatives`,
  `NativeImageBuffer`, and `jdk.internal.misc.Unsafe` memory/CAS/park API).
* `varhandle.zig` — VarHandle access modes (field, static, array element, `byteArrayViewVarHandle`,
  and JDK 25 Foreign `MemorySegment` views).
* `io.zig` — `java.io` file streams, `RandomAccessFile`, `UnixFileSystem`, and
  `java.util.zip` (`CRC32`, `Adler32`, `Inflater`, `Deflater`) on top of the system zlib,
  which is also what the JDK uses — compressed output is byte-identical.

## Closed-world services

* `ServiceLoader`: providers are read from the application's `META-INF/services` files at
  build time, rooted in RTA, and compiled into a table of constructor thunks.
* `Class.forName("constant")`, class literals and `Lookup.ensureInitialized` keep the
  target class and its `<clinit>` alive (constant folding of reflective references).
* `Class.getDeclaredField/getField` produce `Field` objects (declaring class, name, type,
  modifiers) backed by field metadata tables, enough for `Unsafe`/`VarHandle` offsets.

## Class library strategy (`classlib/src`)

The real JDK `java.base` is transpiled as-is, except for classes whose implementation is
tied to HotSpot internals; those are replaced by overlay sources compiled with
`javac --patch-module java.base=…` (GraalVM-style substitutions): `Object`, `String`
(UTF-16 `char[]`), `StringBuilder/StringBuffer`, `Class`, `System`, `Runtime`, `Thread`,
`ThreadLocal`, `Throwable`, `StackTraceElement`, `Integer`, `Long`, `PrintStream`,
`java.lang.ref.*`, `java.util.concurrent.atomic.Atomic{Integer,Long,Boolean,Reference}`,
`java.lang.invoke.{MethodHandles,VarHandle}`, `jdk.internal.misc.{VM,CDS}`,
`jdk.internal.util.Preconditions`, `java.lang.CharacterName`.
Everything else (collections, streams, `java.util.concurrent`, `Math/StrictMath` (fdlibm),
`Double/Float.toString` (Raffaello Giulietti's algorithm), regex, …) is the JDK's own code,
which is what makes results bit-identical to the JVM.
