#!/usr/bin/env python3
"""Report methods/fields that the selected JDK declares on a class but the monolith overlay of
that class lacks (public, protected and package-private members; private ones are ignored).
Run after switching JDK versions: missing members turn into link errors at transpile time.

usage: python3 tools/check_overlay_api.py [ClassName ...]
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from monolith.classpath import ClassPath, jdk_jmods  # noqa: E402
from monolith.classfile import ACC_PRIVATE, ACC_SYNTHETIC  # noqa: E402
from monolith.__main__ import build_classlib  # noqa: E402


def members(cf):
    ms = {m.name + m.desc for m in cf.methods if not m.access & (ACC_PRIVATE | ACC_SYNTHETIC)}
    fs = {f.name + ":" + f.desc for f in cf.fields if not f.access & (ACC_PRIVATE | ACC_SYNTHETIC)}
    return ms, fs


def main():
    build_classlib()
    overlay = ClassPath([build_classlib()])
    jdk = ClassPath(jdk_jmods())
    names = sys.argv[1:] or sorted(overlay.all_names())
    total = 0
    for n in names:
        o, j = overlay.find(n), jdk.find(n)
        if o is None or j is None:
            continue
        om, of = members(o)
        jm, jf = members(j)
        miss = sorted(jm - om) + sorted(jf - of)
        if miss:
            total += len(miss)
            print(f"{n}: {len(miss)} missing")
            for x in miss:
                print("    " + x)
    print(f"total missing members: {total}")


if __name__ == "__main__":
    main()
