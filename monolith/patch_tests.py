#!/usr/bin/env python3
"""Tests of the Rust patch system (monolith/patches.py, patches/README.md).

1. A program with patched methods prints exactly what HotSpot prints.
2. The same with MONO_PATCH_VERIFY=1 (each primitive-returning patch is checked against the translated original).
3. The patches are really applied: a deliberately wrong patch changes the output, and MONO_PATCH_VERIFY=1 aborts on it.
4. Version drift is loud: a patch for a method that does not exist stops the build.
Usage: python3 monolith/patch_tests.py
"""
import json, os, shutil, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__)); ROOT = os.path.dirname(HERE)
JDK = os.environ.get("MONO_JDK_HOME", "/opt/tools/jdk25")
ENV = {k: v for k, v in os.environ.items() if k not in ("JAVA_TOOL_OPTIONS", "_JAVA_OPTIONS")}
ENV["MONO_JDK_HOME"] = JDK
TD = os.path.join(HERE, "tests", "patch")
fails = 0


def check(name, ok, detail=""):
    global fails
    print(("PASS " if ok else "FAIL ") + name + ("" if ok else "\n" + detail))
    fails += 0 if ok else 1


def build(work, manifest, cdir, verify=False, run=True):
    out = os.path.join(work, "out_" + os.path.basename(manifest).replace(".", "_") + ("_v" if verify else ""))
    env = dict(ENV, MONO_PATCH_MANIFEST=manifest)
    if verify:
        env["MONO_PATCH_VERIFY"] = "1"
    return subprocess.run([sys.executable, "-m", "monolith.rustgen", "--cp", cdir, "--main", "Patch1", "-o", out, "--shards", "4",
                           "--shard-jobs", "2", "--opt", "1"] + (["--run"] if run else ["--compile"]),
                          cwd=ROOT, capture_output=True, text=True, env=env)


with tempfile.TemporaryDirectory() as work:
    cdir = os.path.join(work, "classes"); os.makedirs(cdir)
    r = subprocess.run([f"{JDK}/bin/javac", "-d", cdir, os.path.join(TD, "Patch1.java")], capture_output=True, text=True, env=ENV)
    if r.returncode:
        sys.exit("javac failed:\n" + r.stderr[-800:])
    java = subprocess.run([f"{JDK}/bin/java", "-cp", cdir, "Patch1"], capture_output=True, text=True, env=ENV)

    good = os.path.join(TD, "manifest.json")
    g = build(work, good, cdir)
    check("patched program == HotSpot", g.stdout == java.stdout and g.returncode == java.returncode,
          f"rust exit {g.returncode}\n{g.stderr[-1500:]}\n--- java\n{java.stdout}\n--- rust\n{g.stdout}")
    check("report: all 11 patches applied", "patches: 11 applied, 0 unused" in g.stderr, g.stderr[-600:])

    v = build(work, good, cdir, verify=True)
    check("MONO_PATCH_VERIFY=1 passes and still == HotSpot", v.stdout == java.stdout and v.returncode == 0,
          f"exit {v.returncode}\n{v.stderr[-1500:]}")

    # yanlış bir yama: sum() bir fazla döndürür
    bad_dir = os.path.join(work, "bad"); os.makedirs(bad_dir)
    shutil.copy(os.path.join(TD, "test_patch.rs"), os.path.join(bad_dir, "test_patch.rs"))
    src = open(os.path.join(bad_dir, "test_patch.rs")).read().replace("    Ok(s)\n}\n\npub fn nan", "    Ok(s + 1)\n}\n\npub fn nan", 1)
    assert "s + 1" in src
    open(os.path.join(bad_dir, "test_patch.rs"), "w").write(src)
    shutil.copy(good, os.path.join(bad_dir, "manifest.json"))
    bm = os.path.join(bad_dir, "manifest.json")
    b = build(work, bm, cdir)
    check("a wrong patch changes the output (the patch is really applied)", b.stdout != java.stdout and "sum 333328333350001" in b.stdout,
          b.stdout[-400:] + b.stderr[-600:])
    bv = build(work, bm, cdir, verify=True)
    check("MONO_PATCH_VERIFY=1 aborts on the wrong patch", bv.returncode != 0 and "patch mismatch in Patch1$PatchTarget.sum(J)J" in bv.stderr,
          f"exit {bv.returncode}\n{bv.stderr[-800:]}")

    # sapma: sınıf var, metot yok
    dm = json.load(open(good))
    dm["patches"][0]["method"] = "mixx"
    drift = os.path.join(work, "drift.json")
    json.dump(dm, open(drift, "w"))
    shutil.copy(os.path.join(TD, "test_patch.rs"), os.path.join(work, "test_patch.rs"))
    d = build(work, drift, cdir, run=False)
    check("a patch for a missing method stops the build", d.returncode != 0 and "no such method" in (d.stderr + d.stdout),
          f"exit {d.returncode}\n{d.stderr[-600:]}")

print("FAILED" if fails else "ALL PASSED")
sys.exit(1 if fails else 0)
