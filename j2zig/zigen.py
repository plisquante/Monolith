"""Zig code generation.

Translation scheme (per method):
  * the JVM operand stack and local variables are statically typed per instruction
    (abstract interpretation over the CFG); every (depth, kind) stack slot and every
    (index, kind) local becomes a Zig variable, reference-typed ones live in the GC
    frame array `R` (precise roots, see runtime/rt.zig);
  * basic blocks become prongs of a labeled `switch`; jumps are `continue :sw <pc>`
    (Zig lowers this to direct branches);
  * exceptions: every method returns `rt.E!T`; the thrown object is pending in the
    thread state. Throwing operations inside a try-range use `catch continue :sw D`
    where D is a dispatch prong that tests the handler list in exception-table order.
"""
from __future__ import annotations

import os
import re
import struct
from typing import Optional

from .bytecode import Insn, is_terminal
from .classfile import ACC_FINAL, ACC_SYNCHRONIZED, ClassConst, MemberRef, MethodInfo
from .program import RT_EXCEPTIONS, Program
from .world import (ClassInfo, short_hash, LinkError, field_mname, kind_of, mangle, method_mname,
                    parse_method_desc, slots_of)

ZT_FIELD = {"Z": "i8", "B": "i8", "C": "u16", "S": "i16", "I": "i32", "J": "i64",
            "F": "f32", "D": "f64"}
from .reflectmeta import (CompactPool, inner_info, rewrite_annotations, rewrite_default,
                          rewrite_param_annotations, zbytes, zpool)

ZT_KIND = {"I": "i32", "J": "i64", "F": "f32", "D": "f64", "A": "rt.Ref"}
STORE_CONV = {"Z": "rt.z", "B": "rt.b8", "C": "rt.c16", "S": "rt.s16"}


def zt_field(desc: str) -> str:
    return ZT_FIELD.get(desc[0], "rt.Ref") if len(desc) == 1 or desc[0] not in "L[" else "rt.Ref"


def zt_ret(desc: str) -> str:
    return "void" if desc == "V" else ZT_KIND[kind_of(desc)]


def zstr(s: str) -> str:
    """Zig string literal from a Python str (UTF-8)."""
    out = []
    for ch in s.encode("utf-8", "surrogatepass"):
        if 32 <= ch < 127 and ch not in (34, 92):
            out.append(chr(ch))
        else:
            out.append(f"\\x{ch:02x}")
    return '"' + "".join(out) + '"'


def utf16_units(s: str) -> list[int]:
    out = []
    for ch in s:
        c = ord(ch)
        if c >= 0x10000:
            c -= 0x10000
            out.append(0xD800 + (c >> 10))
            out.append(0xDC00 + (c & 0x3FF))
        else:
            out.append(c)
    return out


def fn_type(static: bool, desc: str) -> str:
    """Type of a method's address (vtable/itable slots, reflection, cross-shard symbols): the <name>_r function.
    Value-returning methods return rt.Res(T) (value and error flag in registers), void methods rt.E!void."""
    args, ret = parse_method_desc(desc)
    ps = ([] if static else ["rt.Ref"]) + [ZT_KIND[kind_of(a)] for a in args]
    r = "rt.E!void" if ret == "V" else f"rt.Res({zt_ret(ret)})"
    return f"*const fn ({', '.join(ps)}) {r}"


def fp_call(desc: str, call: str) -> str:
    """Expression of type rt.E!T for a call through a method pointer of fn_type(...)."""
    ret = parse_method_desc(desc)[1]
    return call if ret == "V" else f"rt.unR({zt_ret(ret)}, {call})"


def native_name(cls: str, name: str, desc: str, overloaded: bool) -> str:
    n = f"J_{mangle(cls)}__{mangle(name)}"
    if overloaded:
        n += "__" + mangle(desc[1:desc.index(")")])
    return n


# Native "call a C function pointer" trampolines (LWJGL's org.lwjgl.system.JNI.invoke*/call*:
# arguments..., then the function address as the last long). They are compiled into direct
# C-ABI calls: no JNI, no marshalling, no extra native library.
FFI_TRAMPOLINE_CLASSES = {"org/lwjgl/system/JNI"}
C_ARG = {"Z": "u8", "B": "i8", "C": "u16", "S": "i16", "I": "i32", "J": "i64", "F": "f32", "D": "f64"}


def ffi_trampoline(cls: str, m, args: list[str], ret: str) -> Optional[str]:
    if cls not in FFI_TRAMPOLINE_CLASSES or not m.is_static or not (m.name.startswith("invoke") or m.name.startswith("call")):
        return None
    if not args or args[-1] != "J":
        return None
    ctypes, conv = [], []
    for i, a in enumerate(args[:-1]):
        if a.startswith("["):
            ctypes.append("usize")
            conv.append(f"(if (a{i}) |o| @intFromPtr(rt.arrayData(u8, rt.asArray(o))) else 0)")
        elif a in C_ARG:
            ctypes.append(C_ARG[a])
            conv.append(f"@as(u8, @intCast(a{i} & 1))" if a == "Z" else
                        f"@as(u16, @truncate(@as(u32, @bitCast(a{i}))))" if a == "C" else
                        f"@as({C_ARG[a]}, @truncate(a{i}))" if a in "BS" else f"a{i}")
        else:
            return None
    # The thread is in the "native" (GC-safe) state during the call, like a JNI call: a
    # blocking native function (e.g. vkQueuePresentKHR waiting for vsync) never stalls a
    # collection requested by another thread. Arrays passed by address are not moved (the GC
    # is non-moving) and stay reachable through the caller's frame.
    if ret == "V":
        cret, wrap = "void", "rt.enterSafe();\n    {0};\n    rt.leaveSafe();\n    return;"
    elif ret == "Z":
        cret, wrap = "u8", "rt.enterSafe();\n    const r = {0};\n    rt.leaveSafe();\n    return @as(i32, r & 1);"
    elif ret in "BCS":
        cret, wrap = C_ARG[ret], "rt.enterSafe();\n    const r = {0};\n    rt.leaveSafe();\n    return @as(i32, r);"
    elif ret in C_ARG:
        cret, wrap = C_ARG[ret], "rt.enterSafe();\n    const r = {0};\n    rt.leaveSafe();\n    return r;"
    else:
        return None
    fn_t = f"*const fn ({', '.join(ctypes)}) callconv(.c) {cret}"
    fptr = f"a{len(args) - 1}"
    call = f"f({', '.join(conv)})"
    return (f"    const f: {fn_t} = @ptrFromInt(@as(usize, @bitCast({fptr})));\n"
            f"    {wrap.format(call)}\n")


JDK_PREFIXES = ("java/", "javax/", "jdk/", "sun/", "com/sun/crypto/", "com/sun/security/")
JNI_C = {"Z": "u8", "B": "i8", "C": "u16", "S": "i16", "I": "i32", "J": "i64", "F": "f32", "D": "f64"}


def jni_mangle(s: str) -> str:
    out = []
    for ch in s:
        if ch.isalnum() and ch.isascii():
            out.append(ch)
        elif ch == "/":
            out.append("_")
        elif ch == "_":
            out.append("_1")
        elif ch == ";":
            out.append("_2")
        elif ch == "[":
            out.append("_3")
        else:
            out.append(f"_0{ord(ch):04x}")
    return "".join(out)


def jni_call(ci, m, args: list[str], ret: str, static: bool, head: str, overloaded: bool) -> str:
    """Call the real JNI implementation (Java_<class>_<method>) in a native library."""
    short = f"Java_{jni_mangle(ci.name)}_{jni_mangle(m.name)}"
    long = short + "__" + jni_mangle(m.desc[1:m.desc.index(")")])
    slot = f"JNI_{short}_{short_hash(ci.name + m.name + m.desc)}"
    ctypes = ["jni.EnvPtr", "jni.jobject"]
    conv = ["jni.env()", "jself"]
    for i, a in enumerate(args):
        if a[0] in "L[":
            ctypes.append("jni.jobject")
            conv.append(f"@ptrCast(a{i})")
        else:
            ctypes.append(JNI_C[a])
            conv.append(f"@as(u8, @intCast(a{i} & 1))" if a == "Z" else
                        f"@as(u16, @truncate(@as(u32, @bitCast(a{i}))))" if a == "C" else
                        f"@as({JNI_C[a]}, @truncate(a{i}))" if a in "BS" else f"a{i}")
    if ret == "V":
        cret, back = "void", "return;"
    elif ret[0] in "L[":
        cret, back = "jni.jobject", "return @ptrCast(@alignCast(r));"
    elif ret == "Z":
        cret, back = "u8", "return @as(i32, r & 1);"
    elif ret in "BCS":
        cret, back = JNI_C[ret], "return @as(i32, r);"
    else:
        cret, back = JNI_C[ret], "return r;"
    selfexpr = "@ptrCast(try rt.classMirror(&class))" if static else "@ptrCast(this)"
    body = (f"    const fp = try jni.resolve(&{slot}, \"{short}\", \"{long}\");\n"
            f"    const f: *const fn ({', '.join(ctypes)}) callconv(.c) {cret} = @ptrCast(@alignCast(fp));\n"
            f"    const jself: jni.jobject = {selfexpr};\n"
            "    const mark = jni.begin();\n"
            f"    {'const r = ' if cret != 'void' else ''}f({', '.join(conv)});\n"
            "    try jni.end(mark);\n"
            f"    {back}\n")
    return f"var {slot}: ?*const anyopaque = null;\n" + head + body + "}\n"


class CodegenError(Exception):
    pass


class ClassGen:
    """Generates one Zig module for one class."""

    def __init__(self, gen: "ZigGen", ci: ClassInfo):
        self.g = gen
        self.p = gen.prog
        self.w = gen.prog.w
        self.ci = ci
        self.lits: dict[str, int] = {}
        self.out: list[str] = []
        self.link: list[str] = []       # run-time patches of foreign pointers (sharded builds)
        self.shard = gen.shard_of(ci.name) if gen.nshards else 0
        self._foreign_tok: Optional[str] = None

    def lit(self, s: str) -> str:
        k = self.lits.get(s)
        if k is None:
            k = len(self.lits)
            self.lits[s] = k
        return f"rt.lit(&L{k}, &LD{k})"

    # ------------------------------------------------------------- class refs
    def cref(self, name: str) -> Optional[str]:
        """Zig expression of type *rt.Class for an internal name or array descriptor."""
        if name.startswith("["):
            inner = name[1:]
            if len(inner) == 1:
                return f"rt.arrayOf(rt.primByChar('{inner}'))"
            if inner.startswith("L"):
                e = self.cref(inner[1:-1])
            else:
                e = self.cref(inner)
            return None if e is None else f"rt.arrayOf({e})"
        if name not in self.p.loaded:
            ci = self.p.load(name)
            if ci is None:
                return None
            self.g.late_loaded.add(name)
        return f"P.{self.w.classes[name].mname}.cls()"

    def generate(self) -> str:
        ci = self.ci
        w = self.w
        o = self.out
        o.append("// Generated by j2zig from " + ci.name + " — do not edit.\n")
        o.append("const std = @import(\"std\");\nconst rt = @import(\"rt.zig\");\n"
                 "const P = @import(\"classes.zig\");\nconst VH = @import(\"varhandle.zig\");\nconst jni = @import(\"jni.zig\");\n")
        o.append("const E = rt.E;\nconst Ref = rt.Ref;\n")
        # instance layout
        fields = [] if ci.is_interface else w.instance_fields(ci)
        o.append("// @@LAYOUT-BEGIN\n")
        o.append("pub const Layout = extern struct {\n    hdr: rt.Object,\n")
        for decl, f in fields:
            o.append(f"    {field_mname(decl, f.name, w)}: {zt_field(f.desc)} = "
                     f"{'null' if zt_field(f.desc) == 'rt.Ref' else '0'},\n")
        o.append("};\n")
        o.append("pub inline fn cast(o: *rt.Object) *Layout {\n    return @ptrCast(@alignCast(o));\n}\n")
        # statics
        statics = [f for f in ci.cf.fields if f.is_static]
        o.append("pub const Statics = struct {\n")
        for f in statics:
            zt = zt_field(f.desc)
            init = "null" if zt == "rt.Ref" else "0"
            cv = f.constant_value
            if cv is not None and zt != "rt.Ref":
                init = self.const_literal(cv, f.desc)
            o.append(f"    {field_mname(ci.name, f.name)}: {zt} = {init},\n")
        o.append("};\n// @@LAYOUT-END\npub var S: Statics = .{};\n")
        # methods
        methods_code = []
        for m in ci.cf.methods:
            if (ci.name, m.key) in self.p.reachable:
                methods_code.append(self.method(m))
        # vtable / itable
        inst = ci.name in self.p.instantiated
        thrower_code = []
        vt_entries = []
        it_entries = []
        it_arrays = []
        if inst:
            for idx, s in enumerate(w.vtable(ci)):
                vt_entries.append(self.slot_entry(s.name, s.desc, s.impl, thrower_code, f"v{idx}"))
                if self._foreign_tok:
                    self.link.append(f"vtable[{idx}] = {self._foreign_tok};")
            for n_ii, ii in enumerate(w.all_interfaces(ci)):
                if ii.name not in self.p.loaded:
                    continue
                ents = []
                toks = []
                for k, im in enumerate(w.iface_methods(ii)):
                    sel = w.select(ci, im.name, im.desc)
                    impl = (sel[0].name, sel[1]) if sel else None
                    # thrower names number the interfaces within this class: the global interface id would rename
                    # them (and recompile their shard) whenever any interface is added to the program
                    ents.append(self.slot_entry(im.name, im.desc, impl, thrower_code, f"i{n_ii}_{k}"))
                    toks.append(self._foreign_tok)
                ii_foreign = self.foreign(ii.name)
                if any(toks):
                    arr = f"it_methods_{len(it_arrays)}"
                    it_arrays.append(f"var {arr} = [_]*const anyopaque{{ {', '.join(ents)} }};\n")
                    for k, tk in enumerate(toks):
                        if tk:
                            self.link.append(f"{arr}[{k}] = {tk};")
                    methods_expr = f"&{arr}"
                else:
                    methods_expr = f"&.{{ {', '.join(ents)} }}"
                if ii_foreign:
                    self.link.append(f"itable[{len(it_entries)}].iface = {self.class_tok(ii.name)};")
                    it_entries.append(f".{{ .iface = undefined, .methods = {methods_expr} }}")
                else:
                    it_entries.append(f".{{ .iface = P.{ii.mname}.cls(), .methods = {methods_expr} }}")
        o.extend(it_arrays)
        o.extend(thrower_code)
        # clinit wrapper
        clinit = ci.cf.method("<clinit>", "()V")
        has_clinit = clinit is not None and (ci.name, clinit.key) in self.p.reachable
        str_consts = [f for f in statics if f.constant_value is not None and zt_field(f.desc) == "rt.Ref"]
        need_init_fn = has_clinit or str_consts
        if need_init_fn:
            o.append("fn clinitWrapper() E!void {\n")
            for f in str_consts:
                cv = f.constant_value
                if isinstance(cv, tuple) and cv[0] == "string":
                    o.append(f"    S.{field_mname(ci.name, f.name)} = try {self.lit(cv[1])};\n")
            if has_clinit:
                o.append(f"    try {method_mname(ci.name, '<clinit>', '()V')}();\n")
            o.append("}\n")
        # enum values hook
        enum_vals = None
        if ci.name in self.p.enum_classes:
            vm = ci.cf.method("values", f"()[L{ci.name};")
            if vm and (ci.name, vm.key) in self.p.reachable:
                enum_vals = method_mname(ci.name, "values", vm.desc)
        # class object
        iface_list = [x for x in w.all_interfaces(ci) if x.name in self.p.loaded]
        ifaces = [f"P.{x.mname}.cls()" for x in iface_list]
        if any(self.foreign(x.name) for x in iface_list):
            o.append(f"var ifaces_arr: [{len(iface_list)}]*rt.Class = undefined;\n")
            for k, x in enumerate(iface_list):
                self.link.append(f"ifaces_arr[{k}] = {self.class_tok(x.name)};")
            self.link.append("class.interfaces = &ifaces_arr;")
            ifaces = []
        refoffs = [f"@offsetOf(Layout, \"{field_mname(d, f.name, w)}\")"
                   for d, f in fields if zt_field(f.desc) == "rt.Ref"
                   and not (d == "java/lang/ref/Reference" and f.name == "referent")]
        srefs = [f"&S.{field_mname(ci.name, f.name)}" for f in statics if zt_field(f.desc) == "rt.Ref"]
        sup = f"P.{ci.super.mname}.cls()" if ci.super and not ci.is_interface else "null"
        if ci.super and not ci.is_interface and self.foreign(ci.super.name):
            sup = "null"
            self.link.append(f"class.super = {self.class_tok(ci.super.name)};")
        pool = CompactPool(ci.cf.cp)

        def keep_ann(desc):
            return isinstance(desc, str) and desc.startswith("L") and desc[1:-1] in self.p.loaded

        def fextra(f):
            return (f", .signature = {zstr(f.signature) if f.signature else 'null'}, .annotations = "
                    f"{zbytes(rewrite_annotations(f.attributes.get('RuntimeVisibleAnnotations'), pool, keep_ann))}")
        fmeta = [f".{{ .name = {zstr(f.name)}, .offset = @offsetOf(Layout, \"{field_mname(d, f.name, w)}\"), .flags = {f.access}, .desc = {zstr(f.desc)}{fextra(f)} }}"
                 for d, f in fields if d == ci.name]
        mmeta = []
        for m in ci.cf.methods:
            if m.name == "<clinit>":
                continue
            fp = "null"
            shape = 0
            if (ci.name, m.key) in self.p.reachable and not m.is_abstract:
                fp = f"@ptrCast(&{method_mname(ci.name, m.name, m.desc)}_r)"
                shape = self.g.shape_id(m.is_static, m.desc)
            ma = m.attributes
            extra = (f", .signature = {zstr(m.signature) if m.signature else 'null'}"
                     f", .annotations = {zbytes(rewrite_annotations(ma.get('RuntimeVisibleAnnotations'), pool, keep_ann))}"
                     f", .param_annotations = {zbytes(rewrite_param_annotations(ma.get('RuntimeVisibleParameterAnnotations'), pool, keep_ann))}"
                     f", .annotation_default = {zbytes(rewrite_default(ma.get('AnnotationDefault'), pool))}")
            if m.exceptions:
                extra += f", .exceptions = &.{{ {', '.join(zstr(x) for x in m.exceptions)} }}"
            mmeta.append(f".{{ .name = {zstr(m.name)}, .desc = {zstr(m.desc)}, .flags = {m.access}, "
                         f".fp = {fp}, .shape = {shape}{extra} }}")
        sf_entries = [f".{{ .name = {zstr(f.name)}, .ptr = @ptrCast(&S.{field_mname(ci.name, f.name)}), .flags = {f.access}, .desc = {zstr(f.desc)}{fextra(f)} }}" for f in statics]
        cf = ci.cf
        cann = rewrite_annotations(cf.attributes.get("RuntimeVisibleAnnotations"), pool, keep_ann)
        ie = inner_info(cf)
        declared = [e.inner for e in cf.inner_classes if e.outer == cf.name and e.inner and e.inner in self.p.loaded]
        em = cf.enclosing_method
        refl = []
        if cf.signature:
            refl.append(f".signature = {zstr(cf.signature)}")
        if cann:
            refl.append(f".annotations = {zbytes(cann)}")
        if pool.entries:
            refl.append(f".cpool = {zpool(pool, zstr)}")
        if ie is not None:
            refl.append(f".is_inner = true, .outer = {zstr(ie.outer) if ie.outer else 'null'}, "
                        f".inner_name = {zstr(ie.name) if ie.name else 'null'}, .inner_flags = {ie.access}")
        if em is not None:
            refl.append(f".enclosing_class = {zstr(em[0])}, .enclosing_name = {zstr(em[1]) if em[1] else 'null'}, "
                        f".enclosing_desc = {zstr(em[2]) if em[2] else 'null'}")
        if declared:
            refl.append(f".declared_classes = &.{{ {', '.join(zstr(x) for x in declared)} }}")
        if getattr(cf, "nest_host", None):
            refl.append(f".nest_host = {zstr(cf.nest_host)}")
        if refl:
            o.append(f"const refl = rt.ClassReflect{{ {', '.join(refl)} }};\n")
        o.append(f"const static_fields = [_]rt.StaticFieldMeta{{ {', '.join(sf_entries)} }};\n")
        kw = "var" if self.g.nshards else "const"
        o.append(f"{kw} vtable = [_]*const anyopaque{{ {', '.join(vt_entries)} }};\n")
        o.append(f"{kw} itable = [_]rt.IEntry{{ {', '.join(it_entries)} }};\n")
        o.append("pub var class: rt.Class = .{\n"
                 f"    .name = {zstr(ci.name)},\n"
                 f"    .kind = {'.interface' if ci.is_interface else '.instance'},\n"
                 f"    .flags = {ci.cf.access},\n"
                 f"    .super = {sup},\n"
                 f"    .has_default = {'true' if ci.is_interface and any(not m.is_static and not m.access & 0x0400 and not m.name.startswith('<') for m in ci.cf.methods) else 'false'},\n"
                 f"    .interfaces = &.{{ {', '.join(ifaces)} }},\n"
                 "    .size = @sizeOf(Layout),\n"
                 f"    .ref_offsets = &.{{ {', '.join(refoffs)} }},\n"
                 "    .vtable = &vtable,\n    .itable = &itable,\n"
                 f"    .clinit = {'&clinitWrapper' if need_init_fn else 'null'},\n"
                 f"    .static_refs = &.{{ {', '.join(srefs)} }},\n"
                 f"    .enum_values = {('&' + enum_vals + '_r') if enum_vals else 'null'},\n"
                 f"    .fields = &.{{ {', '.join(fmeta)} }},\n"
                 f"    .static_fields = &static_fields,\n"
                 f"    .methods = &.{{ {', '.join(mmeta)} }},\n"
                 f"    .refl = {'&refl' if refl else 'null'},\n"
                 f"    .boot = {'true' if w.cp.is_boot(ci.name) else 'false'},\n"
                 f"    .is_ref = {'true' if self.is_reference() else 'false'},\n"
                 f"    .init_state = {'0' if need_init_fn or (ci.super and not ci.is_interface) else '2'},\n"
                 "};\n")
        o.append("pub inline fn init() E!void {\n    return rt.ensureInit(&class);\n}\n")
        o.append("pub fn cls() *rt.Class {\n    return &class;\n}\n")
        if self.g.nshards:
            o.append("pub fn link() void {\n" + "".join(f"    {x}\n" for x in self.link) + "}\n")
        o.append("pub fn sp() *Statics {\n    return &S;\n}\n")
        o.extend(methods_code)
        for s, k in self.lits.items():
            u = utf16_units(s)
            o.append(f"var L{k}: rt.Ref = null;\nconst LD{k} = [_]u16{{ {', '.join(map(str, u))} }};\n")
        return "".join(o)

    def is_reference(self) -> bool:
        return any(x.name == "java/lang/ref/Reference" for x in self.ci.supers())

    def foreign(self, name: str) -> bool:
        return bool(self.g.nshards) and self.g.shard_of(name) != self.shard

    def class_tok(self, name: str) -> str:
        """Expression of the *rt.Class of a class living in another shard (resolved at link time)."""
        return f"@as(*rt.Class, @ptrCast(@alignCast(@constCast(P.syms[@@C({name})@@].?))))"

    def slot_entry(self, name, desc, impl, thrower_code, tag) -> str:
        p = self.p
        self._foreign_tok = None
        if impl is not None and (impl[0], impl[1].name + impl[1].desc) in p.reachable \
                and not impl[1].is_abstract:
            ic = self.w.classes[impl[0]]
            if self.foreign(impl[0]):
                self._foreign_tok = f"P.syms[@@M({impl[0]}|{impl[1].name + impl[1].desc})@@].?"
                return "@ptrCast(&rt.pruned)"
            return f"@ptrCast(&P.{ic.mname}.{method_mname(impl[0], impl[1].name, impl[1].desc)}_r)"
        if (name + desc) in p.vcalls:
            # callable slot without implementation -> AbstractMethodError (JVMS 6.5)
            args, ret = parse_method_desc(desc)
            params = ["this: rt.Ref"] + [f"a{i}: {ZT_KIND[kind_of(a)]}" for i, a in enumerate(args)]
            disc = "".join(f" _ = a{i};" for i in range(len(args)))
            msg = zstr(f"{self.ci.name.replace('/', '.')}.{name}{desc}")
            if ret == "V":
                thrower_code.append(f"fn ame_{tag}({', '.join(params)}) E!void {{\n"
                                    f"    _ = this;{disc}\n    try rt.abstractMethod({msg});\n}}\n")
            else:
                thrower_code.append(f"fn ame_{tag}({', '.join(params)}) rt.Res({zt_ret(ret)}) {{\n"
                                    f"    _ = this;{disc}\n    rt.abstractMethod({msg}) catch {{}};\n"
                                    f"    return rt.toR({zt_ret(ret)}, error.Throw);\n}}\n")
            return f"@ptrCast(&ame_{tag})"
        return "rt.pruned"

    def const_literal(self, cv, desc: str) -> str:
        t, v = cv
        if t == "int":
            if desc == "Z":
                return str(v & 1)
            if desc == "C":
                return str(v & 0xFFFF)
            if desc == "B":
                return str(((v + 128) & 0xFF) - 128)
            if desc == "S":
                return str(((v + 32768) & 0xFFFF) - 32768)
            return str(v)
        if t == "long":
            return str(v)
        if t == "float":
            return f"@as(f32, @bitCast(@as(u32, 0x{struct.unpack('>I', v)[0]:08x})))"
        if t == "double":
            return f"@as(f64, @bitCast(@as(u64, 0x{struct.unpack('>Q', v)[0]:016x})))"
        raise CodegenError(f"bad constant {cv!r}")

    # ------------------------------------------------------------- methods
    def method(self, m: MethodInfo) -> str:
        """Value-returning methods come in three pieces: <name>_b (translated body returning E!T),
        <name>_r (callable through vtables/itables/cross-shard symbols returning register-packed rt.Res(T)),
        and inline <name> (direct same-shard call wrapper). Void methods alias <name>_r = <name>."""
        text = self._method_text(m)
        args, ret = parse_method_desc(m.desc)
        name = method_mname(self.ci.name, m.name, m.desc)
        if ret == "V":
            return text + f"pub const {name}_r = {name};\n"
        zt = zt_ret(ret)
        params = ", ".join(([] if m.is_static else ["this: rt.Ref"]) +
                           [f"a{i}: {ZT_KIND[kind_of(a)]}" for i, a in enumerate(args)])
        call = ", ".join(([] if m.is_static else ["this"]) + [f"a{i}" for i in range(len(args))])
        key = f"pub fn {name}("
        if text.count(key) != 1:
            raise CodegenError(f"{self.ci.name}.{m.name}{m.desc}: expected exactly one definition of {name}")
        text = text.replace(key, f"fn {name}_b(", 1)
        return text + (f"pub fn {name}_r({params}) rt.Res({zt}) {{\n    return rt.toR({zt}, {name}_b({call}));\n}}\n"
                       f"pub inline fn {name}({params}) E!{zt} {{\n    return rt.unR({zt}, {name}_r({call}));\n}}\n")

    def _method_text(self, m: MethodInfo) -> str:
        ci = self.ci
        args, ret = parse_method_desc(m.desc)
        static = m.is_static
        params = ([] if static else ["this: rt.Ref"]) + \
                 [f"a{i}: {ZT_KIND[kind_of(a)]}" for i, a in enumerate(args)]
        name = method_mname(ci.name, m.name, m.desc)
        head = f"pub fn {name}({', '.join(params)}) E!{zt_ret(ret)} {{\n"
        meta = (f"const MI_{name} = rt.MethodMeta{{ .cls = &class, .class = {zstr(ci.name)}, .name = {zstr(m.name)}, "
                f".desc = {zstr(m.desc)}, .file = {zstr(ci.cf.source_file or '')} }};\n")
        if m.is_native:
            overloaded = sum(1 for x in ci.cf.methods if x.name == m.name and x.is_native) > 1
            nn = native_name(ci.name, m.name, m.desc, overloaded)
            call_args = ([] if static else ["this"]) + [f"a{i}" for i in range(len(args))]
            if nn in self.g.natives:
                mod = self.g.natives[nn]
                return head + f"    return @import(\"{mod}\").{nn}({', '.join(call_args)});\n}}\n"
            tramp = ffi_trampoline(ci.name, m, args, ret)
            if tramp is not None:
                return head + tramp + "}\n"
            if not ci.name.startswith(JDK_PREFIXES):
                return jni_call(ci, m, args, ret, static, head, overloaded)
            self.g.report("missing-native", f"{ci.name}.{m.name}{m.desc} ({nn})")
            disc = "".join(f"    _ = {a};\n" for a in call_args)
            return head + disc + f"    try rt.unsatisfiedLink({zstr(ci.name.replace('/', '.') + '.' + m.name + m.desc)});\n}}\n"
        if m.code is None:
            disc = "".join(f"    _ = {a};\n" for a in (([] if static else ["this"]) + [f"a{i}" for i in range(len(args))]))
            return head + disc + f"    try rt.abstractMethod({zstr(ci.name + '.' + m.name)});\n}}\n"
        body = MethodCompiler(self, m).compile()
        text = meta + head + body + "}\n"
        pe = self.g.patches.get((ci.name, m.name, m.desc))
        if pe is not None:
            self.g.patches_used.add((ci.name, m.name, m.desc))
            text = re.sub(r"\b" + re.escape(name) + r"\b", name + "_orig", text)
            mod = "patch_" + re.sub(r"\W", "_", pe[0][:-4]) + "_" + short_hash(pe[0])
            call = ", ".join(([] if static else ["this"]) + [f"a{i}" for i in range(len(args))])
            pcall = (f"{name}_orig, " + call) if pe[2] else call      # "orig": the patch gets the original as fallback
            seen = self.__dict__.setdefault("_patch_mods", set())
            imp = "" if mod in seen else f"const {mod} = @import({zstr(pe[0])});\n"
            seen.add(mod)
            wrapper = (imp +
                       f"pub fn {name}({', '.join(params)}) E!{zt_ret(ret)} {{\n")
            if ret != "V" and ret[0] not in "L[":
                wrapper += (f"    if (rt.st.patch_verify) {{\n"
                            f"        const a = try {name}_orig({call});\n"
                            f"        const b = try {mod}.{pe[1]}({pcall});\n"
                            f"        if (!rt.patchSame(a, b)) rt.patchMismatch({zstr(ci.name + '.' + m.name + m.desc)});\n"
                            f"        return a;\n    }}\n")
            wrapper += f"    return {mod}.{pe[1]}({pcall});\n}}\n"
            text += wrapper
        return text


class MethodCompiler:
    def __init__(self, cg: ClassGen, m: MethodInfo):
        self.cg = cg
        self.p = cg.p
        self.w = cg.w
        self.ci = cg.ci
        self.m = m
        self.insns = self.p.insns(self.ci, m)
        self.code = m.code
        self.vars: dict[str, str] = {}
        self.lines: list[str] = []
        self.dry = True
        self.maxd = 4096
        self.depth_seen = 0
        self.dead = False
        self.disp: Optional[int] = None
        ln = sorted(m.code.line_numbers) if m.code else []
        self.line_at: dict[int, int] = {}
        if ln:
            j = 0
            for ins in self.insns:
                while j + 1 < len(ln) and ln[j + 1][0] <= ins.pc:
                    j += 1
                if ln[j][0] <= ins.pc:
                    self.line_at[ins.pc] = ln[j][1]
        self.last_line = -1
        self.pending_line: Optional[int] = None
        self._uid_n = 0
        self.local_allocs: set[int] = set()
        self.leaf = self._is_leaf()
        self.lazy = False      # frame only on the throwing path (see rt.nnL & co)
        self.cur_line = 0
        self.line_in_check = False
        if not self.leaf and self._is_leaf2():
            self.leaf = True
            self.lazy = True

    def _uid(self) -> int:
        self._uid_n += 1
        return self._uid_n

    def flush_line(self):
        if self.pending_line is not None and not self.leaf and not self.dry and not self.dead:
            self.emit(f"F.line = {self.pending_line};")
            self.last_line = self.pending_line
            self.pending_line = None

    # A leaf cannot throw, allocate, call, loop, or reach a safepoint: it needs no shadow-stack
    # frame (nothing can observe it: no stack trace is taken and no GC runs while it executes), so
    # its locals stay in registers and LLVM can inline it.
    _LEAF_BASE = {
        "aconst_null", "bipush", "sipush", "iinc", "iload", "lload", "fload", "dload", "aload", "istore",
        "lstore", "fstore", "dstore", "astore", "pop", "pop2", "dup", "dup_x1", "dup_x2", "dup2", "dup2_x1",
        "dup2_x2", "swap", "goto", "tableswitch", "ireturn", "lreturn", "freturn", "dreturn", "areturn",
        "return", "nop", "getfield", "putfield"}
    LEAF_PUSH = {"iload", "lload", "fload", "dload", "aload", "bipush", "sipush", "aconst_null"}

    def _is_leaf(self) -> bool:
        m = self.m
        code = m.code
        if code is None or code.handlers or m.access & (ACC_SYNCHRONIZED | 0x0100) or len(self.insns) > 60:
            return False
        ins = self.insns
        LEAF_OPS = (set(ARITH) - {"idiv", "irem", "ldiv", "lrem"}) | set(CONV) | set(IF1) | set(IF2) | self._LEAF_BASE
        for i, x in enumerate(ins):
            op = x.op
            if op.startswith(("iconst_", "lconst_", "fconst_", "dconst_")):
                continue
            if op == "ldc":
                if not (isinstance(x.arg, tuple) and x.arg[0] in ("int", "long", "float", "double")):
                    return False
                continue
            if op == "invokestatic":
                r = self.p.resolve(x.arg)
                if r is None:
                    return False
                idc, im = r
                it = INTRINSICS.get((idc.name, im.name + im.desc))
                if it is None or it[1] or parse_method_desc(im.desc)[1] == "V":
                    return False
                continue
            if op not in LEAF_OPS:
                return False
            if x.targets and any(t <= x.pc for t in x.targets):
                return False
            if op == "astore" and x.arg == 0 and not m.is_static:
                return False
            if op == "getfield":
                if m.is_static or i == 0 or ins[i - 1].op != "aload" or ins[i - 1].arg != 0:
                    return False
            if op == "putfield":
                if (m.is_static or i < 2 or ins[i - 2].op != "aload" or ins[i - 2].arg != 0 or not (
                        ins[i - 1].op in self.LEAF_PUSH or ins[i - 1].op.startswith(
                            ("iconst_", "lconst_", "fconst_", "dconst_")))):
                    return False
            if op in ("getfield", "putfield"):
                try:
                    self.w.resolve_field(x.arg.owner, x.arg.name, x.arg.desc)
                except LinkError:
                    return False
        return True

    LEAF2_THROWING = {"iaload", "laload", "faload", "daload", "aaload", "baload", "caload", "saload", "iastore",
                      "lastore", "fastore", "dastore", "aastore", "bastore", "castore", "sastore", "arraylength",
                      "idiv", "irem", "ldiv", "lrem", "getfield", "putfield", "getstatic", "putstatic", "instanceof"}

    def _is_leaf2(self) -> bool:
        """Like _is_leaf, but the method may also throw NPE / index / arithmetic errors: no call, no allocation,
        no loop, no handler, no monitor; statics only of its own class (no class-initialization call)."""
        m = self.m
        code = m.code
        if code is None or code.handlers or m.access & (ACC_SYNCHRONIZED | 0x0100) or len(self.insns) > 120:
            return False
        LEAF_OPS = set(ARITH) | set(CONV) | set(IF1) | set(IF2) | self._LEAF_BASE | self.LEAF2_THROWING
        for x in self.insns:
            op = x.op
            if op.startswith(("iconst_", "lconst_", "fconst_", "dconst_")):
                continue
            if op == "ldc":
                if not (isinstance(x.arg, tuple) and x.arg[0] in ("int", "long", "float", "double")):
                    return False
                continue
            if op == "invokestatic":
                r = self.p.resolve(x.arg)
                if r is None:
                    return False
                idc, im = r
                it = INTRINSICS.get((idc.name, im.name + im.desc))
                if it is None or it[1] or parse_method_desc(im.desc)[1] == "V":
                    return False
                continue
            if op not in LEAF_OPS:
                return False
            if x.targets and any(t <= x.pc for t in x.targets):
                return False
            if op in ("getfield", "putfield", "getstatic", "putstatic"):
                try:
                    dc, f = self.w.resolve_field(x.arg.owner, x.arg.name, x.arg.desc)
                except LinkError:
                    return False
                if op in ("getstatic", "putstatic") and dc is not self.ci:
                    return False
        return True

    def _lazy_expr(self, expr: str) -> str:
        m = re.match(r"rt\.(nn|arrLen|aload|astore|bastore|aastore|idiv|irem|ldiv|lrem)\(", expr)
        if not m or not expr.endswith(")"):
            raise CodegenError(f"{self.ci.name}.{self.m.name}: unexpected checked expression in a lazy-frame method: {expr}")
        meta = f"MI_{method_mname(self.ci.name, self.m.name, self.m.desc)}"
        return f"rt.{m.group(1)}L(" + expr[m.end():-1] + f", &{meta}, {self.cur_line})"

    # ------------------------------------------------------------- naming
    def sv(self, d: int, k: str) -> str:
        self.depth_seen = max(self.depth_seen, d + 1)
        n = f"s{d}{k.lower()}"
        self.vars[n] = ZT_KIND[k]
        return n

    def lv(self, n: int, k: str) -> str:
        v = f"l{n}{k.lower()}"
        self.vars[v] = ZT_KIND[k]
        return v

    def emit(self, s: str):
        if not self.dry and not self.dead:
            self.lines.append("        " + s + "\n")

    # Instructions whose only throwing operations are the null/bounds/zero checks of rt.nn, rt.aload, ...: they
    # need no `F.line = N` store before them; the check passes the line to its cold path instead.
    SELF_THROWING = {"iaload", "laload", "faload", "daload", "aaload", "baload", "caload", "saload", "iastore",
                     "lastore", "fastore", "dastore", "aastore", "bastore", "castore", "sastore", "arraylength",
                     "idiv", "irem", "ldiv", "lrem", "getfield", "putfield"}

    def _frame_expr(self, expr: str) -> str:
        m = re.match(r"rt\.(nn|arrLen|aload|astore|bastore|aastore|idiv|irem|ldiv|lrem)\(", expr)
        if not m or not expr.endswith(")"):
            return expr
        return f"rt.{m.group(1)}F(" + expr[m.end():-1] + f", &F, {self.cur_line})"

    def chk(self, expr: str) -> str:
        if self.lazy:
            expr = self._lazy_expr(expr)
        elif self.line_in_check:
            fexpr = self._frame_expr(expr)
            if fexpr != expr:
                expr = fexpr
            else:
                self.flush_line()
        else:
            self.flush_line()
        if self.disp is None:
            return f"try {expr}"
        return f"({expr} catch continue :sw {self.disp})"

    def throw_stmt(self, expr: str):
        """Emit a statement that never completes normally and mark the block dead."""
        self.emit(self.chk(expr) + ";")
        self.dead = True

    def report(self, kind, msg):
        if not self.dry:
            self.cg.g.report(kind, msg)

    # ------------------------------------------------------------- CFG
    def compile(self) -> str:
        insns = self.insns
        idx = {ins.pc: i for i, ins in enumerate(insns)}
        leaders = {0}
        for i, ins in enumerate(insns):
            if ins.targets:
                leaders.update(ins.targets)
                if i + 1 < len(insns):
                    leaders.add(insns[i + 1].pc)
            elif is_terminal(ins.op) and i + 1 < len(insns):
                leaders.add(insns[i + 1].pc)
        for h in self.code.handlers:
            leaders.add(h.handler_pc)
        self.leaders = leaders
        self.idx = idx
        # handler coverage: pc -> tuple of handlers
        self.cover: dict[int, tuple] = {}
        disp_ids: dict[tuple, int] = {}
        end_pc = (insns[-1].pc + insns[-1].length) if insns else 0
        base = end_pc + 1
        for ins in insns:
            hs = tuple((h.catch_type, h.handler_pc) for h in self.code.handlers
                       if h.start_pc <= ins.pc < h.end_pc)
            if hs:
                if hs not in disp_ids:
                    disp_ids[hs] = base + len(disp_ids)
                self.cover[ins.pc] = disp_ids[hs]
        self.disp_ids = disp_ids
        # pass 1: stack inference
        states: dict[int, list[str]] = {0: []}
        work = [0]
        for h in self.code.handlers:
            if h.handler_pc not in states:
                states[h.handler_pc] = ["A"]
                work.append(h.handler_pc)
        order = []
        while work:
            b = work.pop()
            if b in order:
                continue
            order.append(b)
            st = list(states[b])
            for succ, sst in self.run_block(b, st):
                if succ not in states:
                    states[succ] = sst
                    work.append(succ)
                elif [k for k in states[succ]] != sst:
                    raise CodegenError(f"{self.ci.name}.{self.m.name}{self.m.desc}: stack mismatch at "
                                       f"{succ}: {states[succ]} vs {sst}")
        self.states = states
        self.local_allocs = self._analyze_local_allocs()
        # pass 2: emit
        self.maxd = self.depth_seen
        self.dry = False
        self.vars = {}
        blocks = sorted(states)
        prongs = []
        for b in blocks:
            self.lines = []
            self.dead = False
            st = list(states[b])
            list(self.run_block(b, st))
            prongs.append(f"    {b} => {{\n" + "".join(self.lines) + "    },\n")
        for hs, did in disp_ids.items():
            lines = ["        const ex = rt.pending();\n        _ = &ex;\n"]
            caught_all = False
            for ct, hpc in hs:
                if ct is None or ct == "java/lang/Throwable":
                    self.vars["s0a"] = "rt.Ref"
                    lines.append(f"        s0a = rt.takePending();\n        continue :sw {hpc};\n")
                    caught_all = True
                    break
                ce = self.cg.cref(ct) if ct in self.p.loaded or self.p.load(ct) else None
                if ce is None:
                    continue
                self.vars["s0a"] = "rt.Ref"
                lines.append(f"        if (rt.isSubclass(ex.class, {ce})) {{\n"
                             f"            s0a = rt.takePending();\n            continue :sw {hpc};\n        }}\n")
            if not caught_all:
                lines.append("        return error.Throw;\n")
            if caught_all and len(lines) == 2:
                lines = lines[1:]
            prongs.append(f"    {did} => {{\n" + "".join(lines) + "    },\n")
        # assemble
        args, ret = parse_method_desc(self.m.desc)
        static = self.m.is_static
        out = []
        if len(self.insns) > 150:
            out.append(f"    @setEvalBranchQuota({20 * len(self.insns) + 1000});\n")
        if not self.leaf:
            out.append(f"    var F = rt.Frame{{ .meth = &MI_{method_mname(self.ci.name, self.m.name, self.m.desc)} }};\n")
            out.append("    const _j2z_ts = try rt.pushFrameT(&F);\n    defer rt.popFrameT(_j2z_ts, &F);\n")
        # parameters -> locals
        slot = 0
        pre = []
        if not static:
            pre.append(f"    {self.lv(0, 'A')} = this;\n")
            slot = 1
        for i, a in enumerate(args):
            k = kind_of(a)
            pre.append(f"    {self.lv(slot, k)} = a{i};\n")
            slot += slots_of(a)
        for n, t in sorted(self.vars.items()):
            init_val = "null" if t == "rt.Ref" else "0"
            out.append(f"    var {n}: {t} = {init_val};\n    _ = &{n};\n")
        out.extend(pre)
        if self.m.access & ACC_SYNCHRONIZED:
            if static:
                out.append("    const lock_obj = try rt.classMirror(&class);\n")
            else:
                out.append("    const lock_obj = this;\n")
            out.append("    try rt.monitorEnter(lock_obj);\n    defer rt.monitorExitNoThrow(lock_obj);\n")
        if all("continue :sw" not in pr for pr in prongs):
            prongs = prongs[:1]  # nothing jumps: only the entry block can execute
        if self.local_allocs:
            out.append("    const _j2z_am = rt.arenaMark();\n    defer rt.arenaRestore(_j2z_am);\n")
        if len(prongs) == 1 and "continue :sw" not in prongs[0]:
            # straight-line method: no dispatch loop needed
            body = prongs[0].split("\n", 1)[1].rsplit("    },\n", 1)[0]
            out.append("".join(l[4:] + "\n" for l in body.split("\n") if l))
            return "".join(out)
        out.append("    sw: switch (@as(u32, 0)) {\n")
        out.extend(prongs)
        out.append("        else => unreachable,\n    }\n")
        return "".join(out)

    def run_block(self, b: int, st: list[str]):
        """Simulate/emit block starting at pc b. Yields (successor pc, stack state)."""
        i = self.idx[b]
        insns = self.insns
        self.last_line = -1
        while True:
            ins = insns[i]
            self.disp = self.cover.get(ins.pc)
            line = self.line_at.get(ins.pc)
            if line is not None:
                self.cur_line = line
                if line != self.last_line and not self.leaf:
                    self.pending_line = line
            self.line_in_check = bool(ins.op in self.SELF_THROWING and not self.lazy and not self.leaf)
            succs = self.insn(ins, st)
            self.line_in_check = False
            if succs is not None:
                for s in succs:
                    yield s, list(st)
                return
            i += 1
            if i >= len(insns):
                return
            nxt = insns[i].pc
            if nxt in self.leaders:
                if not self.dead:
                    self.emit(f"continue :sw {nxt};")
                yield nxt, list(st)
                return

    # ------------------------------------------------------------- helpers
    def pop(self, st) -> tuple[str, str]:
        k = st.pop()
        return self.sv(len(st), k), k

    def push(self, st, k) -> str:
        st.append(k)
        return self.sv(len(st) - 1, k)

    def ins_arith(self, st, k, expr_fmt, throws=False, n=2):
        vals = [self.pop(st)[0] for _ in range(n)][::-1]
        e = expr_fmt.format(*vals)
        out = self.push(st, k)
        self.emit(f"{out} = {self.chk(e) if throws else e};")

    # ------------------------------------------------------------- instructions
    def insn(self, ins: Insn, st: list[str]):
        op = ins.op
        a = ins.arg
        emit = self.emit
        if op == "nop":
            return None
        # ---- constants
        if op == "aconst_null":
            emit(f"{self.push(st, 'A')} = null;")
            return None
        if op.startswith("iconst_"):
            v = -1 if op == "iconst_m1" else int(op[-1])
            emit(f"{self.push(st, 'I')} = {v};")
            return None
        if op.startswith("lconst_"):
            emit(f"{self.push(st, 'J')} = {op[-1]};")
            return None
        if op.startswith("fconst_"):
            emit(f"{self.push(st, 'F')} = {op[-1]}.0;")
            return None
        if op.startswith("dconst_"):
            emit(f"{self.push(st, 'D')} = {op[-1]}.0;")
            return None
        if op in ("bipush", "sipush"):
            emit(f"{self.push(st, 'I')} = {a};")
            return None
        if op == "ldc":
            return self.ldc(a, st)
        # ---- locals
        if op in ("iload", "lload", "fload", "dload", "aload"):
            k = LK[op[0]]
            src = self.lv(a, k)
            emit(f"{self.push(st, k)} = {src};")
            return None
        if op in ("istore", "lstore", "fstore", "dstore", "astore"):
            k = LK[op[0]]
            v, vk = self.pop(st)
            if vk != k and not (k == "A"):
                raise CodegenError(f"store kind mismatch {op} {vk}")
            emit(f"{self.lv(a, vk)} = {v};")
            return None
        if op == "iinc":
            emit(f"{self.lv(a, 'I')} +%= {ins.arg2};")
            return None
        # ---- arrays
        if op in ("iaload", "laload", "faload", "daload", "aaload", "baload", "caload", "saload"):
            idx = self.pop(st)[0]
            arr = self.pop(st)[0]
            t = {"i": "i32", "l": "i64", "f": "f32", "d": "f64", "a": "rt.Ref", "b": "i8",
                 "c": "u16", "s": "i16"}[op[0]]
            k = {"i": "I", "l": "J", "f": "F", "d": "D", "a": "A", "b": "I", "c": "I", "s": "I"}[op[0]]
            out = self.push(st, k)
            emit(f"{out} = {self.chk(f'rt.aload({t}, {arr}, {idx})')};")
            return None
        if op in ("iastore", "lastore", "fastore", "dastore", "aastore", "bastore", "castore", "sastore"):
            v = self.pop(st)[0]
            idx = self.pop(st)[0]
            arr = self.pop(st)[0]
            c = op[0]
            if c == "a":
                e = f"rt.aastore({arr}, {idx}, {v})"
            elif c == "b":
                e = f"rt.bastore({arr}, {idx}, {v})"
            elif c == "c":
                e = f"rt.astore(u16, {arr}, {idx}, rt.c16({v}))"
            elif c == "s":
                e = f"rt.astore(i16, {arr}, {idx}, rt.s16({v}))"
            else:
                t = {"i": "i32", "l": "i64", "f": "f32", "d": "f64"}[c]
                e = f"rt.astore({t}, {arr}, {idx}, {v})"
            emit(self.chk(e) + ";")
            return None
        # ---- stack manipulation
        if op in ("pop", "pop2", "dup", "dup_x1", "dup_x2", "dup2", "dup2_x1", "dup2_x2", "swap"):
            self.stackop(op, st)
            return None
        # ---- arithmetic
        ar = ARITH.get(op)
        if ar:
            k, fmt, throws, n = ar
            self.ins_arith(st, k, fmt, throws, n)
            return None
        if op in CONV:
            k, fmt = CONV[op]
            v = self.pop(st)[0]
            emit(f"{self.push(st, k)} = {fmt.format(v)};")
            return None
        # ---- branches
        if op in IF1 or op in IF2:
            if op in IF1:
                cond = IF1[op].format(self.pop(st)[0])
            else:
                b = self.pop(st)[0]
                cond = IF2[op].format(self.pop(st)[0], b)
            t = ins.targets[0]
            nxt = self.next_pc(ins)
            if t <= ins.pc:  # loop back-edge: GC safepoint poll
                emit(f"if ({cond}) {{ rt.safepoint(); continue :sw {t}; }} else continue :sw {nxt};")
            else:
                emit(f"if ({cond}) continue :sw {t} else continue :sw {nxt};")
            return [t, nxt]
        if op == "goto":
            if ins.targets[0] <= ins.pc:
                emit("rt.safepoint();")
            emit(f"continue :sw {ins.targets[0]};")
            return [ins.targets[0]]
        if op == "tableswitch":
            v = self.pop(st)[0]
            by_t: dict[int, list[int]] = {}
            for key, t in ins.arg2:
                by_t.setdefault(t, []).append(key)
            prongs = " ".join(f"{', '.join(map(str, ks))} => continue :sw {t}," for t, ks in by_t.items())
            if any(t <= ins.pc for t in ins.targets):
                emit("rt.safepoint();")
            emit(f"switch ({v}) {{ {prongs} else => continue :sw {ins.arg} }}")
            return list(dict.fromkeys(ins.targets))
        if op in ("ireturn", "lreturn", "freturn", "dreturn", "areturn"):
            v = self.pop(st)[0]
            emit(f"return {v};")
            return []
        if op == "return":
            emit("return;")
            return []
        if op == "athrow":
            v = self.pop(st)[0]
            self.throw_stmt(f"rt.athrow({v})")
            return []
        # ---- fields
        if op in ("getstatic", "putstatic", "getfield", "putfield"):
            self.field(ins, st)
            return None
        # ---- invocations
        if op in ("invokestatic", "invokespecial", "invokevirtual", "invokeinterface"):
            self.invoke(ins, st)
            return None
        # ---- objects
        if op == "new":
            ce = self.cg.cref(a.name)
            out = self.push(st, "A")
            if ce is None:
                self.throw_stmt(f"rt.missingClass({zstr(a.name)})")
            else:
                fn = "newObjectArena" if (not self.dry and ins.pc in self.local_allocs) else "newObject"
                emit(f"{out} = {self.chk(f'rt.{fn}({ce})')};")
            return None
        if op == "newarray":
            n = self.pop(st)[0]
            out = self.push(st, "A")
            fn = "newArrayArena" if (not self.dry and ins.pc in self.local_allocs) else "newArray"
            emit(f"{out} = {self.chk(f'rt.{fn}(rt.arrayOf(rt.prim_{a}), {n})')};")
            return None
        if op == "anewarray":
            n = self.pop(st)[0]
            out = self.push(st, "A")
            ce = self.cg.cref(a.name)
            if ce is None:
                self.throw_stmt(f"rt.missingClass({zstr(a.name)})")
            else:
                fn = "newArrayArena" if (not self.dry and ins.pc in self.local_allocs) else "newArray"
                emit(f"{out} = {self.chk(f'rt.{fn}(rt.arrayOf({ce}), {n})')};")
            return None
        if op == "multianewarray":
            dims = [self.pop(st)[0] for _ in range(ins.arg2)][::-1]
            out = self.push(st, "A")
            ce = self.cg.cref(a.name)
            if ce is None:
                self.throw_stmt(f"rt.missingClass({zstr(a.name)})")
            else:
                emit(f"{out} = {self.chk(f'rt.multiNewArray({ce}, &[_]i32{{ {', '.join(dims)} }})')};")
            return None
        if op == "arraylength":
            v = self.pop(st)[0]
            out = self.push(st, "I")
            emit(f"{out} = {self.chk(f'rt.arrLen({v})')};")
            return None
        if op == "checkcast":
            v = self.sv(len(st) - 1, st[-1])
            ce = self.cg.cref(a.name)
            if ce is None:
                emit(f"if ({v} != null) {self.chk(f'rt.missingClass({zstr(a.name)})')};")
            else:
                emit("{ const H = struct { var c: ?*rt.Class = null; }; " + self.chk(f"rt.checkcastH({v}, {ce}, &H.c)") + "; }")
            return None
        if op == "instanceof":
            v = self.pop(st)[0]
            out = self.push(st, "I")
            ce = self.cg.cref(a.name)
            if ce is None:
                emit(f"{out} = 0;")
            else:
                emit(f"{{ const H = struct {{ var c: ?*rt.Class = null; }}; {out} = @intFromBool(rt.instanceOfH({v}, {ce}, &H.c)); }}")
            return None
        if op == "monitorenter":
            v = self.pop(st)[0]
            emit(self.chk(f"rt.monitorEnter({v})") + ";")
            return None
        if op == "monitorexit":
            v = self.pop(st)[0]
            emit(self.chk(f"rt.monitorExit({v})") + ";")
            return None
        if op in ("jsr", "jsr_w", "ret"):
            self.report("unsupported-insn", f"{self.ci.name}.{self.m.name}: {op}")
            self.throw_stmt(f"rt.unsupported({zstr('jsr/ret bytecode')})")
            return []
        raise CodegenError(f"unhandled opcode {op}")

    def next_pc(self, ins: Insn) -> int:
        return self.insns[self.idx[ins.pc] + 1].pc

    def _safe_ea_callee(self, dc: ClassInfo, m: MethodInfo, depth: int = 0) -> bool:
        """True if calling (dc, m) cannot publish `this` or any ref arg outside itself."""
        key = (dc.name, m.key)
        cached = self.cg.g._ea_safe_callee.get(key)
        if cached is not None:
            return cached
        self.cg.g._ea_safe_callee[key] = False  # break recursion conservatively
        if m.code is None or (m.access & (ACC_SYNCHRONIZED | 0x0100)) or m.name == "<clinit>":
            return False
        args, ret = parse_method_desc(m.desc)
        if any(ad[0] in ("L", "[") for ad in args):
            return False
        if ret != "V" and ret[0] in ("L", "["):
            return False
        for x in self.p.insns(dc, m):
            op = x.op
            if op in ("getstatic", "putstatic", "aastore", "areturn", "athrow",
                      "monitorenter", "monitorexit", "new", "newarray", "anewarray", "multianewarray"):
                return False
            if op == "getfield" and x.arg.desc[0] in ("L", "["):
                return False
            if op in ("invokestatic", "invokespecial", "invokevirtual", "invokeinterface"):
                ma: MemberRef = x.arg
                if op == "invokespecial" and ma.owner == "java/lang/Object" and ma.name == "<init>":
                    continue
                if op == "invokestatic":
                    rr = self.p.resolve(ma)
                    if rr and INTRINSICS.get((rr[0].name, rr[1].name + rr[1].desc)) is not None:
                        continue
                    return False
                if op == "invokespecial" and depth < 2:
                    rr = self.p.resolve_special(dc, ma)
                    if rr and self._safe_ea_callee(rr[0], rr[1], depth + 1):
                        continue
                return False
        self.cg.g._ea_safe_callee[key] = True
        return True

    def _call_preserves_escape(self, op: str, a: MemberRef) -> bool:
        if op == "invokespecial" and a.owner == "java/lang/Object" and a.name == "<init>":
            return True
        if op == "invokestatic":
            r = self.p.resolve(a)
            return r is not None and INTRINSICS.get((r[0].name, r[1].name + r[1].desc)) is not None
        if op == "invokespecial":
            r = self.p.resolve_special(self.ci, a)
        elif op in ("invokevirtual", "invokeinterface"):
            r = self.p.resolve(a)
            if r is None:
                return False
            dc, m = r
            if not (m.is_private or (m.access & ACC_FINAL) or (dc.cf.access & ACC_FINAL and not dc.is_interface)):
                return False
        else:
            return False
        return r is not None and self._safe_ea_callee(r[0], r[1])

    def _analyze_local_allocs(self) -> set[int]:
        """Return allocation PCs inside loop-free DAG methods proven never to escape."""
        if self.leaf or self.code is None or self.code.handlers or self.m.name == "<clinit>":
            return set()
        insns = self.insns
        if not any(x.op in ("new", "newarray", "anewarray") for x in insns):
            return set()
        if any(x.op in ("monitorenter", "monitorexit") or any(t <= x.pc for t in x.targets) for x in insns):
            return set()

        EMPTY = frozenset()
        nloc = max(self.code.max_locals, 64)
        init_locs = [(EMPTY, False)] * nloc
        in_states: dict[int, tuple[list[tuple[frozenset, bool]], list[tuple[frozenset, bool]]]] = {
            0: (list(init_locs), [])
        }
        work = [0]
        alloc_sites: set[int] = set()
        escaped: set[int] = set()
        steps = 0
        budget = len(insns) * 16 + 64

        def step_block(bpc: int):
            locs, stk = in_states[bpc]
            locs = list(locs)
            stk = list(stk)
            i = self.idx[bpc]
            while True:
                ins = insns[i]
                op, a = ins.op, ins.arg
                succs = None
                if op in ("nop", "iinc", "checkcast"):
                    pass
                elif op == "aconst_null" or op.startswith(("iconst_", "fconst_")) or op in ("bipush", "sipush"):
                    stk.append((EMPTY, False))
                elif op.startswith(("lconst_", "dconst_")):
                    stk.append((EMPTY, True))
                elif op == "ldc":
                    is2 = isinstance(a, tuple) and a[0] in ("long", "double")
                    stk.append((EMPTY, is2))
                elif op in ("iload", "fload"):
                    stk.append((EMPTY, False))
                elif op in ("lload", "dload"):
                    stk.append((EMPTY, True))
                elif op == "aload":
                    stk.append(locs[a] if a < len(locs) else (EMPTY, False))
                elif op in ("istore", "lstore", "fstore", "dstore", "astore"):
                    v = stk.pop()
                    while a >= len(locs):
                        locs.append((EMPTY, False))
                    locs[a] = v
                elif op in ("iaload", "faload", "baload", "caload", "saload", "aaload"):
                    stk.pop(); stk.pop(); stk.append((EMPTY, False))
                elif op in ("laload", "daload"):
                    stk.pop(); stk.pop(); stk.append((EMPTY, True))
                elif op in ("iastore", "lastore", "fastore", "dastore", "bastore", "castore", "sastore"):
                    stk.pop(); stk.pop(); stk.pop()
                elif op == "aastore":
                    val = stk.pop(); stk.pop(); stk.pop()
                    escaped.update(val[0])
                elif op == "arraylength" or op == "instanceof":
                    stk.pop(); stk.append((EMPTY, False))
                elif op == "pop":
                    stk.pop()
                elif op == "pop2":
                    top = stk.pop()
                    if not top[1]:
                        stk.pop()
                elif op in ("dup", "dup_x1", "dup_x2", "dup2", "dup2_x1", "dup2_x2", "swap"):
                    c2 = lambda item: item[1]
                    if op == "dup": nin, perm = 1, (0, 0)
                    elif op == "dup_x1": nin, perm = 2, (1, 0, 1)
                    elif op == "dup_x2": nin, perm = (2, (1, 0, 1)) if c2(stk[-2]) else (3, (2, 0, 1, 2))
                    elif op == "dup2": nin, perm = (1, (0, 0)) if c2(stk[-1]) else (2, (0, 1, 0, 1))
                    elif op == "dup2_x1": nin, perm = (2, (1, 0, 1)) if c2(stk[-1]) else (3, (1, 2, 0, 1, 2))
                    elif op == "dup2_x2":
                        if c2(stk[-1]):
                            nin, perm = (2, (1, 0, 1)) if c2(stk[-2]) else (3, (2, 0, 1, 2))
                        else:
                            nin, perm = (3, (1, 2, 0, 1, 2)) if c2(stk[-3]) else (4, (2, 3, 0, 1, 2, 3))
                    else:
                        nin, perm = 2, (1, 0)
                    seg = stk[-nin:]
                    del stk[-nin:]
                    for p_i in perm:
                        stk.append(seg[p_i])
                elif op in ARITH:
                    k, _, _, n = ARITH[op]
                    for _ in range(n):
                        stk.pop()
                    stk.append((EMPTY, k in ("J", "D")))
                elif op in CONV:
                    k, _ = CONV[op]
                    stk.pop()
                    stk.append((EMPTY, k in ("J", "D")))
                elif op in IF1 or op in IF2:
                    stk.pop()
                    if op in IF2:
                        stk.pop()
                    succs = [ins.targets[0], self.next_pc(ins)]
                elif op == "goto":
                    succs = [ins.targets[0]]
                elif op == "tableswitch":
                    stk.pop()
                    succs = list(dict.fromkeys(ins.targets))
                elif op in ("ireturn", "lreturn", "freturn", "dreturn", "return"):
                    if op != "return":
                        stk.pop()
                    return []
                elif op in ("areturn", "athrow"):
                    v = stk.pop()
                    escaped.update(v[0])
                    return []
                elif op == "getstatic":
                    stk.append((EMPTY, a.desc in ("J", "D")))
                elif op == "putstatic":
                    v = stk.pop()
                    escaped.update(v[0])
                elif op == "getfield":
                    stk.pop()
                    stk.append((EMPTY, a.desc in ("J", "D")))
                elif op == "putfield":
                    val = stk.pop(); stk.pop()
                    escaped.update(val[0])
                elif op == "new":
                    alloc_sites.add(ins.pc)
                    stk.append((frozenset((ins.pc,)), False))
                elif op in ("newarray", "anewarray"):
                    stk.pop()
                    alloc_sites.add(ins.pc)
                    stk.append((frozenset((ins.pc,)), False))
                elif op == "multianewarray":
                    for _ in range(ins.arg2):
                        stk.pop()
                    stk.append((EMPTY, False))
                elif op in ("invokestatic", "invokespecial", "invokevirtual", "invokeinterface"):
                    args, ret = parse_method_desc(a.desc)
                    popped = [stk.pop() for _ in range(len(args))]
                    if op != "invokestatic":
                        recv = stk.pop()
                        popped.append(recv)
                    if not self._call_preserves_escape(op, a):
                        for item in popped:
                            escaped.update(item[0])
                    else:
                        # Safe callee keeps `recv` non-escaping; any other non-empty arg escapes conservatively.
                        for item in popped[:-1] if op != "invokestatic" else popped:
                            escaped.update(item[0])
                    if ret != "V":
                        stk.append((EMPTY, ret in ("J", "D")))
                else:
                    return None  # unknown opcode -> bail safely

                if succs is not None:
                    return [(s, locs, stk) for s in succs]
                i += 1
                if i >= len(insns):
                    return []
                nxt = insns[i].pc
                if nxt in self.leaders:
                    return [(nxt, locs, stk)]

        while work:
            steps += 1
            if steps > budget:
                return set()
            bpc = work.pop()
            out_edges = step_block(bpc)
            if out_edges is None:
                return set()
            for spc, nl, ns in out_edges:
                if spc not in in_states:
                    in_states[spc] = (list(nl), list(ns))
                    work.append(spc)
                else:
                    ol, os_ = in_states[spc]
                    if len(os_) != len(ns):
                        return set()
                    ml = [
                        (
                            (ol[k][0] if k < len(ol) else EMPTY) | (nl[k][0] if k < len(nl) else EMPTY),
                            ol[k][1] if k < len(ol) else nl[k][1],
                        )
                        for k in range(max(len(ol), len(nl)))
                    ]
                    ms = [(os_[k][0] | ns[k][0], os_[k][1] or ns[k][1]) for k in range(len(ns))]
                    if ml != ol or ms != os_:
                        in_states[spc] = (ml, ms)
                        work.append(spc)
        return alloc_sites - escaped

    def ldc(self, a, st):
        emit = self.emit
        if isinstance(a, tuple):
            t, v = a
            if t == "int":
                emit(f"{self.push(st, 'I')} = {v};")
            elif t == "long":
                emit(f"{self.push(st, 'J')} = {v};")
            elif t == "float":
                emit(f"{self.push(st, 'F')} = @bitCast(@as(u32, 0x{struct.unpack('>I', v)[0]:08x}));")
            elif t == "double":
                emit(f"{self.push(st, 'D')} = @bitCast(@as(u64, 0x{struct.unpack('>Q', v)[0]:016x}));")
            elif t == "string":
                out = self.push(st, "A")
                emit(f"{out} = {self.chk(self.cg.lit(v))};")
            else:
                raise CodegenError(f"ldc {a!r}")
            return None
        if isinstance(a, ClassConst):
            out = self.push(st, "A")
            ce = self.cg.cref(a.name)
            if ce is None:
                self.throw_stmt(f"rt.missingClass({zstr(a.name)})")
            else:
                emit(f"{out} = {self.chk(f'rt.classMirror({ce})')};")
            return None
        # MethodHandle / MethodType / condy constants
        out = self.push(st, "A")
        kind = type(a).__name__
        self.report("unsupported-ldc", f"{self.ci.name}.{self.m.name}{self.m.desc}: ldc {kind}")
        self.throw_stmt(f"rt.unsupported({zstr('ldc ' + kind)})")
        return None

    def stackop(self, op, st):
        def cat2(k):
            return k in ("J", "D")
        n = len(st)
        if op == "pop":
            st.pop()
            return
        if op == "pop2":
            if cat2(st[-1]):
                st.pop()
            else:
                st.pop()
                st.pop()
            return
        # determine input count and output permutation (indices into inputs, bottom-first)
        if op == "dup":
            nin, perm = 1, [0, 0]
        elif op == "dup_x1":
            nin, perm = 2, [1, 0, 1]
        elif op == "dup_x2":
            if cat2(st[-2]):
                nin, perm = 2, [1, 0, 1]
            else:
                nin, perm = 3, [2, 0, 1, 2]
        elif op == "dup2":
            if cat2(st[-1]):
                nin, perm = 1, [0, 0]
            else:
                nin, perm = 2, [0, 1, 0, 1]
        elif op == "dup2_x1":
            if cat2(st[-1]):
                nin, perm = 2, [1, 0, 1]
            else:
                nin, perm = 3, [1, 2, 0, 1, 2]
        elif op == "dup2_x2":
            if cat2(st[-1]):
                if cat2(st[-2]):
                    nin, perm = 2, [1, 0, 1]          # form 4
                else:
                    nin, perm = 3, [2, 0, 1, 2]       # form 2
            else:
                if cat2(st[-3]):
                    nin, perm = 3, [1, 2, 0, 1, 2]    # form 3
                else:
                    nin, perm = 4, [2, 3, 0, 1, 2, 3]  # form 1
        elif op == "swap":
            nin, perm = 2, [1, 0]
        else:
            raise CodegenError(op)
        base = n - nin
        ins_k = st[base:]
        ins_v = [self.sv(base + i, k) for i, k in enumerate(ins_k)]
        del st[base:]
        temps = [f"t{i}" for i in range(nin)]
        stmts = [f"const {temps[i]} = {ins_v[i]};" for i in range(nin)]
        for j, src in enumerate(perm):
            k = ins_k[src]
            st.append(k)
            stmts.append(f"{self.sv(base + j, k)} = {temps[src]};")
        self.emit("{ " + " ".join(stmts) + " }")

    # ------------------------------------------------------------- fields
    def field(self, ins: Insn, st):
        a: MemberRef = ins.arg
        op = ins.op
        try:
            dc, f = self.w.resolve_field(a.owner, a.name, a.desc)
        except LinkError as e:
            self.report("link-error", str(e))
            if op in ("putfield", "putstatic"):
                self.pop(st)
            if op in ("getfield", "putfield"):
                self.pop(st)
            if op in ("getfield", "getstatic"):
                self.push(st, kind_of(a.desc))
            self.throw_stmt(f"rt.throwKindNR(.NoSuchField, {zstr(a.owner + '.' + a.name)})")
            return
        if dc.name not in self.p.loaded:
            self.p.load(dc.name)
            self.cg.g.late_loaded.add(dc.name)
        fname = field_mname(dc.name, f.name, self.w, f.is_static)
        mod = f"P.{dc.mname}"
        k = kind_of(f.desc)
        conv = STORE_CONV.get(f.desc)
        vol = bool(f.access & 0x0040)
        zt = zt_field(f.desc)

        def load(place):
            return f"@atomicLoad({zt}, &{place}, .seq_cst)" if vol else place

        def store(place, val):
            val = f"{conv}({val})" if conv else val
            return f"@atomicStore({zt}, &{place}, {val}, .seq_cst);" if vol else f"{place} = {val};"
        if op in ("getstatic", "putstatic"):
            self.class_init(dc)
            if op == "getstatic":
                self.emit(f"{self.push(st, k)} = {load(f'{mod}.sp().{fname}')};")
            else:
                v = self.pop(st)[0]
                self.emit(store(f"{mod}.sp().{fname}", v))
            return
        if op == "getfield":
            o = self.pop(st)[0]
            out = self.push(st, k)
            self.emit(f"{out} = {load(mod + '.cast(' + self.chk(f'rt.nn({o})') + ').' + fname)};")
        else:
            v = self.pop(st)[0]
            o = self.pop(st)[0]
            self.emit(store(mod + ".cast(" + self.chk(f"rt.nn({o})") + ")." + fname, v))

    def class_init(self, dc: ClassInfo):
        cur = self.ci
        if dc is cur:
            return
        if not dc.is_interface and not cur.is_interface and self.w.is_subclass(cur, dc):
            return
        if dc.name in self.p.initialized and dc.cf.method("<clinit>", "()V") is None \
                and not any(f.is_static and f.constant_value is not None for f in dc.cf.fields) \
                and all(s.cf.method("<clinit>", "()V") is None for s in dc.supers()):
            return  # nothing to run: initialization is a no-op
        self.emit(self.chk(f"P.{dc.mname}.init()") + ";")

    # ------------------------------------------------------------- invocations
    def invoke(self, ins: Insn, st):
        # Reference slots consumed by the call are cleared afterwards: a stale operand-stack slot would
        # keep garbage (and weakly referenced objects) alive for the rest of the frame.
        a: MemberRef = ins.arg
        args, ret = parse_method_desc(a.desc)
        base = len(st) - len(args) - (0 if ins.op == "invokestatic" else 1)
        stale = [self.sv(i, "A") for i in range(max(base, 0), len(st)) if st[i] == "A"]
        self._invoke(ins, st)
        if ret != "V" and base >= 0:
            out = self.sv(base, kind_of(ret))
            stale = [v for v in stale if v != out]
        for v in stale:
            self.emit(f"{v} = null;")

    def _invoke(self, ins: Insn, st):
        a: MemberRef = ins.arg
        op = ins.op
        if op == "invokevirtual" and a.owner == "java/lang/invoke/VarHandle" and a.name in VH_MODES:
            return self.varhandle(ins, st)
        if op == "invokevirtual" and a.owner == "java/lang/invoke/MethodHandle" and a.name in ("invoke", "invokeExact"):
            args, ret = parse_method_desc(a.desc)
            for _ in range(len(args) + 1):
                self.pop(st)
            if ret != "V":
                self.push(st, kind_of(ret))
            self.report("unsupported-methodhandle", f"{self.ci.name}.{self.m.name}{self.m.desc}: MethodHandle.{a.name}{a.desc}")
            self.throw_stmt(f"rt.unsupported({zstr('MethodHandle.' + a.name)})")
            return
        args, ret = parse_method_desc(a.desc)
        vals = [self.pop(st)[0] for _ in range(len(args))][::-1]
        recv = None if op == "invokestatic" else self.pop(st)[0]
        out = None if ret == "V" else self.push(st, kind_of(ret))
        assign = f"{out} = " if out else ""
        call_args = ([recv] if recv is not None else []) + vals
        argstr = ", ".join(call_args)
        # resolution
        if op == "invokespecial":
            r = self.p.resolve_special(self.ci, a)
        else:
            r = self.p.resolve(a)
        if r is None:
            self.throw_stmt(f"rt.throwKindNR(.NoSuchMethod, {zstr(a.owner + '.' + a.name + a.desc)})")
            return
        dc, m = r
        if op == "invokestatic":
            it = INTRINSICS.get((dc.name, m.name + m.desc))
            if it is not None and out is not None:
                fn, throws = it
                call = f"rt.{fn}({argstr})"
                self.emit(f"{assign}{self.chk(call) if throws else call};")
                return
            self.class_init(dc)
            self.direct(dc, m, argstr, assign)
            return
        if op == "invokespecial":
            if m.is_static:
                self.throw_stmt(f"rt.throwKindNR(.IncompatibleClassChange, {zstr(a.name)})")
                return
            if dc.name == "java/lang/Object" and m.name == "<init>" and m.desc == "()V" \
                    and (dc.name, m.name, m.desc) not in self.cg.g.patches:
                saved_lic = self.line_in_check
                self.line_in_check = not self.lazy and not self.leaf
                self.emit("_ = " + self.chk(f"rt.nn({recv})") + ";")
                self.line_in_check = saved_lic
                return
            self._direct_checked(dc, m, recv, vals, assign)
            return
        # virtual / interface
        if a.owner.startswith("[") and m.name == "clone":
            self.emit(f"{assign}{self.chk(f'rt.cloneObject({recv})')};")
            return
        if m.is_private or (m.access & ACC_FINAL) or (dc.cf.access & ACC_FINAL and not dc.is_interface):
            self._direct_checked(dc, m, recv, vals, assign)
            return
        ft = fn_type(False, m.desc)
        owner = a.owner if not a.owner.startswith("[") else "java/lang/Object"
        if self.cg.g.devirtualize:
            tgt = self.p.single_target(owner, m.name, m.desc)
            self.cg.g.cha_deps.setdefault(self.ci.name, {})[(owner, m.name, m.desc)] = tgt
            if tgt is not None and (tgt[0], tgt[1]) in self.p.reachable:
                tc = self.w.classes[tgt[0]]
                tm = next(x for x in tc.cf.methods if x.key == tgt[1])
                if not tm.is_abstract:
                    self._direct_checked(tc, tm, recv, vals, assign)
                    return
            elif tgt is None:
                sm = self.p.small_targets(owner, m.name, m.desc, limit=2)
                self.cg.g.cha_small_deps.setdefault(self.ci.name, {})[(owner, m.name, m.desc)] = sm
                if sm is not None:
                    targets = []
                    can_guard = True
                    for cname, (icn, ikn) in sm:
                        if (icn, ikn) not in self.p.reachable:
                            can_guard = False
                            break
                        tc = self.w.classes[icn]
                        tm = next(x for x in tc.cf.methods if x.key == ikn)
                        if tm.is_abstract or not self._can_inline_leaf(tc, tm):
                            can_guard = False
                            break
                        targets.append((cname, tc, tm))
                    if can_guard and len(targets) == 2:
                        u = self._uid()
                        rv = f"_o{u}"
                        saved_lic = self.line_in_check
                        self.line_in_check = not self.lazy and not self.leaf
                        recv_nn = self.chk(f"rt.nn({recv})")
                        self.line_in_check = saved_lic
                        self.emit(f"{{ const {rv} = {recv_nn};")
                        g_args = [rv] + vals
                        g_argstr = ", ".join(g_args)
                        for arm_i, (cname, tc, tm) in enumerate(targets):
                            if cname not in self.p.loaded:
                                self.p.load(cname)
                                self.cg.g.late_loaded.add(cname)
                            cc = self.w.classes[cname]
                            kw = "if" if arm_i == 0 else "else if"
                            self.emit(f"{kw} ({rv}.class == P.{cc.mname}.cls()) {{")
                            self.direct(tc, tm, g_argstr, assign, nn_checked=True, recv_obj=rv, call_args=g_args)
                            self.emit("}")
                        self.emit("else {")
                        saved_pl, saved_ll = self.pending_line, self.last_line
                        self._emit_dyn_dispatch(a, dc, m, owner, ft, rv, [rv] + vals, assign)
                        self.pending_line, self.last_line = saved_pl, saved_ll
                        self.emit("} }")
                        return
        saved_lic = self.line_in_check
        self.line_in_check = not self.lazy and not self.leaf
        recv_o = self.chk(f"rt.nn({recv})")
        self.line_in_check = saved_lic
        u = self._uid()
        rv = f"_o{u}"
        self.emit(f"{{ const {rv} = {recv_o};")
        self._emit_dyn_dispatch(a, dc, m, owner, ft, rv, [rv] + vals, assign)
        self.emit("}")

    def _emit_dyn_dispatch(self, a: MemberRef, dc: ClassInfo, m: MethodInfo, owner: str, ft: str,
                           rv: str, call_args: list[str], assign: str):
        argstr = ", ".join(call_args)
        oc = self.w.classes.get(owner)
        use_itable = oc is not None and oc.is_interface and dc.name != "java/lang/Object"
        if use_itable:
            if not dc.is_interface:
                dc = oc
            im = self.w.iface_methods(dc)
            k = next(i for i, x in enumerate(im) if x.name == m.name and x.desc == m.desc)
            fptr = self.chk(f"rt.ilookupH({rv}, P.{dc.mname}.cls(), {k}, &H.h)")
            self.emit(f"{{ const H = struct {{ var h: u16 = 0; }}; const f: {ft} = @ptrCast(@alignCast({fptr})); "
                      f"{assign}{self.chk(fp_call(m.desc, f'f({argstr})'))}; }}")
            return
        vc = oc if (oc is not None and not oc.is_interface) else self.w.classes["java/lang/Object"]
        vi = self.w.vindex(vc).get(m.name + m.desc)
        if vi is None:
            self.throw_stmt(f"rt.throwKindNR(.AbstractMethod, {zstr(a.owner + '.' + a.name)})")
            return
        self.emit(f"{{ const f: {ft} = @ptrCast(@alignCast({rv}.class.vtable[{vi}])); "
                  f"{assign}{self.chk(fp_call(m.desc, f'f({argstr})'))}; }}")

    def _direct_checked(self, dc: ClassInfo, m: MethodInfo, recv: str, vals: list[str], assign: str):
        u = self._uid()
        rv = f"_r{u}"
        saved_lic = self.line_in_check
        self.line_in_check = not self.lazy and not self.leaf
        recv_o = self.chk(f"rt.nn({recv})")
        self.line_in_check = saved_lic
        cargs = [rv] + vals
        self.emit(f"{{ const {rv} = {recv_o};")
        self.direct(dc, m, ", ".join(cargs), assign, nn_checked=True, recv_obj=rv, call_args=cargs)
        self.emit("}")

    def varhandle(self, ins: Insn, st):
        a: MemberRef = ins.arg
        args, ret = parse_method_desc(a.desc)
        family, nvals, volatile_ = VH_MODES[a.name]
        vals = [self.pop(st)[0] for _ in range(len(args))][::-1]
        vh = self.pop(st)[0]
        coords, values = vals[:len(vals) - nvals], vals[len(vals) - nvals:]
        cdesc = args[:len(args) - nvals]
        vdesc = (args[len(args) - nvals:] or [ret])[0]
        nc = len(coords)
        T = zt_field(vdesc)
        if nc == 2 and cdesc[0] == "Ljava/lang/foreign/MemorySegment;" and cdesc[1] == "J":
            ms_dc = self.w.classes.get("java/lang/foreign/MemorySegment")
            ms_k = None if ms_dc is None else next(
                (i for i, x in enumerate(self.w.iface_methods(ms_dc)) if x.name == "address" and x.desc == "()J"),
                None)
            if ms_k is not None:
                if ms_dc.name not in self.p.loaded:
                    self.p.load(ms_dc.name)
                    self.cg.g.late_loaded.add(ms_dc.name)
                _ = vh
                u = self._uid()
                seg_nn = self.chk(f"rt.nn({coords[0]})")
                self.emit(f"const _ms{u} = {seg_nn};")
                self.emit(f"const _msh{u} = struct {{ var h: u16 = 0; }};")
                fptr = self.chk(f"rt.ilookupH(_ms{u}, P.{ms_dc.mname}.cls(), {ms_k}, &_msh{u}.h)")
                self.emit(f"const _msf{u}: *const fn (rt.Ref) rt.Res(i64) = @ptrCast(@alignCast({fptr}));")
                base_addr = self.chk(f"rt.unR(i64, _msf{u}(_ms{u}))")
                addr_expr = f"({base_addr} +% {coords[1]})"
                if family == "get":
                    call = f"VH.getSeg({T}, {'true' if volatile_ else 'false'}, {addr_expr})"
                elif family == "set":
                    call = f"VH.setSeg({T}, {'true' if volatile_ else 'false'}, {addr_expr}, {values[0]})"
                elif family == "cas":
                    call = f"VH.compareAndSetSeg({T}, {addr_expr}, {values[0]}, {values[1]})"
                elif family == "cax":
                    call = f"VH.compareAndExchangeSeg({T}, {addr_expr}, {values[0]}, {values[1]})"
                elif family == "xchg":
                    call = f"VH.getAndSetSeg({T}, {addr_expr}, {values[0]})"
                else:
                    call = f"VH.getAndOpSeg({T}, .{family}, {addr_expr}, {values[0]})"
                out = None if ret == "V" else self.push(st, kind_of(ret))
                lhs = f"{out} = " if out else ("" if family == "set" else "_ = ")
                self.emit(f"{lhs}{self.chk(call)};")
                return
        if nc > 2 or (nc >= 1 and kind_of(cdesc[0]) != "A") or (nc == 2 and kind_of(cdesc[1]) != "I"):
            self.report("unsupported-varhandle", f"{a.name}{a.desc}")
            if ret != "V":
                self.push(st, kind_of(ret))
            self.throw_stmt(f"rt.unsupported({zstr('VarHandle coordinates ' + a.desc)})")
            return
        obj = coords[0] if nc >= 1 else "null"
        idx = coords[1] if nc == 2 else "0"
        common = f"{T}, {vh}, {obj}, {idx}, {nc}"
        if family == "get":
            call = f"VH.get({T}, {'true' if volatile_ else 'false'}, {vh}, {obj}, {idx}, {nc})"
        elif family == "set":
            call = f"VH.set({T}, {'true' if volatile_ else 'false'}, {vh}, {obj}, {idx}, {nc}, {values[0]})"
        elif family == "cas":
            call = f"VH.compareAndSet({common}, {values[0]}, {values[1]})"
        elif family == "cax":
            call = f"VH.compareAndExchange({common}, {values[0]}, {values[1]})"
        elif family == "xchg":
            call = f"VH.getAndSet({common}, {values[0]})"
        else:
            call = f"VH.getAndOp({T}, .{family}, {vh}, {obj}, {idx}, {nc}, {values[0]})"
        out = None if ret == "V" else self.push(st, kind_of(ret))
        # a call site may discard the result of a value-returning access mode (descriptor ...V)
        lhs = f"{out} = " if out else ("" if family == "set" else "_ = ")
        self.emit(f"{lhs}{self.chk(call)};")

    def direct(self, dc: ClassInfo, m: MethodInfo, argstr: str, assign: str,
               nn_checked: bool = False, recv_obj: Optional[str] = None, call_args: Optional[list[str]] = None):
        if (dc.name, m.key) not in self.p.reachable:
            # RTA should have marked it; keep the program total anyway
            self.p.mark(dc, m)
            self.cg.g.late_methods.add((dc.name, m.key))
        if m.is_abstract:
            self.throw_stmt(f"rt.abstractMethod({zstr(dc.name + '.' + m.name)})")
            return
        if nn_checked and dc.name == "java/lang/Object" and m.name == "<init>" and m.desc == "()V" \
                and (dc.name, m.name, m.desc) not in self.cg.g.patches:
            return
        acc = self._accessor(dc, m)
        if acc is not None:
            kind, fdc, f = acc
            if fdc.name not in self.p.loaded:
                self.p.load(fdc.name)
                self.cg.g.late_loaded.add(fdc.name)
            fname = field_mname(fdc.name, f.name, self.w, False)
            cargs = call_args if call_args is not None else ([x.strip() for x in argstr.split(",")] if argstr else [])
            if nn_checked and recv_obj is not None:
                o_expr = recv_obj
            else:
                o_expr = self.chk(f"rt.nn({cargs[0]})")
            place = f"P.{fdc.mname}.cast({o_expr}).{fname}"
            vol = bool(f.access & 0x0040)
            zt = zt_field(f.desc)
            if kind == "get":
                self.emit(f"{assign}{f'@atomicLoad({zt}, &{place}, .seq_cst)' if vol else place};")
            else:
                val = cargs[1]
                conv = STORE_CONV.get(f.desc)
                val = f"{conv}({val})" if conv else val
                self.emit(f"@atomicStore({zt}, &{place}, {val}, .seq_cst);" if vol else f"{place} = {val};")
            return
        cargs = call_args if call_args is not None else ([x.strip() for x in argstr.split(",")] if argstr else [])
        if (m.is_static or nn_checked) and self._inline_leaf(dc, m, cargs, assign, recv_obj=recv_obj):
            return
        self.emit(f"{assign}{self.chk(f'P.{dc.mname}.{method_mname(dc.name, m.name, m.desc)}({argstr})')};")

    def _accessor(self, dc: ClassInfo, m: MethodInfo):
        """Trivial getter/setter (`return this.f;` / `this.f = a;`) called with a statically known
        target: inline the field access (the receiver is null-checked first, like the call)."""
        if (dc.name, m.name, m.desc) in self.cg.g.patches:
            return None
        if m.is_static or m.is_abstract or m.access & ACC_SYNCHRONIZED or m.code is None or m.code.handlers:
            return None
        nparams = len(parse_method_desc(m.desc)[0])
        ins = self.p.insns(dc, m)
        if nparams == 0 and len(ins) == 3 and ins[0].op == "aload" and ins[0].arg == 0 and ins[1].op == "getfield" \
                and ins[2].op in ("ireturn", "lreturn", "freturn", "dreturn", "areturn"):
            kind, fi = "get", ins[1]
        elif nparams == 1 and len(ins) == 4 and ins[0].op == "aload" and ins[0].arg == 0 and ins[1].op in (
                "iload", "lload", "fload", "dload", "aload") and ins[1].arg == 1 \
                and ins[2].op == "putfield" and ins[3].op == "return":
            kind, fi = "set", ins[2]
        else:
            return None
        try:
            fdc, f = self.w.resolve_field(fi.arg.owner, fi.arg.name, fi.arg.desc)
        except LinkError:
            return None
        if f.is_static:
            return None
        return kind, fdc, f

    def _can_inline_leaf(self, dc: ClassInfo, m: MethodInfo) -> bool:
        key = (dc.name, m.key)
        cached = self.cg.g._inline_ok_cache.get(key)
        if cached is not None:
            return cached
        res = self._compute_can_inline_leaf(dc, m)
        self.cg.g._inline_ok_cache[key] = res
        return res

    def _compute_can_inline_leaf(self, dc: ClassInfo, m: MethodInfo) -> bool:
        if (dc.name, m.name, m.desc) in self.cg.g.patches:
            return False
        if m.is_abstract or m.is_native or m.access & ACC_SYNCHRONIZED or m.code is None or m.code.handlers:
            return False
        ins = self.p.insns(dc, m)
        if not ins or len(ins) > 10:
            return False
        if ins[-1].op not in ("ireturn", "lreturn", "freturn", "dreturn", "areturn", "return"):
            return False
        allowed = (set(ARITH) - {"idiv", "irem", "ldiv", "lrem"}) | set(CONV) | {
            "nop", "aconst_null", "bipush", "sipush", "iload", "lload", "fload", "dload", "aload",
            "istore", "lstore", "fstore", "dstore", "astore", "iinc", "pop", "pop2",
            "dup", "dup_x1", "dup_x2", "dup2", "dup2_x1", "dup2_x2", "swap", "getfield", "putfield"}
        for i, x in enumerate(ins[:-1]):
            op = x.op
            if x.targets:
                return False
            if op in ("ireturn", "lreturn", "freturn", "dreturn", "areturn", "return", "athrow"):
                return False
            if op.startswith(("iconst_", "lconst_", "fconst_", "dconst_")):
                continue
            if op == "ldc":
                if not (isinstance(x.arg, tuple) and x.arg[0] in ("int", "long", "float", "double")):
                    return False
                continue
            if op == "invokestatic":
                r = self.p.resolve(x.arg)
                if r is None:
                    return False
                idc, im = r
                it = INTRINSICS.get((idc.name, im.name + im.desc))
                if it is None or it[1] or parse_method_desc(im.desc)[1] == "V":
                    return False
                continue
            if op == "invokespecial":
                a = x.arg
                if (not m.is_static and i > 0 and ins[i - 1].op == "aload" and ins[i - 1].arg == 0
                        and a.owner == "java/lang/Object" and a.name == "<init>" and a.desc == "()V"
                        and ("java/lang/Object", "<init>", "()V") not in self.cg.g.patches):
                    continue
                return False
            if op not in allowed:
                return False
            if op == "astore" and x.arg == 0 and not m.is_static:
                return False
            if op == "getfield":
                if m.is_static or i == 0 or ins[i - 1].op != "aload" or ins[i - 1].arg != 0:
                    return False
            if op == "putfield":
                if (m.is_static or i < 2 or ins[i - 2].op != "aload" or ins[i - 2].arg != 0 or not (
                        ins[i - 1].op in self.LEAF_PUSH or ins[i - 1].op.startswith(
                            ("iconst_", "lconst_", "fconst_", "dconst_")))):
                    return False
            if op in ("getfield", "putfield"):
                try:
                    fdc, f = self.w.resolve_field(x.arg.owner, x.arg.name, x.arg.desc)
                except LinkError:
                    return False
                if f.is_static:
                    return False
        return True

    def _inline_leaf(self, dc: ClassInfo, m: MethodInfo, call_args: list[str], assign: str,
                     recv_obj: Optional[str] = None) -> bool:
        if not self._can_inline_leaf(dc, m):
            return False
        ins = self.p.insns(dc, m)
        # Ensure field owner classes are marked loaded even on pass 1 (dry)
        for x in ins[:-1]:
            if x.op in ("getfield", "putfield"):
                fdc, _ = self.w.resolve_field(x.arg.owner, x.arg.name, x.arg.desc)
                if fdc.name not in self.p.loaded:
                    self.p.load(fdc.name)
                    self.cg.g.late_loaded.add(fdc.name)
        if self.dry or self.dead:
            return True
        u = self._uid()
        px = f"_in{u}_"
        args, _ = parse_method_desc(m.desc)
        lines: list[str] = []
        decl_vars: dict[str, str] = {}
        init_lines: list[str] = []
        slot = 0
        arg_idx = 0
        if not m.is_static:
            tobj = recv_obj if recv_obj is not None else f"{px}tobj"
            if recv_obj is None:
                init_lines.append(f"const {tobj} = try rt.nn({call_args[0]});")
            decl_vars[f"{px}l0a"] = "rt.Ref"
            init_lines.append(f"{px}l0a = {tobj};")
            slot = 1
            arg_idx = 1
        else:
            tobj = ""
        for i, ad in enumerate(args):
            k = kind_of(ad)
            vn = f"{px}l{slot}{k.lower()}"
            decl_vars[vn] = ZT_KIND[k]
            init_lines.append(f"{vn} = {call_args[arg_idx + i]};")
            slot += slots_of(ad)

        ist: list[str] = []

        def sv(d: int, k: str) -> str:
            vn = f"{px}s{d}{k.lower()}"
            decl_vars[vn] = ZT_KIND[k]
            return vn

        def lv(n: int, k: str) -> str:
            vn = f"{px}l{n}{k.lower()}"
            decl_vars[vn] = ZT_KIND[k]
            return vn

        def push(k: str) -> str:
            ist.append(k)
            return sv(len(ist) - 1, k)

        def pop() -> tuple[str, str]:
            k = ist.pop()
            return sv(len(ist), k), k

        for x in ins[:-1]:
            op, a = x.op, x.arg
            if op == "nop":
                continue
            if op == "aconst_null":
                lines.append(f"{push('A')} = null;")
            elif op.startswith("iconst_"):
                v = -1 if op == "iconst_m1" else int(op[-1])
                lines.append(f"{push('I')} = {v};")
            elif op.startswith("lconst_"):
                lines.append(f"{push('J')} = {op[-1]};")
            elif op.startswith("fconst_"):
                lines.append(f"{push('F')} = {op[-1]}.0;")
            elif op.startswith("dconst_"):
                lines.append(f"{push('D')} = {op[-1]}.0;")
            elif op in ("bipush", "sipush"):
                lines.append(f"{push('I')} = {a};")
            elif op == "ldc":
                t, v = a
                if t == "int":
                    lines.append(f"{push('I')} = {v};")
                elif t == "long":
                    lines.append(f"{push('J')} = {v};")
                elif t == "float":
                    lines.append(f"{push('F')} = @bitCast(@as(u32, 0x{struct.unpack('>I', v)[0]:08x}));")
                elif t == "double":
                    lines.append(f"{push('D')} = @bitCast(@as(u64, 0x{struct.unpack('>Q', v)[0]:016x}));")
            elif op in ("iload", "lload", "fload", "dload", "aload"):
                k = LK[op[0]]
                lines.append(f"{push(k)} = {lv(a, k)};")
            elif op in ("istore", "lstore", "fstore", "dstore", "astore"):
                k = LK[op[0]]
                v, vk = pop()
                lines.append(f"{lv(a, vk)} = {v};")
            elif op == "iinc":
                lines.append(f"{lv(a, 'I')} +%= {x.arg2};")
            elif op in ("pop", "pop2", "dup", "dup_x1", "dup_x2", "dup2", "dup2_x1", "dup2_x2", "swap"):
                def cat2(k): return k in ("J", "D")
                n = len(ist)
                if op == "pop":
                    ist.pop()
                elif op == "pop2":
                    if cat2(ist[-1]): ist.pop()
                    else: ist.pop(); ist.pop()
                else:
                    if op == "dup": nin, perm = 1, [0, 0]
                    elif op == "dup_x1": nin, perm = 2, [1, 0, 1]
                    elif op == "dup_x2": nin, perm = (2, [1, 0, 1]) if cat2(ist[-2]) else (3, [2, 0, 1, 2])
                    elif op == "dup2": nin, perm = (1, [0, 0]) if cat2(ist[-1]) else (2, [0, 1, 0, 1])
                    elif op == "dup2_x1": nin, perm = (2, [1, 0, 1]) if cat2(ist[-1]) else (3, [1, 2, 0, 1, 2])
                    elif op == "dup2_x2":
                        if cat2(ist[-1]): nin, perm = (2, [1, 0, 1]) if cat2(ist[-2]) else (3, [2, 0, 1, 2])
                        else: nin, perm = (3, [1, 2, 0, 1, 2]) if cat2(ist[-3]) else (4, [2, 3, 0, 1, 2, 3])
                    else: nin, perm = 2, [1, 0]
                    base = n - nin
                    ins_k = ist[base:]
                    ins_v = [sv(base + i, k) for i, k in enumerate(ins_k)]
                    del ist[base:]
                    stmts = [f"const {px}t{i} = {ins_v[i]};" for i in range(nin)]
                    for j, src in enumerate(perm):
                        k = ins_k[src]
                        ist.append(k)
                        stmts.append(f"{sv(base + j, k)} = {px}t{src};")
                    lines.append("{ " + " ".join(stmts) + " }")
            elif op in ARITH:
                k, fmt, _, n = ARITH[op]
                vals = [pop()[0] for _ in range(n)][::-1]
                lines.append(f"{push(k)} = {fmt.format(*vals)};")
            elif op in CONV:
                k, fmt = CONV[op]
                v = pop()[0]
                lines.append(f"{push(k)} = {fmt.format(v)};")
            elif op in ("getfield", "putfield"):
                fdc, f = self.w.resolve_field(a.owner, a.name, a.desc)
                fname = field_mname(fdc.name, f.name, self.w, False)
                place = f"P.{fdc.mname}.cast({tobj}).{fname}"
                vol = bool(f.access & 0x0040)
                zt = zt_field(f.desc)
                if op == "getfield":
                    pop()
                    out = push(kind_of(f.desc))
                    lines.append(f"{out} = {f'@atomicLoad({zt}, &{place}, .seq_cst)' if vol else place};")
                else:
                    v = pop()[0]
                    pop()
                    conv = STORE_CONV.get(f.desc)
                    v = f"{conv}({v})" if conv else v
                    lines.append(f"@atomicStore({zt}, &{place}, {v}, .seq_cst);" if vol else f"{place} = {v};")
            elif op == "invokestatic":
                idc, im = self.p.resolve(a)
                iargs, iret = parse_method_desc(im.desc)
                ivals = [pop()[0] for _ in range(len(iargs))][::-1]
                fn, _ = INTRINSICS[(idc.name, im.name + im.desc)]
                out = push(kind_of(iret))
                lines.append(f"{out} = rt.{fn}({', '.join(ivals)});")
            elif op == "invokespecial":
                pop()  # Object.<init>()V no-op on already non-null `this`
        ret_op = ins[-1].op
        decls = " ".join(f"var {n}: {t} = {('null' if t == 'rt.Ref' else '0')}; _ = &{n};" for n, t in sorted(decl_vars.items()))
        if ret_op == "return":
            ret_stmt = ""
        else:
            rv = pop()[0]
            ret_stmt = f" {assign}{rv};"
        self.emit(f"{{ {decls} {' '.join(init_lines)} {' '.join(lines)}{ret_stmt} }}")
        return True


LK = {"i": "I", "l": "J", "f": "F", "d": "D", "a": "A"}
# instructions that can throw or call (and therefore need an up-to-date line number)
THROWING = {"iaload", "laload", "faload", "daload", "aaload", "baload", "caload", "saload", "iastore",
            "lastore", "fastore", "dastore", "aastore", "bastore", "castore", "sastore", "idiv", "irem",
            "ldiv", "lrem", "getfield", "putfield", "getstatic", "putstatic", "invokevirtual",
            "invokespecial", "invokestatic", "invokeinterface", "new", "newarray", "anewarray",
            "multianewarray", "arraylength", "athrow", "checkcast", "monitorenter", "monitorexit", "ldc"}
# VarHandle access modes: name -> (family, number of value operands, volatile)
VH_MODES = {}
for _n in ("get", "getOpaque"):
    VH_MODES[_n] = ("get", 0, False)
for _n in ("getVolatile", "getAcquire"):
    VH_MODES[_n] = ("get", 0, True)
for _n in ("set", "setOpaque"):
    VH_MODES[_n] = ("set", 1, False)
for _n in ("setVolatile", "setRelease"):
    VH_MODES[_n] = ("set", 1, True)
for _n in ("compareAndSet", "weakCompareAndSet", "weakCompareAndSetPlain", "weakCompareAndSetAcquire",
           "weakCompareAndSetRelease"):
    VH_MODES[_n] = ("cas", 2, True)
for _n in ("compareAndExchange", "compareAndExchangeAcquire", "compareAndExchangeRelease"):
    VH_MODES[_n] = ("cax", 2, True)
for _n in ("getAndSet", "getAndSetAcquire", "getAndSetRelease"):
    VH_MODES[_n] = ("xchg", 1, True)
for _p, _f in (("getAndAdd", "add"), ("getAndBitwiseOr", "bor"), ("getAndBitwiseAnd", "band"),
               ("getAndBitwiseXor", "bxor")):
    for _sfx in ("", "Acquire", "Release"):
        VH_MODES[_p + _sfx] = (_f, 1, True)
ARITH = {
    "iadd": ("I", "{0} +% {1}", False, 2), "isub": ("I", "{0} -% {1}", False, 2),
    "imul": ("I", "{0} *% {1}", False, 2), "idiv": ("I", "rt.idiv({0}, {1})", True, 2),
    "irem": ("I", "rt.irem({0}, {1})", True, 2), "ineg": ("I", "0 -% {0}", False, 1),
    "ishl": ("I", "rt.ishl({0}, {1})", False, 2), "ishr": ("I", "rt.ishr({0}, {1})", False, 2),
    "iushr": ("I", "rt.iushr({0}, {1})", False, 2), "iand": ("I", "{0} & {1}", False, 2),
    "ior": ("I", "{0} | {1}", False, 2), "ixor": ("I", "{0} ^ {1}", False, 2),
    "ladd": ("J", "{0} +% {1}", False, 2), "lsub": ("J", "{0} -% {1}", False, 2),
    "lmul": ("J", "{0} *% {1}", False, 2), "ldiv": ("J", "rt.ldiv({0}, {1})", True, 2),
    "lrem": ("J", "rt.lrem({0}, {1})", True, 2), "lneg": ("J", "0 -% {0}", False, 1),
    "lshl": ("J", "rt.lshl({0}, {1})", False, 2), "lshr": ("J", "rt.lshr({0}, {1})", False, 2),
    "lushr": ("J", "rt.lushr({0}, {1})", False, 2), "land": ("J", "{0} & {1}", False, 2),
    "lor": ("J", "{0} | {1}", False, 2), "lxor": ("J", "{0} ^ {1}", False, 2),
    "fadd": ("F", "{0} + {1}", False, 2), "fsub": ("F", "{0} - {1}", False, 2),
    "fmul": ("F", "{0} * {1}", False, 2), "fdiv": ("F", "{0} / {1}", False, 2),
    "frem": ("F", "rt.fmod32({0}, {1})", False, 2), "fneg": ("F", "-{0}", False, 1),
    "dadd": ("D", "{0} + {1}", False, 2), "dsub": ("D", "{0} - {1}", False, 2),
    "dmul": ("D", "{0} * {1}", False, 2), "ddiv": ("D", "{0} / {1}", False, 2),
    "drem": ("D", "rt.fmod64({0}, {1})", False, 2), "dneg": ("D", "-{0}", False, 1),
    "lcmp": ("I", "rt.lcmp({0}, {1})", False, 2),
    "fcmpl": ("I", "rt.fcmpl({0}, {1})", False, 2), "fcmpg": ("I", "rt.fcmpg({0}, {1})", False, 2),
    "dcmpl": ("I", "rt.dcmpl({0}, {1})", False, 2), "dcmpg": ("I", "rt.dcmpg({0}, {1})", False, 2),
}
CONV = {
    "i2l": ("J", "@as(i64, {0})"), "i2f": ("F", "@as(f32, @floatFromInt({0}))"),
    "i2d": ("D", "@as(f64, @floatFromInt({0}))"), "l2i": ("I", "@as(i32, @truncate({0}))"),
    "l2f": ("F", "@as(f32, @floatFromInt({0}))"), "l2d": ("D", "@as(f64, @floatFromInt({0}))"),
    "f2i": ("I", "rt.f2i({0})"), "f2l": ("J", "rt.f2l({0})"), "f2d": ("D", "@as(f64, {0})"),
    "d2i": ("I", "rt.d2i({0})"), "d2l": ("J", "rt.d2l({0})"), "d2f": ("F", "@as(f32, @floatCast({0}))"),
    "i2b": ("I", "rt.i2b({0})"), "i2c": ("I", "rt.i2c({0})"), "i2s": ("I", "rt.i2s({0})"),
}
IF1 = {"ifeq": "{0} == 0", "ifne": "{0} != 0", "iflt": "{0} < 0", "ifge": "{0} >= 0",
       "ifgt": "{0} > 0", "ifle": "{0} <= 0", "ifnull": "{0} == null", "ifnonnull": "{0} != null"}
IF2 = {"if_icmpeq": "{0} == {1}", "if_icmpne": "{0} != {1}", "if_icmplt": "{0} < {1}",
       "if_icmpge": "{0} >= {1}", "if_icmpgt": "{0} > {1}", "if_icmple": "{0} <= {1}",
       "if_acmpeq": "{0} == {1}", "if_acmpne": "{0} != {1}"}


PATCH_DIR = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "patches")


def load_patches() -> dict:
    """patches/manifest.json -> {(class, method, desc): (impl file, function)}"""
    path = os.path.join(PATCH_DIR, "manifest.json")
    if not os.path.exists(path):
        return {}
    import json
    out = {}
    for e in json.load(open(path)).get("patches", []):
        impl = os.path.join(PATCH_DIR, e["impl"])
        if not os.path.exists(impl):
            raise CodegenError(f"patch {e['class']}.{e['method']}: missing file patches/{e['impl']}")
        out[(e["class"], e["method"], e["desc"])] = (e["impl"], e["fn"], bool(e.get("orig", False)))
    return out


# JDK static methods compiled to inline Zig helpers (rt.zig, "intrinsics") instead of the translated JDK code:
# (class, name+desc) -> (helper, can throw). Same results as the Java methods, including -0.0, NaN and overflow.
def _intrinsics() -> dict:
    t: dict = {}
    for c in ("java/lang/Math", "java/lang/StrictMath"):
        for k, f, th in (("floor(D)D", "iFloor", 0), ("ceil(D)D", "iCeil", 0), ("sqrt(D)D", "iSqrt", 0),
                         ("abs(I)I", "iAbsI", 0), ("abs(J)J", "iAbsJ", 0), ("abs(F)F", "iAbsF", 0), ("abs(D)D", "iAbsD", 0),
                         ("min(II)I", "iMinI", 0), ("max(II)I", "iMaxI", 0), ("min(JJ)J", "iMinJ", 0), ("max(JJ)J", "iMaxJ", 0),
                         ("min(FF)F", "iMinF", 0), ("max(FF)F", "iMaxF", 0), ("min(DD)D", "iMinD", 0), ("max(DD)D", "iMaxD", 0),
                         ("copySign(DD)D", "iCopySignD", 0), ("copySign(FF)F", "iCopySignF", 0),
                         ("signum(D)D", "iSignumD", 0), ("signum(F)F", "iSignumF", 0),
                         ("fma(DDD)D", "iFmaD", 0), ("fma(FFF)F", "iFmaF", 0),
                         ("floorDiv(II)I", "iFloorDivI", 1), ("floorMod(II)I", "iFloorModI", 1),
                         ("floorDiv(JJ)J", "iFloorDivJ", 1), ("floorMod(JJ)J", "iFloorModJ", 1)):
            t[(c, k)] = (f, bool(th))
    for c in ("java/lang/Math", "java/lang/StrictMath"):
        for k, f in (("getExponent(D)I", "iExpD"), ("getExponent(F)I", "iExpF")):
            t[(c, k)] = (f, False)
    for k, f, th in (("compare(II)I", "iCmpI", 0), ("compareUnsigned(II)I", "iCmpUI", 0),
                     ("divideUnsigned(II)I", "iDivUI", 1), ("remainderUnsigned(II)I", "iRemUI", 1),
                     ("signum(I)I", "iSigI", 0), ("rotateLeft(II)I", "iRotlI", 0),
                     ("rotateRight(II)I", "iRotrI", 0), ("reverse(I)I", "iRevI", 0),
                     ("reverseBytes(I)I", "iRevBytesI", 0), ("hashCode(I)I", "iIdI", 0),
                     ("toUnsignedLong(I)J", "iToUnsignedLong", 0), ("bitCount(I)I", "iBitCountI", 0),
                     ("numberOfLeadingZeros(I)I", "iClzI", 0), ("numberOfTrailingZeros(I)I", "iCtzI", 0)):
        t[("java/lang/Integer", k)] = (f, bool(th))
    for k, f, th in (("compare(JJ)I", "iCmpJ", 0), ("compareUnsigned(JJ)I", "iCmpUJ", 0),
                     ("divideUnsigned(JJ)J", "iDivUJ", 1), ("remainderUnsigned(JJ)J", "iRemUJ", 1),
                     ("signum(J)I", "iSigJ", 0), ("rotateLeft(JI)J", "iRotlJ", 0),
                     ("rotateRight(JI)J", "iRotrJ", 0), ("reverse(J)J", "iRevJ", 0),
                     ("reverseBytes(J)J", "iRevBytesJ", 0), ("hashCode(J)I", "iHashJ", 0),
                     ("bitCount(J)I", "iBitCountJ", 0), ("numberOfLeadingZeros(J)I", "iClzJ", 0),
                     ("numberOfTrailingZeros(J)I", "iCtzJ", 0)):
        t[("java/lang/Long", k)] = (f, bool(th))
    t[("java/lang/Short", "reverseBytes(S)S")] = ("iRevBytesS", False)
    t[("java/lang/Character", "reverseBytes(C)C")] = ("iRevBytesC", False)
    for k, f in (("floatToRawIntBits(F)I", "iF2I"), ("floatToIntBits(F)I", "iFloatToIntBits"),
                 ("intBitsToFloat(I)F", "iI2F"), ("isNaN(F)Z", "iIsNanF"),
                 ("isInfinite(F)Z", "iIsInfF"), ("isFinite(F)Z", "iIsFinF")):
        t[("java/lang/Float", k)] = (f, False)
    for k, f in (("doubleToRawLongBits(D)J", "iD2J"), ("doubleToLongBits(D)J", "iDoubleToLongBits"),
                 ("longBitsToDouble(J)D", "iJ2D"), ("isNaN(D)Z", "iIsNanD"),
                 ("isInfinite(D)Z", "iIsInfD"), ("isFinite(D)Z", "iIsFinD")):
        t[("java/lang/Double", k)] = (f, False)
    return t


INTRINSICS = _intrinsics()


class ZigGen:
    def __init__(self, prog: Program, out_dir: str, natives: set[str], nshards: int = 0):
        self.patches = load_patches()
        self.patches_used: set[tuple] = set()
        self.nshards = nshards
        self.shard_map: dict[str, int] = {}
        self.base_shards = 0
        self.dev_unit: dict[str, int] = {}
        self.prog = prog
        self.out = out_dir
        self.natives = natives
        self.report = prog.report
        self.late_loaded: set[str] = set()
        self.late_methods: set[tuple[str, str]] = set()
        self.devirtualize = True
        self.shapes: dict[str, int] = {}
        self.cha_deps: dict[str, dict[tuple[str, str, str], Optional[tuple[str, str]]]] = {}
        self.cha_small_deps: dict[str, dict[tuple[str, str, str], Optional[tuple]]] = {}
        self._inline_ok_cache: dict[tuple[str, str], bool] = {}
        self._ea_safe_callee: dict[tuple[str, str], bool] = {}

    def shard_of(self, name: str) -> int:
        k = self.shard_map.get(name)
        if k is None:  # class discovered during code generation: stable hash placement
            from .shard import dev_unit_key
            base_n = self.base_shards or self.nshards
            dk = dev_unit_key(name) if self.dev_unit else None
            if dk is not None and dk in self.dev_unit:
                k = self.dev_unit[dk]
            else:
                k = int(short_hash(name), 16) % base_n
            self.shard_map[name] = k
        return k

    def shape_id(self, static: bool, desc: str) -> int:
        """Invoker shape for reflective calls: static flag + erased parameter/return kinds."""
        args, ret = parse_method_desc(desc)
        key = ("S" if static else "I") + "".join(a if a in "ZBCSIJFD" else "A" for a in args) + ":" + \
            (ret if ret in "ZBCSIJFDV" else "A")
        if key not in self.shapes:
            # ids persist across builds (<out>/symbols.json, see shard.write_shards): a new shape gets the next free id
            # instead of shifting the ids of every shape discovered after it
            self.shapes[key] = max(self.shapes.values(), default=-1) + 1
        return self.shapes[key]

    def generate(self, main_class: str):
        os.makedirs(self.out, exist_ok=True)
        if self.nshards:
            # reflective invoker shapes keep their ids across builds (written back by shard.write_shards)
            import json
            symfile = os.path.join(os.path.dirname(os.path.abspath(self.out)), "symbols.json")
            if os.path.isfile(symfile):
                with open(symfile) as fh:
                    self.shapes = dict(json.load(fh).get("shapes", {}))
        if self.nshards:
            from .shard import assign_shards, dev_unit_key
            self.base_shards = self.nshards
            if self.nshards >= 3:
                keys = sorted({dev_unit_key(x) for x in self.prog.loaded if dev_unit_key(x) is not None})
                self.dev_unit = {k: self.base_shards + i for i, k in enumerate(keys)}
                self.nshards = self.base_shards + len(keys)
            self.shard_map = assign_shards(self.prog, sorted(self.prog.loaded), self.nshards, self.base_shards, self.dev_unit)
        done: set[str] = set()
        w = self.prog.w
        while True:
            todo = sorted(self.prog.loaded - done)
            if not todo:
                break
            before_reach = set(self.prog.reachable)
            before_inst = set(self.prog.instantiated)
            before_loaded = set(self.prog.loaded)
            before_vcalls = set(self.prog.vcalls.keys())
            before_enum = set(self.prog.enum_classes)
            for name in todo:
                self.cha_deps[name] = {}
                self.cha_small_deps[name] = {}
                ci = w.classes[name]
                code = ClassGen(self, ci).generate()
                with open(os.path.join(self.out, ci.mname + ".zig"), "w") as fh:
                    fh.write(code)
                done.add(name)
            # Code generation can discover more reachable code (rare: calls RTA proved dead
            # but that codegen still references). Re-run RTA to a fixpoint and regenerate
            # only classes whose own metadata or CHA-devirtualized call sites could change.
            while self.prog.work:
                c, m = self.prog.work.popleft()
                self.prog.scan(c, m)
            if (self.prog.reachable != before_reach or self.prog.instantiated != before_inst
                    or self.prog.loaded != before_loaded or self.prog.vcalls.keys() != before_vcalls
                    or self.prog.enum_classes != before_enum):
                self.prog.__dict__.pop("_cha", None)
                self.prog.__dict__.pop("_cha_small", None)
                dirty: set[str] = set()
                new_reach = self.prog.reachable - before_reach
                # Classes whose reachable methods changed + subclasses that inherit them in vtables/itables.
                sub_seeds = [c for c, _ in new_reach]
                if self.prog.loaded != before_loaded:
                    new_loaded = self.prog.loaded - before_loaded
                    if any(w.classes[nc].cf.access & 0x2000 for nc in new_loaded):
                        dirty |= done
                    for nc in new_loaded:
                        sub_seeds.append(nc)
                        nci = w.classes[nc]
                        for e in nci.cf.inner_classes:
                            if e.outer and e.outer in done:
                                dirty.add(e.outer)
                # Walk subclasses once via BFS
                seen_sub: set[str] = set()
                q = list(sub_seeds)
                while q:
                    x = q.pop()
                    if x in seen_sub:
                        continue
                    seen_sub.add(x)
                    if x in done:
                        dirty.add(x)
                    xci = w.classes.get(x)
                    if xci:
                        for sc in xci.subclasses:
                            if sc.name not in seen_sub:
                                q.append(sc.name)
                dirty |= (self.prog.instantiated - before_inst) & done
                dirty |= (self.prog.enum_classes - before_enum) & done
                if self.prog.vcalls.keys() != before_vcalls:
                    new_vc = self.prog.vcalls.keys() - before_vcalls
                    for cname in done - dirty:
                        if cname in self.prog.instantiated:
                            cci = w.classes[cname]
                            if any((s.name + s.desc) in new_vc for s in w.vtable(cci)) or any(
                                (im.name + im.desc) in new_vc for ii in w.all_interfaces(cci) for im in w.iface_methods(ii)
                            ):
                                dirty.add(cname)
                for cname in done - dirty:
                    deps = self.cha_deps.get(cname)
                    changed = False
                    if deps:
                        for (ow, mn, md), old_tgt in deps.items():
                            new_tgt = self.prog.single_target(ow, mn, md)
                            if new_tgt != old_tgt or (new_tgt is not None and new_tgt not in before_reach):
                                changed = True
                                break
                    if not changed:
                        sdeps = self.cha_small_deps.get(cname)
                        if sdeps:
                            for (ow, mn, md), old_sm in sdeps.items():
                                new_sm = self.prog.small_targets(ow, mn, md, limit=2)
                                if new_sm != old_sm or (new_sm and any(t not in before_reach for _, t in new_sm)):
                                    changed = True
                                    break
                    if changed:
                        dirty.add(cname)
                done -= dirty
            self.late_methods.clear()
        self.write_root(main_class, sorted(done))
        if self.nshards:
            from .shard import write_shards
            for n in done:
                self.shard_of(n)
            write_shards(self, sorted(done), self.root_text, main_class, os.path.dirname(os.path.abspath(__file__)) + "/../runtime")

    def write_root(self, main_class: str, names: list[str]):
        w = self.prog.w
        cls = {n: w.classes[n] for n in names}
        o = []
        o.append("// Generated by j2zig — do not edit.\nconst std = @import(\"std\");\n"
                 "const rt = @import(\"rt.zig\");\n")
        for n in names:
            o.append(f"pub const {cls[n].mname} = @import(\"{cls[n].mname}.zig\");\n")
        o.append(f"pub var all_classes: [{len(names)}]*rt.Class = undefined;\n")
        o.append("pub fn buildAllClasses() void {\n")
        for i, n in enumerate(names):
            o.append(f"    all_classes[{i}] = {cls[n].mname}.cls();\n")
        o.append("}\n")

        def C(n):
            return w.classes[n].mname

        def M(c, n, d):
            return f"{C(c)}.{method_mname(c, n, d)}"
        # make_exception
        o.append("fn makeException(k: rt.Exc, msg: ?[]const u8) rt.E!rt.Ref {\n"
                 "    var R: [2]rt.Ref = @splat(null);\n"
                 "    var F = rt.Frame{ .refs = &R, .n = 2, .meth = &meta };\n"
                 "    try rt.pushFrame(&F);\n    defer rt.popFrame(&F);\n"
                 "    if (msg) |m| R[1] = try rt.newStringUtf8(m);\n"
                 "    switch (k) {\n")
        for tag, cname in RT_EXCEPTIONS:
            ci = w.classes.get(cname)
            if ci is None or cname not in self.prog.instantiated:
                o.append(f"        .{tag} => @panic(\"j2zig: {cname} not available\"),\n")
                continue
            m1 = ci.cf.method("<init>", "(Ljava/lang/String;)V")
            m0 = ci.cf.method("<init>", "()V")
            o.append(f"        .{tag} => {{\n            R[0] = try rt.newObject({C(cname)}.cls());\n")
            if m1 and m0:
                o.append(f"            if (msg != null) try {M(cname, '<init>', '(Ljava/lang/String;)V')}(R[0], R[1])"
                         f" else try {M(cname, '<init>', '()V')}(R[0]);\n")
            elif m1:
                o.append(f"            try {M(cname, '<init>', '(Ljava/lang/String;)V')}(R[0], R[1]);\n")
            else:
                o.append(f"            try {M(cname, '<init>', '()V')}(R[0]);\n")
            o.append("        },\n")
        o.append("    }\n    return R[0];\n}\n")
        o.append("// @@COMMON-BEGIN\n")
        o.append("const meta = rt.MethodMeta{ .class = \"<runtime>\", .name = \"<hook>\", .desc = \"\" };\n")
        o.append("// @@COMMON-END\n")
        eiie = "java/lang/ExceptionInInitializerError"
        o.append("fn wrapInitError(ex: rt.Ref) rt.E!rt.Ref {\n"
                 "    var R: [2]rt.Ref = .{ null, ex };\n"
                 "    var F = rt.Frame{ .refs = &R, .n = 2, .meth = &meta };\n"
                 "    try rt.pushFrame(&F);\n    defer rt.popFrame(&F);\n"
                 f"    R[0] = try rt.newObject({C(eiie)}.cls());\n"
                 f"    try {M(eiie, '<init>', '(Ljava/lang/Throwable;)V')}(R[0], R[1]);\n"
                 "    return R[0];\n}\n")
        o.append("fn isError(ex: rt.Ref) bool {\n"
                 f"    return rt.instanceOf(ex, {C('java/lang/Error')}.cls());\n}}\n")
        S = "java/lang/String"
        fv = field_mname(S, "value", w)
        o.append("fn makeString(units: []const u16) rt.E!rt.Ref {\n"
                 "    var R: [2]rt.Ref = @splat(null);\n"
                 "    var F = rt.Frame{ .refs = &R, .n = 2, .meth = &meta };\n"
                 "    try rt.pushFrame(&F);\n    defer rt.popFrame(&F);\n"
                 "    R[0] = try rt.newArray(rt.arrayOf(rt.prim_C), @intCast(units.len));\n"
                 "    @memcpy(rt.arrayData(u16, rt.asArray(R[0].?))[0..units.len], units);\n"
                 f"    R[1] = try rt.allocObject({C(S)}.cls(), {C(S)}.cls().size);\n"
                 f"    {C(S)}.cast(R[1].?).{fv} = R[0];\n"
                 "    return R[1];\n}\n")
        o.append("fn stringChars(s: *rt.Object) []u16 {\n"
                 f"    const a = rt.asArray({C(S)}.cast(s).{fv}.?);\n"
                 "    return rt.arrayData(u16, a)[0..@intCast(a.length)];\n}\n")
        CL = "java/lang/Class"
        fc = field_mname(CL, "j2zClass", w)
        o.append("fn mirrorSet(m: *rt.Object, c: *rt.Class) void {\n"
                 f"    {C(CL)}.cast(m).{fc} = @bitCast(@as(u64, @intFromPtr(c)));\n}}\n")
        o.append("fn mirrorGet(m: *rt.Object) *rt.Class {\n"
                 f"    return @ptrFromInt(@as(usize, @bitCast({C(CL)}.cast(m).{fc})));\n}}\n")
        T = "java/lang/Thread"
        o.append("fn threadRun(t: rt.Ref) rt.E!void {\n"
                 f"    return {M(T, 'j2zRun', '(Ljava/lang/Thread;)V')}(t);\n}}\n")
        o.append("fn threadUncaught(t: rt.Ref, e: rt.Ref) rt.E!void {\n"
                 f"    return {M(T, 'j2zUncaught', '(Ljava/lang/Thread;Ljava/lang/Throwable;)V')}(t, e);\n}}\n")
        o.append("fn threadStartMain() rt.E!rt.Ref {\n"
                 f"    try {C(T)}.init();\n"
                 f"    return {M(T, 'j2zStartMain', '()Ljava/lang/Thread;')}();\n}}\n")
        o.append("fn shutdown() rt.E!void {\n"
                 f"    try {C('java/lang/System')}.init();\n"
                 f"    return {M('java/lang/System', 'j2zShutdown', '()V')}();\n}}\n")
        o.append("fn eetopSet(t: *rt.Object, v: i64) void {\n"
                 f"    {C(T)}.cast(t).{field_mname(T, 'eetop', w)} = v;\n}}\n")
        RF = "java/lang/ref/Reference"
        if (RF, "enqueue()Z") in self.prog.reachable:
            o.append("fn enqueueRef(r: rt.Ref) rt.E!void {\n"
                     f"    _ = try {M(RF, 'enqueue', '()Z')}(r);\n}}\n")
            ref_hooks = (f"    .referent_offset = @offsetOf({C(RF)}.Layout, \"{field_mname(RF, 'referent', w)}\"),\n"
                         f"    .ref_queue_offset = @offsetOf({C(RF)}.Layout, \"{field_mname(RF, 'queue', w)}\"),\n"
                         "    .enqueue_ref = &enqueueRef,\n")
        else:
            o.append("fn enqueueRef(r: rt.Ref) rt.E!void {\n    _ = r;\n}\n")
            ref_hooks = "    .referent_offset = 0,\n    .ref_queue_offset = 0,\n    .enqueue_ref = &enqueueRef,\n"
        o.append("fn makeNcdfe(c: *rt.Class) rt.E!rt.Ref {\n"
                 "    var R: [1]rt.Ref = @splat(null);\n"
                 "    var F = rt.Frame{ .refs = &R, .n = 1, .meth = &meta };\n"
                 "    try rt.pushFrame(&F);\n    defer rt.popFrame(&F);\n"
                 "    R[0] = try rt.classMirror(c);\n"
                 f"    return {M('java/lang/Throwable', 'j2zNcdfe', '(Ljava/lang/Class;Ljava/lang/Throwable;Ljava/lang/Thread;)Ljava/lang/Throwable;')}(R[0], c.init_error, c.init_error_thread);\n}}\n")
        o.append("// @@COMMON-BEGIN\n")
        # ServiceLoader provider table
        prov_fns = []
        for svc, provs in sorted(getattr(self.prog, "services", {}).items()):
            for pv in provs:
                k = len(prov_fns)
                if (svc, pv) in getattr(self.prog, "service_factories", set()):
                    pci = w.classes[pv]
                    fm = next(m for m in pci.cf.methods if m.name == "provider" and m.is_static and m.desc.startswith("()"))
                    o.append(f"fn newProvider{k}() rt.E!rt.Ref {{\n"
                             f"    try {C(pv)}.init();\n"
                             f"    return {M(pv, 'provider', fm.desc)}();\n}}\n")
                else:
                    o.append(f"fn newProvider{k}() rt.E!rt.Ref {{\n"
                             "    var R: [1]rt.Ref = @splat(null);\n"
                             "    var F = rt.Frame{ .refs = &R, .n = 1, .meth = &meta };\n"
                             "    try rt.pushFrame(&F);\n    defer rt.popFrame(&F);\n"
                             f"    R[0] = try rt.newObject({C(pv)}.cls());\n"
                             f"    try {M(pv, '<init>', '()V')}(R[0]);\n    return R[0];\n}}\n")
                prov_fns.append((svc, k))
        o.append("pub const services = [_]rt.ServiceEntry{ " + ", ".join(
            f".{{ .service = {zstr(svc.replace('/', '.'))}, .create = &newProvider{k} }}" for svc, k in prov_fns) + " };\n")
        o.append("// @@COMMON-END\n")
        # JNI ThrowNew: construct any instantiated Throwable through its (String) constructor
        o.append("fn newThrowable(c: *rt.Class, msg: ?[]const u8) rt.E!rt.Ref {\n"
                 "    @setEvalBranchQuota(10_000_000);\n"
                 "    var R: [2]rt.Ref = @splat(null);\n"
                 "    var F = rt.Frame{ .refs = &R, .n = 2, .meth = &meta };\n"
                 "    try rt.pushFrame(&F);\n    defer rt.popFrame(&F);\n"
                 "    if (msg) |m| R[1] = try rt.newStringUtf8(m);\n")
        thr = w.classes.get("java/lang/Throwable")
        for n in names:
            ci = w.classes[n]
            if n not in self.prog.instantiated or thr is None or not w.is_subclass(ci, thr):
                continue
            mm = ci.cf.method("<init>", "(Ljava/lang/String;)V")
            if mm is None or (n, mm.key) not in self.prog.reachable:
                continue
            o.append(f"    if (c == {C(n)}.cls()) {{ R[0] = try rt.newObject(c); "
                     f"try {M(n, '<init>', '(Ljava/lang/String;)V')}(R[0], R[1]); return R[0]; }}\n")
        o.append("    return rt.throwKind(.InternalError, msg);\n}\n")
        o.append("// @@COMMON-BEGIN\n")
        # ---- reflective invocation (Method.invoke / Constructor.newInstance)
        BX = {"Z": ("java/lang/Boolean", "u8"), "B": ("java/lang/Byte", "i8"), "C": ("java/lang/Character", "u16"),
              "S": ("java/lang/Short", "i16"), "I": ("java/lang/Integer", "i32"), "J": ("java/lang/Long", "i64"),
              "F": ("java/lang/Float", "f32"), "D": ("java/lang/Double", "f64")}
        WIDEN_FROM = {"Z": "Z", "B": "B", "C": "C", "S": "BS", "I": "BCSI", "J": "BCSIJ", "F": "BCSIJF", "D": "BCSIJFD"}
        ok_box = all(b in w.classes and (b, f"valueOf({p})L{b};") in self.prog.reachable for p, (b, _) in BX.items())
        if ok_box:
            o.append("fn rarg(args: rt.Ref, i: usize) rt.Ref {\n"
                     "    return rt.arrayData(rt.Ref, rt.asArray(args.?))[i];\n}\n")
            for p, (b, _) in BX.items():
                zt = ZT_KIND[kind_of(p)]
                o.append(f"pub fn unbox{p}(o: rt.Ref) rt.E!{zt} {{\n    const x = o orelse return rt.throwKind(.IllegalArgument, \"argument type mismatch\");\n")
                for q in WIDEN_FROM[p]:
                    val = f"{C(BX[q][0])}.cast(x).f_value"
                    if p in "ZBCSI":
                        conv = f"@as(i32, {val})"
                    elif p == "J":
                        conv = f"@as(i64, {val})"
                    elif q in "BCSIJ":
                        conv = f"@as({zt}, @floatFromInt({val}))"
                    else:
                        conv = f"@as({zt}, {val})"
                    o.append(f"    if (x.class == {C(BX[q][0])}.cls()) return {conv};\n")
                o.append("    return rt.throwKind(.IllegalArgument, \"argument type mismatch\");\n}\n")
                o.append(f"pub fn box{p}(v: {zt}) rt.E!rt.Ref {{\n    try {C(b)}.init();\n    return {M(b, 'valueOf', '(' + p + ')L' + b + ';')}(v);\n}}\n")
        if self.nshards:
            # Sharded: the invoker switch lists every shape and changes whenever a new one appears. It lives only in
            # shard 0 (the shard rebuilt on every link); the other shards reach it through one exported symbol, so
            # their sources do not depend on the set of shapes.
            o.append("pub fn reflectInvoke(shape: u32, fp: *const anyopaque, recv: rt.Ref, args: rt.Ref) rt.E!rt.Ref {\n"
                     "    return rt.unR(rt.Ref, j2z_reflect_invoke(shape, fp, recv, args));\n}\n"
                     "extern fn j2z_reflect_invoke(shape: u32, fp: *const anyopaque, recv: rt.Ref, args: rt.Ref) callconv(.c) rt.Res(rt.Ref);\n"
                     "// @@COMMON-END\n"
                     "fn reflectInvokeC(shape: u32, fp: *const anyopaque, recv: rt.Ref, args: rt.Ref) callconv(.c) rt.Res(rt.Ref) {\n"
                     "    return rt.toR(rt.Ref, reflectInvokeImpl(shape, fp, recv, args));\n}\n"
                     "comptime {\n    @export(&reflectInvokeC, .{ .name = \"j2z_reflect_invoke\" });\n}\n")
        o.append(("fn reflectInvokeImpl" if self.nshards else "pub fn reflectInvoke") +
                 "(shape: u32, fp: *const anyopaque, recv: rt.Ref, args: rt.Ref) rt.E!rt.Ref {\n"
                 "    @setEvalBranchQuota(10_000_000);\n"
                 "    var R: [2]rt.Ref = .{ recv, args };\n"
                 "    var F = rt.Frame{ .refs = &R, .n = 2, .meth = &meta };\n"
                 "    try rt.pushFrame(&F);\n    defer rt.popFrame(&F);\n")
        if ok_box and self.shapes:
            o.append("    switch (shape) {\n")
            for key, sid in self.shapes.items():
                params_s, ret = key[1:].split(":")
                static = key[0] == "S"
                ptypes = ([] if static else ["rt.Ref"]) + [ZT_KIND[kind_of(p)] if p != "A" else "rt.Ref" for p in params_s]
                rt_t = "void" if ret == "V" else (ZT_KIND[kind_of(ret)] if ret != "A" else "rt.Ref")
                cargs = ([] if static else ["R[0]"]) + [
                    (f"rarg(R[1], {i})" if p == "A" else f"try unbox{p}(rarg(R[1], {i}))") for i, p in enumerate(params_s)]
                call = f"try f({', '.join(cargs)})" if ret == "V" else f"try rt.unR({rt_t}, f({', '.join(cargs)}))"
                fret = "rt.E!void" if ret == "V" else f"rt.Res({rt_t})"
                if ret == "V":
                    body = f"{call};\n            return null;"
                elif ret == "A":
                    body = f"return {call};"
                else:
                    body = f"return box{ret}({call});"
                o.append(f"        {sid} => {{\n            const f: *const fn ({', '.join(ptypes)}) {fret} = @ptrCast(@alignCast(fp));\n"
                         f"            {body}\n        }},\n")
            o.append("        else => unreachable,\n    }\n}\n")
        else:
            o.append("    _ = shape;\n    _ = fp;\n    return rt.throwKind(.UnsupportedOperation, \"reflective invocation\");\n}\n")
        if not self.nshards:
            o.append("// @@COMMON-END\n")
        o.append("// @@COMMON-BEGIN\n")
        UE = "sun/nio/fs/UnixException"
        uci = w.classes.get(UE)
        um = uci.cf.method("<init>", "(I)V") if uci else None
        if um is not None and (UE, um.key) in self.prog.reachable:
            o.append("pub fn newUnixException(errno: i32) rt.E!rt.Ref {\n"
                     "    var R: [1]rt.Ref = @splat(null);\n"
                     "    var F = rt.Frame{ .refs = &R, .n = 1, .meth = &meta };\n"
                     "    try rt.pushFrame(&F);\n    defer rt.popFrame(&F);\n"
                     f"    R[0] = try rt.newObject({C(UE)}.cls());\n"
                     f"    try {M(UE, '<init>', '(I)V')}(R[0], errno);\n    return R[0];\n}}\n")
        else:
            o.append("pub fn newUnixException(errno: i32) rt.E!rt.Ref {\n    _ = errno;\n"
                     "    return rt.throwKind(.InternalError, \"UnixException unavailable\");\n}\n")
        DB = "java/nio/DirectByteBuffer"
        dci = w.classes.get(DB)
        dm = dci.cf.method("<init>", "(JJ)V") if dci else None
        if dm is not None and (DB, dm.key) in self.prog.reachable:
            o.append("pub fn newDirectByteBuffer(addr: i64, cap: i64) rt.E!rt.Ref {\n"
                     "    var R: [1]rt.Ref = @splat(null);\n"
                     "    var F = rt.Frame{ .refs = &R, .n = 1, .meth = &meta };\n"
                     "    try rt.pushFrame(&F);\n    defer rt.popFrame(&F);\n"
                     f"    R[0] = try rt.newObject({C(DB)}.cls());\n"
                     f"    try {M(DB, '<init>', '(JJ)V')}(R[0], addr, cap);\n    return R[0];\n}}\n")
        else:
            o.append("pub fn newDirectByteBuffer(addr: i64, cap: i64) rt.E!rt.Ref {\n    _ = addr;\n    _ = cap;\n"
                     "    return rt.throwKind(.InternalError, \"DirectByteBuffer unavailable\");\n}\n")
        o.append("// @@COMMON-END\n")
        TF = "java/lang/Thread"
        o.append("fn foreignThread() rt.E!rt.Ref {\n"
                 f"    return {M(TF, 'j2zForeign', '()Ljava/lang/Thread;')}();\n}}\n")
        o.append("// @@COMMON-BEGIN\n")
        # LWJGL upcalls: invoke CallbackI.callback(JJ)V on the callback object
        CB = "org/lwjgl/system/CallbackI"
        cbi = w.classes.get(CB)
        if cbi is not None and CB in self.prog.loaded:
            k = next((i for i, x in enumerate(w.iface_methods(cbi)) if x.name == "callback" and x.desc == "(JJ)V"), None)
        else:
            k = None
        if k is not None:
            o.append("pub fn lwjglCallback(o: rt.Ref, ret: i64, args: i64) rt.E!void {\n"
                     "    const obj = try rt.nn(o);\n"
                     f"    const f: *const fn (rt.Ref, i64, i64) rt.E!void = @ptrCast(@alignCast(try rt.ilookup(obj, {C(CB)}.cls(), {k})));\n"
                     "    return f(o, ret, args);\n}\n")
        else:
            o.append("pub fn lwjglCallback(o: rt.Ref, ret: i64, args: i64) rt.E!void {\n"
                     "    _ = o;\n    _ = ret;\n    _ = args;\n    return rt.throwKind(.InternalError, \"no LWJGL callbacks\");\n}\n")
        o.append("// @@COMMON-END\n")
        o.append("pub fn makeHooks() rt.Hooks {\n    return rt.Hooks{\n"
                 "    .make_exception = &makeException,\n    .wrap_init_error = &wrapInitError,\n"
                 "    .is_error = &isError,\n    .make_string = &makeString,\n"
                 "    .string_chars = &stringChars,\n"
                 f"    .object_class = {C('java/lang/Object')}.cls(),\n"
                 f"    .class_class = {C(CL)}.cls(),\n    .string_class = {C(S)}.cls(),\n"
                 f"    .cloneable = {C('java/lang/Cloneable')}.cls(),\n"
                 f"    .serializable = {C('java/io/Serializable')}.cls(),\n"
                 "    .all_classes = &all_classes,\n"
                 "    .mirror_set_class = &mirrorSet,\n    .mirror_get_class = &mirrorGet,\n"
                 "    .thread_run = &threadRun,\n    .thread_uncaught = &threadUncaught,\n"
                 "    .thread_start_main = &threadStartMain,\n    .shutdown = &shutdown,\n"
                 "    .thread_eetop_set = &eetopSet,\n" + ref_hooks + "    .make_ncdfe = &makeNcdfe,\n"
                 "    .new_throwable = &newThrowable,\n    .foreign_thread = &foreignThread,\n    };\n}\n")
        # entry point
        mc = w.classes[main_class]
        o.append("pub fn entry() rt.E!void {\n"
                 "    var R: [2]rt.Ref = @splat(null);\n"
                 "    var F = rt.Frame{ .refs = &R, .n = 2, .meth = &meta };\n"
                 "    try rt.pushFrame(&F);\n    defer rt.popFrame(&F);\n"
                 f"    R[0] = try rt.newArray(rt.arrayOf({C(S)}.cls()), @intCast(rt.st.argv_storage.len));\n"
                 "    for (rt.st.argv_storage, 0..) |a, i| {\n"
                 "        R[1] = try rt.newStringUtf8(std.mem.span(a));\n"
                 "        try rt.aastore(R[0], @intCast(i), R[1]);\n    }\n"
                 f"    try {C('java/lang/System')}.init();\n"
                 f"    try {mc.mname}.init();\n"
                 f"    try {M(main_class, 'main', '([Ljava/lang/String;)V')}(R[0]);\n}}\n")
        if self.nshards:
            self.root_text = "".join(o)
            return
        with open(os.path.join(self.out, "classes.zig"), "w") as fh:
            fh.write("".join(o))
        with open(os.path.join(self.out, "main.zig"), "w") as fh:
            fh.write("const std = @import(\"std\");\nconst rt = @import(\"rt.zig\");\n"
                     "const P = @import(\"classes.zig\");\n"
                     "var android_started = std.atomic.Value(bool).init(false);\n"
                     "pub fn main() u8 {\n"
                     "    const args = std.os.argv;\n"
                     "    rt.initState();\n"
                     "    rt.st.argv_storage = rt.splitJavaOptions(if (args.len > 1) args[1..] else args[0..0]);\n"
                     "    rt.st.gc_verbose = rt.getenv(\"J2Z_GC_VERBOSE\") != null;\n"
                     "    rt.st.patch_verify = rt.getenv(\"J2Z_PATCH_VERIFY\") != null;\n"
                     "    P.buildAllClasses();\n"
                     "    rt.initRuntime(P.makeHooks());\n"
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
                     "    P.buildAllClasses();\n"
                     "    rt.initRuntime(P.makeHooks());\n"
                     "    return rt.runMainAsync(&P.entry);\n}\n")


def scan_natives(paths: list[str]) -> dict[str, str]:
    """Map native symbol name -> runtime Zig file that defines it."""
    names: dict[str, str] = {}
    rx = re.compile(r"pub (?:fn|const) (J_\w+)")
    for p in paths:
        with open(p) as fh:
            for n in rx.findall(fh.read()):
                names[n] = os.path.basename(p)
    return names
