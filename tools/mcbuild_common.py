"""Shared Minecraft client helpers for tools/mcbuild_rust.py: version metadata, library and asset download,
JDK provisioning, resource extraction and branding. Extracted from the removed Zig pipeline (tools/mcbuild.py);
nothing here depends on a code generator.
"""
from __future__ import annotations
import hashlib
import json
import os
import platform
import re
import shutil
import tarfile
import time
import urllib.request
import zipfile


ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


CACHE = os.path.join(os.path.expanduser("~"), ".cache", "monolith")


MANIFEST = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"


ASSET_BASE = "https://resources.download.minecraft.net"


MAIN_CLASS = "net.minecraft.client.main.Main"


UA = {"User-Agent": "monolith-mcbuild/0.15.1"}

_TARGET_ARCH: str | None = None  # set by the caller for cross builds (see tools/mcbuild_rust.py)


def log(msg: str):
    print(f"[mcbuild] {msg}", flush=True)


_HEX40 = re.compile(r"^[0-9a-fA-F]{40}$")
_NAME = re.compile(r"^[A-Za-z0-9._+ -]{1,64}$")


def require_https(url: str) -> str:
    """Downloads go over https only: urllib would also open file:// and ftp:// URLs, and a URL comes from a manifest."""
    if not url.lower().startswith("https://"):
        raise SystemExit(f"[mcbuild] refusing to download from a non-https URL: {url!r}")
    return url


def safe_join(root: str, rel: str) -> str:
    """root/rel, refused when it resolves outside root ('..', an absolute rel: os.path.join would drop root for it)."""
    root_abs = os.path.realpath(root)
    p = os.path.realpath(os.path.join(root_abs, rel.lstrip("/\\")))
    if not rel or p == root_abs or not p.startswith(root_abs + os.sep):
        raise SystemExit(f"[mcbuild] path from the manifest leaves its directory: {rel!r}")
    return p


def safe_name(name: str) -> str:
    """A version or asset-index id that is usable as a file name part."""
    if not _NAME.match(name) or ".." in name or name == ".":
        raise SystemExit(f"[mcbuild] unsafe id in the manifest: {name!r}")
    return name


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
    require_https(url)
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


def mojang_os() -> str:
    s = platform.system().lower()
    if s == "darwin":
        raise RuntimeError("macOS is not a supported build host (removed from this project); use Linux or Windows")
    return {"linux": "linux", "windows": "windows"}.get(s, s)


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
    want = {"linux": "natives-linux", "windows": "natives-windows"}[mojang_os()]
    if mojang_arch(for_target=True) == "arm64":
        return cls == want + "-arm64"
    return cls == want


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
            return fetch_json(v["url"], os.path.join(CACHE, "mc", "versions", safe_name(vid) + ".json"), v.get("sha1"))
    raise SystemExit(f"[mcbuild] version {vid} is not in Mojang's manifest; pass --mc-dir with a local copy")


def _jdk_home_under(root: str):
    """The JDK home below `root`: the directory whose bin/ holds javac (javac.exe on Windows). The search goes a few
    levels deep, since archives may wrap the JDK in a versioned directory."""
    if not os.path.isdir(root):
        return None
    base_depth = root.rstrip(os.sep).count(os.sep)
    for dirpath, dirnames, filenames in os.walk(root):
        if os.path.basename(dirpath) == "bin" and ("javac" in filenames or "javac.exe" in filenames):
            return os.path.dirname(dirpath)
        if dirpath.count(os.sep) - base_depth >= 4:
            dirnames[:] = []
    return None


def ensure_jdk(java_version: int) -> str:
    env = os.environ.get("MONO_JDK_HOME")
    if env and _jdk_home_under(env) == os.path.abspath(env):
        return env
    root = os.path.join(CACHE, f"temurin-{java_version}")
    found = _jdk_home_under(root)
    if found:
        return found
    osn = {"linux": "linux", "windows": "windows"}[mojang_os()]
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
    found = _jdk_home_under(root)
    if found:
        return found
    raise SystemExit(f"[mcbuild] JDK extracted but javac not found under {root}; "
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
        path = fetch(art["url"], safe_join(libdir, art["path"]), art.get("sha1"))
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
    index = fetch_json(idx["url"], os.path.join(assets_dir, "indexes", safe_name(idx["id"]) + ".json"), idx.get("sha1"))
    objs = index["objects"]
    log(f"assets: {len(objs)} objects (index {idx['id']})")
    from concurrent.futures import ThreadPoolExecutor

    def one(item):
        h = item["hash"]
        if not _HEX40.match(h):  # the hash becomes part of a path and of a URL
            raise SystemExit(f"[mcbuild] asset index has a hash that is not a SHA-1: {h!r}")
        fetch(f"{ASSET_BASE}/{h[:2]}/{h}", os.path.join(assets_dir, "objects", h[:2], h), h)
    with ThreadPoolExecutor(16) as ex:
        list(ex.map(one, objs.values()))


NETTY_CHANNELS = {c + ".class" for c in (
    "io/netty/channel/local/LocalServerChannel", "io/netty/channel/local/LocalChannel",
    "io/netty/channel/socket/nio/NioSocketChannel", "io/netty/channel/socket/nio/NioServerSocketChannel",
    "io/netty/channel/socket/nio/NioDatagramChannel")}
# epoll/kqueue kanalları listelenmez: patches/rust/netty.rs EventLoopGroupHolder.remote()'un her zaman NIO seçmesini sağlar, bu yüzden hiçbir yerel
# kanal sınıfı ReflectiveChannelFactory'ye verilmez (docs/mimari/04_KUTUPHANE_VE_YAMA.md bölüm 6)


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
            # LWJGL bellek arka ucunu adla seçer (hesaplanmış sürüm sonekiyle Class.forName)
            if n.startswith("org/lwjgl/system/MemoryBackend") and n.endswith(".class") and "$" not in n:
                out.append(n[:-6])
            # (JNA'nın Structure$FFIType sınıfları listelenmez: derlemedeki hiçbir şey artık JNA kullanmıyor, bkz. patches/rust/sysinfo.rs
            # ve narrator.rs; onları listelemek com.sun.jna.Native'i ve 55 öykünülmüş yerel metodunu programda tutuyordu)
            # netty'nin ReflectiveChannelFactory'si, .channel(Class) ile verilen kanal sınıfını örnekler
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


MODDED_LABEL = " Rust\u00a76+"
# Ana menüde logonun yanında eğik duran sarı yazı (splash). Minecraft bunu assets/minecraft/texts/splashes.txt
# dosyasından rastgele seçer; dosyada tek satır olunca hep bu yazı görünür. Yalnızca çıkarılmış kaynak kopyası
# değişir, jar'a dokunulmaz. (Oyunun kendi koduna gömülü birkaç özel gün yazısı, ör. 24 Aralık, yine görünür.)
SPLASH_TEXT = "işiniz rust gitsin :)"
# Ana menünün sağ altındaki telif satırı (dil anahtarı `title.credits`; özgünü "Copyright Mojang AB. Do not distribute!").
# Bütün dil dosyalarında bu metinle değiştirilir; jar'a dokunulmaz.
CREDITS_TEXT = "mojang yaptı, cemo ve plisquante modladı"


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
    if "title.credits" in lang:
        lang["title.credits"] = CREDITS_TEXT
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
    sp = os.path.join(resources, "assets", "minecraft", "texts", "splashes.txt")
    if os.path.isfile(sp):
        with open(sp, "w", encoding="utf-8") as fh:
            fh.write(SPLASH_TEXT + "\n")
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
