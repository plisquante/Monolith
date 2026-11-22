#!/bin/sh
# Build a native Minecraft executable from a client.jar.
#   ./build.sh /path/to/client.jar [--graphics vulkan|opengl] [-O ReleaseFast] [--out DIR]
# Requirements: python3 (>= 3.10), zig 0.15.1 (pip install ziglang==0.15.1), zlib.
# A matching JDK (class library source) is downloaded automatically if J2Z_JDK_HOME is unset.
set -e
HERE="$(cd "$(dirname "$0")" && pwd)"
if [ -z "$1" ]; then
  echo "usage: $0 /path/to/client.jar [options]   (see: python3 tools/mcbuild.py --help)"
  exit 2
fi
command -v zig >/dev/null 2>&1 || { echo "zig not found: pip install ziglang==0.15.1 (then add it to PATH)"; exit 1; }
exec python3 "$HERE/tools/mcbuild.py" "$@"
