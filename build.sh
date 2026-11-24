#!/bin/sh
# Rust arka ucuyla bir client.jar'dan yerel bir Minecraft çalıştırılabilir dosyası derler.
#   ./build.sh /yol/client.jar                  # masaüstü (Linux x86_64): DIR/run.sh
#   ./build.sh /yol/client.jar --android        # Android arm64: DIR/rust/libminecraft_mono.so
#   seçenekler: -o DIR, --opt {0,1,2,3,s,z}, --shards N, --shard-jobs N, --resume (bkz. tools/mcbuild_rust.py --help)
# Gereksinimler: python3 (>= 3.10), rustc (stable), 3 paralel shard için yaklaşık 12 GiB RAM.
set -e
HERE="$(cd "$(dirname "$0")" && pwd)"
if [ -z "$1" ]; then
  echo "usage: $0 /path/to/client.jar [options]   (see: python3 tools/mcbuild_rust.py --help)"
  exit 2
fi
command -v rustc >/dev/null 2>&1 || { echo "rustc not found: install a stable Rust toolchain (rustup)"; exit 1; }
exec python3 "$HERE/tools/mcbuild_rust.py" "$@"
