"""Sharded output: the program is split into N separately compiled Zig objects.

LLVM optimizing the whole Minecraft closure as one module needs far more memory than most machines
have. With --shards N the generated classes are partitioned (contiguous runs of the sorted class names,
balanced by bytecode size) into N directories, each compiled by its own `zig build-obj` (N-1 objects)
or `zig build-exe` (shard 0, which also holds main). Everything that crosses a shard boundary goes
through one table:

  * `syms` is a weakly exported array (rt.Shared) that all shards see as one object. Slot `base[c]`
    holds the *rt.Class of class c, `base[c]+1` its Statics; every reachable method has its own slot. Slots are
    persistent across builds (<out>/symbols.json).
  * The owner of a class fills its slots at start-up (`fill`), then patches the pointers that its class
    struct holds into other shards (super class, interfaces, inherited vtable/itable slots) in `link`.
  * A shard that uses a class living elsewhere imports a generated *facade* module instead of the class:
    same API as the class module (Layout, cast, Statics, cls(), sp(), init(), m_*), implemented over the
    table. Generated code is identical in both builds.
  * All run-time state lives in rt.Shared structs (weak symbols), thread-locals are mirrored (rt.setCur).
"""
from __future__ import annotations

import glob
import os
import re
import shutil

from .world import parse_method_desc, method_mname

TOK_C = re.compile(r"@@C\(([^)]*)\)@@")
TOK_M = re.compile(r"@@M\(([^|)]*)\|(.*?)\)@@")
REF = re.compile(r"\bP\.(C_[A-Za-z0-9_]+)")
ROOTREF = re.compile(r"\b(C_[A-Za-z0-9_]+)\b")
LAYOUT = re.compile(r"// @@LAYOUT-BEGIN\n(.*?)// @@LAYOUT-END\n", re.S)

RUNTIME_FILES = ["rt.zig", "mem_pool.zig", "thread_pool.zig", "rhi.zig", "rhi_vk.zig", "rhi_gl.zig",
                 "gl_shim.zig", "al_shim.zig", "world_render.zig", "gpu_instancing.zig", "rhi_bridge.zig", "natives.zig",
                 "natives_sys.zig", "io.zig", "jni.zig", "jni_call.zig", "jni_table.zig", "net.zig",
                 "nio.zig", "nio_ch.zig", "varhandle.zig"]


def patch_runtime(dst: str):
    """Shard runtime copies: thread-locals shared by all shards are extern symbols defined once (shared.c)."""
    p = os.path.join(dst, "rt.zig")
    t = open(p).read()
    for old, new in (("pub threadlocal var cur: *ThreadState = undefined;", "pub extern threadlocal var cur: *ThreadState;"),
                     ("threadlocal var processing_refs: bool = false;", "extern threadlocal var processing_refs: bool;"),
                     ("threadlocal var throw_depth: u32 = 0;", "extern threadlocal var throw_depth: u32;")):
        assert old in t, old
        t = t.replace(old, new, 1)
    open(p, "w").write(t)
    p = os.path.join(dst, "jni.zig")
    t = open(p).read()
    old = "threadlocal var attached: bool = false;"
    assert old in t
    open(p, "w").write(t.replace(old, "extern threadlocal var attached: bool;", 1))


_REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
# Java sources we maintain ourselves. Each of them is compiled as its own unit (a "dev unit"): one unit per top-level
# class of the class library overlay and of runtime/java, one per package of the no-LWJGL shims. Every other unit
# reaches them through the symbol table (facades), like separately compiled classes, and they are numbered last, so
# editing one of them recompiles only its own unit and moves no other symbol number.
DEV_SOURCE_ROOTS = [(os.path.join(_REPO, "classlib", "src"), "class"), (os.path.join(_REPO, "runtime", "java"), "class"),
                    (os.path.join(_REPO, "runtime", "java_nolwjgl"), "package")]
_dev_keys: dict[str, str] | None = None  # top-level class -> unit key


def _load_dev_keys() -> dict[str, str]:
    global _dev_keys
    if _dev_keys is None:
        _dev_keys = {}
        for root, grain in DEV_SOURCE_ROOTS:
            for dp, _, fs in os.walk(root):
                for f in fs:
                    if f.endswith(".java"):
                        top = os.path.relpath(os.path.join(dp, f), root)[:-5].replace(os.sep, "/")
                        _dev_keys[top] = top if grain == "class" else top.rsplit("/", 1)[0] + "/"
    return _dev_keys


def dev_unit_key(name: str) -> str | None:
    return _load_dev_keys().get(name.split("$", 1)[0])


def is_dev_class(name: str) -> bool:
    return dev_unit_key(name) is not None


def assign_shards(prog, names: list[str], n: int, base_n: int | None = None,
                  dev_unit: dict[str, int] | None = None) -> dict[str, int]:
    """Shards 0..base_n-1 hold the bulk (contiguous runs of the sorted names); dev_unit maps a dev unit key to its own shard."""
    if dev_unit:
        rest = [x for x in names if not is_dev_class(x)]
        out = _assign_balanced(prog, rest, base_n)
        for x in names:
            k = dev_unit_key(x)
            if k is not None:
                out[x] = dev_unit[k]
        return out
    return _assign_balanced(prog, names, n)


def _assign_balanced(prog, names: list[str], n: int) -> dict[str, int]:
    """Contiguous runs of the sorted class names (classes of one package stay together), balanced by
    the bytecode size of their reachable methods."""
    w = prog.w
    cost = {}
    for name in names:
        ci = w.classes[name]
        c = 64
        for m in ci.cf.methods:
            if (name, m.key) in prog.reachable and m.code is not None:
                c += 24 + len(m.code.code)
        cost[name] = c
    total = sum(cost.values())
    out = {}
    acc = 0
    for name in names:
        out[name] = min(n - 1, acc * n // max(total, 1))
        acc += cost[name]
    return out


def write_shards(gen, names: list[str], root_text: str, main_class: str, runtime_dir: str):
    prog = gen.prog
    w = prog.w
    N = gen.nshards
    src = gen.out
    shard_of = gen.shard_map
    if N > 256:  # rt.MAX_SHARDS: every shard registers its thread-local setter in a fixed table
        raise SystemExit(f"j2zig: {N} shards exceed rt.MAX_SHARDS (256)")
    names = sorted(names, key=lambda x: (is_dev_class(x) if N >= 3 else False, x))  # dev classes numbered last
    mname = {n: w.classes[n].mname for n in names}
    by_mname = {v: k for k, v in mname.items()}
    # ------------------------------------------------------------------ symbol numbering
    # Slots are persistent: <out>/symbols.json (next to src/) keeps the slot of every class and method of earlier
    # builds, new ones are appended and removed ones keep their slot unused. A change in one class therefore never
    # renumbers another class, and the incremental build recompiles only the shards whose own code changed.
    import json
    symfile = os.path.join(os.path.dirname(os.path.abspath(src)), "symbols.json")
    prev = {"next": 0, "slots": {}}
    if os.path.isfile(symfile):
        with open(symfile) as fh:
            prev = json.load(fh)
    slots: dict[str, int] = prev["slots"]
    nxt = prev["next"]

    def slot(key: str, width: int = 1) -> int:
        nonlocal nxt
        v = slots.get(key)
        if v is None:
            v = nxt
            slots[key] = v
            nxt += width
        return v

    base: dict[str, int] = {}
    methods: dict[str, list] = {}
    mslot: dict[str, dict[str, int]] = {}
    for n in names:
        ci = w.classes[n]
        ms = [m for m in ci.cf.methods if (n, m.key) in prog.reachable]
        methods[n] = ms
        base[n] = slot(mname[n], 2)  # class + statics
        mslot[n] = {m.key: slot(mname[n] + "." + method_mname(n, m.name, m.desc)) for m in ms}
    total = nxt
    with open(symfile + ".tmp", "w") as fh:
        json.dump({"next": nxt, "slots": slots, "shapes": gen.shapes}, fh)
    os.replace(symfile + ".tmp", symfile)

    def resolve(text: str) -> str:
        def c(m):
            return str(base[m.group(1)])

        def mm(m):
            return str(mslot[m.group(1)][m.group(2)])
        return TOK_M.sub(mm, TOK_C.sub(c, text))

    # ------------------------------------------------------------------ classes -> shard dirs
    dirs = []
    for k in range(N):
        d = os.path.join(src, f"shard{k}")
        os.makedirs(d, exist_ok=True)
        dirs.append(d)
    class_text: dict[str, str] = {}
    refs: dict[int, set[str]] = {k: set() for k in range(N)}
    own: dict[int, list[str]] = {k: [] for k in range(N)}
    for n in names:
        path = os.path.join(src, mname[n] + ".zig")
        with open(path) as fh:
            text = resolve(fh.read())
        os.remove(path)
        k = shard_of[n]
        class_text[n] = text
        own[k].append(n)
        refs[k] |= set(REF.findall(text))
        with open(os.path.join(dirs[k], mname[n] + ".zig"), "w") as fh:
            fh.write(text)
    # ------------------------------------------------------------------ root pieces
    com = []
    rest = []
    pos = 0
    for m in re.finditer(r"// @@COMMON-BEGIN\n(.*?)// @@COMMON-END\n", root_text, re.S):
        rest.append(root_text[pos:m.start()])
        com.append(m.group(1))
        pos = m.end()
    rest.append(root_text[pos:])
    common = "".join(com)
    mainpart = "".join(rest)
    # drop the generated class list and imports of the single-unit root: rebuilt per shard
    mainpart = re.sub(r"^// Generated by j2zig.*?\nconst std = @import\(\"std\"\);\nconst rt = @import\(\"rt.zig\"\);\n", "",
                      mainpart, flags=re.S, count=1)
    mainpart = re.sub(r"^pub const C_[A-Za-z0-9_]+ = @import\([^\n]*\n", "", mainpart, flags=re.M)
    mainpart = re.sub(r"pub var all_classes:.*?\n}\n", "", mainpart, count=1, flags=re.S)
    build_all = ("pub var all_classes: [%d]*rt.Class = undefined;\npub fn buildAllClasses() void {\n" % len(names) +
                 "".join(f"    all_classes[{i}] = @ptrCast(@alignCast(@constCast(syms[{base[n]}].?)));\n" for i, n in enumerate(names)) +
                 "}\n")
    root_refs_common = set(ROOTREF.findall(common)) | set(REF.findall(common))
    root_refs_main = set(ROOTREF.findall(mainpart)) | set(REF.findall(mainpart))
    rt_refs = set()
    for f in RUNTIME_FILES:
        with open(os.path.join(runtime_dir, f)) as fh:
            rt_refs |= set(REF.findall(fh.read()))
    # ------------------------------------------------------------------ facades
    def facade(n: str) -> str:
        text = class_text[n]
        m = LAYOUT.search(text)
        o = ["// Generated by j2zig: facade of " + n + " (class lives in another shard)\n"
             "const std = @import(\"std\");\nconst rt = @import(\"rt.zig\");\nconst P = @import(\"classes.zig\");\n"
             "const E = rt.E;\nconst Ref = rt.Ref;\n", m.group(1) if m else ""]
        o.append(f"pub fn cls() *rt.Class {{\n    return @ptrCast(@alignCast(@constCast(P.syms[{base[n]}].?)));\n}}\n")
        o.append(f"pub fn sp() *Statics {{\n    return @ptrCast(@alignCast(@constCast(P.syms[{base[n] + 1}].?)));\n}}\n")
        o.append("pub inline fn init() E!void {\n    return rt.ensureInit(cls());\n}\n")
        from .zigen import ZT_KIND, kind_of, zt_ret, fn_type, fp_call
        for k, mi in enumerate(methods[n]):
            args, ret = parse_method_desc(mi.desc)
            st = mi.is_static
            params = ([] if st else ["this: rt.Ref"]) + [f"a{i}: {ZT_KIND[kind_of(a)]}" for i, a in enumerate(args)]
            call = ([] if st else ["this"]) + [f"a{i}" for i in range(len(args))]
            o.append(f"pub inline fn {method_mname(n, mi.name, mi.desc)}({', '.join(params)}) E!{zt_ret(ret)} {{\n"
                     f"    const f: {fn_type(st, mi.desc)} = @ptrCast(@alignCast(P.syms[{mslot[n][mi.key]}].?));\n"
                     f"    return {fp_call(mi.desc, 'f(' + ', '.join(call) + ')')};\n}}\n")
        return "".join(o)

    patch_files = sorted(glob.glob(os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "patches", "*.zig")))
    patch_refs: set[str] = set()
    for pf in patch_files:
        with open(pf) as fh:
            patch_refs |= set(REF.findall(fh.read()))
    facade_cache: dict[str, str] = {}
    for k in range(N):
        d = dirs[k]
        for f in RUNTIME_FILES:
            shutil.copy(os.path.join(runtime_dir, f), d)
        patch_runtime(d)
        for pf in patch_files:
            shutil.copy(pf, d)
        need = (refs[k] | root_refs_common | rt_refs | patch_refs | (root_refs_main if k == 0 else set()))
        ownm = {mname[n] for n in own[k]}
        foreign = sorted(m for m in need if m in by_mname and m not in ownm)
        for fm in foreign:
            n = by_mname[fm]
            if n not in facade_cache:
                facade_cache[n] = facade(n)
            with open(os.path.join(d, fm + ".zig"), "w") as fh:
                fh.write(facade_cache[n])
        imports = "".join(f"pub const {m} = @import(\"{m}.zig\");\n" for m in sorted(ownm | set(foreign)))
        head = ("// Generated by j2zig — shard %d. Do not edit.\nconst std = @import(\"std\");\nconst rt = @import(\"rt.zig\");\n"
                # the table's length is not part of a shard's source: adding symbols (always at the end) leaves the
                # other shards' sources unchanged, so the incremental build reuses their objects (j2z_shared_syms in
                # shared.c has the real size)
                "pub const syms: [*]?*const anyopaque = @ptrCast(rt.Shared([1]?*const anyopaque, \"syms\", 8).ptr);\n" % (k,))
        body = common + ((mainpart + build_all) if k == 0 else "")
        with open(os.path.join(d, "classes.zig"), "w") as fh:
            fh.write(head + imports + body)
        # fill / link / exports
        o = ["const std = @import(\"std\");\nconst rt = @import(\"rt.zig\");\nconst P = @import(\"classes.zig\");\n",
             "pub fn fill() void {\n    rt.registerCurSetter(&rt.setCurLocal);\n"]
        for n in own[k]:
            m = mname[n]
            o.append(f"    P.syms[{base[n]}] = @ptrCast(P.{m}.cls());\n    P.syms[{base[n] + 1}] = @ptrCast(P.{m}.sp());\n")
            for j, mi in enumerate(methods[n]):
                o.append(f"    P.syms[{mslot[n][mi.key]}] = @ptrCast(&P.{m}.{method_mname(n, mi.name, mi.desc)}_r);\n")
        o.append("}\npub fn link() void {\n")
        for n in own[k]:
            if re.search(r"pub fn link\(\) void \{\n\}", class_text[n]):
                continue
            o.append(f"    P.{mname[n]}.link();\n")
        o.append("}\n")
        o.append(f"fn fillC() callconv(.c) void {{\n    fill();\n}}\nfn linkC() callconv(.c) void {{\n    link();\n}}\n"
                 f"comptime {{\n    @export(&fillC, .{{ .name = \"j2z_fill_{k}\" }});\n"
                 f"    @export(&linkC, .{{ .name = \"j2z_link_{k}\" }});\n}}\n")
        with open(os.path.join(d, "shardmode.zig"), "w") as fh:
            fh.write("pub const sharded = true;\n")
        with open(os.path.join(d, "shard.zig"), "w") as fh:
            fh.write("".join(o))
    # ------------------------------------------------------------------ main (shard 0)
    ext = "".join(f"extern fn j2z_fill_{k}() callconv(.c) void;\nextern fn j2z_link_{k}() callconv(.c) void;\n"
                  for k in range(1, N))
    fills = "".join(f"    j2z_fill_{k}();\n" for k in range(1, N))
    links = "".join(f"    j2z_link_{k}();\n" for k in range(1, N))
    with open(os.path.join(dirs[0], "main.zig"), "w") as fh:
        fh.write("const std = @import(\"std\");\nconst rt = @import(\"rt.zig\");\nconst P = @import(\"classes.zig\");\n"
                 "const S0 = @import(\"shard.zig\");\n" + ext +
                 "var android_started = std.atomic.Value(bool).init(false);\n"
                 "pub fn main() u8 {\n    const args = std.os.argv;\n    rt.initState();\n"
                 "    rt.st.argv_storage = rt.splitJavaOptions(if (args.len > 1) args[1..] else args[0..0]);\n"
                 "    rt.st.gc_verbose = rt.getenv(\"J2Z_GC_VERBOSE\") != null;\n"
                 "    rt.st.patch_verify = rt.getenv(\"J2Z_PATCH_VERIFY\") != null;\n"
                 "    S0.fill();\n" + fills + "    S0.link();\n" + links +
                 "    P.buildAllClasses();\n    rt.initRuntime(P.makeHooks());\n"
                 "    return rt.runMain(&P.entry);\n}\n"
                 "pub export fn j2z_android_entry(\n"
                 "    user_arg: ?[*:0]const u8,\n"
                 "    gdir_arg: ?[*:0]const u8,\n"
                 "    adir_arg: ?[*:0]const u8,\n"
                 "    _: ?*anyopaque,\n"
                 ") callconv(.c) i32 {\n"
                 "    if (android_started.swap(true, .seq_cst)) return 0;\n"
                 "    rt.initState();\n"
                 "    rt.st.argv_storage = rt.buildAndroidArgv(user_arg, gdir_arg, adir_arg);\n"
                 "    rt.st.gc_verbose = rt.getenv(\"J2Z_GC_VERBOSE\") != null;\n"
                 "    rt.st.patch_verify = rt.getenv(\"J2Z_PATCH_VERIFY\") != null;\n"
                 "    S0.fill();\n" + fills + "    S0.link();\n" + links +
                 "    P.buildAllClasses();\n    rt.initRuntime(P.makeHooks());\n"
                 "    return rt.runMainAsync(&P.entry);\n}\n")
    with open(os.path.join(src, "shared.c"), "w") as fh:
        fh.write("/* storage of the state shared by all shards (see rt.Shared) */\n"
                 "char j2z_shared_rt[1 << 20] __attribute__((aligned(64)));\n"
                 "char j2z_shared_jni[1 << 16] __attribute__((aligned(64)));\n"
                 "char j2z_shared_rhi[1 << 16] __attribute__((aligned(64)));\n"
                 f"char j2z_shared_syms[{total * 8}] __attribute__((aligned(64)));\n"
                 "/* thread-locals shared by all shards (a per-shard copy would see a different value per shard) */\n"
                 "__thread void *cur;\n__thread _Bool processing_refs;\n__thread unsigned throw_depth;\n"
                 "__thread _Bool attached;\n")
    with open(os.path.join(src, "shards.txt"), "w") as fh:
        fh.write(str(N) + "\n")
