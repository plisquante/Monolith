#!/usr/bin/env python3
"""Incremental Minecraft build: transpile, then recompile only the shards whose generated Zig changed.

Usage: tools/incr_build.py <client.jar> -o <out dir> [-O ReleaseSafe] [--shards N] [--shard-jobs N] [--no-lwjgl] [-- extra mcbuild args]

The first run is a full build and records, for every generated Zig file, its content hash (<out>/.incr_files.json).
Later runs list the generated files (classes) that changed, rebuild only the shards that contain them (the others
keep their object, see `--incremental` in monolith/__main__.py) and link. Python hash randomisation is switched off
(PYTHONHASHSEED=0) so unchanged classes transpile to byte-identical Zig.
"""
import argparse
import hashlib
import json
import os
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent


def file_hashes(src: Path) -> dict[str, str]:
    out = {}
    for p in sorted(src.rglob("*.zig")):
        out[str(p.relative_to(src))] = hashlib.sha1(p.read_bytes()).hexdigest()
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("jar")
    ap.add_argument("-o", "--out", required=True)
    ap.add_argument("-O", "--optimize", default="ReleaseSafe")
    ap.add_argument("--shards", type=int, default=12)
    ap.add_argument("--shard-jobs", type=int, default=2)
    ap.add_argument("--backend", default="llvm", choices=["auto", "llvm", "native"])
    ap.add_argument("--no-lwjgl", action="store_true")
    ap.add_argument("extra", nargs="*", help="further mcbuild arguments")
    a = ap.parse_args()

    out = Path(a.out)
    snap = out / ".incr_files.json"
    old = json.loads(snap.read_text()) if snap.is_file() else None

    env = dict(os.environ, PYTHONHASHSEED="0", MONO_ZIG_LD=os.environ.get("MONO_ZIG_LD", "zig"))
    env.pop("_JAVA_OPTIONS", None)
    env.pop("JAVA_TOOL_OPTIONS", None)
    cmd = [sys.executable, "-u", str(REPO / "tools" / "mcbuild.py"), a.jar, "-O", a.optimize, "--backend", a.backend,
           "--skip-assets", "--shards", str(a.shards), "--shard-jobs", str(a.shard_jobs), "--incremental",
           "-o", str(out)]
    if a.no_lwjgl:
        cmd.append("--no-lwjgl")
    cmd += a.extra
    t0 = time.time()
    r = subprocess.run(cmd, env=env)
    if r.returncode != 0:
        print(f"incr_build: build failed (exit {r.returncode})", file=sys.stderr)
        return r.returncode

    new = file_hashes(out / "src")
    if old is None:
        print(f"incr_build: full build, {len(new)} generated files recorded ({time.time() - t0:.0f}s)")
    else:
        changed = sorted(k for k in new if old.get(k) != new[k])
        gone = sorted(k for k in old if k not in new)
        print(f"incr_build: {len(changed)} generated files changed, {len(gone)} removed ({time.time() - t0:.0f}s)")
        for k in changed[:40]:
            print("  changed", k)
        if len(changed) > 40:
            print(f"  ... and {len(changed) - 40} more")
    snap.write_text(json.dumps(new))
    return 0


if __name__ == "__main__":
    sys.exit(main())
