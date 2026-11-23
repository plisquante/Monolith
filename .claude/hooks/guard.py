#!/usr/bin/env python3
"""Claude Code PreToolUse hook: blocks actions that must never be taken in this repository.

Protocol (stdin): a JSON object {"tool_name": ..., "tool_input": {...}, ...}.
Protocol (exit):  0 = allow, 2 = block (stderr is shown to the caller).

Rules:
  1. Minecraft artifacts are never committed: *.jar, mc/, out/, build/, tests/build/,
     mcout/, *.o, the minecraft executable. Blocked for `git add`, `git commit`
     (checks the staged files) and for Write/Edit of those paths.
  2. Unsafe git is refused: force push, `git reset --hard`, `git clean -f...`, `git branch -D`.
  3. At most one heavy job at a time (zig build-exe/build-obj, tests/run_tests.py,
     tests/run_client_core_e2e.py, tools/mcbuild.py, build.sh). Starting a second one is blocked.
  4. Heavy jobs are refused when MemAvailable is below a floor (see mem_floor_gib()).

Fail-closed: an internal error blocks the action (exit 2) instead of silently allowing it.
--self-test runs the unit tests of the decision functions and exits 0/1.
--print-config shows the machine-derived values (memory floor, sources, paths).
"""
from __future__ import annotations

import json
import os
import re
import shlex
import subprocess
import sys

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
ARTIFACT_RE = re.compile(
    r"(^|/)(out|mc|build|tests/build|\.zig-cache|zig-cache)(/|$)"
    r"|(^|/)mcout(/|$)"
    r"|\.jar$"
    r"|\.o(bj)?$"
    r"|(^|/)minecraft(\.exe)?$"
    r"|\.zip$",
    re.IGNORECASE,
)
HEAVY_RE = re.compile(
    r"zig\s+(build-exe|build-obj)\b"
    r"|(^|\s|/)(tests/)?run_tests\.py\b"
    r"|(^|\s|/)(tests/)?run_client_core_e2e\.py\b"
    r"|tools/mcbuild\.py\b|tools\\mcbuild\.py\b"
    r"|(^|\s|/)mcbuild\.py\b"
    r"|build\.sh\b"
)
def mem_total_gib() -> float:
    try:
        with open("/proc/meminfo") as fh:
            for line in fh:
                if line.startswith("MemTotal:"):
                    return int(line.split()[1]) / (1024 * 1024)
    except OSError:
        pass
    return 16.0  # unknown platform: assume a big machine (matches the historical default)


def mem_avail_gib() -> float:
    try:
        with open("/proc/meminfo") as fh:
            for line in fh:
                if line.startswith("MemAvailable:"):
                    return int(line.split()[1]) / (1024 * 1024)
    except OSError:
        pass
    return mem_total_gib()


def mem_floor_gib() -> float:
    """Free memory required to start a heavy job. Scales to the machine so the same rule
    is meaningful on a 2 GiB laptop and a 15 GiB desktop; override with MONO_MIN_AVAIL_GIB."""
    if os.environ.get("MONO_MIN_AVAIL_GIB"):
        return float(os.environ["MONO_MIN_AVAIL_GIB"])
    return 4.0 if mem_total_gib() >= 8.0 else 1.0


def other_heavy_running(self_pid: int | None = None) -> list[str]:
    try:
        out = subprocess.run(
            ["pgrep", "-af", HEAVY_RE.pattern],
            capture_output=True, text=True, timeout=5,
        ).stdout
    except Exception:
        return []
    me = os.getpid()
    lines = []
    for line in out.splitlines():
        pid = int(line.split(" ", 1)[0]) if line.split(" ", 1)[0].isdigit() else -1
        if pid in (me, self_pid):
            continue
        if "guard.py" in line:
            continue
        lines.append(line.strip())
    return lines


def staged_artifacts() -> list[str]:
    try:
        out = subprocess.run(
            ["git", "-C", REPO, "diff", "--cached", "--name-only"],
            capture_output=True, text=True, timeout=10,
        )
    except Exception:
        return []
    return [l for l in out.stdout.splitlines() if l and ARTIFACT_RE.search(l)]


def command_paths(cmd: str) -> list[str]:
    try:
        return shlex.split(cmd)
    except ValueError:
        return cmd.split()


def is_heavy(cmd: str) -> bool:
    return bool(HEAVY_RE.search(cmd))


def decide_bash(cmd: str) -> str | None:
    """Returns a block reason, or None to allow."""
    paths = command_paths(cmd)

    # ---- 1/2. git rules -------------------------------------------------
    if re.search(r"\bgit\b", cmd):
        # explicit staging of forbidden artifacts, e.g. `git add out/ app.jar`
        if re.search(r"\bgit\s+(add|stage)\b", cmd):
            bad = [t for t in paths[1:] if not t.startswith("-") and ARTIFACT_RE.search(t)]
            if bad:
                return f"refusing to stage build artifacts ({', '.join(bad)}): *.jar, mc/, out/, build/, tests/build/ must never be committed"
        if re.search(r"\bgit\s+commit\b", cmd):
            bad = [f for f in staged_artifacts() if ARTIFACT_RE.search(f)]
            if bad:
                return f"refusing to commit: staged files are build artifacts ({', '.join(bad[:5])}): Minecraft/derived output must never be committed"
        if re.search(r"\bgit\s+push\b", cmd) and re.search(r"(--force($|\s|=)|--force-with-lease($|\s|=)|\s-f($|\s))", cmd):
            return "refusing to force-push (prompt.md: never force-push)"
        if re.search(r"\bgit\s+reset\b", cmd) and re.search(r"\s--hard\b", cmd):
            return "refusing `git reset --hard` (destructive; use `git switch`/`git restore` on explicit paths)"
        if re.search(r"\bgit\s+clean\b", cmd) and re.search(r"-[a-zA-Z]*f", cmd) and re.search(r"-[a-zA-Z]*[dx]", cmd):
            return "refusing `git clean -f...` (destructive, can delete untracked sources)"
        if re.search(r"\bgit\s+branch\b", cmd) and re.search(r"\s-D\b", cmd):
            return "refusing `git branch -D` (destructive branch delete)"

    # ---- 3/4. heavy job rules -------------------------------------------
    if is_heavy(cmd):
        others = other_heavy_running()
        if others:
            return ("another heavy job is already running (never run two at a time):\n  "
                    + "\n  ".join(others[:3]))
        avail = mem_avail_gib()
        floor = mem_floor_gib()
        if avail < floor:
            return (f"refusing to start a heavy job: {avail:.1f} GiB available, "
                    f"floor is {floor:.1f} GiB (set MONO_MIN_AVAIL_GIB to override)")

    return None


def decide_file(path: str) -> str | None:
    if path and ARTIFACT_RE.search(path.replace("\\", "/")):
        return f"refusing to write build artifact path {path!r}: out/, mc/, build/, tests/build/, *.jar are never written by an agent"
    return None


def decide(tool: str, tool_input: dict) -> str | None:
    if tool == "Bash":
        cmd = tool_input.get("command") or tool_input.get("cmd") or ""
        return decide_bash(str(cmd))
    if tool in ("Write", "Edit", "NotebookEdit"):
        path = tool_input.get("file_path") or tool_input.get("path") or ""
        return decide_file(str(path))
    return None


def main() -> int:
    if "--print-config" in sys.argv:
        print(f"repo:            {REPO}")
        print(f"mem total:       {mem_total_gib():.1f} GiB")
        print(f"mem available:   {mem_avail_gib():.1f} GiB")
        print(f"heavy-job floor: {mem_floor_gib():.1f} GiB (MONO_MIN_AVAIL_GIB overrides)")
        return 0
    if "--self-test" in sys.argv:
        return self_test()
    try:
        payload = json.load(sys.stdin)
    except Exception as exc:
        print(f"guard internal error: cannot parse hook input ({exc}); refusing", file=sys.stderr)
        return 2
    try:
        reason = decide(payload.get("tool_name", ""), payload.get("tool_input", {}) or {})
    except Exception as exc:
        print(f"guard internal error: {exc}; refusing (fail-closed)", file=sys.stderr)
        return 2
    if reason:
        print(f"guard: {reason}", file=sys.stderr)
        return 2
    return 0


def self_test() -> int:
    global mem_avail_gib, other_heavy_running, staged_artifacts
    fails: list[str] = []

    def check(name: str, cond: bool) -> None:
        print(("PASS" if cond else "FAIL"), name)
        if not cond:
            fails.append(name)

    # heavy-command detection
    check("heavy: zig build-exe", is_heavy("zig build-exe src/main.zig -ODebug"))
    check("heavy: run_tests.py", is_heavy("python3 tests/run_tests.py Arith"))
    check("heavy: e2e", is_heavy("python3 tests/run_client_core_e2e.py -O Debug"))
    check("heavy: mcbuild", is_heavy("python3 tools/mcbuild.py client.jar"))
    check("not heavy: zig ast-check", not is_heavy("zig ast-check runtime/rt.zig"))
    check("not heavy: git status", not is_heavy("git status --porcelain"))
    check("not heavy: ls", not is_heavy("ls -la"))

    # artifact staging / committing (stub git)
    real_staged = staged_artifacts
    staged_artifacts = lambda: ["out/minecraft", "src/keep.py"]  # type: ignore[assignment]
    check("commit blocked on staged artifact", decide_bash("git commit -m x") is not None)
    check("add blocked on artifact path", decide_bash("git add out/minecraft") is not None)
    check("add of jar blocked", decide_bash("git add libs/client.jar") is not None)
    check("add of normal file allowed", decide_bash("git add src/keep.py") is None)
    staged_artifacts = lambda: ["src/keep.py"]  # type: ignore[assignment]
    check("commit with clean staging allowed", decide_bash("git commit -m x") is None)
    staged_artifacts = real_staged  # type: ignore[assignment]

    # unsafe git
    check("force push blocked", decide_bash("git push --force origin ccr-ebbdaecb-mnpapy") is not None)
    check("force-with-lease blocked", decide_bash("git push --force-with-lease") is not None)
    check("-f push blocked", decide_bash("git push -f origin main") is not None)
    check("reset --hard blocked", decide_bash("git reset --hard HEAD~1") is not None)
    check("clean -fdx blocked", decide_bash("git clean -fdx") is not None)
    check("branch -D blocked", decide_bash("git branch -D old") is not None)
    check("normal push allowed", decide_bash("git push -u origin ccr-ebbdaecb-mnpapy") is None)
    check("status allowed", decide_bash("git status") is None)

    # heavy-job rules with stubs
    real_avail, real_others = mem_avail_gib, other_heavy_running
    other_heavy_running = lambda *a, **kw: ["123 zig build-exe main.zig"]  # type: ignore[assignment]
    check("concurrent heavy job blocked", decide_bash("python3 tests/run_tests.py Oop") is not None)
    other_heavy_running = lambda *a, **kw: []  # type: ignore[assignment]
    mem_avail_gib = lambda: 0.5  # type: ignore[assignment]
    check("low memory heavy job blocked", decide_bash("python3 tests/run_tests.py Oop") is not None)
    mem_avail_gib = lambda: 8.0  # type: ignore[assignment]
    check("enough memory heavy job allowed", decide_bash("python3 tests/run_tests.py Oop") is None)

    # file rule
    check("write of out/ blocked", decide_file("out/minecraft") is not None)
    check("write of jar blocked", decide_file("libs/x.jar") is not None)
    check("write of normal file allowed", decide_file("src/ok.py") is None)

    mem_avail_gib, other_heavy_running = real_avail, real_others  # type: ignore[assignment]
    print()
    if fails:
        print(f"guard self-test: {len(fails)} FAILURE(S)")
        return 1
    print("guard self-test: all checks passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
