#!/usr/bin/env python3
"""Java residue report: how much of the program that runs is still translated Java bytecode, and where it comes from.

    python3 -m monolith.residue --cp <jars> --main <Class> [--reflect-class @file] [--no-lwjgl] [--json out.json]

Counts the methods the translator keeps (reachable from the entry point) per origin: the game, each library, the JDK,
our own Java overlay. Bytecode bytes are the size of the Java method bodies: the measure of what is left to replace
with hand-written Rust (patches, `patches/rust/manifest.json`). Progress = this number going down.
"""
import argparse, collections, json, os, sys


def origin_of(label: str, name: str, ours: tuple[str, ...]) -> str:
    base = os.path.basename(label.rstrip("/"))
    if any(label.startswith(o) or o in label for o in ours):
        return "ours: Java overlay (classlib / LWJGL replacement / bridge)"
    if base == "client.jar":
        return "game: Minecraft (client.jar)"
    if base.endswith(".jar"):
        return "library: " + base.rsplit("-", 1)[0] if base[:-4].rsplit("-", 1)[-1][:1].isdigit() else "library: " + base[:-4]
    return "JDK: " + (base or label)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--cp", action="append", required=True)
    ap.add_argument("--main", required=True)
    ap.add_argument("--reflect-class", action="append", default=[])
    ap.add_argument("--no-lwjgl", action="store_true")
    ap.add_argument("--json")
    ap.add_argument("--top", type=int, default=25)
    a = ap.parse_args()
    from . import rustgen as R
    cp_dirs = [os.path.abspath(p) for item in a.cp for p in item.split(os.pathsep) if p]
    ref = []
    for spec in a.reflect_class:
        ref.extend(open(spec[1:], encoding="utf-8").read().split() if spec.startswith("@") else [spec])
    rt_dir = os.path.join(R.ROOT, "monolith", "rt")
    world, prog, cp = R._generate_parts(a.main, os.path.join(rt_dir, "natives.rs"), cp_dirs, False, no_lwjgl=a.no_lwjgl,
                                        reflect_classes=ref, shards=1, analyze_only=True)
    ours = (os.path.join(R.ROOT, "classlib"), os.path.join(R.ROOT, "runtime"), "monolith_builtin", "mono_nolwjgl",
            "java_nolwjgl", os.path.join("runtime", "java"))
    groups = collections.defaultdict(lambda: [set(), 0, 0])
    pkgs = collections.defaultdict(lambda: [0, 0])
    total_m = total_b = 0
    for (cn, key) in prog.reachable:
        ci = world.classes.get(cn)
        if ci is None:
            continue
        mi = next((m for m in ci.cf.methods if m.name + m.desc == key), None)
        if mi is None or mi.code is None:
            continue   # native or abstract: no Java body
        size = len(mi.code.code)
        label = cp.origin.get(cn, "")
        g = origin_of(label, cn, ours)
        groups[g][0].add(cn); groups[g][1] += 1; groups[g][2] += size
        total_m += 1; total_b += size
        if g.startswith("game"):
            pk = "/".join(cn.split("/")[:4])
            pkgs[pk][0] += 1; pkgs[pk][1] += size
    print(f"Java residue: {total_m} methods with a Java body, {total_b/1024:.0f} KiB of bytecode\n")
    print(f"{'origin':58} {'classes':>8} {'methods':>9} {'KiB':>8} {'share':>7}")
    for g, (cs, ms, b) in sorted(groups.items(), key=lambda kv: -kv[1][2]):
        print(f"{g[:58]:58} {len(cs):8d} {ms:9d} {b/1024:8.0f} {100*b/total_b:6.1f}%")
    print("\nlargest game packages (bytecode KiB):")
    for pk, (ms, b) in sorted(pkgs.items(), key=lambda kv: -kv[1][1])[:a.top]:
        print(f"  {pk:55} {ms:7d} methods {b/1024:7.0f} KiB")
    if a.json:
        json.dump({"methods": total_m, "bytes": total_b,
                   "origins": {g: {"classes": len(v[0]), "methods": v[1], "bytes": v[2]} for g, v in groups.items()},
                   "game_packages": {k: {"methods": v[0], "bytes": v[1]} for k, v in pkgs.items()}}, open(a.json, "w"), indent=1)
    return 0


if __name__ == "__main__":
    sys.exit(main())
