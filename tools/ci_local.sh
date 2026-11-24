#!/bin/sh
# Runs the steps of .github/workflows/game.yml for the host OS, in the same order, on this machine.
# Use it to reproduce a CI game-boot failure locally, or to check a change before pushing.
#
#   tools/ci_local.sh [--out DIR] [--seconds N] [--shards N] [--shard-jobs N] [--skip-build]
#
# Every step is logged to DIR/ci_local.log and the script exits 1 at the first failing step.
# The client jar comes from fetch_client_jar.py (Mojang manifest, SHA-1 checked) and is never committed.
set -u
ROOT=$(cd "$(dirname "$0")/.." && pwd)
OUT="$ROOT/build/ci-local"; SECS=120; SHARDS=32; JOBS=2; SKIP=0
while [ $# -gt 0 ]; do
  case "$1" in
    --out) OUT="$2"; shift 2;;
    --seconds) SECS="$2"; shift 2;;
    --shards) SHARDS="$2"; shift 2;;
    --shard-jobs) JOBS="$2"; shift 2;;
    --skip-build) SKIP=1; shift;;
    *) echo "unknown option $1"; exit 2;;
  esac
done
mkdir -p "$OUT"
LOG="$OUT/ci_local.log"
: > "$LOG"
cd "$ROOT" || exit 2

step() {
  name="$1"; shift
  echo "== $name" | tee -a "$LOG"
  "$@" >>"$LOG" 2>&1
  rc=$?
  echo "   -> exit $rc" | tee -a "$LOG"
  if [ $rc -ne 0 ]; then
    echo "FAIL at: $name (full log: $LOG)"
    exit 1
  fi
}

OS=$(uname -s)
case "$OS" in
  Linux) PY=python3; BOOT="sh tools/ci_game_boot.sh";;
  *) echo "ci_local.sh runs the Linux steps; use tools/ci_game_boot.ps1 on Windows"; exit 2;;
esac

JAR="$OUT/client.jar"
step "istemci jar (resmi manifest, SHA-1 dogrulamali)" $PY tools/fetch_client_jar.py --version 26.3 --out "$JAR"
if [ $SKIP -eq 0 ]; then
  step "oyunu derle" $PY tools/mcbuild_rust.py "$JAR" -o "$OUT/minecraft-26.3" --shards "$SHARDS" --shard-jobs "$JOBS"
fi
step "oyunu ac ve dogrula ($SECS sn)" $BOOT "$OUT/minecraft-26.3" --seconds "$SECS" --out "$OUT/minecraft-26.3/boot"
if [ "$OS" = "Linux" ]; then
  step "eszamanli GC ile acilis ($SECS sn)" env MONO_GC_CONCURRENT=1 $BOOT "$OUT/minecraft-26.3" --seconds "$SECS" --out "$OUT/minecraft-26.3/boot-concurrent" --display :98
fi
echo "PASS: every game.yml step passed on this machine" | tee -a "$LOG"
exit 0
