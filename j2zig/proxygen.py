"""Build-time generation of java.lang.reflect.Proxy classes.

A native image cannot define classes at run time, so for each interface list the closed world
can request (annotation interfaces, constant-folded Proxy.newProxyInstance sites) the transpiler
emits the same class HotSpot's ProxyGenerator would: `final class $J2zProxyN extends Proxy
implements I1..In` whose methods forward to `h.invoke(this, Method, Object[])`, unboxing the
result, rethrowing unchecked and declared exceptions and wrapping others in
UndeclaredThrowableException.
"""
from __future__ import annotations

from .asm import Asm, fref, make_class, make_method, mref
from .classfile import (ACC_FINAL, ACC_PRIVATE, ACC_PUBLIC, ACC_STATIC, ACC_SUPER, ClassConst,
                        ExceptionHandler, FieldInfo)
from .world import World, parse_method_desc, slots_of

BOX = {"Z": ("java/lang/Boolean", "booleanValue"), "B": ("java/lang/Byte", "byteValue"),
       "C": ("java/lang/Character", "charValue"), "S": ("java/lang/Short", "shortValue"),
       "I": ("java/lang/Integer", "intValue"), "J": ("java/lang/Long", "longValue"),
       "F": ("java/lang/Float", "floatValue"), "D": ("java/lang/Double", "doubleValue")}
PROXY = "java/lang/reflect/Proxy"
IH = "java/lang/reflect/InvocationHandler"
OBJECT_METHODS = [("hashCode", "()I"), ("equals", "(Ljava/lang/Object;)Z"), ("toString", "()Ljava/lang/String;")]


def proxy_methods(w: World, interfaces: list[str]):
    """(owner, name, desc, declared exceptions) in ProxyGenerator order: Object methods first."""
    out = [("java/lang/Object", n, d, []) for n, d in OBJECT_METHODS]
    seen = {n + d for n, d in OBJECT_METHODS}
    for iname in interfaces:
        ici = w.get(iname)
        if ici is None:
            continue
        for c in [ici] + w.all_interfaces(ici):
            for m in c.cf.methods:
                if m.is_static or m.access & ACC_PRIVATE or m.name.startswith("<"):
                    continue
                if m.name + m.desc in seen:
                    continue
                seen.add(m.name + m.desc)
                out.append((c.name, m.name, m.desc, list(m.exceptions)))
    return out


def _push_class(a: Asm, desc: str):
    if desc in BOX:
        a.op("getstatic", fref(BOX[desc][0], "TYPE", "Ljava/lang/Class;"))
    else:
        a.op("ldc", ClassConst(desc[1:-1] if desc.startswith("L") else desc))


def make_proxy(w: World, name: str, interfaces: list[str]):
    methods_info = proxy_methods(w, interfaces)
    fields = [FieldInfo(ACC_PRIVATE | ACC_STATIC, f"m{k}", "Ljava/lang/reflect/Method;")
              for k in range(len(methods_info))]
    methods = []
    # <clinit>: resolve the Method objects
    a = Asm()
    for k, (owner, mname, desc, _exc) in enumerate(methods_info):
        params, _ret = parse_method_desc(desc)
        a.op("ldc", ClassConst(owner))
        a.sconst(mname)
        a.iconst(len(params))
        a.op("anewarray", ClassConst("java/lang/Class"))
        for j, p in enumerate(params):
            a.op("dup")
            a.iconst(j)
            _push_class(a, p)
            a.op("aastore")
        a.op("invokevirtual", mref("java/lang/Class", "getMethod",
                                   "(Ljava/lang/String;[Ljava/lang/Class;)Ljava/lang/reflect/Method;"))
        a.op("putstatic", fref(name, f"m{k}", "Ljava/lang/reflect/Method;"))
    a.op("return")
    methods.append(make_method("<clinit>", "()V", ACC_STATIC, a, 0))
    # <init>(InvocationHandler)
    a = Asm()
    a.op("aload", 0)
    a.op("aload", 1)
    a.op("invokespecial", mref(PROXY, "<init>", f"(L{IH};)V"))
    a.op("return")
    methods.append(make_method("<init>", f"(L{IH};)V", ACC_PUBLIC, a, 2))
    # forwarding methods
    for k, (owner, mname, desc, exc) in enumerate(methods_info):
        params, ret = parse_method_desc(desc)
        a = Asm()
        a.label("start")
        a.op("aload", 0)
        a.op("getfield", fref(PROXY, "h", f"L{IH};"))
        a.op("aload", 0)
        a.op("getstatic", fref(name, f"m{k}", "Ljava/lang/reflect/Method;"))
        if not params:
            a.op("aconst_null")
        else:
            a.iconst(len(params))
            a.op("anewarray", ClassConst("java/lang/Object"))
            slot = 1
            for j, p in enumerate(params):
                a.op("dup")
                a.iconst(j)
                a.load(p, slot)
                if p in BOX:
                    a.op("invokestatic", mref(BOX[p][0], "valueOf", f"({p})L{BOX[p][0]};"))
                a.op("aastore")
                slot += slots_of(p)
        a.op("invokeinterface", mref(IH, "invoke",
                                     "(Ljava/lang/Object;Ljava/lang/reflect/Method;[Ljava/lang/Object;)Ljava/lang/Object;",
                                     interface=True))
        if ret == "V":
            a.op("pop")
            a.op("return")
        elif ret in BOX:
            box, getter = BOX[ret]
            a.op("checkcast", ClassConst(box))
            a.op("invokevirtual", mref(box, getter, f"(){ret}"))
            a.ret(ret)
        else:
            if ret != "Ljava/lang/Object;":
                a.op("checkcast", ClassConst(ret[1:-1] if ret.startswith("L") else ret))
            a.op("areturn")
        a.label("end")
        a.label("rethrow")
        a.op("athrow")
        a.label("wrap")
        nloc = 1 + sum(slots_of(p) for p in params)
        a.op("astore", nloc)
        a.op("new", ClassConst("java/lang/reflect/UndeclaredThrowableException"))
        a.op("dup")
        a.op("aload", nloc)
        a.op("invokespecial", mref("java/lang/reflect/UndeclaredThrowableException", "<init>",
                                   "(Ljava/lang/Throwable;)V"))
        a.op("athrow")
        m = make_method(mname, desc, ACC_PUBLIC | ACC_FINAL, a, nloc + 1)
        L = a.labels
        handlers = []
        passthrough = ["java/lang/RuntimeException", "java/lang/Error"] + [x for x in exc]
        if not any(x in ("java/lang/Throwable", "java/lang/Exception") for x in exc):
            for t in passthrough:
                handlers.append(ExceptionHandler(L["start"], L["end"], L["rethrow"], t))
            handlers.append(ExceptionHandler(L["start"], L["end"], L["wrap"], "java/lang/Throwable"))
        m.code.handlers = handlers
        m.exceptions = list(exc)
        methods.append(m)
    return make_class(name, PROXY, list(interfaces), fields, methods,
                      access=ACC_PUBLIC | ACC_FINAL | ACC_SUPER)


def make_mh_invoker(name: str, desc: str):
    """Static stub for a signature-polymorphic MethodHandle.invoke/invokeExact call site:
    (MethodHandle, args...) -> box args, MethodHandle.j2zInvoke(Object[]), unbox result."""
    MH = "java/lang/invoke/MethodHandle"
    params, ret = parse_method_desc(desc)
    a = Asm()
    a.op("aload", 0)
    a.iconst(len(params))
    a.op("anewarray", ClassConst("java/lang/Object"))
    slot = 1
    for j, p in enumerate(params):
        a.op("dup")
        a.iconst(j)
        a.load(p, slot)
        if p in BOX:
            a.op("invokestatic", mref(BOX[p][0], "valueOf", f"({p})L{BOX[p][0]};"))
        a.op("aastore")
        slot += slots_of(p)
    a.op("invokevirtual", mref(MH, "j2zInvoke", "([Ljava/lang/Object;)Ljava/lang/Object;"))
    if ret == "V":
        a.op("pop")
        a.op("return")
    elif ret in BOX:
        getter = {"Z": "Boolean", "B": "Byte", "C": "Char", "S": "Short", "I": "Int", "J": "Long",
                  "F": "Float", "D": "Double"}[ret]
        a.op("invokestatic", mref(MH, "j2zTo" + getter, f"(Ljava/lang/Object;){ret}"))
        a.ret(ret)
    else:
        if ret != "Ljava/lang/Object;":
            a.op("checkcast", ClassConst(ret[1:-1] if ret.startswith("L") else ret))
        a.op("areturn")
    sdesc = f"(L{MH};" + "".join(params) + ")" + ret
    m = make_method("invoke", sdesc, ACC_PUBLIC | ACC_STATIC, a, slot)
    return make_class(name, "java/lang/Object", [], [], [m]), sdesc
