# Patches: hand-written Zig for hot methods

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
