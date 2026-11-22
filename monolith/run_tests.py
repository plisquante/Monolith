#!/usr/bin/env python3
"""Monolith differential tests: every monolith/tests/*.java runs on HotSpot and as generated Rust; stdout and exit code must match."""
import glob, os, subprocess, sys, tempfile
HERE = os.path.dirname(os.path.abspath(__file__)); ROOT = os.path.dirname(HERE)
JDK = os.environ.get("MONO_JDK_HOME", "/opt/tools/jdk25")
env = {k: v for k, v in os.environ.items() if k not in ("JAVA_TOOL_OPTIONS", "_JAVA_OPTIONS")}
env["MONO_JDK_HOME"] = JDK
names = sys.argv[1:] or sorted(os.path.basename(p)[:-5] for p in glob.glob(os.path.join(HERE, "tests", "*.java")))
fails = 0
for n in names:
    with tempfile.TemporaryDirectory() as d:
        cdir = os.path.join(d, "classes"); os.makedirs(cdir)
        r = subprocess.run([f"{JDK}/bin/javac", "-d", cdir, os.path.join(HERE, "tests", n + ".java")], capture_output=True, text=True, env=env)
        if r.returncode:
            print("FAIL", n, "javac:", r.stderr[-300:]); fails += 1; continue
        j = subprocess.run([f"{JDK}/bin/java", "-cp", cdir, n], capture_output=True, text=True, env=env)
        out = os.path.join(d, "out")
        g = subprocess.run([sys.executable, "-m", "monolith.rustgen", "--cp", cdir, "--main", n, "-o", out, "--shards", "8", "--shard-jobs", "2", "--opt", "2", "--run"], capture_output=True, text=True, cwd=ROOT, env=env)
        if g.stdout == j.stdout and g.returncode == j.returncode:
            print("PASS", n, f"({len(j.stdout.splitlines())} lines, exit {j.returncode})")
        else:
            fails += 1
            print("FAIL", n, f"exit java={j.returncode} rust={g.returncode}")
            if g.returncode and g.stderr: print(g.stderr[-1500:])
            if g.stdout or j.stdout:
                a, b = j.stdout.splitlines(), g.stdout.splitlines()
                for i in range(max(len(a), len(b))):
                    x = a[i] if i < len(a) else "<none>"; y = b[i] if i < len(b) else "<none>"
                    if x != y: print(f"  line {i+1}: java={x!r} rust={y!r}"); break
sys.exit(1 if fails else 0)
