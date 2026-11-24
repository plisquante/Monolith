#!/usr/bin/env python3
"""Narrator without JNA (patches/rust/narrator.rs, work package N1).

1. Narrator.getNarrator() gives the same result as HotSpot with the real JNA (the flite narrator when libflite and the kal16
   voice are installed, the empty narrator otherwise).
2. JNA's registration is gone: MONO_COUNT_NATIVES reports no `registerMethod`, `findSymbol` or `open` call of com.sun.jna.Native.
Needs the text2speech, jna, slf4j and guava jars (Minecraft's libraries, ~/.cache/monolith/mc/libraries).
Usage: MONO_JDK_HOME=<jdk 25> python3 -m monolith.narrator_tests
"""
import glob
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
JDK = os.environ.get("MONO_JDK_HOME", "/opt/tools/jdk25")
ENV = {k: v for k, v in os.environ.items() if k not in ("JAVA_TOOL_OPTIONS", "_JAVA_OPTIONS")}
ENV["MONO_JDK_HOME"] = JDK
LIBS = os.path.expanduser("~/.cache/monolith/mc/libraries")
fails = 0


def check(name, ok, detail=""):
    global fails
    print(("PASS " if ok else "FAIL ") + name + ("" if ok else "\n" + detail))
    fails += 0 if ok else 1


def jar(pattern):
    hits = sorted(glob.glob(os.path.join(LIBS, "**", pattern), recursive=True))
    if not hits:
        sys.exit(f"missing library {pattern} under {LIBS}")
    return hits[-1]


def main() -> int:
    jars = [jar("text2speech-*.jar"), jar("jna-5*.jar"), jar("slf4j-api-*.jar"), jar("guava-*-jre.jar")]
    with tempfile.TemporaryDirectory() as work:
        cdir = os.path.join(work, "classes")
        os.makedirs(cdir)
        cp = os.pathsep.join(jars)
        r = subprocess.run([f"{JDK}/bin/javac", "-cp", cp, "-d", cdir, os.path.join(HERE, "tests", "narrator", "NarratorProbe.java")],
                           capture_output=True, text=True, env=ENV)
        if r.returncode:
            sys.exit("javac failed:\n" + r.stderr[-800:])
        jvm = subprocess.run([f"{JDK}/bin/java", "-cp", cdir + os.pathsep + cp, "NarratorProbe"], capture_output=True, text=True,
                             env=ENV, timeout=120)
        want = [ln for ln in jvm.stdout.splitlines() if ln.startswith("narrator:")]
        out = os.path.join(work, "out")
        b = subprocess.run([sys.executable, "-m", "monolith.rustgen", "--cp", cdir + os.pathsep + cp, "--main", "NarratorProbe", "-o", out,
                            "--shards", "4", "--shard-jobs", "2", "--opt", "1", "--compile"], cwd=ROOT, capture_output=True, text=True, env=ENV)
        if b.returncode:
            check("build", False, b.stderr[-3000:])
            return 1
        exe = os.path.join(out, "prog")
        counts = os.path.join(work, "natives.txt")
        run = subprocess.run([exe], capture_output=True, text=True, env=dict(ENV, MONO_COUNT_NATIVES=counts), timeout=120, cwd=out)
        got = [ln for ln in run.stdout.splitlines() if ln.startswith("narrator:")]
        check(f"same narrator as HotSpot ({want})", got == want and run.returncode == jvm.returncode,
              f"jvm {want} rc={jvm.returncode}\nmono {got} rc={run.returncode}\n{run.stderr[-1500:]}")
        table = open(counts).read() if os.path.exists(counts) else ""
        bad = [ln for ln in table.splitlines() if "com_sun_jna_Native" in ln and any(k in ln for k in ("registerMethod", "findSymbol", "__open"))]
        check("no JNA registration calls", not bad, "\n".join(bad) or table[-1500:])
        print(table.strip()[-1200:])
    print("ALL PASSED" if not fails else f"{fails} FAILED")
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
