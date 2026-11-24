#!/bin/sh
# Build a native Minecraft executable from a client.jar with the Rust backend.
#   ./build.sh /path/to/client.jar              # desktop (Linux x86_64): DIR/run.sh
#   ./build.sh /path/to/client.jar --android    # Android arm64: DIR/rust/libminecraft_mono.so
#   options: -o DIR, --opt {0,1,2,3,s,z}, --shards N, --shard-jobs N, --resume (see tools/mcbuild_rust.py --help)
# Requirements: python3 (>= 3.10), rustc (stable), about 12 GiB RAM for 3 parallel shards.
set -e
HERE="$(cd "$(dirname "$0")" && pwd)"
if [ -z "$1" ]; then
  echo "usage: $0 /path/to/client.jar [options]   (see: python3 tools/mcbuild_rust.py --help)"
  exit 2
fi
command -v rustc >/dev/null 2>&1 || { echo "rustc not found: install a stable Rust toolchain (rustup)"; exit 1; }
exec python3 "$HERE/tools/mcbuild_rust.py" "$@"
