#!/usr/bin/env python3
"""Build libminecraft_mono.so (arm64 Android) from a Minecraft client.jar with the Monolith Rust back end.

    python3 tools/mcbuild_android.py /path/to/client.jar [-o DIR] [--opt 2] [--shards 32] [--shard-jobs 3]

Needs: rustc with the aarch64-linux-android target (rustup target add aarch64-linux-android),
the Android NDK (ANDROID_HOME/ndk/* or ANDROID_NDK_HOME), a JDK 25 (downloaded automatically),
about 12 GiB RAM for 3 parallel shards. The result is DIR/libminecraft_mono.so; select it in the app.
"""
import argparse, os, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
import mcbuild as mb  # noqa: E402


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("jar")
    ap.add_argument("-o", "--out")
    ap.add_argument("--opt", default="2", choices=["0", "1", "2", "3", "s", "z"])
    ap.add_argument("--shards", type=int, default=32)
    ap.add_argument("--shard-jobs", type=int, default=3)
    ap.add_argument("--api", type=int, default=24)
    ap.add_argument("--ndk", default="")
    a = ap.parse_args()

    jar = os.path.abspath(a.jar)
    ver = mb.read_jar_version(jar)
    vid = ver.get("id") or sys.exit("[mcbuild_android] version.json in the JAR has no 'id'")
    mb._TARGET_ARCH = "arm64"
    out = os.path.abspath(a.out or os.path.join(ROOT, "build", f"android-{vid}"))
    os.makedirs(out, exist_ok=True)
    meta = mb.version_metadata(vid, None)
    os.environ["MONO_JDK_HOME"] = mb.ensure_jdk(int(ver.get("java_version", 25)))
    libs = mb.download_libraries(meta, os.path.join(out, "natives"), skip_natives=True)
    libs = [j for j in libs if "lwjgl" not in os.path.basename(j).lower()]  # Monolith RHI replaces LWJGL
    jars = [jar] + libs
    reflect = os.path.join(out, "reflect-classes.txt")
    with open(reflect, "w") as fh:
        fh.write("\n".join(sorted(set(mb.reflection_metadata(jars)))))
    cmd = [sys.executable, "-m", "monolith.rustgen", "--cp", os.pathsep.join(jars), "--reflect-class", "@" + reflect,
           "--main", mb.MAIN_CLASS, "--no-lwjgl", "--android", "--shards", str(a.shards), "--shard-jobs", str(a.shard_jobs),
           "--opt", a.opt, "--api", str(a.api), "-o", os.path.join(out, "rust")]
    if a.ndk:
        cmd += ["--ndk", a.ndk]
    r = subprocess.run(cmd, cwd=ROOT)
    if r.returncode:
        return r.returncode
    so = os.path.join(out, "rust", "libminecraft_mono.so")
    print(f"done: {so}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
