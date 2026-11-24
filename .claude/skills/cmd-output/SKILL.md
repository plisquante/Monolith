---
name: cmd-output
description: Filter long command output. Use before running tests, builds, rustc/python compiles, git log/diff, or any command that can print more than ~40 lines.
---

# cmd-output

`rtk` (rtk-ai/rtk) is installed globally with a PreToolUse hook that rewrites common commands (git, ls, grep, test runners) to compact output. Verify: `rtk --version`. If missing, use the manual filters below.

## Never print raw
- Test suite: `python3 tests/run_tests.py ... 2>&1 | tail -40`; on failure add `| grep -nE 'FAIL|Error|error:|Traceback|panic' | head -30`.
- Rust compile: `python3 -m monolith.rustgen ... --compile 2>&1 | grep -E '^error|^warning: unused|panicked' | head -30`, then read only the cited file ranges.
- Transpile: `python3 -m monolith ... 2>&1 | tail -30`; details in `<out>/report.json` via `python3 -c` key extraction, never `cat`.
- Git: `git log --oneline -n 15`, `git diff --stat`, then `git diff -- <one file>`. `git status -sb`.
- Logs: `tail -n 60`, `grep -c` for counts, `grep -m 20` for first hits.

## Rules
- Redirect big output to the scratchpad file, then grep/tail it. Do not re-run to see more.
- Report the verdict line plus failing names only.
- Exit code decides success, not absence of text. Check `echo $?` or `set -o pipefail` when piping.
