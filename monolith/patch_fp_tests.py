#!/usr/bin/env python3
"""Tests of the patch fingerprint (monolith/patches.py `fingerprint`, tools/patch_fingerprints.py). No Rust build, a few seconds.

1. The fingerprint does not change when unrelated code is added to the class (constant-pool numbers and line numbers move).
2. It changes when the method body changes (a constant, an operator, a called method).
3. PatchSet.validate accepts a matching fingerprint, rejects a stale one with an explanation, and ignores a patch without one.
Usage: MONO_JDK_HOME=<jdk 25> python3 -m monolith.patch_fp_tests
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


def check(name: str, ok: bool, detail: str = "") -> None:
    global fails
    print(("PASS " if ok else "FAIL ") + name + ("" if ok else "\n" + detail))
    fails += 0 if ok else 1


def source(extra: str = "", body: str = "return x * 3 + 1;", helper: str = "static int h(int v) { return v + 1; }") -> str:
    return f"public class Fp {{\n{extra}\n    {helper}\n    public static int f(int x) {{ {body} }}\n}}\n"


def compile_and_fingerprint(work: str, tag: str, src: str) -> tuple[str, object, object]:
    from monolith.classpath import ClassPath, jdk_jmods
    from monolith.patches import fingerprint
    from monolith.world import World
    d = os.path.join(work, tag)
    os.makedirs(d)
    path = os.path.join(d, "Fp.java")
    with open(path, "w") as fh:
        fh.write(src)
    r = subprocess.run([f"{JDK}/bin/javac", "-d", d, path], capture_output=True, text=True, env=ENV)
    if r.returncode:
        sys.exit("javac failed:\n" + r.stderr[-800:])
    world = World(ClassPath([d] + jdk_jmods(), release=25, boot_start=1))
    ci = world.get("Fp")
    return fingerprint(ci, ci.cf.method("f", "(I)I")), world, ci


def main() -> int:
    from monolith.patches import Patch, PatchError, PatchSet
    os.environ["MONO_JDK_HOME"] = JDK
    with tempfile.TemporaryDirectory() as work:
        base, _, _ = compile_and_fingerprint(work, "base", source())
        check("fingerprint is 16 hex digits", len(base) == 16 and all(c in "0123456789abcdef" for c in base), base)

        moved, _, _ = compile_and_fingerprint(work, "moved", source(
            extra="    static String unrelated = \"another constant\";\n    static long other(long a) { return a * 31; }\n"))
        check("unrelated code added to the class: same fingerprint", moved == base, f"{base} != {moved}")

        again, _, _ = compile_and_fingerprint(work, "again", source())
        check("recompiling the same source: same fingerprint", again == base, f"{base} != {again}")

        const, _, _ = compile_and_fingerprint(work, "const", source(body="return x * 3 + 2;"))
        check("changed constant: different fingerprint", const != base)
        oper, _, _ = compile_and_fingerprint(work, "oper", source(body="return x * 3 - 1;"))
        check("changed operator: different fingerprint", oper != base)
        call, _, _ = compile_and_fingerprint(work, "call", source(body="return h(x) * 3;"))
        check("different called method: different fingerprint", call != base)

        # PatchSet.validate, yamanın YAZILMADIĞI sürüme karşı
        fp_new, world, _ = compile_and_fingerprint(work, "newver", source(body="return x * 4 + 1;"))

        def pset(fp: str) -> PatchSet:
            return PatchSet({}, [Patch("Fp", "f", "(I)I", "p.rs", "f", False, True, fp)])

        try:
            pset(base).validate(world)
            check("stale fingerprint is rejected", False, "validate accepted a patch written for another body")
        except PatchError as e:
            msg = str(e)
            check("stale fingerprint is rejected", "method body changed" in msg and base in msg and fp_new in msg, msg)
        try:
            pset(fp_new).validate(world)
            check("matching fingerprint is accepted", True)
        except PatchError as e:
            check("matching fingerprint is accepted", False, str(e))
        try:
            pset("").validate(world)
            check("patch without a fingerprint is not checked", True)
        except PatchError as e:
            check("patch without a fingerprint is not checked", False, str(e))
    print("ALL PASSED" if not fails else f"{fails} FAILED")
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
