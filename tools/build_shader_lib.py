#!/usr/bin/env python3
"""Builds libj2zshader.so (shaderc + SPIRV-Cross + JNI glue, android/shader) for arm64-v8a with ndk-build.

shaderc, glslang and SPIRV-Tools come from the NDK (sources/third_party/shaderc); SPIRV-Cross is fetched at a pinned
release tag. Output: <out>/libj2zshader.so (default build/shader). Needs ANDROID_NDK_HOME (or --ndk).
"""
import argparse
import os
import shutil
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
SHADER_DIR = REPO_ROOT / "android" / "shader"
SPIRV_CROSS_URL = "https://github.com/KhronosGroup/SPIRV-Cross"
SPIRV_CROSS_TAG = "vulkan-sdk-1.4.350.1"
SPIRV_CROSS_COMMIT = "1a6169566c73d3da552748fc372fe2bbb856e46e"
CACHE = Path.home() / ".cache" / "j2zig"


def run(cmd, **kw):
    print("+", " ".join(str(c) for c in cmd), flush=True)
    subprocess.run([str(c) for c in cmd], check=True, **kw)


def spirv_cross_checkout() -> Path:
    dest = CACHE / f"SPIRV-Cross-{SPIRV_CROSS_TAG}"
    if not (dest / "spirv_cross_c.cpp").is_file():
        if dest.exists():
            shutil.rmtree(dest)
        dest.parent.mkdir(parents=True, exist_ok=True)
        run(["git", "clone", "--quiet", "--depth", "1", "--branch", SPIRV_CROSS_TAG, SPIRV_CROSS_URL, dest])
    head = subprocess.run(["git", "-C", str(dest), "rev-parse", "HEAD"], capture_output=True, text=True, check=True).stdout.strip()
    if head != SPIRV_CROSS_COMMIT:
        raise SystemExit(f"SPIRV-Cross {SPIRV_CROSS_TAG} is at {head}, expected {SPIRV_CROSS_COMMIT}")
    return dest


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--ndk", type=Path, default=Path(os.environ.get("ANDROID_NDK_HOME", "")) if os.environ.get("ANDROID_NDK_HOME") else None)
    ap.add_argument("--out", type=Path, default=REPO_ROOT / "build" / "shader")
    ap.add_argument("-j", type=int, default=os.cpu_count() or 2)
    args = ap.parse_args()
    if args.ndk is None or not (args.ndk / "ndk-build").is_file():
        raise SystemExit("set ANDROID_NDK_HOME (or --ndk) to an Android NDK with ndk-build")
    spvx = spirv_cross_checkout()
    obj = args.out / "obj"
    libs = args.out / "libs"
    for d in (obj, libs):
        d.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ, ANDROID_NDK_ROOT=str(args.ndk))
    run([args.ndk / "ndk-build",
         f"NDK_PROJECT_PATH={SHADER_DIR}", f"APP_BUILD_SCRIPT={SHADER_DIR / 'jni' / 'Android.mk'}",
         f"NDK_APPLICATION_MK={SHADER_DIR / 'jni' / 'Application.mk'}", f"NDK_OUT={obj}", f"NDK_LIBS_OUT={libs}",
         f"SPIRV_CROSS_DIR={spvx}", f"-j{args.j}"], env=env)
    so = libs / "arm64-v8a" / "libj2zshader.so"
    if not so.is_file():
        raise SystemExit("ndk-build finished without producing libj2zshader.so")
    shutil.copy2(so, args.out / "libj2zshader.so")
    print(f"[shader] {args.out / 'libj2zshader.so'} ({so.stat().st_size // 1024} KiB)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
