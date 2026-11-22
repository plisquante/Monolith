# Hardware requirements per task

Last updated: 2026-11-16. Numbers for the big machine are from the original author's 4-core,
llvmpipe box (see `docs/STATUS.md` sections 8–12). The constrained-machine column was measured
on the machine that audited this repository in November 2026.

## The machine that audited this repo (2026-11-16)

| resource | value |
|---|---|
| CPU | 2 vCPU, Intel Xeon @ 2.60 GHz |
| RAM | 1.94 GiB (`MemTotal: 2030416 kB`), **no swap** |
| disk | ~25 GB, 17% used at start |
| kernel | 6.1.189+ x86_64 |

## What fits where

| task | minimum sensible | on the 2 vCPU / 1.9 GiB box |
|---|---|---|
| `python3 -m py_compile`, `pyflakes`, `zig ast-check` | any | OK (< 1 s per file) |
| `pyflakes`/`ast-check` full pass | any | OK |
| **one** differential test, Debug, self-hosted backend (`tests/run_tests.py Arith`) | ~2 GiB free, 1 job | **PASS in 495 s** (build) + 0.58 s native; machine under heavy load (loadavg ~10), interactivity suffers |
| differential suite, all of `tests/java/` | ≥ 4 GiB free, hours of CPU | thrash risk: do not run; tests are refused below `J2Z_TEST_MIN_AVAIL_GIB` (default 0.5 GiB) |
| full Minecraft build (`build.sh client.jar`) | Debug self-hosted: ~10 GB peak; LLVM unsharded: > 15 GB (OOM-killed on the author's 16 GB machine) | impossible; nothing in `./build.sh` will succeed here |
| Client Core E2E (`tests/run_client_core_e2e.py`), transpile 5 901 classes + compile | ≥ 8 GiB, tens of minutes | impossible; not run here |
| microbench (`tools/microbench/run_client_core_bench.py`) | ReleaseFast, ~2–4 GiB | not run here (ReleaseFast LLVM memory needs) |

## Rules the scripts now enforce

- `tests/run_tests.py` refuses to start a test build below `--min-avail-gib`
  (default 0.5 GiB, env `J2Z_TEST_MIN_AVAIL_GIB`), re-checks before every test and stops the
  suite with exit code 3 rather than thrashing.
- `.claude/hooks/guard.py` refuses starting a heavy job below 1 GiB available
  (4 GiB on ≥ 8 GiB machines; `J2Z_MIN_AVAIL_GIB` overrides), refuses a second concurrent
  heavy job and blocks committing artifacts.
- One heavy job at a time. Never alongside the test suite on a big machine either
  (CLAUDE.md); on this box, never at all.

## Disk expectations

`tests/build/<Name>` keeps about 1–1.4 GB per test (`--keep`); the harness deletes it after
passed tests when several names are given. `tests/build/zig-cache` can grow by hundreds of MB.
Minecraft builds need tens of GB and are out of scope for small disks. Use `rm -rf tests/build
~/.cache/zig` between heavy runs (the gate does this).
