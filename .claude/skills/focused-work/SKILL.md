---
name: focused-work
description: Working protocol for medium effort. Use at session start and before any search, read, or sub-agent call. Prevents redundant grep/read loops.
---

# focused-work

## Before touching files
1. Map once: `.claude/ARCHITECTURE_MAP.md` already lists every module. Use it. Run `git ls-files | rtk`-style summary only if the map lacks the target.
2. Name the target file and symbol before any search. One `Grep` with a precise pattern, then one `Read` with offset and limit on the hit.
3. Independent lookups go in one parallel batch. Dependent ones wait.

## Reading
- Never re-read a file already read this session unless it changed.
- Read ranges (`offset`/`limit`), not whole files. Skip files over 100KB (generated Zig, logs, reports, `docs/STATUS.md` body) unless the task requires them; grep them.
- Generated output (`out/`, `mc/`, `classlib/build/`, `tests/build/`) is never read for editing. Fix the generator.

## Editing
- Edit the exact lines. Use `Edit` with the smallest unique `old_string`. No whole-file rewrites of existing files.
- After editing, do not re-read to verify. Run the narrowest check (one test, `python3 -m py_compile`, `zig ast-check <file>`).

## Delegation
- No sub-agents for lookups, greps, or single edits. Do them inline.
- Spawn one only for a truly independent long task the user asked to parallelize.
- One heavy job at a time (see `monolith-build`).

## Stop conditions
- Two failed attempts on the same hypothesis: state the evidence and change approach, do not repeat.
- Blocked by environment (no network host, missing tool): report the exact blocker, do not fake the result.
