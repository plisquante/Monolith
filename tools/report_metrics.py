"""Per-device performance history from TouchCore report zips, and a comparison page (line charts, one line per device).

    python3 tools/report_metrics.py add RUNS.json REPORT.zip [REPORT.zip ...]   # parse reports, append new runs
    python3 tools/report_metrics.py page RUNS.json OUT.html                      # page for the Artifact tool

A run is one report zip (the launcher's "Dışa aktar"). Figures come from:
  game/metrics.csv       2 s samples: process memory (PSS), process CPU share, GPU load, frames per second
  game/display_info.txt  memory samples of launchers older than metrics.csv
  crash-reports/*.txt    "Client ticks: N ticks / T s" (TPS), device and GPU names
  game/engine_log.txt    collector lines (live heap, longest mark pause), "user <name>" (debug setting)
Figures a report does not contain stay null and the page says "veri yok"; nothing is estimated.
RUNS.json is plain JSON (one object per run), meant to be committed with the project later.
"""
import json
import os
import re
import sys
import zipfile

# deneyen kişilerin telefonlarının model numaraları; başka her model model numarasıyla gösterilir
DEVICE_NAMES = {"SM-S721B": "Galaxy S24 FE", "SM-S942B": "Galaxy S26"}
METRICS = [  # key, title, unit, digits
    ("ram_avg", "RAM (ortalama)", "MB", 0),
    ("ram_max", "RAM (en yüksek)", "MB", 0),
    ("cpu_avg", "İşlemci (ortalama)", "%", 1),
    ("gpu_avg", "Ekran kartı (ortalama)", "%", 1),
    ("fps_avg", "FPS (ortalama)", "kare/sn", 1),
    ("tps", "TPS", "tick/sn", 1),
    ("gc_pause_max", "En uzun GC duraklaması", "ms", 0),
]


def _avg(xs):
    xs = [x for x in xs if x is not None and x >= 0]
    return round(sum(xs) / len(xs), 2) if xs else None


def parse_report(path: str) -> dict:
    z = zipfile.ZipFile(path)
    names = z.namelist()

    def text(n):
        return z.read(n).decode("utf-8", "replace") if n in names else ""

    run = {"id": os.path.basename(path), "time": None, "device": None, "model": None, "user": None, "samples": 0}
    m = re.search(r"rapor-(\d{8})-(\d{6})", path)
    if m:
        d, t = m.groups()
        run["time"] = f"{d[:4]}-{d[4:6]}-{d[6:]} {t[:2]}:{t[2:4]}"
        run["id"] = f"{d}-{t}"
    ram, cpu, gpu, fps = [], [], [], []
    mc = text("game/metrics.csv")
    for line in mc.splitlines():
        if line.startswith("# "):
            dm = re.search(r"device \S+ (\S+)", line)
            if dm:
                run["model"] = dm.group(1)
            um = re.search(r" user (\S+)", line)
            if um:
                run["user"] = um.group(1)
            continue
        p = line.split(",")
        if len(p) == 7 and p[0].isdigit():
            ram.append(float(p[1]))
            cpu.append(float(p[4]))
            gpu.append(float(p[5]))
            fps.append(float(p[6]))
    di = text("game/display_info.txt")
    if not ram:
        ram = [float(x) for x in re.findall(r"process memory (\d+) MB", di)]
    if not run["model"]:
        dm = re.search(r"device \S+ (\S+) android", di)
        if dm:
            run["model"] = dm.group(1)
    if not run["user"]:
        um = re.search(r"^user (\S+)", di, re.M) or re.search(r"^user (\S+)", text("game/engine_log.txt"), re.M)
        if um:
            run["user"] = um.group(1)
    crash = sorted(n for n in names if n.startswith("crash-reports/") and n.endswith(".txt"))
    tps = None
    gpu_name = None
    if crash:
        c = text(crash[-1])
        tm = re.search(r"Client ticks: (\d+) ticks / ([\d.]+)s", c)
        if tm and float(tm.group(2)) > 0:
            tps = round(int(tm.group(1)) / float(tm.group(2)), 2)
        gm = re.search(r"Graphics Device: (.+)", c)
        gpu_name = gm.group(1).strip() if gm else None
    log = text("game/engine_log.txt")
    pauses = [int(x) / 1000 for x in re.findall(r"mark (\d+) us", log)]
    lives = re.findall(r"\[gc #\d+: live (\d+) MiB", log)
    run.update({
        "device": DEVICE_NAMES.get(run["model"], run["model"] or "bilinmiyor"),
        "gpu_name": gpu_name,
        "samples": len(ram),
        "ram_avg": _avg(ram),
        "ram_max": max(ram) if ram else None,
        "cpu_avg": _avg(cpu),
        "gpu_avg": _avg(gpu),
        "fps_avg": _avg(fps),
        "tps": tps,
        "gc_pause_max": round(max(pauses)) if pauses else None,
        "live_heap_mb": int(lives[-1]) if lives else None,
    })
    return run


def cmd_add(runs_path: str, reports: list) -> None:
    runs = json.load(open(runs_path)) if os.path.isfile(runs_path) else []
    known = {r["id"] for r in runs}
    for rp in reports:
        r = parse_report(rp)
        if r["id"] in known:
            continue
        if not r["model"]:
            print(f"{r['id']}: no device in the report (launcher before display_info/metrics.csv), skipped")
            continue
        runs.append(r)
        known.add(r["id"])
        print(f"{r['id']}: {r['device']} user={r['user']} ram_avg={r['ram_avg']} fps={r['fps_avg']} tps={r['tps']}")
    runs.sort(key=lambda r: r["id"])
    with open(runs_path, "w") as f:
        json.dump(runs, f, ensure_ascii=False, indent=1)


def page_html(runs: list) -> str:
    """Artifact fragment: one line chart per metric (x = reports in arrival order, one line per device), the
    latest-vs-previous table, and every run. Data is embedded; the charts are drawn by inline script."""
    data = json.dumps({"runs": runs, "metrics": METRICS}, ensure_ascii=False)
    return """<title>Cihaz Performansı</title>
<meta name="claude:viewport-fit-sides" content="cover">
<style>
/* one column: a line chart per metric (a line per device across reports), then the tables */
:root{--bg:#f6f6f4;--surface:#fcfcfb;--fg:#0b0b0b;--mut:#52514e;--line:#e3e2dd;--grid:#ecebe6;--s1:#2a78d6;--s2:#eb6834;--s3:#1f9d6b;--s4:#8a5cd6;--s5:#c28a14;
  --font:"Inter",system-ui,-apple-system,"Segoe UI",Roboto,sans-serif}
@media (prefers-color-scheme: dark){:root:not([data-theme="light"]){--bg:#121211;--surface:#1a1a19;--fg:#f0efec;--mut:#c3c2b7;--line:#33322f;--grid:#2a2a28;--s1:#3987e5;--s2:#d95926;--s3:#34b583;--s4:#a07be5;--s5:#d9a53a;color-scheme:dark}}
:root[data-theme="dark"]{--bg:#121211;--surface:#1a1a19;--fg:#f0efec;--mut:#c3c2b7;--line:#33322f;--grid:#2a2a28;--s1:#3987e5;--s2:#d95926;--s3:#34b583;--s4:#a07be5;--s5:#d9a53a;color-scheme:dark}
body{background:var(--bg);color:var(--fg);font:16px/1.5 var(--font);padding-left:max(16px,env(safe-area-inset-left,0px));padding-right:max(16px,env(safe-area-inset-right,0px));padding-block:20px}
main{max-width:760px;margin:0 auto;display:grid;gap:18px}
h1{margin:0;font-size:22px;text-wrap:balance}h2{margin:0 0 4px;font-size:16px}
.sub{color:var(--mut);margin:0;font-size:14px}
.legend{display:flex;gap:16px;flex-wrap:wrap;font-size:14px;color:var(--mut)}.legend i{display:inline-block;width:12px;height:12px;border-radius:50%;margin-right:6px;vertical-align:-1px}
.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,340px),1fr));gap:14px}
.card{background:var(--surface);border:1px solid var(--line);border-radius:12px;padding:14px;min-width:0}
.card .u{color:var(--mut);font-size:13px}
svg{display:block;width:100%;height:auto;overflow:visible}
svg text{fill:var(--mut);font:12px var(--font)}svg .v{fill:var(--fg);font-weight:600}
.none{color:var(--mut);font-size:14px;padding:18px 0}
.tw{overflow-x:auto}table{border-collapse:collapse;width:100%;font-size:14px;font-variant-numeric:tabular-nums}
th,td{text-align:right;padding:6px 8px;border-bottom:1px solid var(--line);white-space:nowrap}th:nth-child(-n+2),td:nth-child(-n+2){text-align:left}
th{color:var(--mut);font-weight:600}
#tip{position:fixed;pointer-events:none;background:var(--surface);color:var(--fg);border:1px solid var(--line);border-radius:8px;padding:6px 9px;font-size:13px;box-shadow:0 4px 14px rgba(0,0,0,.15)}
</style>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Inter:wght@400;600;700&display=swap">
<main>
  <div><h1>Cihaz Performansı</h1><p class="sub" id="sub"></p></div>
  <div class="legend" id="legend"></div>
  <div class="grid" id="charts"></div>
  <section class="card"><h2>Son rapor ve fark</h2><div class="tw"><table id="cmp"></table></div></section>
  <section class="card"><h2>Tüm raporlar</h2><div class="tw"><table id="all"></table></div></section>
</main>
<div id="tip" hidden></div>
<script>
const D = """ + data + """;
const runs = D.runs, M = D.metrics;
const devices = [...new Set(runs.map(r => r.device))];
const byDev = {}; for (const d of devices) byDev[d] = runs.filter(r => r.device === d);
const last = d => byDev[d][byDev[d].length - 1], prev = d => byDev[d].length > 1 ? byDev[d][byDev[d].length - 2] : null;
const fmt = (v, k) => v == null ? "veri yok" : Number(v).toFixed(k);
document.getElementById("sub").textContent = runs.length + " rapor, " + devices.length + " cihaz. Son güncelleme: " + (runs.length ? runs[runs.length-1].time : "-");
const tip = document.getElementById("tip");
function show(e, t){ tip.textContent = t; tip.hidden = false; tip.style.left = Math.min(e.clientX + 12, innerWidth - 220) + "px"; tip.style.top = (e.clientY + 12) + "px"; }
const COLORS = ["var(--s1)", "var(--s2)", "var(--s3)", "var(--s4)", "var(--s5)"];
const colorOf = d => COLORS[devices.indexOf(d) % COLORS.length];
document.getElementById("legend").innerHTML = devices.map(d => "<span><i style='background:" + colorOf(d) + "'></i>" + d + "</span>").join("");
// x ekseni = rapor sırası (her cihazın çalıştırmaları geliş sırasıyla); her cihaz için bir çizgi
const order = runs.map((r, i) => i);
function chart([key, title, unit, digits]){
  const card = document.createElement("div"); card.className = "card";
  card.innerHTML = "<h2>" + title + "</h2><div class='u'>" + unit + "</div>";
  const vals = runs.map(r => r[key]).filter(v => v != null);
  if (!vals.length){ card.insertAdjacentHTML("beforeend", "<div class='none'>Bu ölçüm henüz hiçbir raporda yok.</div>"); return card; }
  const W = 340, H = 200, L = 42, R = 12, B = 34, T = 18, max = Math.max(...vals) * 1.12 || 1;
  const n = runs.length, xAt = i => n === 1 ? (L + W - R) / 2 : L + (W - L - R) * i / (n - 1), yAt = v => T + (H - T - B) * (1 - v / max);
  let s = "<svg viewBox='0 0 " + W + " " + H + "' role='img' aria-label='" + title + "'>";
  for (let i = 0; i <= 4; i++){ const y = T + (H - T - B) * (1 - i / 4), v = max * i / 4;
    s += "<line x1='" + L + "' x2='" + (W - R) + "' y1='" + y + "' y2='" + y + "' stroke='var(--grid)'/><text x='" + (L - 6) + "' y='" + (y + 4) + "' text-anchor='end'>" + (v >= 100 ? Math.round(v) : v.toFixed(1)) + "</text>"; }
  const ticks = n === 1 ? [0] : n === 2 ? [0, 1] : [0, Math.floor((n - 1) / 2), n - 1];
  for (const i of ticks) s += "<text x='" + xAt(i) + "' y='" + (H - B + 18) + "' text-anchor='" + (i === 0 && n > 1 ? "start" : i === n - 1 && n > 1 ? "end" : "middle") + "'>" + runs[i].time.slice(5) + "</text>";
  for (const d of devices){
    const col = colorOf(d), pts = order.filter(i => runs[i].device === d && runs[i][key] != null);
    if (!pts.length) continue;
    s += "<path d='" + pts.map((i, j) => (j ? "L" : "M") + xAt(i) + "," + yAt(runs[i][key])).join("") + "' fill='none' stroke='" + col + "' stroke-width='2.5' stroke-linejoin='round' stroke-linecap='round'/>";
    for (const i of pts) s += "<circle cx='" + xAt(i) + "' cy='" + yAt(runs[i][key]) + "' r='4.5' fill='" + col + "' stroke='var(--surface)' stroke-width='2' data-t='" + d + " (" + runs[i].time + "): " + fmt(runs[i][key], digits) + " " + unit + "'/>";
    const e = pts[pts.length - 1];
    s += "<text class='v' x='" + xAt(e) + "' y='" + (yAt(runs[e][key]) - 9) + "' text-anchor='" + (e === n - 1 && n > 1 ? "end" : "middle") + "'>" + fmt(runs[e][key], digits) + "</text>";
  }
  card.insertAdjacentHTML("beforeend", s + "</svg>");
  card.querySelectorAll("circle[data-t]").forEach(p => { p.addEventListener("pointermove", e => show(e, p.dataset.t)); p.addEventListener("pointerleave", () => tip.hidden = true); });
  return card;
}
const ch = document.getElementById("charts"); M.forEach(m => ch.appendChild(chart(m)));
let c = "<tr><th>Cihaz</th><th>Ölçüm</th><th>Önceki</th><th>Son</th><th>Fark</th></tr>";
for (const d of devices) for (const [k, t, u, g] of M){ const a = prev(d) && prev(d)[k], b = last(d)[k];
  const diff = a != null && b != null ? ((b - a) >= 0 ? "+" : "") + (b - a).toFixed(g) + " " + u : "-";
  c += "<tr><td>" + d + "</td><td>" + t + "</td><td>" + fmt(a, g) + "</td><td>" + fmt(b, g) + "</td><td>" + diff + "</td></tr>"; }
document.getElementById("cmp").innerHTML = c;
let a = "<tr><th>Rapor</th><th>Cihaz</th><th>Kişi</th>" + M.map(m => "<th>" + m[1] + "</th>").join("") + "</tr>";
for (const r of runs.slice().reverse()) a += "<tr><td>" + r.time + "</td><td>" + r.device + "</td><td>" + (r.user || "-") + "</td>" + M.map(m => "<td>" + fmt(r[m[0]], m[3]) + "</td>").join("") + "</tr>";
document.getElementById("all").innerHTML = a;
</script>
"""


def main() -> int:
    if len(sys.argv) >= 4 and sys.argv[1] == "add":
        cmd_add(sys.argv[2], sys.argv[3:])
        return 0
    if len(sys.argv) == 4 and sys.argv[1] == "page":
        runs = json.load(open(sys.argv[2])) if os.path.isfile(sys.argv[2]) else []
        with open(sys.argv[3], "w") as f:
            f.write(page_html(runs))
        print(sys.argv[3])
        return 0
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main())
