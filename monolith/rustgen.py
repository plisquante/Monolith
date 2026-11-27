"""Monolith back end, first slice: Java bytecode -> Rust.

Scope (everything else fails loudly with `RustGenError`, never silently):
  * application classes with a superclass chain, instance and static fields, constructors, static initializers
    (run lazily on first use, superclass first), virtual calls through a vtable, `checkcast`/`instanceof`
  * arrays of every primitive type and of references, `multianewarray`, null / bounds / negative-size checks
  * all arithmetic, shift, conversion and comparison opcodes with Java semantics (runtime: monolith/rt/rt.rs)
  * branches, switches, calls between application methods, `System.out.println` for int/long/char/boolean/String
  * not yet: interfaces, exception handlers and `athrow`, string concatenation, the JDK, threads, monitors
Control flow follows the Zig back end: one `loop { match pc { N => { ...; pc = M; continue; } } }` per method,
stack slots and locals are typed Rust variables named `s<depth><kind>` / `l<index><kind>`.

Usage: python3 -m monolith.rustgen --cp <classes dir> --main <Class> -o <out dir> [--run]
"""
from __future__ import annotations

import tempfile
import argparse
import fnmatch
import hashlib
import os
import re
import shutil
import struct
import subprocess
import sys

from .world import LinkError, parse_method_desc
from .vh_modes import VH_MODES
from .reflectmeta import CompactPool, inner_info, rewrite_annotations, rewrite_default, rewrite_param_annotations

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)

RT = {"I": "i32", "J": "i64", "F": "f32", "D": "f64", "A": "rt::Ref"}
DEFAULT = {"I": "0", "J": "0", "F": "0.0", "D": "0.0", "A": "rt::Ref::NULL"}
WIDE = ("J", "D")


class RustGenError(Exception):
    pass


def kind_of(desc: str) -> str:
    c = desc[0]
    if c in "ZBCSI":
        return "I"
    if c in "JFD":
        return c
    if c in "L[":
        return "A"
    raise RustGenError(f"type {desc}")


def rstr(s: str) -> str:
    out = ['"']
    for ch in s:
        if ch == '"':
            out.append('\\"')
        elif ch == "\\":
            out.append("\\\\")
        elif ch == "\n":
            out.append("\\n")
        elif ch == "\r":
            out.append("\\r")
        elif ch == "\t":
            out.append("\\t")
        elif 32 <= ord(ch) < 127:
            out.append(ch)
        else:
            out.append("\\u{%x}" % ord(ch))
    out.append('"')
    return "".join(out)


def rbytes(b) -> str:
    if not b:
        return "&[]"
    return "&[" + ", ".join(str(x) for x in b) + "]"


FT = {"Z": "i8", "B": "i8", "C": "u16", "S": "i16", "I": "i32", "J": "i64", "F": "f32", "D": "f64"}


def ftype(desc: str) -> str:
    return FT.get(desc[0], "rt::Ref")


def mang(name: str) -> str:
    return "".join(c if c.isalnum() else "_" for c in name)


def fname(owner: str, name: str, desc: str) -> str:
    h = hashlib.sha1(desc.encode()).hexdigest()[:6]
    cls = "".join(c if c.isalnum() else "_" for c in owner)
    nm = "".join(c if c.isalnum() else "_" for c in name)
    return f"m_{cls}__{nm}__{h}"


def cret_type(ret: str) -> str:
    return "rt::CVoid" if ret == "V" else f"rt::CRes<{RT[kind_of(ret)]}>"


def cret_wrap(ret: str) -> str:
    return "cvoid" if ret == "V" else "cres"


# Örnekleri her zaman taze oluşturulan (iterator()/listIterator() içinde `return new C(..)`) ve elemanlara
# kendinden hiçbir şey vermeyen yineleyici sınıfları: bir for-each döngüsü yineleyicisinin sahibidir, bu yüzden döngünün sonu
# onu geri verebilir (bkz. gc.rs, "kapsama bağlı nesnelerin geri dönüşümü"). Her sınıf, program üretilirken yine de kendi oluşturma
# noktalarına karşı doğrulanır (`Gen.recycle_slot`).
RECYCLABLE_ITERATORS = (
    "java/util/ArrayList$Itr", "java/util/ArrayList$ListItr", "java/util/AbstractList$Itr", "java/util/AbstractList$ListItr",
    "java/util/ImmutableCollections$ListItr", "java/util/RegularEnumSet$EnumSetIterator", "java/util/HashMap$KeyIterator",
    "java/util/HashMap$ValueIterator", "java/util/HashMap$EntryIterator", "java/util/LinkedHashMap$LinkedKeyIterator",
    "java/util/LinkedHashMap$LinkedValueIterator", "java/util/LinkedHashMap$LinkedEntryIterator", "java/util/EnumMap$EntryIterator",
    "java/util/EnumMap$KeyIterator", "java/util/EnumMap$ValueIterator", "java/util/ArrayDeque$DeqIterator", "java/util/LinkedList$ListItr",
    "java/util/concurrent/CopyOnWriteArrayList$COWIterator", "java/util/concurrent/ConcurrentHashMap$KeyIterator",
    "java/util/concurrent/ConcurrentHashMap$ValueIterator", "java/util/concurrent/ConcurrentHashMap$EntryIterator",
    "it/unimi/dsi/fastutil/objects/ObjectArrayList$1", "it/unimi/dsi/fastutil/objects/ObjectLinkedOpenHashSet$SetIterator",
    "it/unimi/dsi/fastutil/objects/ObjectOpenHashSet$SetIterator", "it/unimi/dsi/fastutil/objects/Object2BooleanOpenHashMap$KeyIterator",
    "it/unimi/dsi/fastutil/objects/Object2ObjectOpenHashMap$KeyIterator", "it/unimi/dsi/fastutil/objects/Object2ObjectOpenHashMap$ValueIterator",
    "it/unimi/dsi/fastutil/ints/Int2ObjectOpenHashMap$ValueIterator", "it/unimi/dsi/fastutil/ints/Int2ObjectOpenHashMap$KeyIterator",
    "it/unimi/dsi/fastutil/longs/Long2ObjectOpenHashMap$ValueIterator", "it/unimi/dsi/fastutil/longs/LongAVLTreeSet$Subset$SubsetIterator",
    "com/google/common/collect/Iterators$SingletonIterator",
)
_ITER_USES = {("hasNext", "()Z"), ("next", "()Ljava/lang/Object;"), ("remove", "()V")}


def find_iterator_scopes(insns) -> dict:
    """for-each shape: `x.iterator()` stored in a local that is only used through hasNext/next/remove until the loop
    exit. Returns {exit pc: [local slots]}: where the loop ends the iterator is dead and can be given back.

    javac reuses a local slot for later variables, so the scope is cut at the loop exit: after it no `aload` of the slot
    may come before the next `astore` of it (otherwise the iterator is still used and nothing is returned)."""
    out: dict = {}
    n = len(insns)
    for i in range(1, n):
        st = insns[i]
        if st.op != "astore":
            continue
        pv = insns[i - 1]
        if pv.op not in ("invokeinterface", "invokevirtual") or pv.arg.name not in ("iterator", "listIterator"):
            continue
        if not pv.arg.desc.endswith(("Ljava/util/Iterator;", "Ljava/util/ListIterator;")):
            continue
        slot = st.arg
        exit_pc = None
        for j in range(i + 1, n - 2):
            if insns[j].op == "aload" and insns[j].arg == slot and insns[j + 1].op in ("invokeinterface", "invokevirtual") \
                    and (insns[j + 1].arg.name, insns[j + 1].arg.desc) == ("hasNext", "()Z") and insns[j + 2].op == "ifeq":
                exit_pc = insns[j + 2].targets[0]
                break
        if exit_pc is None or exit_pc <= st.pc:
            continue
        ok = True
        for j in range(i + 1, n):
            x = insns[j]
            if x.pc >= exit_pc:
                break
            if x.op == "astore" and x.arg == slot:
                ok = False
                break
            if x.op == "aload" and x.arg == slot:
                nx = insns[j + 1] if j + 1 < n else None
                if nx is None or nx.op not in ("invokeinterface", "invokevirtual") or (nx.arg.name, nx.arg.desc) not in _ITER_USES:
                    ok = False
                    break
            if x.op in ("ret", "jsr", "jsr_w"):
                ok = False
                break
        if not ok:
            continue
        for j in range(n):          # after the exit: the slot must be rewritten before it is read again
            x = insns[j]
            if x.pc < exit_pc:
                continue
            if x.op == "astore" and x.arg == slot:
                break
            if x.op == "aload" and x.arg == slot:
                ok = False
                break
        if ok:
            out.setdefault(exit_pc, []).append(slot)
    return out



# java.lang.Math için doğrudan Rust karşılıkları (MONO_INTRINSICS=1). Yalnızca Java'nın tanımıyla HER girdide (NaN, sonsuz,
# -0.0, MIN_VALUE dahil) birebir aynı sonucu veren işlemler buradadır:
#   * int/long max/min: tam sayı karşılaştırması, fark yok.
#   * int/long abs: Java'da Math.abs(MIN_VALUE) == MIN_VALUE (taşar); Rust'ta bunu wrapping_abs verir.
#   * float/double abs: Java işaret bitini temizler (NaN dahil); Rust abs da öyle.
#   * double sqrt: ikisi de IEEE 754'e göre doğru yuvarlanmış karekök.
#   * double floor/ceil: ikisi de IEEE tanımı (NaN, sonsuz ve -0.0 korunur; ceil(-0.5) == -0.0).
# float/double max/min BURADA YOK: Java'da NaN'ı yayar ve -0.0 < 0.0 sayar, Rust'ın max/min'i farklı davranır.
ACC_VOLATILE_F = 0x0040  # alan erişim bayrağı (metotlarda aynı bit ACC_BRIDGE'dir; burada yalnızca alanlarda kullanılır)

MATH_INTRINSICS = {
    ("max", "(II)I"): "{0}.max({1})",
    ("min", "(II)I"): "{0}.min({1})",
    ("max", "(JJ)J"): "{0}.max({1})",
    ("min", "(JJ)J"): "{0}.min({1})",
    ("abs", "(I)I"): "{0}.wrapping_abs()",
    ("abs", "(J)J"): "{0}.wrapping_abs()",
    ("abs", "(F)F"): "{0}.abs()",
    ("abs", "(D)D"): "{0}.abs()",
    ("sqrt", "(D)D"): "{0}.sqrt()",
    ("floor", "(D)D"): "{0}.floor()",
    ("ceil", "(D)D"): "{0}.ceil()",
}

class MethodGen:
    def __init__(self, gen, ci, m):
        self.g, self.p, self.w, self.ci, self.m = gen, gen.prog, gen.world, ci, m
        self.refs: set[str] = set()          # application classes this method refers to
        self.insns = self.p.insns(ci, m)
        self.idx = {x.pc: i for i, x in enumerate(self.insns)}
        self.used_locals: dict[str, str] = {}
        self.used_stack: dict[str, str] = {}
        self.callees: set[tuple[str, str, str]] = set()
        self.where = f"{ci.name}.{m.name}{m.desc}"
        self.noescape = gen.noescape_sites(ci.name, m.key)
        self.has_arena = False
        self.stack_slots: dict[int, tuple] = {}   # `new` sites kept in this function's frame: pc -> (variable, struct)

    # ---------------------------------------------------------------- null denetimi eleme (MONO_NULL_ELIDE)
    _ALLOC_PREFIXES = ("rt::alloc(", "rt::alloc_recyclable(", "rt::alloc_region(", "rt::new_array(", "rt::new_array_arena(",
                       "unsafe { rt::stack_obj(")

    def this_stable(self) -> bool:
        """`this` (yerel 0) metot boyunca hiç yeniden yazılmıyor mu? O zaman l0a her yerde null olmayan alıcıdır
        (Java bir metodu null bir alıcı üzerinde asla çağırmaz)."""
        if not hasattr(self, "_this_stable"):
            self._this_stable = (not self.m.is_static) and not any(
                x.op == "astore" and x.arg == 0 for x in self.insns)
        return self._this_stable

    def known_nonnull(self, var: str, out: list, end: int | None = None) -> bool:
        """`var` (yığın ya da yerel değişkeni) bu noktada kesin null değil mi? Yalnızca AYNI bloğun üretilmiş satırlarına
        geriye doğru bakılır: değişkene son yazılan değer hiç yeniden yazılmamış `this` ya da bir tahsis (`new`, `newarray`)
        ise evet. Başka her durumda (blok dışından gelen değer, çağrı sonucu, satır biçimi tanınmıyor) hayır: denetim kalır."""
        if not self.g.null_elide:
            return False
        if var == "l0a" and self.this_stable():
            return True
        write = re.compile(r"(^|[;{]\s*)" + re.escape(var) + r"\s*=(?!=)")
        i = len(out) if end is None else end
        while i > 0:
            i -= 1
            line = out[i].strip()
            if not write.search(line):
                continue
            if not line.startswith(var + " = ") or not line.endswith(";"):
                return False  # tanınmayan yazım biçimi: güvenli tarafta kal
            rhs = line[len(var) + 3:-1].strip()
            if rhs == "l0a":
                return self.this_stable()
            if rhs.startswith(self._ALLOC_PREFIXES):
                return True
            m = re.fullmatch(r"_t\d+", rhs)
            if m:
                # dup/swap: "{ let _tN = X; ..." satırında _tN'nin kaynağı X; X'in o andaki değerine bak
                let = re.compile(r"\blet " + rhs + r" = ([A-Za-z0-9_]+);")
                j = i
                while j > 0:
                    j -= 1
                    lm = let.search(out[j])
                    if lm:
                        return self.known_nonnull(lm.group(1), out, j)
                return False
            return False
        return False

    def nonthrowing_field_op(self, ins, stk: list, out: list) -> bool:
        """getfield/putfield, alıcı kesin null değilse hata fırlatamaz (alan çözümlemesi derleme anında yapılır).
        MONO_LINE_LAZY bunun önünde bekleyen satırı yazmaz."""
        if ins.op not in ("getfield", "putfield"):
            return False
        try:
            self.field(ins.arg, static=False)  # çözülemeyen alan NoSuchFieldError fırlatır: o zaman satır yazılmalı
        except LinkError:
            return False
        if ins.op == "getfield" and stk and stk[-1] == "A":
            return self.known_nonnull(self.sname(len(stk) - 1, "A"), out)
        if ins.op == "putfield" and len(stk) >= 2 and stk[-2] == "A":
            return self.known_nonnull(self.sname(len(stk) - 2, "A"), out)
        return False

    def nn_ptr(self, var: str, out: list) -> str:
        """Alıcının ham işaretçisi: kesin null değilse doğrudan, değilse rt::nn ile denetlenerek."""
        return f"{var}.0" if self.known_nonnull(var, out) else f"rt::nn({var})?"

    def nn_check(self, var: str, out: list):
        """Yalnızca null denetimi deyimi (çağrıdan önce); kesin null değilse hiç üretilmez."""
        if not self.known_nonnull(var, out):
            out.append(f"rt::nn({var})?;")

    # ---------------------------------------------------------------- yardımcılar
    def err(self, msg: str):
        raise RustGenError(f"{self.where}: {msg}")

    def sname(self, depth: int, k: str) -> str:
        n = f"s{depth}{k.lower()}"
        self.used_stack[n] = k
        return n

    def lname(self, idx: int, k: str) -> str:
        n = f"l{idx}{k.lower()}"
        self.used_locals[n] = k
        return n

    def app_class(self, name: str):
        if name.startswith("["):
            self.err(f"array class {name} used as a plain class")
        return self.g.need_class(name)

    def field(self, a, static: bool):
        self.app_class(a.owner)
        dc, f = self.w.resolve_field(a.owner, a.name, a.desc)
        self.app_class(dc.name)
        if f.is_static != static:
            raise LinkError(f"field {a.owner}.{a.name} static mismatch")
        return dc, f

    def resolve(self, a, special: bool):
        self.app_class(a.owner)
        r = self.p.resolve_special(self.ci, a) if special else self.p.resolve(a)
        if r is None:
            raise LinkError(f"cannot resolve {a.owner}.{a.name}{a.desc}")
        self.app_class(r[0].name)
        self.g.need_method(r[0], r[1])
        return r

    def desc_cref(self, desc: str) -> str:
        """Rust expression (a `&'static rt::Class`) for any JVM type descriptor or internal class name."""
        if desc.startswith("["):
            if len(desc) == 2 and desc[1] in "ZBCSIJFD":
                return f"unsafe {{ &CLS_ARR_{desc[1]} }}"
            return f"rt::array_class_of({self.desc_cref(desc[1:])})"
        if desc.startswith("L") and desc.endswith(";"):
            return f"unsafe {{ &{self.g.class_name(self.app_class(desc[1:-1]))} }}"
        if desc in ("Z", "B", "C", "S", "I", "J", "F", "D", "V"):
            pm = {"Z": "BOOLEAN", "B": "BYTE", "C": "CHAR", "S": "SHORT", "I": "INT",
                  "J": "LONG", "F": "FLOAT", "D": "DOUBLE", "V": "VOID"}[desc]
            return f"&rt::CLS_P_{pm}"
        return f"unsafe {{ &{self.g.class_name(self.app_class(desc))} }}"

    def cref(self, name: str) -> str:
        """Rust expression (a `&Class`) for a class or an array type."""
        if name.startswith("["):
            return self.desc_cref(name)
        return "unsafe { &" + self.g.class_name(self.app_class(name)) + " }"

    def emit_link_err(self, e, err_cls: str, msg: str, stk: list, pops: int, ret_desc: str = "V"):
        for _ in range(pops):
            stk.pop()
        if ret_desc != "V":
            rk = kind_of(ret_desc)
            stk.append(rk)
        self.g.link_errors.append(f"{self.where}: {err_cls}: {msg}")
        e(f"return rt::throw({rstr(err_cls)}, {rstr(msg)}.to_string());")

    VT = {"Z": "i8", "B": "i8", "C": "u16", "S": "i16", "I": "i32", "J": "i64", "F": "f32", "D": "f64"}

    def varhandle(self, ins, stk, e):
        a = ins.arg
        args, ret = parse_method_desc(a.desc)
        family, nvals, volatile_ = VH_MODES[a.name]
        self.g.need_class("java/lang/invoke/VarHandle")
        self.g.vh_used = True
        vals = []
        for _ in range(len(args)):
            k = stk.pop()
            vals.append(self.sname(len(stk), k))
        vals.reverse()
        k = stk.pop()
        vh = self.sname(len(stk), k)
        coords, values = vals[:len(vals) - nvals], vals[len(vals) - nvals:]
        cdesc = args[:len(args) - nvals]
        vdesc = (args[len(args) - nvals:] or [ret])[0]
        nc = len(coords)
        T = self.VT.get(vdesc[0], "rt::Ref")
        t = f"::<{T}>"
        if nc == 2 and cdesc[0] == "Ljava/lang/foreign/MemorySegment;" and cdesc[1] == "J":
            try:
                self.app_class("java/lang/foreign/MemorySegment")
                r = self.p.resolve(type(a)(11, "java/lang/foreign/MemorySegment", "address", "()J"))
                if r is None:
                    raise LinkError("cannot resolve MemorySegment.address()J")
                dc, m2 = r
                self.app_class(dc.name)
                self.g.need_method(dc, m2)
            except LinkError as le:
                self.emit_link_err(e, "java/lang/NoSuchMethodError", str(le), stk, 0, ret)
                return None
            base_call = self.iface_call(m2, coords[0], [], ["rt::Ref"], "rt::CRes<i64>")
            addr_expr = f"({base_call}).wrapping_add({coords[1]})"
            if family == "get":
                call = f"vh::get_seg{t}({str(volatile_).lower()}, {addr_expr})?"
            elif family == "set":
                call = f"vh::set_seg{t}({str(volatile_).lower()}, {addr_expr}, {values[0]})?"
            elif family == "cas":
                call = f"vh::cas_seg{t}({addr_expr}, {values[0]}, {values[1]})?"
            elif family == "cax":
                call = f"vh::cax_seg{t}({addr_expr}, {values[0]}, {values[1]})?"
            elif family == "xchg":
                call = f"vh::get_and_set_seg{t}({addr_expr}, {values[0]})?"
            else:
                op = {"add": 0, "bor": 1, "band": 2, "bxor": 3}[family]
                call = f"vh::get_and_op_seg{t}({op}, {addr_expr}, {values[0]})?"
        else:
            if nc > 2 or (nc >= 1 and kind_of(cdesc[0]) != "A") or (nc == 2 and kind_of(cdesc[1]) != "I"):
                self.err(f"VarHandle coordinates {a.desc}")
            obj = coords[0] if nc >= 1 else "rt::Ref::NULL"
            idx = coords[1] if nc == 2 else "0"
            if family == "get":
                call = f"vh::get{t}({str(volatile_).lower()}, {vh}, {obj}, {idx}, {nc})?"
            elif family == "set":
                call = f"vh::set{t}({str(volatile_).lower()}, {vh}, {obj}, {idx}, {nc}, {values[0]})?"
            elif family == "cas":
                call = f"vh::cas{t}({vh}, {obj}, {idx}, {nc}, {values[0]}, {values[1]})?"
            elif family == "cax":
                call = f"vh::cax{t}({vh}, {obj}, {idx}, {nc}, {values[0]}, {values[1]})?"
            elif family == "xchg":
                call = f"vh::get_and_set{t}({vh}, {obj}, {idx}, {nc}, {values[0]})?"
            else:
                op = {"add": 0, "bor": 1, "band": 2, "bxor": 3}[family]
                call = f"vh::get_and_op{t}({op}, {vh}, {obj}, {idx}, {nc}, {values[0]})?"
        if ret == "V":
            e(call + ";")
        else:
            rk = kind_of(ret)
            stk.append(rk)
            e(f"{self.sname(len(stk) - 1, rk)} = {call};")
        return None

    def itab_expr(self, cls: str, sel) -> str:
        """Arayüz çağrısının hedef metodunu bulan ifade: önbellekli (MONO_ICACHE=1) ya da her seferinde arayan eski yol."""
        if self.g.icache:
            return f"rt::itab_find_ic({{ static IC: rt::ICache = rt::ICache::new(); &IC }}, {cls}, {sel})"
        return f"rt::itab_find({cls}, {sel})"

    def iface_call(self, m2, recv, vals, ptypes, cret) -> str:
        sel = self.g.sel(m2.name, m2.desc)
        fp = (f"unsafe {{ std::mem::transmute::<*const (), extern \"C\" fn({', '.join(ptypes)}) -> {cret}>"
              f"({self.itab_expr(f'(*rt::nn({recv})?).class', sel)}) }}")
        return f"({fp})({', '.join([recv] + vals)}).res()?"

    def init_class(self, ci2, emit):
        if self.g.init_elide and self.already_initialized(ci2):
            return
        if self.g.needs_init(ci2):
            emit(f"unsafe {{ {self.g.init_name(ci2)}() }}.res()?;")

    def already_initialized(self, ci2) -> bool:
        """Bu metot çalışırken `ci2` kesinlikle başlatılmış mı (ya da bu iş parçacığı tarafından başlatılıyor mu)?

        JVM kuralları (JLS 12.4): bir sınıfın metodu ancak sınıf başlatıldıktan sonra (ya da başlatıcısının içinden)
        çalışabilir: statik metot çağrısı ve `new` önce sınıfı başlatır, örnek metodu için o sınıfın bir nesnesi olmalıdır.
        Bir sınıf başlatılmadan önce de üst sınıfı başlatılır. Dolayısıyla metodun kendi sınıfı ve onun üst sınıfları
        (arayüzler değil) için denetim her zaman "zaten yapıldı" cevabını verir; o çağrıyı üretmemek davranışı değiştirmez.
        Arayüz metotları için de kendi arayüzü başlatılmıştır (statik çağrı başlatır; varsayılan metot çalışıyorsa onu
        uygulayan sınıf başlatılmıştır ve varsayılan metot bildiren üst arayüzler onunla birlikte başlatılır).
        """
        c = self.ci
        while c is not None:
            if c is ci2 or c.name == ci2.name:
                return True
            c = c.super
        return False

    @staticmethod
    def kind_of_field(desc: str) -> str:
        return kind_of(desc)

    @staticmethod
    def from_field(desc: str, expr: str) -> str:
        return f"{expr} as i32" if desc[0] in "ZBCS" else expr

    @staticmethod
    def to_field(desc: str, v: str) -> str:
        if desc[0] == "Z":
            return f"({v} & 1) as i8"
        if desc[0] in "BCS":
            return f"{v} as {FT[desc[0]]}"
        return v

    # ---------------------------------------------------------------- tek bir komut
    def gen_insn(self, ins, stk: list, out: list, leaders: set):
        """Emits code for `ins`, mutates `stk` (list of kinds). Returns the successor pcs or None for fall-through."""
        op, a = ins.op, ins.arg
        e = out.append

        def push(k, expr):
            stk.append(k)
            e(f"{self.sname(len(stk) - 1, k)} = {expr};")

        def top(i=1):
            k = stk[-i]
            return self.sname(len(stk) - i, k)

        def pop():
            k = stk.pop()
            return self.sname(len(stk), k)

        if op == "nop":
            return None
        if op == "aconst_null":
            push("A", "rt::Ref::NULL")
        elif op.startswith("iconst_"):
            push("I", "-1" if op == "iconst_m1" else op[-1])
        elif op in ("bipush", "sipush"):
            push("I", str(a))
        elif op.startswith("lconst_"):
            push("J", op[-1])
        elif op.startswith("fconst_"):
            push("F", op[-1] + ".0")
        elif op.startswith("dconst_"):
            push("D", op[-1] + ".0")
        elif op == "ldc":
            if isinstance(a, tuple):
                t, v = a
                if t == "int":
                    push("I", f"{v}i32" if v >= 0 else f"({v}i32)")
                elif t == "long":
                    push("J", f"{v}i64" if v >= 0 else f"({v}i64)")
                elif t == "float":
                    push("F", f"f32::from_bits(0x{struct.unpack('>I', v)[0]:08x})")
                elif t == "double":
                    push("D", f"f64::from_bits(0x{struct.unpack('>Q', v)[0]:016x})")
                elif t == "string":
                    slot, data = self.g.lit(v)
                    push("A", f"unsafe {{ rt::lit(&raw mut {slot}, &{data}) }}")
                else:
                    self.err(f"ldc {t}")
            elif hasattr(a, "name") and not hasattr(a, "bsm_index"):
                try:
                    ce = self.cref(a.name)
                    push("A", f"rt::class_mirror({ce})?")
                except LinkError as le:
                    self.emit_link_err(e, "java/lang/NoClassDefFoundError", str(le), stk, 0, "Ljava/lang/Class;")
                    return None
            else:
                self.g.errors.append(f"{self.where}: ldc {type(a).__name__}")
                stk.append("A")
                e(f"return rt::unsupported_code({rstr(f'{self.where}: ldc {type(a).__name__}')});")
                return None
        elif op in ("iload", "lload", "fload", "dload", "aload"):
            k = {"i": "I", "l": "J", "f": "F", "d": "D", "a": "A"}[op[0]]
            push(k, self.lname(a, k))
        elif op in ("istore", "lstore", "fstore", "dstore", "astore"):
            k = {"i": "I", "l": "J", "f": "F", "d": "D", "a": "A"}[op[0]]
            if stk[-1] != k:
                self.err(f"{op} of a {stk[-1]} value")
            e(f"{self.lname(a, k)} = {pop()};")
        elif op == "iinc":
            n = self.lname(a, "I")
            e(f"{n} = {n}.wrapping_add({ins.arg2});")
        elif op == "pop":
            stk.pop()
        elif op == "pop2":
            if stk.pop() not in WIDE:
                stk.pop()
        elif op in ("dup", "dup_x1", "dup_x2", "dup2", "dup2_x1", "dup2_x2", "swap"):
            w = lambda k: k in WIDE
            if op == "dup": nin, perm = 1, (0, 0)
            elif op == "dup_x1": nin, perm = 2, (1, 0, 1)
            elif op == "dup_x2": nin, perm = (2, (1, 0, 1)) if w(stk[-2]) else (3, (2, 0, 1, 2))
            elif op == "dup2": nin, perm = (1, (0, 0)) if w(stk[-1]) else (2, (0, 1, 0, 1))
            elif op == "dup2_x1": nin, perm = (2, (1, 0, 1)) if w(stk[-1]) else (3, (1, 2, 0, 1, 2))
            elif op == "dup2_x2":
                if w(stk[-1]):
                    nin, perm = (2, (1, 0, 1)) if w(stk[-2]) else (3, (2, 0, 1, 2))
                else:
                    nin, perm = (3, (1, 2, 0, 1, 2)) if w(stk[-3]) else (4, (2, 3, 0, 1, 2, 3))
            else:
                nin, perm = 2, (1, 0)
            base = len(stk) - nin
            seg = [(stk[base + j], self.sname(base + j, stk[base + j])) for j in range(nin)]
            tmp = [f"_t{j}" for j in range(nin)]
            e("{ " + " ".join(f"let {t} = {s};" for t, (_, s) in zip(tmp, seg)))
            del stk[base:]
            for p_i in perm:
                k = seg[p_i][0]
                stk.append(k)
                e(f"{self.sname(len(stk) - 1, k)} = {tmp[p_i]};")
            e("}")
        elif op in BINOPS:
            k, rust = BINOPS[op]
            b = pop(); a_ = pop()
            res = self.sname(len(stk), k)
            stk.append(k)
            e(f"{res} = {rust.format(a=a_, b=b)};")
        elif op in UNOPS:
            k, rust = UNOPS[op]
            x = pop()
            res = self.sname(len(stk), k)
            stk.append(k)
            e(f"{res} = {rust.format(a=x)};")
        elif op in CONVS:
            k, rust = CONVS[op]
            x = pop()
            res = self.sname(len(stk), k)
            stk.append(k)
            e(f"{res} = {rust.format(a=x)};")
        elif op in CMPS:
            rust = CMPS[op]
            b = pop(); a_ = pop()
            res = self.sname(len(stk), "I")
            stk.append("I")
            e(f"{res} = {rust.format(a=a_, b=b)};")
        elif op in IF1:
            x = pop()
            e(f"if {IF1[op].format(a=x)} {{ pc = {ins.targets[0]}; continue; }}")
        elif op in IF2:
            b = pop(); a_ = pop()
            if op in ("if_acmpeq", "if_acmpne") and self.g.region_classes:
                # bölge etkin: asıl nesne ile yığın kopyası aynı nesnedir (iletme işaretçisi izlenir)
                cond = f"{'' if op == 'if_acmpeq' else '!'}rt::ref_eq({a_}, {b})"
            else:
                cond = IF2[op].format(a=a_, b=b)
            e(f"if {cond} {{ pc = {ins.targets[0]}; continue; }}")
        elif op == "goto":
            e(f"pc = {ins.targets[0]}; continue;")
            return [ins.targets[0]]
        elif op == "tableswitch":
            x = pop()
            e(f"match {x} {{")
            for key, tgt in ins.arg2:
                e(f"    {key} => {{ pc = {tgt}; continue; }}")
            e(f"    _ => {{ pc = {ins.arg}; continue; }}")
            e("}")
            return list(dict.fromkeys(ins.targets))
        elif op in ("ireturn", "lreturn", "freturn", "dreturn", "areturn"):
            e(f"return Ok({pop()});")
            return []
        elif op == "return":
            e("return Ok(());")
            return []
        elif op == "getstatic":
            try:
                dc, f = self.field(a, static=True)
            except LinkError as le:
                self.emit_link_err(e, "java/lang/NoSuchFieldError", str(le), stk, 0, a.desc)
                return None
            self.init_class(dc, e)
            if f.access & ACC_VOLATILE_F:
                # Java volatile: edinme çitli okuma (rt::vol_load); arm64'te düz okuma kilitsiz yapılarda görev kaybettirir
                push(self.kind_of_field(f.desc), self.from_field(f.desc, f"unsafe {{ rt::vol_load(&raw const {self.g.static_name(dc, f)}) }}"))
            else:
                push(self.kind_of_field(f.desc), self.from_field(f.desc, f"unsafe {{ {self.g.static_name(dc, f)} }}"))
        elif op == "putstatic":
            try:
                dc, f = self.field(a, static=True)
            except LinkError as le:
                self.emit_link_err(e, "java/lang/NoSuchFieldError", str(le), stk, 1, "V")
                return None
            self.init_class(dc, e)
            v = pop()
            vol = bool(f.access & ACC_VOLATILE_F)
            if f.desc[0] in "L[":
                # referans yazımı: SATB yazma bariyeri (rt::satb_store); işaretleme etkin değilken işlem yapmaz
                st = f"rt::{'satb_store_r' if self.g.region_classes else 'satb_store'}(&raw mut {self.g.static_name(dc, f)}, {v});"
                e(f"unsafe {{ rt::vol_fence_before(); {st} rt::vol_fence_after(); }}" if vol else f"unsafe {{ {st} }}")
            elif vol:
                e(f"unsafe {{ rt::vol_store(&raw mut {self.g.static_name(dc, f)}, {self.to_field(f.desc, v)}); }}")
            else:
                e(f"unsafe {{ {self.g.static_name(dc, f)} = {self.to_field(f.desc, v)}; }}")
        elif op == "getfield":
            try:
                dc, f = self.field(a, static=False)
                st_name = self.g.struct_name(self.w.req(a.owner))
            except LinkError as le:
                self.emit_link_err(e, "java/lang/NoSuchFieldError", str(le), stk, 1, a.desc)
                return None
            o = pop()
            if f.access & ACC_VOLATILE_F:
                # Java volatile: edinme çitli okuma (rt::vol_load)
                rd = f"unsafe {{ rt::vol_load(&raw const (*({self.nn_ptr(o, out)} as *mut {st_name})).{self.g.field_name(dc, f)}) }}"
            else:
                rd = f"unsafe {{ (*({self.nn_ptr(o, out)} as *mut {st_name})).{self.g.field_name(dc, f)} }}"
            if dc.name == "java/lang/ref/Reference" and f.name == "referent":
                # zayıf referans okuma bariyeri: eşzamanlı işaretleme sürerken okunan hedef canlı tutulur (gc.rs weak_read)
                rd = f"rt::weak_read({rd})"
            push(self.kind_of_field(f.desc), self.from_field(f.desc, rd))
        elif op == "putfield":
            try:
                dc, f = self.field(a, static=False)
                st_name = self.g.struct_name(self.w.req(a.owner))
            except LinkError as le:
                self.emit_link_err(e, "java/lang/NoSuchFieldError", str(le), stk, 2, "V")
                return None
            v = pop()
            o = pop()
            vol = bool(f.access & ACC_VOLATILE_F)
            place = f"(*({self.nn_ptr(o, out)} as *mut {st_name})).{self.g.field_name(dc, f)}"
            if f.desc[0] in "L[":
                # referans yazımı: SATB yazma bariyeri (rt::satb_store); işaretleme etkin değilken işlem yapmaz
                st = f"rt::{'satb_store_r' if self.g.region_classes else 'satb_store'}(&raw mut {place}, {v});"
                e(f"unsafe {{ rt::vol_fence_before(); {st} rt::vol_fence_after(); }}" if vol else f"unsafe {{ {st} }}")
            elif vol:
                # Java volatile: çitli yazma (rt::vol_store)
                e(f"unsafe {{ rt::vol_store(&raw mut {place}, {self.to_field(f.desc, v)}); }}")
            else:
                e(f"unsafe {{ {place} = {self.to_field(f.desc, v)}; }}")
        elif op == "new":
            try:
                ci2 = self.app_class(a.name)
                cr = self.cref(a.name)
            except LinkError as le:
                self.emit_link_err(e, "java/lang/NoClassDefFoundError", str(le), stk, 0, "Ljava/lang/Object;")
                return None
            self.init_class(ci2, e)
            rslot = self.g.recycle_slot(a.name)
            if ins.pc in self.noescape:
                # nesne bu çağrıdan (ya da bu döngü turundan) uzun yaşayamaz: bu fonksiyonun çerçevesinde bir yuva
                var = f"stk{ins.pc}"
                self.stack_slots[ins.pc] = (var, self.g.struct_name(ci2))
                push("A", f"unsafe {{ rt::stack_obj(&raw mut {var} as *mut u8, {cr}) }}")
            elif self.g.is_region_class(a.name):
                push("A", f"rt::alloc_region({cr})")
            elif rslot is not None:
                push("A", f"rt::alloc_recyclable({cr}, {rslot})")
            else:
                push("A", f"rt::alloc({cr})")
        elif op == "newarray":
            n = pop()
            if ins.pc in self.noescape:
                self.has_arena = True
                push("A", f"rt::new_array_arena(b'{a}', {n})?")
            else:
                push("A", f"rt::new_array(b'{a}', {n})?")
        elif op == "anewarray":
            n = pop()
            try:
                cr = self.desc_cref(a.name)
            except LinkError as le:
                self.emit_link_err(e, "java/lang/NoClassDefFoundError", str(le), stk, 0, "Ljava/lang/Object;")
                return None
            if ins.pc in self.noescape:
                self.has_arena = True
                push("A", f"rt::new_obj_array_arena({cr}, {n})?")
            else:
                push("A", f"rt::new_obj_array({cr}, {n})?")
        elif op == "multianewarray":
            dims = [None] * ins.arg2
            for j in range(ins.arg2 - 1, -1, -1):
                dims[j] = pop()
            desc = a.name
            depth = len(desc) - len(desc.lstrip("["))
            leaf = desc[depth]
            if leaf == "L":
                try:
                    ec = self.desc_cref(desc[depth:])
                except LinkError as le:
                    self.emit_link_err(e, "java/lang/NoClassDefFoundError", str(le), stk, 0, "Ljava/lang/Object;")
                    return None
                push("A", f"rt::new_multi(Some({ec}), b'A', {depth}, &[{', '.join(dims)}])?")
            else:
                push("A", f"rt::new_multi(None, b'{leaf}', {depth}, &[{', '.join(dims)}])?")
        elif op == "arraylength":
            push("I", f"rt::alen({pop()})?")
        elif op in ALOAD:
            idx = pop(); arr = pop()
            k, ty, cast = ALOAD[op]
            push(k, f"rt::aload::<{ty}>({arr}, {idx})?{cast}")
        elif op in ASTORE:
            v = pop(); idx = pop(); arr = pop()
            ty, cast = ASTORE[op]
            if op == "bastore":
                e(f"rt::bastore({arr}, {idx}, {v})?;")
            elif op == "aastore":
                e(f"rt::aastore_ref({arr}, {idx}, {v})?;")
            else:
                e(f"rt::astore::<{ty}>({arr}, {idx}, {v}{cast})?;")
        elif op == "checkcast":
            try:
                cr = self.cref(a.name)
            except LinkError as le:
                self.emit_link_err(e, "java/lang/NoClassDefFoundError", str(le), stk, 1, "Ljava/lang/Object;")
                return None
            if self.g.icache:
                e(f"{top()} = rt::checkcast_ic({top()}, {cr}, {{ static IC: rt::ICache = rt::ICache::new(); &IC }})?;")
            else:
                e(f"{top()} = rt::checkcast({top()}, {cr})?;")
        elif op == "instanceof":
            try:
                cr = self.cref(a.name)
            except LinkError as le:
                self.emit_link_err(e, "java/lang/NoClassDefFoundError", str(le), stk, 1, "I")
                return None
            x = pop()
            if self.g.icache:
                push("I", f"rt::instance_of_ic({x}, {cr}, {{ static IC: rt::ICache = rt::ICache::new(); &IC }}) as i32")
            else:
                push("I", f"rt::instance_of({x}, {cr}) as i32")
        elif op == "invokestatic":
            args, ret = parse_method_desc(a.desc)
            try:
                dc, m2 = self.resolve(a, special=False)
            except LinkError as le:
                self.emit_link_err(e, "java/lang/NoSuchMethodError", str(le), stk, len(args), ret)
                return None
            intr = MATH_INTRINSICS.get((m2.name, m2.desc)) if (self.g.intrinsics and dc.name == "java/lang/Math"
                                                               and (dc.name, m2.name, m2.desc) not in self.g.patches) else None
            if intr is not None:
                # Doğrudan Rust işlemi: çağrı yok, sınıf başlatma denetimi yok (Math'in başlatıcısının gözlenebilir bir yan
                # etkisi yoktur), derleyici döngüyü vektörleştirebilir. Her ifade Java'nın tanımıyla birebir aynı sonucu verir.
                vals = [None] * len(args)
                for j in range(len(args) - 1, -1, -1):
                    vals[j] = pop()
                push(kind_of(ret), intr.format(*vals))
                return None
            self.init_class(dc, e)
            vals = [None] * len(args)
            for j in range(len(args) - 1, -1, -1):
                vals[j] = pop()
            if self.g.caller_sensitive(dc, m2):
                e(f"rt::set_caller({self.cref(self.ci.name)});")
            call = self.g.call_direct(self.ci.name, dc, m2, vals)
            if ret == "V":
                e(call + ";")
            else:
                push(kind_of(ret), call)
        elif op == "invokespecial":
            args, ret = parse_method_desc(a.desc)
            if a.owner == "java/lang/Object" and a.name == "<init>" and ("java/lang/Object", "<init>", "()V") not in self.g.patches:
                for _ in range(len(args)):
                    pop()
                recv = pop()
                self.nn_check(recv, out)
            else:
                try:
                    dc, m2 = self.resolve(a, special=True)
                except LinkError as le:
                    self.emit_link_err(e, "java/lang/NoSuchMethodError", str(le), stk, len(args) + 1, ret)
                    return None
                vals = [None] * len(args)
                for j in range(len(args) - 1, -1, -1):
                    vals[j] = pop()
                recv = pop()
                if self.g.caller_sensitive(dc, m2):
                    e(f"rt::set_caller({self.cref(self.ci.name)});")
                call = self.g.call_direct(self.ci.name, dc, m2, [recv] + vals)
                if ret == "V":
                    e(call + ";")
                else:
                    push(kind_of(ret), call)
        elif op == "invokeinterface":
            args, ret = parse_method_desc(a.desc)
            try:
                dc, m2 = self.resolve(a, special=False)
            except LinkError as le:
                self.emit_link_err(e, "java/lang/NoSuchMethodError", str(le), stk, len(args) + 1, ret)
                return None
            vals = [None] * len(args)
            for j in range(len(args) - 1, -1, -1):
                vals[j] = pop()
            recv = pop()
            ptypes = ["rt::Ref"] + [RT[kind_of(d)] for d in args]
            cret = cret_type(ret)
            obj_slot = self.w.vindex(self.w.req("java/lang/Object")).get(m2.name + m2.desc)
            if m2.is_private:
                self.g.need_method(dc, m2)
                self.nn_check(recv, out)
                call = self.g.call_direct(self.ci.name, dc, m2, [recv] + vals)
            elif obj_slot is not None:
                if (m2.access & 0x0010):
                    self.nn_check(recv, out)
                    call = self.g.call_direct(self.ci.name, dc, m2, [recv] + vals)
                else:
                    fp = f"unsafe {{ std::mem::transmute::<*const (), extern \"C\" fn({', '.join(ptypes)}) -> {cret}>((*(*rt::nn({recv})?).class).vtable[{obj_slot}].0) }}"
                    call = f"({fp})({', '.join([recv] + vals)}).res()?"
            else:
                owner_ci = self.app_class(a.owner)
                dv = self.g.devirt_targets(owner_ci, m2.name, m2.desc)
                if dv is not None and dv[0] == "single" and not self.g.caller_sensitive(dv[1][0], dv[1][1]):
                    tdc, tm2 = dv[1]
                    self.g.need_class(tdc.name)
                    self.g.need_method(tdc, tm2)
                    self.nn_check(recv, out)
                    call = self.g.call_direct(self.ci.name, tdc, tm2, [recv] + vals)
                elif dv is not None and dv[0] == "poly" and not any(self.g.caller_sensitive(x[1], x[2]) for x in dv[1]):
                    arms_code = []
                    for idx_p, (tc, tdc, tm2) in enumerate(dv[1]):
                        self.g.need_class(tdc.name)
                        self.g.need_method(tdc, tm2)
                        c_ref = self.cref(tc.name)
                        d_call = self.g.call_direct(self.ci.name, tdc, tm2, ["r__"] + vals)
                        kw = "if" if idx_p == 0 else "else if"
                        arms_code.append(f"{kw} std::ptr::eq(c__, {c_ref}) {{ {d_call} }}")
                    sel = self.g.sel(m2.name, m2.desc)
                    fb_fp = f"unsafe {{ std::mem::transmute::<*const (), extern \"C\" fn({', '.join(ptypes)}) -> {cret}>({self.itab_expr('(*o__).class', sel)}) }}"
                    fb = f"({fb_fp})({', '.join(['r__'] + vals)}).res()?"
                    call = f"{{ let r__ = {recv}; let o__ = rt::nn(r__)?; let c__ = unsafe {{ (*o__).class as *const rt::Class }}; {' '.join(arms_code)} else {{ {fb} }} }}"
                else:
                    call = self.iface_call(m2, recv, vals, ptypes, cret)
            if ret == "V":
                e(call + ";")
            else:
                push(kind_of(ret), call)
        elif op == "invokevirtual":
            if a.owner == "java/lang/invoke/VarHandle" and a.name in VH_MODES:
                return self.varhandle(ins, stk, e)
            if a.owner.startswith("[") and a.name == "clone" and a.desc == "()Ljava/lang/Object;":
                push("A", f"rt::array_clone({pop()})?")
                return None
            owner_name = "java/lang/Object" if a.owner.startswith("[") else a.owner
            args, ret = parse_method_desc(a.desc)
            try:
                owner = self.app_class(owner_name)
                r = self.p.resolve(a if not a.owner.startswith("[") else type(a)(a.tag, owner_name, a.name, a.desc))
                if r is None:
                    raise LinkError(f"cannot resolve {a.owner}.{a.name}{a.desc}")
                dc, m2 = r
                self.app_class(dc.name)
                self.g.need_method(dc, m2)
            except LinkError as le:
                self.emit_link_err(e, "java/lang/NoSuchMethodError", str(le), stk, len(args) + 1, ret)
                return None
            vals = [None] * len(args)
            for j in range(len(args) - 1, -1, -1):
                vals[j] = pop()
            recv = pop()
            ptypes = ["rt::Ref"] + [RT[kind_of(d)] for d in args]
            cret = cret_type(ret)
            if self.g.caller_sensitive(dc, m2):
                e(f"rt::set_caller({self.cref(self.ci.name)});")
            if m2.is_private or (m2.access & 0x0010) or (dc.cf.access & 0x0010):
                self.nn_check(recv, out)
                call = self.g.call_direct(self.ci.name, dc, m2, [recv] + vals)
            else:
                dv = None if self.g.caller_sensitive(dc, m2) else self.g.devirt_targets(owner, m2.name, m2.desc)
                if dv is not None and dv[0] == "single" and not self.g.caller_sensitive(dv[1][0], dv[1][1]):
                    tdc, tm2 = dv[1]
                    self.g.need_class(tdc.name)
                    self.g.need_method(tdc, tm2)
                    self.nn_check(recv, out)
                    call = self.g.call_direct(self.ci.name, tdc, tm2, [recv] + vals)
                else:
                    slot = (self.w.vindex(self.w.req("java/lang/Object")).get(m2.name + m2.desc)
                            if owner.is_interface else self.w.vindex(owner).get(m2.name + m2.desc))
                    if slot is None:
                        call = self.iface_call(m2, recv, vals, ptypes, cret)
                        if ret == "V":
                            e(call + ";")
                        else:
                            push(kind_of(ret), call)
                        return None
                    fp = f"unsafe {{ std::mem::transmute::<*const (), extern \"C\" fn({', '.join(ptypes)}) -> {cret}>((*(*rt::nn({recv})?).class).vtable[{slot}].0) }}"
                    call = f"({fp})({', '.join([recv] + vals)}).res()?"
            if ret == "V":
                e(call + ";")
            else:
                push(kind_of(ret), call)
        elif op in ("monitorenter", "monitorexit"):
            e(f"rt::{op}({pop()})?;")
        elif op == "nullchk":            # pseudo instruction of the inliner (monolith/inline.py)
            e(f"rt::nn({pop()})?;")
        elif op == "initclass":
            self.init_class(self.app_class(a), e)
        elif op == "athrow":
            e(f"return rt::throw_ref({pop()});")
            return []
        else:
            self.err(f"opcode {op}")
        return None

    # ---------------------------------------------------------------- tüm metot
    def generate(self) -> str:
        if self.m.code is None:
            self.err("native/abstract methods are not in this slice")
        pe = self.g.patches.by_key.get((self.ci.name, self.m.name, self.m.desc))
        if pe is not None and pe.replace:
            # yama metodu değiştirir: orijinal çevrilmez (hiçbir şey onu çağırmaz; RTA onu izlemedi)
            return self.assemble({0: [f"return rt::unsupported_code({rstr('replaced by ' + pe.fn)});"]}, False)
        blocks, has_sp = self.build_blocks()
        return self.assemble(blocks, has_sp)

    def build_blocks(self):
        """Statement lists per basic block (key: pc) and whether a safepoint counter is needed."""
        m, insns = self.m, self.insns
        handlers = m.code.handlers
        leaders = {0}
        for i, x in enumerate(insns):
            for t in x.targets:
                leaders.add(t)
            if (x.targets or x.op in ("ireturn", "lreturn", "freturn", "dreturn", "areturn", "return", "athrow")) \
                    and i + 1 < len(insns):
                leaders.add(insns[i + 1].pc)
        entry: dict[int, list] = {0: []}
        work = [0]
        for h in handlers:
            leaders.add(h.handler_pc)
            if h.handler_pc not in entry:
                entry[h.handler_pc] = ["A"]
                work.append(h.handler_pc)
        blocks: dict[int, list] = {}
        has_sp = False
        while work:
            bpc = work.pop()
            stk = list(entry[bpc])
            lines: list = []
            i = self.idx[bpc]
            succs: list = []
            line_at = dict(getattr(self.m.code, "line_numbers", None) or [])
            pending_line = None
            lazy = self.g.line_lazy
            while True:
                ins = insns[i]
                if handlers:
                    lines.append(f"pc = {ins.pc};")
                if ins.pc in line_at:
                    # bu ifadenin kaynak satırı: yığın izleri, dışarıya çağrı yapan her çerçeve için bunu okur
                    if lazy:
                        pending_line = line_at[ins.pc]
                    else:
                        lines.append(f"rt::{self.g.set_line_fn}({line_at[ins.pc]});")
                if lazy and pending_line is not None and (ins.op not in LINE_PURE_OPS or ins.targets) \
                        and not self.nonthrowing_field_op(ins, stk, lines):
                    # MONO_LINE_LAZY: satır, hata fırlatabilen ya da çağrı yapan/dallanan ilk komuttan hemen önce yazılır.
                    # Arada yalnızca hatasız komutlar (LINE_PURE_OPS) olduğundan yığın izinde görülen satır aynıdır.
                    lines.append(f"rt::{self.g.set_line_fn}({pending_line});")
                    pending_line = None
                if ins.targets and any(t <= ins.pc for t in ins.targets):
                    has_sp = True
                    lines.append("_sp_tick = _sp_tick.wrapping_add(1); if (_sp_tick & 1023) == 0 { rt::gc_safepoint(); }")
                r = self.gen_insn(ins, stk, lines, leaders)
                if r is not None:
                    succs = r
                    break
                if ins.op in ("ireturn", "lreturn", "freturn", "dreturn", "areturn", "return"):
                    break
                # koşullu dal hedefleri de ardıl olur
                if ins.targets:
                    succs.extend(ins.targets)
                i += 1
                if i >= len(insns):
                    break
                nxt = insns[i].pc
                if nxt in leaders:
                    if pending_line is not None:
                        # bloktan çıkmadan önce bekleyen satır yazılır: sonraki blok bu değeri kalıtır
                        lines.append(f"rt::{self.g.set_line_fn}({pending_line});")
                        pending_line = None
                    lines.append(f"pc = {nxt}; continue;")
                    succs.append(nxt)
                    break
            blocks[bpc] = lines
            for s in succs:
                if s not in entry:
                    entry[s] = list(stk)
                    work.append(s)
        for exit_pc, slots in find_iterator_scopes(insns).items():
            if exit_pc in blocks:
                for sl in slots:
                    ln = self.lname(sl, "A")
                    blocks[exit_pc].insert(0, f"rt::recycle_scope({ln}); {ln} = rt::Ref::NULL;")
        return blocks, has_sp

    def param_init(self, slot: int, k: str, src: str) -> str:
        """Statement that moves argument `src` into the method's variable for local `slot` (kind k)."""
        return f"{self.lname(slot, k)} = {src};"

    def assemble(self, blocks, has_sp) -> str:
        m = self.m
        handlers = m.code.handlers
        args, ret = parse_method_desc(m.desc)
        params = []
        init = []
        slot = 0
        if not m.is_static:
            params.append("this: rt::Ref")
            init.append(self.param_init(0, "A", "this"))
            slot = 1
        for j, d in enumerate(args):
            k = kind_of(d)
            params.append(f"a{j}: {RT[k]}")
            init.append(self.param_init(slot, k, f"a{j}"))
            slot += 2 if k in WIDE else 1
        rret = "()" if ret == "V" else RT[kind_of(ret)]
        if handlers:
            self.sname(0, "A")
        if self.g.fast_frames:
            body = [f"    static FI: rt::FrameInfo = rt::FrameInfo {{ class: {rstr(self.ci.name)}, method: {rstr(m.name)}, "
                    f"desc: {rstr(m.desc)}, file: {rstr(self.ci.cf.source_file or '')} }};",
                    "    let _frame = rt::enter_frame_s(&FI)?;"]
        else:
            body = ["    rt::stack_check()?;",
                    f"    let _frame = rt::enter_frame({rstr(self.ci.name)}, {rstr(m.name)}, {rstr(m.desc)}, {rstr(self.ci.cf.source_file or '')});"]
        for n, k in sorted(self.used_locals.items()):
            body.append(f"    let mut {n}: {RT[k]} = {DEFAULT[k]};")
        for n, k in sorted(self.used_stack.items()):
            body.append(f"    let mut {n}: {RT[k]} = {DEFAULT[k]};")
        for ln in init:
            body.append("    " + ln)
        for pc_, (var, sn) in sorted(self.stack_slots.items()):
            body.append(f"    let mut {var}: std::mem::MaybeUninit<{sn}> = std::mem::MaybeUninit::uninit();")
        if self.has_arena:
            body.append("    let _arena_guard = rt::ArenaGuard::enter();")
        if has_sp:
            body.append("    let mut _sp_tick: u32 = 0;")
        fn_nm = fname(self.ci.name, m.name, m.desc)
        structured = None if handlers else _try_structure_cfg(blocks)
        if structured is not None:
            body.extend("    " + ln for ln in structured)
        else:
            body.append("    let mut pc: u32 = 0;")
            if handlers:
                body.append("    loop {")
                body.append(f"    let r = (|| -> rt::Res<{rret}> {{ loop {{ match pc {{")
            else:
                body.append("    loop { match pc {")
            for bpc in sorted(blocks):
                body.append(f"        {bpc} => {{")
                body.extend("            " + ln for ln in blocks[bpc])
                body.append("        }")
            body.append("        _ => unreachable!(),")
        if handlers:
            body.append("    } } })();")
            body.append("    let t = match r { Ok(v) => return Ok(v), Err(t) => t };")
            for h in handlers:
                cond = f"pc >= {h.start_pc} && pc < {h.end_pc}"
                if h.catch_type is not None:
                    try:
                        cr = self.cref(h.catch_type)
                        cond += f" && rt::instance_of(t.obj, {cr})"
                    except LinkError as le:
                        self.g.link_errors.append(f"{self.where}: catch {h.catch_type}: {le}")
                        cond += " && false"
                body.append(f"    if {cond} {{ {self.sname(0, 'A')} = t.obj; pc = {h.handler_pc}; continue; }}")
            body.append("    return Err(t);")
            body.append("    }")
        else:
            if structured is None:
                body.append("    } }")
        cret = cret_type(ret)
        cwrap = cret_wrap(ret)
        self.g.fn_sigs[fn_nm] = f"pub fn {fn_nm}({', '.join(params)}) -> {cret};"
        pe = self.g.patches.by_key.get((self.ci.name, m.name, m.desc))
        pmod = pmod_src = ""
        if pe is not None:
            self.g.patches.used.add(pe.key)
            pmod, pmod_src = self.g.patches.module_for(self.g, pe)
        if m.access & 0x0020:
            lock = "this" if not m.is_static else f"rt::class_mirror({self.cref(self.ci.name)})?"
            call_args = ", ".join((["this"] if not m.is_static else []) + [f"a{j}" for j in range(len(args))])
            wrapper = (f"#[no_mangle] pub extern \"C\" fn {fn_nm}({', '.join(params)}) -> {cret} {{\n"
                       f"    rt::{cwrap}((|| -> rt::Res<{rret}> {{\n"
                       f"        let lock = {lock};\n        rt::monitorenter(lock)?;\n"
                       f"        let r = {fn_nm}__body({call_args});\n        rt::monitorexit(lock)?;\n        r\n"
                       f"    }})())\n}}\n")
            if pe is not None:
                sig = f"fn {fn_nm}__body_orig({', '.join(params)}) -> rt::Res<{rret}> {{"
                pw = self.g.patches.wrapper(self.g, pe, f"{fn_nm}__body", f"{fn_nm}__body_orig", params,
                                            ([] if m.is_static else ["this"]) + [f"a{j}" for j in range(len(args))],
                                            ret, rret, pmod)
                return pmod_src + wrapper + sig + "\n" + "\n".join(body) + "\n}\n" + pw
            sig = f"fn {fn_nm}__body({', '.join(params)}) -> rt::Res<{rret}> {{"
            return wrapper + sig + "\n" + "\n".join(body) + "\n}\n"
        else:
            r_nm = "r_" + fn_nm[2:]
            inl = "#[inline] " if (not handlers and len(m.code.code) <= 120) else ""
            call_args = ", ".join((["this"] if not m.is_static else []) + [f"a{j}" for j in range(len(args))])
            nt = self.g.is_nothrow(self.ci, m)
            if nt:
                nt_body = []
                for ln in body:
                    if (ln.startswith("    rt::stack_check()?;") or ln.startswith("    let _frame = rt::enter_frame")
                            or ln.startswith("    static FI: rt::FrameInfo") or ln.lstrip().startswith("rt::set_line")):
                        continue  # a method that cannot throw makes no calls: no bound to check, no trace to record
                    s = ln.lstrip()
                    ind = ln[:len(ln) - len(s)]
                    if s == "return Ok(());":
                        nt_body.append(ind + "return;")
                    elif s.startswith("return Ok(") and s.endswith(");"):
                        nt_body.append(ind + "return " + s[10:-2] + ";")
                    else:
                        nt_body.append(ln)
                rsig = f"{inl}fn {r_nm}({', '.join(params)}) -> {rret} {{"
                wrapper = (f"#[no_mangle] pub extern \"C\" fn {fn_nm}({', '.join(params)}) -> {cret} {{\n"
                           f"    rt::{cwrap}(Ok({r_nm}({call_args})))\n}}\n")
                return rsig + "\n" + "\n".join(nt_body) + "\n}\n" + wrapper
            if pe is not None:
                rsig = f"fn {r_nm}_orig({', '.join(params)}) -> rt::Res<{rret}> {{"
                wrapper = (f"#[no_mangle] pub extern \"C\" fn {fn_nm}({', '.join(params)}) -> {cret} {{\n"
                           f"    rt::{cwrap}({r_nm}({call_args}))\n}}\n")
                pw = self.g.patches.wrapper(self.g, pe, r_nm, f"{r_nm}_orig", params,
                                            ([] if m.is_static else ["this"]) + [f"a{j}" for j in range(len(args))],
                                            ret, rret, pmod, inline="#[inline] ")
                return pmod_src + rsig + "\n" + "\n".join(body) + "\n}\n" + pw + wrapper
            rsig = f"{inl}fn {r_nm}({', '.join(params)}) -> rt::Res<{rret}> {{"
            wrapper = (f"#[no_mangle] pub extern \"C\" fn {fn_nm}({', '.join(params)}) -> {cret} {{\n"
                       f"    rt::{cwrap}({r_nm}({call_args}))\n}}\n")
            return rsig + "\n" + "\n".join(body) + "\n}\n" + wrapper


def _try_structure_cfg(blocks: dict[int, list[str]]) -> list[str] | None:
    if len(blocks) > 32:
        return None
    import re
    goto_re = re.compile(r"^pc = (\d+); continue;$")
    if_re = re.compile(r"^if (.+) \{ pc = (\d+); continue; \}$")
    parsed: dict[int, tuple] = {}
    succs: dict[int, list[int]] = {}
    preds: dict[int, list[int]] = {b: [] for b in blocks}
    for bpc, lines in blocks.items():
        if not lines:
            return None
        m_g = goto_re.match(lines[-1])
        if m_g:
            f_pc = int(m_g.group(1))
            m_if = if_re.match(lines[-2]) if len(lines) >= 2 else None
            if m_if:
                body = lines[:-2]
                cond, t_pc = m_if.group(1), int(m_if.group(2))
                if any("pc = " in ln or "continue;" in ln for ln in body):
                    return None
                parsed[bpc] = ("cond", body, cond, t_pc, f_pc)
                succs[bpc] = [t_pc, f_pc]
            else:
                body = lines[:-1]
                if any("pc = " in ln or "continue;" in ln for ln in body):
                    return None
                parsed[bpc] = ("goto", body, f_pc)
                succs[bpc] = [f_pc]
        else:
            if any("pc = " in ln or "continue;" in ln for ln in lines):
                return None
            parsed[bpc] = ("term", lines)
            succs[bpc] = []
        for s in succs[bpc]:
            if s not in preds:
                return None
            preds[s].append(bpc)

    if len(blocks) == 1 and parsed[0][0] == "term":
        return list(parsed[0][1])

    loop_headers: set[int] = set()
    for u, ss in succs.items():
        for v in ss:
            if v <= u:
                loop_headers.add(v)

    loop_exit: dict[int, int] = {}
    for h in loop_headers:
        can_reach = {h}
        stk = [u for u in preds[h] if u >= h]
        while stk:
            cur = stk.pop()
            if cur in can_reach or cur < h:
                continue
            can_reach.add(cur)
            stk.extend(preds[cur])
        fwd = {h}
        fstk = [h]
        while fstk:
            cur = fstk.pop()
            for s in succs[cur]:
                if s in can_reach and s not in fwd:
                    fwd.add(s)
                    fstk.append(s)
        # Düz döngü gövdesi iste: fwd içinde, 1'den fazla öncülü olan başlık dışı iç blok olmasın,
        # ve fwd içinde iç içe döngü başlığı olmasın.
        for u in fwd:
            if u != h and (u in loop_headers or len(preds[u]) > 1):
                return None
        exits = {s for u in fwd for s in succs[u] if s not in fwd}
        if len(exits) > 1:
            return None
        loop_exit[h] = next(iter(exits)) if exits else -1

    # Döngü başlıkları dışındaki döngüsüz düğümler için: bir koşul düğümünün iki kolu da sonlanmadıkça ya da
    # doğrudan tek bir anlık birleşimi paylaşmadıkça, keyfi DAG elmas paylaşımını yasakla.
    budget = [len(blocks) * 2 + 8]

    def emit(bpc: int, stop_pc: int, hdr: int, ex: int, out: list[str]) -> bool:
        budget[0] -= 1
        if budget[0] < 0:
            return False
        if bpc == hdr:
            out.append(f"continue 'L_{hdr};")
            return True
        if bpc == ex:
            out.append(f"break 'L_{hdr};")
            return True
        if bpc == stop_pc:
            return True
        if bpc in loop_headers and bpc != hdr:
            nex = loop_exit[bpc]
            out.append(f"'L_{bpc}: loop {{")
            sub: list[str] = []
            if not emit_node(bpc, nex, bpc, nex, sub):
                return False
            out.extend("    " + ln for ln in sub)
            out.append("}")
            if nex >= 0:
                return emit(nex, stop_pc, hdr, ex, out)
            return True
        if bpc not in loop_headers and len(preds[bpc]) > 1:
            return False
        return emit_node(bpc, stop_pc, hdr, ex, out)

    def emit_node(bpc: int, stop_pc: int, hdr: int, ex: int, out: list[str]) -> bool:
        node = parsed[bpc]
        kind = node[0]
        if kind == "term":
            out.extend(node[1])
            return True
        if kind == "goto":
            out.extend(node[1])
            return emit(node[2], stop_pc, hdr, ex, out)
        _, lines, cond, t_pc, f_pc = node
        out.extend(lines)
        if t_pc == ex:
            out.append(f"if {cond} {{ break 'L_{hdr}; }}")
            return emit(f_pc, stop_pc, hdr, ex, out)
        if f_pc == ex:
            out.append(f"if !({cond}) {{ break 'L_{hdr}; }}")
            return emit(t_pc, stop_pc, hdr, ex, out)
        if t_pc == hdr:
            out.append(f"if {cond} {{ continue 'L_{hdr}; }}")
            return emit(f_pc, stop_pc, hdr, ex, out)
        if f_pc == hdr:
            out.append(f"if !({cond}) {{ continue 'L_{hdr}; }}")
            return emit(t_pc, stop_pc, hdr, ex, out)
        # Koşul elmasına yalnızca iki dal da sonlanıyorsa ya da tam olarak aynı anlık hedefte buluşuyorsa izin ver
        merge = -1
        if parsed[t_pc][0] == "goto" and parsed[f_pc][0] == "goto" and parsed[t_pc][2] == parsed[f_pc][2]:
            merge = parsed[t_pc][2]
        elif t_pc in succs[f_pc]:
            merge = t_pc
        elif f_pc in succs[t_pc]:
            merge = f_pc
        t_out: list[str] = []
        f_out: list[str] = []
        sub_stop = merge if merge >= 0 else stop_pc
        if not emit(t_pc, sub_stop, hdr, ex, t_out) or not emit(f_pc, sub_stop, hdr, ex, f_out):
            return False
        out.append(f"if {cond} {{")
        out.extend("    " + ln for ln in t_out)
        if f_out:
            out.append("} else {")
            out.extend("    " + ln for ln in f_out)
        out.append("}")
        if merge >= 0:
            return emit(merge, stop_pc, hdr, ex, out)
        return True

    out: list[str] = []
    if not emit(0, -1, -1, -1, out):
        return None
    return out


I, J, F, D = "I", "J", "F", "D"
# MONO_LINE_LAZY için: hiçbir zaman hata fırlatmayan, çağrı yapmayan ve dallanmayan komutlar. Bunların önünde bekleyen
# satır numarası yazılmaz (bölme ve kalan ArithmeticException fırlatabildiği için listede yok; ldc sınıf sabiti
# NoClassDefFoundError fırlatabildiği için yok).
LINE_PURE_OPS = frozenset(
    ["nop", "aconst_null", "bipush", "sipush", "iinc", "pop", "pop2", "dup", "dup_x1", "dup_x2", "dup2", "dup2_x1",
     "dup2_x2", "swap", "iload", "lload", "fload", "dload", "aload", "istore", "lstore", "fstore", "dstore", "astore"]
    + [f"iconst_{x}" for x in ("m1", "0", "1", "2", "3", "4", "5")] + ["lconst_0", "lconst_1", "fconst_0", "fconst_1",
    "fconst_2", "dconst_0", "dconst_1"]
    + ["dadd", "dmul", "drem", "dsub", "ddiv", "fadd", "fdiv", "fmul", "frem", "fsub", "iadd", "iand", "imul", "ior",
       "ishl", "ishr", "isub", "iushr", "ixor", "ladd", "land", "lmul", "lor", "lshl", "lshr", "lsub", "lushr", "lxor",
       "dneg", "fneg", "ineg", "lneg", "lcmp", "fcmpl", "fcmpg", "dcmpl", "dcmpg",
       "i2l", "i2f", "i2d", "l2i", "l2f", "l2d", "f2i", "f2l", "f2d", "d2i", "d2l", "d2f", "i2b", "i2c", "i2s"])

BINOPS = {
    "iadd": (I, "{a}.wrapping_add({b})"), "isub": (I, "{a}.wrapping_sub({b})"), "imul": (I, "{a}.wrapping_mul({b})"),
    "idiv": (I, "rt::idiv({a}, {b})?"), "irem": (I, "rt::irem({a}, {b})?"),
    "ishl": (I, "rt::ishl({a}, {b})"), "ishr": (I, "rt::ishr({a}, {b})"), "iushr": (I, "rt::iushr({a}, {b})"),
    "iand": (I, "{a} & {b}"), "ior": (I, "{a} | {b}"), "ixor": (I, "{a} ^ {b}"),
    "ladd": (J, "{a}.wrapping_add({b})"), "lsub": (J, "{a}.wrapping_sub({b})"), "lmul": (J, "{a}.wrapping_mul({b})"),
    "ldiv": (J, "rt::ldiv({a}, {b})?"), "lrem": (J, "rt::lrem({a}, {b})?"),
    "lshl": (J, "rt::lshl({a}, {b})"), "lshr": (J, "rt::lshr({a}, {b})"), "lushr": (J, "rt::lushr({a}, {b})"),
    "land": (J, "{a} & {b}"), "lor": (J, "{a} | {b}"), "lxor": (J, "{a} ^ {b}"),
    "fadd": (F, "{a} + {b}"), "fsub": (F, "{a} - {b}"), "fmul": (F, "{a} * {b}"), "fdiv": (F, "{a} / {b}"),
    "frem": (F, "{a} % {b}"),
    "dadd": (D, "{a} + {b}"), "dsub": (D, "{a} - {b}"), "dmul": (D, "{a} * {b}"), "ddiv": (D, "{a} / {b}"),
    "drem": (D, "{a} % {b}"),
}
UNOPS = {"ineg": (I, "{a}.wrapping_neg()"), "lneg": (J, "{a}.wrapping_neg()"),
         "fneg": (F, "-{a}"), "dneg": (D, "-{a}")}
CONVS = {
    "i2l": (J, "{a} as i64"), "i2f": (F, "{a} as f32"), "i2d": (D, "{a} as f64"),
    "l2i": (I, "{a} as i32"), "l2f": (F, "{a} as f32"), "l2d": (D, "{a} as f64"),
    "f2i": (I, "rt::f2i({a})"), "f2l": (J, "rt::f2l({a})"), "f2d": (D, "{a} as f64"),
    "d2i": (I, "rt::d2i({a})"), "d2l": (J, "rt::d2l({a})"), "d2f": (F, "{a} as f32"),
    "i2b": (I, "rt::i2b({a})"), "i2c": (I, "rt::i2c({a})"), "i2s": (I, "rt::i2s({a})"),
}
CMPS = {"lcmp": "rt::lcmp({a}, {b})", "fcmpl": "rt::fcmpl({a}, {b})", "fcmpg": "rt::fcmpg({a}, {b})",
        "dcmpl": "rt::dcmpl({a}, {b})", "dcmpg": "rt::dcmpg({a}, {b})"}
IF1 = {"ifeq": "{a} == 0", "ifne": "{a} != 0", "iflt": "{a} < 0", "ifge": "{a} >= 0", "ifgt": "{a} > 0",
       "ifle": "{a} <= 0", "ifnull": "{a} == rt::Ref::NULL", "ifnonnull": "{a} != rt::Ref::NULL"}
IF2 = {"if_icmpeq": "{a} == {b}", "if_icmpne": "{a} != {b}", "if_icmplt": "{a} < {b}", "if_icmpge": "{a} >= {b}",
       "if_icmpgt": "{a} > {b}", "if_icmple": "{a} <= {b}", "if_acmpeq": "{a} == {b}", "if_acmpne": "{a} != {b}"}
ALOAD = {
    "iaload": ("I", "i32", ""), "laload": ("J", "i64", ""), "faload": ("F", "f32", ""), "daload": ("D", "f64", ""),
    "aaload": ("A", "rt::Ref", ""), "baload": ("I", "i8", " as i32"), "caload": ("I", "u16", " as i32"),
    "saload": ("I", "i16", " as i32"),
}
ASTORE = {
    "iastore": ("i32", ""), "lastore": ("i64", ""), "fastore": ("f32", ""), "dastore": ("f64", ""),
    "aastore": ("rt::Ref", ""), "bastore": ("i8", ""), "castore": ("u16", " as u16"), "sastore": ("i16", " as i16"),
}
PRINTLN = {"(I)V": "println_i", "(J)V": "println_j", "(Ljava/lang/String;)V": "println_s",
           "(C)V": "println_c", "(Z)V": "println_z"}


ARR_KINDS = "ZBCSIJFDA"
# çalışma zamanının kendisinin fırlattığı sınıflar (null erişimi, sınırlar, aritmetik, cast'ler, ...); her biri <init>(String) gerektirir
RUNTIME_THROWN = [
    "java/lang/ArithmeticException", "java/lang/NullPointerException", "java/lang/ArrayIndexOutOfBoundsException",
    "java/lang/NegativeArraySizeException", "java/lang/ClassCastException", "java/lang/ArrayStoreException",
    "java/lang/OutOfMemoryError", "java/lang/StackOverflowError", "java/lang/IllegalMonitorStateException",
    "java/lang/ExceptionInInitializerError",
    "java/lang/InternalError", "java/lang/UnsatisfiedLinkError", "java/lang/AbstractMethodError",
    "java/lang/NoClassDefFoundError", "java/lang/NoSuchFieldError", "java/lang/NoSuchMethodError",
    "java/lang/IncompatibleClassChangeError", "java/lang/InterruptedException",
    "java/lang/IllegalArgumentException", "java/lang/UnsupportedOperationException",
    "java/lang/IllegalThreadStateException", "java/lang/InstantiationError",
    "java/io/IOException", "java/io/FileNotFoundException",
]
OPTIONAL_THROWN = [
    "java/lang/IndexOutOfBoundsException", "java/lang/InstantiationException", "java/lang/ClassNotFoundException",
    "java/lang/SecurityException", "java/io/SyncFailedException", "java/util/zip/DataFormatException",
    "java/net/SocketException", "java/net/UnknownHostException", "java/net/ConnectException",
    "java/net/BindException", "java/net/SocketTimeoutException",
    "java/net/ProtocolException", "java/net/NoRouteToHostException", "java/net/PortUnreachableException",
    "java/nio/channels/ClosedChannelException",
    "com/mojang/text2speech/Narrator$InitializeException",   # patches/rust/narrator.rs
]


class Gen:
    """Whole-program state. Classes and methods are discovered from the entry point: a class is emitted once it is
    referenced, a method once it is called directly, reachable (Program's analysis) or needed by a vtable/itable."""

    def __init__(self, world, prog, natives_src: str, esc_res=None, patches=None):
        self.world, self.prog, self.esc = world, prog, esc_res
        # MONO_REGION_CLASSES=a/b/C,d/e/F: bu sınıfların örnekleri bölgeye tahsis edilir (docs/PIPELINE_VE_YOL_HARITASI_TR.md
        # bölüm 18). Boşsa üretilen kod değişmez. Sınıflar değişmez olmalı ve kurucuları `this`'i kaçırmamalı
        # (tools/region_eligible.py denetler).
        self.region_classes = frozenset(x.strip().replace(".", "/") for x in os.environ.get("MONO_REGION_CLASSES", "").split(",") if x.strip())
        # `*` içeren girdiler desendir (fnmatch); ör. `*$$Lambda$*` yakalama yapan tüm lambda sınıfları (alanları final,
        # kurucu yalnızca alanları doldurur: monolith/indy.py)
        self._region_exact = frozenset(x for x in self.region_classes if "*" not in x)
        self._region_globs = tuple(x for x in self.region_classes if "*" in x)
        from .patches import PatchSet
        self.patches = patches if patches is not None else PatchSet({}, [])
        import collections as _c
        self.use_mir = bool(os.environ.get("MONO_MIR"))
        # MONO_FRAMES=fast: hızlı gölge yığın (rt.rs `enter_frame_s`/`set_line_s`). Metot başına bir `static FI: FrameInfo`
        # üretilir, çağrıda yalnızca ona işaretçi itilir; yığın taşması denetimi de aynı adımda yapılır. Varsayılan: eski yol
        # (`stack_check` + `enter_frame` + `set_line`), değişmedi.
        # MONO_FRAMES=dwarf (C9): çerçeve bilgisi hızlı yoldaki gibi üretilir, sonra linetab.build ile çalışma anı kaydı
        # koddan silinip yan tabloya taşınır (_apply_line_tables).
        self.fast_frames = os.environ.get("MONO_FRAMES", "") in ("fast", "dwarf")
        self.set_line_fn = "set_line_s" if self.fast_frames else "set_line"
        # MONO_ICACHE=1: arayüz çağrısı, checkcast ve instanceof için çağrı yeri önbelleği (rt.rs `ICache`). Her çağrı yerine
        # kendi `static IC` önbelleği üretilir. Varsayılan: eski yol (`itab_find`/`checkcast`/`instance_of`), değişmedi.
        self.icache = os.environ.get("MONO_ICACHE", "") == "1"
        # MONO_INTRINSICS=1: java.lang.Math'in bazı metotları çağrı yerine doğrudan Rust işlemi olarak üretilir (MATH_INTRINSICS).
        # Varsayılan: eski yol (gerçek çağrı), değişmedi.
        self.intrinsics = os.environ.get("MONO_INTRINSICS", "") == "1"
        # MONO_INIT_ELIDE=1: bir metodun içinde kendi sınıfının ve üst sınıflarının başlatma denetimi üretilmez (gerekçe:
        # MethodGen.init_class). Varsayılan: eski yol (her statik erişimde denetim), değişmedi.
        self.init_elide = os.environ.get("MONO_INIT_ELIDE", "") == "1"
        # MONO_NULL_ELIDE=1: kesin null olmayan alıcılarda (hiç yeniden yazılmamış `this`, aynı blokta `new` ile ayrılmış
        # nesne) null denetimi üretilmez (MethodGen.known_nonnull). Varsayılan: eski yol (her erişimde rt::nn), değişmedi.
        self.null_elide = os.environ.get("MONO_NULL_ELIDE", "") == "1"
        # MONO_LINE_LAZY=1: satır numarası yazımı blok içinde hata fırlatabilen ilk komuta ertelenir (MethodGen blok
        # üretimi, LINE_PURE_OPS). Varsayılan: eski yol (her satır başında yazım), değişmedi.
        self.line_lazy = os.environ.get("MONO_LINE_LAZY", "") == "1"
        # MONO_SHARD=pkg: sınıflar parçalara SHA1 ile rastgele değil, paketlerine göre dağıtılır ve AYNI PARÇADAKİ sınıflar
        # birbirini doğrudan (satır içine alınabilir `r_` fonksiyonuyla) çağırır (shard_of, rust_callable). Varsayılan: eski
        # yol (rastgele dağıtım, yalnızca aynı sınıfın metotları doğrudan), değişmedi.
        self.shard_pkg = os.environ.get("MONO_SHARD", "") == "pkg"
        self._shard_map: dict | None = None
        self.mir_fallback = _c.Counter()
        self.mir_methods = 0
        self.mir_inlined = 0
        self.lits: dict[str, tuple] = {}
        self.sels: dict[str, int] = {}
        self.class_q: list = []
        self.seen_classes: set = set()
        self.method_q: list = []
        self.seen_methods: set = set()
        self.object_vtable = "&[]"
        self.vh_used = False
        self.cs_cache: dict = {}
        self.errors: list[str] = []
        self.link_errors: list[str] = []
        self.missing_natives: list[str] = []
        self.cls_syms: set[str] = {f"CLS_ARR_{k}" for k in ARR_KINDS}
        self.fn_sigs: dict[str, str] = {}
        self.static_sigs: dict[str, str] = {}
        self.struct_defs: dict[str, str] = {}
        self.shapes: dict[str, int] = {}
        self.shards: int = 1
        self._devirt_cache: dict = {}
        self._iface_impls: dict | None = None
        import re
        rt_dir = os.path.dirname(natives_src)
        self.natives = set()
        self.jni_natives = set()   # the JNI/JNA emulation (natives_jni.rs): their stubs count calls for MONO_COUNT_NATIVES
        for fn in os.listdir(rt_dir):
            if fn.endswith(".rs"):
                txt = open(os.path.join(rt_dir, fn), encoding="utf-8").read()
                found = set(re.findall(r"pub fn (n_\w+)", txt)) | set(re.findall(r"\w+!\(\s*(n_\w+)", txt))
                self.natives |= found
                if fn == "natives_jni.rs":
                    self.jni_natives |= found

    _NO_THROW_OPS = frozenset({
        "nop", "aconst_null", "iconst_m1", "iconst_0", "iconst_1", "iconst_2", "iconst_3", "iconst_4", "iconst_5",
        "lconst_0", "lconst_1", "fconst_0", "fconst_1", "fconst_2", "dconst_0", "dconst_1",
        "bipush", "sipush",
        "iload", "lload", "fload", "dload", "aload", "iload_0", "iload_1", "iload_2", "iload_3",
        "lload_0", "lload_1", "lload_2", "lload_3", "fload_0", "fload_1", "fload_2", "fload_3",
        "dload_0", "dload_1", "dload_2", "dload_3", "aload_0", "aload_1", "aload_2", "aload_3",
        "istore", "lstore", "fstore", "dstore", "astore", "istore_0", "istore_1", "istore_2", "istore_3",
        "lstore_0", "lstore_1", "lstore_2", "lstore_3", "fstore_0", "fstore_1", "fstore_2", "fstore_3",
        "dstore_0", "dstore_1", "dstore_2", "dstore_3", "astore_0", "astore_1", "astore_2", "astore_3",
        "pop", "pop2", "dup", "dup_x1", "dup_x2", "dup2", "dup2_x1", "dup2_x2", "swap",
        "iadd", "ladd", "fadd", "dadd", "isub", "lsub", "fsub", "dsub",
        "imul", "lmul", "fmul", "dmul", "fdiv", "ddiv", "frem", "drem",
        "ineg", "lneg", "fneg", "dneg",
        "ishl", "lshl", "ishr", "lshr", "iushr", "lushr",
        "iand", "land", "ior", "lor", "ixor", "lxor", "iinc",
        "i2l", "i2f", "i2d", "l2i", "l2f", "l2d", "f2i", "f2l", "f2d", "d2i", "d2l", "d2f",
        "i2b", "i2c", "i2s", "lcmp", "fcmpl", "fcmpg", "dcmpl", "dcmpg",
        "ifeq", "ifne", "iflt", "ifge", "ifgt", "ifle",
        "if_icmpeq", "if_icmpne", "if_icmplt", "if_icmpge", "if_icmpgt", "if_icmple",
        "if_acmpeq", "if_acmpne", "ifnull", "ifnonnull",
        "goto", "goto_w", "tableswitch", "lookupswitch",
        "ireturn", "lreturn", "freturn", "dreturn", "areturn", "return",
    })

    _PUSH1_OPS = frozenset({"iload", "fload", "aload", "aconst_null", "bipush", "sipush", "iconst_m1", "iconst_0", "iconst_1",
                            "iconst_2", "iconst_3", "iconst_4", "iconst_5", "fconst_0", "fconst_1", "fconst_2",
                            "lload", "dload", "lconst_0", "lconst_1", "dconst_0", "dconst_1"})

    def _this_field_op(self, dc, insns, k: int, targets: set) -> bool:
        """MONO_NULL_ELIDE: insns[k] `this` üzerindeki bir getfield/putfield mi (alıcı kesinlikle yeniden yazılmamış `this`)?
        Kalıplar: `aload 0; getfield` ve `aload 0; <tek değer yükle>; putfield`. Arada dallanma hedefi olmamalı (başka
        yoldan gelen bir yığın değeri alıcı olmasın) ve alan çözülebilmeli (yoksa NoSuchFieldError fırlatılır).
        Böyle bir alan erişimi hata fırlatamaz; getter/setter gibi metotlar böylece is_nothrow olur (çerçevesiz)."""
        ins = insns[k]
        if ins.op == "getfield":
            pre = [k - 1]
        elif ins.op == "putfield":
            pre = [k - 2, k - 1]
            if k - 1 < 0 or insns[k - 1].op not in self._PUSH1_OPS:
                return False
        else:
            return False
        if pre[0] < 0 or insns[pre[0]].op != "aload" or insns[pre[0]].arg != 0:
            return False
        if any(insns[j].pc in targets for j in pre[1:] + [k]):
            return False
        try:
            ci = self.world.get(ins.arg.owner)
            if ci is None or self.world.instance_fields(ci) is None:
                return False
            if not any(f.name == ins.arg.name and f.desc == ins.arg.desc for _, f in self.world.instance_fields(ci)):
                return False
        except Exception:
            return False
        return True

    def is_nothrow(self, dc, m) -> bool:
        k = (dc.name, m.key)
        if hasattr(self, "_nothrow_cache") and k in self._nothrow_cache:
            return self._nothrow_cache[k]
        if not hasattr(self, "_nothrow_cache"):
            self._nothrow_cache = {}
        ok = False
        if (dc.name, m.name, m.desc) in self.patches:
            self._nothrow_cache[k] = False
            return False
        if not m.is_native and m.code is not None and not m.code.handlers and not (m.access & 0x0020):
            insns = self.prog.insns(dc, m)
            ok = True
            this_ok = self.null_elide and not m.is_static and not any(x.op == "astore" and x.arg == 0 for x in insns)
            targets = {t for x in insns for t in (x.targets or ())} if this_ok else set()
            for k, ins in enumerate(insns):
                if ins.op in self._NO_THROW_OPS:
                    continue
                if this_ok and self._this_field_op(dc, insns, k, targets):
                    continue
                # kendine çağrı muaf değildir: sınırsız özyineleme StackOverflowError ile biter, bu da bir fırlatmadır
                ok = False
                break
        self._nothrow_cache[k] = ok
        return ok

    def rust_callable(self, cur_cn: str, dc, m) -> bool:
        return (not m.is_native and m.code is not None and not (m.access & 0x0020)
                and (self.shards <= 1 or cur_cn == dc.name
                     or (self.shard_pkg and self.shard_of(cur_cn) == self.shard_of(dc.name))))

    def shard_of(self, cn: str) -> int:
        """Sınıfın parça numarası. Eski yol: sınıf adının SHA1'i. MONO_SHARD=pkg: paket bazlı dengeli dağıtım.

        Acemi notu: 16 parça ayrı ayrı derlenir; bir parçadaki kod başka parçadaki fonksiyonu yalnızca `extern "C"` üzerinden,
        satır içine almadan çağırabilir. Sınıfları rastgele dağıtınca birbirini en çok çağıran sınıflar (aynı paketin
        sınıfları) çoğunlukla farklı parçalara düşer. Paket bazlı dağıtım onları aynı parçada toplar.
        Dağıtım çeviriden ÖNCE, erişilebilir metotların bytecode boyutlarıyla bir kez hesaplanır (aynı girdi -> aynı
        dağıtım). Tabloda olmayan sınıf (çeviri sırasında üretilen) önce paketinin parçasına, paket de yoksa SHA1'e düşer.
        """
        if not self.shard_pkg or self.shards <= 1:
            return int(hashlib.sha1(cn.encode()).hexdigest()[:8], 16) % self.shards
        if self._shard_map is None:
            self._shard_map = self._build_shard_map()
        cmap, pmap = self._shard_map
        if cn in cmap:
            return cmap[cn]
        pkg = cn.rsplit("/", 1)[0] if "/" in cn else ""
        if pkg in pmap:
            return pmap[pkg]
        return int(hashlib.sha1(cn.encode()).hexdigest()[:8], 16) % self.shards

    def _build_shard_map(self):
        """(sınıf -> parça, paket -> parça). Paketler ağırlığa göre büyükten küçüğe en hafif parçaya konur (LPT). Bir parçaya
        sığmayacak kadar büyük paket, üst düzey sınıflarına göre (iç sınıflar dış sınıfıyla birlikte kalır) bölünür."""
        n = self.shards
        weight: dict[str, int] = {}
        for cn, key in self.prog.reachable:
            ci = self.world.get(cn)
            if ci is None:
                continue
            w = 0
            for mm in ci.cf.methods:
                if mm.name + mm.desc == key and mm.code is not None:
                    w = len(mm.code.code)
                    break
            weight[cn] = weight.get(cn, 200) + w  # 200: sınıf başına sabit pay (sınıf tablosu, vtable, alanlar)
        by_pkg: dict[str, list[str]] = {}
        for cn in weight:
            by_pkg.setdefault(cn.rsplit("/", 1)[0] if "/" in cn else "", []).append(cn)
        total = sum(weight.values()) or 1
        cap = total / n * 0.5  # bundan büyük paket bölünür, böylece dengeleme yapılabilir
        groups: list[tuple[int, str, list[str]]] = []
        for pkg, cls in by_pkg.items():
            pw = sum(weight[c] for c in cls)
            if pw <= cap:
                groups.append((pw, pkg, sorted(cls)))
                continue
            tops: dict[str, list[str]] = {}
            for c in sorted(cls):
                tops.setdefault(c.split("$", 1)[0], []).append(c)
            cur: list[str] = []
            cw = 0
            for t in sorted(tops):
                cur.extend(tops[t])
                cw += sum(weight[c] for c in tops[t])
                if cw >= cap:
                    groups.append((cw, pkg + "#" + t, cur))
                    cur, cw = [], 0
            if cur:
                groups.append((cw, pkg + "#~", cur))
        groups.sort(key=lambda g: (-g[0], g[1]))
        load = [0] * n
        cmap: dict[str, int] = {}
        pmap: dict[str, int] = {}
        for w, key, cls in groups:
            i = min(range(n), key=lambda k: (load[k], k))
            load[i] += w
            for c in cls:
                cmap[c] = i
            pkg = key.split("#", 1)[0]
            pmap.setdefault(pkg, i)
        return cmap, pmap

    def call_direct(self, cur_cn: str, dc, m, arg_exprs: list[str]) -> str:
        fn_nm = fname(dc.name, m.name, m.desc)
        if self.rust_callable(cur_cn, dc, m):
            q = "" if self.is_nothrow(dc, m) else "?"
            return f"r_{fn_nm[2:]}({', '.join(arg_exprs)}){q}"
        return f"unsafe {{ {fn_nm}({', '.join(arg_exprs)}) }}.res()?"

    def devirt_targets(self, owner_ci, name: str, desc: str):
        key = (owner_ci.name, name + desc)
        if key in self._devirt_cache:
            return self._devirt_cache[key]
        res = None
        if owner_ci.is_interface:
            if self._iface_impls is None:
                m_map: dict[str, list] = {}
                for c in self.world.classes.values():
                    if not c.is_interface and not (c.cf.access & 0x0400):
                        for iface in self.world.all_interfaces(c):
                            m_map.setdefault(iface.name, []).append(c)
                self._iface_impls = m_map
            # yalnızca RTA'nın örneklenmiş gördüğü sınıflar alıcı olabilir (kod üretiminden önce kapanış tamamdır;
            # yüklenmiş her sınıfı saymak, cevabı sınıfların yüklenme sırasına bağımlı kılıyordu)
            concs = [c for c in self._iface_impls.get(owner_ci.name, ()) if c.name in self.prog.instantiated]
            if 1 <= len(concs) <= 8:
                by_cls = []
                distinct = {}
                ok = True
                for c in concs:
                    impl = self.world.select(c, name, desc)
                    if impl is None or impl[1].is_abstract or impl[1].is_native:
                        ok = False
                        break
                    dc, m2 = impl
                    distinct[(dc.name, m2.key)] = (dc, m2)
                    by_cls.append((c, dc, m2))
                if ok and len(distinct) == 1:
                    res = ("single", next(iter(distinct.values())))
                elif ok and 2 <= len(by_cls) <= 4:
                    res = ("poly", by_cls)
        else:
            stk = [owner_ci]
            seen = set()
            distinct = {}
            ok = True
            while stk:
                c = stk.pop()
                if c.name in seen:
                    continue
                seen.add(c.name)
                if not c.is_interface and not (c.cf.access & 0x0400) and c.name in self.prog.instantiated:
                    impl = self.world.select(c, name, desc)
                    if impl is None or impl[1].is_abstract or impl[1].is_native:
                        ok = False
                        break
                    dc, m2 = impl
                    distinct[(dc.name, m2.key)] = (dc, m2)
                    if len(distinct) > 1:
                        ok = False
                        break
                stk.extend(c.subclasses)
            if ok and len(distinct) == 1:
                res = ("single", next(iter(distinct.values())))
        self._devirt_cache[key] = res
        return res

    def is_region_class(self, cn: str) -> bool:
        if cn in self._region_exact:
            return True
        return any(fnmatch.fnmatchcase(cn, g) for g in self._region_globs)

    def noescape_sites(self, cn: str, key: str):
        return self.esc.noescape_sites(cn, key) if self.esc is not None else frozenset()

    def shape_id(self, is_static: bool, desc: str) -> int:
        args, ret = parse_method_desc(desc)
        rk = lambda d: d[0] if d[0] in "ZBCSIJFDV" else "A"
        key = ("S" if is_static else "I") + "".join(rk(d) for d in args) + ":" + rk(ret)
        return self.shapes.setdefault(key, len(self.shapes) + 1)

    def caller_sensitive(self, dc, m) -> bool:
        """True if the method calls Reflection.getCallerClass(): its callers publish their class first."""
        k = (dc.name, m.name + m.desc)
        if k not in self.cs_cache:
            r = False
            if m.code is not None:
                r = any(i.op == "invokestatic" and i.arg.owner == "jdk/internal/reflect/Reflection" and i.arg.name == "getCallerClass"
                        for i in self.prog.insns(dc, m))
            self.cs_cache[k] = r
        return self.cs_cache[k]

    def sel(self, name: str, desc: str) -> int:
        """Global selector id of an interface method (name + descriptor)."""
        return self.sels.setdefault(name + desc, len(self.sels))

    def recycle_slot(self, cn: str):
        """Slot of a recyclable iterator class, or None. A class qualifies when it is on the list and every
        `new` of it in its own top-level class family is `new C; dup; ..; invokespecial C.<init>; areturn` (the object
        is created for the caller and handed to nobody else)."""
        if cn not in RECYCLABLE_ITERATORS:
            return None
        if not hasattr(self, "_recycle"):
            self._recycle, self._recycle_ok = {}, {}
        if cn in self._recycle_ok:
            return self._recycle.get(cn) if self._recycle_ok[cn] else None
        ok = self.world.get(cn) is not None
        if ok:
            top = cn.split("$")[0]
            for name in list(self.world.classes):
                if name != top and not name.startswith(top + "$"):
                    continue
                ci = self.world.classes[name]
                for m in ci.cf.methods:
                    if m.code is None:
                        continue
                    ins = self.prog.insns(ci, m)
                    for k, x in enumerate(ins):
                        if x.op == "new" and x.arg.name == cn:
                            j = k + 1
                            while j < len(ins) and not (ins[j].op == "invokespecial" and ins[j].arg.owner == cn and ins[j].arg.name == "<init>"):
                                j += 1
                            if j + 1 >= len(ins) or ins[j + 1].op != "areturn":
                                ok = False
        self._recycle_ok[cn] = ok and len(self._recycle) < 32
        if self._recycle_ok[cn]:
            self._recycle[cn] = len(self._recycle)
        return self._recycle.get(cn) if self._recycle_ok[cn] else None

    def need_class(self, name: str):
        ci = self.world.get(name)
        if ci is None:
            raise LinkError(f"class {name} not found")
        if name not in self.seen_classes:
            self.seen_classes.add(name)
            self.class_q.append(ci)
        return ci

    def need_method(self, ci, m):
        k = (ci.name, m.name + m.desc)
        if k not in self.seen_methods:
            self.seen_methods.add(k)
            self.method_q.append((ci, m))

    def lit(self, text: str):
        if text not in self.lits:
            n = len(self.lits)
            u = []
            for ch in text:
                c = ord(ch)
                if c >= 0x10000:
                    c -= 0x10000
                    u += [0xD800 + (c >> 10), 0xDC00 + (c & 0x3FF)]
                else:
                    u.append(c)
            self.lits[text] = (f"LIT_{n}", f"LD_{n}", u)
        r = self.lits[text]
        return r[0], r[1]

    @staticmethod
    def struct_name(ci) -> str:
        return "O_" + mang(ci.name)

    @staticmethod
    def class_name(ci) -> str:
        return "CLS_" + mang(ci.name)

    @staticmethod
    def array_class_name(desc: str) -> str:
        e = desc[1]
        return "CLS_ARR_" + (e if e in ARR_KINDS else "A")

    @staticmethod
    def init_name(ci) -> str:
        return "init_" + mang(ci.name)

    @staticmethod
    def field_name(dc, f) -> str:
        return f"f_{mang(dc.name)}_{mang(f.name)}"

    @staticmethod
    def static_name(dc, f) -> str:
        return f"S_{mang(dc.name)}__{mang(f.name)}"

    @staticmethod
    def cv(f):
        """(kind, value) of a ConstantValue attribute, or None. Kinds: int long float double string."""
        c = f.constant_value
        if c is None:
            return None
        if isinstance(c, tuple):
            return c
        if isinstance(c, str):
            return ("string", c)
        if isinstance(c, bool) or isinstance(c, int):
            return ("int", int(c))
        raise RustGenError(f"{f.name}: ConstantValue {c!r}")

    def string_consts(self, ci):
        return [f for f in ci.cf.fields if f.is_static and (self.cv(f) or ("",))[0] == "string"]

    def init_iface_supers(self, ci) -> list:
        """Superinterfaces that declare default methods. JLS 12.4.2: initializing a class initializes them too."""
        seen: set[str] = set()
        out: list = []

        def walk(x):
            for i in x.interfaces:
                if i.name in seen:
                    continue
                seen.add(i.name)
                if any(not m.is_static and not m.is_abstract and m.name not in ("<clinit>", "<init>") for m in i.cf.methods):
                    out.append(i)
                walk(i)
            if x.super is not None:
                walk(x.super)

        walk(ci)
        return out

    def needs_init(self, ci) -> bool:
        if ci.cf.method("<clinit>", "()V") is not None or self.string_consts(ci):
            return True
        if ci.super is not None and self.needs_init(ci.super):
            return True
        return not ci.is_interface and any(self.needs_init(i) for i in self.init_iface_supers(ci))

    def is_reachable(self, cname: str, m) -> bool:
        return (cname, m.name + m.desc) in self.prog.reachable

    def slot_fn(self, impl) -> str:
        if impl is not None:
            dcn, mm = impl
            if (mm.code is not None or mm.is_native) and self.is_reachable(dcn, mm):
                self.need_method(self.need_class(dcn), mm)
                return fname(dcn, mm.name, mm.desc)
        return "rt::unreachable_slot"

    def native_name(self, ci, m) -> str:
        base = f"n_{mang(ci.name)}__{mang(m.name)}"
        same = [x for x in ci.cf.methods if x.is_native and x.name == m.name]
        if len(same) > 1:
            base += "__" + hashlib.sha1(m.desc.encode()).hexdigest()[:6]
        return base

    def emit_native(self, ci, m) -> str:
        args, ret = parse_method_desc(m.desc)
        params, call = [], []
        if not m.is_static:
            params.append("this: rt::Ref")
            call.append("this")
        for j, d in enumerate(args):
            params.append(f"a{j}: {RT[kind_of(d)]}")
            call.append(f"a{j}")
        fn_nm = fname(ci.name, m.name, m.desc)
        cret = cret_type(ret)
        cwrap = cret_wrap(ret)
        self.fn_sigs[fn_nm] = f"pub fn {fn_nm}({', '.join(params)}) -> {cret};"
        nn_ = self.native_name(ci, m)
        sig = f"#[no_mangle] pub extern \"C\" fn {fn_nm}({', '.join(params)}) -> {cret} {{"
        if nn_ in self.natives:
            count = f" mono_rt::ncount::count({rstr(nn_)});" if nn_ in self.jni_natives else ""
            return sig + f"{count} rt::{cwrap}(natives::{nn_}({', '.join(call)})) }}\n"
        self.missing_natives.append(f"{nn_}  ({ci.name}.{m.name}{m.desc})")
        return sig + f" rt::{cwrap}(rt::missing_native({rstr(f'{ci.name}.{m.name}{m.desc}')})) }}\n"

    def emit_class(self, ci) -> str:
        w = self.world
        self.cls_syms.add(self.class_name(ci))
        sup = ci.super
        if sup is not None:
            self.need_class(sup.name)
            sup_ref = "Some(unsafe { &" + self.class_name(sup) + " })"
        else:
            sup_ref = "None"
        all_if = w.all_interfaces(ci)
        for i in all_if:
            self.need_class(i.name)
        ifaces_ref = ", ".join("unsafe { &" + self.class_name(i) + " }" for i in all_if)
        itab = []
        if not ci.is_interface:
            seen = set()
            for i in all_if:
                for im in w.iface_methods(i):
                    key = im.name + im.desc
                    if key in seen:
                        continue
                    seen.add(key)
                    impl = w.select(ci, im.name, im.desc)
                    itab.append((self.sel(im.name, im.desc), self.slot_fn((impl[0].name, impl[1]) if impl else None)))
            itab.sort()
        itab_text = ", ".join(f"rt::IEnt({n}, rt::Fp({f} as *const ()))" for n, f in itab)
        out = []
        fields = w.instance_fields(ci)
        sn = self.struct_name(ci)
        st_lines = ["#[repr(C)]", f"pub struct {sn} {{", "    pub hdr: rt::Obj,"]
        for owner, f in fields:
            st_lines.append(f"    pub f_{mang(owner)}_{mang(f.name)}: {ftype(f.desc)},")
        st_lines.append("}")
        self.struct_defs[sn] = "\n".join(st_lines) + "\n"
        out.extend(st_lines)
        slots = []
        for sl in ([] if ci.is_interface else w.vtable(ci)):
            slots.append(f"rt::Fp({self.slot_fn(sl.impl)} as *const ())")
        vt = f"&[{', '.join(slots)}]"
        if ci.name == "java/lang/Object":
            self.object_vtable = vt
        cf = ci.cf
        pool = CompactPool(cf.cp)
        keep_ann = lambda desc: isinstance(desc, str) and desc.startswith("L") and desc[1:-1] in self.prog.loaded
        ents = []
        for f in cf.fields:
            if f.is_static:
                loc = f"rt::FLoc::Static(rt::SPtr(unsafe {{ &raw mut {self.static_name(ci, f)} as *mut u8 }}))"
            else:
                loc = f"rt::FLoc::Off(std::mem::offset_of!({self.struct_name(ci)}, f_{mang(ci.name)}_{mang(f.name)}) as u32)"
            fsig = rstr(f.signature or "")
            fann = rbytes(rewrite_annotations(f.attributes.get("RuntimeVisibleAnnotations"), pool, keep_ann))
            ents.append(f"rt::FInfo {{ name: {rstr(f.name)}, desc: {rstr(f.desc)}, access: {f.access}, loc: {loc}, sig: {fsig}, ann: {fann} }}")
        foffs = ", ".join(ents)
        ments = []
        for m in cf.methods:
            if m.name == "<clinit>":
                continue
            fp_expr = "rt::Fp(std::ptr::null())"
            shape = 0
            if self.is_reachable(ci.name, m) and (m.code is not None or m.is_native) and not m.is_abstract:
                self.need_method(ci, m)
                fp_expr = f"rt::Fp({fname(ci.name, m.name, m.desc)} as *const ())"
                shape = self.shape_id(m.is_static, m.desc)
            ma = m.attributes
            msig = rstr(m.signature or "")
            mann = rbytes(rewrite_annotations(ma.get("RuntimeVisibleAnnotations"), pool, keep_ann))
            mpann = rbytes(rewrite_param_annotations(ma.get("RuntimeVisibleParameterAnnotations"), pool, keep_ann))
            mdflt = rbytes(rewrite_default(ma.get("AnnotationDefault"), pool))
            mex = "&[" + ", ".join(rstr(x) for x in m.exceptions) + "]" if m.exceptions else "&[]"
            ments.append(f"rt::MInfo {{ name: {rstr(m.name)}, desc: {rstr(m.desc)}, access: {m.access}, shape: {shape}, fp: {fp_expr}, "
                         f"sig: {msig}, ann: {mann}, pann: {mpann}, dflt: {mdflt}, ex: {mex} }}")
        moffs = ", ".join(ments)
        cann = rewrite_annotations(cf.attributes.get("RuntimeVisibleAnnotations"), pool, keep_ann)
        ie = inner_info(cf)
        declared = [e.inner for e in cf.inner_classes if e.outer == cf.name and e.inner and e.inner in self.prog.loaded]
        em = cf.enclosing_method
        has_refl = bool(cf.signature or cann or pool.entries or ie is not None or em is not None or declared or getattr(cf, "nest_host", None))
        refl_expr = "None"
        if has_refl:
            cp_items = ["rt::CpEnt::None"]
            for kind, val in pool.entries:
                if kind == "utf8":
                    cp_items.append(f"rt::CpEnt::Str({rstr(val)})")
                elif kind == "int":
                    cp_items.append(f"rt::CpEnt::Int({val})")
                elif kind == "long":
                    cp_items.append(f"rt::CpEnt::Long({val})")
                elif kind == "float":
                    cp_items.append(f"rt::CpEnt::Float(f32::from_bits(0x{struct.unpack('>I', val)[0]:08x}))")
                elif kind == "double":
                    cp_items.append(f"rt::CpEnt::Double(f64::from_bits(0x{struct.unpack('>Q', val)[0]:016x}))")
            cr_nm = "CR_" + mang(ci.name)
            refl_expr = f"Some(&{cr_nm})"
            out.append(
                f"static {cr_nm}: rt::ClassRefl = rt::ClassRefl {{ "
                f"sig: {rstr(cf.signature or '')}, ann: {rbytes(cann)}, "
                f"cpool: &[{', '.join(cp_items)}], "
                f"is_inner: {'true' if ie is not None else 'false'}, "
                f"outer: {rstr((ie.outer or '') if ie else '')}, "
                f"inner_name: {rstr((ie.name or '') if ie else '')}, "
                f"inner_flags: {ie.access if ie else 0}, "
                f"enc_class: {rstr(em[0] if em else '')}, "
                f"enc_name: {rstr((em[1] or '') if em else '')}, "
                f"enc_desc: {rstr((em[2] or '') if em else '')}, "
                f"declared: &[{', '.join(rstr(x) for x in declared)}], "
                f"nest_host: {rstr(getattr(cf, 'nest_host', None) or '')} }};"
            )
        has_init = self.needs_init(ci)
        init_fn = f"Some({self.init_name(ci)})" if has_init else "None"
        enum_fn = "None"
        if ci.cf.access & 0x4000 and ci.super is not None and ci.super.name == "java/lang/Enum":
            vf = next((f for f in ci.cf.fields if f.name == "$VALUES" and f.is_static), None)
            if vf is not None:
                enum_fn = f"Some(ev_{mang(ci.name)})"
                extra_enum = (f"extern \"C\" fn ev_{mang(ci.name)}() -> rt::CRes<rt::Ref> {{ "
                              f"rt::cres((|| -> rt::Res<rt::Ref> {{ {self.init_name(ci)}().res()?; "
                              f"Ok(unsafe {{ {self.static_name(ci, vf)} }}) }})()) }}")
        ref_offs = ", ".join(
            f"std::mem::offset_of!({sn}, f_{mang(owner)}_{mang(f.name)}) as u32"
            for owner, f in fields if ftype(f.desc) == "rt::Ref"
        )
        srefs = ", ".join(
            f"rt::SPtr(unsafe {{ &raw mut {self.static_name(ci, f)} as *mut u8 }})"
            for f in cf.fields if f.is_static and ftype(f.desc) == "rt::Ref"
        )
        out.append(f"#[no_mangle] pub static {self.class_name(ci)}: rt::Class = rt::Class {{ name: {rstr(ci.name)}, super_: {sup_ref}, "
                   f"size: std::mem::size_of::<{self.struct_name(ci)}>(), vtable: {vt}, elem: 0, elem_class: None, array_cache: rt::ArrCache::NEW, "
                   f"itab: &[{itab_text}], ifaces: &[{ifaces_ref}], fields: &[{foffs}], ref_offsets: &[{ref_offs}], static_refs: &[{srefs}], methods: &[{moffs}], refl: {refl_expr}, mirror: rt::Mirror::NEW, kind: {1 if ci.is_interface else 0}, access: {ci.cf.access}, "
                   f"init: {init_fn}, enum_values: {enum_fn} }};")
        if enum_fn != "None":
            out.append(extra_enum)
        for f in ci.cf.fields:
            if f.is_static:
                k = kind_of(f.desc)
                init = DEFAULT[k]
                if k == "I" and not isinstance(DEFAULT[k], int):
                    init = "0"
                cv = self.cv(f)
                if cv is not None and cv[0] != "string":
                    t, v = cv
                    if t in ("int", "long"):
                        init = str(v)
                    elif t == "float":
                        init = f"f32::from_bits(0x{struct.unpack('>I', v)[0]:08x})"
                    elif t == "double":
                        init = f"f64::from_bits(0x{struct.unpack('>Q', v)[0]:016x})"
                    else:
                        raise RustGenError(f"{ci.name}.{f.name}: ConstantValue {cv!r}")
                sn = self.static_name(ci, f)
                ft = ftype(f.desc)
                self.static_sigs[sn] = ft
                out.append(f"#[no_mangle] pub static mut {sn}: {ft} = {init};")
        if self.needs_init(ci):
            st = "ST_" + mang(ci.name)
            inm = self.init_name(ci)
            self.fn_sigs[inm] = f"pub fn {inm}() -> rt::CVoid;"
            out.append(f"static {st}: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);")
            body = [f"#[no_mangle] pub extern \"C\" fn {inm}() -> rt::CVoid {{",
                    "    rt::cvoid((|| -> rt::Res<()> {",
                    f"        if !rt::init_enter(&{st}, {rstr(ci.name)})? {{ return Ok(()); }}",
                    "        let r = (|| -> rt::Res<()> {"]
            if sup is not None and self.needs_init(sup):
                body.append(f"            unsafe {{ {self.init_name(sup)}() }}.res()?;")
            if not ci.is_interface:
                for si in self.init_iface_supers(ci):
                    if self.needs_init(si):
                        body.append(f"            unsafe {{ {self.init_name(si)}() }}.res()?;")
            for f in self.string_consts(ci):
                slot, data = self.lit(self.cv(f)[1])
                body.append(f"            unsafe {{ {self.static_name(ci, f)} = rt::lit(&raw mut {slot}, &{data}); }}")
            clinit = ci.cf.method("<clinit>", "()V")
            if clinit is not None:
                self.need_method(ci, clinit)
                body.append(f"            unsafe {{ {fname(ci.name, '<clinit>', '()V')}() }}.res()?;")
            body += ["            Ok(())", "        })();", f"        let r = rt::init_failure({rstr(ci.name)}, r);", f"        rt::init_leave(&{st}, r.is_ok());", "        r", "    })())", "}"]
            out.append("\n".join(body))
        for m in ci.cf.methods:
            if (m.code is not None or m.is_native) and self.is_reachable(ci.name, m):
                self.need_method(ci, m)
        return "\n".join(out) + "\n"


_SYM_RE = None


def _sym_scan(text: str) -> set[str]:
    global _SYM_RE
    if _SYM_RE is None:
        import re
        _SYM_RE = re.compile(r"\b(?:CLS|S|init|m|LIT|O)_[A-Za-z0-9_]+\b")
    return set(_SYM_RE.findall(text))


def _generate_parts(main: str, natives_src: str, cp_dirs: list, strict: bool = False,
                    no_lwjgl: bool = False, reflect_classes: list[str] | None = None,
                    shards: int = 1, use_escape: bool = False, use_patches: bool = True,
                    analyze_only: bool = False, escape_budget: int = 120, use_inline: bool = False) -> tuple[dict[str, str], Gen]:
    sys.path.insert(0, ROOT)
    from .classlib import build_classlib
    from .classpath import ClassPath, builtin_classes, builtin_nolwjgl_classes, jdk_jmods
    from .program import Program, Report
    from .world import World

    classlib = build_classlib(False)
    builtin = builtin_classes()
    nolwjgl = builtin_nolwjgl_classes() if no_lwjgl else None
    pre = [x for x in (builtin, nolwjgl) if x]
    entries = pre + list(cp_dirs) + [classlib] + jdk_jmods()
    napp = len(pre) + len(cp_dirs)
    cp = ClassPath(entries, release=25, release_overrides={"lwjgl": 11}, boot_start=napp)
    world = World(cp)
    prog = Program(world, Report())
    if reflect_classes:
        prog.reflect_classes.extend(n.replace(".", "/") for n in reflect_classes)
    if world.get("java/util/ImmutableCollections$Access") is not None:
        prog.reflect_classes.append("java/util/ImmutableCollections$Access")
    main = main.replace(".", "/")
    if world.get(main) is None:
        raise RustGenError(f"main class {main} not found")
    from . import patches as P
    pset = P.load(os.environ.get("MONO_PATCH_MANIFEST") or None) if use_patches else P.PatchSet({}, [])
    pset.validate(world)
    prog.skip_scan = pset.replaced()
    prog.run(main, extra_roots=pset.roots(), extra_vroots=pset.virtual_roots())
    if analyze_only:
        return world, prog, cp
    if use_inline:
        from .inline import inline_program

        def _caller_sensitive(dc, m):
            return m.code is not None and any(i.op == "invokestatic" and i.arg.owner == "jdk/internal/reflect/Reflection"
                                              and i.arg.name == "getCallerClass" for i in prog.insns(dc, m))
        st = inline_program(prog, pset, _caller_sensitive, RECYCLABLE_ITERATORS)
        sys.stderr.write(f"monolith: inlined {st['sites']} call sites in {st['methods']} methods\n")
    esc_res = None
    if use_escape:
        from .escape import analyze as escape_analyze
        esc_res = escape_analyze(prog, budget_s=float(escape_budget))
        n_frame = sum(len(esc_res.noescape_sites(cn, key)) for (cn, key) in esc_res.methods)
        sys.stderr.write(f"monolith: escape analysis {esc_res.seconds:.0f}s, {n_frame} frame-slot allocation sites\n")
    g = Gen(world, prog, natives_src, esc_res=esc_res, patches=pset)
    g.shards = shards

    mci = g.need_class(main)
    mm = mci.cf.method("main", "([Ljava/lang/String;)V")
    if mm is None:
        raise RustGenError(f"{main} has no main(String[])")
    g.need_method(mci, mm)
    obj = g.need_class("java/lang/Object")
    sci = g.need_class("java/lang/String")
    ccls = g.need_class("java/lang/Class")
    if world.instance_fields(ccls)[0][1].name != "monoClass":
        raise RustGenError("java.lang.Class: first field must be monoClass")
    names = [f.name for _, f in world.instance_fields(sci)]
    if names != ["value", "hash", "hashIsZero", "coder"]:
        raise RustGenError(f"java.lang.String layout {names} does not match rt::StrObj")
    thr_init = {}
    for cn in RUNTIME_THROWN:
        ci = g.need_class(cn)
        r = world.resolve_method(cn, "<init>", "(Ljava/lang/String;)V")
        if r is None:
            raise RustGenError(f"{cn} has no <init>(String)")
        g.need_method(r[0], r[1])
        thr_init[cn] = (ci, r)
    for cn in OPTIONAL_THROWN:
        if world.get(cn) is None:
            continue
        r = world.resolve_method(cn, "<init>", "(Ljava/lang/String;)V")
        if r is not None:
            ci = g.need_class(cn)
            g.need_method(r[0], r[1])
            thr_init[cn] = (ci, r)
    boot = {}
    for cn, mn, md in (("java/lang/Thread", "monoStartMain", "()Ljava/lang/Thread;"),
                       ("java/lang/Thread", "monoUncaught", "(Ljava/lang/Thread;Ljava/lang/Throwable;)V"),
                       ("java/lang/Thread", "monoRun", "(Ljava/lang/Thread;)V"),
                       ("java/lang/System", "monoShutdown", "()V")):
        g.need_class(cn)
        r = world.resolve_method(cn, mn, md)
        if r is None:
            raise RustGenError(f"{cn}.{mn}{md} missing in the class library overlay")
        g.need_method(r[0], r[1])
        boot[mn] = r
    g.need_class("java/lang/Throwable")
    gm = world.resolve_method("java/lang/Throwable", "getMessage", "()Ljava/lang/String;")
    g.need_method(gm[0], gm[1])
    if world.get("java/util/ImmutableCollections$Access") is not None:
        g.need_class("java/util/ImmutableCollections$Access")
    for px in sorted(prog.proxies):
        if world.get(px) is not None:
            g.need_class(px)
    for rc in sorted(set(prog.reflect_classes) | prog.reflected_seen):
        if world.get(rc) is not None:
            g.need_class(rc)
    if no_lwjgl:
        for rc in ("org/lwjgl/system/MemoryManage$DebugAllocator", "org/lwjgl/system/MemoryUtil$LazyInit"):
            if world.get(rc) is not None:
                g.need_class(rc)

    class_code: dict[str, str] = {}
    class_methods: dict[str, list[str]] = {}
    class_weights: dict[str, int] = {}

    def drain():
        while g.class_q or g.method_q:
            while g.class_q:
                ci = g.class_q.pop()
                class_code[ci.name] = g.emit_class(ci)
                class_methods.setdefault(ci.name, [])
                class_weights.setdefault(ci.name, 64)
            if g.method_q:
                ci, m = g.method_q.pop()
                ms = class_methods.setdefault(ci.name, [])
                if m.is_native:
                    ms.append(g.emit_native(ci, m))
                    class_weights[ci.name] = class_weights.get(ci.name, 64) + 32
                    continue
                if m.code is None:
                    continue
                try:
                    mg = None
                    if g.use_mir:
                        from . import mir as _mir
                        from .mir_emit import MirMethodGen
                        try:
                            mg = MirMethodGen(g, ci, m)
                            body_txt = mg.generate()
                        except _mir.MirError as mex:
                            g.mir_fallback[str(mex).split(":")[-1][:48].strip()] += 1
                            mg = None
                    if mg is None:
                        mg = MethodGen(g, ci, m)
                        body_txt = mg.generate()
                    elif g.use_mir:
                        g.mir_methods += 1
                        g.mir_inlined += mg.inlined
                    ms.append(body_txt)
                    class_weights[ci.name] = class_weights.get(ci.name, 64) + 40 + len(m.code.code)
                except RustGenError as ex:
                    if strict:
                        raise
                    g.errors.append(str(ex))
                    args, ret = parse_method_desc(m.desc)
                    params = (["this: rt::Ref"] if not m.is_static else []) + [f"a{j}: {RT[kind_of(d)]}" for j, d in enumerate(args)]
                    fn_nm = fname(ci.name, m.name, m.desc)
                    cret = cret_type(ret)
                    cwrap = cret_wrap(ret)
                    g.fn_sigs[fn_nm] = f"pub fn {fn_nm}({', '.join(params)}) -> {cret};"
                    ms.append(f"#[no_mangle] pub extern \"C\" fn {fn_nm}({', '.join(params)}) -> {cret} "
                              f"{{ rt::{cwrap}(rt::unsupported_code({rstr(str(ex))})) }}\n")

    BOX = [
        ("Z", "java/lang/Boolean", "i8", "i32"),
        ("B", "java/lang/Byte", "i8", "i32"),
        ("C", "java/lang/Character", "u16", "i32"),
        ("S", "java/lang/Short", "i16", "i32"),
        ("I", "java/lang/Integer", "i32", "i32"),
        ("J", "java/lang/Long", "i64", "i64"),
        ("F", "java/lang/Float", "f32", "f32"),
        ("D", "java/lang/Double", "f64", "f64"),
    ]
    box_info = {}
    for p, bn, fty, sty in BOX:
        bci = world.get(bn)
        if bci is not None:
            vm = bci.cf.method("valueOf", f"({p})L{bn};")
            if vm is not None:
                g.need_class(bn)
                g.need_method(bci, vm)
                box_info[p] = (bci, vm, fty, sty)
    ok_box = len(box_info) == len(BOX)
    arr_iface_cls = []
    for ain in ("java/lang/Cloneable", "java/io/Serializable"):
        aci = world.get(ain)
        if aci is not None:
            g.need_class(ain)
            arr_iface_cls.append(aci)

    drain()
    vh_owner = None
    if g.vh_used:
        vhc = g.need_class("java/lang/invoke/VarHandle")
        drain()
        vs = g.struct_name(vhc)
        vh_owner = vhc.name
        class_code[vh_owner] += (
            f"#[no_mangle] pub static VH_OFFSETS: [usize; 3] = ["
            f"std::mem::offset_of!({vs}, f_{mang(vhc.name)}_monoKind), "
            f"std::mem::offset_of!({vs}, f_{mang(vhc.name)}_monoOffset), "
            f"std::mem::offset_of!({vs}, f_{mang(vhc.name)}_monoBigEndian)];\n"
        )

    main_tail = []
    if not g.vh_used:
        main_tail.append("pub static VH_OFFSETS: [usize; 3] = [0, 0, 0];")
    for k in ARR_KINDS:
        nm = "[" + (k if k != "A" else "Ljava/lang/Object;")
        ec_expr = f"Some(unsafe {{ &{g.class_name(obj)} }})" if k == "A" else "None"
        main_tail.append(
            f"#[no_mangle] pub static CLS_ARR_{k}: rt::Class = rt::Class {{ name: {rstr(nm)}, super_: Some(unsafe {{ &{g.class_name(obj)} }}), size: 24, "
            f"vtable: {g.object_vtable}, elem: b'{k}', elem_class: {ec_expr}, array_cache: rt::ArrCache::NEW, "
            f"itab: &[], ifaces: &ARRAY_IFACES, fields: &[], ref_offsets: &[], static_refs: &[], methods: &[], refl: None, mirror: rt::Mirror::NEW, kind: 2, access: 0x411, init: None, enum_values: None }};"
        )
    main_tail.append(f"static ARRAY_IFACES: [&rt::Class; {len(arr_iface_cls)}] = [" +
                     ", ".join(f"unsafe {{ &{g.class_name(c)} }}" for c in arr_iface_cls) + "];")

    if ok_box and g.shapes:
        WIDEN_FROM = {
            "Z": ["Z"], "B": ["B"], "C": ["C"], "S": ["B", "S"],
            "I": ["B", "S", "C", "I"], "J": ["B", "S", "C", "I", "J"],
            "F": ["B", "S", "C", "I", "J", "F"], "D": ["B", "S", "C", "I", "J", "F", "D"],
        }
        main_tail.append("fn rarg(args: rt::Ref, i: i32) -> rt::Res<rt::Ref> { if args.is_null() { Ok(rt::Ref::NULL) } else { rt::aload::<rt::Ref>(args, i) } }")
        for p, (bci, vm, fty, sty) in box_info.items():
            unbox_arms = []
            for q in WIDEN_FROM[p]:
                qci = box_info[q][0]
                cast_e = f"val as {sty}"
                unbox_arms.append(f"    if std::ptr::eq(c, unsafe {{ &{g.class_name(qci)} }}) {{ let val = unsafe {{ (*(r.0 as *mut {g.struct_name(qci)})).f_{mang(qci.name)}_value }}; return Ok({cast_e}); }}")
            main_tail.append(
                f"fn unbox_{p}(r: rt::Ref) -> rt::Res<{sty}> {{\n"
                f"    let p = rt::nn(r)?;\n    let c = unsafe {{ (*p).class }};\n" +
                "\n".join(unbox_arms) +
                "\n    rt::throw(\"java/lang/IllegalArgumentException\", \"argument type mismatch\".to_string())\n}"
            )
            init_b = f"unsafe {{ {g.init_name(bci)}() }}.res()?; " if g.needs_init(bci) else ""
            main_tail.append(
                f"fn box_{p}(v: {sty}) -> rt::Res<rt::Ref> {{ {init_b}unsafe {{ {fname(bci.name, vm.name, vm.desc)}(v) }}.res() }}"
            )
        ref_arms = []
        rk2k = lambda c: "A" if c == "A" else kind_of(c)
        for key, sid in g.shapes.items():
            ps_s, rkind = key[1:].split(":")
            static = key[0] == "S"
            ptypes = ([] if static else ["rt::Ref"]) + [RT[rk2k(c)] for c in ps_s]
            cret = "rt::CVoid" if rkind == "V" else f"rt::CRes<{RT[rk2k(rkind)]}>"
            cargs = ([] if static else ["recv"]) + [
                f"rarg(args, {i})?" if c == "A" else f"unbox_{c}(rarg(args, {i})?)?"
                for i, c in enumerate(ps_s)
            ]
            fp_ty = f"extern \"C\" fn({', '.join(ptypes)}) -> {cret}"
            call = f"(unsafe {{ std::mem::transmute::<*const (), {fp_ty}>(fp) }})({', '.join(cargs)}).res()?"
            if rkind == "V":
                arm_body = f"{call}; Ok(rt::Ref::NULL)"
            elif rkind == "A":
                arm_body = f"Ok({call})"
            else:
                arm_body = f"box_{rkind}({call})"
            ref_arms.append(f"        {sid} => {{ {arm_body} }}")
        if len(ref_arms) > 400:
            chunks = [ref_arms[i:i + 400] for i in range(0, len(ref_arms), 400)]
            for ci, ch in enumerate(chunks):
                main_tail.append(
                    f"#[inline(never)] fn reflect_h{ci}(shape: u32, fp: *const (), recv: rt::Ref, args: rt::Ref) -> rt::Res<Option<rt::Ref>> {{\n"
                    "    Ok(Some((match shape {\n" + "\n".join(ch) + "\n"
                    "        _ => return Ok(None),\n    })?))\n}"
                )
            calls = "\n".join(f"    if let Some(r) = reflect_h{ci}(shape, fp, recv, args)? {{ return Ok(r); }}" for ci in range(len(chunks)))
            main_tail.append(
                "fn reflect_invoke_hook(shape: u32, fp: *const (), recv: rt::Ref, args: rt::Ref) -> rt::Res<rt::Ref> {\n"
                + calls + "\n"
                "    rt::throw(\"java/lang/UnsupportedOperationException\", \"reflective invocation\".to_string())\n}"
            )
        else:
            main_tail.append(
                "fn reflect_invoke_hook(shape: u32, fp: *const (), recv: rt::Ref, args: rt::Ref) -> rt::Res<rt::Ref> {\n"
                "    match shape {\n" + "\n".join(ref_arms) + "\n"
                "        _ => rt::throw(\"java/lang/UnsupportedOperationException\", \"reflective invocation\".to_string()),\n    }\n}"
            )
    else:
        main_tail.append(
            "fn reflect_invoke_hook(_shape: u32, _fp: *const (), _recv: rt::Ref, _args: rt::Ref) -> rt::Res<rt::Ref> {\n"
            "    rt::throw(\"java/lang/UnsupportedOperationException\", \"reflective invocation\".to_string())\n}"
        )

    arms = []
    for cn, (ci, r) in thr_init.items():
        init = f"unsafe {{ {g.init_name(ci)}() }}.res().ok(); " if g.needs_init(ci) else ""
        arms.append(f'        {rstr(cn)} => {{ {init}let o = rt::alloc(unsafe {{ &{g.class_name(ci)} }}); '
                    f'if unsafe {{ {fname(r[0].name, r[1].name, r[1].desc)}(o, msg) }}.res().is_err() {{ rt::abort("exception constructor threw") }} o }}')
    main_tail.append("fn throw_new_hook(class: &str, msg: rt::Ref) -> rt::Ref {\n    match class {\n" + "\n".join(arms) +
                     "\n        _ => rt::abort(&format!(\"runtime threw {}, which is not a runtime-thrown class\", class)),\n    }\n}")
    main_tail.append(f"fn thread_run_hook(t: rt::Ref) -> rt::Res<()> {{ unsafe {{ {fname(boot['monoRun'][0].name, boot['monoRun'][1].name, boot['monoRun'][1].desc)}(t) }}.res() }}")
    main_tail.append("fn uncaught_hook(t: rt::Ref) -> ! {\n"
                     "    let name = unsafe { (*t.0).class.name }.replace('/', \".\");\n"
                     f"    let m = unsafe {{ {fname(gm[0].name, gm[1].name, gm[1].desc)}(t) }}.res().unwrap_or(rt::Ref::NULL);\n"
                     "    if m.is_null() { eprintln!(\"Exception in thread \\\"main\\\" {}\", name); }\n"
                     "    else { eprintln!(\"Exception in thread \\\"main\\\" {}: {}\", name, rt::str_to_rust(m)); }\n"
                     "    std::process::exit(1)\n}")
    prov_entries = []
    for svc, provs in sorted(getattr(prog, "services", {}).items()):
        for pv in provs:
            k = len(prov_entries)
            pci = world.get(pv)
            if pci is None:
                continue
            g.need_class(pv)
            if (svc, pv) in getattr(prog, "service_factories", set()):
                fm = next((m for m in pci.cf.methods if m.name == "provider" and m.is_static and m.desc.startswith("()")), None)
                if fm is None:
                    continue
                g.need_method(pci, fm)
                init_p = f"unsafe {{ {g.init_name(pci)}() }}.res()?; " if g.needs_init(pci) else ""
                main_tail.append(f"fn new_provider_{k}() -> rt::Res<rt::Ref> {{ {init_p}unsafe {{ {fname(pv, 'provider', fm.desc)}() }}.res() }}")
            else:
                cm = pci.cf.method("<init>", "()V")
                if cm is None:
                    continue
                g.need_method(pci, cm)
                init_p = f"unsafe {{ {g.init_name(pci)}() }}.res()?; " if g.needs_init(pci) else ""
                main_tail.append(
                    f"fn new_provider_{k}() -> rt::Res<rt::Ref> {{\n"
                    f"    {init_p}let o = rt::alloc(unsafe {{ &{g.class_name(pci)} }});\n"
                    f"    unsafe {{ {fname(pv, '<init>', '()V')}(o) }}.res()?;\n    Ok(o)\n}}"
                )
            prov_entries.append((svc.replace("/", "."), k))
    main_tail.append(
        f"static SERVICES: [rt::ServiceEntry; {len(prov_entries)}] = ["
        + ", ".join(f"rt::ServiceEntry {{ service: {rstr(svc)}, create: new_provider_{k} }}" for svc, k in prov_entries)
        + "];"
    )

    UE = "sun/nio/fs/UnixException"
    uci = world.get(UE)
    um = uci.cf.method("<init>", "(I)V") if uci else None
    if um is not None and (UE, um.key) in prog.reachable:
        g.need_class(UE)
        g.need_method(uci, um)
        init_u = f"unsafe {{ {g.init_name(uci)}() }}.res()?; " if g.needs_init(uci) else ""
        main_tail.append(
            f"fn new_unix_exception_hook(errno: i32) -> rt::Res<rt::Ref> {{\n"
            f"    {init_u}let o = rt::alloc(unsafe {{ &{g.class_name(uci)} }});\n"
            f"    unsafe {{ {fname(UE, '<init>', '(I)V')}(o, errno) }}.res()?;\n    Ok(o)\n}}"
        )
    else:
        main_tail.append(
            "fn new_unix_exception_hook(_errno: i32) -> rt::Res<rt::Ref> {\n"
            "    rt::throw(\"java/io/IOException\", \"UnixException unavailable\".to_string())\n}"
        )

    DB = "java/nio/DirectByteBuffer"
    dci = world.get(DB)
    dm = dci.cf.method("<init>", "(JJ)V") if dci else None
    if dm is not None and (DB, dm.key) in prog.reachable:
        g.need_class(DB)
        g.need_method(dci, dm)
        init_d = f"unsafe {{ {g.init_name(dci)}() }}.res()?; " if g.needs_init(dci) else ""
        main_tail.append(
            f"fn new_direct_byte_buffer_hook(addr: i64, cap: i64) -> rt::Res<rt::Ref> {{\n"
            f"    {init_d}let o = rt::alloc(unsafe {{ &{g.class_name(dci)} }});\n"
            f"    unsafe {{ {fname(DB, '<init>', '(JJ)V')}(o, addr, cap) }}.res()?;\n    Ok(o)\n}}"
        )
    else:
        main_tail.append(
            "fn new_direct_byte_buffer_hook(_addr: i64, _cap: i64) -> rt::Res<rt::Ref> {\n"
            "    rt::throw(\"java/lang/InternalError\", \"DirectByteBuffer unavailable\".to_string())\n}"
        )

    drain()

    entry = fname(main, "main", "([Ljava/lang/String;)V")
    tcls = g.need_class("java/lang/Thread")
    scls = g.need_class("java/lang/System")
    F = lambda k: fname(boot[k][0].name, boot[k][1].name, boot[k][1].desc)
    inits = "".join(f"unsafe {{ {g.init_name(c)}() }}.res()?; " for c in (tcls, scls, mci) if g.needs_init(c))
    arrs = ", ".join(f"&CLS_ARR_{k}" for k in ARR_KINDS)
    allc = ", ".join(f"unsafe {{ &{g.class_name(world.get(n))} }}" for n in sorted(g.seen_classes))
    main_tail.append(f"static ALL_CLASSES: [&rt::Class; {len(g.seen_classes)}] = [{allc}];")
    main_tail.append(
        "fn main() {\n"
        f"    rt::init_hooks(rt::Hooks {{ object_class: unsafe {{ &{g.class_name(obj)} }}, string_class: unsafe {{ &{g.class_name(sci)} }}, class_class: unsafe {{ &{g.class_name(ccls)} }}, all_classes: &ALL_CLASSES, arrays: [{arrs}], array_ifaces: &ARRAY_IFACES, "
        "vh_kind: unsafe { VH_OFFSETS[0] }, vh_offset: unsafe { VH_OFFSETS[1] }, vh_bigendian: unsafe { VH_OFFSETS[2] }, "
        "thread_run: thread_run_hook, throw_new: throw_new_hook, uncaught: uncaught_hook, reflect_invoke: reflect_invoke_hook, "
        "services: &SERVICES, new_unix_exception: new_unix_exception_hook, new_direct_byte_buffer: new_direct_byte_buffer_hook });\n"
        "    let _gc_main = rt::gc_thread_guard();\n"
        f"    let th = match (|| -> rt::Res<rt::Ref> {{ unsafe {{ {g.init_name(tcls)}() }}.res()?; unsafe {{ {F('monoStartMain')}() }}.res() }})() {{ Ok(t) => t, Err(t) => uncaught_hook(t.obj) }};\n"
        "    rt::set_main_thread(th);\n"
        f"    let r = (|| -> rt::Res<()> {{ {inits}unsafe {{ {entry}(rt::program_args()?) }}.res() }})();\n"
        "    let mut code = 0;\n"
        f"    if let Err(t) = r {{ code = 1; if unsafe {{ {F('monoUncaught')}(th, t.obj) }}.res().is_err() {{ uncaught_hook(t.obj) }} }}\n"
        f"    if unsafe {{ {F('monoShutdown')}() }}.res().is_err() {{ rt::abort(\"exception during shutdown\") }}\n"
        "    std::process::exit(code)\n}\n"
    )
    main_body = "\n".join(main_tail) + "\n"

    lit_by_slot = {slot: (data, u) for (slot, data, u) in g.lits.values()}
    hdr = "#![allow(warnings, improper_ctypes, static_mut_refs)]\nuse mono_rt::{rt, vh, sync, natives};\n"

    def emit_lits_for(syms: set[str]) -> str:
        lines = []
        for s in sorted(syms):
            if s in lit_by_slot:
                data, u = lit_by_slot[s]
                lines.append(f"static mut {s}: rt::Ref = rt::Ref::NULL;")
                lines.append(f"static {data}: [u16; {len(u)}] = [{', '.join(map(str, u))}];")
        return "\n".join(lines) + ("\n" if lines else "")

    if shards <= 1:
        chunks = [hdr]
        for cn in sorted(class_code):
            chunks.append(class_code[cn])
            chunks.extend(class_methods.get(cn, ()))
        full = "\n".join(chunks) + "\n" + main_body
        return {"main.rs": full + emit_lits_for(_sym_scan(full))}, g

    sorted_cls = sorted(class_code)
    nshards = max(1, min(shards, len(sorted_cls)))
    buckets: list[list[str]] = [[] for _ in range(nshards)]
    for cn in sorted_cls:
        # g.shard_of: eski yolda SHA1 (önceki satırın aynısı), MONO_SHARD=pkg ile paket tablosu. rust_callable da aynı
        # fonksiyonu kullanır: doğrudan çağrının hedefi gerçekten aynı parça dosyasında olmalı.
        idx = g.shard_of(cn) if nshards == g.shards else int(hashlib.sha1(cn.encode()).hexdigest()[:8], 16) % nshards
        buckets[idx].append(cn)

    arr_syms = {f"CLS_ARR_{k}" for k in ARR_KINDS}
    files: dict[str, str] = {}

    def wrap_unit(body: str, owned: set[str], need_vh_ext: bool = False) -> str:
        syms = _sym_scan(body)
        ext_cls = [s for s in sorted(syms) if s in g.cls_syms and s not in owned]
        ext_stat = [s for s in sorted(syms) if s in g.static_sigs and s not in owned]
        ext_fn = [s for s in sorted(syms) if s in g.fn_sigs and s not in owned]
        ext_structs = [g.struct_defs[s] for s in sorted(syms) if s in g.struct_defs and s not in owned]
        ex = []
        if need_vh_ext:
            ex.append("    pub static VH_OFFSETS: [usize; 3];")
        for s in ext_cls:
            ex.append(f"    pub static {s}: rt::Class;")
        for s in ext_stat:
            ex.append(f"    pub static mut {s}: {g.static_sigs[s]};")
        for s in ext_fn:
            ex.append(f"    {g.fn_sigs[s]}")
        ext_block = ("extern \"C\" {\n" + "\n".join(ex) + "\n}\n") if ex else ""
        return hdr + ext_block + "".join(ext_structs) + emit_lits_for(syms) + body

    for idx, cnames in enumerate(buckets):
        cnames.sort()
        chunks = []
        owned: set[str] = set()
        for cn in cnames:
            chunks.append(class_code[cn])
            chunks.extend(class_methods.get(cn, ()))
        body = "\n".join(chunks)
        # Bu shard'da tanımlanan her sembolün (CLS_, O_, S_, init_, m_) tanımı burada bulunur:
        for cn in cnames:
            ci = world.get(cn)
            owned.add(g.class_name(ci))
            owned.add(g.struct_name(ci))
            if g.needs_init(ci):
                owned.add(g.init_name(ci))
            for f in ci.cf.fields:
                if f.is_static:
                    owned.add(g.static_name(ci, f))
            for m in ci.cf.methods:
                fn_nm = fname(ci.name, m.name, m.desc)
                if fn_nm in g.fn_sigs:
                    owned.add(fn_nm)
        files[f"shard_{idx:02d}.rs"] = wrap_unit(body, owned)

    files["main.rs"] = wrap_unit(main_body, arr_syms, need_vh_ext=g.vh_used)
    return files, g


def generate(main: str, natives_src: str, cp_dirs: list, strict: bool = False,
             no_lwjgl: bool = False, reflect_classes: list[str] | None = None) -> tuple:
    files, g = _generate_parts(main, natives_src, cp_dirs, strict, no_lwjgl=no_lwjgl, reflect_classes=reflect_classes, shards=1)
    return files["main.rs"], g


def _older_obj(cache_dir: str, name: str, opt: str, key: str):
    """Newest cached object of a shard whose generated source is byte-identical (`key`), built against any runtime
    version. For runtime changes that keep the existing code valid (new functions or modules); a changed runtime
    function that shards inline would keep its old behaviour in such an object."""
    import glob
    hits = glob.glob(os.path.join(cache_dir, f"{name}_{opt}_*_{key}.o"))
    return max(hits, key=os.path.getmtime) if hits else None


def prune_shard_cache(cache_dir: str, limit_bytes: int | None = None) -> int:
    """Keeps the compiled-object cache under `limit_bytes` (default 8 GiB, MONO_SHARD_CACHE_GB overrides): oldest files first
    (a cache hit refreshes a file's time). The cache is only a speed-up, so deleting from it costs a rebuild and nothing else;
    without a limit it grew by a few hundred MB for every changed shard and filled a 250 GB disk (42 GB measured).
    Returns the number of files removed."""
    if limit_bytes is None:
        try:
            limit_bytes = int(float(os.environ.get("MONO_SHARD_CACHE_GB", "8")) * (1 << 30))
        except ValueError:
            limit_bytes = 8 << 30
    entries = []
    total = 0
    for dp, _dns, fns in os.walk(cache_dir):
        for f in fns:
            p = os.path.join(dp, f)
            try:
                st = os.stat(p)
            except OSError:
                continue
            entries.append((st.st_mtime, st.st_size, p))
            total += st.st_size
    removed = 0
    for _mt, size, p in sorted(entries):
        if total <= limit_bytes:
            break
        try:
            os.remove(p)
            total -= size
            removed += 1
        except OSError:
            pass
    return removed


def _touch(path: str) -> None:
    """Marks a cache file as used now, so the pruning above removes the least recently used first."""
    try:
        os.utime(path)
    except OSError:
        pass


def _to_library_entry(main_rs: str) -> str:
    """Turn the generated `fn main()` into `pub fn mono_run() -> i32` (the host process keeps running)."""
    head = "fn main() {\n    rt::init_hooks"
    tail = "    std::process::exit(code)\n}\n"
    if head not in main_rs or tail not in main_rs:
        raise SystemExit("monolith: generated main.rs has an unexpected shape; cannot build a library entry")
    return main_rs.replace(head, "pub fn mono_run() -> i32 {\n    rt::init_hooks", 1).replace(tail, "    code\n}\n", 1)


def c9_mode() -> bool:
    """MONO_FRAMES=dwarf: yığın izi çağrı başına kayıt yerine makine yığını + DWARF + yan tablodan (PIPELINE 27)."""
    return os.environ.get("MONO_FRAMES", "") == "dwarf"


def c9_flags() -> list:
    """C9 kipinde her rustc çağrısına eklenen bayraklar: çerçeve işaretçisi zinciri ve satır tablosu (satır içi bilgisiyle)."""
    return ["-C", "force-frame-pointers=yes", "-C", "debuginfo=line-tables-only"] if c9_mode() else []


def _apply_line_tables(files: dict) -> dict:
    """C9: her parçadan ve main.rs'den çalışma anı çerçeve kaydını siler, yan tabloyu çıkarır, main.rs'ye tabloları
    kaydeden çağrıyı ekler. Döndürür: {"shard_05.lt": bayt, ...}. files yerinde değişir."""
    from . import linetab
    tables = {}
    names = []
    for fn in sorted(files):
        if fn.startswith("shard_") and fn.endswith(".rs"):
            files[fn], tables[fn[:-3] + ".lt"] = linetab.build(files[fn], strip=True)
            names.append(fn)
    names.append("main.rs")
    reg = ", ".join(f'("{n}", include_bytes!("{n[:-3]}.lt"))' for n in names)
    anchor = "    rt::init_hooks("
    if anchor not in files["main.rs"]:
        raise SystemExit("monolith: C9: main.rs has no rt::init_hooks call to register the line tables before")
    files["main.rs"] = files["main.rs"].replace(anchor, f"    mono_rt::jtrace::register(&[{reg}]);\n" + anchor, 1)
    files["main.rs"], tables["main.lt"] = linetab.build(files["main.rs"], strip=True)
    return tables


LINK_STUB = r"""#![allow(warnings)]
//! Android host entry of the Monolith engine. The TouchCore launcher dlopens this library and calls
//! `mono_android_entry(user, gameDir, assetsDir, input)`; the game runs on its own thread.
use std::ffi::{c_char, c_void, CStr};
use std::sync::atomic::{AtomicBool, Ordering};
extern crate mc_main;
extern crate mono_rt;

static STARTED: AtomicBool = AtomicBool::new(false);

fn pick(key: &str, arg: *const c_char, dflt: &str) -> String {
    if let Ok(v) = std::env::var(key) { if !v.is_empty() { return v; } }
    if !arg.is_null() {
        let s = unsafe { CStr::from_ptr(arg) }.to_string_lossy().into_owned();
        if !s.is_empty() { return s; }
    }
    dflt.to_string()
}

#[no_mangle]
pub extern "C" fn mono_android_entry(user: *const c_char, gdir: *const c_char, adir: *const c_char, _input: *mut c_void) -> i32 {
    if STARTED.swap(true, Ordering::SeqCst) { return 0; }
    let user = pick("MC_USERNAME", user, "");
    let mut args: Vec<String> = Vec::new();
    if !user.is_empty() {
        let kv = [("--username", user), ("--gameDir", pick("MC_GAME_DIR", gdir, ".")),
                  ("--assetsDir", pick("MC_ASSETS_DIR", adir, "assets")),
                  ("--version", pick("MC_VERSION", std::ptr::null(), "Monolith")),
                  ("--accessToken", "0".to_string()), ("--userType", "legacy".to_string())];
        for (k, v) in kv { args.push(k.to_string()); args.push(v); }
        if let Ok(idx) = std::env::var("MC_ASSET_INDEX") {
            if !idx.is_empty() { args.push("--assetIndex".to_string()); args.push(idx); }
        }
        // Minecraft'ın kendi çevrimdışı kipi: Realms denetimi, kullanıcı özelliklerini çekme ve sohbet imza anahtarı
        // isteği yapılmaz (ağ denemesi ve "Couldn't connect to realms" hataları kalkar; oynanışa etkisi yok). MC_ONLINE=1 eski yol.
        if std::env::var("MC_ONLINE").map_or(true, |v| v != "1") { args.push("--offlineDeveloperMode".to_string()); }
    }
    if let Ok(b) = std::env::var("MONO_RHI_BACKEND") {
        if !b.is_empty() { args.push(format!("-Dmono.graphics.backend={}", b)); }
    }
    mono_rt::rt::set_host_args(args);
    match std::thread::Builder::new().name("monolith-main".into()).stack_size(256 << 20).spawn(|| { mc_main::mono_run(); }) {
        Ok(_) => 0,
        Err(_) => -1,
    }
}
"""


def _find_ndk_clang(ndk: str, api: int) -> str:
    import glob
    roots = [ndk] if ndk else []
    for var in ("ANDROID_HOME", "ANDROID_SDK_ROOT"):
        if os.environ.get(var):
            roots += sorted(glob.glob(os.path.join(os.environ[var], "ndk", "*")), reverse=True)
    for r in roots:
        c = os.path.join(r, "toolchains", "llvm", "prebuilt", "linux-x86_64", "bin", f"aarch64-linux-android{api}-clang")
        if os.path.isfile(c):
            return c
    raise SystemExit("monolith: Android NDK clang not found; pass --ndk <ndk root> or set ANDROID_NDK_HOME / ANDROID_HOME")


LIBFFI_URL = "https://github.com/libffi/libffi/releases/download/v3.4.6/libffi-3.4.6.tar.gz"
LIBFFI_SHA256 = "b0dea9df23c863a7a50e825440f3ebffabd65df1497108e5d437747843895a4e"


def _android_libffi(clang: str, cache: str) -> str:
    """Static libffi for aarch64-android (JNA calls go through it). Built once from the official release."""
    import tarfile, urllib.request
    prefix = os.path.join(cache, "libffi")
    if os.path.isfile(os.path.join(prefix, "lib", "libffi.a")):
        return os.path.join(prefix, "lib")
    os.makedirs(cache, exist_ok=True)
    tgz = os.path.join(cache, "libffi.tar.gz")
    urllib.request.urlretrieve(LIBFFI_URL, tgz)
    if hashlib.sha256(open(tgz, "rb").read()).hexdigest() != LIBFFI_SHA256:
        os.remove(tgz)
        raise SystemExit("monolith: libffi archive checksum mismatch")
    with tarfile.open(tgz) as t:
        t.extractall(cache, filter="data")
    src = os.path.join(cache, "libffi-3.4.6")
    tc = os.path.dirname(clang)
    env = dict(os.environ, CC=clang, AR=os.path.join(tc, "llvm-ar"), RANLIB=os.path.join(tc, "llvm-ranlib"), STRIP=os.path.join(tc, "llvm-strip"))
    for cmd in (["./configure", "--host=aarch64-linux-android", "--disable-shared", "--enable-static", "--disable-docs", "--with-pic", f"--prefix={prefix}"],
                ["make", f"-j{os.cpu_count() or 2}"], ["make", "install"]):
        r = subprocess.run(cmd, cwd=src, env=env, capture_output=True, text=True)
        if r.returncode:
            raise SystemExit("monolith: libffi build failed\n" + r.stderr[-3000:])
    return os.path.join(prefix, "lib")


def _rt_sources(rt_dir: str) -> list:
    """Every .rs file of the runtime, including subdirectories (monolith/rt/desk/), as '/'-separated paths
    relative to rt_dir, sorted. The runtime hash and the copies into the build directory both use this list."""
    out = []
    for dirpath, _, names in os.walk(rt_dir):
        for n in names:
            if n.endswith(".rs"):
                out.append(os.path.relpath(os.path.join(dirpath, n), rt_dir).replace(os.sep, "/"))
    return sorted(out)


def _copy_rt_sources(rt_dir: str, dest: str) -> None:
    for rel in _rt_sources(rt_dir):
        d = os.path.join(dest, rel)
        os.makedirs(os.path.dirname(d), exist_ok=True)
        shutil.copy(os.path.join(rt_dir, rel), d)


def native_tls() -> bool:
    """Android derlemesinde çerçeve yığını gerçek yerel TLS'te tutulur. Varsayılan açık (27 Kasım 2026, S26 ölçümü: FPS
    29,8 -> 60,8); MONO_NATIVE_TLS=0 eski yolu (std thread_local!, pthread anahtarı) geri getirir. Yerel TLS'li motor
    yalnızca Android 10 (API 29) ve sonrasında yüklenir; daha eski cihaz için MONO_NATIVE_TLS=0 ile derlenmeli.

    `std`'nin thread_local!'ı Android hedefinde pthread anahtarı kullanır (her erişim pthread_getspecific çağrısı; S26 CPU
    profilinde çerçeve kaydı toplam CPU'nun yaklaşık %60'ı). `#[thread_local]` kararsız bir özellik: derleyiciye
    RUSTC_BOOTSTRAP=1 verilir. Model global-dynamic (TLSDESC): initial-exec denendi ve Android 17 bionic'te dlopen ile
    yüklenen kütüphaneyi reddediyor ("TLS symbol ... using IE access model", qemu + gerçek bağlayıcı ile doğrulandı).
    TLSDESC dlopen'li kütüphanede çalışır (API 29 ve sonrası; daha eskisinde yerel TLS yok)."""
    return os.environ.get("MONO_NATIVE_TLS", "1") != "0"


def native_tls_flags() -> list:
    if not native_tls():
        return []
    # MONO_TLS_SLOT=1 (varsayılan kapalı): çerçeve yığınının adresi bionic'in TLS_SLOT_APP yuvasında (rt.rs with_fast)
    slot = ["--cfg", "mono_tls_slot"] if os.environ.get("MONO_TLS_SLOT", "") == "1" else []
    # MONO_SYM_CACHE=1 (varsayılan kapalı): GL/TouchCore işlev adresleri çağrı yeri başına bir kez aranır (desk/mod.rs sym_fn!)
    slot += ["--cfg", "mono_sym_cache"] if os.environ.get("MONO_SYM_CACHE", "") == "1" else []
    return ["--cfg", "mono_native_tls", "-Z", "tls-model=global-dynamic", *slot]


def _android_build(a, files, rt_dir) -> int:
    """rustc pipeline: mono_rt.rlib + shard objects + mc_main.rlib + link stub -> cdylib."""
    from concurrent.futures import ThreadPoolExecutor
    target = "aarch64-linux-android"
    clang = _find_ndk_clang(a.ndk, a.api)
    out = os.path.abspath(a.output)
    base = ["rustc", "--target", target, "--edition", "2021", "-C", "panic=abort", "-C", f"linker={clang}", *c9_flags(),
            *native_tls_flags()]
    rt_hasher = hashlib.sha256()
    for f in _rt_sources(rt_dir):
        rt_hasher.update(f.encode())
        rt_hasher.update(open(os.path.join(rt_dir, f), "rb").read())
    rt_hasher.update(rt_deps_key().encode())
    rt_hasher.update(" ".join(c9_flags()).encode())  # C9 bayrakları: önbellekteki bayraksız derlemeyle karışmasın
    rt_hasher.update(" ".join(native_tls_flags()).encode())  # yerel TLS: pthread anahtarlı önbellekle karışmasın
    rt_hash = rt_hasher.hexdigest()[:16]
    cache = os.path.join("/tmp", "monolith_shard_cache", "android")
    os.makedirs(cache, exist_ok=True)
    prune_shard_cache(os.path.dirname(cache))

    run_env = dict(os.environ, RUSTC_BOOTSTRAP="1") if native_tls() else None  # yalnızca rustc alt süreçleri için

    def run(cmd, what):
        r = subprocess.run(cmd, capture_output=True, text=True, env=run_env)
        if r.returncode:
            sys.stderr.write(f"monolith: {what} failed\n{r.stderr[-6000:]}")
        return r.returncode

    rlib = os.path.join(out, "libmono_rt.rlib")
    DEPS = rt_deps_flags()
    cached = os.path.join(cache, f"libmono_rt_{a.opt}_{rt_hash}.rlib")
    if os.path.exists(cached):
        shutil.copy(cached, rlib)
    else:
        if run(base + ["-C", f"opt-level={a.opt}", *DEPS, "--crate-type", "rlib", "--crate-name", "mono_rt",
                       os.path.join(out, "lib.rs"), "-o", rlib], "mono_rt"):
            return 1
        shutil.copy(rlib, cached)

    shard_files = sorted(fn for fn in files if fn.startswith("shard_"))

    def compile_shard(fn):
        name = fn[:-3]
        obj = os.path.join(out, name + ".o")
        key = hashlib.sha256(files[fn].encode("utf-8")).hexdigest()[:20]
        cached_obj = os.path.join(cache, f"{name}_{a.opt}_{rt_hash}_{key}.o")
        if a.reuse_objs and not os.path.exists(cached_obj):
            older = _older_obj(cache, name, a.opt, key)
            if older:
                shutil.copy(older, obj)
                return fn, obj, 0
        if os.path.exists(cached_obj):
            shutil.copy(cached_obj, obj)
            return fn, obj, 0
        rc = run(base + ["-C", f"opt-level={a.opt}", "--crate-type", "lib", "--crate-name", name, "--emit=obj",
                         *DEPS, "--extern", f"mono_rt={rlib}", os.path.join(out, fn), "-o", obj], fn)
        if rc == 0:
            shutil.copy(obj, cached_obj)
        return fn, obj, rc

    objs = []
    with ThreadPoolExecutor(max_workers=max(1, a.shard_jobs)) as ex:
        for fn, obj, rc in ex.map(compile_shard, shard_files):
            if rc:
                return rc
            sys.stderr.write(f"monolith: {fn} done\n")
            objs.append(obj)

    mc_main = os.path.join(out, "libmc_main.rlib")
    if run(base + ["-C", "opt-level=0", "-C", "codegen-units=1", "--crate-type", "rlib", "--crate-name", "mc_main",
                   *DEPS, "--extern", f"mono_rt={rlib}", os.path.join(out, "main.rs"), "-o", mc_main], "mc_main"):
        return 1
    stub = os.path.join(out, "link_stub.rs")
    open(stub, "w", encoding="utf-8").write(LINK_STUB)
    so = os.path.join(out, a.lib_name)
    cmd = base + ["-C", f"opt-level={a.opt}", "--crate-type", "cdylib", "-C", "link-arg=-fuse-ld=lld",
                  "-C", "link-arg=-Wl,-z,max-page-size=16384", "-C", "link-arg=-Wl,--gc-sections", *([] if c9_mode() else ["-C", "strip=debuginfo"]),
                  "-L", f"dependency={out}", "-L", f"native={_android_libffi(clang, cache)}", *DEPS, "--extern", f"mono_rt={rlib}", "--extern", f"mc_main={mc_main}"]
    for o in objs:
        cmd += ["-C", f"link-arg={o}"]
    # libffi (statik) kapanış kodu için __clear_cache çağırır. Bu işlev Android'in libc'sinde yoktur; NDK'nın
    # derleyici çalışma zamanı kütüphanesinde (compiler-rt builtins) bulunur. Eklenmezse sembol çözülmeden kalır
    # ve Android kütüphaneyi yüklerken "cannot locate symbol __clear_cache" hatasıyla reddeder.
    builtins = subprocess.run([clang, "-rtlib=compiler-rt", "-print-libgcc-file-name"], capture_output=True, text=True).stdout.strip()
    if not os.path.isfile(builtins):
        raise SystemExit(f"monolith: NDK compiler-rt builtins not found ({builtins!r})")
    cmd += ["-C", f"link-arg={builtins}"]
    if run(cmd + [stub, "-o", so], "link"):
        return 1
    sys.stderr.write(f"monolith: {so} ({os.path.getsize(so) >> 20} MiB)\n")
    return 0


RT_DEPS = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "rt_deps")


def rt_deps_flags(target: str = "") -> list:
    """Third-party crates of the runtime, built offline by cargo in rt_deps (pinned by Cargo.lock, vendored). The
    runtime currently has none, so the list is empty; every rlib cargo builds for rt_deps would be passed by name.
    A failed build stops the generation (no silent fallback)."""
    tgt = ["--target", target] if target else []
    r = subprocess.run(["cargo", "build", "--release", "--offline", "--message-format=json", *tgt], cwd=RT_DEPS,
                       capture_output=True, text=True)
    if r.returncode:
        raise SystemExit("monolith: rt_deps build failed (cargo build --release --offline in rt_deps)\n" + r.stderr[-3000:])
    import json
    libs = {}
    for line in r.stdout.splitlines():
        try:
            msg = json.loads(line)
        except ValueError:
            continue
        if msg.get("reason") != "compiler-artifact" or msg["target"]["name"] == "mono_rt_deps":
            continue
        for f in msg.get("filenames", []):
            if f.endswith(".rlib"):
                libs[msg["target"]["name"].replace("-", "_")] = f
    if not libs:
        return []
    flags = ["-L", "dependency=" + os.path.dirname(next(iter(libs.values())))]
    for name in sorted(libs):
        flags += ["--extern", f"{name}={libs[name]}"]
    return flags


def rt_deps_key(target: str = "") -> str:
    """Part of the runtime hash: a changed lock means other objects."""
    try:
        return hashlib.sha256(open(os.path.join(RT_DEPS, "Cargo.lock"), "rb").read() + target.encode()).hexdigest()[:8]
    except OSError:
        return "none"


def build_runtime_rlib(target: str, dest: str, opt: str = "3") -> None:
    """Builds monolith/rt as an rlib through cargo: rt_deps/check is a package whose library is the runtime. Cargo
    resolves the runtime's dependencies itself, and the shared target directory (rt_deps/target) makes it reuse the
    same crc32fast and cfg-if artifacts that rt_deps_flags passes to rustc (E0460 cannot arise)."""
    tgt = ["--target", target] if target else []
    # çalışma dizini rt_deps: cargo, gömülü kaynak yapılandırmasını (rt_deps/.cargo) çalışma dizininden okur
    # çalışma zamanı, üretilen kodla aynı optimizasyon düzeyine sahip olmalı: share-generics ile (opt-level 0/1)
    # shard'lar yalnızca aynı düzeyde derlenmiş bir rlib'in dışa aktardığı jenerik örneklere başvurur
    r = subprocess.run(["cargo", "rustc", "--release", "--offline", "--lib", "--manifest-path", "check/Cargo.toml",
                        "--target-dir", "target", "--message-format=json", *tgt, "--", "-C", f"opt-level={opt}", *c9_flags()],
                       cwd=RT_DEPS, capture_output=True, text=True)
    if r.returncode:
        raise SystemExit("monolith: runtime build failed (cargo build in rt_deps/check)\n" + r.stderr[-6000:])
    import json
    found = []
    for line in r.stdout.splitlines():
        try:
            msg = json.loads(line)
        except ValueError:
            continue
        if msg.get("reason") == "compiler-artifact" and msg.get("target", {}).get("name") == "mono_rt":
            found += [f for f in msg.get("filenames", []) if f.endswith(".rlib")]
    if len(found) != 1:
        raise SystemExit(f"monolith: runtime build: expected one mono_rt rlib in cargo's output, found {found}")
    shutil.copy(found[0], dest)


def _host_build(a, files, rt_dir) -> int:
    """rustc pipeline for the host: mono_rt.rlib + shard objects + main.rs -> executable `prog`."""
    rlib = os.path.join(a.output, "libmono_rt.rlib")
    target = getattr(a, "target", "") or ""
    # --target ve --linker bu derlemenin her rustc çağrısına gider (çalışma zamanı, shard'lar, main)
    TGT = ["--target", target] if target else []
    LNK = ["-C", f"linker={a.linker}"] if getattr(a, "linker", "") else []
    is_windows = ("windows" in target) if target else sys.platform == "win32"
    DEPS = rt_deps_flags(target)
    rt_hasher = hashlib.sha256()
    for f in _rt_sources(rt_dir):
        rt_hasher.update(f.encode())
        rt_hasher.update(open(os.path.join(rt_dir, f), "rb").read())
    rt_hasher.update(rt_deps_key(target).encode())
    rt_hasher.update(target.encode())
    rt_hasher.update(" ".join(c9_flags()).encode())  # C9 bayrakları: önbellekteki bayraksız derlemeyle karışmasın
    rt_hash = rt_hasher.hexdigest()[:16]
    cache_dir = os.path.join(tempfile.gettempdir(), "monolith_shard_cache")
    os.makedirs(cache_dir, exist_ok=True)
    prune_shard_cache(cache_dir)
    cached_rlib = os.path.join(cache_dir, f"libmono_rt_{a.opt}_{rt_hash}.rlib")
    if os.path.exists(cached_rlib):
        _touch(cached_rlib)
        shutil.copy(cached_rlib, rlib)
    else:
        build_runtime_rlib(target, rlib, str(a.opt))
        shutil.copy(rlib, cached_rlib)
    # Shard'lardan önce çalışma zamanı rlib'ini yokla: rustc'nin yükleyemediği bir crate, her shard'da bir E0463 olarak değil,
    # rlib'in bilgileriyle birlikte burada bildirilir.
    rlib = os.path.abspath(rlib)
    probe_dir = tempfile.mkdtemp(prefix="mono_probe_")
    with open(os.path.join(probe_dir, "probe.rs"), "w", encoding="utf-8") as fh:
        fh.write("extern crate mono_rt;\npub fn probe() {}\n")
    probe = subprocess.run(["rustc", *TGT, "--edition", "2021", "--crate-type", "lib", "--emit=metadata",
                            *DEPS, "--extern", f"mono_rt={rlib}", os.path.join(probe_dir, "probe.rs"),
                            "-o", os.path.join(probe_dir, "probe.rmeta")], capture_output=True, text=True)
    if probe.returncode:
        size = os.path.getsize(rlib) if os.path.exists(rlib) else -1
        sys.stderr.write(f"monolith: the runtime rlib {rlib} ({size} bytes) cannot be loaded:\n" + probe.stderr[-6000:])
        return probe.returncode
    link_args = []
    shard_files = sorted(fn for fn in files if fn.startswith("shard_"))
    if shard_files:
        from concurrent.futures import ThreadPoolExecutor
        def compile_shard(fn: str):
            name = fn[:-3]
            obj = os.path.join(a.output, name + ".o")
            src_bytes = files[fn].encode("utf-8")
            sh_key = hashlib.sha256(src_bytes).hexdigest()[:20]
            cached_obj = os.path.join(cache_dir, f"{name}_{a.opt}_{rt_hash}_{sh_key}.o")
            if a.reuse_objs and not os.path.exists(cached_obj):
                older = _older_obj(cache_dir, name, a.opt, sh_key)
                if older:
                    shutil.copy(older, obj)
                    return fn, obj, subprocess.CompletedProcess([], 0, "", "")
            if os.path.exists(cached_obj):
                _touch(cached_obj)
                shutil.copy(cached_obj, obj)
                return fn, obj, subprocess.CompletedProcess([], 0, "", "")
            res = subprocess.run(["rustc", *TGT, *LNK, *c9_flags(), "-C", f"opt-level={a.opt}", "--edition", "2021", "-C", "panic=abort",
                                  "--crate-type", "lib", "--crate-name", name, "--emit=obj",
                                  *DEPS, "--extern", f"mono_rt={rlib}",
                                  os.path.join(a.output, fn), "-o", obj], capture_output=True, text=True)
            if res.returncode == 0 and os.path.exists(obj):
                shutil.copy(obj, cached_obj)
            return fn, obj, res
        with ThreadPoolExecutor(max_workers=max(1, a.shard_jobs)) as ex:
            for fn, obj, res in ex.map(compile_shard, shard_files):
                if res.returncode:
                    sys.stderr.write(f"shard {fn} failed:\n" + res.stderr[-6000:])
                    return res.returncode
                link_args.extend(["-C", f"link-arg={obj}"])
    exe = os.path.join(a.output, "prog.exe" if is_windows else "prog")
    main_opt = 0 if len(shard_files) >= 16 else a.opt
    cgu_args = ["-C", "codegen-units=1"] if len(shard_files) >= 16 else []
    # ld.lld yalnızca Linux ana makinesinde (Mach-O ve PE, platformun kendi bağlayıcısıyla bağlanır)
    lld_args = ["-C", "link-arg=-fuse-ld=lld"] if (not target and sys.platform.startswith("linux") and shutil.which("ld.lld")) else []
    r = subprocess.run(["rustc", *TGT, *LNK, *c9_flags(), "-C", f"opt-level={main_opt}", *cgu_args, "--edition", "2021", "-C", "panic=abort",
                        *DEPS, "--extern", f"mono_rt={rlib}", *lld_args, *link_args,
                        os.path.join(a.output, "main.rs"), "-o", exe], capture_output=True, text=True)
    if r.returncode:
        sys.stderr.write(r.stderr[-6000:])
        return r.returncode
    if a.run:
        return subprocess.run([exe]).returncode
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description="Monolith back end: Java bytecode (application + JDK) -> Rust")
    ap.add_argument("--cp", action="append", required=True)
    ap.add_argument("--main", required=True)
    ap.add_argument("-o", "--output", required=True)
    ap.add_argument("--run", action="store_true", help="compile with rustc and run")
    ap.add_argument("--compile", action="store_true", help="compile with rustc without running")
    ap.add_argument("--strict", action="store_true", help="fail on untranslatable code instead of emitting a loud runtime stop")
    ap.add_argument("--no-lwjgl", action="store_true", help="use runtime/java_nolwjgl Zero-LWJGL RHI overlay")
    ap.add_argument("--reflect-class", action="append", default=[], help="class name or @file of reflectable classes")
    ap.add_argument("--shards", type=int, default=1, help="split generated classes across N parallel rustc object shards")
    ap.add_argument("--shard-jobs", type=int, default=2, help="max concurrent rustc shard compilations")
    ap.add_argument("--opt", default="1")
    ap.add_argument("--escape", action="store_true",
                    help="run the whole-program escape analysis: allocation sites whose object cannot outlive the method "
                         "live in a frame slot (rt::stack_obj) and non-escaping arrays in the method's arena "
                         "(rt::new_array_arena, released at return) instead of the collected heap")
    ap.add_argument("--inline", action="store_true",
                    help="inline small callees into their callers before the analysis (monolith/inline.py): the allocations "
                         "of `a.subtract(b)`-style helpers become the caller's own and can be proven non-escaping")
    ap.add_argument("--escape-budget", type=int, default=1800, help="seconds the escape analysis may take (the result is all or nothing)")
    ap.add_argument("--android", action="store_true",
                    help="build libminecraft_mono.so (aarch64-linux-android cdylib exporting mono_android_entry) instead of an executable")
    ap.add_argument("--ndk", default=os.environ.get("ANDROID_NDK_HOME", ""), help="Android NDK root (default: $ANDROID_NDK_HOME or $ANDROID_HOME/ndk/*)")
    ap.add_argument("--api", type=int, default=24, help="Android API level of the NDK linker")
    ap.add_argument("--lib-name", default="libminecraft_mono.so")
    ap.add_argument("--target", default="",
                    help="Rust target triple of the executable (default: the host). Windows targets get prog.exe")
    ap.add_argument("--linker", default="", help="linker program passed to rustc as -C linker=... (cross builds)")
    ap.add_argument("--reuse-objs", action="store_true",
                    help="with --resume: reuse the cached object of every shard whose source is unchanged even when the runtime "
                         "changed (only for runtime changes that leave existing code valid, e.g. a new function or module)")
    ap.add_argument("--resume", action="store_true",
                    help="skip code generation and compile the sources already in the output directory "
                         "(a separate process keeps memory low: the generator alone needs ~5 GiB)")
    a = ap.parse_args()
    cp_dirs = []
    for item in a.cp:
        for p in item.split(os.pathsep):
            if p:
                cp_dirs.append(os.path.abspath(p))
    ref_classes = []
    for spec in a.reflect_class:
        if spec.startswith("@"):
            ref_classes.extend(open(spec[1:], encoding="utf-8").read().split())
        else:
            ref_classes.append(spec)
    rt_dir = os.path.join(ROOT, "monolith", "rt")
    natives = os.path.join(rt_dir, "natives.rs")
    if a.resume:
        files = {fn: open(os.path.join(a.output, fn), encoding="utf-8").read()
                 for fn in sorted(os.listdir(a.output)) if fn.startswith("shard_") and fn.endswith(".rs")}
        if not files:
            raise SystemExit(f"monolith: no shard_*.rs in {a.output}; generate first")
        _copy_rt_sources(rt_dir, a.output)  # pick up runtime fixes without regenerating
        if a.android:
            return _android_build(a, files, rt_dir)
        files["main.rs"] = open(os.path.join(a.output, "main.rs"), encoding="utf-8").read()
        return _host_build(a, files, rt_dir)
    files, g = _generate_parts(a.main, natives, cp_dirs, a.strict, no_lwjgl=a.no_lwjgl, reflect_classes=ref_classes, shards=a.shards,
                                use_escape=a.escape, escape_budget=a.escape_budget, use_inline=a.inline)
    if a.android:
        files["main.rs"] = _to_library_entry(files["main.rs"])
    tables = _apply_line_tables(files) if c9_mode() else {}
    os.makedirs(a.output, exist_ok=True)
    for fn, src in files.items():
        with open(os.path.join(a.output, fn), "w", encoding="utf-8") as fh:
            fh.write(src)
    for fn, b in tables.items():
        with open(os.path.join(a.output, fn), "wb") as fh:
            fh.write(b)
    _copy_rt_sources(rt_dir, a.output)
    from .classpath import jdk_home
    jh = jdk_home()
    tz = os.path.join(jh, "lib", "tzdb.dat")
    if os.path.exists(tz):
        os.makedirs(os.path.join(a.output, "lib"), exist_ok=True)
        shutil.copy(tz, os.path.join(a.output, "lib", "tzdb.dat"))
    conf = os.path.join(jh, "conf")
    if os.path.isdir(conf):
        shutil.copytree(conf, os.path.join(a.output, "conf"), dirs_exist_ok=True)
    sys.stderr.write(f"monolith: {len(g.seen_classes)} classes, {len(g.seen_methods)} methods, "
                     f"{len(g.errors)} untranslatable, {len(g.link_errors)} link-errors, {len(g.missing_natives)} natives missing\n")
    if g.use_mir:
        sys.stderr.write(f"monolith: MIR path {g.mir_methods} methods ({g.mir_inlined} calls inlined), fallback {sum(g.mir_fallback.values())} "
                         f"{dict(g.mir_fallback.most_common(6))}\n")
    if g.patches:
        sys.stderr.write("monolith: " + g.patches.report() + "\n")
    if os.environ.get("MONO_VERBOSE"):
        for x in g.errors:
            sys.stderr.write("  untranslatable: " + x + "\n")
        for x in g.link_errors:
            sys.stderr.write("  link-error: " + x + "\n")
        for x in g.missing_natives:
            sys.stderr.write("  missing native: " + x + "\n")
    if a.android:
        return _android_build(a, files, rt_dir)
    if a.run or a.compile:
        return _host_build(a, files, rt_dir)
    return 0


if __name__ == "__main__":
    sys.exit(main())
