"""Rust patches: hand-written replacements for translated methods.

`patches/rust/manifest.json` names methods (class, name, descriptor) and the Rust function that replaces each of
them. The translator still translates the original (it stays available as `r_<name>_orig`) and generates a
wrapper in its place. The patch source is pasted into a module in the shard of the patched class, next to
generated bindings for everything the manifest declares:

    "files": {
      "bitstorage.rs": {
        "fields":  {"size": "net/minecraft/util/SimpleBitStorage.size"},   # F_size: usize (byte offset)
        "statics": {"x": "pkg/Cls.field"},                                  # s_x() -> *mut T
        "classes": {"Level": "net/minecraft/world/level/Level"},            # class_Level(), is_Level(o), init_Level()
        "calls":   {"foo": "pkg/Cls.foo(I)I"},                              # c_foo(args) -> rt::Res<R>, direct (static / private / final)
        "vcalls":  {"next": "pkg/RandomSource.nextInt(I)I"},                # v_next(this, args) -> rt::Res<R>, virtual or interface dispatch
        "news":    {"pos": "net/minecraft/core/BlockPos.<init>(III)V"},     # n_pos(args) -> rt::Res<rt::Ref>: allocate and construct
        "strings": {"tick": "randomTick"}                                   # str_tick() -> rt::Ref: an interned Java String
      }
    },
    "patches": [
      {"class": "pkg/Cls", "method": "get", "desc": "(I)I", "file": "bitstorage.rs", "fn": "get", "orig": true}
    ]

A patch function has the signature of the generated method, `fn(this: rt::Ref, a0: i32, ...) -> rt::Res<R>`
(no `this` for static methods). With `"orig": true` it gets the translated original as its first parameter
(`orig: impl Fn(rt::Ref, i32) -> rt::Res<i32>`) and may hand any input it does not handle to it, so the Java
semantics (exceptions included) stay exact. `"verify": false` excludes a method with side effects from
MONO_PATCH_VERIFY (which runs both implementations on every call). `"fingerprint": "<16 hex>"` (tools/patch_fingerprints.py
writes it) pins the method body the patch was written against: when Minecraft changes that method the build stops with
a PatchError instead of keeping a patch that no longer matches. See patches/README.md.

Nothing of Mojang's code lives in the patch files: only our own Rust and the names of the methods it replaces.
"""
from __future__ import annotations

import hashlib
import json
import os
import re
from dataclasses import dataclass, field


PATCH_DIR = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "patches", "rust")


class PatchError(Exception):
    pass


@dataclass
class PatchFile:
    name: str
    source: str
    fields: dict = field(default_factory=dict)
    statics: dict = field(default_factory=dict)
    classes: dict = field(default_factory=dict)
    calls: dict = field(default_factory=dict)
    vcalls: dict = field(default_factory=dict)
    news: dict = field(default_factory=dict)
    strings: dict = field(default_factory=dict)


@dataclass
class Patch:
    cls: str
    method: str
    desc: str
    file: str
    fn: str
    orig: bool = False
    verify: bool = True   # False for methods with side effects: MONO_PATCH_VERIFY runs original and patch, so a patch that
                          # changes state would be applied twice
    fingerprint: str = ""  # optional: fingerprint() of the method the patch was written against; a mismatch stops the build

    @property
    def key(self) -> tuple:
        return (self.cls, self.method, self.desc)

    def where(self) -> str:
        return f"{self.cls}.{self.method}{self.desc}"


_IDENT = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")


def fingerprint(ci, m) -> str:
    """16 hex digits identifying the behaviour of a method body: its decoded instructions (operands resolved, so constant-pool
    numbering and unrelated edits to the class do not matter), exception table and frame sizes. Line numbers and local variable
    names are not part of it. A patch that carries this value stops the build when Minecraft changes the method it replaces."""
    from .bytecode import decode
    from .classfile import DynamicConst
    code = m.code
    lines = [f"stack={code.max_stack} locals={code.max_locals}"]
    for ins in decode(code.code, ci.cf.cp):
        a = ins.arg
        a = f"dyn:{a.is_indy}:{a.name}:{a.desc}" if isinstance(a, DynamicConst) else repr(a)  # bsm_index is a pool number
        lines.append(f"{ins.pc} {ins.op} {a} {ins.arg2!r} {list(ins.targets)}")
    for h in code.handlers:
        lines.append(f"h {h.start_pc} {h.end_pc} {h.handler_pc} {h.catch_type}")
    return hashlib.sha1("\n".join(lines).encode()).hexdigest()[:16]


def _split_call(spec: str) -> tuple[str, str, str]:
    """'pkg/Cls.name(I)V' -> ('pkg/Cls', 'name', '(I)V')"""
    i = spec.index("(")
    owner, name = spec[:i].rsplit(".", 1)
    return owner, name, spec[i:]


class PatchSet:
    def __init__(self, files: dict[str, PatchFile], patches: list[Patch]):
        self.files = files
        self.patches = patches
        self.by_key: dict[tuple, Patch] = {p.key: p for p in patches}
        self.used: set[tuple] = set()          # patches that were applied to a compiled method
        self.skipped: list[str] = []           # patches whose class is not on the class path
        self._emitted: set[tuple] = set()      # (file, class) modules already emitted

    def __contains__(self, key) -> bool:
        return key in self.by_key

    def __bool__(self) -> bool:
        return bool(self.patches)

    # ------------------------------------------------------------------ validation
    def validate(self, world) -> None:
        """Every patch must name a method that exists. A patch whose class is not on the class path at all is
        skipped (patch sets are shared between programs); a class that exists without the method is an error:
        the patch would otherwise silently stop applying after a Minecraft update."""
        for p in list(self.patches):
            ci = world.get(p.cls)
            if ci is None:
                self.skipped.append(p.where())
                del self.by_key[p.key]
                continue
            m = ci.cf.method(p.method, p.desc)
            if m is None:
                have = ", ".join(sorted(x.desc for x in ci.cf.methods if x.name == p.method)) or "none"
                raise PatchError(f"patch {p.where()}: no such method (descriptors of '{p.method}' in the class: {have})")
            if m.code is None:
                raise PatchError(f"patch {p.where()}: the method has no body (native or abstract); only translated methods can be patched")
            if p.fingerprint:
                have = fingerprint(ci, m)
                if have != p.fingerprint:
                    raise PatchError(f"patch {p.where()}: the method body changed (fingerprint {have}, the patch was written for "
                                     f"{p.fingerprint}). Review the patch against the new bytecode, then update the fingerprint "
                                     f"(python3 tools/patch_fingerprints.py --cp <jars> --write)")
        self.patches = [p for p in self.patches if p.key in self.by_key]

    def roots(self) -> list[tuple[str, str, str]]:
        """Methods the bindings call directly (or construct): they must be compiled even if nothing else reaches them."""
        out = []
        for f in self.files.values():
            for spec in list(f.calls.values()) + list(f.news.values()):
                out.append(_split_call(spec))
        return out

    def virtual_roots(self) -> list[tuple[str, str, str]]:
        """Virtual and interface methods the bindings dispatch: every implementation must be kept."""
        return [_split_call(spec) for f in self.files.values() for spec in f.vcalls.values()]

    # ------------------------------------------------------------------ code generation
    def module_for(self, g, patch: Patch) -> tuple[str, str]:
        """(module name, module source or '' when this shard class already has it)."""
        pf = self.files[patch.file]
        tag = hashlib.sha1((pf.name + "|" + patch.cls).encode()).hexdigest()[:8]
        mod = "patch_" + re.sub(r"\W", "_", pf.name.rsplit(".", 1)[0]) + "_" + tag
        key = (pf.name, patch.cls)
        if key in self._emitted:
            return mod, ""
        self._emitted.add(key)
        return mod, self._module_source(g, pf, mod)

    def _module_source(self, g, pf: PatchFile, mod: str) -> str:
        from .rustgen import RT, kind_of, mang, fname, rstr
        w = g.world
        out = ["#[allow(unused, non_snake_case, non_upper_case_globals, clippy::all)]",
               f"mod {mod} {{", "    use super::*;", "    use mono_rt::patch::*;", ""]
        # instance fields -> byte offsets
        for name, spec in sorted(pf.fields.items()):
            owner, fname_ = spec.rsplit(".", 1)
            ci = g.need_class(owner)
            hit = [(o, f) for o, f in w.instance_fields(ci) if f.name == fname_]
            if not hit:
                raise PatchError(f"{pf.name}: field '{name}': {owner} has no instance field '{fname_}'")
            o, f = hit[-1]  # the most derived declaration
            out.append(f"    /// {owner}.{fname_} : {f.desc}")
            out.append(f"    pub const F_{name}: usize = std::mem::offset_of!({g.struct_name(ci)}, f_{mang(o)}_{mang(f.name)});")
        # static fields
        for name, spec in sorted(pf.statics.items()):
            owner, fname_ = spec.rsplit(".", 1)
            ci = g.need_class(owner)
            dc = ci
            f = None
            while dc is not None and f is None:
                f = next((x for x in dc.cf.fields if x.is_static and x.name == fname_), None)
                if f is None:
                    dc = dc.super
            if f is None:
                raise PatchError(f"{pf.name}: static '{name}': {owner} has no static field '{fname_}'")
            g.need_class(dc.name)
            sn = g.static_name(dc, f)
            ty = RT[kind_of(f.desc)] if f.desc[0] not in "ZBCS" else {"Z": "i8", "B": "i8", "C": "u16", "S": "i16"}[f.desc[0]]
            out.append(f"    /// {owner}.{fname_} : {f.desc}")
            out.append(f"    pub fn s_{name}() -> *mut {ty} {{ unsafe {{ &raw mut {sn} }} }}")
        # classes
        for name, owner in sorted(pf.classes.items()):
            ci = g.need_class(owner)
            out.append(f"    pub fn class_{name}() -> &'static rt::Class {{ unsafe {{ &{g.class_name(ci)} }} }}")
            out.append(f"    /// True when `o` is non-null and exactly an instance of {owner}.")
            out.append(f"    #[inline(always)] pub fn is_{name}(o: rt::Ref) -> bool {{ !o.is_null() && unsafe {{ class_ptr(o) }} == class_{name}() as *const rt::Class }}")
            if g.needs_init(ci):
                out.append(f"    pub fn init_{name}() -> rt::Res<()> {{ unsafe {{ {g.init_name(ci)}() }}.res() }}")
        # calls (direct, non-virtual)
        for name, spec in sorted(pf.calls.items()):
            owner, mname, desc = _split_call(spec)
            try:
                dc, m = w.resolve_method(owner, mname, desc)
            except Exception as e:  # LinkError
                raise PatchError(f"{pf.name}: call '{name}': {e}")
            g.need_class(dc.name)
            g.need_method(dc, m)
            args, ret = _parse(desc)
            params, call = [], []
            if not m.is_static:
                params.append("this: rt::Ref")
                call.append("this")
            for j, d in enumerate(args):
                params.append(f"a{j}: {RT[kind_of(d)]}")
                call.append(f"a{j}")
            rret = "()" if ret == "V" else RT[kind_of(ret)]
            init = ""
            if m.is_static and g.needs_init(dc):
                init = f"unsafe {{ {g.init_name(dc)}() }}.res()?; "
            out.append(f"    /// {owner}.{mname}{desc}")
            out.append(f"    pub fn c_{name}({', '.join(params)}) -> rt::Res<{rret}> {{ {init}unsafe {{ {fname(dc.name, m.name, m.desc)}({', '.join(call)}) }}.res() }}")
        # virtual / interface calls: the same dispatch the translated code uses
        for name, spec in sorted(pf.vcalls.items()):
            owner, mname, desc = _split_call(spec)
            oci = g.need_class(owner)
            try:
                dc, m = w.resolve_method(owner, mname, desc, interface=oci.is_interface)
            except Exception as e:
                raise PatchError(f"{pf.name}: vcall '{name}': {e}")
            g.need_class(dc.name)
            g.need_method(dc, m)
            args, ret = _parse(desc)
            ptypes = ["rt::Ref"] + [RT[kind_of(d)] for d in args]
            params = ["this: rt::Ref"] + [f"a{j}: {RT[kind_of(d)]}" for j, d in enumerate(args)]
            call_args = ", ".join(["this"] + [f"a{j}" for j in range(len(args))])
            rret = "()" if ret == "V" else RT[kind_of(ret)]
            cret = "rt::CVoid" if ret == "V" else f"rt::CRes<{RT[kind_of(ret)]}>"
            fnty = f"extern \"C\" fn({', '.join(ptypes)}) -> {cret}"
            if m.is_static:
                raise PatchError(f"{pf.name}: vcall '{name}': {owner}.{mname}{desc} is static, use \"calls\"")
            obj_slot = w.vindex(w.req("java/lang/Object")).get(m.name + m.desc)
            if m.is_private or (m.access & 0x0010) or (dc.cf.access & 0x0010):
                target = f"unsafe {{ {fname(dc.name, m.name, m.desc)}({call_args}) }}.res()"
                body = f"rt::nn(this)?; {target}"
            elif oci.is_interface and obj_slot is None:
                sel = g.sel(m.name, m.desc)
                body = (f"let o = rt::nn(this)?; let f: {fnty} = unsafe {{ std::mem::transmute::<*const (), {fnty}>("
                        f"rt::itab_find((*o).class, {sel})) }}; f({call_args}).res()")
            else:
                slot = obj_slot if obj_slot is not None else w.vindex(oci).get(m.name + m.desc)
                if slot is None:
                    raise PatchError(f"{pf.name}: vcall '{name}': no vtable slot for {owner}.{mname}{desc}")
                body = (f"let o = rt::nn(this)?; let f: {fnty} = unsafe {{ std::mem::transmute::<*const (), {fnty}>("
                        f"(*(*o).class).vtable[{slot}].0) }}; f({call_args}).res()")
            out.append(f"    /// virtual: {owner}.{mname}{desc}")
            out.append(f"    pub fn v_{name}({', '.join(params)}) -> rt::Res<{rret}> {{ {body} }}")
        # constructors: allocate, initialize the class, run <init>
        for name, spec in sorted(pf.news.items()):
            owner, mname, desc = _split_call(spec)
            if mname != "<init>":
                raise PatchError(f"{pf.name}: new '{name}': {spec} is not a constructor")
            ci = g.need_class(owner)
            m = ci.cf.method("<init>", desc)
            if m is None:
                raise PatchError(f"{pf.name}: new '{name}': {owner} has no constructor {desc}")
            g.need_method(ci, m)
            args, _ = _parse(desc)
            params = [f"a{j}: {RT[kind_of(d)]}" for j, d in enumerate(args)]
            call = ", ".join(["o"] + [f"a{j}" for j in range(len(args))])
            init = f"unsafe {{ {g.init_name(ci)}() }}.res()?; " if g.needs_init(ci) else ""
            out.append(f"    /// new {owner}{desc}")
            out.append(f"    pub fn n_{name}({', '.join(params)}) -> rt::Res<rt::Ref> {{ {init}let o = rt::alloc(unsafe {{ &{g.class_name(ci)} }}); "
                       f"unsafe {{ {fname(ci.name, '<init>', desc)}({call}) }}.res()?; Ok(o) }}")
        # interned Java strings
        for name, text in sorted(pf.strings.items()):
            out.append(f"    /// the interned String {text!r}")
            out.append(f"    pub fn str_{name}() -> rt::Ref {{ static C: std::sync::OnceLock<usize> = std::sync::OnceLock::new(); "
                       f"rt::Ref(*C.get_or_init(|| {{ let r = rt::intern_string(rt::new_str({rstr(text)}.to_string())); mono_rt::gc::pin_ref(r); r.0 as usize }}) as *mut rt::Obj) }}")
        out.append("")
        out.append(pf.source)
        out.append("}")
        return "\n".join(out) + "\n"

    def wrapper(self, g, patch: Patch, r_entry: str, orig_name: str, params: list[str], call: list[str],
                ret: str, rret: str, mod: str, inline: str = "") -> str:
        """The function that stands where the translated method stood: calls the patch (or, when verifying,
        both)."""
        from .rustgen import rstr
        pcall = ([orig_name] if patch.orig else []) + call
        body = f"{mod}::{patch.fn}({', '.join(pcall)})"
        sig = f"{inline}fn {r_entry}({', '.join(params)}) -> rt::Res<{rret}> {{\n"
        if ret != "V" and ret[0] not in "L[" and patch.verify:
            ocall = f"{orig_name}({', '.join(call)})"
            sig += (f"    if mono_rt::patch::verify() {{\n"
                    f"        let a = {ocall}?;\n"
                    f"        let b = {body}?;\n"
                    f"        if !mono_rt::patch::Same::same(&a, &b) {{ mono_rt::patch::mismatch({rstr(patch.where())}); }}\n"
                    f"        return Ok(a);\n    }}\n")
        return sig + f"    {body}\n}}\n"

    def report(self) -> str:
        unused = [p.where() for p in self.patches if p.key not in self.used]
        s = f"patches: {len(self.used)} applied, {len(unused)} unused (method not compiled)"
        if self.skipped:
            s += f", {len(self.skipped)} skipped (class not on the class path)"
        return s


def _parse(desc: str):
    from .world import parse_method_desc
    return parse_method_desc(desc)


def load(path: str | None = None) -> PatchSet:
    path = path or os.path.join(PATCH_DIR, "manifest.json")
    if not os.path.exists(path):
        return PatchSet({}, [])
    base = os.path.dirname(path)
    try:
        doc = json.load(open(path, encoding="utf-8"))
    except ValueError as e:
        raise PatchError(f"{path}: invalid JSON: {e}")
    files: dict[str, PatchFile] = {}
    for name, cfg in doc.get("files", {}).items():
        fp = os.path.join(base, name)
        if not os.path.isfile(fp):
            raise PatchError(f"patch file {name}: not found in {base}")
        for kind in ("fields", "statics", "classes", "calls", "vcalls", "news", "strings"):
            for k in cfg.get(kind, {}):
                if not _IDENT.match(k):
                    raise PatchError(f"{name}: binding name '{k}' is not an identifier")
        files[name] = PatchFile(name, open(fp, encoding="utf-8").read(), cfg.get("fields", {}), cfg.get("statics", {}),
                                cfg.get("classes", {}), cfg.get("calls", {}), cfg.get("vcalls", {}), cfg.get("news", {}),
                                cfg.get("strings", {}))
    patches = []
    for e in doc.get("patches", []):
        for req in ("class", "method", "desc", "file", "fn"):
            if req not in e:
                raise PatchError(f"{path}: patch entry without '{req}': {e}")
        if e["file"] not in files:
            raise PatchError(f"patch {e['class']}.{e['method']}: file '{e['file']}' is not declared under \"files\"")
        if not _IDENT.match(e["fn"]):
            raise PatchError(f"patch {e['class']}.{e['method']}: fn '{e['fn']}' is not an identifier")
        patches.append(Patch(e["class"], e["method"], e["desc"], e["file"], e["fn"], bool(e.get("orig", False)), bool(e.get("verify", True)),
                             str(e.get("fingerprint", ""))))
    seen = set()
    for p in patches:
        if p.key in seen:
            raise PatchError(f"patch {p.where()} is declared twice")
        seen.add(p.key)
    return PatchSet(files, patches)
