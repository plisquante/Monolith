#!/bin/sh
# Yeni bir çıktı dizini ile tek bir monolith.rustgen derlemesinin duvar saati süresi (bkz. docs/BUILD_PERF.md).
#   tools/bench_build.sh <sınıflar dizini> <ana sınıf> <shard> <iş>
# MONO_JDK_HOME (JDK 25) gerekir. Tek satır yazar: shard, iş, çıkış kodu, saniye.
set -e
if [ "$#" -ne 4 ]; then
  echo "usage: $0 <classes dir> <main class> <shards> <jobs>" >&2
  exit 2
fi
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$(mktemp -d "${TMPDIR:-/tmp}/bench_build.XXXXXX")"
start=$(date +%s.%N)
rc=0
(cd "$ROOT" && python3 -m monolith.rustgen --cp "$1" --main "$2" -o "$OUT/out" \
    --shards "$3" --shard-jobs "$4" --opt 1 --compile >"$OUT/log" 2>&1) || rc=$?
end=$(date +%s.%N)
echo "shards=$3 jobs=$4 rc=$rc wall=$(echo "$end - $start" | bc) s (log: $OUT/log)"
