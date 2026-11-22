"""Status card of a running sharded engine build (tools/mcbuild.py log), as a phone-sized PNG.

    python3 tools/progress_card.py BUILD.log OUT.png [--title T] [--tz-offset 3] [--note TEXT]
    python3 tools/progress_card.py BUILD.log OUT.html --history HIST.jsonl [...]   (status page for the Artifact tool)

Reads the log: the start time (first line, `date +%T` of the build script), units reused ("unchanged"), units
compiled ("shard N/M compiled in"), the link and the end (BUILD_RC). Progress counts compiled units of those that
had to be compiled; the ETA extrapolates the compile rate so far plus LINK_MIN for shard 0 and the link. The card is
drawn by tools/status_card.js.
"""
import argparse
import datetime as dt
import json
import os
import re
import subprocess
import sys
import tempfile

BULK = 31     # shards 1..31 hold the bulk of the classes; higher numbers are one-class dev units
JOBS = 2      # units compiled in parallel
LINK_MIN = 10  # shard 0 + link of the shared library, minutes (measured on the 101-unit Android build)


def _esc(x) -> str:
    import html
    return html.escape(str(x), quote=True)


def page_html(c: dict, hist: list) -> str:
    """Status page for the Artifact tool (fragment: the tool adds the document skeleton). Phone first, both themes."""
    fail = any(st["state"] == "fail" for st in c["steps"])
    icon = {"done": "✓", "run": "●", "wait": "○", "fail": "✕"}
    steps = "".join(f'<li class="{_esc(st["state"])}"><span class="i">{icon.get(st["state"], "○")}</span>{_esc(st["label"])}</li>'
                    for st in c["steps"])
    rows = "".join(f'<li><span class="t">{_esc(h["time"])}</span><span class="e">ETA {_esc(h["eta"])} · %{_esc(h["pct"])}</span>'
                   f'<span class="n">{_esc(h["note"])}</span></li>' for h in hist)
    return f"""<title>MONOLITH Derleme Durumu</title>
<meta name="claude:viewport-fit-sides" content="cover">
<style>
/* one column, phone first: ETA and progress on top, steps, then the report history */
:root{{--bg:#f4f6f9;--surface:#ffffff;--fg:#141922;--mut:#5c6675;--line:#dde2ea;--accent:#2f6fdf;--ok:#1f8a4c;--bad:#c62f2f;
  --font:"Inter",system-ui,-apple-system,"Segoe UI",Roboto,sans-serif;--num:"Space Grotesk","Inter",system-ui,sans-serif}}
@media (prefers-color-scheme: dark){{:root:not([data-theme="light"]){{--bg:#0f1218;--surface:#171b23;--fg:#e9edf3;--mut:#9aa4b3;--line:#2a303b;--accent:#5b9bff;--ok:#43c17a;--bad:#ef6464;color-scheme:dark}}}}
:root[data-theme="dark"]{{--bg:#0f1218;--surface:#171b23;--fg:#e9edf3;--mut:#9aa4b3;--line:#2a303b;--accent:#5b9bff;--ok:#43c17a;--bad:#ef6464;color-scheme:dark}}
body{{background:var(--bg);color:var(--fg);font:17px/1.5 var(--font);padding-left:max(16px,env(safe-area-inset-left,0px));padding-right:max(16px,env(safe-area-inset-right,0px));padding-block:20px}}
main{{max-width:560px;margin:0 auto;display:grid;gap:16px}}
.head{{display:flex;justify-content:space-between;align-items:baseline;gap:12px}}
h1{{margin:0;font-size:20px;font-weight:650;text-wrap:balance}}
.time{{color:var(--mut);font-size:15px;white-space:nowrap;font-variant-numeric:tabular-nums}}
.hero{{background:var(--surface);border:1px solid var(--line);border-radius:14px;padding:20px;display:grid;gap:14px}}
.eta{{display:flex;align-items:baseline;gap:10px;flex-wrap:wrap}}
.eta small{{color:var(--mut);font-size:15px;letter-spacing:.06em}}
.eta b{{font:700 56px/1 var(--num);letter-spacing:-1px;font-variant-numeric:tabular-nums}}
.pct{{margin-left:auto;font:700 32px/1 var(--num);color:{"var(--bad)" if fail else "var(--accent)"};font-variant-numeric:tabular-nums}}
.bar{{height:12px;border-radius:6px;background:var(--line);overflow:hidden}}
.bar i{{display:block;height:100%;width:{int(c["pct"])}%;background:{"var(--bad)" if fail else "var(--accent)"}}}
.note{{margin:0;font-size:18px}}
h2{{margin:8px 0 0;font-size:13px;color:var(--mut);text-transform:uppercase;letter-spacing:.08em;font-weight:600}}
ul{{list-style:none;margin:0;padding:0}}
.steps li{{display:flex;gap:10px;padding:6px 0;color:var(--mut)}}.steps .i{{width:20px;text-align:center}}
.steps .done .i{{color:var(--ok)}}.steps .run{{color:var(--fg);font-weight:600}}.steps .run .i{{color:var(--accent)}}.steps .fail{{color:var(--bad)}}
.hist li{{display:grid;grid-template-columns:auto 1fr;gap:2px 12px;padding:10px 0;border-top:1px solid var(--line)}}
.hist .t{{color:var(--mut);font-variant-numeric:tabular-nums}}.hist .e{{font-weight:600;font-variant-numeric:tabular-nums}}
.hist .n{{grid-column:2;color:var(--mut);font-size:15px;min-width:0}}
</style>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Inter:wght@400;600;700&family=Space+Grotesk:wght@700&display=swap">
<main>
  <div class="head"><h1>{_esc(c["title"])}</h1><span class="time">Güncelleme {_esc(c["time"])}</span></div>
  <section class="hero">
    <div class="eta"><small>ETA</small><b>{_esc(c["eta"])}</b><span class="pct">%{int(c["pct"])}</span></div>
    <div class="bar"><i></i></div>
    <p class="note">{_esc(c["note"])}</p>
  </section>
  <h2>Adımlar</h2>
  <ul class="steps">{steps}</ul>
  <h2>Önceki raporlar</h2>
  <ul class="hist">{rows}</ul>
</main>
"""


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("log")
    ap.add_argument("out")
    ap.add_argument("--title", default="Android .so derlemesi")
    ap.add_argument("--tz-offset", type=float, default=3.0, help="hours added to the machine clock (UTC) for display")
    ap.add_argument("--note", default=None, help="replaces the generated note")
    ap.add_argument("--history", default=None, help="JSON-lines file of earlier reports (page output: appended, last 8 shown)")
    a = ap.parse_args()

    text = open(a.log, errors="replace").read()
    lines = text.splitlines()
    now = dt.datetime.now(dt.timezone.utc).replace(tzinfo=None)
    tz = dt.timedelta(hours=a.tz_offset)
    start = None
    if lines and re.fullmatch(r"\d\d:\d\d:\d\d", lines[0].strip()):
        t = dt.datetime.strptime(lines[0].strip(), "%H:%M:%S").time()
        start = dt.datetime.combine(now.date(), t)
        if start > now:
            start -= dt.timedelta(days=1)
    total_units = 0
    m = re.search(r"shard \d+/(\d+) ", text)
    if m:
        total_units = int(m.group(1)) + 1  # shards are numbered 0..N
    reused = len(re.findall(r"shard \d+/\d+ unchanged", text))
    compiled = len(re.findall(r"shard \d+/\d+ compiled in", text))
    failed = re.search(r"zig failed|Traceback|Killed|BUILD_RC=[1-9]", text)
    done = "BUILD_RC=0" in text
    to_compile = max(total_units - reused, compiled, 1)

    steps = [
        {"label": "Çeviri (transpile)", "state": "done" if total_units else "run"},
        {"label": f"{reused} parça yeniden kullanıldı", "state": "done" if total_units else "wait"},
        {"label": f"Derleme {compiled}/{to_compile}", "state": "done" if compiled >= to_compile else ("run" if total_units else "wait")},
        {"label": "shard0 + bağlama", "state": "done" if done else ("run" if compiled >= to_compile else "wait")},
    ]
    if done:
        pct, eta, note = 100, (now + tz).strftime("%H:%M"), "Derleme bitti. .so paketlenip gönderilecek."
    elif failed:
        pct, eta = round(100 * compiled / to_compile), "--:--"
        note = "Derleme hata verdi: " + failed.group(0) + ". Log inceleniyor."
        steps[2]["state"] = "fail"
    else:
        pct = round(95 * compiled / to_compile) if total_units else 2
        # remaining work from the measured unit times: bulk shards (1..BULK) take minutes, the per-class dev units
        # seconds; two compile in parallel; shard 0 and the link follow
        done = {int(k): float(t) for k, t in re.findall(r"shard (\d+)/\d+ compiled in ([\d.]+)s", text)}
        reused_ids = {int(k) for k in re.findall(r"shard (\d+)/\d+ unchanged", text)}
        bulk_t = [t for k, t in done.items() if 1 <= k <= BULK]
        dev_t = [t for k, t in done.items() if k > BULK]
        bulk_avg = sum(bulk_t) / len(bulk_t) if bulk_t else 240.0
        dev_avg = sum(dev_t) / len(dev_t) if dev_t else 15.0
        left = [k for k in range(1, total_units) if k not in done and k not in reused_ids]
        secs = sum(bulk_avg if k <= BULK else dev_avg for k in left) / JOBS
        eta_t = now + dt.timedelta(seconds=secs, minutes=LINK_MIN) if total_units else None
        eta = (eta_t + tz).strftime("%H:%M") if eta_t else "--:--"
        left = to_compile - compiled
        note = (f"{compiled}/{to_compile} parça derlendi, {left} kaldı; hata yok." if total_units
                else "Kod çevriliyor; derlenecek parça sayısı birazdan belli olacak.")
    card = {"title": a.title, "eta": eta, "pct": pct, "note": a.note or note,
            "time": (now + tz).strftime("%H:%M"), "steps": steps}
    if a.out.endswith(".html"):
        hist = []
        if a.history:
            if os.path.isfile(a.history):
                hist = [json.loads(l) for l in open(a.history) if l.strip()]
            hist.append({"time": card["time"], "eta": eta, "pct": pct, "note": card["note"]})
            with open(a.history, "w") as fh:
                fh.write("".join(json.dumps(h, ensure_ascii=False) + "\n" for h in hist[-50:]))
        with open(a.out, "w") as fh:
            fh.write(page_html(card, hist[-8:][::-1]))
        print(a.out)
        return 0
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as f:
        json.dump(card, f, ensure_ascii=False)
    try:
        here = os.path.dirname(os.path.abspath(__file__))
        return subprocess.call(["node", os.path.join(here, "status_card.js"), f.name, a.out])
    finally:
        os.unlink(f.name)


if __name__ == "__main__":
    sys.exit(main())
