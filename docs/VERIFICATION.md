# Headless gameplay verification (Linux, 26.3 client)

Scope: what a scripted run of the Rust build (`tools/mcbuild_rust.py`) showed on this machine. It is a smoke test
through the real window, not a test suite. Input goes through xdotool; a plain click or key press was not reliable,
so mouse buttons and keys were held for 0.3 s or more.

Setup: Xvfb `:99` at 1280x720x24 with GLX, `MC_GRAPHICS=opengl`, `LIBGL_ALWAYS_SOFTWARE=1`, `./run.sh` in the build
directory. Options: `onboardAccessibility:false`, `narrator:0`, `skipMultiplayerWarning:true`.

| Step | Result |
|---|---|
| Boot to the title screen | pass (about 70 s on 4 vCPU, software GL) |
| Singleplayer, world list | pass: saved world listed |
| Load saved world | pass: "Loading terrain" then the world renders |
| Walk forward (W held 3 s) | pass: the tutorial advanced from "Move with W, A, S and D" to "Look around" |
| Pause menu (Escape) | pass: Game Menu with "Save and Quit to Title" |
| Save and quit to title | pass: back on the title screen, no crash |
| Reload the saved world | pass: same view and HUD as before the save |

Log: the run log has three error lines, all network related: Mojang user-properties fetch, and Realms
(`Couldn't connect to realms`, one PKIX error because the sandbox proxy certificate is not in the game's trust
store). No exception or panic was logged.

Concurrent GC (`MONO_GC_CONCURRENT=1`), one boot: title screen reached, process still running at 120 s, no
ClassCastException, panic or fatal line in the log. The same three network-only error lines as the plain run.

Not covered: multiplayer, Realms, audio output, a second session, long play, repeated boots, FPS and memory figures.
