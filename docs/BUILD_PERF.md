# Build speed: shards and parallel jobs

Question: does compiling the generated Rust in parallel (more `rustc` shards, more concurrent jobs) make the build
faster on a multi-core machine?

## Measurement (one run each, same input)

Input: `monolith/tests/stack/Stack1.java` compiled with javac, then `python3 -m monolith.rustgen --main Stack1
--opt 1 --compile`. The class library was already built (cached), so the numbers are the program build only.

| shards | shard jobs | wall time |
|---|---|---|
| 1 | 1 | 217.6 s |
| 4 | 4 | 70.5 s |

Speedup: about 3.1x on 4 cores (1358 classes, 7036 methods in the output).

Machine: 4 vCPU, 15 GB RAM, Linux. One run per setting; the numbers are not averaged.

## How to reproduce

    tools/bench_build.sh <classes dir> <main class> <shards> <jobs>

It prints the wall-clock time of one build with a fresh output directory. Run it twice per setting and compare.

## Notes

* The defaults follow the core count: `tests/run_tests.py` and `monolith/stack_tests.py` use `min(4, cores)` shards
  and `cores` jobs. `tools/mcbuild_rust.py` (the Minecraft build) still uses `--shards 32 --shard-jobs 3`; it has
  not been measured on this machine.
## Not measured yet

* The Minecraft build (about 65 min and 12 GB with the defaults). It needs the client jar and a quiet machine; run
  `tools/bench_build.sh` on the Minecraft classes with `--shards 4 --shard-jobs 4` and with the defaults, and
  record both here.
* Repeated runs: one run per setting is not enough for a claim about speed. Three runs each would be the minimum.

* Memory: each concurrent `rustc` takes its own share. The 4x4 run stayed well inside 15 GB for Stack1, which is
  much smaller than a Minecraft build.
