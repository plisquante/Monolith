#!/usr/bin/env python3
"""Same-conditions benchmark of the native (j2zig) Minecraft build against vanilla on HotSpot.

  python3 tools/bench.py --out DIR --mcout OUT_DIR --client-jar client.jar --libs LIBDIR \
        --target native|jvm [--display :97] [--seconds 180] [--world "New World"]

Per run it starts the game with --quickPlaySingleplayer, samples /proc every second (process CPU
time, RSS, threads), watches the world save (session.lock = integrated server opened the world,
region files = generated/saved chunks), toggles the F3 overlay and saves screenshots so FPS, server
tick time and GPU/driver strings can be read from them, and writes samples.csv + summary.json.
GPU utilisation is read from nvidia-smi / amdgpu sysfs when present (absent on software renderers).
Run on a machine with a real GPU and without LIBGL_ALWAYS_SOFTWARE for hardware numbers.
"""
import argparse, csv, glob, json, os, shutil, signal, subprocess, time


def proc_stat(pid):
    try:
        with open(f"/proc/{pid}/stat") as f:
            s = f.read().rsplit(")", 1)[1].split()
        utime, stime, threads = int(s[11]), int(s[12]), int(s[17])
        with open(f"/proc/{pid}/statm") as f:
            rss = int(f.read().split()[1]) * os.sysconf("SC_PAGE_SIZE")
        return (utime + stime) / os.sysconf("SC_CLK_TCK"), rss, threads
    except (OSError, IndexError, ValueError):
        return None


def gpu_sample():
    try:
        r = subprocess.run(["nvidia-smi", "--query-gpu=utilization.gpu,memory.used,power.draw",
                            "--format=csv,noheader,nounits"], capture_output=True, text=True, timeout=3)
        if r.returncode == 0 and r.stdout.strip():
            u, m, p = [x.strip() for x in r.stdout.strip().splitlines()[0].split(",")]
            return {"gpu_util_pct": float(u), "gpu_mem_mib": float(m), "gpu_power_w": float(p)}
    except (OSError, subprocess.SubprocessError, ValueError):
        pass
    for f in glob.glob("/sys/class/drm/card*/device/gpu_busy_percent"):
        try:
            return {"gpu_util_pct": float(open(f).read())}
        except (OSError, ValueError):
            pass
    return {}


def region_stats(world):
    files = glob.glob(os.path.join(world, "**", "region", "*.mca"), recursive=True)
    return len(files), sum(os.path.getsize(f) for f in files)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--mcout", required=True, help="directory with natives/, assets/, game/ (j2zig build output)")
    ap.add_argument("--target", choices=["native", "jvm"], required=True)
    ap.add_argument("--client-jar")
    ap.add_argument("--libs", help="directory tree with the library jars (jvm target)")
    ap.add_argument("--java", default="java")
    ap.add_argument("--display", default=os.environ.get("DISPLAY", ":97"))
    ap.add_argument("--seconds", type=int, default=180, help="sampling time after the world is open")
    ap.add_argument("--world", default="New World")
    ap.add_argument("--timeout", type=int, default=1500)
    ap.add_argument("--graphics", choices=["auto", "vulkan", "opengl"], default="auto",
                    help="RHI graphics backend (auto: Vulkan 1.1+ with OpenGL 3.3 Core fallback)")
    ap.add_argument("--no-lwjgl", action="store_true",
                    help="run native build without external LWJGL/JNA overrides (routes via J2ZIG RHI)")
    ap.add_argument("--env", action="append", default=[], help="extra KEY=VALUE for the game process")
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    nat = os.path.join(a.mcout, "natives")
    game = os.path.join(a.out, "game")
    if os.path.exists(game):
        shutil.rmtree(game)
    shutil.copytree(os.path.join(a.mcout, "game_pristine") if os.path.exists(os.path.join(a.mcout, "game_pristine"))
                    else os.path.join(a.mcout, "game"), game, ignore=shutil.ignore_patterns("crash-reports", "logs"))
    gfx_flag = os.environ.get("MC_GRAPHICS", "opengl" if a.graphics == "auto" else a.graphics)
    args = ["--version", "26.3", "--gameDir", game, "--assetsDir", os.path.join(a.mcout, "assets"), "--assetIndex", "34",
            "--username", "Player", "--accessToken", "0", "--userType", "legacy", "--graphicsBackend",
            gfx_flag, "--quickPlaySingleplayer", a.world]
    if a.target == "native" and a.no_lwjgl:
        props = []
        env = dict(os.environ, DISPLAY=a.display, J2Z_RHI_BACKEND=a.graphics,
                   J2Z_NO_LWJGL="1", J2Z_RESOURCES=os.path.join(a.mcout, "resources"))
    else:
        props = [f"-Djava.library.path={nat}", f"-Dorg.lwjgl.librarypath={nat}", f"-Djna.boot.library.path={nat}",
                 "-Djna.nosys=false", "-Dorg.lwjgl.system.allocator=system"]
        env = dict(os.environ, DISPLAY=a.display, LD_LIBRARY_PATH=nat + ":" + os.environ.get("LD_LIBRARY_PATH", ""),
                   J2Z_RHI_BACKEND=a.graphics, J2Z_RESOURCES=os.path.join(a.mcout, "resources"))
    env.pop("JAVA_TOOL_OPTIONS", None)
    for kv in a.env:
        k, v = kv.split("=", 1)
        env[k] = v
    if a.target == "native":
        cmd = [os.path.join(a.mcout, "minecraft")] + props + args
    else:
        jars = [a.client_jar] + sorted(glob.glob(os.path.join(a.libs, "**", "*.jar"), recursive=True))
        jars = [j for j in jars if "natives" not in os.path.basename(j) and "linux-aarch" not in j]
        cmd = [a.java, "-Xmx4g"] + props + ["-cp", os.pathsep.join(jars), "net.minecraft.client.main.Main"] + args
    log = open(os.path.join(a.out, "game.log"), "w")
    t0 = time.time()
    p = subprocess.Popen(cmd, env=env, stdout=log, stderr=subprocess.STDOUT, preexec_fn=os.setsid)
    world = os.path.join(game, "saves", a.world)
    lock = os.path.join(world, "session.lock")
    ev = {"launch": 0.0}
    rows = []
    last = proc_stat(p.pid)
    last_t = t0
    shots = 0
    world_t = None
    next_shot = None
    ncpu = os.cpu_count() or 1
    try:
        while time.time() - t0 < a.timeout and p.poll() is None:
            time.sleep(1)
            now = time.time()
            st = proc_stat(p.pid)
            if st is None:
                break
            cpu = (st[0] - last[0]) / max(now - last_t, 1e-6) * 100.0  # % of one core
            last, last_t = st, now
            nch, nbytes = region_stats(world)
            row = {"t": round(now - t0, 1), "cpu_pct_of_one_core": round(cpu, 1), "cpu_pct_of_machine": round(cpu / ncpu, 1),
                   "rss_mib": round(st[1] / 2**20, 1), "threads": st[2], "region_files": nch, "region_bytes": nbytes}
            row.update(gpu_sample())
            rows.append(row)
            if "window" not in ev:
                r = subprocess.run(["import", "-display", a.display, "-window", "root", os.path.join(a.out, "probe.png")],
                                   capture_output=True)
                if r.returncode == 0 and os.path.getsize(os.path.join(a.out, "probe.png")) > 50000:
                    ev["window"] = round(now - t0, 1)
            if "session_lock" not in ev and os.path.exists(lock) and os.path.getmtime(lock) >= t0:
                ev["session_lock"] = round(now - t0, 1)
            if "session_lock" in ev and "world_ready" not in ev and int(now - t0) % 3 == 0:
                # the "Loading terrain" screen draws a pure green progress bar at (470,264); once the
                # player is in the world that pixel is no longer 00FF00
                shot = os.path.join(a.out, "probe.png")
                r = subprocess.run(["import", "-display", a.display, "-window", "root", shot], capture_output=True)
                if r.returncode == 0:
                    px = subprocess.run(["convert", shot, "-crop", "1x1+470+264", "txt:-"], capture_output=True, text=True).stdout
                    if "#00FF00" not in px.upper() and ev.get("loading_seen"):
                        ev["world_ready"] = round(now - t0, 1)
                        world_t = now
                        next_shot = now + 10
                    elif "#00FF00" in px.upper():
                        ev["loading_seen"] = True
            if "first_region_file" not in ev and nch > 1:
                ev["first_region_file"] = round(now - t0, 1)
            if world_t is not None and now >= next_shot and shots < 8:
                if shots == 0:
                    subprocess.run(["xdotool", "key", "F3"], env=env)
                    time.sleep(2)
                subprocess.run(["import", "-display", a.display, "-window", "root",
                                os.path.join(a.out, f"f3_{shots:02d}.png")], capture_output=True)
                shots += 1
                next_shot = now + max(15, a.seconds // 8)
            if world_t is not None and now - world_t > a.seconds:
                break
            if "session_lock" in ev and "world_ready" not in ev and now - t0 > ev["session_lock"] + a.timeout // 2:
                break
    finally:
        try:
            os.killpg(os.getpgid(p.pid), signal.SIGTERM)
            time.sleep(3)
            os.killpg(os.getpgid(p.pid), getattr(signal, "SIGKILL", signal.SIGTERM))
        except OSError:
            pass
    with open(os.path.join(a.out, "samples.csv"), "w", newline="") as f:
        if rows:
            w = csv.DictWriter(f, fieldnames=sorted({k for r in rows for k in r}, key=lambda k: (k != "t", k)))
            w.writeheader()
            w.writerows(rows)
    win = [r for r in rows if r["t"] >= ev.get("world_ready", 1e9)]

    def avg(k):
        v = [r[k] for r in win if k in r]
        return round(sum(v) / len(v), 1) if v else None

    summary = {"target": a.target, "graphics": a.graphics, "no_lwjgl": bool(a.no_lwjgl), "events_s": ev, "samples": len(rows), "machine_cpus": ncpu,
               "avg_cpu_pct_of_one_core_after_world_ready": avg("cpu_pct_of_one_core"),
               "avg_cpu_pct_of_machine_after_world_ready": avg("cpu_pct_of_machine"),
               "avg_rss_mib_after_world_ready": avg("rss_mib"),
               "peak_rss_mib": max((r["rss_mib"] for r in rows), default=None),
               "max_threads": max((r["threads"] for r in rows), default=None),
               "final_region_files": rows[-1]["region_files"] if rows else None,
               "final_region_mib": round(rows[-1]["region_bytes"] / 2**20, 2) if rows else None,
               "gpu_util_pct_avg": avg("gpu_util_pct"), "screenshots": shots, "exit_code": p.poll()}
    json.dump(summary, open(os.path.join(a.out, "summary.json"), "w"), indent=1)
    print(json.dumps(summary, indent=1))


if __name__ == "__main__":
    main()
