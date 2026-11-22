#!/usr/bin/env python3
"""Resolve a J2Z_PROF sample file (raw u64 instruction pointers) against the executable's symbol table.

usage: prof_report.py <samples.bin> <executable> [top-N]
Prints self time per function (the function containing the interrupted instruction) and per class
(generated names C_pkg_Class.m_name_hash are grouped by class)."""
import array
import bisect
import collections
import re
import subprocess
import sys


def main():
    samples, exe = sys.argv[1], sys.argv[2]
    top = int(sys.argv[3]) if len(sys.argv) > 3 else 40
    a = array.array("Q")
    with open(samples, "rb") as fh:
        data = fh.read()
    a.frombytes(data[: len(data) // 8 * 8])
    syms = []
    out = subprocess.run(["nm", "-n", "--defined-only", exe], capture_output=True, text=True).stdout
    for ln in out.splitlines():
        p = ln.split(None, 2)
        if len(p) == 3 and p[1] in "tTwW":
            syms.append((int(p[0], 16), p[2]))
    addrs = [s[0] for s in syms]
    cnt = collections.Counter()
    for pc in a:
        i = bisect.bisect_right(addrs, pc) - 1
        cnt[syms[i][1] if i >= 0 else "?"] += 1
    total = sum(cnt.values())
    strip = lambda n: re.sub(r"_[0-9a-f]{8}$", "", n)
    print(f"{total} samples (about {total / 1000:.1f} s of CPU time)")
    print("--- self time per function")
    for n, c in cnt.most_common(top):
        print(f"{100 * c / total:5.1f}%  {c:7d}  {strip(n)}")
    cls = collections.Counter()
    for n, c in cnt.items():
        m = re.match(r"(C_[A-Za-z0-9_]+?)\.m_", n)
        cls[m.group(1) if m else (n.split(".")[0] if "." in n else n)] += c
    print("--- self time per class / runtime function")
    for n, c in cls.most_common(top):
        print(f"{100 * c / total:5.1f}%  {c:7d}  {n}")


if __name__ == "__main__":
    main()
