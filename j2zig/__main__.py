"""j2zig command line.

  python -m j2zig build --cp app.jar[:lib.jar...] --main pkg.Main -o out/ [-O ReleaseFast]
  python -m j2zig analyze app.jar [lib.jar ...]

`build` transpiles the reachable part of the application + class library into Zig
sources under out/src, writes out/report.json / out/report.txt, and (unless
--no-compile) compiles a native executable out/<name> with `zig build-exe`.
"""
from __future__ import annotations

import argparse
import glob
import json
import os
import platform
import shutil
import subprocess
import sys
import time

from .classpath import ClassPath, builtin_classes, builtin_nolwjgl_classes, jdk_home, jdk_jmods, jdk_tool
from .program import Program, Report
from .world import World
from .zigen import ZigGen, scan_natives

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CLASSLIB_SRC = os.path.join(ROOT, "classlib", "src")
CLASSLIB_BUILD = os.path.join(ROOT, "classlib", "build")
RUNTIME = os.path.join(ROOT, "runtime")


def classlib_out() -> str:
    """Overlay classes are compiled per JDK (the JDK-specific JavaLangAccess is generated)."""
    import hashlib
    h = hashlib.sha1(os.path.realpath(jdk_home()).encode()).hexdigest()[:12]
    return os.path.join(CLASSLIB_BUILD, h)


def build_classlib(force: bool = False) -> str:
    out = classlib_out()
    os.makedirs(CLASSLIB_BUILD, exist_ok=True)
    with open(out + ".lock", "w") as lock:
        # Advisory lock only: the output is content-addressed (immutable, keyed by sources), so
        # losing the lock is safe (worst case: duplicate javac work). fcntl is Unix-only; on
        # Windows use msvcrt, and where neither exists, proceed.
        try:
            import fcntl
            fcntl.flock(lock, fcntl.LOCK_EX)
        except ImportError:
            try:
                import msvcrt
                msvcrt.locking(lock.fileno(), msvcrt.LK_LOCK, 1)
            except (ImportError, OSError):
                pass
        return _build_classlib(out, force)


def _build_classlib(out: str, force: bool) -> str:
    import hashlib
    gen = os.path.join(out, "gen")
    # JavaLangAccess differs between JDK releases: generate it for the selected JDK
    jla = os.path.join(gen, "java", "lang", "J2zJavaLangAccess.java")
    subprocess.run([sys.executable, os.path.join(ROOT, "tools", "gen_jla.py"), jla], check=True)
    srcs = sorted(glob.glob(os.path.join(CLASSLIB_SRC, "**", "*.java"), recursive=True)) + [jla]
    # Output directories are immutable and keyed by the sources, so a transpile that is reading
    # one is never affected by a concurrent rebuild.
    h = hashlib.sha1()
    for x in srcs:
        h.update(x.encode())
        with open(x, "rb") as fh:
            h.update(fh.read())
    classes = os.path.join(out, "classes-" + h.hexdigest()[:12])
    stamp = os.path.join(classes, ".stamp")
    if not force and os.path.exists(stamp):
        return classes
    tmp = classes + f".tmp{os.getpid()}"
    if os.path.isdir(tmp):
        shutil.rmtree(tmp)
    os.makedirs(tmp)
    cmd = [jdk_tool("javac"), "-J-Xmx192m", "-J-XX:+UseSerialGC",
           "-nowarn", "-XDstringConcat=inline", "--patch-module",
           f"java.base={CLASSLIB_SRC}{os.pathsep}{gen}", "-d", tmp] + srcs
    r = subprocess.run(cmd, capture_output=True, text=True)
    if r.returncode != 0:
        sys.stderr.write(r.stdout + r.stderr)
        shutil.rmtree(tmp, ignore_errors=True)
        raise SystemExit("classlib compilation failed")
    # javac --patch-module puts classes under <out>/java.base/...
    base = os.path.join(tmp, "java.base")
    if os.path.isdir(base):
        for item in os.listdir(base):
            shutil.move(os.path.join(base, item), os.path.join(tmp, item))
        os.rmdir(base)
    with open(os.path.join(tmp, ".stamp"), "w") as fh:
        fh.write(jdk_home())
    if os.path.isdir(classes):
        shutil.rmtree(classes)
    os.rename(tmp, classes)
    return classes


def cmd_build(a) -> int:
    if a.compile_only:
        return compile_native(a)
    if not a.no_compile and not os.environ.get("J2Z_TRANSPILE_ONLY"):
        # Transpile in a child process so its whole-program model (several GB for large
        # programs) is released before the Zig compiler, which needs most of the memory, runs.
        argv = [sys.executable, "-m", "j2zig"] + sys.argv[1:]
        env = dict(os.environ, J2Z_TRANSPILE_ONLY="1")
        r = subprocess.run(argv, env=env, cwd=os.getcwd())
        if r.returncode != 0:
            return r.returncode
        return compile_native(a)
    t0 = time.time()
    classlib = build_classlib(a.rebuild_classlib)
    builtin = builtin_classes()  # the j2z.rhi native bridge (NativeRhi); None when disabled/absent
    nolwjgl = builtin_nolwjgl_classes() if getattr(a, "no_lwjgl", False) else None
    app_entries = [p for p in a.cp.split(os.pathsep) if p]
    pre = [x for x in (builtin, nolwjgl) if x]
    entries = pre + app_entries + [classlib] + (a.jdk or jdk_jmods())
    overrides = {}
    for spec in a.multi_release:
        prefix, _, r = spec.partition("=")
        overrides[prefix] = int(r)
    napp = len(pre) + len(app_entries)
    cp = ClassPath(entries, release=a.release, release_overrides=overrides, boot_start=napp)
    report = Report()
    world = World(cp)
    prog = Program(world, report)
    prog.reflect_prefixes = [x.replace(".", "/") for x in a.reflect]
    for d in a.native_libs:
        if os.path.isdir(d):
            for root, _, files in os.walk(d):
                prog.native_libs += [os.path.join(root, f) for f in sorted(files)
                                     if f.endswith(".so") or ".so." in f]
        elif os.path.isfile(d):
            prog.native_libs.append(d)
    for spec in a.reflect_class:
        names = open(spec[1:]).read().split() if spec.startswith("@") else [spec]
        prog.reflect_classes.extend(n.replace(".", "/") for n in names)
    main = a.main.replace(".", "/")
    prog.run(main)
    t1 = time.time()
    esc_path = os.environ.get("J2Z_ESCAPE_REPORT")
    if esc_path:
        # whole-program escape analysis (statistics for compile-time memory decisions); J2Z_ESCAPE_ONLY=1 stops after it
        from . import escape
        t_esc = time.time()
        esc_result = escape.analyze(prog)
        escape.write_report(esc_result, esc_path)
        print(f"escape analysis: {time.time() - t_esc:.1f}s, report {esc_path}", file=sys.stderr)
        if os.environ.get("J2Z_ESCAPE_ONLY"):
            return 0
    out = a.output
    src = os.path.join(out, "src")
    if os.path.isdir(src):
        shutil.rmtree(src)
    os.makedirs(src)
    # java.home == directory of the executable: ship the time-zone database and the JDK's conf/
    # (java.security, net.properties, ...) next to it
    tz = os.path.join(jdk_home(), "lib", "tzdb.dat")
    if os.path.exists(tz):
        os.makedirs(os.path.join(out, "lib"), exist_ok=True)
        shutil.copy(tz, os.path.join(out, "lib", "tzdb.dat"))
    conf = os.path.join(jdk_home(), "conf")
    if os.path.isdir(conf):
        shutil.copytree(conf, os.path.join(out, "conf"), dirs_exist_ok=True)
    natives_files = sorted(glob.glob(os.path.join(RUNTIME, "*.zig")))
    natives = scan_natives(natives_files)
    nshards = a.shards
    if nshards == 0 and not a.no_compile and _mem_avail_gib() < 4.5 and len(prog.loaded) > 450:
        nshards = max(4, (len(prog.loaded) + 249) // 250)
    gen = ZigGen(prog, src, natives, nshards)
    gen.generate(main)
    if gen.patches:
        for cls, m, d in sorted(gen.patches):
            if (cls, m, d) not in gen.patches_used:
                report("unused-patch", f"{cls}.{m}{d}: patch matched no loaded class "
                                       f"(wrong mapping namespace, or the class is not reachable from {a.main})")
    if not nshards:
        for f in natives_files:
            shutil.copy(f, src)
        for f in glob.glob(os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "patches", "*.zig")):
            shutil.copy(f, src)
        with open(os.path.join(src, "shardmode.zig"), "w") as fh:
            fh.write("pub const sharded = false;\n")
    t2 = time.time()
    n_methods = len(prog.reachable)
    summary = {
        "main": a.main,
        "classes_loaded": len(prog.loaded),
        "classes_instantiated": len(prog.instantiated),
        "methods_reachable": n_methods,
        "native_methods_reachable": len(prog.natives),
        "builtin_rhi": bool(builtin) if 'builtin' in dir() else None,
        "seconds_analysis": round(t1 - t0, 2),
        "seconds_codegen": round(t2 - t1, 2),
        "issues": {k: sorted(set(v)) for k, v in report.items.items()},
    }
    with open(os.path.join(out, "report.json"), "w") as fh:
        json.dump(summary, fh, indent=1)
    print(f"j2zig: {summary['classes_loaded']} classes, {n_methods} methods "
          f"({summary['native_methods_reachable']} native) in {t2 - t0:.1f}s")
    for k, v in summary["issues"].items():
        print(f"  {k}: {len(v)}")
        for x in v[: a.show_issues]:
            print(f"     {x}")
    return 0


def _zig_preexec():
    """Optional hard ceiling (J2Z_ZIG_AS_GIB, e.g. '1') on the *address space* of every zig
    invocation (build-exe / build-obj / cc), plus CPU affinity cap on mobile/small-RAM hosts
    so zig's thread pool does not spawn 10 concurrent compiler threads and trigger Android LMKD."""
    if os.name != "posix":
        return None
    cap = float(os.environ.get("J2Z_ZIG_AS_GIB", "0"))
    low_mem = _mem_total_gib() < 10.0
    if cap <= 0 and not low_mem:
        return None
    import resource
    lim = int(cap * (1024 ** 3)) if cap > 0 else 0
    def _apply():
        if low_mem:
            try:
                os.nice(10)
            except OSError:
                pass
            try:
                if hasattr(os, "sched_setaffinity") and (os.cpu_count() or 1) > 2:
                    os.sched_setaffinity(0, {0, 1})
            except OSError:
                pass
        if lim > 0:
            resource.setrlimit(resource.RLIMIT_AS, (lim, lim))
    return _apply


def _mem_total_gib() -> float:
    try:
        with open("/proc/meminfo") as f:
            for line in f:
                if line.startswith("MemTotal:"):
                    return int(line.split()[1]) / (1024 * 1024)
    except OSError:
        pass
    return 1e9  # unknown platform: assume enough


def _mem_avail_gib() -> float:
    try:
        with open("/proc/meminfo") as f:
            for line in f:
                if line.startswith("MemAvailable:"):
                    return int(line.split()[1]) / (1024 * 1024)
    except OSError:
        pass
    return _mem_total_gib()


# LLVM optimizing the whole 240k-method Minecraft closure as one module needs well over 15 GiB;
# below this much RAM the self-hosted backend (no LLVM, same safety setting, far less memory)
# is used instead. Override with --backend llvm|native.
LLVM_RELEASE_GIB_PER_10K_FILES = 4.0


TERMUX_USR = "/data/data/com.termux/files/usr"


def _resolve_target(a) -> str:
    t = getattr(a, "target", None) or ""
    if t == "android":
        return "aarch64-linux-android.26"
    if not t and getattr(a, "dynamic", False) and os.path.isdir(os.path.join(TERMUX_USR, "include")):
        return "aarch64-linux-android.26"
    return t


def _is_android_target(target: str) -> bool:
    return target.startswith("aarch64-linux-android")


def _find_android_ndk_prebuilt() -> str | None:
    """Locate the Android NDK LLVM prebuilt root (supports ANDROID_NDK_HOME, ANDROID_NDK_LATEST_HOME, and SDK ndk/)."""
    candidates: list[str] = []
    for var in ("ANDROID_NDK_HOME", "ANDROID_NDK_LATEST_HOME", "ANDROID_NDK_ROOT"):
        val = os.environ.get(var, "")
        if val:
            candidates.append(val)
    sdk_roots = [os.environ.get("ANDROID_HOME", ""), os.environ.get("ANDROID_SDK_ROOT", ""), "/usr/local/lib/android/sdk"]
    for sdk in sdk_roots:
        if sdk and os.path.isdir(os.path.join(sdk, "ndk")):
            for d in sorted(os.listdir(os.path.join(sdk, "ndk")), reverse=True):
                candidates.append(os.path.join(sdk, "ndk", d))
    for ndk in candidates:
        pre = os.path.join(ndk, "toolchains", "llvm", "prebuilt")
        if not os.path.isdir(pre):
            continue
        for tag in ("linux-x86_64", "linux-aarch64", "darwin-arm64", "darwin-x86_64"):
            p = os.path.join(pre, tag)
            if os.path.isdir(os.path.join(p, "sysroot", "usr", "include")):
                return p
    return None


def _android_sysroot() -> tuple[str, str, str, str, list[str]]:
    """Return (inc_dir, arch_inc_dir, crt_dir, lib_search_dir, builtins_libs)."""
    termux_inc = os.path.join(TERMUX_USR, "include")
    if os.path.isdir(termux_inc):
        arch_inc = os.path.join(termux_inc, "aarch64-linux-android")
        builtins = sorted(glob.glob(os.path.join(TERMUX_USR, "lib", "clang", "*", "lib", "linux", "libclang_rt.builtins-aarch64-android.a")))
        return (termux_inc, arch_inc if os.path.isdir(arch_inc) else "", os.path.join(TERMUX_USR, "lib"), "/system/lib64", builtins[:1])
    pre = _find_android_ndk_prebuilt()
    if pre:
        usr = os.path.join(pre, "sysroot", "usr")
        inc = os.path.join(usr, "include")
        arch_inc = os.path.join(inc, "aarch64-linux-android")
        crt = os.path.join(usr, "lib", "aarch64-linux-android", "26")
        if not os.path.isdir(crt):
            crt = os.path.join(usr, "lib", "aarch64-linux-android")
        builtins = sorted(glob.glob(os.path.join(pre, "lib", "clang", "*", "lib", "linux", "libclang_rt.builtins-aarch64-android.a")))
        return (inc, arch_inc if os.path.isdir(arch_inc) else "", crt, crt, builtins[:1])
    return (termux_inc, "", os.path.join(TERMUX_USR, "lib"), "/system/lib64", [])


def _android_include_flags(flag: str = "-I") -> list[str]:
    _, arch_inc, _, _, _ = _android_sysroot()
    return [flag, arch_inc] if arch_inc else []


def _ensure_android_libc(out: str) -> str:
    libc_path = os.path.join(out, "libc.txt")
    inc, _, crt, _, _ = _android_sysroot()
    with open(libc_path, "w") as fh:
        fh.write(f"include_dir={inc}\nsys_include_dir={inc}\ncrt_dir={crt}\n"
                 "msvc_lib_dir=\nkernel32_lib_dir=\ngcc_dir=\n")
    return libc_path


def _android_clang_bin() -> str | None:
    p = os.path.join(TERMUX_USR, "bin", "clang")
    if os.path.isfile(p) and os.access(p, os.X_OK):
        return p
    pre = _find_android_ndk_prebuilt()
    if pre:
        c = os.path.join(pre, "bin", "aarch64-linux-android26-clang")
        if os.path.isfile(c) and os.access(c, os.X_OK):
            return c
    return None


def _android_lld_bin() -> str | None:
    p = os.path.join(TERMUX_USR, "bin", "ld.lld")
    if os.path.isfile(p) and os.access(p, os.X_OK):
        return p
    pre = _find_android_ndk_prebuilt()
    if pre:
        lld = os.path.join(pre, "bin", "ld.lld")
        if os.path.isfile(lld) and os.access(lld, os.X_OK):
            return lld
    return None


def _compile_c_object(c_src: str, obj_out: str, target: str, dynamic: bool, opt_flag: str = "-O2") -> int:
    if _is_android_target(target):
        clang = _android_clang_bin()
        if clang:
            inc, arch_inc, _, _, _ = _android_sysroot()
            cmd = [clang, "--target=aarch64-linux-android26", "-c", opt_flag, "-fPIC"]
            if arch_inc:
                cmd += ["-isystem", arch_inc]
            if os.path.isdir(inc):
                cmd += ["-isystem", inc]
            cmd += [c_src, "-o", obj_out]
            r = subprocess.run(cmd, capture_output=True, text=True)
            if r.returncode != 0:
                sys.stderr.write(r.stderr)
            return r.returncode
    cmd = ["zig", "cc", "-c", opt_flag]
    if dynamic:
        cmd.append("-fPIC")
    if target:
        cmd += ["-target", target]
    cmd += [c_src, "-o", obj_out]
    r = subprocess.run(cmd, capture_output=True, text=True, preexec_fn=_zig_preexec())
    if r.returncode != 0:
        sys.stderr.write(r.stderr)
    return r.returncode


def _shared_lib_output_path(out: str, a) -> str:
    raw_name = a.name or a.main.rsplit(".", 1)[-1]
    if getattr(a, "dynamic", False):
        if raw_name.endswith(".so"):
            return os.path.join(out, raw_name)
        stem = raw_name if raw_name.startswith("lib") else ("lib" + raw_name)
        return os.path.join(out, stem + ".so")
    return os.path.join(out, raw_name)


def _link_android_shared(objs: list[str], so_out: str) -> int:
    fix = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "tools", "fix_tls_syms.py")
    if os.path.isfile(fix):
        subprocess.run([sys.executable, fix] + objs, check=True, capture_output=True, text=True)
    # On Termux (on-device) use Termux ld.lld + /system/lib64/*.so directly.
    termux_lld = os.path.join(TERMUX_USR, "bin", "ld.lld")
    if os.path.isfile(termux_lld) and os.access(termux_lld, os.X_OK):
        _, _, _, lib_dir, rt_builtins = _android_sysroot()
        sys_libs = [
            os.path.join(lib_dir, lib)
            for lib in ("libandroid.so", "liblog.so", "libc.so", "libm.so", "libdl.so", "libz.so")
        ]
        cmd = [
            termux_lld, "-shared", "--sysroot=/", "-z", "max-page-size=16384", "--gc-sections",
            "-soname", os.path.basename(so_out),
            *objs,
            *rt_builtins,
            *[s for s in sys_libs if os.path.exists(s)],
            "-o", so_out,
        ]
        r = subprocess.run(cmd, capture_output=True, text=True)
        if r.returncode != 0:
            sys.stderr.write(r.stderr[-20000:])
        return r.returncode
    # On Linux/macOS hosts use Android NDK aarch64-linux-android26-clang -fuse-ld=lld -shared.
    cc = _android_clang_bin() or os.environ.get("CC", "clang")
    cmd = [cc, "-fuse-ld=lld", "-shared", "-Wl,-z,max-page-size=16384", "-Wl,--gc-sections",
           f"-Wl,-soname,{os.path.basename(so_out)}",
           *objs, "-landroid", "-llog", "-lc", "-lz", "-lm", "-ldl", "-o", so_out]
    r = subprocess.run(cmd, capture_output=True, text=True)
    if r.returncode != 0:
        sys.stderr.write(r.stderr[-20000:])
    return r.returncode


def _shard_source_key(shard_dir: str, flags: list[str]) -> str:
    import hashlib
    h = hashlib.sha1(" ".join(flags).encode())
    for root, dirs, files in os.walk(shard_dir):
        dirs.sort()
        for f in sorted(files):
            fp = os.path.join(root, f)
            h.update(os.path.relpath(fp, shard_dir).encode())
            with open(fp, "rb") as fh:
                h.update(hashlib.sha1(fh.read()).digest())
    return h.hexdigest()


def _compile_sharded(a, nshards: int, backend: str) -> int:
    out = a.output
    src = os.path.join(out, "src")
    dynamic = bool(getattr(a, "dynamic", False))
    target = _resolve_target(a)
    android = _is_android_target(target)
    exe = _shared_lib_output_path(out, a)
    base = ["-O" + a.optimize, "-lc"]
    if not android:
        base.append("-lz")
    if dynamic or android:
        base.append("-fPIC")
    if target:
        base += ["-target", target]
    if android:
        base += ["--libc", _ensure_android_libc(out)] + _android_include_flags("-I")
    if backend == "native" and not android:
        base.append("-fno-llvm")
    if a.optimize == "Debug" or backend == "native":
        base.append("-fstrip")
    cache = ["--cache-dir", a.zig_cache, "--global-cache-dir", a.zig_cache] if a.zig_cache else []
    objs = []
    jobs = []
    stamps: dict[int, tuple[str, str]] = {}
    if _compile_c_object(os.path.join(src, "shared.c"), os.path.join(out, "shared.o"), target, dynamic, "-O1") != 0:
        return 2
    objs.append(os.path.join(out, "shared.o"))
    jni_va_c = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "runtime", "jni_va.c")
    jni_va_o = os.path.join(out, "jni_va.o")
    if _compile_c_object(jni_va_c, jni_va_o, target, dynamic, "-O2") != 0:
        return 2
    objs.append(jni_va_o)
    only = {int(x) for x in a.only_shards.split(",") if x}
    for k in range(1, nshards):
        obj = os.path.join(out, f"shard{k}.o")
        objs.append(obj)
        if only and k not in only:
            continue
        cmd_k = ["zig", "build-obj", os.path.join(src, f"shard{k}", "shard.zig")] + base + [f"-femit-bin={obj}"] + cache
        if getattr(a, "stamp_only", False):
            # record that the existing object of this shard was built from the current sources (used to adopt objects
            # of an earlier build whose stamps were lost); nothing is compiled
            if not os.path.isfile(obj):
                raise SystemExit(f"--stamp-only: {obj} does not exist")
            with open(os.path.join(out, f"shard{k}.srchash"), "w") as fh:
                fh.write(_shard_source_key(os.path.join(src, f"shard{k}"), base))
            continue
        if getattr(a, "incremental", False):
            # class-level incremental build: a shard whose generated Zig (and compile flags) are byte-identical to
            # the ones its object was built from is reused; only shards holding changed classes are recompiled
            key = _shard_source_key(os.path.join(src, f"shard{k}"), base)
            stamp = os.path.join(out, f"shard{k}.srchash")
            if os.path.isfile(obj) and os.path.isfile(stamp) and open(stamp).read() == key:
                print(f"j2zig: shard {k}/{nshards - 1} unchanged, object reused")
                continue
            if os.path.isfile(stamp):
                os.remove(stamp)
            stamps[k] = (stamp, key)
        jobs.append((k, cmd_k))
    t3 = time.time()
    running: list[tuple[int, subprocess.Popen, float]] = []
    pending = list(jobs)
    failed = False
    default_jobs = 1 if _mem_avail_gib() < 5.0 else 2
    jobs_limit = max(1, a.shard_jobs if a.shard_jobs is not None else default_jobs)
    while (pending or running) and not failed:
        while pending and len(running) < jobs_limit:
            k, cmd = pending.pop(0)
            logf = open(os.path.join(out, f"shard{k}.log"), "w")
            running.append((k, subprocess.Popen(cmd, stdout=logf, stderr=subprocess.STDOUT, text=True,
                                              preexec_fn=_zig_preexec()), time.time()))
        time.sleep(0.05)
        for item in list(running):
            k, pr, t0 = item
            if pr.poll() is None:
                continue
            running.remove(item)
            err = open(os.path.join(out, f"shard{k}.log")).read()
            if pr.returncode != 0:
                sys.stderr.write(f"shard {k}: zig failed (exit {pr.returncode})\n" + err[-6000:])
                failed = True
            else:
                print(f"j2zig: shard {k}/{nshards - 1} compiled in {time.time() - t0:.0f}s")
                if k in stamps:
                    with open(stamps[k][0], "w") as fh:
                        fh.write(stamps[k][1])
    for _, pr, _ in running:
        pr.kill()
    if failed:
        return 2
    if getattr(a, "stamp_only", False):
        print(f"j2zig: stamps written for shards {sorted(only)}")
        return 0
    if getattr(a, "no_link", False):
        print(f"j2zig: shards {sorted(only)} compiled (no link requested)")
        return 0
    if android or os.environ.get("J2Z_ZIG_LD", "cc" if _mem_avail_gib() < 4.5 else "") == "cc":
        # Link with the system C toolchain instead of the zig self-hosted linker. The self-hosted
        # linker needs more than the 1 GiB address-space ceiling J2Z_ZIG_AS_GIB enforces (it died of
        # OutOfMemory in this exact configuration), while GNU ld links the 640 MiB of objects in ~2 s.
        # zig emits cross-shard `extern threadlocal` refs as NOTYPE UND symbols, which GNU ld refuses
        # to bind to STT_TLS definitions, so retype those four symbols first.
        r0 = subprocess.run(["zig", "build-obj", os.path.join(src, "shard0", "main.zig")] + base +
                            [f"-femit-bin={os.path.join(out, 'shard0.o')}"] + cache,
                            capture_output=True, text=True, preexec_fn=_zig_preexec())
        if r0.returncode != 0:
            sys.stderr.write(r0.stderr[-20000:])
            print(f"j2zig: zig build-obj of shard 0 failed (exit {r0.returncode})", file=sys.stderr)
            return 2
        objs.append(os.path.join(out, "shard0.o"))
        if android and dynamic:
            if _link_android_shared(objs, exe) != 0:
                if os.path.exists(exe):
                    os.remove(exe)
                return 2
            print(f"j2zig: Android shared library {exe} ({os.path.getsize(exe) // 1024} KiB, {nshards} shards, zig {time.time() - t3:.1f}s)")
            return 0
        fix = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "tools", "fix_tls_syms.py")
        subprocess.run([sys.executable, fix] + objs, check=True, capture_output=True, text=True)
        cflags = ["-O1"]
        if dynamic:
            cflags.append("-shared")
        r = subprocess.run([os.environ.get("CC", "cc")] + cflags + objs + ["-o", exe, "-lc", "-lz", "-lm", "-ldl", "-lpthread"],
                          capture_output=True, text=True)
        if r.returncode != 0:
            sys.stderr.write(r.stderr[-20000:])
            print(f"j2zig: cc link failed (exit {r.returncode})", file=sys.stderr)
            if os.path.exists(exe):
                os.remove(exe)
            return 2
        kind_lbl = "shared library" if dynamic else "native executable"
        print(f"j2zig: {kind_lbl} {exe} ({os.path.getsize(exe) // 1024} KiB, {nshards} shards, "
              f"{backend} backend, zig {time.time() - t3:.1f}s, linked with {os.environ.get('CC', 'cc')})")
        return 0
    zig_verb = "build-lib" if dynamic else "build-exe"
    cmd = ["zig", zig_verb, os.path.join(src, "shard0", "main.zig")] + objs + base + [f"-femit-bin={exe}",
           "--name", os.path.basename(exe)]
    if dynamic:
        cmd.append("-dynamic")
    cmd += cache
    r = subprocess.run(cmd, capture_output=True, text=True, preexec_fn=_zig_preexec())
    if r.returncode != 0:
        sys.stderr.write(r.stderr[-20000:])
        print(f"j2zig: zig link/compile of shard 0 failed (exit {r.returncode})", file=sys.stderr)
        if os.path.exists(exe):
            os.remove(exe)
        return 2
    kind_lbl = "shared library" if dynamic else "native executable"
    print(f"j2zig: {kind_lbl} {exe} ({os.path.getsize(exe) // 1024} KiB, {nshards} shards, "
          f"{backend} backend, zig {time.time() - t3:.1f}s)")
    return 0


def compile_native(a) -> int:
    out = a.output
    src = os.path.join(out, "src")
    dynamic = bool(getattr(a, "dynamic", False))
    target = _resolve_target(a)
    android = _is_android_target(target)
    exe = _shared_lib_output_path(out, a)
    backend = getattr(a, "backend", "auto")
    sh = os.path.join(src, "shards.txt")
    if os.path.exists(sh):
        if backend == "auto":
            has_selfhosted = platform.machine() in ("x86_64", "AMD64")
            nfiles = len(os.listdir(src)) if os.path.isdir(src) else 0
            need = LLVM_RELEASE_GIB_PER_10K_FILES * nfiles / 10000 * 1.6
            if has_selfhosted and (a.optimize in ("Debug", "ReleaseSafe") or _mem_total_gib() < need):
                if a.optimize not in ("Debug", "ReleaseSafe"):
                    print(f"j2zig: {nfiles} files need ~{need:.0f} GiB for the sharded LLVM {a.optimize} build, "
                          f"{_mem_total_gib():.0f} GiB available: using the self-hosted backend instead")
                backend = "native"
            else:
                backend = "llvm"
        return _compile_sharded(a, int(open(sh).read()), backend)
    nfiles = len(os.listdir(src))
    has_selfhosted = platform.machine() in ("x86_64", "AMD64")
    if backend == "auto":
        need = LLVM_RELEASE_GIB_PER_10K_FILES * nfiles / 10000 * 1.6
        if has_selfhosted and (a.optimize == "Debug" or _mem_total_gib() < need):
            backend = "native"
            print(f"j2zig: {nfiles} files need ~{need:.0f} GiB for the LLVM {a.optimize} build, "
                  f"{_mem_total_gib():.0f} GiB available: using the self-hosted backend (--backend llvm to force, "
                  f"--shards N to split the build)")
        else:
            backend = "llvm"
    jni_va_c = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "runtime", "jni_va.c")
    if android and dynamic:
        t3 = time.time()
        libc_txt = _ensure_android_libc(out)
        main_o = os.path.join(out, "main.o")
        jni_va_o = os.path.join(out, "jni_va.o")
        if _compile_c_object(jni_va_c, jni_va_o, target, True, "-O2") != 0:
            return 2
        cmd = ["zig", "build-obj", os.path.join(src, "main.zig"), f"-O{a.optimize}",
               "-fPIC", "-lc", "--libc", libc_txt, *_android_include_flags("-I"),
               "-target", target, f"-femit-bin={main_o}"]
        if a.optimize == "Debug":
            cmd.append("-fstrip")
        if a.zig_cache:
            cmd += ["--cache-dir", a.zig_cache, "--global-cache-dir", a.zig_cache]
        r = subprocess.run(cmd, capture_output=True, text=True, preexec_fn=_zig_preexec())
        if r.returncode != 0:
            sys.stderr.write(r.stderr[-20000:])
            return 2
        if _link_android_shared([main_o, jni_va_o], exe) != 0:
            if os.path.exists(exe):
                os.remove(exe)
            return 2
        print(f"j2zig: Android shared library {exe} ({os.path.getsize(exe) // 1024} KiB, zig {time.time() - t3:.1f}s)")
        return 0

    while True:
        zig_verb = "build-lib" if dynamic else "build-exe"
        cmd = ["zig", zig_verb, os.path.join(src, "main.zig"), jni_va_c, f"-O{a.optimize}",
               f"-femit-bin={exe}", "--name", os.path.basename(exe), "-lc", "-lz"]
        if dynamic:
            cmd.append("-dynamic")
        if target:
            cmd += ["-target", target]
        if backend == "native":
            cmd.append("-fno-llvm")
        if a.optimize == "Debug" or backend == "native":
            cmd.append("-fstrip")
        if a.zig_cache:
            cmd += ["--cache-dir", a.zig_cache, "--global-cache-dir", a.zig_cache]
        t3 = time.time()
        r = subprocess.run(cmd, capture_output=True, text=True, preexec_fn=_zig_preexec())
        if r.returncode == -9 and backend == "llvm" and has_selfhosted and getattr(a, "backend", "auto") == "auto":
            print("j2zig: LLVM build was killed (out of memory); retrying with the self-hosted backend",
                  file=sys.stderr)
            backend = "native"
            continue
        break
    if r.returncode != 0:
        sys.stderr.write(r.stderr[-20000:])
        print(f"j2zig: zig compilation failed (exit {r.returncode}"
              f"{', killed by signal ' + str(-r.returncode) if r.returncode < 0 else ''})", file=sys.stderr)
        if os.path.exists(exe):
            os.remove(exe)
        return 2
    kind_lbl = "shared library" if dynamic else "native executable"
    print(f"j2zig: {kind_lbl} {exe} ({os.path.getsize(exe) // 1024} KiB, "
          f"{backend} backend, zig {time.time() - t3:.1f}s)")
    return 0


def _add_build_args(p: argparse.ArgumentParser, default_dynamic: bool = False) -> None:
    p.add_argument("--cp", required=True, help="application class path (jars / dirs)")
    p.add_argument("--main", required=True)
    p.add_argument("-o", "--output", required=True)
    p.add_argument("--name")
    p.add_argument("--jdk", nargs="*", help="JDK module files (default: java.base.jmod)")
    p.add_argument("--release", type=int, default=25)
    p.add_argument("--multi-release", action="append", default=[], metavar="JARPREFIX=N",
                   help="multi-release version for JARs whose name starts with JARPREFIX (repeatable)")
    p.add_argument("-O", "--optimize", default="ReleaseSafe",
                   choices=["Debug", "ReleaseSafe", "ReleaseFast", "ReleaseSmall"])
    p.add_argument("--shards", type=int, default=0, metavar="N",
                   help="split the program into N separately compiled Zig objects (LLVM release builds in less memory)")
    p.add_argument("--compile-only", action="store_true", help="skip the transpile, compile the existing output")
    p.add_argument("--stamp-only", action="store_true", help="with --compile-only --only-shards: write the source stamps of those shards' existing objects, compile nothing")
    p.add_argument("--incremental", action="store_true", help="sharded build: reuse the object of every shard whose generated Zig did not change")
    p.add_argument("--no-link", action="store_true", help="with --only-shards: compile those shards, skip the final link")
    p.add_argument("--only-shards", default="", help="comma separated shard numbers to (re)compile; the other objects are reused")
    p.add_argument("--shard-jobs", type=int, default=None, help="shards compiled in parallel (default: 1 if <5 GiB free RAM, else 2)")
    p.add_argument("--backend", default="auto", choices=["auto", "llvm", "native"],
                   help="Zig code generator: llvm (optimizing, memory hungry), native (self-hosted), auto")
    p.add_argument("--reflect", action="append", default=[], metavar="PREFIX",
                   help="keep all methods/constructors of loaded classes with this name prefix for reflection")
    p.add_argument("--reflect-class", action="append", default=[], metavar="NAME|@FILE",
                   help="class whose methods/constructors/fields are used reflectively")
    p.add_argument("--native-libs", action="append", default=[], metavar="DIR|FILE",
                   help="native libraries (or directories of them) that call back into Java by name")
    p.add_argument("--no-lwjgl", action="store_true",
                   help="include Zero-LWJGL RHI Java shim (org.lwjgl -> NativeRhi) when lwjgl jars are stripped")
    p.add_argument("--dynamic", action="store_true", default=default_dynamic,
                   help="emit shared library (.so via zig build-lib -dynamic / ld.lld)")
    p.add_argument("--target", default="", help="Zig target triple (e.g. aarch64-linux-android.26 or 'android')")
    p.add_argument("--no-compile", action="store_true")
    p.add_argument("--rebuild-classlib", action="store_true")
    p.add_argument("--show-issues", type=int, default=10)
    p.add_argument("--zig-cache")


def main(argv=None) -> int:
    raw = list(sys.argv[1:] if argv is None else argv)
    norm = ["--dynamic" if x == "-dynamic" else x for x in raw]
    ap = argparse.ArgumentParser(prog="j2zig")
    sub = ap.add_subparsers(dest="cmd", required=True)
    b = sub.add_parser("build")
    _add_build_args(b, default_dynamic=False)
    bl = sub.add_parser("build-lib")
    _add_build_args(bl, default_dynamic=True)
    an = sub.add_parser("analyze")
    an.add_argument("rest", nargs=argparse.REMAINDER)
    a = ap.parse_args(norm)
    if a.cmd in ("build", "build-lib"):
        return cmd_build(a)
    from .analyze import main as amain
    return amain(a.rest)


if __name__ == "__main__":
    sys.exit(main())
