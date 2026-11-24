#!/usr/bin/env python3
"""Classify every hand-written Java file of the overlays (docs/mimari/04_KUTUPHANE_VE_YAMA.md section 2).

  K1  shell: only fields, signatures, native/abstract methods and trivial bodies (empty, return literal, throw): generatable
  K2  logic: at least one method body with real statements: move to Rust or back to the JDK original
  K3  test helper (path contains /test/)

    python3 tools/lib_inventory.py [--out docs/KUTUPHANE_ENVANTER.json] [--fail-unclassified]

Heuristic (no javac needed): strip comments and strings, find method bodies by brace matching, a body is trivial when its
statements are all `return <literal|identifier|new X()>;`, `throw ...;` or empty. Prints counts and the K2 list by line size.
"""
import argparse
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DIRS = ("classlib/src", "runtime/java_nolwjgl")

_STR = re.compile(r'"(?:\\.|[^"\\\n])*"|\'(?:\\.|[^\'\\\n])\'')
_CMT = re.compile(r"/\*.*?\*/|//[^\n]*", re.S)
_HEAD = re.compile(r"\)\s*(?:throws\s+[\w.,\s]+)?\{")
_TRIVIAL = re.compile(r"^(?:return\s*(?:[-\w.\"'()\[\]]*|new\s+[\w.<>\[\]]+\(\s*\))?\s*;|throw\s+[^;]*;)?$")
_TYPE = re.compile(r"\b(?:class|interface|enum|record)\b")


def clean(src: str) -> str:
    src = _CMT.sub(" ", src)
    return _STR.sub('""', src)


def bodies(src: str):
    """Yield method/constructor/initializer bodies (text between braces) excluding class bodies."""
    for m in _HEAD.finditer(src):
        i = m.end()
        depth = 1
        j = i
        while j < len(src) and depth:
            c = src[j]
            depth += (c == "{") - (c == "}")
            j += 1
        head = src[max(0, m.start() - 160):m.start()]
        if re.search(r"\b(?:if|for|while|switch|catch|synchronized)\s*\([^()]*$", head.split(";")[-1].split("{")[-1].split("}")[-1]):
            continue
        yield src[i:j - 1].strip()


def classify(path: str, src: str):
    if "/test/" in path.replace(os.sep, "/"):
        return "K3", 0
    src = clean(src)
    logic = 0
    for b in bodies(src):
        b = re.sub(r"\s+", " ", b)
        stmts = [s.strip() + ";" for s in b.split(";") if s.strip()]
        if "{" in b or any(not _TRIVIAL.match(s) for s in stmts):
            logic += 1
    return ("K2" if logic else "K1"), logic


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out", default="")
    ap.add_argument("--fail-unclassified", action="store_true")
    a = ap.parse_args()
    rows = []
    for d in DIRS:
        for dp, _dn, fn in os.walk(os.path.join(ROOT, d)):
            for f in fn:
                if not f.endswith(".java"):
                    continue
                p = os.path.join(dp, f)
                with open(p, encoding="utf-8", errors="replace") as fh:
                    src = fh.read()
                k, n = classify(p, src)
                rows.append({"file": os.path.relpath(p, ROOT), "class": k, "logic_bodies": n, "lines": src.count("\n") + 1})
    counts = {}
    for r in rows:
        counts[r["class"]] = counts.get(r["class"], 0) + 1
    print("files:", len(rows), " ".join(f"{k}={v}" for k, v in sorted(counts.items())))
    for d in DIRS:
        sub = [r for r in rows if r["file"].startswith(d)]
        print(f"  {d}: K1={sum(r['class'] == 'K1' for r in sub)} K2={sum(r['class'] == 'K2' for r in sub)} "
              f"K2 lines={sum(r['lines'] for r in sub if r['class'] == 'K2')}")
    k2 = sorted((r for r in rows if r["class"] == "K2"), key=lambda r: -r["lines"])
    for r in k2[:15]:
        print(f"  K2 {r['lines']:5d} lines {r['logic_bodies']:4d} bodies  {r['file']}")
    if a.out:
        with open(a.out, "w", encoding="utf-8") as fh:
            json.dump(sorted(rows, key=lambda r: r["file"]), fh, indent=1)
    return 1 if a.fail_unclassified and any(r["class"] not in ("K1", "K2", "K3") for r in rows) else 0


if __name__ == "__main__":
    sys.exit(main())
