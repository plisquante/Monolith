"""Rust emitter for MIR (work package C2, docs/mimari/01_DERLEYICI.md sections 3 and 9).

`MirMethodGen` is a `MethodGen` whose basic blocks come from the MIR instead of the bytecode stack simulation. Every MIR
instruction that is more than data movement is emitted by the existing `gen_insn` (same field access, calls, allocation, patches,
escape slots), so the semantics are the old path's by construction; what changes is the data flow: values are SSA variables
`v<id><kind>`, block parameters are assigned on edges (parallel assignment), local-slot and stack renaming is gone. The method
prologue, wrappers, patches and handler dispatch are shared (`MethodGen.assemble`).

Methods the MIR path cannot take raise `MirUnsupported` from the constructor, before any state of the generator is touched; the
caller falls back to `MethodGen` and counts the reason (never silent: `Gen.mir_fallback`).
"""
import bisect

from . import mir
from .rustgen import IF1, IF2, MethodGen, find_iterator_scopes

_KIND = {"int": "I", "long": "J", "float": "F", "double": "D"}


def kind(v) -> str:
    return _KIND.get(v.ty, "A")


class MirMethodGen(MethodGen):
    def __init__(self, gen, ci, m):
        super().__init__(gen, ci, m)
        # for-each iterators handed back where the loop ends (MethodGen does it at the exit block with the slot variable;
        # here every edge into the exit recycles the Value its slot holds)
        self.iter_scopes = find_iterator_scopes(self.insns)
        self.fn = mir.build(ci, m, self.insns, self.iter_scopes)
        mir.verify(self.fn)
        from . import mir_opt
        self.inlined = mir_opt.run_passes(self.fn, gen, ci)
        if self.inlined:
            mir.verify(self.fn)
        self.by_pc = {i.pc: i for i in self.insns}
        ln = sorted(m.code.line_numbers or [])
        self.line_pcs = [p for p, _l in ln]
        self.line_vals = [l for _p, l in ln]
        self.handler_blocks = {h.handler_pc for h in m.code.handlers}

    # ---- naming
    def vref(self, v) -> str:
        n = f"v{v.id}{_KIND.get(v.ty, 'A').lower()}"
        self.used_locals[n] = kind(v)
        return n

    def param_init(self, slot: int, k: str, src: str) -> str:
        entry = self.fn.blocks[self.fn.entry]
        for p, s in zip(entry.params, entry.param_slots):
            if s == slot:
                return f"{self.vref(p)} = {src};"
        return ""

    def line_for(self, pc: int):
        i = bisect.bisect_right(self.line_pcs, pc) - 1
        return self.line_vals[i] if i >= 0 else None

    # ---- assignments
    @staticmethod
    def _assign(pairs) -> str:
        pairs = [(d, s) for d, s in pairs if d != s]
        if not pairs:
            return ""
        if len(pairs) == 1:
            return f"{pairs[0][0]} = {pairs[0][1]};"
        return f"({', '.join(d for d, _ in pairs)}) = ({', '.join(s for _, s in pairs)});"

    def edge_assign(self, e: mir.Edge) -> str:
        t = self.fn.blocks[e.target]
        return self._assign([(self.vref(p), self.vref(a)) for p, a in zip(t.params, e.args) if a is not None])

    def exc_assign(self, edges, lines) -> None:
        for e in edges:
            t = self.fn.blocks[e.target]
            st = self._assign([(self.vref(p), self.vref(a)) for p, a in zip(t.params, e.args) if a is not None])
            if st:
                lines.append(st)

    # ---- blocks
    def build_blocks(self):
        handlers = self.m.code.handlers
        blocks: dict = {}
        has_sp = False
        for b in self.fn.blocks.values():
            lines: list = []
            if b.id in self.handler_blocks:
                for p, s in zip(b.params, b.param_slots):
                    if s < 0:
                        lines.append(f"{self.vref(p)} = {self.sname(0, 'A')};")
            cur_line = [None]

            def mark(pc):
                if handlers:
                    lines.append(f"pc = {pc};")
                ln = self.line_for(pc)
                if ln is not None and ln != cur_line[0]:
                    lines.append(f"rt::set_line({ln});")
                    cur_line[0] = ln

            for mi in b.insns:
                mark(mi.pc)
                self.exc_assign(mi.exc, lines)
                self.emit_minsn(mi, lines)
            t = b.term
            has_sp |= self.emit_term(b, t, lines, mark)
            blocks[b.id] = lines
        return blocks, has_sp

    def emit_minsn(self, mi, lines) -> None:
        if mi.op == "iinc":
            lines.append(f"{self.vref(mi.dst)} = {self.vref(mi.args[0])}.wrapping_add({mi.attr});")
            return
        if mi.op == "nullcheck":   # receiver of an inlined call (mir_opt.inline_leaf)
            lines.append(f"rt::nn({self.vref(mi.args[0])})?;")
            return
        if mi.op == "copy":        # a value forwarded unchanged (mir_opt)
            lines.append(f"{self.vref(mi.dst)} = {self.vref(mi.args[0])};")
            return
        stk = []
        for i, a in enumerate(mi.args):
            k = kind(a)
            stk.append(k)
            lines.append(f"{self.sname(i, k)} = {self.vref(a)};")
        ins = mi.ins if mi.ins is not None else self.by_pc[mi.pc]
        r = self.gen_insn(ins, stk, lines, set())
        if r is not None:
            raise mir.MirError(f"{self.where}@{mi.pc}: {mi.op} returned successors")
        if mi.dst is not None:
            if len(stk) != 1 or stk[0] != kind(mi.dst):
                raise mir.MirError(f"{self.where}@{mi.pc}: {mi.op} left stack {stk}, wanted {kind(mi.dst)}")
            lines.append(f"{self.vref(mi.dst)} = {self.sname(0, stk[0])};")
        elif stk:
            raise mir.MirError(f"{self.where}@{mi.pc}: {mi.op} left stack {stk}")

    def emit_term(self, b, t, lines, mark) -> bool:
        sp = False
        back = any(e.target <= t.pc for e in t.edges)
        if back:
            sp = True
            lines.append("_sp_tick = _sp_tick.wrapping_add(1); if (_sp_tick & 1023) == 0 { rt::gc_safepoint(); }")

        def go(e) -> str:
            a = self.edge_assign(e)
            rec = ""
            if e.watched:
                for sl in self.iter_scopes.get(e.target, ()):
                    v = e.watched.get(sl)
                    if v is not None and kind(v) == "A":
                        rec += f"rt::recycle_scope({self.vref(v)}); "
            return f"{rec}{a} pc = {e.target}; continue;".strip()

        if t.kind == "ret":
            mark(t.pc)
            lines.append(f"return Ok({self.vref(t.args[0])});" if t.args else "return Ok(());")
        elif t.kind == "throw":
            mark(t.pc)
            self.exc_assign(t.exc, lines)
            lines.append(f"return rt::throw_ref({self.vref(t.args[0])});")
        elif t.kind == "goto":
            lines.append(go(t.edges[0]))
        elif t.kind == "if":
            a = [self.vref(x) for x in t.args]
            cond = IF1[t.op].format(a=a[0]) if len(a) == 1 else IF2[t.op].format(a=a[0], b=a[1])
            lines.append(f"if {cond} {{ {go(t.edges[0])} }}")
            lines.append(go(t.edges[1]))
        else:  # switch
            x = self.vref(t.args[0])
            lines.append(f"match {x} {{")
            for k, e in zip(t.keys, t.edges[1:]):
                lines.append(f"    {k} => {{ {go(e)} }}")
            lines.append(f"    _ => {{ {go(t.edges[0])} }}")
            lines.append("}")
        return sp
