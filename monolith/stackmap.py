"""StackMapTable parser (docs/mimari/01_DERLEYICI.md section 4, work package C0).

`frames(cf, m)` returns the verification frames of a method, one per branch target / handler, as
`Frame(pc, locals, stack)`; every verification type is a string:

    "top" "int" "float" "long" "double" "null" "uninit_this"
    "L<internal name>;" for a reference of that class (arrays: the array descriptor, e.g. "[I", "[Ljava/lang/String;")
    "uninit@<pc>"       for an uninitialized object created by the `new` at that pc

`long` and `double` take ONE entry in `locals` and `stack` (JVMS 4.10.1.2 counts two slots; here the entry list is the
logical list, `Frame.slots_locals()` expands it). The implicit initial frame (method descriptor) is `initial_frame`.
Methods without a StackMapTable (straight-line code) have only the initial frame.
"""
from dataclasses import dataclass

from .classfile import ClassConst, ClassFile, ClassFormatError, MethodInfo
from .world import parse_method_desc

ACC_STATIC = 0x0008
_SIMPLE = ("top", "int", "float", "double", "long", "null", "uninit_this")


@dataclass
class Frame:
    pc: int
    locals: list
    stack: list

    def slots_locals(self) -> list:
        out = []
        for t in self.locals:
            out.append(t)
            if t in ("long", "double"):
                out.append("top")
        return out


def _ref(desc_or_name: str) -> str:
    """Internal class name or array descriptor -> verification type string."""
    return desc_or_name if desc_or_name.startswith("[") else "L" + desc_or_name + ";"


def _desc_type(d: str) -> str:
    c = d[0]
    if c in "ZBCSI":
        return "int"
    if c == "F":
        return "float"
    if c == "J":
        return "long"
    if c == "D":
        return "double"
    return d  # "Lx;" or "[..." already a verification string


def initial_frame(cf: ClassFile, m: MethodInfo) -> Frame:
    args, _ret = parse_method_desc(m.desc)
    loc = []
    if not m.access & ACC_STATIC:
        loc.append("uninit_this" if m.name == "<init>" and cf.name != "java/lang/Object" else "L" + cf.name + ";")
    loc.extend(_desc_type(a) for a in args)
    return Frame(-1, loc, [])


def _vtype(r, cp) -> str:
    t = r.u1()
    if t == 0:
        return "top"
    if t == 1:
        return "int"
    if t == 2:
        return "float"
    if t == 3:
        return "double"
    if t == 4:
        return "long"
    if t == 5:
        return "null"
    if t == 6:
        return "uninit_this"
    if t == 7:
        c = cp[r.u2()]
        return _ref(c.name if isinstance(c, ClassConst) else str(c))
    if t == 8:
        return f"uninit@{r.u2()}"
    raise ClassFormatError(f"bad verification type tag {t}")


def frames(cf: ClassFile, m: MethodInfo) -> list[Frame]:
    """The initial frame (pc -1) followed by every explicit frame in pc order."""
    from .classfile import _Reader
    cur = initial_frame(cf, m)
    out = [cur]
    if m.code is None:
        return out
    raw = m.code.attributes.get("StackMapTable")
    if raw is None:
        return out
    r = _Reader(raw)
    n = r.u2()
    pc = -1
    loc = list(cur.locals)
    for _ in range(n):
        ft = r.u1()
        stack: list
        if ft < 64:                                   # same_frame
            delta, stack = ft, []
        elif ft < 128:                                # same_locals_1_stack_item
            delta, stack = ft - 64, [_vtype(r, cf.cp)]
        elif ft == 247:                               # same_locals_1_stack_item_extended
            delta = r.u2()
            stack = [_vtype(r, cf.cp)]
        elif 248 <= ft <= 250:                        # chop
            delta = r.u2()
            for _k in range(251 - ft):
                loc.pop()
            stack = []
        elif ft == 251:                               # same_frame_extended
            delta, stack = r.u2(), []
        elif 252 <= ft <= 254:                        # append
            delta = r.u2()
            for _k in range(ft - 251):
                loc.append(_vtype(r, cf.cp))
            stack = []
        elif ft == 255:                               # full_frame
            delta = r.u2()
            loc = [_vtype(r, cf.cp) for _k in range(r.u2())]
            stack = [_vtype(r, cf.cp) for _k in range(r.u2())]
        else:
            raise ClassFormatError(f"reserved StackMapTable frame type {ft}")
        pc = pc + delta + 1
        out.append(Frame(pc, list(loc), stack))
    if r.p != len(raw):
        raise ClassFormatError("trailing bytes in StackMapTable")
    return out
