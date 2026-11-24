#!/usr/bin/env python3
"""Why do the hottest allocation classes still reach the collector? Runs the whole-program escape analysis on the Minecraft
closure (no Rust build) and, for each class of the allocation profile's top list, counts its allocation sites by outcome:

  frame      does not escape its method and is not in a loop (or lives one iteration): can live on the stack
  loop       does not escape but sits in a loop with objects that survive an iteration
  escapes    outlives the method; the reason (stored in a field, returned, passed to an unresolved call, ...) is counted

    python3 tools/escape_hot.py --cp client.jar:libs... --reflect reflect-classes.txt --out report.json [--classes A,B,...]

The top classes of tools/alloc_report.py (Vec3, AABB, BlockPos, iterators, ...) are the default list. The report names the
methods where most sites of a class escape and why: that is the work list of layer 1 of docs/GC_PLAN.md.
Takes about as long as the transpile plus the analysis (minutes); MONO_JDK_HOME must point to a JDK 25.
"""
import argparse
import collections
import json
import os
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, ROOT)

DEFAULT_CLASSES = (
    "net/minecraft/world/phys/Vec3", "net/minecraft/world/phys/AABB", "net/minecraft/core/BlockPos",
    "net/minecraft/core/BlockPos$MutableBlockPos", "net/minecraft/world/level/ChunkPos",
    "net/minecraft/world/phys/shapes/ArrayVoxelShape", "net/minecraft/world/phys/shapes/CubePointRange",
    "net/minecraft/world/phys/shapes/OffsetDoubleList", "net/minecraft/world/level/BlockCollisions",
    "java/util/Optional", "java/util/ArrayList", "java/util/ArrayList$Itr", "java/util/ImmutableCollections$ListItr",
    "java/util/RegularEnumSet$EnumSetIterator", "it/unimi/dsi/fastutil/objects/ObjectArrayList$1",
    "it/unimi/dsi/fastutil/doubles/DoubleArrayList", "com/mojang/renderpearl/api/buffers/GpuBufferSlice",
    "com/mojang/renderpearl/api/commands/RenderPass$Draw",
)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--cp", required=True, help="jars (os.pathsep separated)")
    ap.add_argument("--reflect", default="", help="reflect-classes.txt written by tools/mcbuild_rust.py")
    ap.add_argument("--out", required=True)
    ap.add_argument("--classes", default=",".join(DEFAULT_CLASSES))
    ap.add_argument("--budget", type=int, default=1800, help="seconds the escape analysis may take")
    a = ap.parse_args()

    from monolith import escape
    from monolith.rustgen import _generate_parts
    from monolith.classfile import ClassConst

    hot = [c.strip() for c in a.classes.split(",") if c.strip()]
    reflect = [l.strip() for l in open(a.reflect) if l.strip()] if a.reflect else None
    t0 = time.time()
    world, prog, _cp = _generate_parts("net.minecraft.client.main.Main", os.path.join(ROOT, "monolith", "rt", "natives.rs"),
                                       [p for p in a.cp.split(os.pathsep) if p], no_lwjgl=True, reflect_classes=reflect,
                                       analyze_only=True)
    t_prog = time.time() - t0
    res = escape.analyze(prog, a.budget)
    if res is None:
        print("the escape analysis hit its time budget: no result")
        return 1
    stats = {c: {"sites": 0, "frame": 0, "loop": 0, "escapes": 0, "reasons": collections.Counter(),
                 "worst_methods": collections.Counter()} for c in hot}
    for (cn, key), r in res.methods.items():
        if not r.sites:
            continue
        ci = world.get(cn)
        m = ci.cf.method(key.split("(")[0], "(" + key.split("(", 1)[1]) if ci is not None and "(" in key else None
        if m is None or m.code is None:
            continue
        by_pc = {i.pc: i for i in prog.insns(ci, m)}
        for pc, kind in r.sites.items():
            if kind != "new":
                continue
            ins = by_pc.get(pc)
            arg = getattr(ins, "arg", None)
            name = arg.name if isinstance(arg, ClassConst) else arg
            if name not in stats:
                continue
            s = stats[name]
            s["sites"] += 1
            if pc in r.escaped:
                s["escapes"] += 1
                why = r.escaped[pc]
                why = why.split(" of ")[0] if why.startswith("arg of") else why
                s["reasons"][why] += 1
                s["worst_methods"][f"{cn}.{key} [{why}]"] += 1
            elif pc in r.loop_pcs and pc not in r.iter_pcs:
                s["loop"] += 1
            else:
                s["frame"] += 1
    doc = {"seconds_program": round(t_prog), "seconds_analysis": round(res.seconds), "classes": {}}
    for c, s in stats.items():
        doc["classes"][c] = {"sites": s["sites"], "frame": s["frame"], "loop": s["loop"], "escapes": s["escapes"],
                             "reasons": dict(s["reasons"].most_common(8)),
                             "top_escaping_methods": [f"{n} x{k}" for n, k in s["worst_methods"].most_common(8)]}
    with open(a.out, "w", encoding="utf-8") as fh:
        json.dump(doc, fh, indent=1)
    print(f"{'class':58s} {'sites':>6s} {'frame':>6s} {'loop':>5s} {'escape':>6s}  main reason")
    for c, d in doc["classes"].items():
        top = next(iter(d["reasons"]), "")
        print(f"{c:58s} {d['sites']:6d} {d['frame']:6d} {d['loop']:5d} {d['escapes']:6d}  {top}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
