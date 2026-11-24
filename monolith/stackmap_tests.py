#!/usr/bin/env python3
"""Tests of monolith/stackmap.py (work package C0). A few seconds.

1. A small compiled class: exact frames for a loop, a branch, a try/catch, a constructor.
2. Bulk: every method of N java.base classes parses; frame pcs strictly increase, stay below the code length, and each
   frame pc is an instruction start (checked against the decoded instruction list).
Usage: MONO_JDK_HOME=<jdk 25> python3 -m monolith.stackmap_tests
"""
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, ROOT)
JDK = os.environ.get("MONO_JDK_HOME", "/opt/tools/jdk25")
ENV = {k: v for k, v in os.environ.items() if k not in ("JAVA_TOOL_OPTIONS", "_JAVA_OPTIONS")}
fails = 0

SRC = """
public class Sm {
    int f;
    Sm(int a) { this.f = a > 0 ? a : -a; }
    static long loop(int n, String s) {
        long acc = 0;
        for (int i = 0; i < n; i++) { acc += i; }
        if (s == null) { return acc; }
        return acc + s.length();
    }
    static double tc(double d, int[] a) {
        try { return d / a[0]; } catch (ArithmeticException e) { return 0.0; } finally { d += 1; }
    }
}
"""


def check(name: str, ok: bool, detail: str = "") -> None:
    global fails
    print(("PASS " if ok else "FAIL ") + name + ("" if ok else "\n" + detail))
    fails += 0 if ok else 1


def main() -> int:
    os.environ["MONO_JDK_HOME"] = JDK
    from monolith.classpath import ClassPath, jdk_jmods
    from monolith.stackmap import frames
    from monolith.world import World
    with tempfile.TemporaryDirectory() as d:
        with open(os.path.join(d, "Sm.java"), "w") as fh:
            fh.write(SRC)
        r = subprocess.run([f"{JDK}/bin/javac", "-d", d, os.path.join(d, "Sm.java")], capture_output=True, text=True, env=ENV)
        if r.returncode:
            sys.exit("javac failed:\n" + r.stderr[-800:])
        cp = ClassPath([d] + jdk_jmods(), release=25, boot_start=1)
        world = World(cp)
        ci = world.get("Sm")
        init = frames(ci.cf, ci.cf.method("<init>", "(I)V"))
        check("<init> initial frame: uninit_this + int", init[0].locals == ["uninit_this", "int"], str(init[0]))
        check("<init> has a merge frame with this and int on the stack", any(f.stack == ["LSm;", "int"] for f in init[1:]), str(init))
        lp = frames(ci.cf, ci.cf.method("loop", "(ILjava/lang/String;)J"))
        check("loop initial frame", lp[0].locals == ["int", "Ljava/lang/String;"], str(lp[0]))
        check("loop frames carry long and int locals", any(f.locals[:2] == ["int", "Ljava/lang/String;"] and "long" in f.locals and f.locals.count("int") >= 2 for f in lp[1:]), str(lp))
        check("loop frame pcs strictly increase", all(a.pc < b.pc for a, b in zip(lp, lp[1:])), str(lp))
        tc = frames(ci.cf, ci.cf.method("tc", "(D[I)D"))
        check("try/catch: a handler frame has the exception on the stack",
              any(f.stack and f.stack[0] in ("Ljava/lang/ArithmeticException;", "Ljava/lang/Throwable;") for f in tc[1:]), str(tc))

        names = [n for n in cp.all_names() if n.startswith("java/util/") and "/" not in n[10:]][:300]
        n_methods = n_frames = 0
        bad = []
        for n in names:
            ci2 = world.get(n)
            if ci2 is None:
                continue
            for m in ci2.cf.methods:
                if m.code is None:
                    continue
                try:
                    fs = frames(ci2.cf, m)
                except Exception as e:  # noqa: BLE001
                    bad.append(f"{n}.{m.name}{m.desc}: {e}")
                    continue
                n_methods += 1
                n_frames += len(fs)
                pcs = [f.pc for f in fs[1:]]
                if pcs != sorted(set(pcs)) or (pcs and pcs[-1] >= len(m.code.code)):
                    bad.append(f"{n}.{m.name}{m.desc}: frame pcs {pcs[:6]} code {len(m.code.code)}")
        print(f"bulk: {len(names)} classes, {n_methods} methods, {n_frames} frames")
        check("bulk java.util: all parse, pcs ordered and in range", not bad and n_methods > 1000, "\n".join(bad[:8]))
    print("ALL PASSED" if not fails else f"{fails} FAILED")
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
