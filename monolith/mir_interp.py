"""Reference interpreter for MIR (test oracle of work package C1, docs/mimari/05_DOGRULAMA.md T4).

Runs the primitive/array/static-call subset with exact Java semantics (32/64-bit wrap, truncating division, shift masks, saturating
float->int, IEEE compares). Anything else raises Unsupported. Used by monolith/mir_tests.py to compare the MIR of a compiled
sample against the JVM's own output; it is not part of the product.
"""
import math
import struct

from .mir import Fn


class Unsupported(Exception):
    pass


class JavaThrow(Exception):
    def __init__(self, cls):
        super().__init__(cls)
        self.cls = cls


_SUPER = {"java/lang/ArithmeticException": "java/lang/RuntimeException",
          "java/lang/ArrayIndexOutOfBoundsException": "java/lang/IndexOutOfBoundsException",
          "java/lang/IndexOutOfBoundsException": "java/lang/RuntimeException",
          "java/lang/NegativeArraySizeException": "java/lang/RuntimeException",
          "java/lang/NullPointerException": "java/lang/RuntimeException",
          "java/lang/RuntimeException": "java/lang/Exception", "java/lang/Exception": "java/lang/Throwable"}


def _is(cls, catch):
    while cls:
        if cls == catch:
            return True
        cls = _SUPER.get(cls)
    return False


def i32(x):
    x &= 0xFFFFFFFF
    return x - (1 << 32) if x & 0x80000000 else x


def i64(x):
    x &= 0xFFFFFFFFFFFFFFFF
    return x - (1 << 64) if x & (1 << 63) else x


def f32(x):
    try:
        return struct.unpack(">f", struct.pack(">f", x))[0]
    except OverflowError:
        return math.copysign(math.inf, x)


def _f2i(x, lo, hi):
    if x != x:
        return 0
    if x >= hi:
        return hi
    if x <= lo:
        return lo
    return int(x)


def _cmp(a, b, nan):
    if a != a or b != b:
        return nan
    return (a > b) - (a < b)


def _div(a, b, wrap):
    if b == 0:
        raise JavaThrow("java/lang/ArithmeticException")
    q = abs(a) // abs(b)
    return wrap(q if (a < 0) == (b < 0) else -q)


def _rem(a, b, wrap):
    if b == 0:
        raise JavaThrow("java/lang/ArithmeticException")
    return wrap(a - b * (abs(a) // abs(b)) * (1 if (a < 0) == (b < 0) else -1))


def _fdiv(a, b):
    if b == 0:
        return math.nan if (a == 0 or a != a) else math.copysign(math.inf, a) * math.copysign(1, b)
    return a / b


def _frem(a, b):
    if b == 0 or a != a or b != b or math.isinf(a):
        return math.nan
    return math.fmod(a, b)


class Interp:
    def __init__(self, fns: dict, steps: int = 5_000_000):
        self.fns = fns          # (name, desc) -> Fn
        self.budget = steps

    def call(self, name, desc, args):
        return self.run(self.fns[(name, desc)], args)

    def run(self, fn: Fn, args: list):
        blk = fn.blocks[fn.entry]
        env = {}
        # metot argümanları giriş bloğunun parametrelerini yuva sırasıyla doldurur (long/double: tek mantıksal giriş)
        if len(args) != len(blk.params):
            raise Unsupported(f"{fn.name}: {len(args)} args for {len(blk.params)} entry params")
        for p, a in zip(blk.params, args):
            env[p] = a
        while True:
            for i in blk.insns:
                self.budget -= 1
                if self.budget < 0:
                    raise Unsupported("step budget")
                try:
                    r = self.exec(i, env)
                except JavaThrow as t:
                    nxt = self._handler(i.exc, t, env)
                    if nxt is None:
                        raise
                    tgt = fn.blocks[nxt.target]
                    vals = [env[a] if a is not None else t for a in nxt.args]
                    for p, v in zip(tgt.params, vals):
                        env[p] = v
                    blk = tgt
                    break
                if i.dst is not None:
                    env[i.dst] = r
            else:
                t = blk.term
                if t.kind == "ret":
                    return env[t.args[0]] if t.args else None
                if t.kind == "throw":
                    raise JavaThrow("java/lang/Throwable")
                if t.kind == "goto":
                    e = t.edges[0]
                elif t.kind == "if":
                    e = t.edges[0] if self.cond(t.op, [env[a] for a in t.args]) else t.edges[1]
                else:
                    v = env[t.args[0]]
                    e = t.edges[0]
                    for k, ed in zip(t.keys, t.edges[1:]):
                        if k == v:
                            e = ed
                            break
                tgt = fn.blocks[e.target]
                vals = [env[a] for a in e.args]
                for p, v in zip(tgt.params, vals):
                    env[p] = v
                blk = tgt

    def _handler(self, edges, t, env):
        for e in edges:
            if e.catch is None or _is(t.cls, e.catch):
                return e
        return None

    @staticmethod
    def cond(op, a):
        x = a[0]
        if op in ("ifnull", "ifnonnull"):
            return (x is None) == (op == "ifnull")
        if len(a) == 2:
            y = a[1]
            return {"if_icmpeq": x == y, "if_icmpne": x != y, "if_icmplt": x < y, "if_icmpge": x >= y, "if_icmpgt": x > y,
                    "if_icmple": x <= y, "if_acmpeq": x is y, "if_acmpne": x is not y}[op]
        return {"ifeq": x == 0, "ifne": x != 0, "iflt": x < 0, "ifge": x >= 0, "ifgt": x > 0, "ifle": x <= 0}[op]

    def exec(self, i, env):
        op, a = i.op, [env[x] for x in i.args]
        at = i.attr
        if op in ("iconst_m1", "iconst_0", "iconst_1", "iconst_2", "iconst_3", "iconst_4", "iconst_5", "bipush", "sipush",
                  "lconst_0", "lconst_1", "fconst_0", "fconst_1", "fconst_2", "dconst_0", "dconst_1"):
            return at
        if op == "ldc":
            if isinstance(at, tuple) and at[0] in ("int", "long"):
                return at[1]
            if isinstance(at, tuple) and at[0] == "float":
                return struct.unpack(">f", at[1])[0]
            if isinstance(at, tuple) and at[0] == "double":
                return struct.unpack(">d", at[1])[0]
            raise Unsupported("ldc")
        if op == "aconst_null":
            return None
        if op == "copy":
            return a[0]
        if op == "nullcheck":
            if a[0] is None:
                raise JavaThrow("java/lang/NullPointerException")
            return None
        if op == "iinc":
            return i32(a[0] + at)
        c, k = op[0], op[1:]
        if c in "il" and k in ("add", "sub", "mul"):
            w = i32 if c == "i" else i64
            return w({"add": a[0] + a[1], "sub": a[0] - a[1], "mul": a[0] * a[1]}[k])
        if c in "il" and k == "div":
            return _div(a[0], a[1], i32 if c == "i" else i64)
        if c in "il" and k == "rem":
            return _rem(a[0], a[1], i32 if c == "i" else i64)
        if c in "il" and k == "neg":
            return (i32 if c == "i" else i64)(-a[0])
        if c in "il" and k in ("and", "or", "xor"):
            return {"and": a[0] & a[1], "or": a[0] | a[1], "xor": a[0] ^ a[1]}[k]
        if c in "il" and k in ("shl", "shr", "ushr"):
            bits = 32 if c == "i" else 64
            s = a[1] & (bits - 1)
            if k == "shl":
                return (i32 if c == "i" else i64)(a[0] << s)
            if k == "shr":
                return a[0] >> s
            return (i32 if c == "i" else i64)((a[0] & ((1 << bits) - 1)) >> s)
        if c in "fd" and k in ("add", "sub", "mul", "div", "rem", "neg"):
            r = {"add": lambda: a[0] + a[1], "sub": lambda: a[0] - a[1], "mul": lambda: a[0] * a[1],
                 "div": lambda: _fdiv(a[0], a[1]), "rem": lambda: _frem(a[0], a[1]), "neg": lambda: -a[0]}[k]()
            return f32(r) if c == "f" else r
        conv = {"i2l": lambda x: x, "i2f": f32, "i2d": float, "l2i": i32, "l2f": lambda x: f32(float(x)), "l2d": float,
                "f2i": lambda x: _f2i(x, -2**31, 2**31 - 1), "f2l": lambda x: _f2i(x, -2**63, 2**63 - 1), "f2d": lambda x: x,
                "d2i": lambda x: _f2i(x, -2**31, 2**31 - 1), "d2l": lambda x: _f2i(x, -2**63, 2**63 - 1), "d2f": f32,
                "i2b": lambda x: ((x + 128) & 0xFF) - 128, "i2c": lambda x: x & 0xFFFF, "i2s": lambda x: ((x + 32768) & 0xFFFF) - 32768}
        if op in conv:
            return conv[op](a[0])
        if op == "lcmp":
            return (a[0] > a[1]) - (a[0] < a[1])
        if op in ("fcmpl", "dcmpl"):
            return _cmp(a[0], a[1], -1)
        if op in ("fcmpg", "dcmpg"):
            return _cmp(a[0], a[1], 1)
        if op == "newarray":
            if a[0] < 0:
                raise JavaThrow("java/lang/NegativeArraySizeException")
            return [0.0 if at in "FD" else 0] * a[0]
        if op == "arraylength":
            if a[0] is None:
                raise JavaThrow("java/lang/NullPointerException")
            return len(a[0])
        if op.endswith("aload") and len(op) == 6:
            return self._aidx(a[0], a[1])[a[1]]
        if op.endswith("astore") and len(op) == 7:
            arr = self._aidx(a[0], a[1])
            v = a[2]
            arr[a[1]] = ((v + 128) & 0xFF) - 128 if op[0] == "b" else v & 0xFFFF if op[0] == "c" else \
                ((v + 32768) & 0xFFFF) - 32768 if op[0] == "s" else v
            return None
        if op == "invokestatic" and (at.owner, at.name) == ("java/lang/Double", "compare"):
            x, y = a
            if x < y:
                return -1
            if x > y:
                return 1
            bx, by = struct.unpack(">q", struct.pack(">d", x))[0], struct.unpack(">q", struct.pack(">d", y))[0]
            if x != x:
                bx = 0x7ff8000000000000
            if y != y:
                by = 0x7ff8000000000000
            return (bx > by) - (bx < by)
        if op == "invokestatic" and at.owner == self.owner:
            return self.call(at.name, at.desc, a)
        raise Unsupported(op)

    owner = ""

    @staticmethod
    def _aidx(arr, idx):
        if arr is None:
            raise JavaThrow("java/lang/NullPointerException")
        if idx < 0 or idx >= len(arr):
            raise JavaThrow("java/lang/ArrayIndexOutOfBoundsException")
        return arr
