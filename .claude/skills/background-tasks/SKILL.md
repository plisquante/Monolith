---
name: background-tasks
description: "Efficient handling of long-running commands. Use when running tests, builds, or any command that takes more than a few seconds. Prevents token waste from polling."
---

# Background Tasks

## Overview

Long-running commands (tests, builds, pushes) run in the background. Wait for them with ONE blocking call, not with repeated checks.

**The Iron Law:** wait with a single blocking wait (an `until` loop in a background Bash, or a Monitor). Never poll with repeated `sleep && cat`, `tail`, or `ps` calls.

**Tool check:** `TaskOutput` is not available in every session. Check first with `ToolSearch` (`select:TaskOutput`). If it reports no match, use the methods below. Do not retry it.

## When to Use Background Execution

| Command Type | Duration | Use Background? |
|--------------|----------|-----------------|
| Full Minecraft build (`build.sh`) | 30-70 min | YES |
| E2E tests | 2-10 min | YES |
| Full test suite | 1-5 min | YES |
| Git push with hooks | 30s-2 min | YES |
| Build/compile | 30s-5 min | YES |
| Single unit test | < 30s | NO |
| Quick commands | < 10s | NO |

**Rule of thumb:** if it may take more than 30 seconds, run it in the background.

## Method 1: One completion notification (default)

```
1. Bash(command, run_in_background: true)        → background task id
2. Bash(until grep -q '<DONE_MARKER>' "$LOG"; do sleep 5; done; tail -n 40 "$LOG",
        run_in_background: true, timeout: <ms>)  → one notification when finished
```

- The command writes its own done marker (for example `echo EXIT=$?` at the end of the job, redirected into the log). The `until` loop exits only when that marker appears.
- Keep the job's own output in a log file. Read only the filtered tail or the matching lines afterwards (`grep -E 'error|FAIL|Traceback'`), never the whole log.
- Make the wait loop exit on failure too. If the job can die without writing the marker, add a check on its PID (`kill -0 $PID`) to the loop condition.

## Method 2: Event stream (progress and failures)

Use Monitor when you want notifications while the job runs, not just at the end:

```
Monitor(
  command: "tail -n 0 -F \"$LOG\" | grep -E --line-buffered 'EXIT=|Traceback|Error|FAIL|panicked|No space|Killed'",
  description: "<job> log: exit and failure signatures",
  timeout_ms: 1800000
)
```

- The filter must match every terminal state: success marker and all failure signatures. A filter that matches only success stays silent on a crash, and silence looks like "still running".
- Monitor expires after `timeout_ms` (max 30 min). Re-arm it if the job is still running.
- Do not pipe through `head` in a Monitor. It buffers until N lines are seen.

## Parallel background tasks

Start several jobs, each with its own log and done marker, then wait on each with its own `until` loop. Do not run two heavy jobs at once if they compete for memory or CPU (see the monolith guard in `.claude/hooks/guard.py`).

## Non-blocking status

When you need to do other work and only want a snapshot, read the last lines of the log once:

```
tail -n 5 "$LOG"
```

Do this once per check, and only when you have other work to do. Do not loop on it.

## Timeout guidelines

| Task Type | Timeout | Rationale |
|-----------|---------|-----------|
| Single unit test file | 60000 (1 min) | Fast feedback |
| Integration tests | 120000 (2 min) | DB/API calls |
| E2E single spec | 300000 (5 min) | Browser startup + test |
| E2E full suite | 600000 (10 min) | Multiple specs |
| Git push with hooks | 180000 (3 min) | Pre-push tests |
| Full build | 3600000 (60 min) | Compilation and shard build |

## Red flags

You are doing it wrong if:
- A foreground `sleep` is used to wait for a job (foreground sleep is blocked in this environment; use a background `until` loop).
- `tail -f` or `cat` is run again and again on the output file.
- `ps aux | grep` is used to see whether a job is still alive.
- More than 2 or 3 tool calls are spent on one background job.

## Guard gotchas (monolith)

- The PreToolUse guard (`.claude/hooks/guard.py`) blocks any Bash command that matches its heavy-job pattern (`build.sh`, `run_tests.py`, `mcbuild_rust.py`, ...) while another heavy job runs. This includes a commit message or a grep pattern that contains one of those strings. Keep such strings out of commit messages and search patterns while a build runs.
