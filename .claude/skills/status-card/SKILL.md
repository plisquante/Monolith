---
name: status-card
description: Periodic progress report as a phone-sized image card (ETA, percent, two-sentence note, steps). Use for every periodic status message (15-minute engine compile reports) and whenever the user asks "durum ne" during a long job.
---

# status-card

Users read status on a phone. Every periodic report is one image plus the same content as two text lines.

## Default: live status page (Artifact)
One private claude.ai page, republished at every report (same file path keeps the URL). Current page:
https://claude.ai/artifact/LGgtEaPZYFGepKoUXLZsYf (file `<scratchpad>/durum.html`, history `<scratchpad>/durum_hist.jsonl`).
```
python3 tools/progress_card.py /tmp/w/build_so.log <scratchpad>/durum.html --history <scratchpad>/durum_hist.jsonl
```
then Artifact publish of that file (no icon on redeploy). Do not Read or send images in chat; reply with the two
lines below only.

## Image card (fallback)
```
python3 tools/progress_card.py /tmp/w/build_so.log <scratchpad>/status.png [--title "Android .so derlemesi"] [--note "..."]
```
Send the PNG (SendUserFile, display render) and reply with exactly:
```
ETA: SS:DD %NN
<at most two sentences: where it is, any risk>
```
Times are Turkey time (UTC+3, `--tz-offset 3` default). ETA = compile rate so far x units left + 10 min link.

## Any other job
Write a JSON card and render it:
```
{"title": "...", "eta": "21:30", "pct": 71, "time": "20:53", "note": "...",
 "steps": [{"label": "...", "state": "done|run|wait|fail"}]}
node tools/status_card.js card.json <scratchpad>/status.png [card.html]
```

## Rules
- No long text in periodic reports: the card plus the two lines above.
- A failed step is shown as fail with the reason in the note; never a green card for a failed job.
- Reschedule the next 15-minute reminder (send_later) only while the job runs.
