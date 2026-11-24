#!/usr/bin/env python3
"""Inventory of the native methods a generated program calls, and where each one is implemented.

    python3 tools/native_report.py <gen-dir> [--md report.md]

<gen-dir> is the output of monolith/rustgen.py (shard_*.rs). Every native call site is `natives::n_<mangled>` (or
`n_<mangled>` inside natives). The category comes from the Java class of the call:

* JNA        com/sun/jna/...           (JNA emulation over dlopen/libffi, rt/natives_jni.rs)
* netty/JNI  io/netty/...              (Netty's JNI transport, Rust emulation, rt/natives_jni.rs)
* JDK        java/, jdk/, sun/         (JDK natives, Rust)
* graphics   org/lwjgl/, mono/rhi/    (window, GL, audio, Rust)
* game/other everything else          (Minecraft and libraries)

This is a static inventory: it counts call sites in the generated code, not calls at run time. Run-time counting needs
a change in the generated code (one counter per native call), which is part of the next full regeneration.
"""
import argparse, collections, glob, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
RT = os.path.join(os.path.dirname(HERE), "monolith", "rt")
CALL = re.compile(r"natives::n_([A-Za-z0-9_]+)\s*\(")
DEF = re.compile(r"^pub fn n_([A-Za-z0-9_]+)\s*\(", re.M)


def category(mangled: str) -> str:
    # bozulmuş ad sınıf__metot biçimindedir, '/' -> '_' ve '$' -> '_'; önek paketi tanımlar
    if mangled.startswith("com_sun_jna"):
        return "JNA"
    if mangled.startswith("io_netty"):
        return "netty/JNI"
    if mangled.startswith("org_lwjgl") or mangled.startswith("mono_rhi"):
        return "graphics/audio"
    if mangled.startswith(("java_", "jdk_", "sun_")):
        return "JDK"
    return "game/other"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("gen")
    ap.add_argument("--md")
    a = ap.parse_args()

    defs = {}
    for f in sorted(glob.glob(os.path.join(RT, "natives*.rs")) + [os.path.join(RT, "rt.rs")]):
        src = open(f, encoding="utf-8").read()
        for m in DEF.finditer(src):
            defs.setdefault(m.group(1), os.path.basename(f))

    sites = collections.Counter()
    shards = sorted(glob.glob(os.path.join(a.gen, "shard_*.rs")))
    if not shards:
        sys.exit(f"no shard_*.rs in {a.gen}")
    for f in shards:
        for m in CALL.finditer(open(f, encoding="utf-8", errors="replace").read()):
            sites[m.group(1)] += 1
    rows = []
    for name in sorted(sites):
        rows.append((category(name), name, sites[name], defs.get(name, "MISSING")))
    tot = collections.Counter()
    impl = collections.Counter()
    for cat, name, n, where in rows:
        tot[cat] += n
        impl[(cat, where != "MISSING")] += 1

    out = []
    out.append(f"# Native inventory ({len(rows)} native methods called by the generated program)\n")
    out.append("| category | native methods | call sites | implemented in rt | missing |")
    out.append("|---|---:|---:|---:|---:|")
    for cat in ["JNA", "netty/JNI", "JDK", "graphics/audio", "game/other"]:
        n_m = sum(1 for r in rows if r[0] == cat)
        out.append(f"| {cat} | {n_m} | {tot[cat]} | {impl[(cat, True)]} | {impl[(cat, False)]} |")
    out.append("")
    out.append("## Methods (category, native, call sites, implemented in)\n")
    out.append("| category | native | call sites | file |")
    out.append("|---|---|---:|---|")
    for cat, name, n, where in rows:
        out.append(f"| {cat} | `n_{name}` | {n} | {where} |")
    text = "\n".join(out) + "\n"
    if a.md:
        open(a.md, "w", encoding="utf-8").write(text)
    print("\n".join(out[:8]))
    if a.md:
        print(f"full table: {a.md}")
    missing = [r for r in rows if r[3] == "MISSING"]
    if missing:
        print(f"{len(missing)} native methods called but not implemented")
    return 1 if missing else 0


if __name__ == "__main__":
    sys.exit(main())
