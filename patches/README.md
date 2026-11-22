# Patches: hand-written Rust for hot methods (Rust back end)

The Rust back end reads `patches/rust/manifest.json` (implementation: `monolith/patches.py`). Nothing of Mojang's code
is stored here: a patch is a Rust function plus a manifest entry that names the method it replaces
(`class`, `method`, `desc`). The translator still translates the original (it stays available as
`r_<name>_orig`) and generates a wrapper in its place. This is how game logic moves to Rust method by method:
a replaced method whose original is no longer called disappears from the build.

## manifest.json

    {
      "files": {
        "bitstorage.rs": {
          "fields":  {"size": "net/minecraft/util/SimpleBitStorage.size"},   // F_size: usize, byte offset
          "statics": {"count": "pkg/Cls.count"},                              // s_count() -> *mut T
          "classes": {"Level": "net/minecraft/world/level/Level"},            // class_Level(), is_Level(o), init_Level()
          "calls":   {"helper": "pkg/Cls.helper(I)I"}                         // c_helper(args) -> rt::Res<R>
        }
      },
      "patches": [
        {"class": "pkg/Cls", "method": "get", "desc": "(I)I", "file": "bitstorage.rs", "fn": "get_value", "orig": true}
      ]
    }

The patch file is pasted into a module in the shard of the patched class, after the generated bindings, so it can
use `rt::` (the runtime), `mono_rt::patch::*` (`get`, `set`, `array`, `array_len`, `class_ptr`) and the bindings above.
A field declared in a superclass is found through the subclass named in the binding. `calls` are direct
(non-virtual) calls; the callee is compiled even if nothing else reaches it, and a static callee's class is
initialized first.

## Rules for patch functions

* Signature of the generated method: `pub fn f(this: rt::Ref, a0: i32, ...) -> rt::Res<R>` (no `this` for static
  methods). Return `Err` through `rt::throw(...)`, or hand the call to the original.
* `"orig": true`: the first parameter is the translated original (`orig: impl Fn(rt::Ref, i32) -> rt::Res<i32>`).
  Handle only the common case and pass every input the Java code would reject (null, index out of range, ...) to
  `orig`, which then throws exactly what the Java code throws. See `patches/rust/bitstorage.rs`.
* Same observable behaviour as the Java method, exceptions included.
* References held in locals are found by the collector (it scans stacks conservatively): no frames to register.
* `MONO_PATCH_VERIFY=1` runs original and patch on every call of a method that returns a primitive and aborts on the
  first difference (compared as raw bits). Methods with side effects must say `"verify": false` (both would run).
  Methods that return references or `void` are covered by differential tests: `python3 monolith/patch_tests.py`
  and `monolith/tests/patch/`.
* A patch whose class is not on the class path is skipped (the generator prints how many applied / were unused /
  skipped). A class that exists without the method stops the build, so a Minecraft update cannot silently
  disable a patch. `MONO_PATCH_MANIFEST=<file>` selects another manifest (tests).

---

# Patches: hand-written Zig for hot methods (legacy Zig back end)

Nothing of Mojang's code is stored here. A patch is a Zig function plus a manifest entry that names the
method it replaces (`class`, `method`, `desc`). The transpiler still translates the original bytecode (it stays
available as `<name>_orig`), and generates a wrapper in its place:

    pub fn m_x(this, a0, ...) E!R {
        if (rt.st.patch_verify) { run original and patch, compare, abort on mismatch; return original }
        return Patch.fn(this, a0, ...);
    }

## manifest.json entry

    { "class": "pkg/Cls", "method": "name", "desc": "(IF)D", "impl": "file.zig", "fn": "functionName" }

`impl` is a file in this directory; it is copied next to the generated classes and may import `rt.zig` and
`classes.zig` (as `P`) exactly like the generated code (`P.C_pkg_Cls.cast(obj).f_field`, `P.C_pkg_Cls.sp()`
for statics). The function must have the signature of the generated method:
`pub fn f(this: rt.Ref, a0: i32, ...) rt.E!R`, with `this` absent for static methods.

## Rules for patch functions

* Same observable behaviour as the Java method, including exceptions; throw with `rt.throwKind` or by calling
  the original (`P.C_pkg_Cls.m_x_hash_orig(...)` is not exported: delegate through the Java method instead).
* A patch that allocates or calls Java code must register its references like generated code (a `rt.Frame`
  with the reference slots). Pure numeric kernels need none: no frame, no allocation, no calls.
* Verification: `MONO_PATCH_VERIFY=1 ./run.sh` runs original and patch for every call of a method that returns a
  primitive and aborts on the first difference (value compared as raw bits). Methods that return references or
  `void`, or that have side effects, are covered by the differential tests in tests/java instead (write one
  that exercises the method and compare against HotSpot with `python3 tests/run_tests.py`).

## Fallback to the original ("orig": true)

With `"orig": true` in the manifest entry the patch function receives the translated original as its first
parameter (`orig: anytype`) and can handle only the common case: any input the Java code would reject
(null, index out of range, ...) is passed to `orig(this, args...)`, which throws exactly what the Java code throws.
See `bitstorage.zig`.

## JDK intrinsics

Independent of the manifest, `INTRINSICS` in `monolith/zigen.py` compiles calls of pure JDK statics
(`Math.floor/ceil/sqrt/abs/min/max/floorDiv/floorMod`, `Integer/Long.bitCount/numberOfLeadingZeros/...`,
raw bit casts of Float/Double) to inline helpers in `runtime/rt.zig` (`iFloor`, ...). Tested bit for bit
against HotSpot by `tests/java/Intrinsics.java`. Known difference: an ArithmeticException thrown by
`floorDiv/floorMod` no longer has a `Math.floorDiv` frame in its stack trace.
