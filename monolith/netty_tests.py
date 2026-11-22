"""Netty native transport switch: the runtime default `io.netty.transport.noNative=true` (rt.rs user_props) makes
Netty leave epoll for NIO, so the Rust runtime and HotSpot agree on `Epoll.isAvailable()`.

Needs the Minecraft libraries (Netty jars) in the monolith cache, or MONO_NETTY_DIR pointing at a directory of jars.
Run: MONO_JDK_HOME=<jdk 25> python3 -m monolith.netty_tests
"""
import glob, os, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
JDK = os.environ.get("MONO_JDK_HOME") or sys.exit("set MONO_JDK_HOME to a JDK 25 home")
ENV = dict(os.environ)
NETTY_DIR = os.environ.get("MONO_NETTY_DIR", "/root/.cache/monolith/mc/libraries/io/netty")
jars = sorted(glob.glob(os.path.join(NETTY_DIR, "**", "*.jar"), recursive=True))
if not jars:
    sys.exit(f"no Netty jars under {NETTY_DIR}")
fails = 0


def check(name, ok, detail=""):
    global fails
    fails += 0 if ok else 1
    print(("PASS " if ok else "FAIL ") + name + ("" if ok or not detail else f"\n{detail}"))


with tempfile.TemporaryDirectory() as work:
    cdir = os.path.join(work, "classes"); os.makedirs(cdir)
    cp = os.pathsep.join([cdir] + jars)
    src = os.path.join(HERE, "tests", "crc", "NettyProbe.java")
    r = subprocess.run([f"{JDK}/bin/javac", "-cp", cp, "-d", cdir, src], capture_output=True, text=True, env=ENV)
    if r.returncode:
        sys.exit("javac failed:\n" + r.stderr[-800:])
    ref_on = subprocess.run([f"{JDK}/bin/java", "-Dio.netty.transport.noNative=true", "-cp", cp, "NettyProbe"],
                            capture_output=True, text=True, env=ENV)
    ref_default = subprocess.run([f"{JDK}/bin/java", "-cp", cp, "NettyProbe"], capture_output=True, text=True, env=ENV)
    check("HotSpot control: without the flag Netty finds epoll (the probe can tell the two apart)",
          "epoll true" in ref_default.stdout, ref_default.stdout + ref_default.stderr[-400:])
    out = os.path.join(work, "out")
    g = subprocess.run([sys.executable, "-m", "monolith.rustgen", "--cp", cp, "--main", "NettyProbe", "-o", out,
                        "--shards", "4", "--shard-jobs", "2", "--opt", "1", "--compile"],
                       cwd=ROOT, capture_output=True, text=True, env=ENV)
    if g.returncode:
        sys.exit("build failed:\n" + g.stderr[-1500:])
    rust = subprocess.run([os.path.join(out, "prog")], capture_output=True, text=True, env=ENV)
    check("Rust runtime (default) == HotSpot with noNative=true",
          rust.stdout == ref_on.stdout and rust.returncode == ref_on.returncode,
          f"--- hotspot\n{ref_on.stdout}\n--- rust\n{rust.stdout}\n{rust.stderr[-600:]}")
print("FAILED" if fails else "ALL PASSED")
sys.exit(1 if fails else 0)
