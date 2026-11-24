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


SEM = """
public class Sem {
    static int fib(int n) { int a = 0, b = 1; for (int i = 0; i < n; i++) { int t = a + b; a = b; b = t; } return a; }
    static long collatz(long n) { long steps = 0; while (n != 1) { n = (n & 1) == 0 ? n >> 1 : 3 * n + 1; steps++; } return steps; }
    static int gcd(int a, int b) { while (b != 0) { int t = a % b; a = b; b = t; } return a; }
    static int incs(int i) { int j = i++ + ++i; return j * 100 + i; }
    static long wide(int n) { long[] a = new long[n + 3]; double[] d = new double[n + 3]; long s = 0;
        for (int i = 0; i < a.length; i++) { a[i] += i * 3L; a[i]++; d[i] = a[i] / 7.0; s += a[i]--; s ^= (long) d[i]; } return s + a[n]; }
    static int sw(int k) { switch (k) { case 1: return 10; case 5: return 50; case 9: return 90; case -3: return 33; default: return -1; } }
    static int shifts(int x, int s) { return (x << s) ^ (x >> s) ^ (x >>> s) ^ (int) ((long) x << s) ^ (int) ((long) x >>> s); }
    static int dd(int a, int b) { try { return a / b + a % b; } catch (ArithmeticException e) { return -999; } }
    static int arr(int i) { int[] a = new int[4]; try { a[i] = 7; return a[i] + a.length; } catch (ArrayIndexOutOfBoundsException e) { return -1; } finally { a[0]++; } }
    static double mix(int i, long l, float f, double d) { return (i * f + l / (d + 1.5)) % 5.5 - (float) l; }
    static int conv(double d, float f, long l) { return (int) d + (int) f + (int) l + (byte) l + (short) d + (char) f; }
    static long cmp(double a, double b) { return (a < b ? 1 : 0) + (a > b ? 2 : 0) + (a == b ? 4 : 0) + (Double.compare(a, b) * 8L); }
    static int nested(int n) { int c = 0; for (int i = 0; i < n; i++) for (int j = i; j < n; j++) { if ((i ^ j) % 3 == 0) continue; c += i * j; } return c; }
    static int sq(int x) { return x * x; }
    static int clamp(int v, int lo, int hi) { return v < lo ? lo : v > hi ? hi : v; }
    static long poly(int x) { return sq(x) + 3L * clamp(x, -5, 5) - sq(clamp(x, 0, 9)); }
    static long dupx(long[] a, int i) { long x = a[i]++; long y = ++a[i]; a[i] <<= 1; return x * 31 + y + a[i]; }
    public static void main(String[] x) {
        for (int n : new int[]{0, 1, 2, 10, 46}) System.out.println("fib " + n + " " + fib(n));
        for (long n : new long[]{1, 7, 27, 97}) System.out.println("collatz " + n + " " + collatz(n));
        System.out.println("gcd " + gcd(84, 36) + " " + gcd(17, 5) + " " + gcd(0, 9));
        for (int n : new int[]{0, 5, -7, 2147483647}) System.out.println("incs " + n + " " + incs(n));
        for (int n : new int[]{0, 1, 5, 40}) System.out.println("wide " + n + " " + wide(n));
        for (int n : new int[]{1, 5, 9, -3, 4, 100}) System.out.println("sw " + n + " " + sw(n));
        for (int[] p : new int[][]{{1, 3}, {-1, 5}, {-2147483648, 31}, {123456789, 33}, {-5, 0}}) System.out.println("shifts " + p[0] + " " + p[1] + " " + shifts(p[0], p[1]));
        for (int[] p : new int[][]{{7, 2}, {-7, 2}, {7, -2}, {5, 0}, {-2147483648, -1}}) System.out.println("dd " + p[0] + " " + p[1] + " " + dd(p[0], p[1]));
        for (int n : new int[]{0, 3, 4, -1}) System.out.println("arr " + n + " " + arr(n));
        System.out.println("mix " + Double.doubleToLongBits(mix(3, 1234567890123L, 1.25f, 0.5)) + " " + Double.doubleToLongBits(mix(-2, -99L, 3.5f, -1.5)));
        System.out.println("conv " + conv(1e20, -3.7f, 5000000000L) + " " + conv(-2.5, Float.NaN, -1L) + " " + conv(70000.9, 65537.5f, 129L));
        System.out.println("cmp " + cmp(1.0, 2.0) + " " + cmp(2.0, 1.0) + " " + cmp(1.0, 1.0) + " " + cmp(Double.NaN, 1.0) + " " + cmp(0.0, -0.0));
        for (int n : new int[]{0, 1, 9, 30}) System.out.println("nested " + n + " " + nested(n));
        for (int n : new int[]{-7, 0, 3, 12}) System.out.println("poly " + n + " " + poly(n));
        System.out.println("dupx " + dupx(new long[]{5, 6, 7}, 1) + " " + dupx(new long[]{-9, 1L << 40, 3}, 1));
    }
}
"""


def check(name, ok, detail=""):
    global fails
    print(("PASS " if ok else "FAIL ") + name + ("" if ok else "\n" + detail))
    fails += 0 if ok else 1


def _dbits(x):
    import struct
    if x != x:
        return 0x7ff8000000000000
    return struct.unpack(">q", struct.pack(">d", x))[0]


def semantic(d, world, prog):
    """Interpret the MIR of Sem and compare each printed line with the JVM's output."""
    from monolith import mir
    from monolith.mir_interp import Interp
    with open(os.path.join(d, "Sem.java"), "w") as fh:
        fh.write(SEM)
    r = subprocess.run([f"{JDK}/bin/javac", "-d", d, os.path.join(d, "Sem.java")], capture_output=True, text=True, env=ENV)
    if r.returncode:
        sys.exit("javac failed:\n" + r.stderr[-800:])
    jvm = subprocess.run([f"{JDK}/bin/java", "-cp", d, "Sem"], capture_output=True, text=True, env=ENV).stdout.splitlines()
    world2 = type(world)(type(world.cp)([d] + jdk_jmods_(), release=25, boot_start=1))
    ci = world2.get("Sem")
    from monolith.program import Program
    p2 = Program(world2)
    fns = {}
    for m in ci.cf.methods:
        if m.code is not None and m.name != "main":
            fns[(m.name, m.desc)] = mir.build(ci, m, p2.insns(ci, m))
            mir.verify(fns[(m.name, m.desc)])
    it = Interp(fns)
    it.owner = "Sem"
    call = lambda n, *a: it.call(n, next(k[1] for k in fns if k[0] == n), list(a))  # noqa: E731
    mine = []
    for n in (0, 1, 2, 10, 46):
        mine.append(f"fib {n} {call('fib', n)}")
    for n in (1, 7, 27, 97):
        mine.append(f"collatz {n} {call('collatz', n)}")
    mine.append(f"gcd {call('gcd', 84, 36)} {call('gcd', 17, 5)} {call('gcd', 0, 9)}")
    for n in (0, 5, -7, 2147483647):
        mine.append(f"incs {n} {call('incs', n)}")
    for n in (0, 1, 5, 40):
        mine.append(f"wide {n} {call('wide', n)}")
    for n in (1, 5, 9, -3, 4, 100):
        mine.append(f"sw {n} {call('sw', n)}")
    for a, b in ((1, 3), (-1, 5), (-2147483648, 31), (123456789, 33), (-5, 0)):
        mine.append(f"shifts {a} {b} {call('shifts', a, b)}")
    for a, b in ((7, 2), (-7, 2), (7, -2), (5, 0), (-2147483648, -1)):
        mine.append(f"dd {a} {b} {call('dd', a, b)}")
    for n in (0, 3, 4, -1):
        mine.append(f"arr {n} {call('arr', n)}")
    from monolith.mir_interp import f32
    mine.append(f"mix {_dbits(call('mix', 3, 1234567890123, f32(1.25), 0.5))} {_dbits(call('mix', -2, -99, f32(3.5), -1.5))}")
    mine.append(f"conv {call('conv', 1e20, f32(-3.7), 5000000000)} {call('conv', -2.5, float('nan'), -1)} {call('conv', 70000.9, f32(65537.5), 129)}")
    nan = float("nan")
    mine.append("cmp " + " ".join(str(call("cmp", a, b)) for a, b in ((1.0, 2.0), (2.0, 1.0), (1.0, 1.0), (nan, 1.0), (0.0, -0.0))))
    for n in (0, 1, 9, 30):
        mine.append(f"nested {n} {call('nested', n)}")
    for n in (-7, 0, 3, 12):
        mine.append(f"poly {n} {call('poly', n)}")
    mine.append(f"dupx {call('dupx', [5, 6, 7], 1)} {call('dupx', [-9, 1 << 40, 3], 1)}")
    bad = [f"jvm: {a}\nmir: {b}" for a, b in zip(jvm, mine) if a != b]
    check(f"MIR interpreter == JVM on {len(jvm)} result lines", not bad and len(jvm) == len(mine), "\n".join(bad[:6]) + f"\n({len(jvm)} vs {len(mine)})")
    # the same after inlining (mir_opt.inline_leaf): poly calls sq and clamp, which are leaves
    from monolith import mir_opt
    from monolith.rustgen import Gen
    g = Gen(world2, p2, os.path.join(ROOT, "monolith", "rt", "natives.rs"))
    n_inl = 0
    for k, f in fns.items():
        n_inl += mir_opt.inline_leaf(f, g, ci)
        mir.verify(f)
    pm = [f"poly {n} {call('poly', n)}" for n in (-7, 0, 3, 12)]
    check(f"inline_leaf: {n_inl} calls inlined, poly still == JVM", n_inl >= 4 and pm == [x for x in jvm if x.startswith("poly ")],
          f"{n_inl} inlined; {pm}")


def jdk_jmods_():
    from monolith.classpath import jdk_jmods
    return jdk_jmods()


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

        semantic(d, world, prog)

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
