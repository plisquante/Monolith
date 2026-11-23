#!/usr/bin/env python3
"""Packages a finished Monolith build into a release archive: the program, the launcher files, the resources, and a
manifest with the version, the source commit and SHA-256 sums of every file. The game's own files (saves, options,
logs, crash reports) are never included.

    python3 tools/make_release.py <build-dir> --out dist/ [--name Monolith-Linux-x86_64] [--version 0.x.y]

<build-dir> is the directory tools/mcbuild_rust.py wrote (rust/prog, run.sh, resources/, assets/). The archive is
<out>/<name>.tar.gz; <out>/<name>.sha256 holds its checksum; the manifest lists every file of the archive.
"""
import argparse, hashlib, json, os, subprocess, sys, tarfile, datetime

EXCLUDE_DIRS = {"game", "shots", "logs"}
EXCLUDE_NAMES = {"headless.log", "soak.log"}


def sha256(path: str) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def git_commit(root: str) -> str:
    try:
        return subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=root, capture_output=True,
                              text=True, check=True).stdout.strip()
    except (subprocess.CalledProcessError, FileNotFoundError):
        return "unknown"


def collect(build: str):
    """(archive path, file path) of every file that goes into the release."""
    must = ["rust/prog", "run.sh"]
    for m in must:
        if not os.path.isfile(os.path.join(build, m)):
            raise SystemExit(f"make_release: {m} missing in {build} (build the game first)")
    out = []
    for dp, dns, fns in os.walk(build):
        dns[:] = [d for d in dns if d not in EXCLUDE_DIRS]
        for f in fns:
            if f in EXCLUDE_NAMES:
                continue
            full = os.path.join(dp, f)
            rel = os.path.relpath(full, build)
            if rel.startswith(("game" + os.sep, "shots" + os.sep)):
                continue
            out.append((rel, full))
    return sorted(out)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("build")
    ap.add_argument("--out", default="dist")
    ap.add_argument("--name", default="Monolith-Linux-x86_64")
    ap.add_argument("--version", default="0.0.0-dev")
    ap.add_argument("--dry-run", action="store_true", help="list the files and the size, write nothing")
    a = ap.parse_args()

    files = collect(os.path.abspath(a.build))
    total = sum(os.path.getsize(f) for _, f in files)
    print(f"{len(files)} files, {total / 1048576:.1f} MiB")
    if a.dry_run:
        for rel, _ in files[:20]:
            print("  " + rel)
        return 0

    os.makedirs(a.out, exist_ok=True)
    root = os.path.dirname(os.path.abspath(__file__))
    manifest = {
        "name": a.name,
        "version": a.version,
        "commit": git_commit(os.path.dirname(root)),
        "built": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "files": {},
    }
    for rel, full in files:
        manifest["files"][rel] = {"sha256": sha256(full), "size": os.path.getsize(full)}
    mpath = os.path.join(a.out, a.name + ".manifest.json")
    with open(mpath, "w", encoding="utf-8") as fh:
        json.dump(manifest, fh, indent=2, sort_keys=True)
        fh.write("\n")

    archive = os.path.join(a.out, a.name + ".tar.gz")
    with tarfile.open(archive, "w:gz") as tf:
        top = a.name
        for rel, full in files:
            tf.add(full, arcname=os.path.join(top, rel))
        tf.add(mpath, arcname=os.path.join(top, "MANIFEST.json"))
    digest = sha256(archive)
    with open(os.path.join(a.out, a.name + ".sha256"), "w", encoding="utf-8") as fh:
        fh.write(f"{digest}  {os.path.basename(archive)}\n")
    print(f"archive: {archive}  sha256 {digest}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
