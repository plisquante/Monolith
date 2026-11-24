#!/usr/bin/env python3
"""Tests of the download guards of tools/mcbuild_common.py (require_https, safe_join, safe_name, the asset hash check).
No network: every case is decided before a connection would be made. Usage: python3 -m monolith.fetch_guard_tests
"""
import os
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(os.path.dirname(HERE), "tools"))
fails = 0


def check(name: str, ok: bool, detail: str = "") -> None:
    global fails
    print(("PASS " if ok else "FAIL ") + name + ("" if ok else "\n" + detail))
    fails += 0 if ok else 1


def refused(fn, *a) -> bool:
    try:
        fn(*a)
    except SystemExit:
        return True
    return False


def main() -> int:
    import mcbuild_common as mb
    for u in ("file:///etc/passwd", "ftp://example.org/x", "http://piston-meta.mojang.com/x", "//x", "", "javascript:1"):
        check(f"non-https refused: {u!r}", refused(mb.require_https, u))
    check("https accepted", mb.require_https("https://piston-meta.mojang.com/x") == "https://piston-meta.mojang.com/x")
    with tempfile.TemporaryDirectory() as td:
        # fetch(), diske dokunmadan önce reddetmeli
        dest = os.path.join(td, "never", "x.jar")
        check("fetch refuses file:// before creating anything", refused(mb.fetch, "file:///etc/passwd", dest)
              and not os.path.exists(os.path.dirname(dest)))

        root = os.path.join(td, "libs")
        os.makedirs(root)
        ok = mb.safe_join(root, "org/lwjgl/a.jar")
        check("safe_join keeps a normal path under the root", ok.startswith(os.path.realpath(root) + os.sep))
        check("safe_join: .. that stays inside is fine", mb.safe_join(root, "a/../b.jar").endswith(os.sep + "b.jar"))
        check("safe_join: an absolute path is made relative to the root",
              mb.safe_join(root, "/etc/passwd").startswith(os.path.realpath(root) + os.sep))
        for bad in ("../escape", "a/../../escape", "", "..", "."):
            check(f"safe_join refuses {bad!r}", refused(mb.safe_join, root, bad))
        os.symlink(td, os.path.join(root, "link"))
        check("safe_join refuses a path through a symlink that leaves the root", refused(mb.safe_join, root, "link/x.jar"))

    for n in ("26.3", "1.21-pre1", "25w10a", "3D Shareware v1.34", "17"):
        check(f"safe_name accepted: {n!r}", mb.safe_name(n) == n)
    for n in ("", "..", ".", "a/b", "a\\b", "x..y", "a\x00b", "é", "x" * 65):
        check(f"safe_name refused: {n!r}", refused(mb.safe_name, n))
    check("a SHA-1 is 40 hex digits", bool(mb._HEX40.match("e877b6a07acd633fb3bb475002175cec036e7b87")))
    check("a non-SHA-1 hash is not", not mb._HEX40.match("../../etc/passwd" + "0" * 24))
    print("ALL PASSED" if not fails else f"{fails} FAILED")
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
