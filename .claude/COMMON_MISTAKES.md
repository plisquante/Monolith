# Common mistakes
- Tests and builds must use JDK 25: `export MONO_JDK_HOME=$HOME/.cache/monolith/temurin-25/jdk-25.0.5+8` (JAVA_HOME may point to 21 and silently produce wrong output).
- Do not run the Minecraft Rust build (`tools/mcbuild_rust.py`) and the test suite at once (OOM kills rustc).
- On machines with < 8 GiB RAM the guard/test floors scale to /proc/meminfo; override with MONO_MIN_AVAIL_GIB (guard) and MONO_TEST_MIN_AVAIL_GIB (tests/run_tests.py, default 0.5 GiB, 0 disables).
- Field names are mangled: `_` -> `_1`, `$` -> `_2` (e.g. `f_st_1mode`). See `monolith/world.py:mangle`.
- Natives are Rust functions named `n_<class>__<method>` in `monolith/rt/natives*.rs` (underscores in method names become `_1`).
- Generated sources in `<out>/src` are rebuilt on every transpile; fix the generator, not the output.
- `tools/mcbuild_rust.py` default graphics is vulkan; use `--graphics opengl` / `MC_GRAPHICS=opengl` under Xvfb + `LIBGL_ALWAYS_SOFTWARE=1`.
- Complex `rm`-style one-liners get blocked; do such edits via Python.
