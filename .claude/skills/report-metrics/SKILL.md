---
name: report-metrics
description: Every time a TouchCore report zip (j2zig-rapor-*.zip) arrives, add it to the per-device performance history and republish the "Cihaz Performansı" page (line charts of RAM, CPU, GPU, FPS, TPS, GC pause per phone; a line per device across reports). Use on every uploaded report, in addition to reading it for errors.
---

# report-metrics

Testers: Alper (Galaxy S24 FE, SM-S721B), Cem (Galaxy S26, SM-S942B). Any other model in a report is added as its own
device automatically (shown by model number until DEVICE_NAMES in tools/report_metrics.py names it).

## On every report
```
python3 -I tools/report_metrics.py add <scratchpad>/perf_runs.json <report.zip>
python3 -I tools/report_metrics.py page <scratchpad>/perf_runs.json <scratchpad>/performans.html
```
Then publish `<scratchpad>/performans.html` with the Artifact tool (same file path keeps the URL:
https://claude.ai/artifact/R19V7Ut329q35PYJhNbJXt). Reply with one line per device: latest vs previous for RAM and FPS,
plus anything that got worse. Do not paste the charts into chat.

## Where the numbers come from
- `game/metrics.csv` (launcher sampler, every 2 s): PSS memory, process CPU share of all cores, GPU load from the
  kernel node when the phone exposes it, frames per second from the host's eglSwapBuffers counter.
- `game/display_info.txt`: memory samples of older launchers. Crash report: `Client ticks` gives TPS.
- `game/engine_log.txt`: GC lines (longest mark pause), `user <name>` when the debug setting is on.
- A figure the report does not contain is null and shown as "veri yok". Never estimate one.

## Later: GitHub
`perf_runs.json` is plain JSON meant to be committed (for example `docs/perf/runs.json`) and the page regenerated from
it in CI. Not wired yet: do it only when the user asks.
