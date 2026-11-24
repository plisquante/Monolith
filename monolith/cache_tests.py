#!/usr/bin/env python3
"""Tests of the compiled-object cache limit (monolith/rustgen.py `prune_shard_cache`, `_touch`). Pure Python, instant.

1. Under the limit nothing is removed.
2. Over the limit the oldest files go first, until the total fits.
3. A file that was used (touched) outlives older ones.
4. Subdirectories (the android cache) count and are pruned too; the environment variable sets the default limit.
Usage: python3 -m monolith.cache_tests
"""
import os
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
fails = 0


def check(name: str, ok: bool, detail: str = "") -> None:
    global fails
    print(("PASS " if ok else "FAIL ") + name + ("" if ok else "\n" + detail))
    fails += 0 if ok else 1


def make(d: str, name: str, size: int, age: int) -> str:
    p = os.path.join(d, name)
    os.makedirs(os.path.dirname(p), exist_ok=True)
    with open(p, "wb") as fh:
        fh.write(b"x" * size)
    t = 1_700_000_000 - age
    os.utime(p, (t, t))
    return p


def main() -> int:
    from monolith.rustgen import _touch, prune_shard_cache
    with tempfile.TemporaryDirectory() as d:
        a = make(d, "a.o", 100, age=300)
        b = make(d, "b.o", 100, age=200)
        c = make(d, "c.o", 100, age=100)
        check("under the limit: nothing removed", prune_shard_cache(d, 1000) == 0 and all(map(os.path.exists, (a, b, c))))
        n = prune_shard_cache(d, 250)
        check("over the limit: the oldest file goes", n == 1 and not os.path.exists(a) and os.path.exists(b) and os.path.exists(c),
              f"removed {n}")
        n = prune_shard_cache(d, 100)
        check("until the total fits", n == 1 and not os.path.exists(b) and os.path.exists(c), f"removed {n}")

    with tempfile.TemporaryDirectory() as d:
        old = make(d, "old.o", 100, age=300)
        mid = make(d, "mid.o", 100, age=200)
        new = make(d, "new.o", 100, age=100)
        _touch(old)  # a cache hit: the oldest file is now the most recently used
        prune_shard_cache(d, 200)
        check("a used file outlives older ones", os.path.exists(old) and os.path.exists(new) and not os.path.exists(mid),
              f"old={os.path.exists(old)} mid={os.path.exists(mid)} new={os.path.exists(new)}")

    with tempfile.TemporaryDirectory() as d:
        top = make(d, "top.o", 100, age=100)
        sub = make(d, "android/sub.o", 100, age=300)
        prune_shard_cache(d, 150)
        check("subdirectory files count and are pruned", os.path.exists(top) and not os.path.exists(sub))
        os.environ["MONO_SHARD_CACHE_GB"] = "0.0000001"  # about 107 bytes
        make(d, "more.o", 100, age=50)
        n = prune_shard_cache(d)
        check("MONO_SHARD_CACHE_GB sets the default limit", n >= 1, f"removed {n}")
        os.environ["MONO_SHARD_CACHE_GB"] = "not a number"
        check("an invalid value falls back to the default, nothing removed", prune_shard_cache(d) == 0)
        del os.environ["MONO_SHARD_CACHE_GB"]
    print("ALL PASSED" if not fails else f"{fails} FAILED")
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
