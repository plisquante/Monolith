"""Closed-world class hierarchy model.

Provides JVMS §5.4.3 field / method resolution, method selection (§5.4.6),
vtable and itable layouts, instance field layouts and name mangling.
"""
from __future__ import annotations

import hashlib
from dataclasses import dataclass, field
from typing import Optional

from .classfile import (ACC_ABSTRACT, ACC_FINAL, ACC_PRIVATE, ACC_STATIC, ClassFile,
                        FieldInfo, MethodInfo)
from .classpath import ClassPath


class LinkError(Exception):
    pass


def mangle(s: str) -> str:
    out = []
    for ch in s:
        if ch.isalnum() and ch.isascii():
            out.append(ch)
        elif ch == "/":
            out.append("_")
        elif ch == "_":
            out.append("_1")
        elif ch == "$":
            out.append("_2")
        elif ch == ";":
            out.append("_3")
        elif ch == "[":
            out.append("_4")
        else:
            out.append(f"_u{ord(ch):04x}")
    return "".join(out)


def short_hash(s: str) -> str:
    return hashlib.sha1(s.encode()).hexdigest()[:8]


def parse_method_desc(desc: str) -> tuple[list[str], str]:
    assert desc[0] == "("
    i = 1
    args = []
    while desc[i] != ")":
        j = i
        while desc[j] == "[":
            j += 1
        if desc[j] == "L":
            j = desc.index(";", j)
        args.append(desc[i:j + 1])
        i = j + 1
    return args, desc[i + 1:]


def kind_of(desc: str) -> str:
    """Stack kind of a field/arg descriptor: I J F D A (V for void)."""
    c = desc[0]
    if c in "ZBCSI":
        return "I"
    if c in "JFDV":
        return c
    return "A"


def slots_of(desc: str) -> int:
    return 2 if desc[0] in "JD" else 1


@dataclass
class VSlot:
    name: str
    desc: str
    impl: Optional[tuple[str, MethodInfo]]  # (declaring class, method) or None (abstract)


@dataclass
class ClassInfo:
    cf: ClassFile
    name: str
    super: Optional["ClassInfo"]
    interfaces: list["ClassInfo"]
    instantiated: bool = False
    initialized: bool = False
    # computed lazily
    _all_ifaces: Optional[list["ClassInfo"]] = None
    _all_iface_set: Optional[set[str]] = None
    _super_set: Optional[set[str]] = None
    _vtable: Optional[list[VSlot]] = None
    _vindex: Optional[dict[str, int]] = None
    _fields: Optional[list[tuple[str, FieldInfo]]] = None
    subclasses: list["ClassInfo"] = field(default_factory=list)
    id: int = 0

    @property
    def is_interface(self) -> bool:
        return self.cf.is_interface

    @property
    def mname(self) -> str:
        return "C_" + mangle(self.name)

    def supers(self):
        c = self
        while c:
            yield c
            c = c.super


class World:
    def __init__(self, cp: ClassPath, synthetic: Optional[dict[str, ClassFile]] = None):
        self.cp = cp
        self.classes: dict[str, ClassInfo] = {}
        self.synthetic = synthetic if synthetic is not None else {}
        self.missing: set[str] = set()

    # ---------------------------------------------------------------- loading
    def get(self, name: str) -> Optional[ClassInfo]:
        if name in self.classes:
            return self.classes[name]
        if name.startswith("["):
            return None
        cf = self.synthetic.get(name) or self.cp.find(name)
        if cf is None:
            self.missing.add(name)
            return None
        sup = self.get(cf.super_name) if cf.super_name else None
        if cf.super_name and sup is None:
            self.missing.add(cf.super_name)
            raise LinkError(f"{name}: superclass {cf.super_name} not found")
        ifs = []
        for i in cf.interfaces:
            ii = self.get(i)
            if ii is None:
                raise LinkError(f"{name}: interface {i} not found")
            ifs.append(ii)
        ci = ClassInfo(cf, name, sup, ifs)
        ci.id = len(self.classes)
        self.classes[name] = ci
        if sup:
            sup.subclasses.append(ci)
        for i in ifs:
            i.subclasses.append(ci)
        return ci

    def req(self, name: str) -> ClassInfo:
        c = self.get(name)
        if c is None:
            raise LinkError(f"class not found: {name}")
        return c

    def add_synthetic(self, cf: ClassFile) -> ClassInfo:
        self.synthetic[cf.name] = cf
        return self.req(cf.name)

    # ---------------------------------------------------------------- subtyping
    def all_interfaces(self, c: ClassInfo) -> list[ClassInfo]:
        if c._all_ifaces is None:
            seen: dict[str, ClassInfo] = {}

            def walk(x: ClassInfo):
                for i in x.interfaces:
                    if i.name not in seen:
                        seen[i.name] = i
                        walk(i)
                if x.super:
                    walk(x.super)
            walk(c)
            c._all_ifaces = list(seen.values())
            c._all_iface_set = set(seen.keys())
        return c._all_ifaces

    def is_subclass(self, c: ClassInfo, of: ClassInfo) -> bool:
        if c is of:
            return True
        if of.is_interface:
            if c._all_iface_set is None:
                self.all_interfaces(c)
            return of.name in c._all_iface_set
        ss = c._super_set
        if ss is None:
            ss = {x.name for x in c.supers()}
            c._super_set = ss
        return of.name in ss

    # ---------------------------------------------------------------- fields
    def resolve_field(self, owner: str, name: str, desc: str) -> tuple[ClassInfo, FieldInfo]:
        """JVMS 5.4.3.2: the class itself, then superinterfaces, then superclass."""
        c = self.req(owner)

        def look(x: ClassInfo):
            f = x.cf.field(name, desc)
            if f:
                return x, f
            for i in x.interfaces:
                r = look(i)
                if r:
                    return r
            if x.super:
                return look(x.super)
            return None
        r = look(c)
        if not r:
            raise LinkError(f"NoSuchFieldError: {owner}.{name}:{desc}")
        return r

    def instance_fields(self, c: ClassInfo) -> list[tuple[str, FieldInfo]]:
        """All instance fields in layout order: superclass fields first."""
        if c._fields is None:
            base = self.instance_fields(c.super) if c.super else []
            own = [(c.name, f) for f in c.cf.fields if not f.is_static]
            # order by descending size for compact layout within the class's own segment
            sz = {"J": 8, "D": 8, "I": 4, "F": 4, "C": 2, "S": 2, "B": 1, "Z": 1}
            own.sort(key=lambda t: -sz.get(t[1].desc[0], 8))
            c._fields = base + own
        return c._fields

    # ---------------------------------------------------------------- methods
    def resolve_method(self, owner: str, name: str, desc: str, interface: bool = False
                       ) -> tuple[ClassInfo, MethodInfo]:
        """JVMS 5.4.3.3 / 5.4.3.4 method resolution."""
        c = self.req(owner)
        # 1. class and superclasses
        if not c.is_interface:
            for x in c.supers():
                m = x.cf.method(name, desc)
                if m:
                    return x, m
        else:
            m = c.cf.method(name, desc)
            if m:
                return c, m
            obj = self.req("java/lang/Object")
            m = obj.cf.method(name, desc)
            if m and not m.is_static and (m.access & 0x0001):
                return obj, m
        # 2. maximally-specific superinterface methods (prefer non-abstract)
        cands = self._max_specific(c, name, desc)
        nonabs = [x for x in cands if not x[1].is_abstract]
        if nonabs:
            return nonabs[0]
        if cands:
            return cands[0]
        raise LinkError(f"NoSuchMethodError: {owner}.{name}{desc}")

    def _max_specific(self, c: ClassInfo, name: str, desc: str) -> list[tuple[ClassInfo, MethodInfo]]:
        found = []
        for i in self.all_interfaces(c):
            m = i.cf.method(name, desc)
            if m and not m.is_static and not m.is_private:
                found.append((i, m))
        # drop those whose declaring interface is a superinterface of another candidate
        res = []
        for i, m in found:
            if not any(j is not i and self.is_subclass(j, i) for j, _ in found):
                res.append((i, m))
        return res

    def select(self, c: ClassInfo, name: str, desc: str) -> Optional[tuple[ClassInfo, MethodInfo]]:
        """JVMS 5.4.6 method selection for invokevirtual / invokeinterface on receiver class c."""
        for x in c.supers():
            m = x.cf.method(name, desc)
            if m and not m.is_static and not m.is_private:
                return x, m
        cands = [t for t in self._max_specific(c, name, desc) if not t[1].is_abstract]
        if len(cands) >= 1:
            return cands[0]
        return None

    # ---------------------------------------------------------------- vtables
    def vtable(self, c: ClassInfo) -> list[VSlot]:
        if c._vtable is not None:
            return c._vtable
        assert not c.is_interface
        slots: list[VSlot] = [VSlot(s.name, s.desc, s.impl) for s in self.vtable(c.super)] if c.super else []
        index = dict(self.vindex(c.super)) if c.super else {}
        for m in c.cf.methods:
            if m.is_static or m.is_private or m.name in ("<init>", "<clinit>"):
                continue
            k = m.name + m.desc
            impl = None if m.is_abstract else (c.name, m)
            if k in index:
                slots[index[k]].impl = impl
            else:
                index[k] = len(slots)
                slots.append(VSlot(m.name, m.desc, impl))
        # miranda / default methods from superinterfaces
        for i in self.all_interfaces(c):
            for m in i.cf.methods:
                if m.is_static or m.is_private or m.name == "<clinit>":
                    continue
                k = m.name + m.desc
                if k not in index:
                    index[k] = len(slots)
                    slots.append(VSlot(m.name, m.desc, None))
        # fill unimplemented slots with default methods
        for s in slots:
            if s.impl is None or s.impl[1].is_abstract:
                sel = self.select(c, s.name, s.desc)
                if sel:
                    s.impl = (sel[0].name, sel[1])
        c._vtable = slots
        c._vindex = index
        return slots

    def vindex(self, c: ClassInfo) -> dict[str, int]:
        self.vtable(c)
        return c._vindex  # type: ignore[return-value]

    def iface_methods(self, i: ClassInfo) -> list[MethodInfo]:
        """Methods that get an itable slot in interface i (declared in i itself)."""
        return [m for m in i.cf.methods
                if not m.is_static and not m.is_private and m.name != "<clinit>"]

    def is_final_method(self, decl: ClassInfo, m: MethodInfo) -> bool:
        return bool(m.access & (ACC_FINAL | ACC_PRIVATE)) or bool(decl.cf.access & ACC_FINAL)


def method_mname(cls: str, m_name: str, desc: str) -> str:
    n = m_name.replace("<", "").replace(">", "")
    return f"m_{mangle(n)}_{short_hash(cls + '.' + m_name + desc)}"


def field_mname(cls: str, f_name: str, world: "World" = None, static: bool = False) -> str:
    """Zig field name. Instance fields shadowing a superclass field get a depth suffix."""
    base = f"f_{mangle(f_name)}"
    if static or world is None:
        return base
    ci = world.classes.get(cls)
    if ci is None:
        return base
    shadow = 0
    s = ci.super
    while s is not None:
        if any(f.name == f_name and not f.is_static for f in s.cf.fields):
            shadow += 1
        s = s.super
    return base if shadow == 0 else f"{base}_{shadow}"


__all__ = ["World", "ClassInfo", "LinkError", "mangle", "parse_method_desc", "kind_of",
           "slots_of", "method_mname", "field_mname", "ACC_STATIC", "ACC_ABSTRACT"]
