#!/usr/bin/env python3
"""Differential test harness: every tests/java/<Name>.java program runs on the JVM and as a Rust native executable
built by monolith.rustgen. Stdout and the exit code must match exactly.

usage: python3 tests/run_tests.py [-O 2] [--shards N] [--min-avail-gib GIB] [--timeout SECS] [--keep] [Name ...]

Refuses to start when MemAvailable is below --min-avail-gib (env MONO_TEST_MIN_AVAIL_GIB, default 0.5), so the
machine does not thrash. Needs MONO_JDK_HOME (or a JDK 25 under ~/.cache/monolith/temurin-25) and rustc.
Programs that need a native host the test machine lacks are listed in SKIP with the reason.
"""
from __future__ import annotations

import argparse
import difflib
import glob
import os
import shutil
import subprocess
import sys
import tempfile
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
JAVA_DIR = os.path.join(HERE, "java")

# Programs that need something this harness cannot provide (a display, network access, a JNI library).
SKIP = {
    "ClientCoreE2E": "needs the Zig-era Client Core harness; the Rust core is tested by cargo test",
    "HttpFetch": "needs network access",
    "NetIo": "needs network access",
    "Jni": "needs the native JNI library built by the Jni.c harness",
}


def jdk_home() -> str:
    env = os.environ.get("MONO_JDK_HOME")
    if env and os.path.isfile(os.path.join(env, "bin", "javac")):
        return env
    for cand in sorted(glob.glob(os.path.expanduser("~/.cache/monolith/temurin-25/jdk-25*"))):
        if os.path.isfile(os.path.join(cand, "bin", "javac")):
            return cand
    sys.exit("no JDK 25: set MONO_JDK_HOME")


def mem_avail_gib() -> float:
    try:
        with open("/proc/meminfo") as fh:
            for line in fh:
                if line.startswith("MemAvailable:"):
                    return int(line.split()[1]) / (1024 * 1024)
    except OSError:
        pass
    return float("inf")


def env_for_children() -> dict[str, str]:
    # JAVA_TOOL_OPTIONS and _JAVA_OPTIONS print to stderr and would corrupt the first-line comparison
    return {k: v for k, v in os.environ.items() if k not in ("JAVA_TOOL_OPTIONS", "_JAVA_OPTIONS")}


def run_java(jdk: str, classes: str, name: str, env: dict[str, str], timeout: float) -> subprocess.CompletedProcess:
    # -Xss8m: the reference stack depth the Exceptions test is written against
    return subprocess.run([os.path.join(jdk, "bin", "java"), "-Xss8m", "-cp", classes, name], capture_output=True, text=True,
                          env=env, timeout=timeout)


def build_rust(work: str, classes: str, name: str, opt: str, shards: int, env: dict[str, str]) -> tuple[str, str]:
    out = os.path.join(work, "out")
    cmd = [sys.executable, "-m", "monolith.rustgen", "--cp", classes, "--main", name, "-o", out,
           "--shards", str(shards), "--shard-jobs", str(max(1, os.cpu_count() or 1)), "--opt", opt, "--compile"]
    r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, env=env)
    return (os.path.join(out, "prog"), r.stderr[-1500:]) if r.returncode == 0 else ("", r.stderr[-1500:])


def run_one(name: str, jdk: str, args: argparse.Namespace, env: dict[str, str]) -> tuple[bool, str]:
    src = os.path.join(JAVA_DIR, name + ".java")
    with tempfile.TemporaryDirectory(prefix="mt_" + name + "_") as work:
        classes = os.path.join(work, "classes")
        os.makedirs(classes)
        jc = subprocess.run([os.path.join(jdk, "bin", "javac"), "-d", classes, src], capture_output=True, text=True,
                            env=env)
        if jc.returncode:
            return False, "javac failed:\n" + jc.stderr[-800:]
        ref = run_java(jdk, classes, name, env, args.timeout)
        exe, err = build_rust(work, classes, name, args.opt, args.shards, env)
        if not exe:
            return False, "build failed:\n" + err
        t0 = time.time()
        try:
            got = subprocess.run([exe], capture_output=True, text=True, timeout=args.timeout, env=env)
        except subprocess.TimeoutExpired:
            return False, f"native run exceeded {args.timeout:.0f}s"
        dt = time.time() - t0
        if got.stdout == ref.stdout and got.returncode == ref.returncode:
            return True, f"{dt:.2f}s"
        diff = "".join(difflib.unified_diff(ref.stdout.splitlines(True), got.stdout.splitlines(True),
                                            "hotspot", "monolith", n=2))
        return False, f"exit hotspot={ref.returncode} monolith={got.returncode}\n{diff[-1500:]}\n{got.stderr[-600:]}"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("-O", "--opt", default="2", help="rustc opt-level (0, 1, 2, 3, s, z)")
    ap.add_argument("--shards", type=int, default=min(4, os.cpu_count() or 1),
                    help="rustc object shards per program (parallel codegen units)")
    ap.add_argument("--min-avail-gib", type=float,
                    default=float(os.environ.get("MONO_TEST_MIN_AVAIL_GIB", "0.5")))
    ap.add_argument("--timeout", type=float, default=300.0)
    ap.add_argument("names", nargs="*")
    args = ap.parse_args()

    avail = mem_avail_gib()
    if avail < args.min_avail_gib:
        sys.exit(f"refusing to run: {avail:.2f} GiB available, floor {args.min_avail_gib} GiB")
    if shutil.which("rustc") is None:
        sys.exit("rustc not found")
    jdk = jdk_home()
    env = env_for_children()
    env["MONO_JDK_HOME"] = jdk

    names = args.names or sorted(os.path.splitext(os.path.basename(p))[0]
                                 for p in glob.glob(os.path.join(JAVA_DIR, "*.java")))
    passed = failed = skipped = 0
    for name in names:
        if name in SKIP:
            print(f"SKIP   {name}: {SKIP[name]}")
            skipped += 1
            continue
        ok, detail = run_one(name, jdk, args, env)
        print(f"{'PASS' if ok else 'FAIL'}   {name}: {detail if ok else ''}")
        if not ok:
            print(detail)
            failed += 1
        else:
            passed += 1
    print(f"{passed}/{passed + failed} passed, {skipped} skipped")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
