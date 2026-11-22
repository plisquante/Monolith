#!/usr/bin/env python3
"""Cross-platform launcher for the natively compiled Minecraft (Windows, Linux, macOS).

Zero dependencies (Python >= 3.10 standard library only).

    python3 launcher/launcher.py            # opens the launcher UI in your browser
    python3 launcher/launcher.py --cli      # list builds / start without UI
    python3 launcher/launcher.py --cli --play --username Steve --graphics vulkan

A "build" is a directory produced by tools/mcbuild.py (contains run.sh / run.bat).
Searched in: ./build/minecraft-*, $MC_BUILD_DIR, and any --build path.
Android: use android/touchcore (APK) instead, see README.
"""
import argparse, json, os, platform, secrets, subprocess, sys, threading, webbrowser
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
HERE = os.path.dirname(os.path.abspath(__file__))
IS_WIN = os.name == "nt"
OS_NAME = {"Windows": "windows", "Darwin": "macos", "Linux": "linux"}.get(platform.system(), platform.system().lower())


def cfg_path():
    if IS_WIN:
        base = os.environ.get("APPDATA") or os.path.expanduser("~")
    elif OS_NAME == "macos":
        base = os.path.expanduser("~/Library/Application Support")
    else:
        base = os.environ.get("XDG_CONFIG_HOME") or os.path.expanduser("~/.config")
    return os.path.join(base, "monolith-launcher", "settings.json")


DEFAULTS = {"username": "Player", "graphics": "auto", "build": "", "game_dir": ""}


def load_cfg():
    try:
        with open(cfg_path(), encoding="utf-8") as fh:
            return {**DEFAULTS, **json.load(fh)}
    except (OSError, ValueError):
        return dict(DEFAULTS)


def save_cfg(c):
    p = cfg_path()
    os.makedirs(os.path.dirname(p), exist_ok=True)
    with open(p, "w", encoding="utf-8") as fh:
        json.dump({k: c.get(k, v) for k, v in DEFAULTS.items()}, fh, indent=2)


def find_builds(extra=()):
    roots = [os.path.join(ROOT, "build")] + ([os.environ["MC_BUILD_DIR"]] if os.environ.get("MC_BUILD_DIR") else [])
    found, seen = [], set()
    cands = list(extra)
    for r in roots:
        if os.path.isdir(r):
            cands += [os.path.join(r, d) for d in sorted(os.listdir(r))]
    for d in cands:
        d = os.path.abspath(d)
        script = os.path.join(d, "run.bat" if IS_WIN else "run.sh")
        if d not in seen and os.path.isfile(script):
            seen.add(d)
            found.append({"path": d, "name": os.path.basename(d), "script": script})
    return found


def start(build, username, graphics, game_dir):
    script = os.path.join(build, "run.bat" if IS_WIN else "run.sh")
    if not os.path.isfile(script):
        raise FileNotFoundError(f"{script} not found. Build first: ./build.sh /path/to/client.jar")
    env = dict(os.environ, MC_USERNAME=username or "Player")
    if graphics and graphics != "auto":
        env["MC_GRAPHICS"] = graphics
    elif IS_WIN:
        env.setdefault("MC_GRAPHICS", "auto")
    if game_dir:
        env["MC_GAME_DIR"] = game_dir
    if OS_NAME == "macos":  # run.sh only sets LD_LIBRARY_PATH
        nat = os.path.join(build, "natives")
        env["DYLD_LIBRARY_PATH"] = nat + (":" + env["DYLD_LIBRARY_PATH"] if env.get("DYLD_LIBRARY_PATH") else "")
    cmd = [script] if IS_WIN else ["/bin/sh", script]
    flags = 0x00000010 if IS_WIN else 0  # CREATE_NEW_CONSOLE
    return subprocess.Popen(cmd, cwd=build, env=env, creationflags=flags)


class Handler(BaseHTTPRequestHandler):
    token = ""
    extra = ()
    procs = []

    def log_message(self, *a):
        pass

    def _send(self, code, body, ctype="application/json"):
        data = body if isinstance(body, bytes) else body.encode()
        self.send_response(code)
        self.send_header("Content-Type", ctype + "; charset=utf-8")
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(data)

    def _ok(self):  # DNS-rebinding / cross-site guard
        host = (self.headers.get("Host") or "").split(":")[0]
        return host in ("127.0.0.1", "localhost")

    def do_GET(self):
        if not self._ok():
            return self._send(403, "{}")
        if self.path.split("?")[0] == "/":
            html = open(os.path.join(HERE, "ui.html"), encoding="utf-8").read().replace("__TOKEN__", self.token)
            return self._send(200, html, "text/html")
        if self.path.startswith("/brand/"):
            name = os.path.basename(self.path)
            f = os.path.join(ROOT, "brand", name)
            if os.path.isfile(f) and name.endswith((".png", ".svg")):
                return self._send(200, open(f, "rb").read(), "image/svg+xml" if name.endswith(".svg") else "image/png")
        if self.path == "/api/state":
            return self._send(200, json.dumps({"cfg": load_cfg(), "builds": find_builds(self.extra),
                                              "os": OS_NAME, "running": sum(p.poll() is None for p in self.procs)}))
        self._send(404, "{}")

    def do_POST(self):
        if not self._ok() or self.headers.get("X-Token") != self.token:
            return self._send(403, "{}")
        try:
            body = json.loads(self.rfile.read(int(self.headers.get("Content-Length") or 0)) or b"{}")
            if self.path == "/api/save":
                save_cfg(body)
                return self._send(200, '{"ok":true}')
            if self.path == "/api/play":
                save_cfg(body)
                builds = {b["path"] for b in find_builds(self.extra)}
                if body.get("build") not in builds:
                    raise ValueError("unknown build")
                self.procs.append(start(body["build"], body.get("username"), body.get("graphics"), body.get("game_dir")))
                return self._send(200, '{"ok":true}')
        except Exception as e:  # report loudly to the UI
            return self._send(400, json.dumps({"error": str(e)}))
        self._send(404, "{}")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--cli", action="store_true")
    ap.add_argument("--play", action="store_true")
    ap.add_argument("--build", action="append", default=[])
    ap.add_argument("--username")
    ap.add_argument("--graphics", choices=["auto", "vulkan", "opengl"])
    ap.add_argument("--game-dir")
    ap.add_argument("--port", type=int, default=0)
    ap.add_argument("--no-browser", action="store_true")
    a = ap.parse_args()
    builds = find_builds(a.build)
    if a.cli:
        c = load_cfg()
        if not builds:
            print("no builds found. Run: ./build.sh /path/to/client.jar", file=sys.stderr)
            return 1
        for b in builds:
            print(b["path"])
        if a.play:
            b = next((x for x in builds if x["path"] == c["build"]), builds[-1])["path"]
            return start(b, a.username or c["username"], a.graphics or c["graphics"], a.game_dir or c["game_dir"]).wait()
        return 0
    Handler.token, Handler.extra = secrets.token_urlsafe(24), tuple(a.build)
    srv = ThreadingHTTPServer(("127.0.0.1", a.port), Handler)
    url = f"http://127.0.0.1:{srv.server_address[1]}/"
    print(f"Monolith launcher: {url}  (Ctrl+C to quit)")
    if not a.no_browser:
        threading.Timer(0.4, webbrowser.open, (url,)).start()
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        pass
    return 0


if __name__ == "__main__":
    sys.exit(main())
