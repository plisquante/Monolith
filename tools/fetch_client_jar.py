#!/usr/bin/env python3
"""Minecraft istemci jar'ını Mojang'ın resmi sürüm manifest'inden indirir ve SHA-1 ile doğrular.

    python3 tools/fetch_client_jar.py --version 26.3 --out /tmp/client.jar

Akış: version_manifest_v2.json -> sürümün JSON'u -> downloads.client (url, sha1, size). İndirilen dosyanın boyutu
ve SHA-1'i tutmazsa betik 1 ile çıkar ve çıktıyı yazmaz.

Ortam değişkenleri:
    MC_CLIENT_JAR_URL    resmi adres yerine bu adres kullanılır (ayna, yalnız https). MC_CLIENT_JAR_SHA1 (40 onaltılı hane) ZORUNLUDUR:
                         çalıştırılacak bir jar, sağlama toplamı olmayan bir aynadan kabul edilmez.

Jar depoya ya da CI artefaktlarına konmaz; yalnızca derleme makinesinin geçici dizinine yazılır.
"""
import argparse
import hashlib
import json
import os
import sys
import tempfile
import urllib.request

MANIFEST = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"


def get_json(url: str) -> dict:
    with urllib.request.urlopen(url, timeout=60) as r:
        return json.loads(r.read().decode("utf-8"))


def sha1_of(path: str) -> str:
    h = hashlib.sha1()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def fail(msg: str) -> int:
    print(f"FAIL: {msg}", file=sys.stderr)
    return 1


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--version", default="26.3")
    ap.add_argument("--out", required=True)
    a = ap.parse_args()

    mirror = os.environ.get("MC_CLIENT_JAR_URL", "").strip()
    if mirror:
        url = mirror
        want_sha1 = os.environ.get("MC_CLIENT_JAR_SHA1", "").strip().lower() or None
        want_size = None
        source = "mirror (MC_CLIENT_JAR_URL)"
        # the jar is executed code: a mirror is only as trustworthy as the checksum it is checked against
        if want_sha1 is None or len(want_sha1) != 40 or any(c not in "0123456789abcdef" for c in want_sha1):
            return fail("MC_CLIENT_JAR_URL needs MC_CLIENT_JAR_SHA1 (40 hex digits): a mirror is not accepted without a checksum")
    else:
        try:
            manifest = get_json(MANIFEST)
        except Exception as e:  # network or JSON error: stop, there is no fallback
            return fail(f"cannot read the Mojang version manifest: {e}")
        entry = next((v for v in manifest["versions"] if v["id"] == a.version), None)
        if entry is None:
            return fail(f"version {a.version} is not in the Mojang manifest")
        meta = get_json(entry["url"])
        client = meta.get("downloads", {}).get("client")
        if not client:
            return fail(f"version {a.version} has no client download")
        url, want_sha1, want_size = client["url"], client["sha1"], client.get("size")
        source = "Mojang manifest"

    if not url.lower().startswith("https://"):  # urllib would also open file:// and ftp://
        return fail(f"refusing a non-https download URL: {url}")
    print(f"downloading {a.version} client from the {source}")
    # the temporary file sits next to the target: os.replace cannot move files between drives (Windows C: to D:)
    out_dir = os.path.dirname(os.path.abspath(a.out))
    os.makedirs(out_dir, exist_ok=True)
    tmp_fd, tmp = tempfile.mkstemp(prefix="client_", suffix=".jar", dir=out_dir)
    os.close(tmp_fd)
    try:
        with urllib.request.urlopen(url, timeout=600) as r, open(tmp, "wb") as f:
            while True:
                chunk = r.read(1 << 20)
                if not chunk:
                    break
                f.write(chunk)
        size = os.path.getsize(tmp)
        if want_size is not None and size != want_size:
            return fail(f"size {size} does not match the manifest ({want_size})")
        got = sha1_of(tmp)
        if want_sha1 is not None and got != want_sha1:
            return fail(f"SHA-1 {got} does not match {want_sha1}")
        if size == 0:
            return fail("the download is empty")
        os.makedirs(os.path.dirname(os.path.abspath(a.out)), exist_ok=True)
        os.replace(tmp, a.out)
        tmp = None
    finally:
        if tmp and os.path.exists(tmp):
            os.remove(tmp)
    print(f"OK: {a.out} ({size} bytes, sha1 {got})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
