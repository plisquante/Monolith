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

Concurrent GC (`MONO_GC_CONCURRENT=1`), first boot: title screen reached, process still running at 120 s, no
ClassCastException, panic or fatal line in the log. The same three network-only error lines as the plain run.

Concurrent GC, second boot, same script: the saved world loaded with the same view, walking advanced the tutorial to
"Look around", save and quit returned to the title. No crash, panic or fatal line; the same three network-only errors.
Two runs out of two in concurrent mode; the 60 min leak soak is still required before it can be the default.

## Android native core (build and link, no device)

| Step | Result |
|---|---|
| `cargo test --offline` in `android/touchcore/native` | pass: 28 of 28 host tests |
| `tools/build_touchcore_apk.py` with `ANDROID_NDK_HOME` = NDK 26.3.11579264 | pass: Rust core for `aarch64-linux-android` (release), C bridge, APK signed, 5.58 MiB |
| `libtouchcore_mono.so` in the APK | AArch64 shared object; exports `tc_init`, `tc_tick_frame`, `tc_shutdown`; needs only `libandroid`, `liblog`, `libc`, `libm`, `libdl` |

Not covered: running the engine on a device or emulator, the GPU back ends (`rhi.rs` returns `UnsupportedBackend` for
Vulkan and GL 3.3), the Minecraft engine library (`libminecraft_mono.so`) inside the APK.

## Leak gate, 60 minutes standing still (collector on, plain mode)

Run: `tools/leak_soak.sh /home/user/mc_build --minutes 60` (saved world, no movement), then
`tools/leak_budget.py soak-prof.json --warmup-s 1800 --budget-mb-per-min 1`.

| Measure | Result |
|---|---|
| Game still running after 60 min, no crash report | yes |
| Collections, total / after 1800 s warm-up | 111 / 42 |
| Live after first collection / last | 7.9 MiB / 593.5 MiB |
| Live growth after warm-up (least squares) | -0.054 MiB/min, budget 1.00, within budget, tool exit 0 |
| Pause, longest / mean | 1473 ms / 1045 ms |

What this shows: after the warm-up the live managed heap is flat, so this gate passes as written. What it does not
show: the heap grew from 7.9 to 593.5 MiB over the whole run, the warm-up window hides that, and the pauses are about
1 s, far above the < 1 ms target in `GC_PLAN.md`. The run used the tracing collector, not the collector-off mode, so it
says nothing yet about removing the collector. One pass only.

## Collector-off switch (`MONO_GC=off`), test programs only

`tests/run_tests.py GcStress Oop Threads` passes with `MONO_GC=off`.

Game run with `MONO_GC=off` (rebuilt binary, saved world, standing still): the process reached 13.3 GB resident memory
after about 10 minutes on this 16 GB machine. The run was stopped there to avoid an out-of-memory kill, so the 60 min
gate was not completed. With the collector off the heap is not bounded, and the collector stays in the build for now.

Not covered: multiplayer, Realms, audio output, a second session, long play, repeated boots, FPS and memory figures.
