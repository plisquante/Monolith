#!/usr/bin/env python3
"""Prints, checks or writes the bytecode fingerprint of every method that a Rust patch replaces.

    python3 tools/patch_fingerprints.py --cp client.jar:lib1.jar:...            # print, exit 1 on a mismatch
    python3 tools/patch_fingerprints.py --cp client.jar:... --write             # store the current fingerprints in the manifest

A fingerprint (monolith/patches.py `fingerprint`) identifies the body of the method a patch was written against. With one in
the manifest the build stops, loudly, when a Minecraft update changes that method, instead of silently keeping a patch that
no longer matches. Review the patch against the new bytecode first, then run with --write.
Patches whose class is not on the class path are listed as skipped and keep their stored value.
"""
import argparse
import json
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, ROOT)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--cp", required=True, help="jars and class directories (os.pathsep separated), the program the patches apply to")
    ap.add_argument("--manifest", default=os.path.join(ROOT, "patches", "rust", "manifest.json"))
    ap.add_argument("--write", action="store_true", help="store the current fingerprints in the manifest")
    ap.add_argument("--no-lwjgl", action="store_true", default=True, help="use the runtime/java_nolwjgl overlay (the only supported mode)")
    a = ap.parse_args()

    from monolith.classlib import build_classlib
    from monolith.classpath import ClassPath, builtin_classes, builtin_nolwjgl_classes, jdk_jmods
    from monolith.patches import fingerprint
    from monolith.world import World

    pre = [x for x in (builtin_classes(), builtin_nolwjgl_classes() if a.no_lwjgl else None) if x]
    cp_dirs = [p for p in a.cp.split(os.pathsep) if p]
    cp = ClassPath(pre + cp_dirs + [build_classlib(False)] + jdk_jmods(), release=25, release_overrides={"lwjgl": 11},
                   boot_start=len(pre) + len(cp_dirs))
    world = World(cp)

    with open(a.manifest, encoding="utf-8") as fh:
        doc = json.load(fh)
    bad = 0
    changed = 0
    for e in doc.get("patches", []):
        where = f"{e['class']}.{e['method']}{e['desc']}"
        ci = world.get(e["class"])
        if ci is None:
            print(f"skipped   {where}  (class not on the class path)")
            continue
        m = ci.cf.method(e["method"], e["desc"])
        if m is None or m.code is None:
            print(f"ERROR     {where}  (no such method with a body)")
            bad += 1
            continue
        have = fingerprint(ci, m)
        stored = e.get("fingerprint", "")
        if a.write:
            if stored != have:
                e["fingerprint"] = have
                changed += 1
            print(f"{'written  ' if stored != have else 'unchanged'} {have}  {where}")
        elif not stored:
            print(f"no value  {have}  {where}")
        elif stored == have:
            print(f"ok        {have}  {where}")
        else:
            print(f"MISMATCH  stored {stored}, now {have}  {where}")
            bad += 1
    if a.write and changed:
        with open(a.manifest, "w", encoding="utf-8") as fh:
            json.dump(doc, fh, indent=2, ensure_ascii=False)
            fh.write("\n")
        print(f"{changed} fingerprint(s) written to {a.manifest}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
