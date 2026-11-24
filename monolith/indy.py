"""invokedynamic desugaring.

Every supported invokedynamic call site is rewritten to an `invokestatic` of a
synthetic method whose body is ordinary bytecode, so the rest of the pipeline
(RTA, type inference, Rust codegen) needs no indy-specific logic.

Supported bootstrap methods (cover 100% of indy sites in Minecraft 26.3 client.jar
and its libraries, see docs/ANALYSIS.md):
  * java/lang/invoke/LambdaMetafactory.metafactory / altMetafactory
  * java/lang/invoke/StringConcatFactory.makeConcatWithConstants / makeConcat
  * java/lang/runtime/ObjectMethods.bootstrap   (record toString/hashCode/equals)
  * java/lang/runtime/SwitchBootstraps.typeSwitch / enumSwitch
Anything else is reported as unsupported (and the call site throws
BootstrapMethodError at run time, which is what a JVM does on bootstrap failure).
"""
from __future__ import annotations

from .asm import Asm, fref, make_class, make_method, mref
from .bytecode import Insn
from .classfile import (ACC_PUBLIC, ACC_STATIC, ACC_SYNTHETIC, ACC_FINAL, ClassConst, ClassFile,
                        DynamicConst, FieldInfo, MethodHandleConst, MethodTypeConst,
                        REF_getField, REF_invokeInterface, REF_invokeSpecial, REF_invokeStatic,
                        REF_invokeVirtual, REF_newInvokeSpecial)
from .world import kind_of, parse_method_desc, slots_of

BOX = {"Z": "java/lang/Boolean", "B": "java/lang/Byte", "C": "java/lang/Character",
       "S": "java/lang/Short", "I": "java/lang/Integer", "J": "java/lang/Long",
       "F": "java/lang/Float", "D": "java/lang/Double"}
UNBOX_NAME = {"Z": "booleanValue", "B": "byteValue", "C": "charValue", "S": "shortValue",
              "I": "intValue", "J": "longValue", "F": "floatValue", "D": "doubleValue"}
PRIM = set("ZBCSIJFD")
WIDEN = {("I", "J"): "i2l", ("I", "F"): "i2f", ("I", "D"): "i2d", ("J", "F"): "l2f",
         ("J", "D"): "l2d", ("F", "D"): "f2d"}


def _unbox_owner(desc: str) -> str | None:
    if desc.startswith("L"):
        n = desc[1:-1]
        for p, b in BOX.items():
            if b == n:
                return p
    return None


def _ref_name(desc: str) -> str:
    return desc[1:-1] if desc.startswith("L") else desc


class Unsupported(Exception):
    pass


class IndyDesugarer:
    def __init__(self, report):
        self.report = report  # callable(kind, message)
        # synthetic class names are numbered per enclosing class: a new lambda in one class must not rename the
        # synthetic classes of every class desugared after it (that would change their generated code)
        self.counters: dict[str, int] = {}
        self.new_classes: list[ClassFile] = []

    # -------------------------------------------------------------- conversions
    def convert(self, a: Asm, frm: str, to: str):
        """Emit an asType-style conversion of the value on top of stack from `frm` to `to`."""
        if frm == to:
            return
        fp, tp = frm in PRIM, to in PRIM
        if fp and tp:
            kf, kt = kind_of(frm), kind_of(to)
            if kf != kt:
                op = WIDEN.get((kf, kt))
                if op is None:
                    raise Unsupported(f"primitive conversion {frm}->{to}")
                a.op(op)
            elif to == "B" and frm in "SCI":
                a.op("i2b")
            elif to == "S" and frm in "CI":
                a.op("i2s")
            elif to == "C" and frm in "BSI":
                a.op("i2c")
            return
        if fp and not tp:
            box = BOX[frm]
            a.op("invokestatic", mref(box, "valueOf", f"({frm})L{box};"))
            if to not in (f"L{box};", "Ljava/lang/Object;"):
                a.op("checkcast", ClassConst(_ref_name(to)))
            return
        if not fp and tp:
            p = _unbox_owner(frm)
            if p is None:
                # Object/Number/Comparable -> cast to wrapper of the target primitive
                p = to
                a.op("checkcast", ClassConst(BOX[p]))
            a.op("invokevirtual", mref(BOX[p], UNBOX_NAME[p], f"(){p}"))
            if p != to:
                self.convert(a, p, to)
            return
        if to != "Ljava/lang/Object;" and to != frm:
            a.op("checkcast", ClassConst(_ref_name(to)))

    # -------------------------------------------------------------- dispatch
    def desugar(self, cf: ClassFile, method, ins: Insn) -> Insn:
        d: DynamicConst = ins.arg
        bsm = cf.bootstrap_methods[d.bsm_index]
        h = bsm.handle.ref
        key = f"{h.owner}.{h.name}"
        try:
            if key in ("java/lang/invoke/LambdaMetafactory.metafactory",
                       "java/lang/invoke/LambdaMetafactory.altMetafactory"):
                ref = self.lambda_(cf, d, bsm.args, key.endswith("altMetafactory"))
            elif key == "java/lang/invoke/StringConcatFactory.makeConcatWithConstants":
                ref = self.concat(cf, d, bsm.args[0][1], bsm.args[1:])
            elif key == "java/lang/invoke/StringConcatFactory.makeConcat":
                args, _ = parse_method_desc(d.desc)
                ref = self.concat(cf, d, "\u0001" * len(args), [])
            elif key == "java/lang/runtime/ObjectMethods.bootstrap":
                ref = self.object_methods(cf, d, bsm.args)
            elif key == "java/lang/runtime/SwitchBootstraps.typeSwitch":
                ref = self.type_switch(cf, d, bsm.args, enum=False)
            elif key == "java/lang/runtime/SwitchBootstraps.enumSwitch":
                ref = self.type_switch(cf, d, bsm.args, enum=True)
            else:
                raise Unsupported(f"bootstrap method {key}")
        except Unsupported as e:
            self.report("unsupported-indy", f"{cf.name}.{method.name}{method.desc}@{ins.pc}: {e}")
            ref = self.bootstrap_error(cf, d, str(e))
        return Insn(ins.pc, "invokestatic", ref, None, (), ins.length)

    def _next(self, cf: ClassFile) -> int:
        n = self.counters.get(cf.name, 0) + 1
        self.counters[cf.name] = n
        return n

    def _holder(self, cf: ClassFile, suffix: str) -> str:
        return f"{cf.name}$$MONO{suffix}${self._next(cf)}"

    def _static_holder(self, cf: ClassFile, name: str, mname: str, desc: str, a: Asm, locals_: int):
        m = make_method(mname, desc, ACC_PUBLIC | ACC_STATIC | ACC_SYNTHETIC, a, locals_)
        self.new_classes.append(make_class(name, "java/lang/Object", [], [], [m]))
        return mref(name, mname, desc)

    def bootstrap_error(self, cf: ClassFile, d: DynamicConst, msg: str):
        name = self._holder(cf, "BsmError")
        args, ret = parse_method_desc(d.desc)
        a = Asm()
        a.op("new", ClassConst("java/lang/BootstrapMethodError"))
        a.op("dup")
        a.sconst(msg)
        a.op("invokespecial", mref("java/lang/BootstrapMethodError", "<init>", "(Ljava/lang/String;)V"))
        a.op("athrow")
        return self._static_holder(cf, name, "call", d.desc, a, sum(slots_of(x) for x in args))

    # -------------------------------------------------------------- lambdas
    def lambda_(self, cf: ClassFile, d: DynamicConst, bargs, alt: bool):
        sam_t: MethodTypeConst = bargs[0]
        impl: MethodHandleConst = bargs[1]
        inst_t: MethodTypeConst = bargs[2]
        iface = _ref_name(parse_method_desc(d.desc)[1])
        caps, _ = parse_method_desc(d.desc)
        markers: list[str] = []
        bridges: list[str] = []
        if alt:
            flags = bargs[3][1]
            if flags & 1:  # FLAG_SERIALIZABLE
                markers.append("java/io/Serializable")
            k = 4
            if flags & 2:
                n = bargs[k][1]
                markers += [x.name for x in bargs[k + 1:k + 1 + n] if x.name not in markers]
                k += 1 + n
            if flags & 4:
                n = bargs[k][1]
                bridges = [x.desc for x in bargs[k + 1:k + 1 + n]]
        name = f"{cf.name}$$Lambda${self._next(cf)}"
        fields = [FieldInfo(ACC_FINAL, f"cap{i}", t) for i, t in enumerate(caps)]
        methods = []
        # <init>(caps)
        a = Asm()
        a.op("aload", 0)
        a.op("invokespecial", mref("java/lang/Object", "<init>", "()V"))
        slot = 1
        for i, t in enumerate(caps):
            a.op("aload", 0)
            a.load(t, slot)
            a.op("putfield", fref(name, f"cap{i}", t))
            slot += slots_of(t)
        a.op("return")
        init_desc = "(" + "".join(caps) + ")V"
        methods.append(make_method("<init>", init_desc, ACC_PUBLIC, a, slot))
        # static factory
        a = Asm()
        a.op("new", ClassConst(name))
        a.op("dup")
        slot = 0
        for t in caps:
            a.load(t, slot)
            slot += slots_of(t)
        a.op("invokespecial", mref(name, "<init>", init_desc))
        a.op("areturn")
        methods.append(make_method("create", d.desc, ACC_PUBLIC | ACC_STATIC, a, slot))
        # SAM implementation (+ bridges)
        for sam_desc in [sam_t.desc] + [b for b in bridges if b != sam_t.desc]:
            methods.append(self._lambda_body(name, d.name, sam_desc, inst_t.desc, impl, caps))
        self.new_classes.append(make_class(name, "java/lang/Object", [iface] + markers, fields, methods))
        return mref(name, "create", d.desc)

    def _lambda_body(self, name, mname, sam_desc, inst_desc, impl: MethodHandleConst, caps):
        sam_args, sam_ret = parse_method_desc(sam_desc)
        inst_args, inst_ret = parse_method_desc(inst_desc)
        r = impl.ref
        impl_args, impl_ret = parse_method_desc(r.desc)
        kind = impl.ref_kind
        a = Asm()
        if kind == REF_newInvokeSpecial:
            a.op("new", ClassConst(r.owner))
            a.op("dup")
            impl_ret = f"L{r.owner};"
            params = list(impl_args)
        elif kind == REF_invokeStatic:
            params = list(impl_args)
        elif kind in (REF_invokeVirtual, REF_invokeInterface, REF_invokeSpecial):
            params = [f"L{r.owner};"] + list(impl_args)
        else:
            raise Unsupported(f"lambda impl handle kind {kind}")
        # sources: captured fields then SAM args
        sources = []  # (load-fn, from-desc)
        for i, t in enumerate(caps):
            sources.append(("cap", i, t, t))
        slot = 1
        for i, t in enumerate(sam_args):
            it = inst_args[i] if i < len(inst_args) else t
            sources.append(("arg", slot, t, it))
            slot += slots_of(t)
        if len(sources) != len(params):
            raise Unsupported(f"lambda arity mismatch {sam_desc} -> {r.owner}.{r.name}{r.desc}")
        for src, p in zip(sources, params):
            if src[0] == "cap":
                a.op("aload", 0)
                a.op("getfield", fref(name, f"cap{src[1]}", src[2]))
                self.convert(a, src[2], p)
            else:
                a.load(src[2], src[1])
                frm = src[2]
                if src[3] != frm and src[3] not in PRIM and frm not in PRIM:
                    self.convert(a, frm, src[3])
                    frm = src[3]
                self.convert(a, frm, p)
        if kind == REF_newInvokeSpecial:
            a.op("invokespecial", mref(r.owner, "<init>", r.desc))
        elif kind == REF_invokeStatic:
            a.op("invokestatic", r)
        elif kind == REF_invokeVirtual:
            a.op("invokevirtual", r)
        elif kind == REF_invokeInterface:
            a.op("invokeinterface", r)
        else:
            a.op("invokespecial", r)
        if sam_ret == "V":
            if impl_ret != "V":
                a.op("pop2" if slots_of(impl_ret) == 2 else "pop")
        else:
            frm = impl_ret
            if inst_ret != frm and inst_ret not in PRIM and frm not in PRIM:
                self.convert(a, frm, inst_ret)
                frm = inst_ret
            elif frm in PRIM and inst_ret in PRIM and frm != inst_ret:
                self.convert(a, frm, inst_ret)
                frm = inst_ret
            self.convert(a, frm, sam_ret)
        a.ret(sam_ret)
        return make_method(mname, sam_desc, ACC_PUBLIC, a, slot)

    # -------------------------------------------------------------- string concat
    def concat(self, cf: ClassFile, d: DynamicConst, recipe: str, consts):
        args, _ = parse_method_desc(d.desc)
        name = self._holder(cf, "Concat")
        SB = "java/lang/StringBuilder"
        a = Asm()
        a.op("new", ClassConst(SB))
        a.op("dup")
        a.op("invokespecial", mref(SB, "<init>", "()V"))
        lit = []
        ai = 0
        ci = 0
        slot = 0

        def flush():
            if lit:
                a.sconst("".join(lit))
                a.op("invokevirtual", mref(SB, "append", f"(Ljava/lang/String;)L{SB};"))
                lit.clear()
        for ch in recipe:
            if ch == "\u0001":
                flush()
                t = args[ai]
                ai += 1
                a.load(t, slot)
                slot += slots_of(t)
                if t in ("B", "S"):
                    t = "I"
                if t not in PRIM and t != "Ljava/lang/String;":
                    t = "Ljava/lang/Object;"
                a.op("invokevirtual", mref(SB, "append", f"({t})L{SB};"))
            elif ch == "\u0002":
                c = consts[ci]
                ci += 1
                if isinstance(c, tuple) and c[0] == "string":
                    lit.append(c[1])
                elif isinstance(c, tuple) and c[0] in ("int", "long"):
                    lit.append(str(c[1]))
                else:
                    raise Unsupported(f"concat constant {c!r}")
            else:
                lit.append(ch)
        flush()
        a.op("invokevirtual", mref(SB, "toString", "()Ljava/lang/String;"))
        a.op("areturn")
        return self._static_holder(cf, name, "concat", d.desc, a, slot)

    # -------------------------------------------------------------- records
    def object_methods(self, cf: ClassFile, d: DynamicConst, bargs):
        rec = bargs[0].name
        names = bargs[1][1].split(";") if bargs[1][1] else []
        getters = [g.ref for g in bargs[2:]]
        for g in bargs[2:]:
            if g.ref_kind != REF_getField:
                raise Unsupported("ObjectMethods getter is not a getfield handle")
        name = self._holder(cf, "Record")
        a = Asm()
        if d.name == "toString":
            simple = rec.rsplit("/", 1)[-1]
            for ic in cf.inner_classes:
                if ic.inner == rec and ic.name:
                    simple = ic.name
            SB = "java/lang/StringBuilder"
            a.op("new", ClassConst(SB))
            a.op("dup")
            a.op("invokespecial", mref(SB, "<init>", "()V"))
            for i, g in enumerate(getters):
                a.sconst(("" if i == 0 else ", ") + (simple + "[" if i == 0 else "") + names[i] + "=")
                a.op("invokevirtual", mref(SB, "append", f"(Ljava/lang/String;)L{SB};"))
                a.op("aload", 0)
                a.op("getfield", g)
                t = g.desc
                if t in ("B", "S"):
                    t = "I"
                if t not in PRIM:
                    t = "Ljava/lang/Object;"
                a.op("invokevirtual", mref(SB, "append", f"({t})L{SB};"))
            a.sconst("]" if getters else simple + "[]")
            a.op("invokevirtual", mref(SB, "append", f"(Ljava/lang/String;)L{SB};"))
            a.op("invokevirtual", mref(SB, "toString", "()Ljava/lang/String;"))
            a.op("areturn")
            return self._static_holder(cf, name, "toString", d.desc, a, 1)
        if d.name == "hashCode":
            a.iconst(0)
            for g in getters:
                a.iconst(31)
                a.op("imul")
                a.op("aload", 0)
                a.op("getfield", g)
                t = g.desc
                if t in PRIM:
                    a.op("invokestatic", mref(BOX[t], "hashCode", f"({t})I"))
                else:
                    a.op("invokestatic", mref("java/util/Objects", "hashCode", "(Ljava/lang/Object;)I"))
                a.op("iadd")
            a.op("ireturn")
            return self._static_holder(cf, name, "hashCode", d.desc, a, 1)
        if d.name == "equals":
            a.op("aload", 0)
            a.op("aload", 1)
            a.jump("if_acmpeq", "T")
            a.op("aload", 1)
            a.op("instanceof", ClassConst(rec))
            a.jump("ifeq", "F")
            a.op("aload", 1)
            a.op("checkcast", ClassConst(rec))
            a.op("astore", 2)
            for g in reversed(getters):
                t = g.desc
                a.op("aload", 0)
                a.op("getfield", g)
                a.op("aload", 2)
                a.op("getfield", g)
                if t in "ZBCSI" and len(t) == 1:
                    a.jump("if_icmpne", "F")
                elif t == "J":
                    a.op("lcmp")
                    a.jump("ifne", "F")
                elif t == "F":
                    a.op("invokestatic", mref("java/lang/Float", "compare", "(FF)I"))
                    a.jump("ifne", "F")
                elif t == "D":
                    a.op("invokestatic", mref("java/lang/Double", "compare", "(DD)I"))
                    a.jump("ifne", "F")
                else:
                    a.op("invokestatic", mref("java/util/Objects", "equals",
                                              "(Ljava/lang/Object;Ljava/lang/Object;)Z"))
                    a.jump("ifeq", "F")
            a.label("T")
            a.iconst(1)
            a.op("ireturn")
            a.label("F")
            a.iconst(0)
            a.op("ireturn")
            return self._static_holder(cf, name, "equals", d.desc, a, 3)
        raise Unsupported(f"ObjectMethods.{d.name}")

    # -------------------------------------------------------------- pattern switch
    def type_switch(self, cf: ClassFile, d: DynamicConst, labels, enum: bool):
        args, _ = parse_method_desc(d.desc)  # (Object target, int restart)I
        name = self._holder(cf, "Switch")
        a = Asm()
        a.op("aload", 0)
        a.jump("ifnonnull", "nn")
        a.iconst(-1)
        a.op("ireturn")
        a.label("nn")
        for i, lab in enumerate(labels):
            nxt = f"n{i}"
            a.op("iload", 1)
            a.iconst(i)
            a.jump("if_icmpgt", nxt)
            if isinstance(lab, ClassConst):
                a.op("aload", 0)
                a.op("instanceof", lab)
                a.jump("ifeq", nxt)
            elif isinstance(lab, tuple) and lab[0] == "string":
                if enum:
                    a.op("aload", 0)
                    a.op("checkcast", ClassConst("java/lang/Enum"))
                    a.op("invokevirtual", mref("java/lang/Enum", "name", "()Ljava/lang/String;"))
                    a.sconst(lab[1])
                    a.op("invokevirtual", mref("java/lang/String", "equals", "(Ljava/lang/Object;)Z"))
                else:
                    a.sconst(lab[1])
                    a.op("aload", 0)
                    a.op("invokevirtual", mref("java/lang/String", "equals", "(Ljava/lang/Object;)Z"))
                a.jump("ifeq", nxt)
            elif isinstance(lab, tuple) and lab[0] == "int":
                a.op("aload", 0)
                a.op("instanceof", ClassConst("java/lang/Number"))
                a.jump("ifeq", f"c{i}")
                a.op("aload", 0)
                a.op("checkcast", ClassConst("java/lang/Number"))
                a.op("invokevirtual", mref("java/lang/Number", "intValue", "()I"))
                a.iconst(lab[1])
                a.jump("if_icmpne", nxt)
                a.iconst(i)
                a.op("ireturn")
                a.label(f"c{i}")
                a.op("aload", 0)
                a.op("instanceof", ClassConst("java/lang/Character"))
                a.jump("ifeq", nxt)
                a.op("aload", 0)
                a.op("checkcast", ClassConst("java/lang/Character"))
                a.op("invokevirtual", mref("java/lang/Character", "charValue", "()C"))
                a.iconst(lab[1])
                a.jump("if_icmpne", nxt)
            else:
                raise Unsupported(f"switch label {lab!r}")
            a.iconst(i)
            a.op("ireturn")
            a.label(nxt)
        a.iconst(len(labels))
        a.op("ireturn")
        return self._static_holder(cf, name, "typeSwitch", d.desc, a, 2)
