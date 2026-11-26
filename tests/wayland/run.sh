#!/bin/sh
# Wayland masaüstü yüzeyi sınaması (monolith/rt/desk/wayland.rs), Linux.
#   tests/wayland/run.sh
# Gerekenler (PATH'te): sway, wtype, rustc; ayrıca libEGL ve Mesa (yazılım çizimi yeter: LIBGL_ALWAYS_SOFTWARE=1).
#   Debian/Ubuntu: apt-get install sway wtype libegl1 libegl-mesa0
# Ne yapar: başsız (ekransız) bir sway açar; sınama programını (wl_probe: yüzeyi aynen kullanır), kalıcı bir sanal fareyi
# (vptr) ve wtype ile klavyeyi sürer; pencere, OpenGL, klavye (Türkçe harfler), fare, fare kilidi ve kapatma isteğini
# denetler. Çıkış kodu 0: hepsi geçti.
set -e
HERE="$(cd "$(dirname "$0")" && pwd)"
for t in sway wtype rustc; do
  command -v "$t" >/dev/null 2>&1 || { echo "gerekli arac bulunamadi: $t"; exit 2; }
done
# sway'in IPC soketi uzun yola sığmaz: kısa bir klasör kullanılır
WORK="$(mktemp -d /tmp/wlt.XXXXXX)"
chmod 700 "$WORK"
SWAY_PID=""
cleanup() {
  [ -n "$SWAY_PID" ] && kill "$SWAY_PID" 2>/dev/null || true
  pkill -P $$ 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT

echo "[1/4] sinama programlari derleniyor"
rustc --edition 2021 -O -o "$WORK/wl_probe" "$HERE/wl_probe.rs" 2>/dev/null
rustc --edition 2021 -O -o "$WORK/vptr" "$HERE/vptr.rs" 2>/dev/null

echo "[2/4] basiz sway aciliyor"
cat > "$WORK/sway.conf" <<'EOF'
output HEADLESS-1 resolution 1280x720
default_border none
EOF
export XDG_RUNTIME_DIR="$WORK"
unset WAYLAND_DISPLAY DISPLAY
WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1 sway -c "$WORK/sway.conf" >"$WORK/sway.log" 2>&1 &
SWAY_PID=$!
i=0
until ls "$WORK"/wayland-[0-9]* >/dev/null 2>&1 && ls "$WORK"/sway-ipc.*.sock >/dev/null 2>&1; do
  i=$((i + 1)); [ "$i" -gt 50 ] && { echo "sway acilamadi:"; tail -5 "$WORK/sway.log"; exit 1; }
  sleep 0.2
done
WAYLAND_DISPLAY="$(basename "$(ls "$WORK"/wayland-[0-9]* | grep -v '\.lock$' | head -1)")"
SWAYSOCK="$(ls "$WORK"/sway-ipc.*.sock | head -1)"
export WAYLAND_DISPLAY SWAYSOCK MONO_DISPLAY_BACKEND=wayland LIBGL_ALWAYS_SOFTWARE=1 LANG=C.UTF-8 LC_ALL=C.UTF-8

echo "[3/4] pencere, OpenGL, klavye, fare ve fare kilidi (yaklasik 10 sn)"
# kalıcı sanal fare: konum, sol ve sağ tıklama, sonra yakalama sırasında göreli hareket
"$WORK/vptr" sleep:1500 abs:200,100 sleep:300 click:left sleep:300 click:right sleep:500 abs:640,360 \
  sleep:2300 move:30,10 sleep:300 move:-12,5 sleep:2600 move:50,50 sleep:600 quit >"$WORK/vptr.log" 2>&1 &
# klavye: Türkçe harfler ve W tuşu (wtype'ın aygıtı yalnızca çalıştığı sürece var olur, bu yüzden başlangıç beklemesi var)
wtype -s 2500 "şığ" -s 400 -P w -s 400 -p w -s 3000 >/dev/null 2>&1 &
sleep 1
# 5. saniyede fare yakalama açılır, 8. saniyede kapanır
"$WORK/wl_probe" 9 5 8 >"$WORK/probe.txt" 2>&1 || true

echo "[4/4] pencere kapatma istegi"
# set -e açık olduğundan çıkış kodu `|| rc=$?` ile yakalanır (yoksa alt kabuk sıfırdan farklı kodda echo'ya gelmeden biter)
( rc=0; "$WORK/wl_probe" 20 >"$WORK/close.txt" 2>&1 || rc=$?; echo "cikis=$rc" >>"$WORK/close.txt" ) &
sleep 3
swaymsg '[app_id="monolith"] kill' >/dev/null 2>&1 || true
sleep 2

FAIL=0
check() { # check "açıklama" "aranan metin" dosya
  if grep -qF -- "$2" "$3"; then echo "  GECTI  $1"; else echo "  KALDI  $1   (aranan: $2)"; FAIL=$((FAIL + 1)); fi
}
echo "--- sonuclar"
check "yuzey aciliyor"                    "try_init=true"                        "$WORK/probe.txt"
check "OpenGL baglami, kirmizi boyama"    "PIXEL(1,1)=[255, 0, 0, 255]"          "$WORK/probe.txt"
check "pencere yoneticisi boyutu"         "SIZE 1280x720"                        "$WORK/probe.txt"
check "fare konumu (200,100)"             "CURSOR 0.156,0.139"                   "$WORK/probe.txt"
check "sol tus"                           "KEYS [0, 0, 0, 0, 0, 0, 0, 1"         "$WORK/probe.txt"
check "sag tus"                           "KEYS [0, 0, 0, 0, 0, 0, 0, 0, 1"      "$WORK/probe.txt"
check "Turkce harf s"                     "TEXT char 'ş'"                        "$WORK/probe.txt"
check "Turkce harf i"                     "TEXT char 'ı'"                        "$WORK/probe.txt"
check "Turkce harf g"                     "TEXT char 'ğ'"                        "$WORK/probe.txt"
check "oyun tusu W"                       "KEYS [1, 0, 0,"                       "$WORK/probe.txt"
check "yakalamada goreli hareket"         "LOOK dx=50.00 dy=50.00"               "$WORK/probe.txt"
check "kapatma istegi SIGTERM"            "SIGTERM ile kapatiliyor"              "$WORK/close.txt"
check "kapatma cikis kodu 143"            "cikis=143"                            "$WORK/close.txt"
# yakalamada imleç konumu değişmemeli: 5. ile 8. saniye arasında CURSOR satırı olmamalı
if awk '/set_grab\(true\)/{g=1} /set_grab\(false\)/{g=0} g && /CURSOR/{bad=1} END{exit bad}' "$WORK/probe.txt"; then
  echo "  GECTI  yakalamada imlec konumu sabit"
else
  echo "  KALDI  yakalamada imlec konumu degisti"; FAIL=$((FAIL + 1))
fi
if [ "$FAIL" -eq 0 ]; then echo "HEPSI GECTI"; else echo "$FAIL KONTROL KALDI (ayrinti: $WORK/probe.txt)"; trap - EXIT; kill "$SWAY_PID" 2>/dev/null || true; exit 1; fi
