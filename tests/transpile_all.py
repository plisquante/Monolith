#!/usr/bin/env python3
"""Transpile EVERY method of EVERY class in the given archives (not only the RTA-reachable
ones) through the full front end and Zig code generator, and report failures.

usage: python3 tests/transpile_all.py [--extra-cp a.jar:b.jar] archive.jar|java.base.jmod ...

This is the per-class coverage check: it exercises the decoder, invokedynamic desugaring,
stack-type inference and code generation on every bytecode shape present in real code.
"""
from __future__ import annotations

import argparse
import collections
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from j2zig.classpath import ClassPath, ZipSource, DirSource, jdk_jmods  # noqa: E402
from j2zig.program import Program, Report  # noqa: E402
from j2zig.world import World, LinkError  # noqa: E402
from j2zig.zigen import ClassGen, MethodCompiler, ZigGen  # noqa: E402
from j2zig.__main__ import build_classlib  # noqa: E402


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--extra-cp", default="")
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("archives", nargs="+")
    a = ap.parse_args()
    extra = [p for p in a.extra_cp.split(os.pathsep) if p]
    cp = ClassPath(a.archives + extra + [build_classlib()] + jdk_jmods())
    rep = Report()
    prog = Program(World(cp), rep)
    gen = ZigGen(prog, "/nonexistent", {})
    t0 = time.time()
    stats = collections.Counter()
    failures = []
    for arc in a.archives:
        src = DirSource(arc) if os.path.isdir(arc) else ZipSource(arc)
        for n, name in enumerate(sorted(src.names())):
            if a.limit and n >= a.limit:
                break
            try:
                ci = prog.load(name)
            except LinkError:
                stats["class_link_error"] += 1
                continue
            if ci is None:
                stats["class_unloadable"] += 1
                continue
            stats["classes"] += 1
            cg = ClassGen(gen, ci)
            for m in ci.cf.methods:
                if m.code is None:
                    continue
                stats["methods"] += 1
                try:
                    MethodCompiler(cg, m).compile()
                    stats["ok"] += 1
                except Exception as e:  # noqa: BLE001 - collect every failure
                    stats["failed"] += 1
                    failures.append(f"{name}.{m.name}{m.desc}: {type(e).__name__}: {e}")
    dt = time.time() - t0
    print(f"classes={stats['classes']} methods={stats['methods']} ok={stats['ok']} failed={stats['failed']} "
          f"class_link_errors={stats['class_link_error']} unloadable={stats['class_unloadable']} ({dt:.0f}s)")
    kinds = collections.Counter(f.split(": ", 1)[1].split(":")[0] for f in failures)
    for k, v in kinds.most_common(20):
        print(f"  {v:6d} {k}")
    for f in failures[:40]:
        print("   ", f)
    issues = {k: len(set(v)) for k, v in rep.items.items()}
    print("reported issues:", issues)
    return 0 if not failures else 1


if __name__ == "__main__":
    sys.exit(main())
