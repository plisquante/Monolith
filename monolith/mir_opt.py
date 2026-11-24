"""MIR passes (work package C4, docs/mimari/01_DERLEYICI.md section 6).

inline_leaf: a call whose target is known at compile time (static, special, final/private, or a single RTA receiver) and whose
body cannot throw once its receiver is known to be non-null (no calls, no allocation, no static fields, field access only on
`this`, no loops) is replaced by the callee's blocks. This is what LLVM cannot do across classes: such calls go through an
`extern "C"` wrapper in another shard (docs/mimari/00_ANA_MIMARI.md AB3). The receiver null check stays at the call site
(`nullcheck`), which is where HotSpot reports it; nothing inside the inlined code can throw, so stack traces are unchanged.

Disabled with MONO_MIR_NO=inline (comma-separated pass names).
"""
import os

from . import mir
from .bytecode import Insn

MAX_INSNS = 24
_INLINE_ID = 1 << 24        # cloned block ids start here (above any bytecode pc, below u32 max)
_LEAF_OPS = mir._NOTHROW - {"goto", "return", "ireturn", "lreturn", "freturn", "dreturn", "areturn"}


def _disabled() -> set:
    return {x.strip() for x in os.environ.get("MONO_MIR_NO", "").split(",") if x.strip()}


def run_passes(fn: mir.Fn, gen, ci) -> int:
    off = _disabled()
    n = 0
    if "inline" not in off:
        n += inline_leaf(fn, gen, ci)
    return n


def _target(gen, ci, mi):
    """(class, method) the call surely reaches, or None."""
    a = mi.attr
    p = gen.prog
    if mi.op == "invokestatic":
        r = p.resolve(a)
        return r if r is not None and r[1].is_static else None
    if a.owner.startswith("["):
        return None
    if mi.op == "invokespecial":
        if a.name == "<init>":
            return None
        return p.resolve_special(ci, a)
    r = p.resolve(a)
    if r is None:
        return None
    dc, m = r
    if mi.op == "invokevirtual" and (m.is_private or (m.access & 0x0010) or (dc.cf.access & 0x0010)):
        return r
    owner = gen.world.get(a.owner)
    if owner is None:
        return None
    dv = gen.devirt_targets(owner, m.name, m.desc)
    if dv is not None and dv[0] == "single":
        return dv[1]
    return None


_leaf_cache: dict = {}


def _leaf_body(gen, dc, m):
    """MIR of the callee when it is an inlinable leaf, else None."""
    k = (dc.name, m.name, m.desc)
    if k in _leaf_cache and _leaf_cache[k][0] is gen:
        return _leaf_cache[k][1]
    res = None
    if (not m.is_native and m.code is not None and not m.code.handlers and not (m.access & 0x0020)
            and k not in gen.patches and not gen.caller_sensitive(dc, m)):
        try:
            cf = mir.build(dc, m, gen.prog.insns(dc, m))
        except mir.MirError:
            cf = None
        if cf is not None:
            this = None
            if not cf.static:
                entry = cf.blocks[cf.entry]
                this = next((p for p, s in zip(entry.params, entry.param_slots) if s == 0), None)
            n = 0
            ok = True
            for b in cf.blocks.values():
                for i in b.insns:
                    n += 1
                    if i.op in _LEAF_OPS:
                        continue
                    if i.op in ("getfield", "putfield") and this is not None and i.args[0] is this:
                        continue
                    ok = False
                    break
                t = b.term
                if not ok or t.kind in ("throw", "switch") or any(e.target <= t.pc for e in t.edges):
                    ok = False
                if not ok:
                    break
            if ok and n <= MAX_INSNS:
                res = cf
    _leaf_cache[k] = (gen, res)
    return res


def inline_leaf(fn: mir.Fn, gen, ci) -> int:
    count = 0
    next_id = [max(_INLINE_ID, max(fn.blocks) + 1)]
    work = list(fn.blocks.values())
    while work:
        b = work.pop()
        for k, mi in enumerate(b.insns):
            if mi.op not in ("invokestatic", "invokespecial", "invokevirtual", "invokeinterface"):
                continue
            try:
                tgt = _target(gen, ci, mi)
            except Exception:  # noqa: BLE001  (link errors: leave the call to gen_insn, which reports them)
                tgt = None
            if tgt is None or (tgt[0].name == fn.cls and tgt[1].name == fn.name and tgt[1].desc == fn.desc):
                continue
            body = _leaf_body(gen, *tgt)
            if body is None:
                continue
            cont = _splice(fn, b, k, mi, body, tgt, gen, ci, next_id)
            count += 1
            work.append(cont)
            break
    return count


def _splice(fn, b, k, mi, body, tgt, gen, ci, next_id):
    """Replace call `mi` (b.insns[k]) by a copy of `body`; returns the continuation block (rest of b)."""
    dc = tgt[0]

    def new_id():
        i = next_id[0]
        next_id[0] += 1
        return i

    def new_val(v):
        nv = mir.Value(fn.nvalues, v.ty, v.cls)
        fn.nvalues += 1
        return nv

    # continuation: the rest of b, entered with the call's result
    cont_id = new_id()
    cparams = [new_val(mi.dst)] if mi.dst is not None else []
    cont = mir.Block(cont_id, cparams, [-1] * len(cparams), b.insns[k + 1:], b.term)
    if mi.dst is not None:
        # uses of the call result after the call now read the continuation parameter
        _rename(cont, {id(mi.dst): cparams[0]})
        # and every later block that used it (dominated by b): keep the old Value defined by a copy
        cont.insns.insert(0, mir.MInsn("copy", mi.dst, [cparams[0]], None, mi.pc, [], None))
    fn.blocks[cont_id] = cont

    pre = b.insns[:k]
    if mi.op == "invokestatic":
        if dc.name != fn.cls and gen.needs_init(dc):
            pre.append(mir.MInsn("initclass", None, [], dc.name, mi.pc, mi.exc, Insn(mi.pc, "initclass", dc.name)))
    else:
        pre.append(mir.MInsn("nullcheck", None, [mi.args[0]], None, mi.pc, mi.exc, None))

    # clone the callee
    vmap: dict = {}
    bmap = {bid: new_id() for bid in body.blocks}

    def v(x):
        if x is None:
            return None
        y = vmap.get(id(x))
        if y is None:
            y = new_val(x)
            vmap[id(x)] = y
        return y

    for cb in body.blocks.values():
        nb = mir.Block(bmap[cb.id], [v(p) for p in cb.params], list(cb.param_slots))
        for i in cb.insns:
            nb.insns.append(mir.MInsn(i.op, v(i.dst), [v(a) for a in i.args], i.attr, mi.pc, [], i.ins))
        t = cb.term
        if t.kind == "ret":
            nb.term = mir.Term("goto", edges=[mir.Edge(cont_id, [v(a) for a in t.args])], pc=mi.pc)
        else:
            nb.term = mir.Term(t.kind, t.op, [v(a) for a in t.args],
                               [mir.Edge(bmap[e.target], [v(a) for a in e.args]) for e in t.edges], list(t.keys), [], mi.pc)
        fn.blocks[nb.id] = nb
    b.insns = pre
    b.term = mir.Term("goto", edges=[mir.Edge(bmap[body.entry], list(mi.args))], pc=mi.pc)
    return cont


def _rename(block, mp):
    for i in block.insns:
        i.args = [mp.get(id(a), a) if a is not None else None for a in i.args]
        for e in i.exc:
            e.args = [mp.get(id(a), a) if a is not None else None for a in e.args]
    t = block.term
    t.args = [mp.get(id(a), a) for a in t.args]
    for e in t.edges + t.exc:
        e.args = [mp.get(id(a), a) if a is not None else None for a in e.args]
