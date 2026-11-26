#!/usr/bin/env python3
"""Reproducible CLI builder for TouchCore For Monolith APK (arm64-v8a)."""

import argparse
import os
import shutil
import struct
import subprocess
import sys
import zipfile
import zlib
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
TOUCHCORE_DIR = REPO_ROOT / "android" / "touchcore"
BRAND_DIR = REPO_ROOT / "brand"
DEFAULT_OUT_APK = REPO_ROOT / "releases" / "Monolith.apk"

CACHE_ROOT = Path.home() / ".cache" / "monolith"
CACHE_SDK = CACHE_ROOT / "android-sdk"
KEYSTORE_PATH = Path.home() / ".cache" / "monolith" / "touchcore-release.jks"
KEY_ALIAS = "touchcore"
KEY_PASS = "touchcore_monolith_2026"

TERMUX_USR = Path("/data/data/com.termux/files/usr")


def find_tool(name: str, candidates: list[Path]) -> str:
    for c in candidates:
        if c.is_file() and os.access(c, os.X_OK):
            return str(c)
    found = shutil.which(name)
    if found:
        return found
    raise FileNotFoundError(f"Required tool '{name}' not found")


def find_jdk_bin(tool_name: str) -> str:
    temurin_root = Path.home() / ".cache" / "monolith" / "temurin-25"
    candidates = []
    if temurin_root.is_dir():
        for child in sorted(temurin_root.iterdir(), reverse=True):
            candidates.append(child / "bin" / tool_name)
    java_home = os.environ.get("JAVA_HOME")
    if java_home:
        candidates.append(Path(java_home) / "bin" / tool_name)
    candidates.append(Path("/usr/bin") / tool_name)
    return find_tool(tool_name, candidates)


def run_step(desc: str, cmd: list[str], env: dict[str, str] | None = None) -> str:
    print(f"[touchcore] {desc}...")
    proc = subprocess.run(cmd, env=env, text=True, capture_output=True)
    if proc.returncode != 0:
        sys.stderr.write(f"FAILED: {' '.join(cmd)}\n")
        if proc.stdout:
            sys.stderr.write(proc.stdout + "\n")
        if proc.stderr:
            sys.stderr.write(proc.stderr + "\n")
        raise SystemExit(proc.returncode)
    return proc.stdout


def write_png(path: Path, width: int, height: int, rgba_rows: list[bytes]) -> None:
    def chunk(tag: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    raw = bytearray()
    for row in rgba_rows:
        raw.append(0)
        raw.extend(row)

    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    png = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + chunk(b"IEND", b"")
    )
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(png)


def install_launcher_icon(path: Path, density: str) -> None:
    src = BRAND_DIR / "android" / f"ic_launcher_{density}.png"
    if not src.is_file():
        raise SystemExit(f"missing brand icon {src} (see brand/src/render.js)")
    path.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(src, path)


def prepare_resources(build_res: Path) -> None:
    if build_res.exists():
        shutil.rmtree(build_res)
    shutil.copytree(TOUCHCORE_DIR / "res", build_res)

    for density in ("mdpi", "hdpi", "xhdpi", "xxhdpi", "xxxhdpi"):
        install_launcher_icon(build_res / f"mipmap-{density}" / "ic_launcher.png", density)

    logo_dst = build_res / "drawable" / "monolith_logo.png"
    logo_dst.parent.mkdir(parents=True, exist_ok=True)
    title_png = BRAND_DIR / "title.png"
    if title_png.is_file():
        shutil.copy2(title_png, logo_dst)
    else:
        install_launcher_icon(logo_dst, "xhdpi")


def find_ndk_prebuilt() -> Path | None:
    candidates: list[Path] = []
    for var in ("ANDROID_NDK_HOME", "ANDROID_NDK_LATEST_HOME", "ANDROID_NDK_ROOT"):
        val = os.environ.get(var)
        if val:
            candidates.append(Path(val))
    for sdk_var in ("ANDROID_HOME", "ANDROID_SDK_ROOT"):
        val = os.environ.get(sdk_var)
        if val and (Path(val) / "ndk").is_dir():
            candidates.extend(sorted((Path(val) / "ndk").iterdir(), reverse=True))
    std_ndk = Path("/usr/local/lib/android/sdk/ndk")
    if std_ndk.is_dir():
        candidates.extend(sorted(std_ndk.iterdir(), reverse=True))
    for ndk in candidates:
        pre = ndk / "toolchains" / "llvm" / "prebuilt"
        if not pre.is_dir():
            continue
        for tag in ("linux-x86_64", "linux-aarch64"):
            p = pre / tag
            if (p / "sysroot" / "usr" / "include").is_dir():
                return p
    return None


def find_sdk_paths() -> tuple[Path, Path, Path | None]:
    """Locate (android.jar, d8_or_r8_jar, build_tools_bin_dir)."""
    if (CACHE_SDK / "android.jar").is_file() and (CACHE_SDK / "r8.jar").is_file():
        return (CACHE_SDK / "android.jar", CACHE_SDK / "r8.jar", None)
    sdk_roots: list[Path] = []
    for var in ("ANDROID_HOME", "ANDROID_SDK_ROOT"):
        val = os.environ.get(var)
        if val:
            sdk_roots.append(Path(val))
    sdk_roots.append(Path("/usr/local/lib/android/sdk"))
    for sdk in sdk_roots:
        plats = sdk / "platforms"
        bts = sdk / "build-tools"
        if not plats.is_dir() or not bts.is_dir():
            continue
        jars = sorted(plats.glob("android-*/android.jar"), reverse=True)
        bt_dirs = sorted([d for d in bts.iterdir() if d.is_dir()], reverse=True)
        for bt in bt_dirs:
            d8 = bt / "lib" / "d8.jar"
            if d8.is_file() and jars:
                return (jars[0], d8, bt)
    raise FileNotFoundError(
        f"Missing Android SDK artifacts under {CACHE_SDK} or $ANDROID_HOME (expected android.jar & r8.jar/d8.jar)"
    )


def build_native_so(build_dir: Path, out_so: Path) -> None:
    """Builds libtouchcore_mono.so: the Rust core (android/touchcore/native, staticlib) plus the JNI bridge (C)."""
    cargo = find_tool("cargo", [Path.home() / ".cargo" / "bin" / "cargo"])
    use_termux = (TERMUX_USR / "include").is_dir() and (TERMUX_USR / "bin" / "clang").is_file()
    ndk_pre = None if use_termux else find_ndk_prebuilt()
    if use_termux:
        clang = find_tool("clang", [TERMUX_USR / "bin" / "clang"])
        inc_dir = TERMUX_USR / "include"
        arch_inc = inc_dir / "aarch64-linux-android"
        sys_lib_dir = Path("/system/lib64")
    elif ndk_pre is not None:
        clang = find_tool("aarch64-linux-android26-clang", [ndk_pre / "bin" / "aarch64-linux-android26-clang"])
        inc_dir = ndk_pre / "sysroot" / "usr" / "include"
        arch_inc = inc_dir / "aarch64-linux-android"
        sys_lib_dir = ndk_pre / "sysroot" / "usr" / "lib" / "aarch64-linux-android" / "26"
    else:
        raise FileNotFoundError("Neither Termux clang nor Android NDK LLVM toolchain found")

    # Rust çekirdeği. Bağlayıcı NDK clang'ıdır, bu yüzden derleyici çalışma zamanı yerleşikleri onunla gelir.
    rust_target = "aarch64-linux-android"
    cargo_env = dict(os.environ)
    cargo_env[f"CARGO_TARGET_{rust_target.upper().replace('-', '_')}_LINKER"] = str(clang)
    core_dir = TOUCHCORE_DIR / "native"
    run_step(
        "Building the Rust client core (aarch64-linux-android, release)",
        [cargo, "build", "--release", "--offline", "--target", rust_target, "--manifest-path", str(core_dir / "Cargo.toml")],
        env=cargo_env,
    )
    core_lib = core_dir / "target" / rust_target / "release" / "libtouchcore_native.a"
    if not core_lib.is_file():
        raise FileNotFoundError(f"cargo produced no {core_lib}")

    c_obj = build_dir / "touchcore_android.o"
    c_cmd = [
        clang,
        "--target=aarch64-linux-android26",
        "-c",
        "-O2",
        "-fPIC",
        "-ffunction-sections",
        "-fdata-sections",
        "-Wall",
        "-Wextra",
    ]
    if arch_inc.is_dir():
        c_cmd.extend(["-isystem", str(arch_inc)])
    c_cmd.extend([
        "-isystem",
        str(inc_dir),
        str(TOUCHCORE_DIR / "jni" / "touchcore_android.c"),
        "-o",
        str(c_obj),
    ])
    run_step("Compiling Android JNI bridge (clang arm64-v8a)", c_cmd)

    out_so.parent.mkdir(parents=True, exist_ok=True)
    link_cmd = [
        clang,
        "--target=aarch64-linux-android26",
        "-shared",
        "-Wl,-z,max-page-size=16384",
        "-Wl,--gc-sections",
        "-Wl,-soname,libtouchcore_mono.so",
        str(c_obj),
        str(core_lib),
        str(sys_lib_dir / "libandroid.so"),
        str(sys_lib_dir / "liblog.so"),
        # Rust çekirdeği (gles.rs) gl* işlevlerini doğrudan çağırır. Rust'taki #[link(name = "GLESv3")] statik
        # kütüphaneden (.a) bu bağlama taşınmaz; burada açıkça verilmezse libGLESv3 bağımlılık olarak yazılmaz ve
        # Android kütüphaneyi "cannot locate symbol glBindBuffer" hatasıyla yüklemez.
        str(sys_lib_dir / "libGLESv3.so"),
        str(sys_lib_dir / "libc.so"),
        str(sys_lib_dir / "libm.so"),
        str(sys_lib_dir / "libdl.so"),
        "-o",
        str(out_so),
    ]
    run_step("Linking libtouchcore_mono.so (16KB page aligned)", link_cmd)


def ensure_keystore(keytool: str) -> None:
    if KEYSTORE_PATH.is_file():
        return
    KEYSTORE_PATH.parent.mkdir(parents=True, exist_ok=True)
    run_step(
        "Generating TouchCore APK signing keystore",
        [
            keytool,
            "-genkeypair",
            "-v",
            "-keystore",
            str(KEYSTORE_PATH),
            "-storepass",
            KEY_PASS,
            "-alias",
            KEY_ALIAS,
            "-keypass",
            KEY_PASS,
            "-keyalg",
            "RSA",
            "-keysize",
            "2048",
            "-validity",
            "10000",
            "-dname",
            "CN=TouchCore For Monolith, OU=MONOLITH, O=MONOLITH, L=Istanbul, C=TR",
        ],
    )


def zipalign_supports_page_flag(zipalign: str) -> bool:
    proc = subprocess.run([zipalign], text=True, capture_output=True)
    help_text = (proc.stdout or "") + (proc.stderr or "")
    return "-P <pagesize_kb>" in help_text or "-P " in help_text


JNA_VERSION = "5.17.0"  # must equal the JNA jar of the Minecraft libraries (JNA refuses a native library of another version)
JNA_AAR_SHA1 = "4134d63c53cbf9650b11ee950df093add29f14cf"  # net.java.dev.jna:jna:5.17.0@aar on Maven Central


def jna_android_dispatch_lib() -> bytes:
    """libjnidispatch.so for Android arm64 from JNA's official AAR. The jar of the game ships only the glibc builds
    (linux-aarch64 needs libc.so.6), which cannot be loaded on Android; the AAR's arm64-v8a build links against bionic."""
    import hashlib
    import urllib.request
    aar = CACHE_ROOT / f"jna-{JNA_VERSION}.aar"
    if not aar.is_file():
        aar.parent.mkdir(parents=True, exist_ok=True)
        url = f"https://repo1.maven.org/maven2/net/java/dev/jna/jna/{JNA_VERSION}/jna-{JNA_VERSION}.aar"
        with urllib.request.urlopen(url, timeout=120) as r:
            aar.write_bytes(r.read())
    got = hashlib.sha1(aar.read_bytes()).hexdigest()
    if got != JNA_AAR_SHA1:
        aar.unlink()
        raise SystemExit(f"jna-{JNA_VERSION}.aar has SHA-1 {got}, expected {JNA_AAR_SHA1}")
    with zipfile.ZipFile(aar) as z:
        return z.read("jni/arm64-v8a/libjnidispatch.so")


SHIZUKU_VERSION = "13.1.5"
# Maven Central üzerinde dev.rikka.shizuku:<artifact>:13.1.5@aar
SHIZUKU_AARS = {
    "api": "1f760c56493bdbc01462930e75bb325033b36fe2",
    "provider": "4135b99cae9362fe2f63380230d04a9bcba9be3f",
    "shared": "8c4e73ce466cd27aff10565d2ec49809c258f974",
    "aidl": "457504c277d796adea698b80b4e765f98dd0799a",
}


def shizuku_jars(build_dir: Path) -> list[Path]:
    """classes.jar of the Shizuku client library (adb-level shell commands for the launcher's RAM setting), fetched
    once from Maven Central and pinned by SHA-1."""
    import hashlib
    import urllib.request
    out = []
    for art, sha1 in SHIZUKU_AARS.items():
        aar = CACHE_ROOT / f"shizuku-{art}-{SHIZUKU_VERSION}.aar"
        if not aar.is_file():
            aar.parent.mkdir(parents=True, exist_ok=True)
            url = f"https://repo1.maven.org/maven2/dev/rikka/shizuku/{art}/{SHIZUKU_VERSION}/{art}-{SHIZUKU_VERSION}.aar"
            with urllib.request.urlopen(url, timeout=120) as r:
                aar.write_bytes(r.read())
        got = hashlib.sha1(aar.read_bytes()).hexdigest()
        if got != sha1:
            aar.unlink()
            raise SystemExit(f"{aar.name} has SHA-1 {got}, expected {sha1}")
        jar = build_dir / f"shizuku-{art}.jar"
        with zipfile.ZipFile(aar) as z:
            jar.write_bytes(z.read("classes.jar"))
        out.append(jar)
    return out


def make_engine_resources_zip(res_dir: Path, client_jar: Path, dest: Path) -> int:
    """Zip the library resources of an engine build: every file of `res_dir` (the engine's MONO_RESOURCES tree)
    that the Minecraft client jar does not provide. The launcher extracts the jar part itself at run time, so
    Mojang content never ships inside the APK. Native libraries of other platforms are left out."""
    with zipfile.ZipFile(client_jar) as z:
        jar_names = {i.filename for i in z.infolist()}
        jar_sizes = {i.filename: i.file_size for i in z.infolist()}
    skip_ext = (".so", ".a", ".dll", ".dylib", ".jnilib")
    count = 0
    with zipfile.ZipFile(dest, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as zf:
        for path in sorted(res_dir.rglob("*")):
            if not path.is_file():
                continue
            rel = path.relative_to(res_dir).as_posix()
            if rel.startswith("."):
                continue
            if rel in jar_names:
                # markalama (Monolith başlık dokuları) motor derlemesindeki bir jar dosyasını değiştirdi: geçersiz kılma olarak gönder
                if path.stat().st_size != jar_sizes[rel] and rel.startswith("assets/minecraft/textures/gui/title/"):
                    zf.write(path, "__override__/" + rel)
                    count += 1
                continue
            # JNA (oshi, anlatıcı) com/sun/jna/linux-aarch64/libjnidispatch.so dosyasını kaynak ağacından yükler.
            # Oyunun kendi dosyası Android'in yükleyemediği bir glibc derlemesidir: Android derlemesi onu değiştirir
            # (aşağıda). Diğer her platformun kütüphaneleri dışarıda bırakılır.
            if rel.endswith(skip_ext) and (rel.startswith("com/sun/jna/") or rel.startswith("META-INF/native/")):
                continue
            zf.write(path, rel)
            count += 1
        zf.writestr("com/sun/jna/linux-aarch64/libjnidispatch.so", jna_android_dispatch_lib())
        count += 1
        # motorun java.home'u: motor derlemesinin kaynaklarının yanında JDK'nın conf/ ağacı ve lib/tzdb.dat
        # Derleyici (rustgen) bu dosyaları <çıktı>/rust/ altına kopyalar; eski düzende <çıktı>/ altındaydı.
        # İkisine de bakılır: önce <çıktı>/, yoksa <çıktı>/rust/.
        jhome = res_dir.parent
        if not (jhome / "conf" / "security" / "java.security").is_file() and (jhome / "rust" / "conf").is_dir():
            jhome = jhome / "rust"
        jfiles = [p for p in sorted((jhome / "conf").rglob("*")) if p.is_file()] + [jhome / "lib" / "tzdb.dat"]
        if not (jhome / "conf" / "security" / "java.security").is_file() or not (jhome / "lib" / "tzdb.dat").is_file():
            raise SystemExit(f"engine build {jhome} has no conf/security/java.security or lib/tzdb.dat (java.home files)")
        for path in jfiles:
            zf.write(path, "__javahome__/" + path.relative_to(jhome).as_posix())
            count += 1
    return count


def build_apk(out_apk: Path, engine_resources: Path | None = None, client_jar: Path | None = None,
              shader_lib: Path | None = None) -> None:
    android_jar, r8_jar, bt_bin = find_sdk_paths()
    bt_cands = [bt_bin] if bt_bin else []

    aapt = find_tool("aapt", [*(c / "aapt" for c in bt_cands), Path("/usr/bin/aapt")])
    zipalign = find_tool("zipalign", [*(c / "zipalign" for c in bt_cands), Path("/usr/bin/zipalign")])
    apksigner = find_tool("apksigner", [*(c / "apksigner" for c in bt_cands), Path("/usr/bin/apksigner")])
    javac = find_jdk_bin("javac")
    java = find_jdk_bin("java")
    keytool = find_jdk_bin("keytool")

    build_dir = REPO_ROOT / "build" / "touchcore"
    if build_dir.exists():
        shutil.rmtree(build_dir)
    build_dir.mkdir(parents=True, exist_ok=True)

    build_res = build_dir / "res"
    gen_dir = build_dir / "gen"
    classes_dir = build_dir / "classes"
    dex_dir = build_dir / "dex"
    lib_dir = build_dir / "lib" / "arm64-v8a"
    for d in (gen_dir, classes_dir, dex_dir, lib_dir):
        d.mkdir(parents=True, exist_ok=True)

    prepare_resources(build_res)
    so_path = lib_dir / "libtouchcore_mono.so"
    build_native_so(build_dir, so_path)

    base_ap_ = build_dir / "base.ap_"
    run_step(
        "Packaging resources & AndroidManifest via aapt",
        [
            aapt,
            "package",
            "-f",
            "-m",
            "-M",
            str(TOUCHCORE_DIR / "AndroidManifest.xml"),
            "-S",
            str(build_res),
            "-I",
            str(android_jar),
            "-J",
            str(gen_dir),
            "-F",
            str(base_ap_),
        ],
    )

    java_sources = [str(p) for p in sorted(gen_dir.rglob("*.java"))]
    java_sources += [str(p) for p in sorted((TOUCHCORE_DIR / "src").rglob("*.java"))]
    lib_jars = shizuku_jars(build_dir)
    run_step(
        f"Compiling {len(java_sources)} Java files (release 17, low-RAM)",
        [
            javac,
            "--release",
            "17",
            "-encoding",
            "UTF-8",
            "-J-Xmx192m",
            "-J-XX:+UseSerialGC",
            "-cp",
            os.pathsep.join([str(android_jar), *map(str, lib_jars)]),
            "-d",
            str(classes_dir),
            *java_sources,
        ],
    )

    class_files = [str(p) for p in sorted(classes_dir.rglob("*.class"))]
    run_step(
        f"Dexing {len(class_files)} class files via D8",
        [
            java,
            "-Xmx256m",
            "-XX:+UseSerialGC",
            "-cp",
            str(r8_jar),
            "com.android.tools.r8.D8",
            "--release",
            "--min-api",
            "26",
            "--lib",
            str(android_jar),
            "--output",
            str(dex_dir),
            *class_files,
            *map(str, lib_jars),
        ],
    )

    unsigned_apk = build_dir / "unsigned.apk"
    shutil.copy2(base_ap_, unsigned_apk)
    with zipfile.ZipFile(unsigned_apk, "a") as zf:
        for dex in sorted(dex_dir.glob("*.dex")):
            zf.write(dex, dex.name, compress_type=zipfile.ZIP_DEFLATED)
        if engine_resources is not None:
            assert client_jar is not None
            res_zip = build_dir / "engine_resources.zip"
            n = make_engine_resources_zip(engine_resources, client_jar, res_zip)
            if n == 0:
                raise SystemExit("--engine-resources: no library resources found (is this an engine build's resources dir?)")
            print(f"[touchcore] bundling {n} library resource files ({res_zip.stat().st_size // 1024} KiB) as assets/engine_resources.zip")
            zf.write(res_zip, "assets/engine_resources.zip", compress_type=zipfile.ZIP_STORED)
        if shader_lib is not None:
            # motor için shaderc + SPIRV-Cross (android/shader, tools/build_shader_lib.py)
            zf.write(shader_lib, "lib/arm64-v8a/libmonoshader.so", compress_type=zipfile.ZIP_STORED)
        # Android 15 16KB sayfa eşleme + dlopen doğrudan çalışsın diye yerel .so dosyalarını sıkıştırmadan sakla
        zf.write(so_path, "lib/arm64-v8a/libtouchcore_mono.so", compress_type=zipfile.ZIP_STORED)

    aligned_apk = build_dir / "aligned.apk"
    align_cmd = [zipalign, "-f"]
    if zipalign_supports_page_flag(zipalign):
        align_cmd.extend(["-P", "16"])
    else:
        align_cmd.extend(["-p"])
    align_cmd.extend(["4", str(unsigned_apk), str(aligned_apk)])
    run_step("Aligning APK for Android Runtime", align_cmd)

    ensure_keystore(keytool)
    out_apk.parent.mkdir(parents=True, exist_ok=True)
    run_step(
        "Signing APK with apksigner",
        [
            apksigner,
            "sign",
            "--ks",
            str(KEYSTORE_PATH),
            "--ks-key-alias",
            KEY_ALIAS,
            "--ks-pass",
            f"pass:{KEY_PASS}",
            "--key-pass",
            f"pass:{KEY_PASS}",
            "--out",
            str(out_apk),
            str(aligned_apk),
        ],
    )

    verify_out = run_step(
        "Verifying APK signature",
        [apksigner, "verify", "--verbose", "--print-certs", str(out_apk)],
    )
    print(verify_out.strip())
    size_mb = out_apk.stat().st_size / (1024.0 * 1024.0)
    print(f"[touchcore] SUCCESS: {out_apk} ({size_mb:.2f} MiB)")


def main() -> int:
    ap = argparse.ArgumentParser(description="Build TouchCore For Monolith APK")
    ap.add_argument("--out", type=Path, default=DEFAULT_OUT_APK, help="Output APK path")
    ap.add_argument("--engine-resources", type=Path, default=None,
                    help="resources dir of an engine build (<out>/resources); its library files ship as assets/engine_resources.zip")
    ap.add_argument("--shader-lib", type=Path, default=None,
                    help="libmonoshader.so for arm64-v8a (android/shader), packaged next to the launcher library")
    ap.add_argument("--client-jar", type=Path, default=None,
                    help="client.jar the engine was built from (needed with --engine-resources)")
    args = ap.parse_args()
    if (args.engine_resources is None) != (args.client_jar is None):
        ap.error("--engine-resources and --client-jar go together")
    build_apk(args.out.resolve(), args.engine_resources, args.client_jar, args.shader_lib)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
