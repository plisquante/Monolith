#!/usr/bin/env python3
"""Build Minecraft natively with the Monolith Rust back end.

    python3 tools/mcbuild_rust.py /path/to/client.jar               # desktop (Linux x86_64): DIR/run.sh
    python3 tools/mcbuild_rust.py /path/to/client.jar --android     # Android arm64: DIR/rust/libminecraft_mono.so

Options: -o DIR (default build/minecraft-<version>[-android]), --opt 2, --shards 32, --shard-jobs 3,
--resume (skip code generation, compile what is in DIR/rust).
Needs rustc (+ `rustup target add aarch64-linux-android` and the Android NDK for --android) and about
12 GiB RAM for 3 parallel shards. Code generation runs in its own process first (it needs ~5 GiB) so the
compile does not run out of memory.
"""
import argparse, os, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
import mcbuild as mb  # noqa: E402

RUN_BAT = """@echo off
rem Starts Minecraft {vid} built by Monolith (Windows).
set "HERE=%~dp0"
set "MONO_RESOURCES=%HERE%resources"
if "%MC_GAME_DIR%"=="" set "MC_GAME_DIR=%HERE%game"
if not exist "%MC_GAME_DIR%" mkdir "%MC_GAME_DIR%"
if "%MC_GRAPHICS%"=="" set "MC_GRAPHICS=opengl"
if "%MC_USERNAME%"=="" set "MC_USERNAME=Player"
"%HERE%rust\\prog.exe" -Djavax.net.ssl.trustStore="%HERE%resources\\cacerts" -Djavax.net.ssl.trustStoreType=JKS -Dmono.graphics.backend=%MC_GRAPHICS% --version {vid} --gameDir "%MC_GAME_DIR%" --assetsDir "%HERE%assets" --assetIndex {idx} --username %MC_USERNAME% --accessToken 0 --userType legacy %*
"""

RUN_SH = """#!/bin/sh
# Starts Minecraft {vid} built by Monolith.
HERE="$(cd "$(dirname "$0")" && pwd)"
export MONO_RESOURCES="$HERE/resources"
GAME_DIR="${{MC_GAME_DIR:-$HERE/game}}"
mkdir -p "$GAME_DIR"
exec "$HERE/rust/prog" -Djavax.net.ssl.trustStore="$HERE/resources/cacerts" -Djavax.net.ssl.trustStoreType=JKS -Dmono.graphics.backend="${{MC_GRAPHICS:-opengl}}" --version {vid} --gameDir "$GAME_DIR" \\
  --assetsDir "$HERE/assets" --assetIndex {idx} --username "${{MC_USERNAME:-Player}}" --accessToken 0 --userType legacy "$@"
"""


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("jar")
    ap.add_argument("--android", action="store_true")
    ap.add_argument("-o", "--out")
    ap.add_argument("--opt", default="2", choices=["0", "1", "2", "3", "s", "z"])
    ap.add_argument("--shards", type=int, default=32)
    ap.add_argument("--shard-jobs", type=int, default=3)
    ap.add_argument("--api", type=int, default=24)
    ap.add_argument("--ndk", default="")
    ap.add_argument("--resume", action="store_true")
    ap.add_argument("--target", default="",
                    help="Rust target triple (default: the host): x86_64-pc-windows-msvc, aarch64-apple-darwin, ...")
    ap.add_argument("--linker", default="", help="linker for a cross build (passed to rustc as -C linker=...)")
    a = ap.parse_args()

    jar = os.path.abspath(a.jar)
    ver = mb.read_jar_version(jar)
    vid = ver.get("id") or sys.exit("[mcbuild_rust] version.json in the JAR has no 'id'")
    if a.android:
        mb._TARGET_ARCH = "arm64"
    out = os.path.abspath(a.out or os.path.join(ROOT, "build", f"minecraft-{vid}" + ("-android" if a.android else "")))
    rust = os.path.join(out, "rust")
    os.makedirs(out, exist_ok=True)
    meta = mb.version_metadata(vid, None)
    os.environ["MONO_JDK_HOME"] = mb.ensure_jdk(int(ver.get("java_version", 25)))
    libs = mb.download_libraries(meta, os.path.join(out, "natives"), skip_natives=True)
    libs = [j for j in libs if "lwjgl" not in os.path.basename(j).lower()]  # Monolith RHI replaces LWJGL
    jars = [jar] + libs
    common = [sys.executable, "-m", "monolith.rustgen", "--no-lwjgl", "--shards", str(a.shards),
              "--shard-jobs", str(a.shard_jobs), "--opt", a.opt, "-o", rust]
    if a.android:
        common += ["--android", "--api", str(a.api)] + (["--ndk", a.ndk] if a.ndk else [])
    if a.target:
        common += ["--target", a.target]
    if a.linker:
        common += ["--linker", a.linker]
    windows = ("windows" in a.target) if a.target else (sys.platform == "win32")
    if not a.resume:
        reflect = os.path.join(out, "reflect-classes.txt")
        with open(reflect, "w") as fh:
            # library metadata plus the JDK provider classes (created by name at run time: JCA, JSSE, X.509)
            import jdk_runtime_data
            fh.write("\n".join(sorted(set(mb.reflection_metadata(jars)) | set(jdk_runtime_data.provider_classes(os.environ["MONO_JDK_HOME"])))))
        gen = [sys.executable, "-m", "monolith.rustgen", "--cp", os.pathsep.join(jars), "--reflect-class", "@" + reflect,
               "--main", mb.MAIN_CLASS, "--no-lwjgl", "--shards", str(a.shards), "-o", rust] + (["--android"] if a.android else [])
        if subprocess.run(gen, cwd=ROOT).returncode:
            return 1
    comp = common + ["--cp", "-", "--main", "-", "--resume"] + ([] if a.android else ["--compile"])
    if subprocess.run(comp, cwd=ROOT).returncode:
        return 1
    if a.android:
        print(f"done: {os.path.join(rust, 'libminecraft_mono.so')}")
        return 0
    mb.extract_resources(jars, os.path.join(out, "resources"))
    # JDK data the runtime reads by name (ICU normalisation for X.500 names) and the trust store (public CA roots)
    import jdk_runtime_data, shutil
    jdk = os.environ["MONO_JDK_HOME"]
    jdk_runtime_data.main_copy(jdk, os.path.join(out, "resources"))
    shutil.copy(os.path.join(jdk, "lib", "security", "cacerts"), os.path.join(out, "resources", "cacerts"))
    mb.download_assets(meta, os.path.join(out, "assets"))
    mb.apply_branding(os.path.join(out, "resources"), os.path.join(out, "assets"))
    if windows:
        run = os.path.join(out, "run.bat")
        with open(run, "w", newline="\r\n") as fh:
            fh.write(RUN_BAT.format(vid=vid, idx=meta["assetIndex"]["id"]))
    else:
        run = os.path.join(out, "run.sh")
        with open(run, "w", newline="\n") as fh:
            fh.write(RUN_SH.format(vid=vid, idx=meta["assetIndex"]["id"]))
        os.chmod(run, 0o755)
    print(f"done: {run}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
