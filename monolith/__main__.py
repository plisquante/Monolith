"""monolith command line.

  python -m monolith analyze app.jar [lib.jar ...]

The Zig backend has been removed. Native builds go through the Rust backend:

  python3 tools/mcbuild_rust.py client.jar                # desktop
  python3 tools/mcbuild_rust.py client.jar --android      # Android arm64
  python -m monolith.rustgen --help                       # transpile to Rust
"""
from __future__ import annotations

import argparse
import sys


ZIG_REMOVED = ("monolith: the Zig backend has been removed. Build with the Rust backend instead: "
               "python3 tools/mcbuild_rust.py <client.jar> [--android], or python -m monolith.rustgen --help")


def main(argv=None) -> int:
    raw = list(sys.argv[1:] if argv is None else argv)
    ap = argparse.ArgumentParser(prog="monolith")
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("build").add_argument("rest", nargs=argparse.REMAINDER)
    sub.add_parser("build-lib").add_argument("rest", nargs=argparse.REMAINDER)
    an = sub.add_parser("analyze")
    an.add_argument("rest", nargs=argparse.REMAINDER)
    a = ap.parse_args(raw)
    if a.cmd in ("build", "build-lib"):
        print(ZIG_REMOVED, file=sys.stderr)
        return 2
    from .analyze import main as amain
    return amain(a.rest)


if __name__ == "__main__":
    sys.exit(main())
