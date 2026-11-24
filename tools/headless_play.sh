#!/bin/sh
# Plays the natively built game headless (Xvfb + xdotool) so a change can be checked and profiled the same way
# every time: opens the saved world, walks and looks around for a while, takes screenshots.
#
#   tools/headless_play.sh <build-dir> [--seconds 180] [--shots DIR] [--display :7] [--clean-exit]
#
# <build-dir> is what tools/mcbuild_rust.py wrote: it holds rust/prog, resources/ and assets/. Needs xvfb, xdotool
# and imagemagick (import). Environment passes through, so this works too:
#   MONO_ALLOC_PROFILE=/tmp/p.json  MONO_PATCH_VERIFY=1  MONO_ARENA_CHECK=abort  tools/headless_play.sh build/minecraft-26.3
# A game directory without a saved world gets one: Singleplayer then opens "Create New World" and the script creates
# it (checked on 2026-11-24 from an empty game directory). With a saved world it selects the first one.
# --clean-exit ends the run like a player: Escape, "Save and Quit to Title", "Quit Game", then waits for the process to end
# (so exit handlers run, e.g. the MONO_COUNT_NATIVES table) and fails if the exit status is not 0.
# A crash writes mono-crash-<pid>.txt into the game directory (MONO_CRASH_DIR); its presence fails the run.
# The click coordinates assume the 1280x720 window the game opens with.
set -e
BUILD="$1"; shift || { echo "usage: $0 <build-dir> [--seconds N] [--shots DIR] [--display :N] [--clean-exit]"; exit 2; }
SECONDS_TOTAL=180; SHOTS="${BUILD}/shots"; DISPLAY_NUM=":7"; CLEAN_EXIT=0
while [ $# -gt 0 ]; do
  case "$1" in
    --seconds) SECONDS_TOTAL="$2"; shift 2;;
    --shots) SHOTS="$2"; shift 2;;
    --display) DISPLAY_NUM="$2"; shift 2;;
    --clean-exit) CLEAN_EXIT=1; shift;;
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

rm -f "$BUILD"/game/mono-crash-*.txt
MONO_CRASH_DIR="$BUILD/game" MONO_RESOURCES="$BUILD/resources" "$EXE" -Dmono.graphics.backend=opengl --version "${MC_VERSION:-26.3}" \
  --gameDir "$BUILD/game" --assetsDir "$BUILD/assets" --assetIndex "${MC_ASSET_INDEX:-34}" \
  --username "${MC_USERNAME:-Player}" --accessToken 0 --userType legacy > "$BUILD/game/headless.log" 2>&1 &
GAME=$!

click() { xdotool mousemove "$1" "$2"; sleep 0.5; xdotool mousedown 1; sleep 0.3; xdotool mouseup 1; sleep "${3:-3}"; }
sleep 50                                   # start-up
import -window root "$SHOTS/01-title.png"
click 638 671 6                            # Continue (first-run accessibility screen)
click 638 353 6                            # Singleplayer
if [ -n "$(ls -A "$BUILD/game/saves" 2>/dev/null)" ]; then
  click 500 200 2                          # select the world
  click 402 593 70                         # Play Selected World, wait for the terrain
else
  # no world yet: the screen is "Create New World". Game Mode cycles Survival, Hardcore, Creative: two clicks make it
  # Creative, so a random spawn in water cannot drown the walking player and end the run
  click 638 257 1
  click 638 257 1
  click 402 671 100                        # create it, wait for the terrain
fi
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
ls "$BUILD"/game/mono-crash-*.txt 2>/dev/null && { echo "native crash report found"; exit 1; } || true
if [ "$CLEAN_EXIT" = 1 ]; then
  # SIGTERM, as a system shutdown or `kill` sends it: the runtime runs the shutdown hooks (Minecraft saves the world there)
  # and exits with 143. Driving the pause menu with the mouse is not reliable under software GL (a frame can take seconds).
  T0=$(date +%s)
  kill -TERM $GAME
  i=0; while kill -0 $GAME 2>/dev/null && [ $i -lt 90 ]; do sleep 1; i=$((i + 1)); done
  if kill -0 $GAME 2>/dev/null; then echo "the game did not end within 90 s of SIGTERM"; exit 1; fi
  set +e; wait $GAME; STATUS=$?; set -e
  [ "$STATUS" = 143 ] || { echo "exit status $STATUS after SIGTERM (143 expected: shutdown hooks ran, then exit)"; exit 1; }
  SAVED=$(find "$BUILD/game/saves" -name level.dat -newermt "@$T0" 2>/dev/null | head -1)
  [ -n "$SAVED" ] || { echo "the world was not saved on exit (no level.dat newer than the SIGTERM)"; exit 1; }
  echo "clean exit: status 143 after SIGTERM, world saved on exit"
fi
echo "screenshots: $SHOTS"
