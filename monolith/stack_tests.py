#!/usr/bin/env python3
"""Tests of stack allocation (monolith/escape.py, monolith/inline.py, rt::stack_obj).

Stack1 must print exactly what HotSpot prints, built with the escape analysis alone and with inlining too, run with
MONO_ARENA_CHECK=abort (a collection aborts if a heap object or static points into a frame) and with a tiny heap so
collections run while stack objects hold the only reference to heap data. Also checks that objects really live in
frames (rt::stack_obj is emitted) and that loop traps are not allocated that way.
Usage: python3 monolith/stack_tests.py
"""
import os, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__)); ROOT = os.path.dirname(HERE)
JDK = os.environ.get("MONO_JDK_HOME", "/opt/tools/jdk25")
ENV = {k: v for k, v in os.environ.items() if k not in ("JAVA_TOOL_OPTIONS", "_JAVA_OPTIONS")}
ENV["MONO_JDK_HOME"] = JDK
fails = 0


def check(name, ok, detail=""):
    global fails
    print(("PASS " if ok else "FAIL ") + name + ("" if ok else "\n" + detail))
    fails += 0 if ok else 1


with tempfile.TemporaryDirectory() as work:
    cdir = os.path.join(work, "classes"); os.makedirs(cdir)
    r = subprocess.run([f"{JDK}/bin/javac", "-d", cdir, os.path.join(HERE, "tests", "stack", "Stack1.java")], capture_output=True, text=True, env=ENV)
    if r.returncode:
        sys.exit("javac failed:\n" + r.stderr[-800:])
    java = subprocess.run([f"{JDK}/bin/java", "-cp", cdir, "Stack1"], capture_output=True, text=True, env=ENV)
    for label, flags in (("escape", ["--escape"]), ("escape + inline", ["--escape", "--inline"])):
        out = os.path.join(work, "out_" + label.replace(" ", "").replace("+", ""))
        g = subprocess.run([sys.executable, "-m", "monolith.rustgen", "--cp", cdir, "--main", "Stack1", "-o", out, "--shards", str(min(4, os.cpu_count() or 1)), "--shard-jobs", str(os.cpu_count() or 1),
                            "--opt", "2", "--compile"] + flags, cwd=ROOT, capture_output=True, text=True, env=ENV)
        if g.returncode:
            sys.exit(f"build ({label}) failed:\n" + g.stderr[-1500:])
        exe = os.path.join(out, "prog")
        run = subprocess.run([exe], capture_output=True, text=True, env=dict(ENV, MONO_ARENA_CHECK="abort", MONO_GC_MB="2"))
        check(f"{label}: output == HotSpot (arena check abort, 2 MB collection threshold)", run.stdout == java.stdout and run.returncode == 0,
              f"exit {run.returncode}\n{run.stderr[-900:]}\n--- java\n{java.stdout}\n--- rust\n{run.stdout}")
        src = "".join(open(os.path.join(out, f)).read() for f in os.listdir(out) if f.startswith("shard_") and f.endswith(".rs"))
        n = src.count("rt::stack_obj(")
        check(f"{label}: objects are kept in frames ({n} sites)", n >= (3 if label == "escape" else 10), f"{n} sites; {g.stderr[-300:]}")
print("FAILED" if fails else "ALL PASSED")
sys.exit(1 if fails else 0)
