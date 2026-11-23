#!/bin/sh
# Steady-state leak measurement: opens the saved world, then does NOTHING for N minutes (no walking, so no new
# chunks are loaded) and writes an allocation profile. The live managed heap after warm-up must be flat; tools/leak_budget.py
# judges the slope.
#
#   tools/leak_soak.sh <build-dir> [--minutes 10] [--profile DIR/prof.json] [--display :8]
#
# <build-dir> is what tools/mcbuild_rust.py wrote (rust/prog, resources/, assets/). The world "New World" must exist.
set -e
BUILD="$1"; shift || { echo "usage: $0 <build-dir> [--minutes N] [--profile FILE] [--display :N]"; exit 2; }
MINUTES=10; PROFILE="${BUILD}/soak-prof.json"; DISPLAY_NUM=":8"
while [ $# -gt 0 ]; do
  case "$1" in
    --minutes) MINUTES="$2"; shift 2;;
    --profile) PROFILE="$2"; shift 2;;
    --display) DISPLAY_NUM="$2"; shift 2;;
    *) echo "unknown option $1"; exit 2;;
  esac
done
for t in Xvfb xdotool; do command -v $t >/dev/null || { echo "$t not found"; exit 1; }; done
EXE="$BUILD/rust/prog"; [ -x "$EXE" ] || EXE="$BUILD/prog"
[ -x "$EXE" ] || { echo "no executable in $BUILD"; exit 1; }
mkdir -p "$BUILD/game"
export DISPLAY="$DISPLAY_NUM"
pkill -f "Xvfb $DISPLAY_NUM" 2>/dev/null || true
Xvfb "$DISPLAY_NUM" -screen 0 1280x720x24 +extension GLX >/dev/null 2>&1 &
XVFB=$!
sleep 2
trap 'kill $GAME 2>/dev/null; kill $XVFB 2>/dev/null' EXIT INT TERM

# MONO_ALLOC_SNAPSHOTS=1 (optional) adds per-snapshot class tables; set only when wanted (the variable is checked for presence)
env MONO_RESOURCES="$BUILD/resources" MONO_ALLOC_PROFILE="$PROFILE" MONO_ALLOC_DUMP_SECS=30 ${MONO_ALLOC_SNAPSHOTS:+MONO_ALLOC_SNAPSHOTS=1} "$EXE" -Dmono.graphics.backend=opengl \
  --version "${MC_VERSION:-26.3}" --gameDir "$BUILD/game" --assetsDir "$BUILD/assets" --assetIndex "${MC_ASSET_INDEX:-34}" \
  --username "${MC_USERNAME:-Player}" --accessToken 0 --userType legacy > "$BUILD/game/soak.log" 2>&1 &
GAME=$!

click() { xdotool mousemove "$1" "$2"; sleep 0.5; xdotool mousedown 1; sleep 0.3; xdotool mouseup 1; sleep "${3:-3}"; }
sleep 50
click 638 671 6                            # Continue (first-run accessibility screen)
click 638 353 6                            # Singleplayer
click 500 200 2                            # select the world
click 402 593 70                           # Play Selected World, wait for the terrain
END=$(( $(date +%s) + MINUTES * 60 ))
while [ "$(date +%s)" -lt "$END" ]; do
  kill -0 $GAME 2>/dev/null || { echo "the game exited"; break; }
  sleep 10
done
if kill -0 $GAME 2>/dev/null; then echo "soak finished after $MINUTES min, game still running"; else echo "game exited before the end"; exit 1; fi
ls "$BUILD/game/crash-reports" 2>/dev/null && { echo "crash report found"; exit 1; } || true
echo "profile: $PROFILE"
