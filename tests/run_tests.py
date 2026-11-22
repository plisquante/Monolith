#!/usr/bin/env python3
"""Differential test harness: every tests/java/<Name>.java program is run on the JVM and as a
j2zig native executable; stdout, the first line of stderr and the exit code must match exactly.

usage: python3 tests/run_tests.py [-O ReleaseSafe] [--min-avail-gib GIB] [Name ...]

Refuses to run when MemAvailable is below --min-avail-gib (env J2Z_TEST_MIN_AVAIL_GIB, default
0.5 GiB) instead of letting the machine thrash: each test peak-RSS is roughly the closure's
transpiled size and zig holds much of it in memory. Set the floor to 0 to override (not advised).
"""
from __future__ import annotations

import argparse
import difflib
import glob
import json
import os
import shutil
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))


def mem_avail_gib() -> float:
    try:
        with open("/proc/meminfo") as fh:
            for line in fh:
                if line.startswith("MemAvailable:"):
                    return int(line.split()[1]) / (1024 * 1024)
    except OSError:
        pass
    return 1e9  # unknown platform: do not block
ROOT = os.path.dirname(HERE)
BUILD = os.path.join(HERE, "build")
ENV = {k: v for k, v in os.environ.items() if k != "JAVA_TOOL_OPTIONS"}
sys.path.insert(0, ROOT)
from j2zig.classpath import jdk_tool  # noqa: E402


def _child_preexec():
    if os.name != "posix":
        return None
    def _apply():
        try:
            os.nice(10)
        except OSError:
            pass
        try:
            if hasattr(os, "sched_setaffinity") and (os.cpu_count() or 1) > 2:
                os.sched_setaffinity(0, {0, 1})
        except OSError:
            pass
    return _apply


def run(cmd, timeout=300, **kw):
    return subprocess.run(cmd, capture_output=True, text=True, timeout=timeout, env=ENV, preexec_fn=_child_preexec(), **kw)


def first_line(s: str) -> str:
    return s.strip().splitlines()[0] if s.strip() else ""


def test_one(name: str, opt: str, zig_cache: str, min_avail_gib: float | None = None) -> dict:
    src = os.path.join(HERE, "java", name + ".java")
    cdir = os.path.join(BUILD, name, "classes")
    os.makedirs(cdir, exist_ok=True)
    r = run([jdk_tool("javac"), "-J-Xmx192m", "-J-XX:+UseSerialGC", "-d", cdir, src])
    if r.returncode != 0:
        return {"name": name, "ok": False, "stage": "javac", "detail": r.stderr}
    # optional JNI library: tests/java/<Name>.c -> lib<Name>.so (built with `zig cc`)
    libdir = os.path.join(BUILD, name, "lib")
    csrc = os.path.join(HERE, "java", name + ".c")
    jopts = []
    nenv = dict(ENV)
    if os.path.exists(csrc):
        os.makedirs(libdir, exist_ok=True)
        from j2zig.classpath import jdk_home
        inc = os.path.join(jdk_home(), "include")
        r = run(["zig", "cc", "-shared", "-fPIC", "-O2", "-I", inc, "-I", os.path.join(inc, "linux"),
                 "-o", os.path.join(libdir, f"lib{name}.so"), csrc])
        if r.returncode != 0:
            return {"name": name, "ok": False, "stage": "cc", "detail": r.stderr}
        jopts = [f"-Djava.library.path={libdir}", "--enable-native-access=ALL-UNNAMED"]
        nenv["J2Z_NATIVES"] = libdir
    t0 = time.time()
    jr = run([jdk_tool("java"), "-Xmx256m", "-XX:+UseSerialGC", "-Xss4m"] + jopts + ["-cp", cdir, name])
    tj = time.time() - t0
    out = os.path.join(BUILD, name, "native")
    floor = min_avail_gib if min_avail_gib is not None else float(os.environ.get("J2Z_TEST_MIN_AVAIL_GIB", "0.5"))
    if floor > 0 and mem_avail_gib() < floor:
        return {"name": name, "ok": False, "stage": "preflight",
                "detail": f"refusing to build: {mem_avail_gib():.1f} GiB available, floor {floor:.1f} GiB (set J2Z_TEST_MIN_AVAIL_GIB=0 to override)"}
    t0 = time.time()
    br = run([sys.executable, "-m", "j2zig", "build", "--cp", cdir, "--main", name, "-o", out,
              "-O", opt, "--backend", os.environ.get("J2Z_TEST_BACKEND", "auto"), "--shards", os.environ.get("J2Z_TEST_SHARDS", "0"), "--zig-cache", zig_cache, "--show-issues", "0"], cwd=ROOT, timeout=int(os.environ.get("J2Z_TEST_TIMEOUT", "3600")))
    tb = time.time() - t0
    if br.returncode != 0:
        return {"name": name, "ok": False, "stage": "build", "detail": (br.stdout + br.stderr)[-6000:]}
    t0 = time.time()
    try:
        nr = subprocess.run([os.path.join(out, name)], capture_output=True, text=True, timeout=300, env=nenv, preexec_fn=_child_preexec())
    except subprocess.TimeoutExpired:
        return {"name": name, "ok": False, "stage": "run", "detail": "timeout"}
    tn = time.time() - t0
    res = {"name": name, "jvm_s": round(tj, 2), "native_s": round(tn, 2), "build_s": round(tb, 1)}
    problems = []
    if jr.stdout != nr.stdout:
        diff = difflib.unified_diff(jr.stdout.splitlines(), nr.stdout.splitlines(), "jvm", "native", lineterm="", n=1)
        problems.append("stdout differs:\n" + "\n".join(list(diff)[:80]))
    if first_line(jr.stderr) != first_line(nr.stderr):
        problems.append(f"stderr first line differs:\n  jvm:    {first_line(jr.stderr)!r}\n  native: {first_line(nr.stderr)!r}")
    if jr.returncode != nr.returncode:
        problems.append(f"exit code jvm={jr.returncode} native={nr.returncode}\n{nr.stderr[-3000:]}")
    res["ok"] = not problems
    res["stage"] = "compare"
    res["detail"] = "\n".join(problems)
    res["lines"] = len(jr.stdout.splitlines())
    return res


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("-O", default="Debug")
    ap.add_argument("--json")
    ap.add_argument("--keep", action="store_true", help="keep the build directory of passed tests (about 1.4 GB each)")
    ap.add_argument("--min-avail-gib", type=float, default=float(os.environ.get("J2Z_TEST_MIN_AVAIL_GIB", "0.5")),
                    help="refuse to start when MemAvailable is below this (default: 0.5, env J2Z_TEST_MIN_AVAIL_GIB; 0 disables)")
    ap.add_argument("names", nargs="*")
    a = ap.parse_args()
    # ClientCoreE2E needs the built-in j2z.rhi bridge on its classpath and has its own runner
    # (see tests/ directory); name it explicitly to try it here anyway.
    names = a.names or sorted(n for n in (os.path.basename(p)[:-5] for p in glob.glob(os.path.join(HERE, "java", "*.java")))
                              if n != "ClientCoreE2E")
    zig_cache = os.path.join(BUILD, "zig-cache")
    results = []
    for n in names:
        if a.min_avail_gib > 0 and mem_avail_gib() < a.min_avail_gib:
            print(f"refusing to continue: {mem_avail_gib():.1f} GiB available (floor {a.min_avail_gib:.1f} GiB); "
                  f"stopping the suite instead of thrashing the machine", flush=True)
            return 3
        r = test_one(n, a.O, zig_cache, a.min_avail_gib)
        results.append(r)
        status = "PASS" if r["ok"] else f"FAIL ({r['stage']})"
        extra = f"  [{r.get('lines', 0)} lines; jvm {r.get('jvm_s')}s, native {r.get('native_s')}s, build {r.get('build_s')}s]" if "jvm_s" in r else ""
        print(f"{status:16s} {n}{extra}", flush=True)
        if not r["ok"]:
            print("    " + r["detail"].replace("\n", "\n    ")[:6000], flush=True)
        elif not a.keep:
            # remove per-test build directory; retain zig-cache when disk space permits so Zig reuses std/libc
            shutil.rmtree(os.path.join(BUILD, n), ignore_errors=True)
            try:
                free_gib = shutil.disk_usage(BUILD).free / (1024 ** 3)
            except OSError:
                free_gib = 100.0
            if free_gib < 8.0:
                shutil.rmtree(zig_cache, ignore_errors=True)
    passed = sum(r["ok"] for r in results)
    print(f"\n{passed}/{len(results)} passed")
    if a.json:
        with open(a.json, "w") as fh:
            json.dump(results, fh, indent=1)
    return 0 if passed == len(results) else 1


if __name__ == "__main__":
    sys.exit(main())
