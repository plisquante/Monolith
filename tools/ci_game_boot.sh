#!/bin/sh
# Oyunu sanal ekranda (Xvfb) açar, belirli süre çalıştırır ve açılışın başarılı olduğunu denetler:
#   - süreç hâlâ yaşıyor (çökmedi, kendiliğinden çıkmadı)
#   - çökme raporu yazılmadı
#   - günlükte "Sound engine ERROR" ve "FFI types not initialized" yok
#   - ekran görüntüsü boş değil (başlık ekranı çizildi)
# Hepsi geçerse PASS yazar ve 0 ile çıkar; herhangi biri başarısızsa FAIL yazar ve 1 ile çıkar.
#
#   tools/ci_game_boot.sh <build-dir> [--seconds 120] [--out DIR] [--display :99]
#
# Gerekenler: Xvfb, ImageMagick (import, convert). .github/workflows/game.yml bu betiği çalıştırır.
set -u
[ $# -ge 1 ] || { echo "usage: $0 <build-dir> [--seconds N] [--out DIR] [--display :N]"; exit 2; }
BUILD="$1"; shift
SECONDS_TOTAL=120; OUT=""; DISP=":99"
while [ $# -gt 0 ]; do
  case "$1" in
    --seconds) SECONDS_TOTAL="$2"; shift 2;;
    --out) OUT="$2"; shift 2;;
    --display) DISP="$2"; shift 2;;
    *) echo "unknown option $1"; exit 2;;
  esac
done
[ -n "$OUT" ] || OUT="$BUILD/boot"
# Linux only: sanal ekran (Xvfb) ve ImageMagick
OS=$(uname -s)
[ "$OS" = "Linux" ] || { echo "FAIL: this boot script runs on Linux only (got $OS)"; exit 1; }
for t in Xvfb import convert; do command -v "$t" >/dev/null || { echo "FAIL: $t not found"; exit 1; }; done
EXE="$BUILD/rust/prog"; [ -x "$EXE" ] || EXE="$BUILD/prog"
[ -x "$EXE" ] || { echo "FAIL: no executable in $BUILD"; exit 1; }

mkdir -p "$OUT" "$BUILD/game"
rm -rf "$BUILD/game/crash-reports"
export DISPLAY="$DISP"
Xvfb "$DISP" -screen 0 1280x720x24 +extension GLX >"$OUT/xvfb.log" 2>&1 &
XVFB=$!
GAME=""
cleanup() {
  [ -n "$GAME" ] && kill "$GAME" 2>/dev/null
  [ -n "$XVFB" ] && kill "$XVFB" 2>/dev/null
}
trap cleanup EXIT INT TERM
sleep 2

MONO_RESOURCES="$BUILD/resources" "$EXE" -Dmono.graphics.backend=opengl --version "${MC_VERSION:-26.3}" \
  --gameDir "$BUILD/game" --assetsDir "$BUILD/assets" --assetIndex "${MC_ASSET_INDEX:-34}" \
  --username "${MC_USERNAME:-Player}" --accessToken 0 --userType legacy >"$OUT/game.log" 2>&1 &
GAME=$!

sleep "$SECONDS_TOTAL"
import -window root "$OUT/title.png" 2>>"$OUT/game.log" || true

FAILED=0
report() { echo "FAIL: $1"; FAILED=1; }

kill -0 "$GAME" 2>/dev/null || report "the game exited before the end of the $SECONDS_TOTAL s run (see $OUT/game.log)"
if [ -d "$BUILD/game/crash-reports" ] && [ -n "$(ls -A "$BUILD/game/crash-reports" 2>/dev/null)" ]; then
  report "crash report written: $(ls "$BUILD/game/crash-reports" | head -1)"
fi
grep -q "Sound engine ERROR" "$OUT/game.log" && report "the sound engine reported an error"
grep -q "FFI types not initialized" "$OUT/game.log" && report "FFI types were not initialized (JNA emulation)"
if [ -s "$OUT/title.png" ]; then
  SD=$(convert "$OUT/title.png" -format '%[fx:standard_deviation]' info: 2>/dev/null)
  echo "screenshot variation: ${SD:-unknown}"
  case "$SD" in
    ""|unknown) report "the screenshot could not be measured";;
    *) awk -v v="$SD" 'BEGIN { exit !(v+0 < 0.01) }' && report "the screenshot is blank (nothing was drawn)";;
  esac
else
  report "no screenshot was taken"
fi

if [ "$FAILED" -eq 0 ]; then
  echo "PASS: the game ran $SECONDS_TOTAL s on a virtual display; screenshot: $OUT/title.png"
  exit 0
fi
echo "--- last lines of the game log:"
tail -n 40 "$OUT/game.log"
exit 1
