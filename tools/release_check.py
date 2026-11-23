#!/usr/bin/env python3
"""Runs the release gates of docs/RELEASE.md in order and prints one line per gate. Exit status 0 only if every gate
passes. A gate that needs a game build is FAILED when the build is missing (it is never silently skipped).

    MONO_JDK_HOME=<jdk 25> python3 tools/release_check.py [--build build/minecraft-26.3] [--soak-minutes 60]
"""
import argparse, os, subprocess, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def run(name, cmd, env=None, timeout=3600, check=None):
    try:
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, timeout=timeout,
                           env={**os.environ, **(env or {})})
    except subprocess.TimeoutExpired:
        return name, False, "timeout"
    if check is not None:
        ok, detail = check(r)
    else:
        ok, detail = r.returncode == 0, (r.stdout + r.stderr).strip().splitlines()[-1:] or [""]
        detail = detail[0] if isinstance(detail, list) else detail
    return name, ok, detail


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--build", default="build/minecraft-26.3")
    ap.add_argument("--soak-minutes", type=int, default=60)
    ap.add_argument("--skip-game", action="store_true", help="only the gates that need no game build")
    a = ap.parse_args()
    jdk = os.environ.get("MONO_JDK_HOME")
    if not jdk:
        print("FAIL  setup: MONO_JDK_HOME is not set")
        return 1

    results = []
    results.append(run("lint", [sys.executable, "-m", "pyflakes", "monolith", "tools", "tests"],
                       check=lambda r: (r.returncode == 0 and not r.stdout.strip(), r.stdout.strip()[:120] or "clean")))
    results.append(run("runtime unit tests", ["rustc", "--edition", "2021", "--test", "tests/rust/audio_main.rs",
                                              "-o", "/tmp/monolith_audio_tests"]))
    results.append(run("runtime unit tests (run)", ["/tmp/monolith_audio_tests"],
                       check=lambda r: ("test result: ok" in r.stdout, r.stdout.strip().splitlines()[-1:][0] if r.stdout.strip() else "")))
    for t in ("crc_tests", "netty_tests", "recycle_tests", "stack_tests", "patch_tests"):
        results.append(run(t, [sys.executable, "-m", f"monolith.{t}"], env={"MONO_JDK_HOME": jdk}, timeout=7200,
                           check=lambda r: (r.returncode == 0 and "ALL PASSED" in r.stdout,
                                            "ALL PASSED" if "ALL PASSED" in r.stdout else "not all passed")))

    if not a.skip_game:
        game = os.path.join(ROOT, a.build, "rust", "prog")
        if not os.path.isfile(game):
            results.append(("game build", False, f"missing: {game}"))
        else:
            results.append(run("game start (plain, 60 s)", ["tools/headless_play.sh", a.build, "--seconds", "60"],
                               timeout=900, check=lambda r: ("game still running" in r.stdout,
                                                             "running" if "still running" in r.stdout else "game did not run")))
            results.append(run(f"leak gate ({a.soak_minutes} min)",
                               [sys.executable, "tools/leak_budget.py", os.path.join(a.build, "soak-prof.json"),
                                "--warmup-s", str(a.soak_minutes * 60 // 2), "--budget-mb-per-min", "1"],
                               check=lambda r: (r.returncode == 0, r.stdout.strip().splitlines()[-1:][0] if r.stdout.strip() else "")))

    failed = 0
    for name, ok, detail in results:
        print(f"{'PASS' if ok else 'FAIL'}  {name}: {detail}")
        failed += 0 if ok else 1
    print(f"{len(results) - failed}/{len(results)} gates passed")
    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
