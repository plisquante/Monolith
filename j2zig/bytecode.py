"""JVM bytecode decoder: turns a Code attribute into a list of decoded instructions.

Every opcode of JVMS §6.5 (0x00..0xC9) is known. jsr/ret (0xA8, 0xA9, 0xC8, 0xC9)
are decoded but reported as unsupported by the analyzer (javac stopped emitting
them with class file version 50; they cannot appear in version >= 51 classes).
"""
from __future__ import annotations

import struct
from dataclasses import dataclass

OPNAMES = {}
_ops = """
0 nop;1 aconst_null;2 iconst_m1;3 iconst_0;4 iconst_1;5 iconst_2;6 iconst_3;7 iconst_4;8 iconst_5;
9 lconst_0;10 lconst_1;11 fconst_0;12 fconst_1;13 fconst_2;14 dconst_0;15 dconst_1;16 bipush;17 sipush;
18 ldc;19 ldc_w;20 ldc2_w;21 iload;22 lload;23 fload;24 dload;25 aload;26 iload_0;27 iload_1;28 iload_2;
29 iload_3;30 lload_0;31 lload_1;32 lload_2;33 lload_3;34 fload_0;35 fload_1;36 fload_2;37 fload_3;
38 dload_0;39 dload_1;40 dload_2;41 dload_3;42 aload_0;43 aload_1;44 aload_2;45 aload_3;46 iaload;
47 laload;48 faload;49 daload;50 aaload;51 baload;52 caload;53 saload;54 istore;55 lstore;56 fstore;
57 dstore;58 astore;59 istore_0;60 istore_1;61 istore_2;62 istore_3;63 lstore_0;64 lstore_1;65 lstore_2;
66 lstore_3;67 fstore_0;68 fstore_1;69 fstore_2;70 fstore_3;71 dstore_0;72 dstore_1;73 dstore_2;
74 dstore_3;75 astore_0;76 astore_1;77 astore_2;78 astore_3;79 iastore;80 lastore;81 fastore;
82 dastore;83 aastore;84 bastore;85 castore;86 sastore;87 pop;88 pop2;89 dup;90 dup_x1;91 dup_x2;
92 dup2;93 dup2_x1;94 dup2_x2;95 swap;96 iadd;97 ladd;98 fadd;99 dadd;100 isub;101 lsub;102 fsub;
103 dsub;104 imul;105 lmul;106 fmul;107 dmul;108 idiv;109 ldiv;110 fdiv;111 ddiv;112 irem;113 lrem;
114 frem;115 drem;116 ineg;117 lneg;118 fneg;119 dneg;120 ishl;121 lshl;122 ishr;123 lshr;124 iushr;
125 lushr;126 iand;127 land;128 ior;129 lor;130 ixor;131 lxor;132 iinc;133 i2l;134 i2f;135 i2d;
136 l2i;137 l2f;138 l2d;139 f2i;140 f2l;141 f2d;142 d2i;143 d2l;144 d2f;145 i2b;146 i2c;147 i2s;
148 lcmp;149 fcmpl;150 fcmpg;151 dcmpl;152 dcmpg;153 ifeq;154 ifne;155 iflt;156 ifge;157 ifgt;
158 ifle;159 if_icmpeq;160 if_icmpne;161 if_icmplt;162 if_icmpge;163 if_icmpgt;164 if_icmple;
165 if_acmpeq;166 if_acmpne;167 goto;168 jsr;169 ret;170 tableswitch;171 lookupswitch;172 ireturn;
173 lreturn;174 freturn;175 dreturn;176 areturn;177 return;178 getstatic;179 putstatic;180 getfield;
181 putfield;182 invokevirtual;183 invokespecial;184 invokestatic;185 invokeinterface;
186 invokedynamic;187 new;188 newarray;189 anewarray;190 arraylength;191 athrow;192 checkcast;
193 instanceof;194 monitorenter;195 monitorexit;196 wide;197 multianewarray;198 ifnull;199 ifnonnull;
200 goto_w;201 jsr_w
"""
for part in _ops.replace("\n", "").split(";"):
    part = part.strip()
    if part:
        n, s = part.split(" ")
        OPNAMES[int(n)] = s
OPCODES = {v: k for k, v in OPNAMES.items()}

UNSUPPORTED = {"jsr", "ret", "jsr_w"}

ATYPE = {4: "Z", 5: "C", 6: "F", 7: "D", 8: "B", 9: "S", 10: "I", 11: "J"}


@dataclass
class Insn:
    pc: int
    op: str
    arg: object = None        # constant / cp entry / local index / immediate
    arg2: object = None       # iinc const, multianewarray dims, switch data
    targets: tuple = ()       # branch targets (pcs)
    length: int = 1

    def __repr__(self):
        a = "" if self.arg is None else f" {self.arg!r}"
        t = f" ->{list(self.targets)}" if self.targets else ""
        return f"{self.pc}: {self.op}{a}{t}"


def decode(code: bytes, cp: list) -> list[Insn]:
    out: list[Insn] = []
    p = 0
    n = len(code)
    while p < n:
        start = p
        opc = code[p]
        op = OPNAMES.get(opc)
        if op is None:
            raise ValueError(f"unknown opcode 0x{opc:02x} at {p}")
        p += 1
        ins = Insn(start, op)
        if op == "wide":
            sub = OPNAMES[code[p]]
            idx = struct.unpack_from(">H", code, p + 1)[0]
            if sub == "iinc":
                ins = Insn(start, "iinc", idx, struct.unpack_from(">h", code, p + 3)[0])
                p += 5
            else:
                ins = Insn(start, sub, idx)
                p += 3
        elif op in ("bipush",):
            ins.arg = struct.unpack_from(">b", code, p)[0]
            p += 1
        elif op == "sipush":
            ins.arg = struct.unpack_from(">h", code, p)[0]
            p += 2
        elif op == "ldc":
            ins.arg = cp[code[p]]
            p += 1
        elif op in ("ldc_w", "ldc2_w"):
            ins.arg = cp[struct.unpack_from(">H", code, p)[0]]
            ins.op = "ldc"
            p += 2
        elif op in ("iload", "lload", "fload", "dload", "aload",
                    "istore", "lstore", "fstore", "dstore", "astore", "ret"):
            ins.arg = code[p]
            p += 1
        elif op[-2:] in ("_0", "_1", "_2", "_3") and ("load" in op or "store" in op):
            base, k = op.rsplit("_", 1)
            ins.op = base
            ins.arg = int(k)
        elif op == "iinc":
            ins.arg = code[p]
            ins.arg2 = struct.unpack_from(">b", code, p + 1)[0]
            p += 2
        elif op.startswith("if") or op in ("goto", "jsr"):
            off = struct.unpack_from(">h", code, p)[0]
            ins.targets = (start + off,)
            p += 2
        elif op in ("goto_w", "jsr_w"):
            off = struct.unpack_from(">i", code, p)[0]
            ins.op = "goto" if op == "goto_w" else "jsr_w"
            ins.targets = (start + off,)
            p += 4
        elif op == "tableswitch":
            p = (p + 3) & ~3
            default, low, high = struct.unpack_from(">iii", code, p)
            p += 12
            offs = struct.unpack_from(f">{high - low + 1}i", code, p)
            p += 4 * (high - low + 1)
            keys = list(range(low, high + 1))
            ins.arg = start + default
            ins.arg2 = [(k, start + o) for k, o in zip(keys, offs)]
            ins.targets = tuple([start + default] + [start + o for o in offs])
        elif op == "lookupswitch":
            p = (p + 3) & ~3
            default, npairs = struct.unpack_from(">ii", code, p)
            p += 8
            pairs = []
            for _ in range(npairs):
                k, o = struct.unpack_from(">ii", code, p)
                pairs.append((k, start + o))
                p += 8
            ins.op = "tableswitch"  # unified representation: default + (key, target) pairs
            ins.arg = start + default
            ins.arg2 = pairs
            ins.targets = tuple([start + default] + [t for _, t in pairs])
        elif op in ("getstatic", "putstatic", "getfield", "putfield",
                    "invokevirtual", "invokespecial", "invokestatic",
                    "new", "anewarray", "checkcast", "instanceof"):
            ins.arg = cp[struct.unpack_from(">H", code, p)[0]]
            p += 2
        elif op == "invokeinterface":
            ins.arg = cp[struct.unpack_from(">H", code, p)[0]]
            p += 4
        elif op == "invokedynamic":
            ins.arg = cp[struct.unpack_from(">H", code, p)[0]]
            p += 4
        elif op == "newarray":
            ins.arg = ATYPE[code[p]]
            p += 1
        elif op == "multianewarray":
            ins.arg = cp[struct.unpack_from(">H", code, p)[0]]
            ins.arg2 = code[p + 2]
            p += 3
        ins.length = p - start
        out.append(ins)
    return out


def is_terminal(op: str) -> bool:
    return op in ("goto", "tableswitch", "athrow", "ireturn", "lreturn", "freturn",
                  "dreturn", "areturn", "return", "ret")
