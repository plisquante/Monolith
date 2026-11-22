#!/usr/bin/env python3
"""Tests of for-each iterator recycling (monolith/rustgen.py find_iterator_scopes, gc.rs `recycle`).

1. Iter1 prints exactly what HotSpot prints, with recycling on and off (MONO_RECYCLE=0).
2. The loops are really recognized: the generated code calls rt::recycle_scope.
3. It works: with recycling on, almost no ArrayList$Itr reaches the heap (allocation profile); off, one per loop.
Usage: python3 monolith/recycle_tests.py
"""
import json, os, subprocess, sys, tempfile

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
    r = subprocess.run([f"{JDK}/bin/javac", "-d", cdir, os.path.join(HERE, "tests", "recycle", "Iter1.java")], capture_output=True, text=True, env=ENV)
    if r.returncode:
        sys.exit("javac failed:\n" + r.stderr[-800:])
    java = subprocess.run([f"{JDK}/bin/java", "-cp", cdir, "Iter1"], capture_output=True, text=True, env=ENV)
    out = os.path.join(work, "out")
    g = subprocess.run([sys.executable, "-m", "monolith.rustgen", "--cp", cdir, "--main", "Iter1", "-o", out, "--shards", "4", "--shard-jobs", "2",
                        "--opt", "2", "--compile"], cwd=ROOT, capture_output=True, text=True, env=ENV)
    if g.returncode:
        sys.exit("build failed:\n" + g.stderr[-1500:])
    exe = os.path.join(out, "prog")
    on = subprocess.run([exe], capture_output=True, text=True, env=dict(ENV, MONO_RECYCLE="1"))
    off = subprocess.run([exe], capture_output=True, text=True, env=dict(ENV, MONO_RECYCLE="0"))
    check("recycling on == HotSpot", on.stdout == java.stdout and on.returncode == java.returncode, f"{on.stderr[-600:]}\n--- java\n{java.stdout}\n--- rust\n{on.stdout}")
    check("recycling off == HotSpot", off.stdout == java.stdout and off.returncode == java.returncode, off.stdout[-500:])
    src = "".join(open(os.path.join(out, f)).read() for f in os.listdir(out) if f.startswith("shard_") and f.endswith(".rs"))
    check("loops are recognized (rt::recycle_scope emitted)", src.count("rt::recycle_scope(") >= 10, f"{src.count('rt::recycle_scope(')} sites")
    check("recyclable iterators allocate through rt::alloc_recyclable", "rt::alloc_recyclable(" in src)

    def itr_heap(recycle):
        prof = os.path.join(work, f"p{recycle}.json")
        subprocess.run([exe], capture_output=True, text=True, env=dict(ENV, MONO_RECYCLE=recycle, MONO_ALLOC_PROFILE=prof, MONO_ALLOC_DUMP_SECS="1"))
        P = json.load(open(prof))
        return {c["name"]: (c["an"], c["arn"]) for c in P["classes"]}
    a1, a0 = itr_heap("1"), itr_heap("0")
    n_on = a1.get("java/util/ArrayList$Itr", (0, 0)); n_off = a0.get("java/util/ArrayList$Itr", (0, 0))
    heap_on, heap_off = n_on[0] - n_on[1], n_off[0] - n_off[1]
    # 5 of the loop shapes are excluded on purpose (return inside the loop, exception, use after the loop, alias, stored
    # iterator): 5 x 3000 rounds stay on the heap, every other loop is served from the pool
    check(f"ArrayList$Itr: {n_on[1]:,} of {n_on[0]:,} served by recycling; heap {heap_on:,} (expected ~15,000 from the excluded loops)",
          heap_off == n_off[0] and n_on[1] >= 80_000 and 14_000 <= heap_on <= 16_000, f"on {n_on} off {n_off}")
print("FAILED" if fails else "ALL PASSED")
sys.exit(1 if fails else 0)
