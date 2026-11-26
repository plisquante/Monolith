#!/bin/sh
# Rust arka ucuyla bir client.jar'dan yerel bir Minecraft çalıştırılabilir dosyası derler.
#   ./build.sh /yol/client.jar                  # masaüstü (Linux x86_64): DIR/run.sh
#   ./build.sh /yol/client.jar --android        # Android arm64: DIR/rust/libminecraft_mono.so
#   seçenekler: -o DIR, --opt {0,1,2,3,s,z}, --shards N, --shard-jobs N, --resume (bkz. ./build.sh --help)
# Gereksinimler: uv (Python'u ve ortamı kendisi yönetir; venv gerekmez), rustc (stable),
#   3 paralel shard için yaklaşık 12 GiB RAM.
set -e
HERE="$(cd "$(dirname "$0")" && pwd)"
if [ -z "$1" ]; then
  echo "usage: $0 /path/to/client.jar [options]   (see: $0 --help)"
  exit 2
fi
command -v rustc >/dev/null 2>&1 || { echo "rustc not found: install a stable Rust toolchain (rustup)"; exit 1; }

# uv varsa onunla çalıştır: uygun Python'u kendisi bulur (yoksa indirir), ortamı kendisi kurar.
# --project: pyproject.toml'u depodan okur; göreli yollar (client.jar, -o DIR) çağırdığınız klasöre göre kalır.
if command -v uv >/dev/null 2>&1; then
  exec uv run --project "$HERE" "$HERE/tools/mcbuild_rust.py" "$@"
fi

# uv yoksa eski yol: sistemdeki python3 (>= 3.10). Yine de uv'yi öneririz.
echo "uyari: uv bulunamadi; sistemdeki python3 kullaniliyor. uv kurmak icin: curl -LsSf https://astral.sh/uv/install.sh | sh" >&2
command -v python3 >/dev/null 2>&1 || { echo "python3 (>= 3.10) de bulunamadi: uv kurun (yukaridaki komut)"; exit 1; }
exec python3 "$HERE/tools/mcbuild_rust.py" "$@"
