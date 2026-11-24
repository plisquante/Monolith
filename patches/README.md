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
* `"fingerprint"`: 16 hex digits identifying the bytecode of the method the patch was written against (decoded instructions,
  exception table, frame sizes; independent of constant-pool numbering and line numbers). When Minecraft changes that body the
  build stops with a message naming both values. After reviewing the patch against the new bytecode, store the new value with
  `python3 tools/patch_fingerprints.py --cp <client.jar:libs...> --write`. Without `--write` the tool only reports
  (`ok`, `MISMATCH`, `no value`, `skipped`). All patches in the manifest carry one. Tests: `python3 -m monolith.patch_fp_tests`.
