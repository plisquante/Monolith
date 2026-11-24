#!/usr/bin/env python3
"""Pieces of the JDK that a native build needs but that the closed-world analysis cannot see.

1. Provider classes. JCA and JSSE (the security providers, TLS, X.509) create their implementations by name at run
   time (Provider.Service, Class.forName). A class that is only named in a provider table is not reachable for the
   analysis, so it would be missing from the binary. All classes of the provider packages are therefore kept.
2. Data files. ICU (Unicode normalisation, used by X.500 names in certificates) reads its data from class-path
   resources of java.base. The runtime serves resources from the directory in MONO_RESOURCES, so the files are
   copied there.

    python3 tools/jdk_runtime_data.py <jdk-home> --reflect OUT.txt --resources DIR

The JDK home must contain lib/modules (jimage) and bin/jimage.
"""
import argparse, os, re, subprocess, sys

# sınıfları çalışma zamanında adıyla örneklenen paketler (sağlayıcılar ve yardımcıları)
PROVIDER_PACKAGES = (
    "sun.security.ssl.", "sun.security.provider.", "sun.security.ec.", "sun.security.rsa.",
    "sun.security.x509.", "sun.security.pkcs12.", "sun.security.pkcs.", "sun.security.util.",
    "sun.security.jca.", "sun.security.timestamp.", "com.sun.crypto.provider.",
)
# çalışma zamanının adıyla okuduğu java.base kaynak önekleri
RESOURCE_PREFIXES = ("jdk/internal/icu/impl/data/",)


def jimage_entries(jdk: str):
    """(module, path) of every entry of the JDK image."""
    out = subprocess.run([os.path.join(jdk, "bin", "jimage"), "list", os.path.join(jdk, "lib", "modules")],
                         capture_output=True, text=True, check=True).stdout
    module = None
    for line in out.splitlines():
        if line.startswith("Module: "):
            module = line.split(":", 1)[1].strip()
        elif module and line.startswith("    "):
            yield module, line.strip()


def provider_classes(jdk: str) -> list:
    names = []
    for module, path in jimage_entries(jdk):
        if module != "java.base" or not path.endswith(".class"):
            continue
        dotted = path[:-len(".class")].replace("/", ".")
        if dotted.startswith(PROVIDER_PACKAGES):
            names.append(dotted)
    return sorted(set(names))


def resource_files(jdk: str) -> list:
    out = []
    for module, path in jimage_entries(jdk):
        if module == "java.base" and path.startswith(RESOURCE_PREFIXES):
            out.append(path)
    return sorted(out)


def extract(jdk: str, paths: list, dest: str):
    """Writes the given java.base entries under dest, keeping their paths."""
    import tempfile
    with tempfile.TemporaryDirectory() as tmp:
        # hepsi için tek bir kalıp: java.base'in çalışma zamanı veri önekleri
        pattern = "regex:.*(" + "|".join(re.escape(x) for x in RESOURCE_PREFIXES) + ").*"
        cmd = [os.path.join(jdk, "bin", "jimage"), "extract", "--dir", tmp, "--include", pattern,
               os.path.join(jdk, "lib", "modules")]
        subprocess.run(cmd, check=True, capture_output=True)
        base = os.path.join(tmp, "java.base")
        for p in paths:
            src = os.path.join(base, p)
            if not os.path.isfile(src):
                raise SystemExit(f"jdk_runtime_data: {p} not extracted")
            dst = os.path.join(dest, p)
            os.makedirs(os.path.dirname(dst), exist_ok=True)
            with open(src, "rb") as fi, open(dst, "wb") as fo:
                fo.write(fi.read())


def main_copy(jdk: str, resources: str) -> None:
    """Copies the runtime data files into a resources directory (used by the build)."""
    files = resource_files(jdk)
    extract(jdk, files, resources)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("jdk")
    ap.add_argument("--reflect", help="write the provider class names here (one per line)")
    ap.add_argument("--resources", help="copy the runtime data files into this directory")
    a = ap.parse_args()
    if a.reflect:
        names = provider_classes(a.jdk)
        with open(a.reflect, "w", encoding="utf-8") as fh:
            fh.write("\n".join(names) + "\n")
        print(f"provider classes: {len(names)} -> {a.reflect}")
    if a.resources:
        files = resource_files(a.jdk)
        extract(a.jdk, files, a.resources)
        print(f"runtime data files: {len(files)} -> {a.resources}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
