"""Class path: resolves internal class names to parsed ClassFile objects.

Sources (searched in order):
  * directories of .class files
  * .jar / .zip archives (multi-release JARs: highest META-INF/versions/N <= target wins)
  * .jmod archives (JDK modules; 4-byte 'JM\\x01\\x00' header followed by a zip)
"""
from __future__ import annotations

import io
import os
import zipfile
from typing import Iterator, Optional

from .classfile import ClassFile, parse_class


class _Source:
    def names(self) -> Iterator[str]:
        raise NotImplementedError

    def read(self, name: str) -> Optional[bytes]:
        raise NotImplementedError


class DirSource(_Source):
    def __init__(self, path: str):
        self.path = path
        self.label = path
        self._dir_cache: dict[str, Optional[frozenset[str]]] = {}

    def names(self):
        for root, _, files in os.walk(self.path):
            for f in files:
                if f.endswith(".class") and f != "module-info.class":
                    full = os.path.join(root, f)
                    yield os.path.relpath(full, self.path)[:-6].replace(os.sep, "/")

    def read(self, name):
        slash = name.rfind("/")
        if slash >= 0:
            d = os.path.join(self.path, name[:slash])
            fname = name[slash + 1:] + ".class"
        else:
            d = self.path
            fname = name + ".class"
        entries = self._dir_cache.get(d)
        if entries is None and d not in self._dir_cache:
            try:
                with os.scandir(d) as it:
                    entries = frozenset(e.name for e in it if e.name.endswith(".class"))
            except OSError:
                entries = None
            self._dir_cache[d] = entries
        if not entries or fname not in entries:
            return None
        with open(os.path.join(d, fname), "rb") as fh:
            return fh.read()


class ZipSource(_Source):
    def __init__(self, path: str, prefix: str = "", release: int = 25):
        self.label = os.path.basename(path)
        self.path = path
        if path.endswith(".jmod"):
            with open(path, "rb") as fh:
                data = fh.read()
            self.zf = zipfile.ZipFile(io.BytesIO(data[4:]))
            prefix = "classes/"
        else:
            self.zf = zipfile.ZipFile(path)
        self.prefix = prefix
        self.index: dict[str, str] = {}
        best: dict[str, int] = {}
        for e in self.zf.namelist():
            if not e.endswith(".class") or not e.startswith(prefix):
                continue
            rel = e[len(prefix):]
            ver = 0
            if rel.startswith("META-INF/versions/"):
                parts = rel.split("/", 3)
                try:
                    ver = int(parts[2])
                except ValueError:
                    continue
                if ver > release:
                    continue
                rel = parts[3]
            if rel.endswith("module-info.class"):
                continue
            n = rel[:-6]
            if ver >= best.get(n, -1):
                best[n] = ver
                self.index[n] = e

    def names(self):
        return iter(self.index.keys())

    def read(self, name):
        e = self.index.get(name)
        return self.zf.read(e) if e else None


def _parse_services(text: str) -> list[str]:
    out = []
    for line in text.splitlines():
        line = line.split("#", 1)[0].strip()
        if line:
            out.append(line.replace(".", "/"))
    return out


class ClassPath:
    def __init__(self, entries: list[str], release: int = 25, release_overrides: dict[str, int] | None = None,
                 boot_start: int | None = None):
        """release_overrides: {jar-basename-prefix: release} — e.g. {"lwjgl": 11} selects the
        JNI flavour of LWJGL instead of its Java 25 FFM (java.lang.foreign) flavour."""
        self.sources: list[_Source] = []
        for e in entries:
            if os.path.isdir(e):
                self.sources.append(DirSource(e))
            else:
                rel = release
                for prefix, r in (release_overrides or {}).items():
                    if os.path.basename(e).startswith(prefix):
                        rel = r
                self.sources.append(ZipSource(e, release=rel))
        self._cache: dict[str, Optional[ClassFile]] = {}
        self.origin: dict[str, str] = {}
        # entries[boot_start:] are the JDK (class library overlay + JDK modules): their classes
        # belong to the boot loader; everything before is application code
        self.boot_start = len(self.sources) if boot_start is None else boot_start
        self.boot: set[str] = set()

    def is_boot(self, name: str) -> bool:
        if name in self.boot:
            return True
        host = name.split("$$", 1)[0]
        return host != name and host in self.boot

    def find(self, name: str) -> Optional[ClassFile]:
        if name in self._cache:
            return self._cache[name]
        cf = None
        for i, s in enumerate(self.sources):
            b = s.read(name)
            if b is not None:
                cf = parse_class(b)
                self.origin[name] = s.label
                if i >= self.boot_start:
                    self.boot.add(name)
                break
        self._cache[name] = cf
        return cf

    def services(self) -> dict[str, list[str]]:
        """META-INF/services registrations of the application class path (not the JDK)."""
        res: dict[str, list[str]] = {}
        sources = self.sources[:self.boot_start] if self.boot_start else self.sources
        for src in sources:
            if getattr(src, "path", "").endswith(".jmod"):
                continue
            if isinstance(src, ZipSource):
                for e in src.zf.namelist():
                    if e.startswith("META-INF/services/") and not e.endswith("/"):
                        svc = e[len("META-INF/services/"):].replace(".", "/")
                        for p in _parse_services(src.zf.read(e).decode("utf-8", "replace")):
                            if p not in res.setdefault(svc, []):
                                res[svc].append(p)
            elif isinstance(src, DirSource):
                d = os.path.join(src.path, "META-INF", "services")
                if os.path.isdir(d):
                    for f in os.listdir(d):
                        with open(os.path.join(d, f), encoding="utf-8", errors="replace") as fh:
                            for p in _parse_services(fh.read()):
                                if p not in res.setdefault(f.replace(".", "/"), []):
                                    res[f.replace(".", "/")].append(p)
        return res

    def module_services(self) -> dict[str, list[tuple[str, bool]]]:
        """`provides S with P...` of the module-info.class of every JDK module on the path:
        service -> [(provider class, is_factory)]. A provider is a factory when it has no usable
        public constructor but a public static provider() method (resolved by the caller)."""
        import struct
        res: dict[str, list[tuple[str, bool]]] = {}
        for src in self.sources:
            root = getattr(src, "path", "")
            mi_label = root
            if isinstance(src, DirSource):
                mi = os.path.join(root, "module-info.class")
                if not os.path.isfile(mi):
                    continue
                mi_label = mi
                try:
                    with open(mi, "rb") as fh:
                        mi_bytes = fh.read()
                except Exception as e:
                    import sys as _sys
                    _sys.stderr.write(f"j2zig: warning: cannot read {mi}: {e}\n")
                    continue
            elif hasattr(src, "zf"):
                mi_entry = getattr(src, "prefix", "") + "module-info.class"
                try:
                    mi_bytes = src.zf.read(mi_entry)
                except KeyError:
                    continue
                except Exception as e:
                    import sys as _sys
                    _sys.stderr.write(f"j2zig: warning: cannot read {mi_entry} in {root}: {e}\n")
                    continue
            else:
                continue
            try:
                cf = parse_class(mi_bytes)
            except Exception as e:  # loud: a corrupt module-info silently drops JDK service providers otherwise
                import sys as _sys
                _sys.stderr.write(f"j2zig: warning: cannot parse {mi_label}: {e}\n")
                continue
            raw = cf.attributes.get("Module")
            if raw is None:
                continue
            p = 6
            for _ in range(struct.unpack_from(">H", raw, p)[0]):  # requires
                p += 6
            p += 2
            for _ in range(2):  # exports, opens
                n = struct.unpack_from(">H", raw, p)[0]
                p += 2
                for _ in range(n):
                    to = struct.unpack_from(">H", raw, p + 4)[0]
                    p += 6 + 2 * to
            # uses
            p = _skip_uses(raw, p)
            n = struct.unpack_from(">H", raw, p)[0]
            p += 2
            for _ in range(n):
                svc_i, with_n = struct.unpack_from(">HH", raw, p)
                p += 4
                svc = cf.cp[svc_i].name
                for _ in range(with_n):
                    (pi,) = struct.unpack_from(">H", raw, p)
                    p += 2
                    res.setdefault(svc, []).append((cf.cp[pi].name, False))
        return res

    def all_names(self, source_filter=None) -> list[str]:
        seen = set()
        out = []
        for s in self.sources:
            if source_filter and not source_filter(s):
                continue
            for n in s.names():
                if n not in seen:
                    seen.add(n)
                    out.append(n)
        return out


def jdk_home() -> str:
    """JDK used as class library source: $J2Z_JDK_HOME, else $JAVA_HOME, else common locations.
    Minecraft 26.x requires Java 25."""
    import glob as _glob
    cands = [os.environ.get("J2Z_JDK_HOME"), os.environ.get("JAVA_HOME")]
    # mcbuild installs Temurin under ~/.cache/j2zig/temurin-<major>/jdk-*/ — look there first
    cache = os.path.join(os.path.expanduser("~"), ".cache", "j2zig")
    homes = [os.path.dirname(os.path.dirname(p)) for p in
             _glob.glob(os.path.join(cache, "temurin-25", "**", "bin", "javac"), recursive=True)]
    cands += sorted(homes, reverse=True)
    cands += sorted(_glob.glob("/opt/jdk25/jdk-25*"), reverse=True)
    cands += ["/usr/lib/jvm/java-25-openjdk-amd64", "/usr/lib/jvm/java-21-openjdk-amd64", "/usr/lib/jvm/default-java"]
    for c in cands:
        if c and os.path.isfile(os.path.join(c, "bin", "javac")):
            return c
    raise SystemExit("j2zig: no JDK found (set J2Z_JDK_HOME)")


def jdk_tool(name: str) -> str:
    return os.path.join(jdk_home(), "bin", name)


# JDK modules made available to applications (java.base first). Desktop/AWT is not needed by
# the supported targets; add modules here if an application requires them.
JDK_MODULES = ["java.base", "java.logging", "java.management", "java.net.http", "java.sql",
               "java.naming", "java.desktop", "java.scripting", "java.xml", "java.prefs",
               "java.compiler", "java.instrument", "jdk.unsupported", "jdk.jfr", "jdk.management",
               "java.datatransfer", "java.transaction.xa", "jdk.crypto.ec", "jdk.zipfs", "jdk.httpserver"]


def _skip_uses(raw: bytes, p: int) -> int:
    import struct
    n = struct.unpack_from(">H", raw, p)[0]
    return p + 2 + 2 * n


def jdk_jmods() -> list[str]:
    """Class roots of the selected JDK's modules: jmods when present, otherwise the runtime
    image (lib/modules) extracted once with `jimage` into a cache (JDK 24+ ship no jmods)."""
    home = jdk_home()
    if os.path.isfile(os.path.join(home, "jmods", "java.base.jmod")):
        return [os.path.join(home, "jmods", m + ".jmod") for m in JDK_MODULES
                if os.path.isfile(os.path.join(home, "jmods", m + ".jmod"))]
    import hashlib
    import subprocess
    key = hashlib.sha1(os.path.realpath(home).encode()).hexdigest()[:12]
    cache = os.path.join(os.path.expanduser("~"), ".cache", "j2zig", "jdk-" + key)
    if not os.path.isfile(os.path.join(cache, ".complete")):
        os.makedirs(cache, exist_ok=True)
        env = {k: v for k, v in os.environ.items() if k != "JAVA_TOOL_OPTIONS"}
        subprocess.run([os.path.join(home, "bin", "jimage"), "extract", "--dir", cache,
                        os.path.join(home, "lib", "modules")], check=True, env=env, capture_output=True)
        open(os.path.join(cache, ".complete"), "w").close()
    return [os.path.join(cache, m) for m in JDK_MODULES if os.path.isdir(os.path.join(cache, m))]


# ---------------------------------------------------------------------- builtin bridge
_ROOT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RUNTIME_JAVA = os.path.join(_ROOT_DIR, "runtime", "java")
RUNTIME_JAVA_NOLWJGL = os.path.join(_ROOT_DIR, "runtime", "java_nolwjgl")
BUILTIN_CACHE = os.path.join(os.path.expanduser("~"), ".cache", "j2zig", "builtin")
BUILTIN_NOLWJGL_CACHE = os.path.join(os.path.expanduser("~"), ".cache", "j2zig", "builtin_nolwjgl")


def _compile_java_tree(src_dir: str, cache_dir: str, extra_cp: Optional[str] = None, force: bool = False) -> Optional[str]:
    import glob
    import hashlib
    import shutil
    import subprocess
    srcs = sorted(glob.glob(os.path.join(src_dir, "**", "*.java"), recursive=True))
    if not srcs:
        return None
    h = hashlib.sha1()
    if extra_cp:
        h.update(extra_cp.encode())
        h.update(b"\0")
    for x in srcs:
        h.update(x.encode())
        h.update(b"\0")
        with open(x, "rb") as fh:
            h.update(fh.read())
    out = os.path.join(cache_dir, h.hexdigest()[:12])
    stamp = os.path.join(out, ".stamp")
    if not force and os.path.isfile(stamp):
        return out
    os.makedirs(cache_dir, exist_ok=True)
    lock_path = out + ".lock"
    with open(lock_path, "w") as lock:
        try:
            import fcntl
            fcntl.flock(lock, fcntl.LOCK_EX)
        except ImportError:
            try:
                import msvcrt
                msvcrt.locking(lock.fileno(), msvcrt.LK_LOCK, 1)
            except (ImportError, OSError):
                pass
        if not force and os.path.isfile(stamp):
            return out
        tmp = out + f".tmp{os.getpid()}"
        if os.path.isdir(tmp):
            shutil.rmtree(tmp)
        os.makedirs(tmp)
        cp_args = ["-cp", extra_cp] if extra_cp else []
        cmd = [jdk_tool("javac"), "-J-Xmx192m", "-J-XX:+UseSerialGC",
               "-nowarn", "--add-exports", "java.base/jdk.internal.misc=ALL-UNNAMED"] + cp_args + ["-d", tmp] + srcs
        r = subprocess.run(cmd, capture_output=True, text=True)
        if r.returncode != 0:
            shutil.rmtree(tmp, ignore_errors=True)
            raise SystemExit("j2zig: built-in runtime classes failed to compile:\n" + r.stdout + r.stderr)
        with open(os.path.join(tmp, ".stamp"), "w") as fh:
            fh.write(jdk_home())
        if os.path.isdir(out):
            shutil.rmtree(out)
        os.rename(tmp, out)
    return out


def builtin_classes(force: bool = False) -> Optional[str]:
    """Compile the built-in Java bridge (runtime/java/**/*.java, j2z.rhi.NativeRhi)
    into a content-keyed cache, or return None when absent / J2Z_NO_BUILTIN=1."""
    if os.environ.get("J2Z_NO_BUILTIN") == "1":
        return None
    return _compile_java_tree(RUNTIME_JAVA, BUILTIN_CACHE, force=force)


def builtin_nolwjgl_classes(force: bool = False) -> Optional[str]:
    """Compile opt-in Zero-LWJGL shim classes (runtime/java_nolwjgl/**/*.java) when `--no-lwjgl` is active."""
    if os.environ.get("J2Z_NO_BUILTIN") == "1":
        return None
    bridge = builtin_classes(force=force)
    return _compile_java_tree(RUNTIME_JAVA_NOLWJGL, BUILTIN_NOLWJGL_CACHE, extra_cp=bridge, force=force)
