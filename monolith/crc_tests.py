"""CRC32 / Adler32 through the Rust runtime (natives_io.rs, crc32fast) against HotSpot.

Run: MONO_JDK_HOME=<jdk 25> python3 -m monolith.crc_tests
"""
import os, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
JDK = os.environ.get("MONO_JDK_HOME") or sys.exit("set MONO_JDK_HOME to a JDK 25 home")
ENV = dict(os.environ)
fails = 0


def check(name, ok, detail=""):
    global fails
    fails += 0 if ok else 1
    print(("PASS " if ok else "FAIL ") + name + ("" if ok or not detail else f"\n{detail}"))


with tempfile.TemporaryDirectory() as work:
    cdir = os.path.join(work, "classes"); os.makedirs(cdir)
    r = subprocess.run([f"{JDK}/bin/javac", "-d", cdir, os.path.join(HERE, "tests", "crc", "CrcTest.java")],
                       capture_output=True, text=True, env=ENV)
    if r.returncode:
        sys.exit("javac failed:\n" + r.stderr[-800:])
    java = subprocess.run([f"{JDK}/bin/java", "-cp", cdir, "CrcTest"], capture_output=True, text=True, env=ENV)
    check("HotSpot reference reproduces the known check values",
          "check cbf43926" in java.stdout and "adler 11e60398" in java.stdout, java.stdout)
    out = os.path.join(work, "out")
    g = subprocess.run([sys.executable, "-m", "monolith.rustgen", "--cp", cdir, "--main", "CrcTest", "-o", out,
                        "--shards", "2", "--shard-jobs", "2", "--opt", "1", "--compile"],
                       cwd=ROOT, capture_output=True, text=True, env=ENV)
    if g.returncode:
        sys.exit("build failed:\n" + g.stderr[-1500:])
    rust = subprocess.run([os.path.join(out, "prog")], capture_output=True, text=True, env=ENV)
    check("Rust runtime output == HotSpot", rust.stdout == java.stdout and rust.returncode == java.returncode,
          f"--- java\n{java.stdout}\n--- rust\n{rust.stdout}\n{rust.stderr[-600:]}")
print("FAILED" if fails else "ALL PASSED")
sys.exit(1 if fails else 0)
