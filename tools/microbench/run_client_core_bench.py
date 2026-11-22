#!/usr/bin/env python3
"""Compile and run the GC-Free Client Core microbenchmark (-O ReleaseFast)."""
from __future__ import annotations

import os
import shutil
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def main() -> int:
    bdir = os.path.join(ROOT, "tests", "build", "ClientCoreBench")
    os.makedirs(bdir, exist_ok=True)
    for fn in ["mem_pool.zig", "thread_pool.zig", "rhi.zig", "rhi_vk.zig", "rhi_gl.zig", "world_render.zig", "gpu_instancing.zig"]:
        shutil.copy(os.path.join(ROOT, "runtime", fn), os.path.join(bdir, fn))

    with open(os.path.join(ROOT, "tools", "microbench", "client_core_bench.zig")) as fh:
        src = fh.read()
    for mod in ["mem_pool", "thread_pool", "rhi", "world_render", "gpu_instancing"]:
        src = src.replace(f'@import("{mod}")', f'@import("{mod}.zig")')

    bench_src = os.path.join(bdir, "client_core_bench.zig")
    with open(bench_src, "w") as fh:
        fh.write(src)

    exe = os.path.join(bdir, "client_core_bench")
    try:
        subprocess.run(["zig", "build-exe", "-lc", "-O", "ReleaseFast", f"-femit-bin={exe}", bench_src],
                       cwd=ROOT, check=True)
        r = subprocess.run([exe], cwd=ROOT)
        return r.returncode
    finally:
        shutil.rmtree(bdir, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
