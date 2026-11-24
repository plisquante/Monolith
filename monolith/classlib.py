"""Class library build: compiles the Java overlays in classlib/src with the JDK, content-addressed by source hash.
Shared by the Rust code generator and the CLI; it has no dependency on any code generator.
"""
from __future__ import annotations

import glob
import os
import shutil
import subprocess
import sys

from .classpath import jdk_home, jdk_tool

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CLASSLIB_SRC = os.path.join(ROOT, "classlib", "src")
CLASSLIB_BUILD = os.path.join(ROOT, "classlib", "build")
RUNTIME = os.path.join(ROOT, "runtime")


def classlib_out() -> str:
    """Overlay classes are compiled per JDK (the JDK-specific JavaLangAccess is generated)."""
    import hashlib
    h = hashlib.sha1(os.path.realpath(jdk_home()).encode()).hexdigest()[:12]
    return os.path.join(CLASSLIB_BUILD, h)


def build_classlib(force: bool = False) -> str:
    out = classlib_out()
    os.makedirs(CLASSLIB_BUILD, exist_ok=True)
    with open(out + ".lock", "w") as lock:
        # Yalnızca tavsiye niteliğinde kilit: çıktı içerik adreslidir (değişmez, kaynaklara göre anahtarlı), bu yüzden
        # kilidi kaybetmek güvenlidir (en kötü durum: yinelenen javac işi). fcntl yalnızca Unix'e özgüdür;
        # Windows'ta msvcrt kullan, ikisi de yoksa devam et.
        try:
            import fcntl
            fcntl.flock(lock, fcntl.LOCK_EX)
        except ImportError:
            try:
                import msvcrt
                msvcrt.locking(lock.fileno(), msvcrt.LK_LOCK, 1)
            except (ImportError, OSError):
                pass
        return _build_classlib(out, force)


def _build_classlib(out: str, force: bool) -> str:
    import hashlib
    gen = os.path.join(out, "gen")
    # JavaLangAccess JDK sürümleri arasında farklıdır: seçilen JDK için üret
    jla = os.path.join(gen, "java", "lang", "MonoJavaLangAccess.java")
    subprocess.run([sys.executable, os.path.join(ROOT, "tools", "gen_jla.py"), jla], check=True)
    srcs = sorted(glob.glob(os.path.join(CLASSLIB_SRC, "**", "*.java"), recursive=True)) + [jla]
    # Çıktı dizinleri değişmezdir ve kaynaklara göre anahtarlıdır; bu yüzden birini okuyan bir çeviri
    # eşzamanlı bir yeniden derlemeden asla etkilenmez.
    h = hashlib.sha1()
    for x in srcs:
        h.update(x.encode())
        with open(x, "rb") as fh:
            h.update(fh.read())
    classes = os.path.join(out, "classes-" + h.hexdigest()[:12])
    stamp = os.path.join(classes, ".stamp")
    if not force and os.path.exists(stamp):
        return classes
    tmp = classes + f".tmp{os.getpid()}"
    if os.path.isdir(tmp):
        shutil.rmtree(tmp)
    os.makedirs(tmp)
    cmd = [jdk_tool("javac"), "-J-Xmx192m", "-J-XX:+UseSerialGC",
           "-nowarn", "-XDstringConcat=inline", "--patch-module",
           f"java.base={CLASSLIB_SRC}{os.pathsep}{gen}", "-d", tmp] + srcs
    r = subprocess.run(cmd, capture_output=True, text=True)
    if r.returncode != 0:
        sys.stderr.write(r.stdout + r.stderr)
        shutil.rmtree(tmp, ignore_errors=True)
        raise SystemExit("classlib compilation failed")
    # javac --patch-module sınıfları <out>/java.base/... altına koyar
    base = os.path.join(tmp, "java.base")
    if os.path.isdir(base):
        for item in os.listdir(base):
            shutil.move(os.path.join(base, item), os.path.join(tmp, item))
        os.rmdir(base)
    with open(os.path.join(tmp, ".stamp"), "w") as fh:
        fh.write(jdk_home())
    if os.path.isdir(classes):
        shutil.rmtree(classes)
    os.rename(tmp, classes)
    return classes
