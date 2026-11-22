# CLAUDE.md

Minecraft: Zig Edition. j2zig transpiles JVM bytecode (Minecraft, libs, JDK 25) to Zig, builds a JVM-free native executable. Python transpiler `j2zig/`, Zig runtime `runtime/`, Java overlay `classlib/src`, Android launcher `android/touchcore`.

## Start
- Map: `.claude/ARCHITECTURE_MAP.md` (read once). Status: `docs/STATUS.md` (grep, do not read whole).
- Load skills on demand from `.claude/skills/`: `focused-work` (always), `caveman` (always), `ponytail` (code tasks), `cmd-output`, `j2zig-build`, `git-attribution`.

## Reply
- Terse, answer first, no filler, no emoji, no em-dashes. Details: `caveman`.
- Reply in the user's language.

## Work
- Read before write. No re-reads unless changed. Target line ranges.
- No sub-agents for lookups or single edits.
- Skip files over 100KB (generated Zig, logs, reports) unless required.
- Do not guess APIs, versions, flags, SHAs, package names. Verify.
- No stubs, no TODO, no fake success. Unsupported must fail loudly and be reported.
- Behavioral equivalence vs HotSpot first (`tests/run_tests.py`), optimize after.

## Output
- Run `rtk`-wrapped or filtered commands. Never print raw test, build, or git log output: `| tail -40`, `| grep -E 'error|FAIL'`. Details: `cmd-output`.

## Hard limits
- Branch: the one the session names. Never commit jar, `mc/`, `out/`, `build/`, `tests/build`.
- Zig compile of Minecraft needs ~10 GB. Never run with the test suite. See `j2zig-build`.
- Commits and PRs: authors `cem-ali152` and `plisquante` only. No Co-Authored-By, no Claude-Session, no mention of Claude or AI. See `git-attribution`.
