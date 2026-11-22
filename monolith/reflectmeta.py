"""Reflection metadata for the native image: annotations, generic signatures, nesting info.

Annotations are kept as the class file's RuntimeVisible*Annotations / AnnotationDefault bytes,
so the JDK's own sun.reflect.annotation.AnnotationParser decodes them at run time exactly as on
HotSpot. Their constant-pool indices are rewritten to a compact per-class pool that holds only
the entries annotations use (Utf8 and numeric constants) — that pool is what
jdk.internal.reflect.ConstantPool reads in the native image.

Annotations whose type is not part of the closed world are dropped: code can only query an
annotation through its Class object, which keeps the annotation interface loaded.
"""
from __future__ import annotations

import struct
from typing import Callable, Optional

from .classfile import ClassFile


class CompactPool:
    def __init__(self, cp: list):
        self.cp = cp
        self.entries: list[tuple[str, object]] = []  # index 1.. -> (kind, value)
        self.index: dict[tuple[str, object], int] = {}

    def ref(self, old: int) -> int:
        v = self.cp[old]
        if isinstance(v, str):
            key = ("utf8", v)
        elif isinstance(v, tuple) and v[0] in ("int", "long", "float", "double"):
            raw = v[1]
            if v[0] == "float" and not isinstance(raw, (bytes, bytearray)):
                raw = struct.pack(">f", raw)
            if v[0] == "double" and not isinstance(raw, (bytes, bytearray)):
                raw = struct.pack(">d", raw)
            key = (v[0], bytes(raw) if isinstance(raw, (bytes, bytearray)) else raw)
        else:
            raise ValueError(f"annotation references unsupported constant #{old}: {v!r}")
        k = self.index.get(key)
        if k is None:
            self.entries.append(key)
            k = self.index[key] = len(self.entries)
        return k


class _Rewriter:
    def __init__(self, data: bytes, pool: CompactPool, keep: Callable[[str], bool]):
        self.d = data
        self.p = 0
        self.pool = pool
        self.keep = keep
        self.out = bytearray()

    def u1(self) -> int:
        v = self.d[self.p]
        self.p += 1
        return v

    def u2(self) -> int:
        v = struct.unpack_from(">H", self.d, self.p)[0]
        self.p += 2
        return v

    def element(self, out: bytearray):
        tag = chr(self.u1())
        out.append(ord(tag))
        if tag in "BCDFIJSZs":
            out += struct.pack(">H", self.pool.ref(self.u2()))
        elif tag == "e":
            out += struct.pack(">HH", self.pool.ref(self.u2()), self.pool.ref(self.u2()))
        elif tag == "c":
            out += struct.pack(">H", self.pool.ref(self.u2()))
        elif tag == "@":
            self.annotation(out)
        elif tag == "[":
            n = self.u2()
            out += struct.pack(">H", n)
            for _ in range(n):
                self.element(out)
        else:
            raise ValueError(f"bad element_value tag {tag!r}")

    def annotation(self, out: bytearray):
        out += struct.pack(">H", self.pool.ref(self.u2()))
        n = self.u2()
        out += struct.pack(">H", n)
        for _ in range(n):
            out += struct.pack(">H", self.pool.ref(self.u2()))
            self.element(out)

    def annotations(self, out: bytearray):
        n = self.u2()
        kept = []
        for _ in range(n):
            type_desc = self.pool.cp[struct.unpack_from(">H", self.d, self.p)[0]]
            buf = bytearray()
            self.annotation(buf)
            if self.keep(type_desc):
                kept.append(buf)
        out += struct.pack(">H", len(kept))
        for b in kept:
            out += b
        return len(kept)


def rewrite_annotations(data: Optional[bytes], pool: CompactPool, keep) -> Optional[bytes]:
    if not data:
        return None
    r = _Rewriter(data, pool, keep)
    out = bytearray()
    if r.annotations(out) == 0:
        return None
    return bytes(out)


def rewrite_param_annotations(data: Optional[bytes], pool: CompactPool, keep) -> Optional[bytes]:
    if not data:
        return None
    r = _Rewriter(data, pool, keep)
    out = bytearray()
    n = r.u1()
    out.append(n)
    total = 0
    for _ in range(n):
        total += r.annotations(out)
    return bytes(out) if total else None


def rewrite_default(data: Optional[bytes], pool: CompactPool) -> Optional[bytes]:
    if not data:
        return None
    r = _Rewriter(data, pool, lambda _d: True)
    out = bytearray()
    r.element(out)
    return bytes(out)


def zbytes(b: Optional[bytes]) -> str:
    if b is None:
        return "null"
    return '"' + "".join(f"\\x{x:02x}" for x in b) + '"'


def zpool(pool: CompactPool, zstr) -> str:
    items = [".none"]
    for kind, v in pool.entries:
        if kind == "utf8":
            items.append(f".{{ .utf8 = {zstr(v)} }}")
        elif kind == "int":
            items.append(f".{{ .int = {int(v)} }}")
        elif kind == "long":
            items.append(f".{{ .long = {int(v)} }}")
        elif kind == "float":
            items.append(f".{{ .float = 0x{struct.unpack('>I', v)[0]:08x} }}")
        else:
            items.append(f".{{ .double = 0x{struct.unpack('>Q', v)[0]:016x} }}")
    return "&.{ " + ", ".join(items) + " }"


def java_string_hash(s: str) -> int:
    h = 0
    data = s.encode("utf-16-be")
    for i in range(0, len(data), 2):
        h = (31 * h + ((data[i] << 8) | data[i + 1])) & 0xFFFFFFFF
    return h


def proxy_class_name(interfaces: list[str]) -> str:
    """Must match java.lang.reflect.Proxy (overlay) monoProxyName()."""
    joined = ";".join(i.replace("/", ".") for i in interfaces)
    return f"jdk/proxy/$MonoProxy{java_string_hash(joined):x}"


def inner_info(cf: ClassFile):
    for e in cf.inner_classes:
        if e.inner == cf.name:
            return e
    return None
