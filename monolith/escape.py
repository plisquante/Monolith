"""Whole-program escape analysis over the reachable program (closed world).

For every reachable method with code it computes
  * a summary: for each reference parameter (receiver first) whether the callee may let it escape, and
  * for every allocation site of the method whether the object can outlive the method invocation.

An object escapes when it is stored into a field, a static, or an array element, is returned or thrown,
is a monitor, is passed to a call whose target may let it escape (or to an unresolvable call), or is
captured by an invokedynamic. Everything the analysis cannot see is treated as an escape, so a site that
is reported as not escaping is safe to allocate in the caller's scope (stack or arena).

The summaries start optimistic (nothing escapes) and only ever grow, so a worklist over the call graph
reaches the least fixpoint. If the time budget runs out the whole result is discarded (never partial),
because optimistic summaries of unfinished methods would be wrong.

Entry points: `analyze(prog, budget_s)` and `write_report(result, path)`.
"""
from __future__ import annotations

import collections
import json
import os
import time
from dataclasses import dataclass, field
from typing import Optional

from .bytecode import Insn
from .classfile import MemberRef
from .world import parse_method_desc

EMPTY: frozenset = frozenset()

# natives and library methods that are known not to let their reference arguments escape
NOESCAPE_NATIVES = {
    ("java/lang/System", "arraycopy(Ljava/lang/Object;ILjava/lang/Object;II)V"),
    ("java/lang/Object", "hashCode()I"),
    ("java/lang/Object", "getClass()Ljava/lang/Class;"),
}

MAX_TARGETS = 6          # virtual call sites with more possible targets count as unresolved
CALLEE_DEPTH_NONE = 0


class AnalysisTimeout(Exception):
    pass


@dataclass
class MethodResult:
    params: tuple                     # per reference parameter (receiver first): True = may escape
    sites: dict                       # alloc pc -> kind ("new", "newarray", "anewarray", "multianewarray")
    escaped: dict                     # alloc pc -> reason it escapes (absent = does not escape)
    loop_pcs: frozenset               # alloc pcs that lie inside a loop of the method
    callees: frozenset                # (class, key) pairs consulted for this result
    ret_fresh: bool = False           # every returned reference is a fresh object that nothing else holds (ownership moves to the caller)
    rsites: frozenset = EMPTY         # call pcs whose returned object is a fresh object of the callee (a pseudo allocation site here)
    rescaped: dict = field(default_factory=dict)   # rsite pc -> reason the returned object escapes the caller
    rloop_pcs: frozenset = EMPTY      # rsite pcs inside a loop
    rtargets: dict = field(default_factory=dict)   # rsite pc -> [(class, key)] the call may reach
    cparams: tuple = ()               # per reference parameter: True = something read out of the object may escape
    iter_pcs: frozenset = EMPTY       # loop alloc pcs whose object never survives one iteration of any loop around it


@dataclass
class Result:
    methods: dict = field(default_factory=dict)       # (class, key) -> MethodResult
    seconds: float = 0.0
    passes: int = 0

    def noescape_sites(self, cn: str, key: str) -> frozenset:
        """Allocation pcs of the method that never outlive one invocation and are not in a loop."""
        r = self.methods.get((cn, key))
        if r is None:
            return EMPTY
        return frozenset(pc for pc in r.sites if pc not in r.escaped and (pc not in r.loop_pcs or pc in r.iter_pcs))


def _ref_params(m, is_static: bool) -> list[tuple[int, int]]:
    """[(local slot, reference ordinal)] for every reference parameter, receiver first."""
    out: list[tuple[int, int]] = []
    slot = 0
    ordinal = 0
    if not is_static:
        out.append((0, 0))
        slot = 1
        ordinal = 1
    args, _ = parse_method_desc(m.desc)
    for d in args:
        if d[0] in ("L", "["):
            out.append((slot, ordinal))
            ordinal += 1
        slot += 2 if d in ("J", "D") else 1
    return out


class EscapeAnalysis:
    def __init__(self, prog, budget_s: float = 1500.0):
        from .zigen import ARITH, CONV, IF1, IF2, INTRINSICS
        self.p = prog
        self.w = prog.w
        self.ARITH, self.CONV, self.IF1, self.IF2, self.INTRINSICS = ARITH, CONV, IF1, IF2, INTRINSICS
        self.deadline = time.time() + budget_s
        self.summary: dict[tuple[str, str], tuple] = {}
        self.ret_fresh: dict[tuple[str, str], bool] = {}
        self.csummary: dict[tuple[str, str], tuple] = {}
        self.res: dict[tuple[str, str], MethodResult] = {}
        self.callers: dict[tuple[str, str], set] = collections.defaultdict(set)
        self._methods: dict[tuple[str, str], tuple] = {}
        self._targets_cache: dict[tuple[str, str, str], Optional[list]] = {}

    # ------------------------------------------------------------------ setup
    def _collect(self) -> None:
        for cn, key in self.p.reachable:
            ci = self.w.classes.get(cn)
            if ci is None:
                continue
            m = ci.cf.method(key.split("(")[0], "(" + key.split("(", 1)[1])
            if m is None or m.code is None or m.name == "<clinit>":
                continue
            self._methods[(cn, key)] = (ci, m)
            self.summary[(cn, key)] = tuple(False for _ in _ref_params(m, m.is_static))
            self.csummary[(cn, key)] = tuple(False for _ in _ref_params(m, m.is_static))
            self.ret_fresh[(cn, key)] = m.desc.rpartition(")")[2][:1] in ("L", "[")   # optimistic; only ever turns False

    # ------------------------------------------------------------------ call targets
    def _cha_targets(self, owner: str, name: str, desc: str) -> Optional[list]:
        """All implementations an invokevirtual/invokeinterface on `owner` can reach, or None if unbounded."""
        k = (owner, name, desc)
        if k in self._targets_cache:
            return self._targets_cache[k]
        oc = self.w.classes.get(owner)
        res: Optional[list] = None
        if oc is not None:
            impls: dict[tuple[str, str], tuple] = {}
            seen: set[str] = set()
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
                    if len(impls) > MAX_TARGETS:
                        ok = False
                        break
                stack.extend(c.subclasses)
            if ok and impls:
                res = list(impls.values())
        self._targets_cache[k] = res
        return res

    def _call_targets(self, ci, op: str, a: MemberRef) -> Optional[list]:
        """[(ClassInfo, MethodInfo)] the call may reach, or None when unresolvable."""
        if op == "invokestatic":
            r = self.p.resolve(a)
            return [r] if r else None
        if op == "invokespecial":
            r = self.p.resolve_special(ci, a)
            return [r] if r else None
        r = self.p.resolve(a)
        if r is None:
            return None
        dc, m = r
        if m.is_private or (m.access & 0x0010) or (dc.cf.access & 0x0010 and not dc.is_interface):
            return [r]
        owner = a.owner if not a.owner.startswith("[") else "java/lang/Object"
        if owner == "java/lang/Object":
            return None
        return self._cha_targets(owner, a.name, a.desc)

    def _target_summary(self, dc, m) -> tuple:
        key = (dc.name, m.key)
        s = self.summary.get(key)
        if s is not None:
            return s
        n = len(_ref_params(m, m.is_static))
        if (dc.name, m.key) in NOESCAPE_NATIVES:
            return tuple(False for _ in range(n))
        if dc.name == "java/lang/Object" and m.name == "<init>":
            return (False,)
        return tuple(True for _ in range(n))     # native, abstract, no code: unknown

    def _target_csummary(self, dc, m) -> tuple:
        cs = self.csummary.get((dc.name, m.key))
        if cs is not None:
            return cs
        n = len(_ref_params(m, m.is_static))
        if (dc.name, m.key) in NOESCAPE_NATIVES:
            # arraycopy moves elements from one array into the other: what is inside both may flow anywhere
            return tuple(m.name == "arraycopy" for _ in range(n))
        if dc.name == "java/lang/Object" and m.name == "<init>":
            return (False,)
        return tuple(True for _ in range(n))

    # ------------------------------------------------------------------ one method
    def _analyze(self, ci, m) -> Optional[MethodResult]:
        insns: list[Insn] = self.p.insns(ci, m)
        code = m.code
        if not insns:
            return MethodResult(tuple(False for _ in _ref_params(m, m.is_static)), {}, {}, EMPTY, EMPTY)
        idx = {x.pc: i for i, x in enumerate(insns)}
        refp = _ref_params(m, m.is_static)
        nparams = len(refp)
        nloc = max(code.max_locals, 1) + 2

        # leaders, loops, allocation sites
        leaders = {0}
        loops: list[tuple[int, int]] = []
        sites: dict[int, str] = {}
        for i, x in enumerate(insns):
            if x.op in ("new", "newarray", "anewarray", "multianewarray"):
                sites[x.pc] = x.op
            if x.targets:
                for t in x.targets:
                    leaders.add(t)
                    if t <= x.pc:
                        loops.append((t, x.pc))
                if i + 1 < len(insns):
                    leaders.add(insns[i + 1].pc)
            elif x.op in ("ireturn", "lreturn", "freturn", "dreturn", "areturn", "return", "athrow") and i + 1 < len(insns):
                leaders.add(insns[i + 1].pc)
        for h in code.handlers:
            leaders.add(h.handler_pc)
        loop_pcs = frozenset(pc for pc in sites if any(a <= pc <= b for a, b in loops))

        escaped: dict = {}                      # origin -> first reason
        reasons_of: dict = collections.defaultdict(set)   # origin -> every reason
        ret_vals: list = []                     # origin sets of every areturn operand
        contents: dict = collections.defaultdict(set)    # tracked owner origin -> origins stored into it
        rsites: dict = {}                       # call pc -> [(class, key)] for calls returning a fresh callee object
        callees: set = set()
        init_locs = [EMPTY] * nloc
        for slot, ordn in refp:
            init_locs[slot] = frozenset((("P", ordn),))
        in_states: dict[int, tuple[list, list]] = {0: (init_locs, [])}
        # handlers may start with any local holding any origin seen in the method (sound over-approximation)
        if code.handlers:
            every = frozenset([("A", pc) for pc in sites] + [("P", o) for _, o in refp])
            for h in code.handlers:
                in_states[h.handler_pc] = ([every] * nloc, [(EMPTY, False)])
        work = list(in_states.keys())
        steps = 0
        budget = len(insns) * 24 + 256

        def esc(origins, why: str) -> None:
            for o in origins:
                reasons_of[o].add(why)
                if o not in escaped:
                    escaped[o] = why

        def roots_of(origins):
            """Tracked owners an object read out of `origins` can belong to: A/R sites, or ('PC', i) for a parameter's content."""
            out = set()
            for o in origins:
                if o[0] in ("A", "R"):
                    out.add(o)
                elif o[0] == "C":
                    out.add(o[1])
                elif o[0] == "P":
                    out.add(("PC", o[1]))
                elif o[0] == "PC":
                    out.add(o)
            return out

        def store_into(owners, vals, why: str) -> None:
            """vals stored into the field/element of an object whose origins are `owners`."""
            if not vals:
                return
            tracked = [o for o in owners if o[0] in ("A", "R", "C")]
            if not owners or len(tracked) != len(owners):
                esc(vals, why)          # owner unknown or a parameter's/other object: the value may outlive this call
            for o in tracked:
                contents[o if o[0] != "C" else o[1]].update(vals)

        def content_esc(vals, why: str) -> None:
            """Something read out of the objects in `vals` escapes."""
            for r in roots_of(vals):
                esc(frozenset((("C", r),)) if r[0] != "PC" else frozenset((r,)), why)

        def run_block(bpc: int):
            locs, stk = in_states[bpc]
            locs = list(locs)
            stk = list(stk)
            i = idx[bpc]
            while True:
                ins = insns[i]
                op, a = ins.op, ins.arg
                succs = None
                if op in ("nop", "iinc", "checkcast"):
                    pass
                elif op == "aconst_null" or op.startswith(("iconst_", "fconst_")) or op in ("bipush", "sipush"):
                    stk.append((EMPTY, False))
                elif op.startswith(("lconst_", "dconst_")):
                    stk.append((EMPTY, True))
                elif op == "ldc":
                    is2 = isinstance(a, tuple) and a[0] in ("long", "double")
                    stk.append((EMPTY, is2))
                elif op in ("iload", "fload"):
                    stk.append((EMPTY, False))
                elif op in ("lload", "dload"):
                    stk.append((EMPTY, True))
                elif op == "aload":
                    stk.append((locs[a] if a < len(locs) else EMPTY, False))
                elif op == "astore":
                    v = stk.pop()
                    while a >= len(locs):
                        locs.append(EMPTY)
                    locs[a] = v[0]
                elif op in ("istore", "lstore", "fstore", "dstore"):
                    stk.pop()
                    while a >= len(locs):
                        locs.append(EMPTY)
                    locs[a] = EMPTY
                elif op == "aaload":
                    stk.pop()
                    arr = stk.pop()
                    stk.append((frozenset(("C", r) if r[0] != "PC" else r for r in roots_of(arr[0])), False))
                elif op in ("iaload", "faload", "baload", "caload", "saload"):
                    stk.pop(); stk.pop(); stk.append((EMPTY, False))
                elif op in ("laload", "daload"):
                    stk.pop(); stk.pop(); stk.append((EMPTY, True))
                elif op in ("iastore", "lastore", "fastore", "dastore", "bastore", "castore", "sastore"):
                    stk.pop(); stk.pop(); stk.pop()
                elif op == "aastore":
                    val = stk.pop(); stk.pop(); arr = stk.pop()
                    store_into(arr[0], val[0], "aastore")
                elif op in ("arraylength", "instanceof"):
                    stk.pop(); stk.append((EMPTY, False))
                elif op == "pop":
                    stk.pop()
                elif op == "pop2":
                    top = stk.pop()
                    if not top[1]:
                        stk.pop()
                elif op in ("dup", "dup_x1", "dup_x2", "dup2", "dup2_x1", "dup2_x2", "swap"):
                    c2 = lambda item: item[1]
                    if op == "dup": nin, perm = 1, (0, 0)
                    elif op == "dup_x1": nin, perm = 2, (1, 0, 1)
                    elif op == "dup_x2": nin, perm = (2, (1, 0, 1)) if c2(stk[-2]) else (3, (2, 0, 1, 2))
                    elif op == "dup2": nin, perm = (1, (0, 0)) if c2(stk[-1]) else (2, (0, 1, 0, 1))
                    elif op == "dup2_x1": nin, perm = (2, (1, 0, 1)) if c2(stk[-1]) else (3, (1, 2, 0, 1, 2))
                    elif op == "dup2_x2":
                        if c2(stk[-1]):
                            nin, perm = (2, (1, 0, 1)) if c2(stk[-2]) else (3, (2, 0, 1, 2))
                        else:
                            nin, perm = (3, (1, 2, 0, 1, 2)) if c2(stk[-3]) else (4, (2, 3, 0, 1, 2, 3))
                    else:
                        nin, perm = 2, (1, 0)
                    seg = stk[-nin:]
                    del stk[-nin:]
                    for p_i in perm:
                        stk.append(seg[p_i])
                elif op in self.ARITH:
                    k, _, _, n = self.ARITH[op]
                    for _ in range(n):
                        stk.pop()
                    stk.append((EMPTY, k in ("J", "D")))
                elif op in self.CONV:
                    k, _ = self.CONV[op]
                    stk.pop()
                    stk.append((EMPTY, k in ("J", "D")))
                elif op in self.IF1 or op in self.IF2:
                    stk.pop()
                    if op in self.IF2:
                        stk.pop()
                    succs = [ins.targets[0], insns[i + 1].pc]
                elif op == "goto":
                    succs = [ins.targets[0]]
                elif op == "tableswitch":
                    stk.pop()
                    succs = list(dict.fromkeys(ins.targets))
                elif op in ("ireturn", "lreturn", "freturn", "dreturn", "return"):
                    return []
                elif op == "areturn":
                    v = stk.pop()
                    esc(v[0], "areturn")
                    ret_vals.append(v[0])
                    return []
                elif op == "athrow":
                    v = stk.pop()
                    esc(v[0], "athrow")
                    return []
                elif op == "getstatic":
                    stk.append((EMPTY, a.desc in ("J", "D")))
                elif op == "putstatic":
                    v = stk.pop()
                    esc(v[0], "putstatic")
                elif op == "getfield":
                    obj = stk.pop()
                    if a.desc[0] in ("L", "["):
                        stk.append((frozenset(("C", r) if r[0] != "PC" else r for r in roots_of(obj[0])), False))
                    else:
                        stk.append((EMPTY, a.desc in ("J", "D")))
                elif op == "putfield":
                    val = stk.pop(); obj = stk.pop()
                    if a.desc[0] in ("L", "["):
                        store_into(obj[0], val[0], "putfield")
                elif op == "new":
                    stk.append((frozenset((("A", ins.pc),)), False))
                elif op in ("newarray", "anewarray"):
                    stk.pop()
                    stk.append((frozenset((("A", ins.pc),)), False))
                elif op == "multianewarray":
                    for _ in range(ins.arg2):
                        stk.pop()
                    stk.append((frozenset((("A", ins.pc),)), False))
                elif op in ("monitorenter", "monitorexit"):
                    v = stk.pop()
                    esc(v[0], "monitor")
                elif op == "nullchk":          # pseudo instruction of the inliner: null check of the receiver
                    stk.pop()
                elif op == "initclass":        # pseudo instruction of the inliner: class initialization
                    pass
                elif op == "invokedynamic":
                    args, ret = parse_method_desc(a.desc)
                    for _ in args:
                        esc(stk.pop()[0], "invokedynamic")
                    if ret != "V":
                        stk.append((EMPTY, ret in ("J", "D")))
                elif op in ("invokestatic", "invokespecial", "invokevirtual", "invokeinterface"):
                    args, ret = parse_method_desc(a.desc)
                    popped = [stk.pop() for _ in range(len(args))]
                    popped.reverse()
                    if op != "invokestatic":
                        popped.insert(0, stk.pop())
                    # reference arguments in order (receiver first): the index into a callee summary
                    kinds = ([] if op == "invokestatic" else ["L"]) + args
                    ref_vals = [popped[j][0] for j, d in enumerate(kinds) if d[0] in ("L", "[")]
                    targets = self._call_targets(ci, op, a)
                    is_obj_init = op == "invokespecial" and a.owner == "java/lang/Object" and a.name == "<init>"
                    intrinsic = False
                    if op == "invokestatic" and targets and len(targets) == 1:
                        dc0, m0 = targets[0]
                        intrinsic = self.INTRINSICS.get((dc0.name, m0.name + m0.desc)) is not None
                    if is_obj_init or intrinsic:
                        pass
                    elif not targets:
                        for vals in ref_vals:
                            esc(vals, "unresolved call")
                    else:
                        for dc, tm in targets:
                            callees.add((dc.name, tm.key))
                            self.callers[(dc.name, tm.key)].add(key_of_current)
                            s = self._target_summary(dc, tm)
                            cs = self._target_csummary(dc, tm)
                            for n_i, vals in enumerate(ref_vals):
                                if not vals:
                                    continue
                                if n_i >= len(s) or s[n_i]:
                                    esc(vals, f"arg of {dc.name}.{tm.name}")
                                elif n_i >= len(cs) or cs[n_i]:
                                    content_esc(vals, f"content via {dc.name}.{tm.name}")
                    if ret != "V":
                        fresh = False
                        if ret[0] in ("L", "[") and targets and not is_obj_init and not intrinsic:
                            fresh = all(self.ret_fresh.get((dc.name, tm.key), False) for dc, tm in targets)
                        if fresh:
                            rsites[ins.pc] = [(dc.name, tm.key) for dc, tm in targets]
                            stk.append((frozenset((("R", ins.pc),)), False))
                        else:
                            stk.append((EMPTY, ret in ("J", "D")))
                else:
                    raise _Bail(f"opcode {op}")

                if succs is not None:
                    return [(s, locs, stk) for s in succs]
                i += 1
                if i >= len(insns):
                    return []
                nxt = insns[i].pc
                if nxt in leaders:
                    return [(nxt, locs, stk)]

        key_of_current = (ci.name, m.key)
        try:
            while work:
                steps += 1
                if steps > budget:
                    raise _Bail("step budget")
                if steps % 512 == 0 and time.time() > self.deadline:
                    raise AnalysisTimeout()
                bpc = work.pop()
                for spc, nl, ns in run_block(bpc):
                    if spc not in in_states:
                        in_states[spc] = (list(nl), list(ns))
                        work.append(spc)
                        continue
                    ol, os_ = in_states[spc]
                    if len(os_) != len(ns):
                        raise _Bail("stack mismatch")
                    n = max(len(ol), len(nl))
                    ml = [(ol[k] if k < len(ol) else EMPTY) | (nl[k] if k < len(nl) else EMPTY) for k in range(n)]
                    ms = [(os_[k][0] | ns[k][0], os_[k][1] or ns[k][1]) for k in range(len(ns))]
                    if ml != ol or ms != os_:
                        in_states[spc] = (ml, ms)
                        work.append(spc)
        except _Bail:
            # not analyzable: everything escapes
            return MethodResult(tuple(True for _ in range(nparams)), sites,
                                {pc: "unanalyzable" for pc in sites}, loop_pcs, frozenset(callees), False,
                                cparams=tuple(True for _ in range(nparams)))
        # an escaping owner takes everything stored inside it along; an escaping content takes what was stored there
        closing = True
        while closing:
            closing = False
            for o in list(escaped):
                inner = contents.get(o) if o[0] in ("A", "R") else (contents.get(o[1]) if o[0] == "C" else None)
                for c in inner or ():
                    if c not in escaped:
                        escaped[c] = "via owner"
                        reasons_of[c].add("via owner")
                        closing = True
        params = tuple(("P", o) in escaped for _, o in refp)
        cparams = tuple(("PC", o) in escaped or ("P", o) in escaped for _, o in refp)
        esc_sites = {pc: escaped[("A", pc)] for pc in sites if ("A", pc) in escaped}
        rloop = frozenset(pc for pc in rsites if any(a <= pc <= b for a, b in loops))
        resc = {pc: escaped[("R", pc)] for pc in rsites if ("R", pc) in escaped}
        # a returned reference moves to the caller only if it is a fresh object whose sole escape is this return
        fresh = bool(ret_vals)
        for vals in ret_vals:
            if not vals:
                fresh = False
            for o in vals:
                if o[0] not in ("A", "R") or reasons_of[o] - {"areturn"}:
                    fresh = False
        iter_pcs = self._iteration_local(insns, idx, loops, loop_pcs, escaped, in_states, contents, code)
        return MethodResult(params, sites, esc_sites, loop_pcs, frozenset(callees), fresh,
                            frozenset(rsites), resc, rloop, dict(rsites), cparams, iter_pcs)

    # ------------------------------------------------------------------ allocation sites that live one loop iteration
    def _iteration_local(self, insns, idx, loops, loop_pcs, escaped, in_states, contents, code) -> frozenset:
        """Loop allocation sites whose object cannot be seen by the next iteration.

        The site must not escape the method (checked by the caller) and, for every loop around it, no local that is
        live at the loop header and no stack entry may hold the object, nor anything that holds it (through the stored
        contents). Then the object is dead when the iteration ends, and one slot (stack or arena) is enough for all
        iterations. Liveness matters: javac leaves dead locals holding last iteration's object, which must not count."""
        cand = [pc for pc in loop_pcs if ("A", pc) not in escaped]
        if not cand:
            return EMPTY
        n = len(insns)
        # liveness of locals, backward over instructions
        gen: list = [None] * n
        kill: list = [None] * n
        succ: list = [None] * n
        LOAD = ("iload", "lload", "fload", "dload", "aload")
        STORE_ = ("istore", "lstore", "fstore", "dstore", "astore")
        for i, x in enumerate(insns):
            g = k = ()
            if x.op in LOAD:
                g = (x.arg, x.arg + 1) if x.op in ("lload", "dload") else (x.arg,)
            elif x.op in STORE_:
                k = (x.arg, x.arg + 1) if x.op in ("lstore", "dstore") else (x.arg,)
            elif x.op == "iinc":
                g = (x.arg,)
            gen[i], kill[i] = frozenset(g), frozenset(k)
            sc = [idx[t] for t in x.targets if t in idx]
            if x.op not in ("goto", "ireturn", "lreturn", "freturn", "dreturn", "areturn", "return", "athrow", "tableswitch", "lookupswitch") and i + 1 < n:
                sc.append(i + 1)
            succ[i] = sc
        for h in code.handlers:
            hi = idx.get(h.handler_pc)
            if hi is None:
                continue
            for i, x in enumerate(insns):
                if h.start_pc <= x.pc < h.end_pc:
                    succ[i].append(hi)
        live_in = [frozenset()] * n
        changed = True
        rounds = 0
        while changed and rounds < 60:
            changed = False
            rounds += 1
            for i in range(n - 1, -1, -1):
                out = frozenset().union(*(live_in[j] for j in succ[i])) if succ[i] else frozenset()
                li = gen[i] | (out - kill[i])
                if li != live_in[i]:
                    live_in[i] = li
                    changed = True
        if changed:
            return EMPTY                      # no fixpoint in budget: give up (sound)
        carried_by_loop: dict = {}
        for (t, b) in loops:
            st = in_states.get(t)
            if st is None:
                carried_by_loop[(t, b)] = None            # unreachable header: nothing to check
                continue
            locs, stk = st
            live = live_in[idx[t]]
            roots = set()
            for j in live:
                if j < len(locs):
                    roots.update(locs[j])
            for v, _ in stk:
                roots.update(v)
            reach = set(roots)
            work = [o for o in roots if o[0] in ("A", "R")]
            while work:
                o = work.pop()
                for c in contents.get(o, ()):
                    if c not in reach:
                        reach.add(c)
                        if c[0] in ("A", "R"):
                            work.append(c)
            carried_by_loop[(t, b)] = reach
        ok = []
        for pc in cand:
            good = True
            for (t, b) in loops:
                if t <= pc <= b:
                    reach = carried_by_loop[(t, b)]
                    if reach is not None and ("A", pc) in reach:
                        good = False
                        break
            if good:
                ok.append(pc)
        return frozenset(ok)

    # ------------------------------------------------------------------ driver
    def run(self) -> Result:
        t0 = time.time()
        self._collect()
        queue = collections.deque(self._methods.keys())
        queued = set(queue)
        passes = 0
        while queue:
            key = queue.popleft()
            queued.discard(key)
            passes += 1
            ci, m = self._methods[key]
            r = self._analyze(ci, m)
            self.res[key] = r
            changed = False
            if r.params != self.summary[key]:
                self.summary[key] = r.params
                changed = True
            if r.cparams != self.csummary[key]:
                self.csummary[key] = r.cparams
                changed = True
            rf = self.ret_fresh[key] and r.ret_fresh
            if rf != self.ret_fresh[key]:
                self.ret_fresh[key] = rf
                changed = True
            if changed:
                for caller in self.callers.get(key, ()):
                    if caller not in queued:
                        queued.add(caller)
                        queue.append(caller)
        out = Result(dict(self.res), time.time() - t0, passes)
        return out


class _Bail(Exception):
    pass


def analyze(prog, budget_s: float = 1500.0) -> Optional[Result]:
    """Run the analysis; returns None (never a partial result) if the time budget is exhausted."""
    try:
        return EscapeAnalysis(prog, budget_s).run()
    except AnalysisTimeout:
        return None


def write_report(result: Optional[Result], path: str) -> None:
    """Statistics for deciding where compile-time memory management pays off."""
    if result is None:
        with open(path, "w", encoding="utf-8") as fh:
            json.dump({"error": "escape analysis hit its time budget; no result"}, fh)
        return
    total = noesc = noesc_loop = 0
    fresh_methods = fresh_calls = lift_calls = lift_calls_loop = 0
    areturn_sites_in_fresh = 0
    reasons: collections.Counter = collections.Counter()
    by_pkg: dict[str, list] = collections.defaultdict(lambda: [0, 0, 0])
    worth: list[tuple[int, str]] = []
    for (cn, key), r in result.methods.items():
        if r.ret_fresh:
            fresh_methods += 1
            areturn_sites_in_fresh += sum(1 for w in r.escaped.values() if w == "areturn")
        fresh_calls += len(r.rsites)
        lift_calls += sum(1 for pc in r.rsites if pc not in r.rescaped and pc not in r.rloop_pcs)
        lift_calls_loop += sum(1 for pc in r.rsites if pc not in r.rescaped and pc in r.rloop_pcs)
        n_sites = len(r.sites)
        if not n_sites:
            continue
        pkg = "/".join(cn.split("/")[:3])
        ne = [pc for pc in r.sites if pc not in r.escaped]
        nl = [pc for pc in ne if pc in r.loop_pcs]
        total += n_sites
        noesc += len(ne)
        noesc_loop += len(nl)
        by_pkg[pkg][0] += n_sites
        by_pkg[pkg][1] += len(ne) - len(nl)
        by_pkg[pkg][2] += len(nl)
        for pc, why in r.escaped.items():
            reasons[why.split(" of ")[0] if why.startswith("arg of") else why] += 1
        if len(ne) - len(nl):
            worth.append((len(ne) - len(nl), f"{cn}.{key}"))
    worth.sort(reverse=True)
    doc = {
        "seconds": round(result.seconds, 1),
        "method_analyses": result.passes,
        "alloc_sites": total,
        "not_escaping_total": noesc,
        "not_escaping_in_loop": noesc_loop,
        "not_escaping_usable": noesc - noesc_loop,
        "escape_reasons": dict(reasons.most_common(12)),
        "methods_returning_fresh_object": fresh_methods,
        "calls_receiving_fresh_object": fresh_calls,
        "of_those_not_escaping_caller_usable": lift_calls,
        "of_those_not_escaping_caller_in_loop": lift_calls_loop,
        "areturn_sites_in_fresh_methods": areturn_sites_in_fresh,
        "by_package": {k: {"sites": v[0], "usable": v[1], "in_loop": v[2]}
                       for k, v in sorted(by_pkg.items(), key=lambda kv: -kv[1][0])[:40]},
        "top_methods": [f"{n} {name}" for n, name in worth[:40]],
    }
    prefix = os.environ.get("MONO_ESCAPE_DUMP")
    if prefix:
        dump = {}
        for (cn, key), r in result.methods.items():
            if cn.startswith(tuple(prefix.split(","))) and (r.sites or r.rsites):
                dump[f"{cn}.{key}"] = {
                    "params_escape": list(r.params),
                    "ret_fresh": r.ret_fresh,
                    "fresh_targets": {str(pc): [f"{c}.{k}" for c, k in r.rtargets.get(pc, [])] for pc in sorted(r.rsites)},
                    "fresh_returns_received": {str(pc): (r.rescaped[pc] if pc in r.rescaped else ("noescape-in-loop" if pc in r.rloop_pcs else "LIFTABLE"))
                                               for pc in sorted(r.rsites)},
                    "sites": {str(pc): (r.escaped[pc] if pc in r.escaped else ("noescape-in-loop" if pc in r.loop_pcs else "NOESCAPE"))
                              for pc in sorted(r.sites)},
                }
        doc["dump"] = dump
    with open(path, "w", encoding="utf-8") as fh:
        json.dump(doc, fh, indent=1)
