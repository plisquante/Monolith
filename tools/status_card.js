// Status card: a phone-sized HTML card (title, ETA, progress, two-line note, optional steps) rendered to PNG.
//
//   node tools/status_card.js card.json out.png [out.html]
//
// card.json: {"title": "...", "eta": "21:30", "pct": 71, "note": "one or two sentences",
//             "time": "20:53", "steps": [{"label": "Transpile", "state": "done|run|wait|fail"}, ...]}
const fs = require('fs');
const { chromium } = require('playwright');

function esc(s) {
  return String(s == null ? '' : s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
}

function html(c) {
  const pct = Math.max(0, Math.min(100, Math.round(Number(c.pct) || 0)));
  const fail = (c.steps || []).some((s) => s.state === 'fail');
  const steps = (c.steps || []).map((s) => {
    const icon = { done: '✓', run: '●', wait: '○', fail: '✕' }[s.state] || '○';
    return `<li class="${esc(s.state)}"><span class="i">${icon}</span>${esc(s.label)}</li>`;
  }).join('');
  return `<!doctype html><html lang="tr"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1"><title>Durum</title>
<style>
:root{--bg:#0f1115;--card:#181b22;--fg:#e8eaf0;--mut:#9aa3b2;--acc:#4f9cff;--ok:#38c172;--bad:#ef5350;--track:#2a2f3a}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--fg);font:16px/1.45 system-ui,-apple-system,Segoe UI,Roboto,sans-serif}
.card{margin:16px;padding:20px;border-radius:16px;background:var(--card)}
.top{display:flex;justify-content:space-between;align-items:baseline;gap:12px}
h1{margin:0;font-size:17px;font-weight:600}.time{color:var(--mut);font-size:14px;white-space:nowrap}
.eta{margin:18px 0 6px;display:flex;align-items:baseline;gap:10px}
.eta b{font-size:44px;font-weight:700;letter-spacing:-1px;font-variant-numeric:tabular-nums}
.eta span{color:var(--mut);font-size:15px}
.pct{margin-left:auto;font-size:26px;font-weight:700;color:${fail ? 'var(--bad)' : 'var(--acc)'};font-variant-numeric:tabular-nums}
.bar{height:10px;border-radius:5px;background:var(--track);overflow:hidden}
.bar i{display:block;height:100%;width:${pct}%;background:${fail ? 'var(--bad)' : 'var(--acc)'};border-radius:5px}
.note{margin:16px 0 0;font-size:16px}
ul{list-style:none;margin:16px 0 0;padding:14px 0 0;border-top:1px solid var(--track)}
li{display:flex;gap:10px;padding:4px 0;color:var(--mut);font-size:15px}.i{width:18px;text-align:center}
li.done .i{color:var(--ok)}li.run{color:var(--fg)}li.run .i{color:var(--acc)}li.fail{color:var(--bad)}
</style></head><body><div class="card" id="card">
<div class="top"><h1>${esc(c.title || 'Durum')}</h1><div class="time">${esc(c.time || '')}</div></div>
<div class="eta"><span>ETA</span><b>${esc(c.eta || '--:--')}</b><div class="pct">%${pct}</div></div>
<div class="bar"><i></i></div>
<p class="note">${esc(c.note || '')}</p>
${steps ? `<ul>${steps}</ul>` : ''}
</div></body></html>`;
}

(async () => {
  const [, , inJson, outPng, outHtml] = process.argv;
  if (!inJson || !outPng) {
    console.error('usage: node tools/status_card.js card.json out.png [out.html]');
    process.exit(2);
  }
  const page_html = html(JSON.parse(fs.readFileSync(inJson, 'utf8')));
  if (outHtml) fs.writeFileSync(outHtml, page_html);
  const b = await chromium.launch();
  const p = await b.newPage({ viewport: { width: 390, height: 600 }, deviceScaleFactor: 2 });
  await p.setContent(page_html);
  await (await p.$('#card')).screenshot({ path: outPng });
  await b.close();
  console.log(outPng);
})();
