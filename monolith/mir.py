"""MIR builder (docs/mimari/01_DERLEYICI.md section 3, work package C1).

Bytecode -> typed SSA with block parameters. Types come from the StackMapTable (`stackmap.py`) at every block entry and from
the instruction semantics inside a block; no dataflow fixpoint is needed because the verifier already recorded the state at
every branch target and handler.

    fn = build(ci, method, insns)          # insns: program.insns(ci, m) (indy already desugared)
    verify(fn)                             # structural checks, raises MirError
    print(dump(fn))

Types are strings: "int" "long" "float" "double", "null", "L<name>;", "[<desc>". `long`/`double` take one logical entry.
Locals are kept as `slot -> Value`; a block's parameters are the live locals (per its frame) followed by its stack entries.

Instructions keep the bytecode opcode names (`iadd`, `getfield`, `invokevirtual`, ...) in three-address form: `dst = op(args)`
with `attr` holding the constant-pool operand. Pure data movement (loads, stores, dup, swap, pop) disappears: it is renaming.
Every instruction that may throw carries `exc`, one Edge per covering handler (the last arg of an exc Edge is None: the
thrown object, supplied by the runtime). Terminators: goto, if (op + 2 edges: taken, fall-through), switch, ret, throw.
"""
from dataclasses import dataclass, field
from typing import Optional

from .bytecode import Insn
from .classfile import ClassConst, MemberRef
from .stackmap import Frame, frames as stack_frames
from .world import parse_method_desc

ACC_STATIC = 0x0008


class MirError(Exception):
    pass


class MirUnsupported(MirError):
    """Construct the MIR path does not handle (reported as mir_fallback, never silent)."""


@dataclass(eq=False)
class Value:
    id: int
    ty: str
    cls: Optional[str] = None   # `new`: the class of an uninitialized object

    def __repr__(self):
        return f"v{self.id}"


@dataclass
class Edge:
    target: int                 # block id == the pc of its first instruction
    args: list                  # Values (None = the thrown object of an exception edge)
    catch: Optional[str] = None  # exception edges: the caught class (None = any)
    watched: Optional[dict] = None  # build(watch=...): local slot -> Value held on this edge


@dataclass
class MInsn:
    op: str
    dst: Optional[Value]
    args: list
    attr: object = None
    pc: int = -1
    exc: list = field(default_factory=list)
    ins: Optional[Insn] = None   # the bytecode instruction this came from (the emitter hands it to MethodGen.gen_insn)


@dataclass
class Term:
    kind: str                   # goto | if | switch | ret | throw
    op: str = ""                # if: the bytecode branch opcode
    args: list = field(default_factory=list)
    edges: list = field(default_factory=list)   # if: [taken, fall]; switch: [default, case...]; goto: [target]
    keys: list = field(default_factory=list)    # switch case keys
    exc: list = field(default_factory=list)     # throw only
    pc: int = -1


@dataclass
class Block:
    id: int
    params: list
    param_slots: list           # per param: local slot, or -1 for a stack entry
    insns: list = field(default_factory=list)
    term: Optional[Term] = None


@dataclass
class Fn:
    cls: str
    name: str
    desc: str
    static: bool
    blocks: dict
    entry: int
    nvalues: int


_CAT2 = ("long", "double")
_NOTHROW = set("""nop aconst_null iconst_m1 iconst_0 iconst_1 iconst_2 iconst_3 iconst_4 iconst_5 lconst_0 lconst_1 fconst_0
fconst_1 fconst_2 dconst_0 dconst_1 bipush sipush iadd ladd fadd dadd isub lsub fsub dsub imul lmul fmul dmul fdiv ddiv frem drem
ineg lneg fneg dneg ishl lshl ishr lshr iushr lushr iand land ior lor ixor lxor iinc i2l i2f i2d l2i l2f l2d f2i f2l f2d d2i d2l d2f
i2b i2c i2s lcmp fcmpl fcmpg dcmpl dcmpg goto return ireturn lreturn freturn dreturn areturn""".split())
_ARITH = {"i": "int", "l": "long", "f": "float", "d": "double"}
_CONV = {"i2l": "long", "i2f": "float", "i2d": "double", "l2i": "int", "l2f": "float", "l2d": "double", "f2i": "int", "f2l": "long",
         "f2d": "double", "d2i": "int", "d2l": "long", "d2f": "float", "i2b": "int", "i2c": "int", "i2s": "int"}
_IFS = {"ifeq", "ifne", "iflt", "ifge", "ifgt", "ifle", "if_icmpeq", "if_icmpne", "if_icmplt", "if_icmpge", "if_icmpgt",
        "if_icmple", "if_acmpeq", "if_acmpne", "ifnull", "ifnonnull"}
_IF_ARGS = {op: (1 if op in ("ifnull", "ifnonnull") or len(op) == 4 else 2) for op in _IFS}


def desc_type(d: str) -> str:
    c = d[0]
    if c in "ZBCSI":
        return "int"
    return {"F": "float", "J": "long", "D": "double"}.get(c, d)


def ref_type(name: str) -> str:
    return name if name.startswith("[") else "L" + name + ";"


def _elem(arr_ty: str) -> str:
    if arr_ty.startswith("["):
        return desc_type(arr_ty[1:])
    return "Ljava/lang/Object;"


class _B:
    def __init__(self, ci, m, insns, frs, watch=None):
        self.watch = watch or {}
        self.ci, self.m = ci, m
        self.insns = insns
        self.idx = {i.pc: k for k, i in enumerate(insns)}
        self.init = frs[0]
        self.frame_at = {f.pc: f for f in frs[1:]}
        self.blocks: dict = {}
        self.nv = 0
        self.queue: list = []
        self.cur_snapshot = None
        self.handlers = m.code.handlers

    def val(self, ty, cls=None) -> Value:
        if cls is None and ty.startswith("uninit@"):
            cls = self.insns[self.idx[int(ty[7:])]].arg.name
        v = Value(self.nv, ty, cls)
        self.nv += 1
        return v

    # ---- bloklar
    def _slots(self, frame: Frame):
        out, s = [], 0
        for t in frame.locals:
            if t != "top":
                out.append((s, t))
            s += 2 if t in _CAT2 else 1
        return out

    def framed(self, pc: int) -> Block:
        b = self.blocks.get(pc)
        if b is not None:
            return b
        f = self.frame_at.get(pc)
        if f is None:
            raise MirError(f"no StackMapTable frame at branch target {pc}")
        params, slots = [], []
        for s, t in self._slots(f):
            params.append(self.val(t))
            slots.append(s)
        for t in f.stack:
            params.append(self.val(t))
            slots.append(-1)
        b = Block(pc, params, slots)
        self.blocks[pc] = b
        self.queue.append(pc)
        return b

    def implicit(self, pc: int, locs: dict, stack: list) -> Block:
        params, slots = [], []
        for s in sorted(locs):
            params.append(self.val(locs[s].ty, locs[s].cls))
            slots.append(s)
        for v in stack:
            params.append(self.val(v.ty, v.cls))
            slots.append(-1)
        b = Block(pc, params, slots)
        self.blocks[pc] = b
        self.queue.append(pc)
        return b

    def edge(self, pc: int, locs: dict, stack: list) -> Edge:
        b = self.blocks.get(pc)
        if b is None:
            b = self.framed(pc) if pc in self.frame_at else self.implicit(pc, locs, stack)
        args = []
        for s in b.param_slots:
            if s >= 0:
                v = locs.get(s)
                if v is None:
                    raise MirError(f"{self.ci.name}.{self.m.name}: slot {s} undefined at edge to {pc}")
                args.append(v)
        nstack = sum(1 for s in b.param_slots if s < 0)
        if nstack != len(stack):
            raise MirError(f"{self.ci.name}.{self.m.name}: stack depth {len(stack)} != {nstack} at edge to {pc}")
        args.extend(stack)
        w = self.watch.get(pc)
        return Edge(pc, args, None, {sl: locs.get(sl) for sl in w} if w else None)

    def exc_edges(self, pc: int, locs: dict) -> list:
        out = []
        for h in self.handlers:
            if h.start_pc <= pc < h.end_pc:
                b = self.framed(h.handler_pc)
                args = []
                for s in b.param_slots:
                    if s >= 0:
                        v = locs.get(s)
                        if v is None:
                            raise MirError(f"{self.ci.name}.{self.m.name}: slot {s} undefined for handler {h.handler_pc} at {pc}")
                        args.append(v)
                args.append(None)
                out.append(Edge(h.handler_pc, args, h.catch_type))
        return out

    # ---- kurma
    def build(self) -> Fn:
        params, slots = [], []
        for s, t in self._slots(self.init):
            params.append(self.val(t))
            slots.append(s)
        self.blocks[0] = Block(0, params, slots)
        self.queue.append(0)
        while self.queue:
            self.run_block(self.queue.pop())
        m = self.m
        return Fn(self.ci.name, m.name, m.desc, bool(m.access & ACC_STATIC), self.blocks, 0, self.nv)

    def run_block(self, start: int) -> None:
        blk = self.blocks[start]
        locs, stack = {}, []
        for p, s in zip(blk.params, blk.param_slots):
            if s >= 0:
                locs[s] = p
            else:
                stack.append(p)
        k = self.idx[start]
        first = True
        last_pc = start
        while True:
            ins = self.insns[k]
            if not first and ins.pc in self.frame_at:
                blk.term = Term("goto", edges=[self.edge(ins.pc, locs, stack)], pc=last_pc)
                return
            first = False
            last_pc = ins.pc
            if self.step(blk, ins, locs, stack):
                return
            k += 1

    def pop(self, stack):
        return stack.pop()

    def step(self, blk: Block, ins: Insn, locs: dict, stack: list) -> bool:
        op = ins.op
        pc = ins.pc
        ci = self.ci

        def emit(dst_ty, args, attr=None, cls=None):
            dst = self.val(dst_ty, cls) if dst_ty else None
            exc = [] if op in _NOTHROW else self.exc_edges(pc, locs)
            blk.insns.append(MInsn(op, dst, args, attr, pc, exc, ins))
            if dst is not None:
                stack.append(dst)
            return dst

        # ---- sabitler
        if op == "nop":
            return False
        if op == "aconst_null":
            emit("null", [])
        elif op.startswith("iconst_") or op in ("bipush", "sipush"):
            emit("int", [], ins.arg if op in ("bipush", "sipush") else (-1 if op[7:] == "m1" else int(op[7:])))
        elif op.startswith("lconst_"):
            emit("long", [], int(op[7:]))
        elif op.startswith("fconst_"):
            emit("float", [], float(op[7:]))
        elif op.startswith("dconst_"):
            emit("double", [], float(op[7:]))
        elif op == "ldc":
            a = ins.arg
            if isinstance(a, tuple):
                t = a[0]
                if t in ("int", "long", "float", "double"):
                    emit(t, [], a)
                elif t == "string":
                    emit("Ljava/lang/String;", [], a)
                else:
                    raise MirUnsupported(f"ldc {t}")
            elif isinstance(a, ClassConst):
                emit("Ljava/lang/Class;", [], a)
            else:
                raise MirUnsupported(f"ldc {type(a).__name__}")
        # ---- yerel değişkenler (yalnızca yeniden adlandırma)
        elif op in ("iload", "lload", "fload", "dload", "aload"):
            v = locs.get(ins.arg)
            if v is None:
                raise MirError(f"{ci.name}.{self.m.name}@{pc}: load of undefined slot {ins.arg}")
            stack.append(v)
        elif op in ("istore", "lstore", "fstore", "dstore", "astore"):
            v = stack.pop()
            locs[ins.arg] = v
            if v.ty in _CAT2:
                locs.pop(ins.arg + 1, None)
            if ins.arg - 1 in locs and locs[ins.arg - 1].ty in _CAT2:
                del locs[ins.arg - 1]
        elif op == "iinc":
            old = locs[ins.arg]
            dst = self.val("int")
            blk.insns.append(MInsn("iinc", dst, [old], ins.arg2, pc, [], ins))
            locs[ins.arg] = dst
        # ---- yığın karıştırma
        elif op == "pop":
            stack.pop()
        elif op == "pop2":
            if stack.pop().ty not in _CAT2:
                stack.pop()
        elif op == "dup":
            stack.append(stack[-1])
        elif op == "dup_x1":
            a, b = stack.pop(), stack.pop()
            stack.extend([a, b, a])
        elif op == "dup_x2":
            a, b = stack.pop(), stack.pop()
            if b.ty in _CAT2:
                stack.extend([a, b, a])
            else:
                c = stack.pop()
                stack.extend([a, c, b, a])
        elif op == "dup2":
            if stack[-1].ty in _CAT2:
                stack.append(stack[-1])
            else:
                stack.extend(stack[-2:])
        elif op == "dup2_x1":
            a = stack.pop()
            if a.ty in _CAT2:
                b = stack.pop()
                stack.extend([a, b, a])
            else:
                b, c = stack.pop(), stack.pop()
                stack.extend([b, a, c, b, a])
        elif op == "dup2_x2":
            a = stack.pop()
            if a.ty in _CAT2:
                b = stack.pop()
                if b.ty in _CAT2:
                    stack.extend([a, b, a])
                else:
                    c = stack.pop()
                    stack.extend([a, c, b, a])
            else:
                b, c = stack.pop(), stack.pop()
                if c.ty in _CAT2:
                    stack.extend([b, a, c, b, a])
                else:
                    d = stack.pop()
                    stack.extend([b, a, d, c, b, a])
        elif op == "swap":
            stack[-1], stack[-2] = stack[-2], stack[-1]
        # ---- aritmetik
        elif op in _CONV:
            emit(_CONV[op], [stack.pop()])
        elif op in ("lcmp", "fcmpl", "fcmpg", "dcmpl", "dcmpg"):
            b, a = stack.pop(), stack.pop()
            emit("int", [a, b])
        elif op in ("ineg", "lneg", "fneg", "dneg"):
            emit(_ARITH[op[0]], [stack.pop()])
        elif op[0] in "ilfd" and op[1:] in ("add", "sub", "mul", "div", "rem", "shl", "shr", "ushr", "and", "or", "xor"):
            b, a = stack.pop(), stack.pop()
            emit(_ARITH[op[0]], [a, b])
        # ---- diziler
        elif op in ("iaload", "laload", "faload", "daload", "aaload", "baload", "caload", "saload"):
            i, a = stack.pop(), stack.pop()
            ty = _elem(a.ty) if op == "aaload" else {"i": "int", "l": "long", "f": "float", "d": "double", "b": "int", "c": "int",
                                                     "s": "int"}[op[0]]
            if op == "aaload" and a.ty == "null":
                ty = "null"
            emit(ty, [a, i])
        elif op in ("iastore", "lastore", "fastore", "dastore", "aastore", "bastore", "castore", "sastore"):
            v, i, a = stack.pop(), stack.pop(), stack.pop()
            emit(None, [a, i, v])
        elif op == "arraylength":
            emit("int", [stack.pop()])
        elif op == "newarray":
            emit("[" + ins.arg, [stack.pop()], ins.arg)
        elif op == "anewarray":
            n = ins.arg.name
            emit("[" + (n if n.startswith("[") else "L" + n + ";"), [stack.pop()], ins.arg)
        elif op == "multianewarray":
            dims = [stack.pop() for _ in range(ins.arg2)][::-1]
            emit(ins.arg.name, dims, ins.arg)
        # ---- nesneler ve alanlar
        elif op == "new":
            emit("uninit@%d" % pc, [], ins.arg, cls=ins.arg.name)
        elif op == "checkcast":
            emit(ref_type(ins.arg.name), [stack.pop()], ins.arg)
        elif op == "instanceof":
            emit("int", [stack.pop()], ins.arg)
        elif op == "getstatic":
            emit(desc_type(ins.arg.desc), [], ins.arg)
        elif op == "putstatic":
            emit(None, [stack.pop()], ins.arg)
        elif op == "getfield":
            emit(desc_type(ins.arg.desc), [stack.pop()], ins.arg)
        elif op == "putfield":
            v, o = stack.pop(), stack.pop()
            emit(None, [o, v], ins.arg)
        elif op in ("monitorenter", "monitorexit"):
            emit(None, [stack.pop()])
        # ---- çağrılar
        elif op in ("invokevirtual", "invokespecial", "invokestatic", "invokeinterface"):
            ref: MemberRef = ins.arg
            ats, rt = parse_method_desc(ref.desc)
            args = [stack.pop() for _ in ats][::-1]
            recv = None
            if op != "invokestatic":
                recv = stack.pop()
                args.insert(0, recv)
            ret = None if rt == "V" else desc_type(rt)
            emit(ret, args, ref)
            if op == "invokespecial" and ref.name == "<init>" and recv is not None and recv.ty.startswith("uninit"):
                cls = recv.cls if recv.ty.startswith("uninit@") else ci.name
                recv.ty = "L" + cls + ";"
        elif op == "initclass":
            emit(None, [], ins.arg)
        elif op == "invokedynamic":
            raise MirUnsupported("invokedynamic left after desugaring")
        # ---- kontrol
        elif op in _IFS:
            n = _IF_ARGS[op]
            a = [stack.pop() for _ in range(n)][::-1]
            taken = self.edge(ins.targets[0], locs, stack)
            nxt = pc + ins.length
            fall = self.edge(nxt, locs, stack)
            blk.term = Term("if", op, a, [taken, fall], pc=pc)
            return True
        elif op == "goto":
            blk.term = Term("goto", edges=[self.edge(ins.targets[0], locs, stack)], pc=pc)
            return True
        elif op == "tableswitch":
            v = stack.pop()
            keys = [k for k, _t in ins.arg2]
            edges = [self.edge(ins.arg, locs, stack)] + [self.edge(t, locs, stack) for _k, t in ins.arg2]
            blk.term = Term("switch", args=[v], edges=edges, keys=keys, pc=pc)
            return True
        elif op in ("ireturn", "lreturn", "freturn", "dreturn", "areturn"):
            blk.term = Term("ret", args=[stack.pop()], pc=pc)
            return True
        elif op == "return":
            blk.term = Term("ret", pc=pc)
            return True
        elif op == "athrow":
            blk.term = Term("throw", args=[stack.pop()], exc=self.exc_edges(pc, locs), pc=pc)
            return True
        elif op in ("jsr", "ret", "jsr_w"):
            raise MirUnsupported(op)
        else:
            raise MirUnsupported(f"opcode {op}")
        return False


def build(ci, m, insns, watch=None) -> Fn:
    """watch: {target pc: local slots}; edges into those pcs record the Values held in the slots (Edge.watched)."""
    if m.code is None:
        raise MirUnsupported("no code")
    frs = stack_frames(ci.cf, m)
    return _B(ci, m, insns, frs, watch).build()


def successors(b: Block) -> list:
    out = [e.target for e in b.term.edges] + [e.target for e in b.term.exc]
    for i in b.insns:
        out.extend(e.target for e in i.exc)
    return out


def dominators(fn: Fn) -> dict:
    """Immediate dominator of every reachable block (Cooper, Harvey, Kennedy); the entry maps to itself."""
    order, seen = [], set()
    stack = [(fn.entry, iter(successors(fn.blocks[fn.entry])))]
    seen.add(fn.entry)
    while stack:
        b, it = stack[-1]
        nxt = next(it, None)
        if nxt is None:
            order.append(b)
            stack.pop()
        elif nxt not in seen:
            seen.add(nxt)
            stack.append((nxt, iter(successors(fn.blocks[nxt]))))
    rpo = order[::-1]
    num = {b: i for i, b in enumerate(rpo)}
    preds: dict = {b: [] for b in rpo}
    for b in rpo:
        for s in successors(fn.blocks[b]):
            preds[s].append(b)
    idom = {fn.entry: fn.entry}

    def meet(a, b):
        while a != b:
            while num[a] > num[b]:
                a = idom[a]
            while num[b] > num[a]:
                b = idom[b]
        return a

    changed = True
    while changed:
        changed = False
        for b in rpo[1:]:
            ps = [p for p in preds[b] if p in idom]
            if not ps:
                continue
            d = ps[0]
            for p in ps[1:]:
                d = meet(p, d)
            if idom.get(b) != d:
                idom[b] = d
                changed = True
    return idom


def verify(fn: Fn) -> None:
    """Structural checks: edges match target parameter lists, every use is dominated by its definition (same block: earlier;
    other block: a dominating block), every block ends in a terminator."""
    idom = dominators(fn)
    def_block: dict = {}
    for b in fn.blocks.values():
        for p in b.params:
            def_block[id(p)] = b.id
        for i in b.insns:
            if i.dst is not None:
                def_block[id(i.dst)] = b.id

    def dominates(a, b) -> bool:
        while True:
            if a == b:
                return True
            nb = idom.get(b)
            if nb is None or nb == b:
                return False
            b = nb

    for b in fn.blocks.values():
        if b.term is None:
            raise MirError(f"{fn.cls}.{fn.name}: block {b.id} has no terminator")
        defined = set(id(p) for p in b.params)

        def use(v, where):
            if v is None:
                return
            if id(v) in defined:
                return
            d = def_block.get(id(v))
            if d is None or d == b.id or not dominates(d, b.id):
                raise MirError(f"{fn.cls}.{fn.name}: {where} uses undefined {v!r} in block {b.id}")

        def check_edge(e: Edge, exc=False):
            t = fn.blocks.get(e.target)
            if t is None:
                raise MirError(f"{fn.cls}.{fn.name}: edge to unknown block {e.target}")
            if len(e.args) != len(t.params):
                raise MirError(f"{fn.cls}.{fn.name}: edge {b.id}->{e.target} passes {len(e.args)} args for {len(t.params)} params")
            for a, p in zip(e.args, t.params):
                if a is None:
                    if not exc:
                        raise MirError(f"{fn.cls}.{fn.name}: None arg on a normal edge")
                    continue
                use(a, f"edge to {e.target}")
                if (a.ty in _CAT2) != (p.ty in _CAT2) or ((a.ty in ("int", "float")) != (p.ty in ("int", "float")) and
                                                            not (a.ty.startswith(("L", "[", "null", "uninit")) and p.ty.startswith(("L", "[", "null", "uninit")))):
                    raise MirError(f"{fn.cls}.{fn.name}: edge {b.id}->{e.target} type {a.ty} -> {p.ty}")

        for i in b.insns:
            for a in i.args:
                use(a, i.op)
            for e in i.exc:
                check_edge(e, True)
            if i.dst is not None:
                defined.add(id(i.dst))
        t = b.term
        for a in t.args:
            use(a, t.kind)
        for e in t.edges:
            check_edge(e)
        for e in t.exc:
            check_edge(e, True)


def dump(fn: Fn) -> str:
    out = [f"fn {fn.cls}.{fn.name}{fn.desc}"]
    for b in sorted(fn.blocks.values(), key=lambda x: x.id):
        out.append(f"  b{b.id}({', '.join(f'{p!r}:{p.ty}' for p in b.params)}):")
        for i in b.insns:
            d = f"{i.dst!r}:{i.dst.ty} = " if i.dst is not None else ""
            at = f" [{i.attr}]" if i.attr is not None else ""
            ex = f" exc->{[e.target for e in i.exc]}" if i.exc else ""
            out.append(f"    {d}{i.op}({', '.join(repr(a) for a in i.args)}){at}{ex}")
        t = b.term
        es = ", ".join(f"b{e.target}({', '.join(repr(a) for a in e.args)})" for e in t.edges)
        out.append(f"    {t.kind} {t.op} {t.args} {es}")
    return "\n".join(out)
