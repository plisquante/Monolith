#!/bin/sh
# Linux için başlatıcı. Monolith başlatıcı arayüzünü açar (uv Python'u ve ortamı kendisi yönetir; venv gerekmez).
HERE="$(cd "$(dirname "$0")" && pwd)"
if command -v uv >/dev/null 2>&1; then
  exec uv run --project "$HERE" "$HERE/launcher/launcher.py" "$@"
fi
# uv yoksa eski yol: sistemdeki python3 (>= 3.10)
echo "uyari: uv bulunamadi; sistemdeki python3 kullaniliyor. uv kurmak icin: curl -LsSf https://astral.sh/uv/install.sh | sh" >&2
command -v python3 >/dev/null 2>&1 || { echo "python3 (>= 3.10) not found: install uv (command above)"; exit 1; }
exec python3 "$HERE/launcher/launcher.py" "$@"
