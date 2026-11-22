"""Static analysis of a JAR set: what the transpiler has to cover.

Usage: python -m monolith.analyze [--json out.json] app.jar [lib.jar ...]

Reports: class file versions, class/method/instruction counts, opcode histogram,
invokedynamic bootstrap methods, native methods, JDK API surface by package,
reflection / Unsafe / VarHandle / MethodHandle / threading usage, and any
unsupported instructions (jsr/ret).
"""
from __future__ import annotations

import argparse
import collections
import json
import os
import sys

from .bytecode import UNSUPPORTED, decode
from .classfile import (ACC_NATIVE, ACC_SYNCHRONIZED, MemberRef, ClassConst)
from .classpath import ZipSource, DirSource
from .classfile import parse_class

REFLECTIVE = {
    ("java/lang/Class", "forName"), ("java/lang/Class", "getDeclaredMethod"),
    ("java/lang/Class", "getMethod"), ("java/lang/Class", "getDeclaredField"),
    ("java/lang/Class", "getField"), ("java/lang/Class", "getDeclaredConstructor"),
    ("java/lang/Class", "getConstructor"), ("java/lang/Class", "newInstance"),
    ("java/lang/Class", "getDeclaredFields"), ("java/lang/Class", "getDeclaredMethods"),
    ("java/lang/Class", "getMethods"), ("java/lang/Class", "getFields"),
    ("java/lang/reflect/Method", "invoke"), ("java/lang/reflect/Constructor", "newInstance"),
    ("java/lang/reflect/Field", "get"), ("java/lang/reflect/Field", "set"),
    ("java/lang/reflect/Proxy", "newProxyInstance"), ("java/util/ServiceLoader", "load"),
    ("java/lang/Class", "getRecordComponents"), ("java/lang/Class", "getEnumConstants"),
}


def analyze(paths: list[str]) -> dict:
    st = {
        "jars": {}, "versions": collections.Counter(), "classes": 0, "interfaces": 0,
        "methods": 0, "abstract_methods": 0, "native_methods": [], "synchronized_methods": 0,
        "code_bytes": 0, "instructions": 0, "opcodes": collections.Counter(),
        "indy_bsm": collections.Counter(), "condy_bsm": collections.Counter(),
        "jdk_packages": collections.Counter(), "jdk_classes": collections.Counter(),
        "reflection_calls": collections.Counter(), "unsafe_calls": 0, "varhandle_calls": 0,
        "methodhandle_calls": 0, "thread_classes_used": collections.Counter(),
        "unsupported": [], "parse_errors": [], "max_code_len": 0,
        "exception_handlers": 0, "string_switch_like": 0,
    }
    for p in paths:
        src = DirSource(p) if os.path.isdir(p) else ZipSource(p)
        names = list(src.names())
        st["jars"][os.path.basename(p)] = len(names)
        for n in names:
            try:
                cf = parse_class(src.read(n))
            except Exception as e:  # noqa: BLE001 - report and continue
                st["parse_errors"].append(f"{n}: {e}")
                continue
            st["classes"] += 1
            st["versions"][cf.major] += 1
            if cf.is_interface:
                st["interfaces"] += 1
            for e in cf.cp:
                if isinstance(e, ClassConst) or isinstance(e, MemberRef):
                    owner = e.name if isinstance(e, ClassConst) else e.owner
                    owner = owner.lstrip("[")
                    if owner.startswith("L"):
                        owner = owner[1:-1]
                    if owner.startswith(("java/", "javax/", "jdk/", "sun/")):
                        st["jdk_packages"][owner.rsplit("/", 1)[0]] += 1
                        st["jdk_classes"][owner] += 1
            for m in cf.methods:
                st["methods"] += 1
                if m.access & ACC_NATIVE:
                    st["native_methods"].append(f"{cf.name}.{m.name}{m.desc}")
                if m.access & ACC_SYNCHRONIZED:
                    st["synchronized_methods"] += 1
                if m.is_abstract:
                    st["abstract_methods"] += 1
                if not m.code:
                    continue
                st["code_bytes"] += len(m.code.code)
                st["max_code_len"] = max(st["max_code_len"], len(m.code.code))
                st["exception_handlers"] += len(m.code.handlers)
                try:
                    insns = decode(m.code.code, cf.cp)
                except Exception as e:  # noqa: BLE001
                    st["parse_errors"].append(f"{cf.name}.{m.name}: {e}")
                    continue
                for ins in insns:
                    st["instructions"] += 1
                    st["opcodes"][ins.op] += 1
                    if ins.op in UNSUPPORTED:
                        st["unsupported"].append(f"{cf.name}.{m.name}{m.desc}@{ins.pc}: {ins.op}")
                    if ins.op == "invokedynamic":
                        bsm = cf.bootstrap_methods[ins.arg.bsm_index].handle.ref
                        st["indy_bsm"][f"{bsm.owner}.{bsm.name}"] += 1
                    elif ins.op == "ldc" and type(ins.arg).__name__ == "DynamicConst":
                        bsm = cf.bootstrap_methods[ins.arg.bsm_index].handle.ref
                        st["condy_bsm"][f"{bsm.owner}.{bsm.name}"] += 1
                    elif ins.op.startswith("invoke") and isinstance(ins.arg, MemberRef):
                        o, nm = ins.arg.owner, ins.arg.name
                        if (o, nm) in REFLECTIVE:
                            st["reflection_calls"][f"{o}.{nm}"] += 1
                        if o in ("sun/misc/Unsafe", "jdk/internal/misc/Unsafe"):
                            st["unsafe_calls"] += 1
                        if o == "java/lang/invoke/VarHandle":
                            st["varhandle_calls"] += 1
                        if o == "java/lang/invoke/MethodHandle":
                            st["methodhandle_calls"] += 1
                        if o.startswith("java/util/concurrent") or o == "java/lang/Thread":
                            st["thread_classes_used"][o] += 1
    return st


def main(argv=None):
    ap = argparse.ArgumentParser()
    ap.add_argument("--json")
    ap.add_argument("--top", type=int, default=25)
    ap.add_argument("paths", nargs="+")
    a = ap.parse_args(argv)
    st = analyze(a.paths)
    t = a.top
    print(f"archives: {len(st['jars'])}  classes: {st['classes']}  interfaces: {st['interfaces']}")
    print(f"class file versions: {dict(sorted(st['versions'].items()))}")
    print(f"methods: {st['methods']}  abstract: {st['abstract_methods']}  "
          f"native: {len(st['native_methods'])}  synchronized: {st['synchronized_methods']}")
    print(f"bytecode bytes: {st['code_bytes']}  instructions: {st['instructions']}  "
          f"largest method: {st['max_code_len']} bytes  exception handlers: {st['exception_handlers']}")
    print(f"unsupported instructions (jsr/ret): {len(st['unsupported'])}")
    for u in st["unsupported"][:t]:
        print("   ", u)
    print(f"parse errors: {len(st['parse_errors'])}")
    for u in st["parse_errors"][:t]:
        print("   ", u)
    print("invokedynamic bootstrap methods:")
    for k, v in st["indy_bsm"].most_common(t):
        print(f"   {v:8d}  {k}")
    print("constant-dynamic bootstrap methods:")
    for k, v in st["condy_bsm"].most_common(t):
        print(f"   {v:8d}  {k}")
    print(f"reflection call sites: {sum(st['reflection_calls'].values())}")
    for k, v in st["reflection_calls"].most_common(t):
        print(f"   {v:8d}  {k}")
    print(f"Unsafe call sites: {st['unsafe_calls']}  VarHandle: {st['varhandle_calls']}  "
          f"MethodHandle: {st['methodhandle_calls']}")
    print(f"JDK packages referenced: {len(st['jdk_packages'])}  JDK classes referenced: {len(st['jdk_classes'])}")
    for k, v in st["jdk_packages"].most_common(t):
        print(f"   {v:8d}  {k}")
    print("concurrency classes:")
    for k, v in st["thread_classes_used"].most_common(t):
        print(f"   {v:8d}  {k}")
    print(f"native methods ({len(st['native_methods'])}), first {t}:")
    for k in st["native_methods"][:t]:
        print("   ", k)
    if a.json:
        def conv(o):
            if isinstance(o, collections.Counter):
                return dict(o.most_common())
            return o
        with open(a.json, "w") as fh:
            json.dump({k: conv(v) for k, v in st.items()}, fh, indent=1)


if __name__ == "__main__":
    sys.exit(main())
