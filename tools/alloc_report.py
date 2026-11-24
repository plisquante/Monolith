#!/usr/bin/env python3
"""Turns an allocation profile (MONO_ALLOC_PROFILE=<file>, see monolith/rt/prof.rs) into the plan for leaving the GC.

    MONO_ALLOC_PROFILE=/tmp/prof.json ./run.sh          # play for a while
    python3 tools/alloc_report.py /tmp/prof.json [--exe build/.../rust/prog] [--md report.md] [--top 40]

Classification of every class by how long its objects live (collections survived: 0, 1, 2, 3+):
  TEMP    < 10% of the judged bytes survive a collection: die young. Candidates for stack / arena / no allocation.
  MIXED   10% to 60%: needs a look at the allocating method (often a temporary that is sometimes stored).
  LONG    > 60%: data the game keeps: owned by a chunk, a world or the program (Rust-owned storage, freed by Drop).
Judged bytes = objects that died or survived at least one collection (objects younger than one collection have no
known fate yet and are left out).
"""
import argparse, bisect, collections, json, os, re, subprocess, sys

RUNTIME_PREFIXES = ("mono_rt::", "std::", "core::", "alloc::", "<", "__")


class Symbols:
    """Address -> Java-style method name, for the executable that wrote the profile."""

    def __init__(self, exe: str, maps: list, classes: list[str]):
        self.addrs: list[int] = []
        self.names: list[str] = []
        self.maps = maps
        self.exe = os.path.realpath(exe)
        # nm -C her iki Rust ad bozmasını (legacy ve v0) anlar
        out = subprocess.run(["nm", "-C", "-n", "--defined-only", exe], capture_output=True, text=True).stdout
        for line in out.splitlines():
            p = line.split(None, 2)
            if len(p) == 3 and p[1] in "tTwW":
                self.addrs.append(int(p[0], 16))
                self.names.append(p[2])
        # PT_LOAD segmentleri: dosya ofseti -> sanal adres (metin segmenti vaddr == ofset olacak şekilde eşlenmez)
        self.segs = []
        rl = subprocess.run(["readelf", "-lW", exe], capture_output=True, text=True).stdout
        for line in rl.splitlines():
            t = line.split()
            if t and t[0] == "LOAD":
                self.segs.append((int(t[1], 16), int(t[2], 16), int(t[4], 16)))  # offset, vaddr, filesz
        self.mangled = {}
        for c in classes:
            self.mangled.setdefault("".join(ch if ch.isalnum() else "_" for ch in c), c)
        self.cache: dict[int, str] = {}

    def vaddr(self, a: int):
        for lo, hi, off, path in self.maps:
            if lo <= a < hi and os.path.realpath(path) == self.exe:
                fo = a - lo + off
                for so, sv, sz in self.segs:
                    if so <= fo < so + sz:
                        return fo - so + sv
                return fo
        return None

    def java(self, sym: str) -> str:
        m = re.match(r"^(?:[A-Za-z0-9_]+::)*(r|m|init)_(.+)$", sym)
        if not m:
            return sym
        kind, body = m.group(1), m.group(2)
        if kind == "init":
            c = self.mangled.get(body)
            return f"{c.replace('/', '.')}.<clinit>" if c else sym
        i = body.find("__")
        while i != -1:
            c = self.mangled.get(body[:i])
            if c:
                meth = body[i + 2:].rsplit("__", 1)[0]
                return f"{c.replace('/', '.')}.{meth}"
            i = body.find("__", i + 1)
        return sym

    def name(self, a: int) -> str:
        if a in self.cache:
            return self.cache[a]
        v = self.vaddr(a)
        r = f"0x{a:x}"
        if v is not None:
            i = bisect.bisect_right(self.addrs, v) - 1
            if i >= 0:
                r = self.java(self.names[i])
        self.cache[a] = r
        return r


def subtract(P: dict, B: dict) -> dict:
    """P minus the earlier snapshot B: counters and sites since B. Live bytes and ages stay P's (the state now)."""
    base = {c["name"]: c for c in B["classes"]}
    cl = []
    for c in P["classes"]:
        b = base.get(c["name"])
        if b:
            c = dict(c, an=c["an"] - b["an"], ab=c["ab"] - b["ab"], arn=c["arn"] - b["arn"], arb=c["arb"] - b["arb"],
                     dn=[x - y for x, y in zip(c["dn"], b["dn"])], db=[x - y for x, y in zip(c["db"], b["db"])])
        if c["an"] > 0:
            cl.append(c)
    bs = {(s["c"], tuple(s["f"])): s for s in B["sites"]}
    sites = []
    for s in P["sites"]:
        b = bs.get((s["c"], tuple(s["f"])))
        if b:
            s = dict(s, b=s["b"] - b["b"], s=s["s"] - b["s"])
        if s["b"] > 0:
            sites.append(s)
    bt = {t["name"]: t for t in B["threads"]}
    th = [dict(t, b=t["b"] - bt.get(t["name"], {"b": 0})["b"]) for t in P["threads"]]
    T = {k: P["total"][k] - B["total"][k] for k in P["total"]}
    gcs = [g for g in P["gcs"] if g["t_ms"] > B["elapsed_ms"]]
    return dict(P, classes=cl, sites=sites, threads=[t for t in th if t["b"] > 0], total=T, gcs=gcs,
                elapsed_ms=P["elapsed_ms"] - B["elapsed_ms"])


def classify(c) -> tuple[str, float, int]:
    db, lb = c["db"], c["lb"]
    survivors = sum(db[1:]) + sum(lb[1:])
    judged = db[0] + survivors
    if judged == 0:
        return "NEW", 0.0, 0
    f = survivors / judged
    cat = "TEMP" if f < 0.10 else ("MIXED" if f <= 0.60 else "LONG")
    return cat, f, judged


def mb(x) -> str:
    return f"{x / 1048576:,.1f}"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("profile")
    ap.add_argument("--exe", help="the executable that produced the profile (default: from the profile's memory map)")
    ap.add_argument("--base", help="an earlier snapshot of the same run (MONO_ALLOC_SNAPSHOTS=1): report only what happened after it, "
                                   "i.e. the steady state without the start-up")
    ap.add_argument("--md")
    ap.add_argument("--top", type=int, default=40)
    a = ap.parse_args()
    P = json.load(open(a.profile))
    if a.base:
        P = subtract(P, json.load(open(a.base)))
    classes_file = a.profile + ".classes"
    all_classes = open(classes_file).read().split("\n") if os.path.exists(classes_file) else [c["name"] for c in P["classes"]]
    maps = [(int(lo, 16), int(hi, 16), int(off, 16), path) for lo, hi, off, path in P["maps"]]
    exe = a.exe or (maps[0][3] if maps else None)
    sym = Symbols(exe, maps, all_classes) if exe and os.path.exists(exe) else None

    out: list[str] = []
    w = out.append
    T = P["total"]
    secs = max(P["elapsed_ms"] / 1000, 0.001)
    w("# Allocation profile\n")
    w(f"- run time: {secs:,.0f} s; allocated {mb(T['b'])} MiB in {T['n']:,} objects ({mb(T['b']) .replace(',', '')} MiB, "
      f"{T['b'] / 1048576 / secs:,.1f} MiB/s, {T['b'] / 1048576 / secs * 60 / 1024:,.1f} GiB/min)")
    w(f"- {mb(T['arena_b'])} MiB ({100 * T['arena_b'] / max(T['b'], 1):.1f}%) were served without the collected heap "
      f"(recycled for-each iterators, scope arenas); these are counted in the totals above")
    gcs = P["gcs"]
    if gcs:
        tot_pause = sum(g["pause_us"] for g in gcs) / 1000
        w(f"- {len(gcs)} collections, total pause {tot_pause:,.0f} ms (longest {max(g['pause_us'] for g in gcs) / 1000:,.0f} ms); "
          f"live after the last: {mb(gcs[-1]['live'])} MiB, heap {mb(gcs[-1]['heap'])} MiB")
    w("")

    cl = []
    for c in P["classes"]:
        cat, f, judged = classify(c)
        cl.append(dict(c, cat=cat, f=f, judged=judged, live=sum(c["lb"]), died=sum(c["db"])))
    cat_b = collections.Counter()
    for c in cl:
        cat_b[c["cat"]] += c["ab"]
    w("## Where the allocated bytes go, by lifetime class\n")
    w("| class | share of allocated bytes | MiB |")
    w("|---|---:|---:|")
    for k in ("TEMP", "MIXED", "LONG", "NEW"):
        w(f"| {k} | {100 * cat_b[k] / max(T['b'], 1):.1f}% | {mb(cat_b[k])} |")
    w("")
    w("TEMP is what must stop reaching a collector (stack, arena, no allocation); LONG is what must be owned (freed by `Drop` "
      "when its chunk/world goes); MIXED needs the allocating method looked at.\n")

    w(f"## Top {a.top} classes by allocated bytes\n")
    w("| class | allocated MiB | objects | avg B | survive>=1 GC | class | live MiB | not on the heap MiB |")
    w("|---|---:|---:|---:|---:|---|---:|---:|")
    for c in sorted(cl, key=lambda c: -c["ab"])[:a.top]:
        w(f"| {c['name']} | {mb(c['ab'])} | {c['an']:,} | {c['ab'] // max(c['an'], 1)} | {100 * c['f']:.0f}% | {c['cat']} | {mb(c['live'])} | {mb(c['arb'])} |")
    w("")

    w("## Long-lived data: classes with the most live bytes after the last collection\n")
    w("| class | live MiB | objects | allocated MiB | age 0/1/2/3+ (MiB) |")
    w("|---|---:|---:|---:|---|")
    for c in sorted(cl, key=lambda c: -c["live"])[:a.top]:
        ages = "/".join(mb(x) for x in c["lb"])
        w(f"| {c['name']} | {mb(c['live'])} | {sum(c['ln']):,} | {mb(c['ab'])} | {ages} |")
    w("")

    if sym:
        cat_of = {c["name"]: c["cat"] for c in cl}
        selfs = collections.defaultdict(lambda: [0, 0, collections.Counter(), collections.Counter(), collections.Counter(), collections.Counter()])
        for s in P["sites"]:
            frames = [sym.name(int(x, 16)) for x in s["f"]]
            # tahsis ediciyi ayıkla: öndeki çalışma zamanı çerçeveleri
            i = 0
            while i < len(frames) and (frames[i].startswith(RUNTIME_PREFIXES) or frames[i].startswith("0x")):
                i += 1
            frames = frames[i:]
            # r_/m_ sarmalayıcı çiftlerini daralt ve çevrilmiş çerçeveler arasındaki çalışma zamanı yardımcılarını atla
            ded = []
            for f in frames:
                if f.startswith(RUNTIME_PREFIXES) or f.startswith("0x") or (ded and ded[-1] == f):
                    continue
                ded.append(f)
            if not ded:
                continue
            e = selfs[ded[0]]
            e[0] += s["b"]; e[1] += s["s"]
            e[2][s["c"]] += s["b"]
            e[3][ded[1] if len(ded) > 1 else "-"] += s["b"]
            e[4][ded[2] if len(ded) > 2 else "-"] += s["b"]
            if cat_of.get(s["c"]) == "TEMP":
                e[5][s["c"]] += s["b"]
        tot_s = sum(e[0] for e in selfs.values()) or 1
        w(f"## Methods that allocate the most (sampled, one sample per {P['interval']:,} B)\n")
        w("| allocating method | est. MiB | share | TEMP part | top classes | called from |")
        w("|---|---:|---:|---:|---|---|")
        for m, e in sorted(selfs.items(), key=lambda kv: -kv[1][0])[:a.top]:
            temp = sum(e[5].values())
            top = ", ".join(f"{k.split('/')[-1]} {100 * v / e[0]:.0f}%" for k, v in e[2].most_common(3))
            frm = ", ".join(f"{k.rsplit('.', 2)[-2] + '.' + k.rsplit('.', 1)[-1] if '.' in k else k}" for k, _ in e[3].most_common(2))
            w(f"| {m} | {mb(e[0])} | {100 * e[0] / tot_s:.1f}% | {100 * temp / e[0]:.0f}% | {top} | {frm} |")
        w("")
        w("## First patch candidates: methods producing the most TEMP garbage\n")
        w("A Rust patch for these allocates nothing (stack values, reused buffers, iterators without boxing) or a scope arena.\n")
        w("| method | TEMP MiB (est.) | classes |")
        w("|---|---:|---|")
        for m, e in sorted(selfs.items(), key=lambda kv: -sum(kv[1][5].values()))[:a.top]:
            t = sum(e[5].values())
            if t == 0:
                break
            w(f"| {m} | {mb(t)} | {', '.join(k.split('/')[-1] for k, _ in e[5].most_common(3))} |")
        w("")
    else:
        w("(no executable given or found: allocation sites are not symbolized; pass --exe)\n")

    w("## Allocation by thread\n")
    w("| thread | est. MiB | share |")
    w("|---|---:|---:|")
    tt = sum(t["b"] for t in P["threads"]) or 1
    for t in sorted(P["threads"], key=lambda t: -t["b"])[:15]:
        w(f"| {t['name']} | {mb(t['b'])} | {100 * t['b'] / tt:.1f}% |")
    w("")

    if gcs:
        w("## Collections\n")
        w("| # | time s | live MiB | heap MiB | allocated since the previous MiB | pause ms |")
        w("|---:|---:|---:|---:|---:|---:|")
        show = gcs if len(gcs) <= 30 else gcs[:10] + gcs[-20:]
        for g in show:
            w(f"| {g['n']} | {g['t_ms'] / 1000:.1f} | {mb(g['live'])} | {mb(g['heap'])} | {mb(g['since'])} | {g['pause_us'] / 1000:.1f} |")
        w("")

    per_min = T["b"] / 1048576 / secs * 60
    keep = (cat_b["LONG"] + cat_b["MIXED"] * 0.35) / max(T["b"], 1)
    w("## Reading it for the GC-free plan\n")
    w(f"- Without a collector the program would allocate {per_min / 1024:,.1f} GiB per minute; a 16 GiB machine lasts "
      f"{16 * 1024 / max(per_min, 1):,.1f} minutes.")
    w(f"- If every TEMP allocation stops reaching the heap, about {100 * (1 - cat_b['TEMP'] / max(T['b'], 1)):.0f}% of today's volume remains "
      f"({per_min * (1 - cat_b['TEMP'] / max(T['b'], 1)) / 1024:,.2f} GiB/min): that part must be owned and freed (chunks, worlds), not leaked.")
    w(f"- Data that really stays (LONG plus the kept part of MIXED) is about {100 * keep:.0f}% of the volume.")
    text = "\n".join(out)
    print(text)
    if a.md:
        open(a.md, "w", encoding="utf-8").write(text + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
