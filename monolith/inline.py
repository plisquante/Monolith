"""Inlining of small methods into their callers, on the decoded bytecode, before the escape analysis.

Why: `a.subtract(b)` allocates a Vec3 inside `subtract` and hands it to the caller, so the caller's object is "returned
from a call" and cannot be proven to stay in the caller. With the callee's body in the caller the `new` is the caller's
own allocation site and the escape analysis (and the stack allocation that follows from it) can see everything.

What is inlined: a call whose single target is known (static, private, final, constructor, or one implementation by
class hierarchy analysis) and whose body is small, straight-line enough and has no exception handlers, no monitors, no
loops, no `super` calls. Constructors are inlined too, which exposes the field stores of `new Vec3(x, y, z)`.

How: the caller's pcs are scaled by K so every inlined instruction gets a pc between the call and the next
instruction (exception ranges, branch targets and line numbers stay consistent). Arguments are popped into fresh
locals, the callee's locals are shifted behind them, `return` becomes a jump to the instruction after the call. Two
pseudo instructions keep the semantics of a call that is no longer there: `nullchk` (NullPointerException for a null
receiver) and `initclass` (class initialization before a static call).

Not inlined: patched methods (the patch must keep running), caller-sensitive methods, `<clinit>`, recursion.
"""
from __future__ import annotations

import dataclasses
from dataclasses import replace

from .bytecode import Insn
from .world import parse_method_desc

K = 256                      # pc scale
MAX_CALLEE = 48              # instructions of a callee (after its own inlining)
MAX_CALLER = 1400            # instructions a caller may grow to
MAX_DEPTH = 3

RETURNS = ("ireturn", "lreturn", "freturn", "dreturn", "areturn", "return")
STORE = {"I": "istore", "J": "lstore", "F": "fstore", "D": "dstore", "A": "astore"}
LOCAL_OPS = {"iload", "lload", "fload", "dload", "aload", "istore", "lstore", "fstore", "dstore", "astore", "iinc", "ret"}


def _kind(d: str) -> str:
    return "A" if d[0] in "L[" else ("I" if d in "ZBCSI" else d)


def _width(d: str) -> int:
    return 2 if d in ("J", "D") else 1


class Inliner:
    def __init__(self, prog, patches=None, caller_sensitive=None, keep_ctors=()):
        self.p = prog
        self.keep_ctors = frozenset(keep_ctors)   # classes whose constructor call must stay (the recycling matches `new C; dup; ..; invokespecial C.<init>`)
        self.w = prog.w
        self.patches = patches
        self.caller_sensitive = caller_sensitive or (lambda dc, m: False)
        self.done: dict[tuple[str, str], list[Insn]] = {}     # (class, key) -> final instruction list
        self.busy: set[tuple[str, str]] = set()
        self.stats = {"methods": 0, "sites": 0, "bytes": 0}
        self._cha: dict = {}

    # ------------------------------------------------------------------ target resolution
    def _single_target(self, ci, ins: Insn):
        a = ins.arg
        op = ins.op
        if op == "invokedynamic":
            return None
        owner = a.owner
        if owner.startswith("["):
            return None
        r = None
        try:
            if op == "invokestatic":
                r = self.p.resolve(a)
            elif op == "invokespecial":
                if a.name != "<init>":
                    m0 = self.p.resolve_special(ci, a)
                    if m0 is None or not m0[1].is_private:
                        return None        # super.foo(): resolved relative to the class of the instruction
                    return m0
                r = self.p.resolve_special(ci, a)
            elif op in ("invokevirtual", "invokeinterface"):
                r = self.p.resolve(a)
                if r is None:
                    return None
                dc, m = r
                if m.is_private or (m.access & 0x0010) or (dc.cf.access & 0x0010 and not dc.is_interface):
                    return r
                return self._cha_single(owner, a.name, a.desc)
        except Exception:
            return None
        return r

    def _cha_single(self, owner: str, name: str, desc: str):
        """The one implementation reachable by a virtual call on `owner`, or None."""
        k = (owner, name, desc)
        if k in self._cha:
            return self._cha[k]
        res = None
        oc = self.w.classes.get(owner)
        if oc is not None and owner != "java/lang/Object":
            impls = {}
            seen = set()
            stack = [oc]
            ok = True
            while stack and ok:
                c = stack.pop()
                if c.name in seen:
                    continue
                seen.add(c.name)
                if c.name in self.p.instantiated:
                    sel = self.w.select(c, name, desc)
                    if sel is None:
                        ok = False
                        break
                    impls[(sel[0].name, sel[1].key)] = sel
                    if len(impls) > 1:
                        ok = False
                        break
                stack.extend(c.subclasses)
            if ok and len(impls) == 1:
                res = next(iter(impls.values()))
        self._cha[k] = res
        return res

    # ------------------------------------------------------------------ eligibility
    def _inlinable(self, dc, m, caller_key) -> list[Insn] | None:
        """The callee's instructions when `m` can be inlined, else None."""
        key = (dc.name, m.key)
        if key == caller_key or key in self.busy:
            return None
        if m.code is None or m.is_native or m.is_abstract or m.name == "<clinit>":
            return None
        if m.access & 0x0020 or m.code.handlers:            # synchronized, exception handlers
            return None
        if self.patches is not None and (dc.name, m.name, m.desc) in self.patches:
            return None
        if self.caller_sensitive(dc, m):
            return None
        insns = self.final_insns(dc, m)
        if len(insns) > MAX_CALLEE:
            return None
        for x in insns:
            if x.op in ("monitorenter", "monitorexit", "jsr", "jsr_w", "ret", "tableswitch", "lookupswitch"):
                return None
            if x.targets and any(t <= x.pc for t in x.targets):
                return None                                   # a loop
            if x.op == "invokespecial" and x.arg.name != "<init>":
                try:
                    r = self.p.resolve_special(dc, x.arg)
                except Exception:
                    return None
                if r is None or not r[1].is_private:
                    return None                               # super call
        return insns

    # ------------------------------------------------------------------ the transformation
    def final_insns(self, ci, m) -> list[Insn]:
        key = (ci.name, m.key)
        if key in self.done:
            return self.done[key]
        orig = self.p.insns(ci, m)
        if m.code is None or len(self.busy) >= MAX_DEPTH:
            self.done[key] = orig
            return orig
        self.busy.add(key)
        try:
            res = self._transform(ci, m, orig, key)
        finally:
            self.busy.discard(key)
        self.done[key] = res
        return res

    def _transform(self, ci, m, orig: list[Insn], key) -> list[Insn]:
        code = m.code
        out: list[Insn] = []
        next_local = code.max_locals + 2
        changed = False
        n = len(orig)
        # the pc of the instruction after each one (the target of an inlined `return`)
        nxt = {orig[i].pc: (orig[i + 1].pc if i + 1 < n else orig[i].pc + 1) for i in range(n)}
        for i, ins in enumerate(orig):
            base = ins.pc * K
            tgt = self._single_target(ci, ins) if ins.op in ("invokestatic", "invokespecial", "invokevirtual", "invokeinterface") else None
            if tgt is not None and ins.arg.name == "<init>" and ins.arg.owner in self.keep_ctors:
                tgt = None
            if tgt is not None and ins.arg.name in ("iterator", "listIterator") and ins.arg.desc.endswith(("Ljava/util/Iterator;", "Ljava/util/ListIterator;")):
                tgt = None      # stays a call: the for-each recycling (rustgen.find_iterator_scopes) matches `iterator()` + `astore`
            body = None
            if tgt is not None and len(out) + len(orig) < MAX_CALLER:
                dc, cm = tgt
                if cm.code is not None and not (ins.op == "invokestatic" and cm.is_static is False):
                    body = self._inlinable(dc, cm, key)
            if body is None:
                out.append(self._scale(ins))
                continue
            dc, cm = tgt
            args, ret = parse_method_desc(cm.desc)
            slots = []                      # (slot, descriptor) of receiver and parameters, in order
            s = next_local
            if not cm.is_static:
                slots.append((s, "Ljava/lang/Object;"))
                s += 1
            for d in args:
                slots.append((s, d))
                s += _width(d)
            shift = next_local
            maxloc = max((x.arg for x in body if x.op in LOCAL_OPS and isinstance(x.arg, int)), default=-1)
            next_local = shift + max(maxloc + 2, s - shift) + 1
            end_pc = nxt[ins.pc] * K
            seq: list[Insn] = []
            for slot, d in reversed(slots):
                seq.append(Insn(0, STORE[_kind(d)], slot))
            if not cm.is_static and cm.name != "<init>":
                seq.append(Insn(0, "aload", slots[0][0]))
                seq.append(Insn(0, "nullchk"))
            if cm.is_static and self._needs_init(dc):
                seq.append(Insn(0, "initclass", dc.name))
            pcmap: dict[int, int] = {}
            first_body = len(seq)
            for x in body:
                seq.append(None)            # placeholder, filled below once pcs are known
            # pcs: sequential after the call's own scaled pc
            pcs = [base + j for j in range(len(seq))]
            for j in range(first_body):
                seq[j] = replace(seq[j], pc=pcs[j])
            for j, x in enumerate(body):
                pcmap[x.pc] = pcs[first_body + j]
            for j, x in enumerate(body):
                npc = pcs[first_body + j]
                if x.op in RETURNS:
                    if j == len(body) - 1:
                        seq[first_body + j] = Insn(npc, "nop")
                    else:
                        seq[first_body + j] = Insn(npc, "goto", None, None, (end_pc,))
                    continue
                y = replace(x, pc=npc)
                if y.op in LOCAL_OPS and isinstance(y.arg, int):
                    y = replace(y, arg=y.arg + shift)
                if x.targets:
                    y = replace(y, targets=tuple(pcmap[t] for t in x.targets))
                seq[first_body + j] = y
            out.extend(seq)
            changed = True
            self.stats["sites"] += 1
        if not changed:
            return orig
        # branch targets of the untouched instructions were scaled in _scale; instructions inlined above carry their
        # own remapped targets. The caller's code attribute follows the new numbering.
        new_code = replace(code, handlers=[replace(h, start_pc=h.start_pc * K, end_pc=h.end_pc * K, handler_pc=h.handler_pc * K) for h in code.handlers],
                           line_numbers=[(pc * K, ln) for pc, ln in code.line_numbers], max_locals=next_local + 2)
        m.insns = out
        m.code = new_code
        self.p._insns[(ci.name, m.key)] = out
        self.stats["methods"] += 1
        return out

    def _scale(self, ins: Insn) -> Insn:
        return replace(ins, pc=ins.pc * K, targets=tuple(t * K for t in ins.targets),
                       arg=(self._scale_switch_default(ins) if ins.op in ("tableswitch", "lookupswitch") else ins.arg),
                       arg2=(self._scale_switch(ins) if ins.op in ("tableswitch", "lookupswitch") else ins.arg2))

    @staticmethod
    def _scale_switch_default(ins: Insn):
        return ins.arg * K

    @staticmethod
    def _scale_switch(ins: Insn):
        return [(k, t * K) for k, t in ins.arg2]

    def _needs_init(self, dc) -> bool:
        return dc.cf.method("<clinit>", "()V") is not None or any(f.is_static and f.constant_value is not None for f in dc.cf.fields)


def inline_program(prog, patches=None, caller_sensitive=None, keep_ctors=()) -> dict:
    """Inlines small callees into every reachable method. Returns statistics."""
    inl = Inliner(prog, patches, caller_sensitive, keep_ctors)
    for cn, key in sorted(prog.reachable):
        ci = prog.w.classes.get(cn)
        if ci is None:
            continue
        name, desc = key.split("(")[0], "(" + key.split("(", 1)[1]
        m = ci.cf.method(name, desc)
        if m is None or m.code is None or m.name == "<clinit>":
            continue
        inl.final_insns(ci, m)
    return inl.stats
