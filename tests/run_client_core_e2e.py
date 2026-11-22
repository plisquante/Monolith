#!/usr/bin/env python3
"""Fast unified End-to-End Integration Test for J2ZIG Client Core:
1. GC-free ArenaAllocator + SlabPool + WorkerPool
2. Zero-LWJGL RHI (Vulkan 1.1+ / OpenGL 3.3 Core / Headless)
3. Vanilla-safe FOV + Portal + Hi-Z + Resolution World Render Pipeline
4. GPU-Driven Unified Non-Block Instancing
5. Transpiled Java E2E verifying 0 missing-native, 0 link-error, 0 unsupported-varhandle
"""
from __future__ import annotations

import argparse
import glob
import json
import os
import shutil
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, ROOT)
STEP_TIMEOUT = int(os.environ.get("J2Z_E2E_TIMEOUT", "10800"))  # seconds per phase; 3 h default


def _jdk_home() -> str:
    """J2Z_JDK_HOME if set, else the first JDK the transpiler can find (env, common locations)."""
    try:
        from j2zig.classpath import jdk_home
        return jdk_home()
    except Exception:
        cands = sorted(glob.glob(os.path.join(os.path.expanduser("~"), ".cache", "j2zig", "temurin-*", "jdk-*")))
        if cands:
            return cands[-1]
        raise SystemExit("no JDK found: set J2Z_JDK_HOME (e.g. to the Temurin 25 JDK used for the other tests)")


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


def run_step(label: str, cmd: list[str], env: dict[str, str]):
    t0 = time.time()
    print(f"\n==> [{label}] {' '.join(cmd)}", flush=True)
    try:
        r = subprocess.run(cmd, cwd=ROOT, env=env, timeout=STEP_TIMEOUT, preexec_fn=_child_preexec())
    except subprocess.TimeoutExpired:
        raise SystemExit(f"FAILED [{label}] (timed out after {STEP_TIMEOUT}s)")
    dt = time.time() - t0
    if r.returncode != 0:
        raise SystemExit(f"FAILED [{label}] (exit {r.returncode}) in {dt:.1f}s")
    print(f"OK [{label}] in {dt:.1f}s", flush=True)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("-O", "--optimize", default="Debug",
                    choices=["Debug", "ReleaseSafe", "ReleaseFast"])
    ap.add_argument("--shards", type=int, default=0)
    ap.add_argument("--shard-jobs", type=int, default=1)
    ap.add_argument("--zig-only", action="store_true", help="run only Phase 1 (Zig unit + 4-subsystem E2E)")
    ap.add_argument("--java-only", action="store_true", help="run only Phase 2 (Java ClientCoreE2E)")
    ap.add_argument("--keep-build", action="store_true")
    a = ap.parse_args()

    env = dict(os.environ)
    env.pop("JAVA_TOOL_OPTIONS", None)
    if "J2Z_JDK_HOME" not in env:
        env["J2Z_JDK_HOME"] = _jdk_home()

    t_all = time.time()

    comp_dir = os.path.join(ROOT, "tests", "build", "E2EComposite")
    if not a.java_only:
        # Phase 1: Native Zig E2E verification across all 4 core subsystems
        for mod in ["runtime/mem_pool.zig", "runtime/thread_pool.zig", "runtime/rhi.zig", "runtime/world_render.zig", "runtime/gpu_instancing.zig"]:
            run_step(f"Zig unit ({mod})", ["zig", "test", "-lc", "-O", a.optimize, mod], env)

        # Run full composite 4-subsystem Zig E2E directly inside tests/ build cache
        os.makedirs(comp_dir, exist_ok=True)
        for fn in ["mem_pool.zig", "thread_pool.zig", "rhi.zig", "rhi_vk.zig", "rhi_gl.zig", "world_render.zig", "gpu_instancing.zig"]:
            shutil.copy(os.path.join(ROOT, "runtime", fn), os.path.join(comp_dir, fn))
        # Patch relative imports cleanly only if needed -> our e2e_client_core_test.zig uses module names; rewrite to relative for comp_dir
        with open(os.path.join(ROOT, "tests", "e2e_client_core_test.zig")) as fh:
            src_e2e = fh.read()
        for mod in ["mem_pool", "thread_pool", "rhi", "world_render", "gpu_instancing"]:
            src_e2e = src_e2e.replace(f'@import("{mod}")', f'@import("{mod}.zig")')
        comp_test = os.path.join(comp_dir, "e2e_client_core_test.zig")
        with open(comp_test, "w") as fh:
            fh.write(src_e2e)
        run_step("Zig 4-Subsystem E2E", ["zig", "test", "-lc", "-O", a.optimize, comp_test], env)
        if not a.keep_build:
            shutil.rmtree(comp_dir, ignore_errors=True)
        if a.zig_only:
            print(f"\nZIG CLIENT CORE E2E CHECKS PASSED ({a.optimize}) in {time.time() - t_all:.1f}s!", flush=True)
            return 0

    # Phase 2: Transpile, compile & execute Java ClientCoreE2E + check 0 missing-native / 0 link-error
    out_dir = os.path.join(ROOT, "tests", "build", f"ClientCoreE2E_{a.optimize}")
    if os.path.isdir(out_dir):
        shutil.rmtree(out_dir)
    cls_dir = os.path.join(out_dir, "classes")
    nat_dir = os.path.join(out_dir, "native")
    os.makedirs(cls_dir, exist_ok=True)

    javac = os.path.join(env["J2Z_JDK_HOME"], "bin", "javac")
    # the built-in j2z.rhi bridge (runtime/java) is compiled by the transpiler's builtin mechanism;
    # the test sources only need it on their javac classpath
    from j2zig.classpath import builtin_classes
    builtin = builtin_classes()
    assert builtin, "built-in j2z.rhi bridge failed to compile (set J2Z_NO_BUILTIN=1 to debug without it)"
    run_step("javac ClientCoreE2E", [
        javac, "-J-Xmx192m", "-J-XX:+UseSerialGC",
        "-nowarn", "-cp", builtin, "-d", cls_dir,
        os.path.join(ROOT, "tests", "java", "ClientCoreE2E.java"),
    ], env)

    zig_cache = os.path.join(ROOT, "tests", "build", "zig-cache")
    run_step(f"j2zig build ({a.optimize}, shards={a.shards})", [
        sys.executable, "-m", "j2zig", "build",
        "--cp", cls_dir,
        "--main", "ClientCoreE2E",
        "-o", nat_dir,
        "--name", "ClientCoreE2E",
        "-O", a.optimize,
        "--shards", str(a.shards),
        "--shard-jobs", str(a.shard_jobs),
        "--zig-cache", zig_cache,
        "--show-issues", "20",
    ], env)

    report_path = os.path.join(nat_dir, "report.json")
    with open(report_path) as fh:
        rep = json.load(fh)
    issues = rep.get("issues", {})
    for key in ("missing-native", "link-error", "unsupported-varhandle"):
        vals = issues.get(key, [])
        if vals:
            raise SystemExit(f"FAILED: report.json still has {len(vals)} {key}: {vals[:10]}")
    print(f"OK [report.json] 0 missing-native, 0 link-error, 0 unsupported-varhandle "
          f"({rep.get('classes_loaded')} classes, {rep.get('methods_reachable')} methods)", flush=True)

    exe = os.path.join(nat_dir, "ClientCoreE2E")
    run_step("Run ClientCoreE2E native binary", [exe], env)

    if not a.keep_build:
        shutil.rmtree(comp_dir, ignore_errors=True)
        shutil.rmtree(out_dir, ignore_errors=True)

    print(f"\nALL 4 CLIENT CORE E2E CHECKS PASSED ({a.optimize}) in {time.time() - t_all:.1f}s!", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
