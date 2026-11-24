#!/bin/sh
# Launcher for Linux. Opens the Monolith launcher UI (needs python3 >= 3.10).
HERE="$(cd "$(dirname "$0")" && pwd)"
command -v python3 >/dev/null 2>&1 || { echo "python3 (>= 3.10) not found"; exit 1; }
exec python3 "$HERE/launcher/launcher.py" "$@"
