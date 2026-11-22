#!/bin/sh
# Plays the natively built game headless (Xvfb + xdotool) so a change can be checked and profiled the same way
# every time: opens the saved world, walks and looks around for a while, takes screenshots.
#
#   tools/headless_play.sh <build-dir> [--seconds 180] [--shots DIR] [--display :7]
#
# <build-dir> is what tools/mcbuild_rust.py wrote: it holds rust/prog, resources/ and assets/. Needs xvfb, xdotool
# and imagemagick (import). Environment passes through, so this works too:
#   MONO_ALLOC_PROFILE=/tmp/p.json  MONO_PATCH_VERIFY=1  MONO_ARENA_CHECK=abort  tools/headless_play.sh build/minecraft-26.3
# The world "New World" must exist (create it once by hand: Singleplayer, Create New World).
# The click coordinates assume the 1280x720 window the game opens with.
set -e
BUILD="$1"; shift || { echo "usage: $0 <build-dir> [--seconds N] [--shots DIR] [--display :N]"; exit 2; }
SECONDS_TOTAL=180; SHOTS="${BUILD}/shots"; DISPLAY_NUM=":7"
while [ $# -gt 0 ]; do
  case "$1" in
    --seconds) SECONDS_TOTAL="$2"; shift 2;;
    --shots) SHOTS="$2"; shift 2;;
    --display) DISPLAY_NUM="$2"; shift 2;;
    *) echo "unknown option $1"; exit 2;;
  esac
done
for t in Xvfb xdotool import; do command -v $t >/dev/null || { echo "$t not found"; exit 1; }; done
EXE="$BUILD/rust/prog"; [ -x "$EXE" ] || EXE="$BUILD/prog"
[ -x "$EXE" ] || { echo "no executable in $BUILD (rust/prog)"; exit 1; }
mkdir -p "$SHOTS" "$BUILD/game"
export DISPLAY="$DISPLAY_NUM"
pkill -f "Xvfb $DISPLAY_NUM" 2>/dev/null || true
Xvfb "$DISPLAY_NUM" -screen 0 1280x720x24 +extension GLX >/dev/null 2>&1 &
XVFB=$!
sleep 2
trap 'kill $GAME 2>/dev/null; kill $XVFB 2>/dev/null' EXIT INT TERM

MONO_RESOURCES="$BUILD/resources" "$EXE" -Dmono.graphics.backend=opengl --version "${MC_VERSION:-26.3}" \
  --gameDir "$BUILD/game" --assetsDir "$BUILD/assets" --assetIndex "${MC_ASSET_INDEX:-34}" \
  --username "${MC_USERNAME:-Player}" --accessToken 0 --userType legacy > "$BUILD/game/headless.log" 2>&1 &
GAME=$!

click() { xdotool mousemove "$1" "$2"; sleep 0.5; xdotool mousedown 1; sleep 0.3; xdotool mouseup 1; sleep "${3:-3}"; }
sleep 50                                   # start-up
import -window root "$SHOTS/01-title.png"
click 638 671 6                            # Continue (first-run accessibility screen)
click 638 353 6                            # Singleplayer
click 500 200 2                            # select the world
click 402 593 70                           # Play Selected World, wait for the terrain
import -window root "$SHOTS/02-world.png"
END=$(( $(date +%s) + SECONDS_TOTAL ))
N=3
while [ "$(date +%s)" -lt "$END" ]; do
  kill -0 $GAME 2>/dev/null || { echo "the game exited"; break; }
  xdotool keydown w; sleep 6; xdotool keydown space; sleep 0.4; xdotool keyup space; sleep 5; xdotool keyup w
  for k in 1 2 3 4 5; do xdotool mousemove_relative -- 60 0; sleep 0.6; done
  import -window root "$SHOTS/$(printf %02d $N)-walk.png"; N=$((N + 1))
  sleep 4
done
if kill -0 $GAME 2>/dev/null; then echo "game still running after $SECONDS_TOTAL s: OK"; else echo "game exited"; wait $GAME || true; exit 1; fi
ls "$BUILD/game/crash-reports" 2>/dev/null && { echo "crash report found"; exit 1; } || true
echo "screenshots: $SHOTS"
