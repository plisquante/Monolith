#!/usr/bin/env python3
"""Tests of monolith/mir.py (work package C1): structural verification of the MIR of a compiled sample and of every method of
java.util (stack shuffles, wide types, switches, handlers, constructors). A few seconds.
Usage: MONO_JDK_HOME=<jdk 25> python3 -m monolith.mir_tests
"""
import os
import subprocess
import sys
import tempfile
import collections

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, ROOT)
JDK = os.environ.get("MONO_JDK_HOME", "/opt/tools/jdk25")
ENV = {k: v for k, v in os.environ.items() if k not in ("JAVA_TOOL_OPTIONS", "_JAVA_OPTIONS")}
fails = 0

SRC = """
public class Mt {
    int f; static long sl;
    Mt(int a) { this.f = a > 0 ? a : -a; }
    static long loop(int n, String s) { long acc = 0; for (int i = 0; i < n; i++) { acc += i; } return s == null ? acc : acc + s.length(); }
    static double tc(double d, int[] a) { try { return d / a[0]; } catch (ArithmeticException e) { return 0.0; } finally { d += 1; } }
    static int sw(int k) { switch (k) { case 1: return 10; case 5: return 50; case 9: return 90; default: return -1; } }
    static void wide(long[] a, double[] b, int i) { a[i] += 2; b[i]++; sl = a[i]--; }
    static Object mk(int n) { return new Mt(n); }
    synchronized void sync() { f++; }
}
"""


def check(name, ok, detail=""):
    global fails
    print(("PASS " if ok else "FAIL ") + name + ("" if ok else "\n" + detail))
    fails += 0 if ok else 1


def main() -> int:
    os.environ["MONO_JDK_HOME"] = JDK
    from monolith import mir
    from monolith.classpath import ClassPath, jdk_jmods
    from monolith.program import Program
    from monolith.world import World
    with tempfile.TemporaryDirectory() as d:
        with open(os.path.join(d, "Mt.java"), "w") as fh:
            fh.write(SRC)
        r = subprocess.run([f"{JDK}/bin/javac", "-d", d, os.path.join(d, "Mt.java")], capture_output=True, text=True, env=ENV)
        if r.returncode:
            sys.exit("javac failed:\n" + r.stderr[-800:])
        cp = ClassPath([d] + jdk_jmods(), release=25, boot_start=1)
        world = World(cp)
        prog = Program(world)
        ci = world.get("Mt")
        for m in ci.cf.methods:
            if m.code is None:
                continue
            try:
                fn = mir.build(ci, m, prog.insns(ci, m))
                mir.verify(fn)
                check(f"sample {m.name}{m.desc}", True)
            except mir.MirError as e:
                check(f"sample {m.name}{m.desc}", False, str(e))
        lp = mir.build(ci, ci.cf.method("loop", "(ILjava/lang/String;)J"), prog.insns(ci, ci.cf.method("loop", "(ILjava/lang/String;)J")))
        check("loop: a loop header block has the long accumulator as a parameter",
              any("long" in [p.ty for p in b.params] and len(b.params) >= 3 for b in lp.blocks.values()), mir.dump(lp))
        sw = mir.build(ci, ci.cf.method("sw", "(I)I"), prog.insns(ci, ci.cf.method("sw", "(I)I")))
        check("switch: terminator with 3 keys + default", any(b.term.kind == "switch" and len(b.term.keys) >= 3 for b in sw.blocks.values()))
        tc = mir.build(ci, ci.cf.method("tc", "(D[I)D"), prog.insns(ci, ci.cf.method("tc", "(D[I)D")))
        check("try/catch: some instruction has an exception edge", any(i.exc for b in tc.blocks.values() for i in b.insns), mir.dump(tc))

        names = [n for n in cp.all_names() if n.startswith("java/util/") and "/" not in n[10:]][:400]
        ok = fb = 0
        reasons = collections.Counter()
        bad = []
        for n in names:
            c2 = world.get(n)
            if c2 is None:
                continue
            for m in c2.cf.methods:
                if m.code is None:
                    continue
                try:
                    fn = mir.build(c2, m, prog.insns(c2, m))
                    mir.verify(fn)
                    ok += 1
                except mir.MirUnsupported as e:
                    fb += 1
                    reasons[str(e)[:40]] += 1
                except mir.MirError as e:
                    bad.append(f"{n}.{m.name}{m.desc}: {e}")
                except Exception as e:  # noqa: BLE001
                    bad.append(f"{n}.{m.name}{m.desc}: {type(e).__name__} {e}")
        print(f"bulk: built+verified {ok}, unsupported {fb} {dict(reasons)}, errors {len(bad)}")
        check("bulk java.util: no MIR errors", not bad and ok > 3000, "\n".join(bad[:10]))
    print("ALL PASSED" if not fails else f"{fails} FAILED")
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
