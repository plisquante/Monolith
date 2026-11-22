"""JVM class file parser (JVMS chapter 4), Java SE 1.0 .. 25 (major 45..69).

Parses everything the transpiler needs: constant pool, access flags, fields,
methods, Code (bytecode + exception table + sub attributes), BootstrapMethods,
Signature (generics metadata), InnerClasses, EnclosingMethod, NestHost/Members,
Record, PermittedSubclasses, SourceFile, ConstantValue, Exceptions,
MethodParameters and annotations (kept as raw bytes, decoded lazily).
"""
from __future__ import annotations

import struct
from dataclasses import dataclass, field
from typing import Optional

# Constant pool tags
CONSTANT_Utf8 = 1
CONSTANT_Integer = 3
CONSTANT_Float = 4
CONSTANT_Long = 5
CONSTANT_Double = 6
CONSTANT_Class = 7
CONSTANT_String = 8
CONSTANT_Fieldref = 9
CONSTANT_Methodref = 10
CONSTANT_InterfaceMethodref = 11
CONSTANT_NameAndType = 12
CONSTANT_MethodHandle = 15
CONSTANT_MethodType = 16
CONSTANT_Dynamic = 17
CONSTANT_InvokeDynamic = 18
CONSTANT_Module = 19
CONSTANT_Package = 20

# Access flags
ACC_PUBLIC = 0x0001
ACC_PRIVATE = 0x0002
ACC_PROTECTED = 0x0004
ACC_STATIC = 0x0008
ACC_FINAL = 0x0010
ACC_SYNCHRONIZED = 0x0020
ACC_SUPER = 0x0020
ACC_VOLATILE = 0x0040
ACC_BRIDGE = 0x0040
ACC_TRANSIENT = 0x0080
ACC_VARARGS = 0x0080
ACC_NATIVE = 0x0100
ACC_INTERFACE = 0x0200
ACC_ABSTRACT = 0x0400
ACC_STRICT = 0x0800
ACC_SYNTHETIC = 0x1000
ACC_ANNOTATION = 0x2000
ACC_ENUM = 0x4000
ACC_MODULE = 0x8000

# MethodHandle reference kinds
REF_getField = 1
REF_getStatic = 2
REF_putField = 3
REF_putStatic = 4
REF_invokeVirtual = 5
REF_invokeStatic = 6
REF_invokeSpecial = 7
REF_newInvokeSpecial = 8
REF_invokeInterface = 9


class ClassFormatError(Exception):
    pass


@dataclass(frozen=True)
class MemberRef:
    kind: int  # CONSTANT_Fieldref / Methodref / InterfaceMethodref
    owner: str  # internal name, e.g. java/lang/Object (or array descriptor)
    name: str
    desc: str

    @property
    def is_interface(self) -> bool:
        return self.kind == CONSTANT_InterfaceMethodref


@dataclass(frozen=True)
class MethodHandleConst:
    ref_kind: int
    ref: MemberRef


@dataclass(frozen=True)
class MethodTypeConst:
    desc: str


@dataclass(frozen=True)
class ClassConst:
    name: str  # internal name or array descriptor


@dataclass(frozen=True)
class DynamicConst:
    """CONSTANT_Dynamic (condy) or CONSTANT_InvokeDynamic."""
    is_indy: bool
    bsm_index: int
    name: str
    desc: str


@dataclass
class ExceptionHandler:
    start_pc: int
    end_pc: int
    handler_pc: int
    catch_type: Optional[str]  # None == catch-all (finally)


@dataclass
class Code:
    max_stack: int
    max_locals: int
    code: bytes
    handlers: list[ExceptionHandler]
    line_numbers: list[tuple[int, int]] = field(default_factory=list)
    local_vars: list[tuple[int, int, str, str, int]] = field(default_factory=list)
    attributes: dict[str, bytes] = field(default_factory=dict)


@dataclass
class FieldInfo:
    access: int
    name: str
    desc: str
    signature: Optional[str] = None
    constant_value: object = None
    attributes: dict[str, bytes] = field(default_factory=dict)

    @property
    def is_static(self) -> bool:
        return bool(self.access & ACC_STATIC)


@dataclass
class MethodInfo:
    access: int
    name: str
    desc: str
    code: Optional[Code] = None
    signature: Optional[str] = None
    exceptions: list[str] = field(default_factory=list)
    attributes: dict[str, bytes] = field(default_factory=dict)

    @property
    def is_static(self) -> bool:
        return bool(self.access & ACC_STATIC)

    @property
    def is_abstract(self) -> bool:
        return bool(self.access & ACC_ABSTRACT)

    @property
    def is_native(self) -> bool:
        return bool(self.access & ACC_NATIVE)

    @property
    def is_private(self) -> bool:
        return bool(self.access & ACC_PRIVATE)

    @property
    def key(self) -> str:
        return self.name + self.desc


@dataclass
class BootstrapMethod:
    handle: MethodHandleConst
    args: list[object]


@dataclass
class InnerClassEntry:
    inner: Optional[str]
    outer: Optional[str]
    name: Optional[str]
    access: int


@dataclass
class ClassFile:
    minor: int
    major: int
    access: int
    name: str
    super_name: Optional[str]
    interfaces: list[str]
    fields: list[FieldInfo]
    methods: list[MethodInfo]
    cp: list  # resolved constant pool entries (index aligned)
    bootstrap_methods: list[BootstrapMethod] = field(default_factory=list)
    signature: Optional[str] = None
    source_file: Optional[str] = None
    inner_classes: list[InnerClassEntry] = field(default_factory=list)
    enclosing_method: Optional[tuple[str, Optional[str], Optional[str]]] = None
    nest_host: Optional[str] = None
    nest_members: list[str] = field(default_factory=list)
    permitted_subclasses: list[str] = field(default_factory=list)
    record_components: list[tuple[str, str]] = field(default_factory=list)
    attributes: dict[str, bytes] = field(default_factory=dict)
    _mmap: Optional[dict[tuple[str, str], MethodInfo]] = field(default=None, init=False, repr=False, compare=False)
    _fmap_desc: Optional[dict[tuple[str, str], FieldInfo]] = field(default=None, init=False, repr=False, compare=False)
    _fmap_name: Optional[dict[str, FieldInfo]] = field(default=None, init=False, repr=False, compare=False)

    @property
    def is_interface(self) -> bool:
        return bool(self.access & ACC_INTERFACE)

    @property
    def is_abstract(self) -> bool:
        return bool(self.access & ACC_ABSTRACT)

    def method(self, name: str, desc: str) -> Optional[MethodInfo]:
        mm = self._mmap
        if mm is None:
            mm = {}
            for m in self.methods:
                mm.setdefault((m.name, m.desc), m)
            self._mmap = mm
        return mm.get((name, desc))

    def field(self, name: str, desc: Optional[str] = None) -> Optional[FieldInfo]:
        fn = self._fmap_name
        if fn is None:
            fn = {}
            fd = {}
            for f in self.fields:
                fn.setdefault(f.name, f)
                fd.setdefault((f.name, f.desc), f)
            self._fmap_name = fn
            self._fmap_desc = fd
        if desc is None:
            return fn.get(name)
        return self._fmap_desc.get((name, desc))


def _decode_mutf8(b: bytes) -> str:
    """Decode JVM 'modified UTF-8' (0xC0 0x80 for NUL, surrogate pairs encoded separately)."""
    try:
        s = b.decode("utf-8")
        if "\x00" not in s:
            # Fast path: plain UTF-8 does not contain encoded surrogates as 3-byte sequences
            # (python would reject them), so this is correct.
            return s
    except UnicodeDecodeError:
        pass
    out = []
    i = 0
    n = len(b)
    while i < n:
        c = b[i]
        if c < 0x80:
            out.append(chr(c))
            i += 1
        elif (c & 0xE0) == 0xC0:
            out.append(chr(((c & 0x1F) << 6) | (b[i + 1] & 0x3F)))
            i += 2
        else:
            out.append(chr(((c & 0x0F) << 12) | ((b[i + 1] & 0x3F) << 6) | (b[i + 2] & 0x3F)))
            i += 3
    # Keep lone surrogates as-is (Java strings are UTF-16 code unit sequences).
    return "".join(out)


_U2 = struct.Struct(">H").unpack_from
_U4 = struct.Struct(">I").unpack_from
_I4 = struct.Struct(">i").unpack_from
_I8 = struct.Struct(">q").unpack_from


class _Reader:
    __slots__ = ("b", "p")

    def __init__(self, b: bytes, p: int = 0):
        self.b = b
        self.p = p

    def u1(self) -> int:
        v = self.b[self.p]
        self.p += 1
        return v

    def u2(self) -> int:
        v = _U2(self.b, self.p)[0]
        self.p += 2
        return v

    def u4(self) -> int:
        v = _U4(self.b, self.p)[0]
        self.p += 4
        return v

    def bytes(self, n: int) -> bytes:
        p = self.p
        if p + n > len(self.b):
            raise ClassFormatError("truncated class file")
        self.p = p + n
        return self.b[p:p + n]


def parse_class(data: bytes) -> ClassFile:
    u2_from = _U2
    u4_from = _U4
    i4_from = _I4
    i8_from = _I8
    b = data
    nbytes = len(b)
    if nbytes < 10 or u4_from(b, 0)[0] != 0xCAFEBABE:
        raise ClassFormatError("bad magic")
    minor = u2_from(b, 4)[0]
    major = u2_from(b, 6)[0]
    cp_count = u2_from(b, 8)[0]
    p = 10
    raw: list = [None] * cp_count
    i = 1
    while i < cp_count:
        tag = b[p]
        p += 1
        if tag == CONSTANT_Utf8:
            ln = u2_from(b, p)[0]
            p += 2
            raw[i] = (tag, _decode_mutf8(b[p:p + ln]))
            p += ln
        elif tag == CONSTANT_Integer:
            raw[i] = (tag, i4_from(b, p)[0])
            p += 4
        elif tag == CONSTANT_Float:
            raw[i] = (tag, b[p:p + 4])  # keep raw bits (NaN payloads, -0.0)
            p += 4
        elif tag == CONSTANT_Long:
            raw[i] = (tag, i8_from(b, p)[0])
            p += 8
            i += 1
        elif tag == CONSTANT_Double:
            raw[i] = (tag, b[p:p + 8])
            p += 8
            i += 1
        elif tag in (CONSTANT_Class, CONSTANT_String, CONSTANT_MethodType, CONSTANT_Module, CONSTANT_Package):
            raw[i] = (tag, u2_from(b, p)[0])
            p += 2
        elif tag in (CONSTANT_Fieldref, CONSTANT_Methodref, CONSTANT_InterfaceMethodref,
                     CONSTANT_NameAndType, CONSTANT_Dynamic, CONSTANT_InvokeDynamic):
            raw[i] = (tag, u2_from(b, p)[0], u2_from(b, p + 2)[0])
            p += 4
        elif tag == CONSTANT_MethodHandle:
            raw[i] = (tag, b[p], u2_from(b, p + 1)[0])
            p += 3
        else:
            raise ClassFormatError(f"unknown constant pool tag {tag} at {i}")
        i += 1
    r = _Reader(b, p)

    cp: list = [None] * cp_count

    def utf(idx: int) -> str:
        e = raw[idx]
        if e is None or e[0] != CONSTANT_Utf8:
            raise ClassFormatError(f"cp#{idx} not Utf8")
        return e[1]

    def resolve(idx: int):
        if idx == 0:
            return None
        if cp[idx] is not None:
            return cp[idx]
        e = raw[idx]
        tag = e[0]
        if tag == CONSTANT_Utf8:
            v = e[1]
        elif tag == CONSTANT_Integer:
            v = ("int", e[1])
        elif tag == CONSTANT_Float:
            v = ("float", e[1])
        elif tag == CONSTANT_Long:
            v = ("long", e[1])
        elif tag == CONSTANT_Double:
            v = ("double", e[1])
        elif tag == CONSTANT_Class:
            v = ClassConst(utf(e[1]))
        elif tag == CONSTANT_String:
            v = ("string", utf(e[1]))
        elif tag == CONSTANT_MethodType:
            v = MethodTypeConst(utf(e[1]))
        elif tag in (CONSTANT_Module, CONSTANT_Package):
            v = ("modpkg", utf(e[1]))
        elif tag == CONSTANT_NameAndType:
            v = (utf(e[1]), utf(e[2]))
        elif tag in (CONSTANT_Fieldref, CONSTANT_Methodref, CONSTANT_InterfaceMethodref):
            owner = utf(raw[e[1]][1])
            n, d = resolve(e[2])
            v = MemberRef(tag, owner, n, d)
        elif tag == CONSTANT_MethodHandle:
            v = MethodHandleConst(e[1], resolve(e[2]))
        elif tag in (CONSTANT_Dynamic, CONSTANT_InvokeDynamic):
            n, d = resolve(e[2])
            v = DynamicConst(tag == CONSTANT_InvokeDynamic, e[1], n, d)
        else:
            raise ClassFormatError(f"bad tag {tag}")
        cp[idx] = v
        return v

    for j in range(1, cp_count):
        if raw[j] is not None:
            resolve(j)

    access = r.u2()
    name = cp[r.u2()].name
    sup_idx = r.u2()
    super_name = cp[sup_idx].name if sup_idx else None
    interfaces = [cp[r.u2()].name for _ in range(r.u2())]

    def read_attrs() -> dict[str, bytes]:
        attrs = {}
        for _ in range(r.u2()):
            an = utf(r.u2())
            ln = r.u4()
            attrs[an] = r.bytes(ln)
        return attrs

    fields = []
    for _ in range(r.u2()):
        acc = r.u2()
        fn = utf(r.u2())
        fd = utf(r.u2())
        attrs = read_attrs()
        f = FieldInfo(acc, fn, fd, attributes=attrs)
        if "Signature" in attrs:
            f.signature = utf(struct.unpack(">H", attrs["Signature"])[0])
        if "ConstantValue" in attrs:
            f.constant_value = cp[struct.unpack(">H", attrs["ConstantValue"])[0]]
        fields.append(f)

    methods = []
    for _ in range(r.u2()):
        acc = r.u2()
        mn = utf(r.u2())
        md = utf(r.u2())
        attrs = read_attrs()
        m = MethodInfo(acc, mn, md, attributes=attrs)
        if "Signature" in attrs:
            m.signature = utf(struct.unpack(">H", attrs["Signature"])[0])
        if "Exceptions" in attrs:
            ar = _Reader(attrs["Exceptions"])
            m.exceptions = [cp[ar.u2()].name for _ in range(ar.u2())]
        if "Code" in attrs:
            m.code = _parse_code(attrs["Code"], cp, utf)
        methods.append(m)

    cattrs = read_attrs()
    cf = ClassFile(minor, major, access, name, super_name, interfaces, fields, methods, cp, attributes=cattrs)
    if "Signature" in cattrs:
        cf.signature = utf(struct.unpack(">H", cattrs["Signature"])[0])
    if "SourceFile" in cattrs:
        cf.source_file = utf(struct.unpack(">H", cattrs["SourceFile"])[0])
    if "BootstrapMethods" in cattrs:
        ar = _Reader(cattrs["BootstrapMethods"])
        for _ in range(ar.u2()):
            h = cp[ar.u2()]
            args = [cp[ar.u2()] for _ in range(ar.u2())]
            cf.bootstrap_methods.append(BootstrapMethod(h, args))
    if "InnerClasses" in cattrs:
        ar = _Reader(cattrs["InnerClasses"])
        for _ in range(ar.u2()):
            ii, oi, ni, fl = ar.u2(), ar.u2(), ar.u2(), ar.u2()
            cf.inner_classes.append(InnerClassEntry(cp[ii].name if ii else None,
                                                    cp[oi].name if oi else None,
                                                    utf(ni) if ni else None, fl))
    if "EnclosingMethod" in cattrs:
        ar = _Reader(cattrs["EnclosingMethod"])
        ci, mi = ar.u2(), ar.u2()
        nt = cp[mi] if mi else (None, None)
        cf.enclosing_method = (cp[ci].name, nt[0], nt[1])
    if "NestHost" in cattrs:
        cf.nest_host = cp[struct.unpack(">H", cattrs["NestHost"])[0]].name
    if "NestMembers" in cattrs:
        ar = _Reader(cattrs["NestMembers"])
        cf.nest_members = [cp[ar.u2()].name for _ in range(ar.u2())]
    if "PermittedSubclasses" in cattrs:
        ar = _Reader(cattrs["PermittedSubclasses"])
        cf.permitted_subclasses = [cp[ar.u2()].name for _ in range(ar.u2())]
    if "Record" in cattrs:
        ar = _Reader(cattrs["Record"])
        for _ in range(ar.u2()):
            rn, rd = utf(ar.u2()), utf(ar.u2())
            for _ in range(ar.u2()):
                ar.u2()
                skip = ar.u4()
                ar.p += skip
            cf.record_components.append((rn, rd))
    return cf


def _parse_code(b: bytes, cp, utf) -> Code:
    u2_from = _U2
    u4_from = _U4
    max_stack, max_locals, clen = struct.unpack_from(">HHI", b, 0)
    p = 8
    code = b[p:p + clen]
    p += clen
    nhandlers = u2_from(b, p)[0]
    p += 2
    handlers = []
    if nhandlers:
        ex_unpack = struct.Struct(">HHHH").unpack_from
        for _ in range(nhandlers):
            s, e, h, t = ex_unpack(b, p)
            p += 8
            handlers.append(ExceptionHandler(s, e, h, cp[t].name if t else None))
    c = Code(max_stack, max_locals, code, handlers)
    nattrs = u2_from(b, p)[0]
    p += 2
    for _ in range(nattrs):
        an = utf(u2_from(b, p)[0])
        ln = u4_from(b, p + 2)[0]
        p += 6
        ab = b[p:p + ln]
        p += ln
        c.attributes[an] = ab
        if an == "LineNumberTable":
            cnt = u2_from(ab, 0)[0]
            ln_unpack = struct.Struct(">HH").unpack_from
            c.line_numbers = [ln_unpack(ab, 2 + 4 * k) for k in range(cnt)]
    return c
