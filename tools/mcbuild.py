#!/usr/bin/env python3
"""One-shot Minecraft Java Edition -> native build.

    ./build.sh  /path/to/client.jar            (Linux / macOS)
    build.bat   C:\\path\\to\\client.jar          (Windows)

Steps (all cached under ~/.cache/monolith, every download verified by SHA-1):
  1. read version.json inside the JAR (version id, required Java version)
  2. get the version metadata (local .minecraft/versions/<id>/<id>.json or Mojang's manifest)
  3. provide a matching JDK (MONO_JDK_HOME, or download Eclipse Temurin <java_version>)
  4. download the libraries for this OS/arch and extract their native libraries
  5. download the asset index + objects (unless --skip-assets)
  6. extract the JAR's resources (assets/data/...) for the native ClassLoader
  7. transpile with monolith and compile with Zig (ReleaseFast by default)
  8. write a launcher (run.sh / run.bat) that starts the game with the chosen graphics API

Nothing produced here may be redistributed (Minecraft EULA); output stays local.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import time
import urllib.request
import zipfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CACHE = os.path.join(os.path.expanduser("~"), ".cache", "monolith")
MANIFEST = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"
ASSET_BASE = "https://resources.download.minecraft.net"
MAIN_CLASS = "net.minecraft.client.main.Main"
# Cloudflare on the Adoptium/Mojang endpoints 403s the default "python-urllib" UA; identify ourselves.
UA = {"User-Agent": "monolith-mcbuild/0.15.1"}


def log(msg: str):
    print(f"[mcbuild] {msg}", flush=True)


# ----------------------------------------------------------------------------- downloads
def sha1_of(path: str) -> str:
    h = hashlib.sha1()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def sha256_of(path: str) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def _first_redirect(url: str) -> str:
    """The URL `url` redirects to (the Adoptium API 307s to the GitHub release asset)."""
    class _NoRedirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self, req, fp, code, msg, headers, newurl):
            return None

    opener = urllib.request.build_opener(_NoRedirect)
    try:
        with opener.open(urllib.request.Request(url, headers=UA), timeout=30) as r:
            return r.geturl()  # no redirect happened
    except urllib.error.HTTPError as e:
        loc = e.headers.get("Location")
        if loc:
            return loc
        raise SystemExit(f"[mcbuild] could not resolve {url}: HTTP {e.code}")
    except OSError as e:
        raise SystemExit(f"[mcbuild] could not resolve {url}: {e}")


def _jdk_expected_sha256(asset_url: str) -> str:
    """Temurin publishes <asset>.sha256.txt next to every release asset."""
    sha_url = asset_url + ".sha256.txt"
    try:
        with urllib.request.urlopen(urllib.request.Request(sha_url, headers=UA), timeout=60) as r:
            body = r.read().decode("utf-8", "replace")
    except (urllib.error.URLError, OSError) as e:
        raise SystemExit(f"[mcbuild] could not fetch the JDK checksum ({sha_url}): {e}\n"
                         f"Set MONO_JDK_NO_CHECKSUM=1 to bypass (integrity NOT verified), or point MONO_JDK_HOME at a local JDK.")
    m = re.search(r"\b([0-9a-fA-F]{64})\b", body)
    if not m:
        raise SystemExit(f"[mcbuild] unreadable SHA-256 from {sha_url}: {body[:120]!r}")
    return m.group(1).lower()


def _safe_extract_zip(z: zipfile.ZipFile, root: str) -> None:
    root_abs = os.path.realpath(root)
    for e in z.infolist():
        dest = os.path.realpath(os.path.join(root, e.filename))
        if dest != root_abs and not dest.startswith(root_abs + os.sep):
            raise SystemExit(f"[mcbuild] unsafe path in archive: {e.filename!r}")
    z.extractall(root)


def _safe_extract_tar(t: tarfile.TarFile, root: str) -> None:
    root_abs = os.path.realpath(root)
    try:
        t.extractall(root, filter="data")  # Python >= 3.12: blocks absolute paths, '..', escaping links
        return
    except TypeError:
        pass  # Python 3.10/3.11: fall back to manual containment checks
    for m in t.getmembers():
        dest = os.path.realpath(os.path.join(root, m.name))
        if dest != root_abs and not dest.startswith(root_abs + os.sep):
            raise SystemExit(f"[mcbuild] unsafe path in JDK archive: {m.name!r}")
        if m.issym() or m.islnk():
            link_dest = os.path.realpath(os.path.join(os.path.dirname(dest), m.linkname))
            if link_dest != root_abs and not link_dest.startswith(root_abs + os.sep):
                raise SystemExit(f"[mcbuild] unsafe link in JDK archive: {m.name!r} -> {m.linkname!r}")
    t.extractall(root)


def fetch(url: str, dest: str, sha1: str | None = None, retries: int = 4) -> str:
    if os.path.isfile(dest) and (sha1 is None or sha1_of(dest) == sha1):
        return dest
    os.makedirs(os.path.dirname(dest), exist_ok=True)
    tmp = dest + ".part"
    for attempt in range(retries):
        try:
            req = urllib.request.Request(url, headers=UA)
            with urllib.request.urlopen(req, timeout=120) as r, open(tmp, "wb") as fh:
                shutil.copyfileobj(r, fh, 1 << 20)
            if sha1 is not None and sha1_of(tmp) != sha1:
                raise IOError(f"SHA-1 mismatch for {url}")
            os.replace(tmp, dest)
            return dest
        except Exception as e:  # noqa: BLE001 - retried, then reported
            if attempt == retries - 1:
                try:
                    os.remove(tmp)
                except OSError:
                    pass
                raise SystemExit(f"[mcbuild] download failed: {url}: {e}")
            time.sleep(2 ** attempt)
    return dest


def fetch_json(url: str, dest: str, sha1: str | None = None) -> dict:
    with open(fetch(url, dest, sha1), encoding="utf-8") as fh:
        return json.load(fh)


# ----------------------------------------------------------------------------- platform
_TARGET_ARCH: str | None = None


def mojang_os() -> str:
    s = platform.system().lower()
    return {"linux": "linux", "darwin": "osx", "windows": "windows"}.get(s, s)


def mojang_arch(for_target: bool = False) -> str:
    if for_target and _TARGET_ARCH:
        return _TARGET_ARCH
    m = platform.machine().lower()
    return "arm64" if m in ("aarch64", "arm64") else "x86_64"


def rules_allow(rules) -> bool:
    if not rules:
        return True
    allowed = False
    arch = mojang_arch(for_target=True)
    for r in rules:
        osr = r.get("os")
        match = True
        if osr:
            if "name" in osr and osr["name"] != mojang_os():
                match = False
            if "arch" in osr and osr["arch"] not in (arch, platform.machine().lower()):
                match = False
        if "features" in r:
            match = False  # launcher features (demo, custom resolution, ...) are off
        if match:
            allowed = r.get("action") == "allow"
    return allowed


def native_classifier_ok(name: str) -> bool:
    """LWJGL style 'group:artifact:version:natives-linux[-arm64]' names."""
    parts = name.split(":")
    if len(parts) < 4:
        return True
    cls = parts[3]
    if not cls.startswith("natives-"):
        return True
    want = {"linux": "natives-linux", "osx": "natives-macos", "windows": "natives-windows"}[mojang_os()]
    if mojang_arch(for_target=True) == "arm64":
        return cls == want + "-arm64"
    return cls == want


# ----------------------------------------------------------------------------- steps
def read_jar_version(jar: str) -> dict:
    with zipfile.ZipFile(jar) as z:
        try:
            return json.loads(z.read("version.json"))
        except KeyError:
            raise SystemExit("[mcbuild] version.json not found in the JAR: is this a Minecraft client.jar?")


def version_metadata(vid: str, mc_dir: str | None) -> dict:
    if mc_dir:
        local = os.path.join(mc_dir, "versions", vid, vid + ".json")
        if os.path.isfile(local):
            log(f"using local version metadata {local}")
            with open(local, encoding="utf-8") as fh:
                return json.load(fh)
    man_path = os.path.join(CACHE, "mc", "version_manifest_v2.json")
    if os.path.isfile(man_path):
        log(f"using cached version manifest ({man_path}); delete it to refresh")
    man = fetch_json(MANIFEST, man_path)
    for v in man["versions"]:
        if v["id"] == vid:
            return fetch_json(v["url"], os.path.join(CACHE, "mc", "versions", vid + ".json"), v.get("sha1"))
    raise SystemExit(f"[mcbuild] version {vid} is not in Mojang's manifest; pass --mc-dir with a local copy")


def ensure_jdk(java_version: int) -> str:
    env = os.environ.get("MONO_JDK_HOME")
    if env and os.path.isfile(os.path.join(env, "bin", "javac")):
        return env
    root = os.path.join(CACHE, f"temurin-{java_version}")
    if os.path.isdir(root):
        for d in os.listdir(root):
            if os.path.isfile(os.path.join(root, d, "bin", "javac")):
                return os.path.join(root, d)
    osn = {"linux": "linux", "osx": "mac", "windows": "windows"}[mojang_os()]
    arch = "aarch64" if mojang_arch() == "arm64" else "x64"
    url = f"https://api.adoptium.net/v3/binary/latest/{java_version}/ga/{osn}/{arch}/jdk/hotspot/normal/eclipse"
    log(f"downloading JDK {java_version} (Eclipse Temurin)")
    archive = fetch(url, os.path.join(root, "jdk.tar.gz" if osn != "windows" else "jdk.zip"))
    if os.environ.get("MONO_JDK_NO_CHECKSUM") == "1":
        log("WARNING: MONO_JDK_NO_CHECKSUM=1 - JDK archive integrity NOT verified")
    else:
        asset_url = _first_redirect(url)
        expected = _jdk_expected_sha256(asset_url)
        actual = sha256_of(archive)
        if actual != expected:
            try:
                os.remove(archive)
            except OSError:
                pass
            raise SystemExit(f"[mcbuild] JDK SHA-256 mismatch for {asset_url}\n"
                             f"expected {expected}\nactual   {actual}\n"
                             f"The archive was deleted. Set MONO_JDK_HOME to a known-good JDK instead.")
        log("JDK SHA-256 verified")
    if archive.endswith(".zip"):
        with zipfile.ZipFile(archive) as z:
            _safe_extract_zip(z, root)
    else:
        with tarfile.open(archive) as t:
            _safe_extract_tar(t, root)
    os.remove(archive)
    for d in os.listdir(root):
        if os.path.isfile(os.path.join(root, d, "bin", "javac")):
            return os.path.join(root, d)
    raise SystemExit(f"[mcbuild] JDK extracted but bin/javac not found under {root}; "
                     f"pass MONO_JDK_HOME pointing at a local JDK {java_version}")


def download_libraries(meta: dict, natives_dir: str, skip_natives: bool = False) -> list[str]:
    jars: list[str] = []
    libdir = os.path.join(CACHE, "mc", "libraries")
    os.makedirs(natives_dir, exist_ok=True)
    for lib in meta["libraries"]:
        if not rules_allow(lib.get("rules")) or not native_classifier_ok(lib["name"]):
            continue
        is_native = ":natives-" in lib["name"]
        if skip_natives and is_native:
            continue
        art = lib.get("downloads", {}).get("artifact")
        if not art:
            continue
        path = fetch(art["url"], os.path.join(libdir, art["path"]), art.get("sha1"))
        if is_native:
            with zipfile.ZipFile(path) as z:
                for e in z.namelist():
                    if e.endswith((".so", ".dylib", ".dll")) and not e.startswith("META-INF"):
                        target = os.path.join(natives_dir, os.path.basename(e))
                        with z.open(e) as src, open(target, "wb") as dst:
                            shutil.copyfileobj(src, dst)
        else:
            jars.append(path)
    return jars


def download_assets(meta: dict, assets_dir: str):
    idx = meta["assetIndex"]
    index = fetch_json(idx["url"], os.path.join(assets_dir, "indexes", idx["id"] + ".json"), idx.get("sha1"))
    objs = index["objects"]
    log(f"assets: {len(objs)} objects (index {idx['id']})")
    from concurrent.futures import ThreadPoolExecutor

    def one(item):
        h = item["hash"]
        fetch(f"{ASSET_BASE}/{h[:2]}/{h}", os.path.join(assets_dir, "objects", h[:2], h), h)
    with ThreadPoolExecutor(16) as ex:
        list(ex.map(one, objs.values()))


def native_callback_classes(natives_dir: str, jars: list[str]) -> list[str]:
    """Java classes that native libraries refer to by name (FindClass/GetMethodID strings in the
    .so files): they may be called back from C, so all their methods are kept."""
    import re
    known: set[str] = set()
    for j in jars:
        try:
            for n in zipfile.ZipFile(j).namelist():
                if n.endswith(".class") and not n.startswith("META-INF/"):
                    known.add(n[:-6])
        except (OSError, zipfile.BadZipFile):
            continue
    pat = re.compile(rb"(?:org|com|net|io|it)/[A-Za-z0-9_/$]{3,160}")
    found: set[str] = set()
    for root, _, files in os.walk(natives_dir):
        for f in files:
            if not f.endswith((".so", ".dylib", ".dll")):
                continue
            with open(os.path.join(root, f), "rb") as fh:
                data = fh.read()
            for m in pat.findall(data):
                name = m.decode("ascii")
                if name in known:
                    found.add(name)
    return sorted(found)


NETTY_CHANNELS = {c + ".class" for c in (
    "io/netty/channel/local/LocalServerChannel", "io/netty/channel/local/LocalChannel",
    "io/netty/channel/socket/nio/NioSocketChannel", "io/netty/channel/socket/nio/NioServerSocketChannel",
    "io/netty/channel/socket/nio/NioDatagramChannel", "io/netty/channel/epoll/EpollSocketChannel",
    "io/netty/channel/epoll/EpollServerSocketChannel", "io/netty/channel/epoll/EpollDatagramChannel")}


def reflection_metadata(jars: list[str]) -> list[str]:
    """Classes instantiated reflectively by name from library metadata (closed-world reachability
    metadata): log4j plugins listed in META-INF/.../Log4j2Plugins.dat."""
    import io
    import struct
    out: list[str] = []
    for j in jars:
        try:
            z = zipfile.ZipFile(j)
        except (OSError, zipfile.BadZipFile):
            continue
        for n in z.namelist():
            # LWJGL picks its memory backend by name (Class.forName with a computed version suffix)
            if n.startswith("org/lwjgl/system/MemoryBackend") and n.endswith(".class") and "$" not in n:
                out.append(n[:-6])
            # JNA instantiates its own Structure subclasses reflectively (Klass.newInstance)
            if n.startswith("com/sun/jna/Structure$FFIType") and n.endswith(".class"):
                out.append(n[:-6])
            # netty's ReflectiveChannelFactory instantiates the channel class passed to .channel(Class)
            if n in NETTY_CHANNELS or (n.startswith("io/netty/util/internal/CleanerJava") and n.endswith(".class")):
                out.append(n[:-6])  # CleanerJava*: direct-buffer cleaners built through method handles
            if not n.endswith("/Log4j2Plugins.dat"):
                continue
            b = io.BytesIO(z.read(n))

            def u4():
                return struct.unpack(">i", b.read(4))[0]

            def utf():
                ln = struct.unpack(">H", b.read(2))[0]
                return b.read(ln).decode("utf-8")
            for _ in range(u4()):
                utf()
                for _ in range(u4()):
                    utf()
                    out.append(utf())
                    utf()
                    b.read(2)
    return sorted(set(out))


MODDED_LABEL = " Rust\u00a76+"  # version line of the title screen: "Minecraft 26.3 Rust+" with a gold plus


def _patch_modded_label(path: str, src: str | None = None) -> bool:
    """Set menu.modded in a lang file (read from `src`, default `path`) and write it to `path`."""
    try:
        with open(src or path, encoding="utf-8") as fh:
            lang = json.load(fh)
    except (OSError, ValueError):
        return False
    if "menu.modded" not in lang:
        return False
    lang["menu.modded"] = MODDED_LABEL
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as fh:
        json.dump(lang, fh, ensure_ascii=False, indent=2)
    return True


def apply_branding(resources: str, assets: str | None = None):
    """Monolith title screen: the Monolith logo (brand/title.png) replaces the vanilla "MINECRAFT" title
    texture, the "JAVA EDITION" subtitle texture becomes transparent (the logo carries its own edition line),
    and the "(Modded)" suffix of the version line reads "Rust+"."""
    d = os.path.join(resources, "assets", "minecraft", "textures", "gui", "title")
    if not os.path.isdir(d):
        log("branding: no title textures in the resources, skipped")
        return
    for src, names in (("title.png", ("minecraft.png", "minceraft.png")), ("edition.png", ("edition.png",))):
        src_path = os.path.join(ROOT, "brand", src)
        if not os.path.isfile(src_path):
            continue
        for n in names:
            shutil.copyfile(src_path, os.path.join(d, n))
    lang_dir = os.path.join(resources, "assets", "minecraft", "lang")
    n_lang = int(_patch_modded_label(os.path.join(lang_dir, "en_us.json")))
    if assets:  # the other languages live in the asset index, not in the jar
        idx_dir = os.path.join(assets, "indexes")
        for idx in (os.listdir(idx_dir) if os.path.isdir(idx_dir) else []):
            with open(os.path.join(idx_dir, idx), encoding="utf-8") as fh:
                objs = json.load(fh).get("objects", {})
            for name, o in objs.items():
                if name.startswith("minecraft/lang/") and name.endswith(".json") and not name.endswith("en_us.json"):
                    h = o["hash"]
                    obj = os.path.join(assets, "objects", h[:2], h)
                    n_lang += _patch_modded_label(os.path.join(resources, "assets", name), obj)
    log(f"branding: Monolith title screen applied ({n_lang} language files)")


def extract_resources(jars, dest: str):
    """Non-class entries of the client JAR and the libraries (JNA's bundled libjnidispatch.so,
    log4j/gson metadata, ...), served by the native ClassLoader (MONO_RESOURCES). The client JAR
    is listed first and wins on name clashes; the multi-release META-INF/versions tree is skipped."""
    if isinstance(jars, str):
        jars = [jars]
    stamp = os.path.join(dest, ".source")
    ident = "|".join(f"{os.path.abspath(j)}:{os.path.getsize(j)}:{os.path.getmtime(j)}" for j in jars)
    if os.path.isfile(stamp) and open(stamp).read() == ident:
        return
    if os.path.isdir(dest):
        shutil.rmtree(dest)
    os.makedirs(dest)
    for jar in jars:
        try:
            z = zipfile.ZipFile(jar)
        except (OSError, zipfile.BadZipFile):
            continue
        with z:
            for e in z.infolist():
                n = e.filename
                if e.is_dir() or n.endswith(".class") or n.startswith("META-INF/versions/"):
                    continue
                if n.upper().startswith("META-INF/") and n.upper().endswith((".SF", ".RSA", ".DSA", ".MF")):
                    continue
                out = os.path.realpath(os.path.join(dest, n))
                root_abs = os.path.realpath(dest)
                if out != root_abs and not out.startswith(root_abs + os.sep):
                    raise SystemExit(f"[mcbuild] unsafe path in resource archive ({n!r} in {jar})")
                if os.path.exists(out):
                    continue
                os.makedirs(os.path.dirname(out), exist_ok=True)
                with z.open(e) as src_fh, open(out, "wb") as dst_fh:
                    shutil.copyfileobj(src_fh, dst_fh)
    with open(stamp, "w") as fh:
        fh.write(ident)


def write_launchers(out: str, exe: str, vid: str, meta: dict, graphics: str, no_lwjgl: bool = False):
    asset_index = meta["assetIndex"]["id"]
    if no_lwjgl:
        sh_extra_env = f'export MONO_RHI=1\nexport MONO_NO_LWJGL=1\nexport MONO_RHI_BACKEND="${{MC_GRAPHICS:-{graphics}}}"\n'
        sh_props = f'-Djna.boot.library.path="$HERE/natives" -Djna.nosys=false -Dmono.graphics.backend="${{MC_GRAPHICS:-{graphics}}}"'
        bat_extra_env = 'set MONO_RHI=1\r\nset MONO_NO_LWJGL=1\r\nset MONO_RHI_BACKEND=%MC_GRAPHICS%\r\n'
        bat_props = '-Djna.boot.library.path="%HERE%natives" -Djna.nosys=false -Dmono.graphics.backend=%MC_GRAPHICS%'
    else:
        sh_extra_env = ""
        sh_props = (f'-Djava.library.path="$HERE/natives" -Dorg.lwjgl.librarypath="$HERE/natives" '
                    f'-Djna.boot.library.path="$HERE/natives" -Djna.nosys=false -Dorg.lwjgl.system.allocator=system '
                    f'-Dmono.graphics.backend="${{MC_GRAPHICS:-{graphics}}}"')
        bat_extra_env = ""
        bat_props = ('-Djava.library.path="%HERE%natives" -Dorg.lwjgl.librarypath="%HERE%natives" '
                     '-Djna.boot.library.path="%HERE%natives" -Djna.nosys=false -Dorg.lwjgl.system.allocator=system '
                     '-Dmono.graphics.backend=%MC_GRAPHICS%')
    sh = f"""#!/bin/sh
# Generated by tools/mcbuild.py — starts the natively compiled Minecraft {vid}.
HERE="$(cd "$(dirname "$0")" && pwd)"
export MONO_RESOURCES="$HERE/resources"
export LD_LIBRARY_PATH="$HERE/natives${{LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}}"
{sh_extra_env}# X11 servers without a 32-bit sRGB GLX visual need EGL for GLX/SDL window creation
if [ -z "${{SDL_VIDEO_FORCE_EGL+x}}" ] && command -v glxinfo >/dev/null 2>&1; then
  if ! glxinfo -B 2>/dev/null | grep -qi "srgb"; then
    export SDL_VIDEO_FORCE_EGL=1
  fi
fi
GAME_DIR="${{MC_GAME_DIR:-$HERE/game}}"
mkdir -p "$GAME_DIR"
exec "$HERE/{os.path.basename(exe)}" {sh_props} --version {vid} --gameDir "$GAME_DIR" \\
  --assetsDir "$HERE/assets" --assetIndex {asset_index} \\
  --username "${{MC_USERNAME:-Player}}" --accessToken 0 --userType legacy \\
  "$@"
"""
    with open(os.path.join(out, "run.sh"), "w", newline="\n") as fh:
        fh.write(sh)
    os.chmod(os.path.join(out, "run.sh"), 0o755)
    bat = f"""@echo off
rem Generated by tools/mcbuild.py — starts the natively compiled Minecraft {vid}.
set HERE=%~dp0
set MONO_RESOURCES=%HERE%resources
set PATH=%HERE%natives;%PATH%
if "%MC_GAME_DIR%"=="" set MC_GAME_DIR=%HERE%game
if "%MC_USERNAME%"=="" set MC_USERNAME=Player
if "%MC_GRAPHICS%"=="" set MC_GRAPHICS={graphics}
{bat_extra_env}if not exist "%MC_GAME_DIR%" mkdir "%MC_GAME_DIR%"
"%HERE%{os.path.basename(exe)}.exe" {bat_props} --version {vid} --gameDir "%MC_GAME_DIR%" --assetsDir "%HERE%assets" --assetIndex {asset_index} --username "%MC_USERNAME%" --accessToken 0 --userType legacy %*
"""
    with open(os.path.join(out, "run.bat"), "w", newline="\r\n") as fh:
        fh.write(bat)


def main() -> int:
    raw_argv = list(sys.argv[1:])
    build_lib_mode = False
    if raw_argv and raw_argv[0] == "build-lib":
        build_lib_mode = True
        raw_argv = raw_argv[1:]
    norm_argv = ["--dynamic" if x == "-dynamic" else x for x in raw_argv]

    ap = argparse.ArgumentParser(description="Build a native Minecraft executable or Android libminecraft_mono.so from a client.jar")
    ap.add_argument("jar", help="path to the Minecraft client.jar (any version with version.json)")
    ap.add_argument("-o", "--out", help="output directory (default: build/minecraft-<version>)")
    ap.add_argument("-O", "--optimize", default="ReleaseFast",
                    choices=["Debug", "ReleaseSafe", "ReleaseFast", "ReleaseSmall"])
    ap.add_argument("--graphics", default="auto", choices=["auto", "vulkan", "opengl"],
                    help="RHI backend: auto (Vulkan 1.1+ with OpenGL 3.3 Core fallback), vulkan, or opengl")
    ap.add_argument("--no-lwjgl", action="store_true",
                    help="strip external LWJGL native binaries cleanly and route exclusively through MONOLITH RHI")
    ap.add_argument("--dynamic", action="store_true", default=build_lib_mode,
                    help="build shared library libminecraft_mono.so (-dynamic) exporting mono_android_entry")
    ap.add_argument("--target", default="",
                    help="target triple (e.g. aarch64-linux-android.26 or 'android'; auto-selected with --dynamic on Android)")
    ap.add_argument("--mc-dir", help="existing .minecraft directory (offline version metadata)")
    ap.add_argument("--backend", default="auto", choices=["auto", "llvm", "native"],
                    help="Zig code generator (auto: LLVM when the machine has enough RAM, else self-hosted)")
    ap.add_argument("--shards", type=int, default=None, metavar="N",
                    help="split the program into N separately compiled objects: LLVM release builds fit in ~2.5 GiB "
                         "per parallel job instead of >15 GiB (default: 32 for release builds, 0 for Debug)")
    ap.add_argument("--shard-jobs", type=int, default=None,
                    help="shards compiled in parallel (default: from RAM and cores)")
    ap.add_argument("--only-shards", default="", metavar="K,K",
                    help="resume a sharded build: recompile only these shard numbers, reuse the other objects, then link")
    ap.add_argument("--incremental", action="store_true",
                    help="sharded build: reuse the object of every shard whose generated Zig is unchanged (class-level rebuilds)")
    ap.add_argument("--compile-only", action="store_true", help="compile the existing generated Zig of --out, no transpile")
    ap.add_argument("--stamp-only", action="store_true",
                    help="with --compile-only --only-shards: adopt the existing objects of those shards (write their source stamps)")
    ap.add_argument("--skip-assets", action="store_true")
    ap.add_argument("--no-compile", action="store_true", help="only transpile (generate Zig)")
    a = ap.parse_args(norm_argv)

    if a.dynamic and not a.target:
        a.target = "aarch64-linux-android.26"
    if a.dynamic and not a.no_lwjgl:
        a.no_lwjgl = True
        log("Android libminecraft_mono.so (--dynamic): enabling --no-lwjgl automatically")
    is_android_target = a.target.startswith("aarch64-linux-android") or a.target == "android"
    if is_android_target:
        global _TARGET_ARCH
        _TARGET_ARCH = "arm64"
        if a.target == "android":
            a.target = "aarch64-linux-android.26"

    jar = os.path.abspath(a.jar)
    if not os.path.isfile(jar):
        raise SystemExit(f"[mcbuild] not found: {jar}")
    ver = read_jar_version(jar)
    vid = ver.get("id")
    if not vid:
        raise SystemExit("[mcbuild] version.json in the JAR has no 'id': is this a Minecraft client.jar?")
    try:
        java_version = int(ver.get("java_version", 21))
    except (TypeError, ValueError):
        raise SystemExit(f"[mcbuild] unparsable java_version in version.json: {ver.get('java_version')!r}")
    out = os.path.abspath(a.out or os.path.join(ROOT, "build", f"minecraft-{vid}"))
    os.makedirs(out, exist_ok=True)
    log(f"Minecraft {vid} (Java {java_version}) -> {out}")

    meta = version_metadata(vid, a.mc_dir)
    # Cross-compiling for Android uses host JDK; ensure host arch for JDK download, target arch for Mojang libs.
    jdk = ensure_jdk(java_version)
    os.environ["MONO_JDK_HOME"] = jdk
    log(f"JDK: {jdk}")

    libs = download_libraries(meta, os.path.join(out, "natives"), skip_natives=(a.no_lwjgl or is_android_target))
    if a.no_lwjgl:
        nat_dir = os.path.join(out, "natives")
        for f in os.listdir(nat_dir):
            if "lwjgl" in f.lower():
                os.remove(os.path.join(nat_dir, f))
        libs = [j for j in libs if "lwjgl" not in os.path.basename(j).lower()]
        log("stripped external LWJGL binaries cleanly (pure MONOLITH RHI mode)")
    log(f"libraries: {len(libs)} jars, natives in {os.path.join(out, 'natives')}")
    if not a.skip_assets:
        download_assets(meta, os.path.join(out, "assets"))
    extract_resources([jar] + libs, os.path.join(out, "resources"))
    apply_branding(os.path.join(out, "resources"), None if a.skip_assets else os.path.join(out, "assets"))

    reflect_file = os.path.join(out, "reflect-classes.txt")
    with open(reflect_file, "w") as fh:
        names = set(reflection_metadata([jar] + libs))
        for c in sorted(names):
            fh.write(c + "\n")
    subcmd = "build-lib" if a.dynamic else "build"
    out_name = "minecraft_mono" if a.dynamic else "minecraft"
    cmd = [sys.executable, "-m", "monolith", subcmd, "--cp", os.pathsep.join([jar] + libs),
           "--reflect-class", "@" + reflect_file,
           "--native-libs", os.path.join(out, "natives"), "--native-libs", os.path.join(out, "resources"),
           "--main", MAIN_CLASS, "-o", out, "--name", out_name, "-O", a.optimize, "--show-issues", "5",
           # LWJGL: use the JNI flavour (direct C calls generated by monolith), not the FFM one
           "--release", str(java_version), "--multi-release", "lwjgl=11"]
    if a.dynamic:
        cmd.append("--dynamic")
    if a.target:
        cmd += ["--target", a.target]
    if a.no_lwjgl:
        cmd.append("--no-lwjgl")
    if a.no_compile:
        cmd.append("--no-compile")
    if a.only_shards:
        cmd += ["--only-shards", a.only_shards]
    if a.incremental:
        cmd.append("--incremental")
    if a.compile_only:
        cmd.append("--compile-only")
    if a.stamp_only:
        cmd.append("--stamp-only")
    shards = a.shards if a.shards is not None else (0 if a.optimize == "Debug" else 32)
    jobs = a.shard_jobs
    if jobs is None:
        try:
            mem_gib = os.sysconf("SC_PAGE_SIZE") * os.sysconf("SC_PHYS_PAGES") / 2**30
        except (ValueError, OSError):
            mem_gib = 8
        jobs = max(1, min(os.cpu_count() or 2, int((mem_gib - 2) // 3)))
    cmd += ["--backend", a.backend, "--shards", str(shards), "--shard-jobs", str(jobs)]
    if shards:
        log(f"sharded build: {shards} shards, {jobs} compiled in parallel")
    log("transpiling: " + " ".join(cmd[:4]) + " ...")
    env = {k: v for k, v in os.environ.items() if k != "JAVA_TOOL_OPTIONS"}
    r = subprocess.run(cmd, cwd=ROOT, env=env)
    if r.returncode != 0:
        log(f"monolith failed (exit {r.returncode}); see {os.path.join(out, 'report.json')}")
        return r.returncode
    target_bin = os.path.join(out, "libminecraft_mono.so" if a.dynamic else "minecraft")
    write_launchers(out, target_bin, vid, meta, a.graphics, no_lwjgl=a.no_lwjgl)
    if a.no_compile:
        log(f"done (transpile only): Zig sources in {os.path.join(out, 'src')}")
    elif a.dynamic:
        log(f"done Android shared library: {target_bin} (import in TouchCore APK settings)")
    else:
        log(f"done. start with: {os.path.join(out, 'run.sh' if mojang_os() != 'windows' else 'run.bat')}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
