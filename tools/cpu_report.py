#!/usr/bin/env python3
"""MONO_CPU_PROFILE çıktısını (monolith/rt/cpuprof.rs) özetler: zaman nerede harcanıyor?

    MONO_CPU_PROFILE=/tmp/cpu.json ./run.sh          # bir süre oyna (Android: "Ek Ortam Degiskenleri" alanına MONO_CPU_PROFILE=1)
    python3 tools/cpu_report.py /tmp/cpu.json [--exe build/.../rust/prog] [--md rapor.md] [--top 40]

Örnekler süreç CPU süresine bağlıdır (ITIMER_PROF): bir iş parçacığı ne kadar çok CPU yerse o kadar çok örnek düşer. Adresler,
profili yazan çalıştırılabilir dosyanın sembol tablosuyla Java metot adlarına çevrilir (`--exe` verilmezse profilin bellek
eşlemesindeki ilk dosya). Sembolleri olmayan kütüphaneler (sürücü, libc) kütüphane adıyla gösterilir.

Kategoriler:
  Java kodu      çevrilmiş Java metotları (oyun ve kütüphane kodu)
  GC / bellek    toplayıcı, yığın, bölge (mono_rt::gc, heap, region)
  Çalışma zamanı mono_rt'nin geri kalanı ve Rust std/core/alloc
  Yerel kütüphane  GL sürücüsü, libc, libm vb. (kütüphane adıyla)
"""
import argparse
import collections
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from alloc_report import Symbols  # noqa: E402

KINDS = {0: "ana / yerel", 2: "sunucu", 3: "işçi (Worker-)", 4: "diğer Java"}
GC_MARKS = ("mono_rt::gc", "mono_rt::heap", "mono_rt::region", "gc::", "heap::", "region::")


def category(name: str, lib: str | None) -> str:
    if name.startswith("["):
        return f"Yerel kütüphane: {name}"
    if any(m in name for m in GC_MARKS):
        return "GC / bellek"
    if "::" in name:
        return "Çalışma zamanı"
    return "Java kodu"


def java_class(name: str) -> str:
    if "::" in name or name.startswith("["):
        return name
    i = name.rfind(".")
    return name[:i] if i > 0 else name


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("profile")
    ap.add_argument("--exe", help="profili yazan çalıştırılabilir dosya (varsayılan: profilin bellek eşlemesinden)")
    ap.add_argument("--lib", default="libminecraft_mono.so",
                    help="telefondaki profilde --exe ile eşleştirilecek kütüphane adı (--exe dosyası başka adlıysa)")
    ap.add_argument("--slow", metavar="FRAMETIME_CSV",
                    help="yalnızca yavaş anları çözümle: başlatıcının frametime.csv'sinde FPS'i --fps altında kalan 2 sn'lik "
                         "pencerelere düşen örnekler (profil sürüm 2: saniyelik pencereler)")
    ap.add_argument("--fps", type=float, default=30.0, help="--slow eşiği (varsayılan 30)")
    ap.add_argument("--md")
    ap.add_argument("--top", type=int, default=40)
    a = ap.parse_args()
    P = json.load(open(a.profile))
    classes_file = a.profile + ".classes"
    all_classes = open(classes_file).read().split("\n") if os.path.exists(classes_file) else []
    maps = [(int(lo, 16), int(hi, 16), int(off, 16), path) for lo, hi, off, path in P["maps"]]
    exe = a.exe or (maps[0][3] if maps else None)
    if not exe or not os.path.exists(exe):
        print("çalıştırılabilir dosya bulunamadı: --exe verin (profili yazan dosyanın aynısı)", file=sys.stderr)
        return 1
    # Telefonda yazılan profilin eşleme yolları telefondaki yollardır (/data/user/0/.../libminecraft_mono.so): adı --exe
    # ile aynı olanlar yerel dosyayla eşleştirilir (aksi halde tüm motor adresleri tek "kütüphane" olarak toplanırdı).
    maps = [(lo, hi, off, exe if os.path.basename(path) in (os.path.basename(exe), a.lib) else path) for lo, hi, off, path in maps]
    sym = Symbols(exe, maps, all_classes)

    def lib_of(addr: int):
        for lo, hi, _off, path in maps:
            if lo <= addr < hi:
                return os.path.basename(path)
        return None

    rows = P["hist"]
    title = "CPU profili"
    if a.slow:
        if "win" not in P:
            print("profil saniyelik pencere içermiyor (sürüm 2 gerekli)", file=sys.stderr)
            return 1
        import csv
        slow = []
        for r in csv.DictReader(open(a.slow)):
            if int(r["frames"]) / 2.0 < a.fps:
                t = int(r["t_unix_ms"])
                slow.append((t - 2000, t))
        agg = collections.Counter()
        secs = set()
        for t0, kind, pc, cnt in P["win"]:
            if any(lo <= t0 < hi for lo, hi in slow):
                agg[(kind, pc)] += cnt
                secs.add(t0)
        rows = [(k, pc, c) for (k, pc), c in agg.items()]
        P["elapsed_ms"] = len(secs) * 1000  # süre satırı yalnızca seçilen saniyeleri saysın
        title = f"CPU profili: yalnızca yavaş anlar (FPS < {a.fps:g}: {len(slow)} pencere, {len(secs)} sn örnek)"
        if not rows:
            print("eşleşen yavaş an yok", file=sys.stderr)
            return 1
    total = 0
    by_kind = collections.Counter()
    by_func = collections.Counter()
    by_func_kind = collections.defaultdict(collections.Counter)
    by_cat = collections.Counter()
    by_class = collections.Counter()
    for kind, pc, cnt in rows:
        name = sym.name(pc)
        lib = lib_of(pc)
        if name.startswith("0x"):
            # sembolü olmayan adres: bulunduğu kütüphanenin adıyla topla (sürücü, libc, vb.)
            name = f"[{lib}]" if lib else "[bilinmeyen]"
        total += cnt
        by_kind[kind] += cnt
        by_func[name] += cnt
        by_func_kind[name][kind] += cnt
        by_cat[category(name, lib)] += cnt
        by_class[java_class(name)] += cnt
    hz = P["hz"]
    out: list[str] = []
    w = out.append
    w(f"# {title}\n")
    w(f"- süre {P['elapsed_ms'] / 1000:.0f} sn, örnek {total} ({hz} Hz), toplam CPU süresi yaklaşık {total / hz:.0f} sn "
      f"(ortalama {total / hz / max(P['elapsed_ms'] / 1000, 0.001):.2f} çekirdek)\n")
    w("## İş parçacığı türüne göre\n")
    w("| tür | örnek | pay | CPU sn |\n|---|---:|---:|---:|")
    for k, c in by_kind.most_common():
        w(f"| {KINDS.get(k, k)} | {c} | {100 * c / total:.1f}% | {c / hz:.1f} |")
    w("\n## Kategoriye göre\n")
    w("| kategori | pay |\n|---|---:|")
    for k, c in by_cat.most_common(12):
        w(f"| {k} | {100 * c / total:.1f}% |")
    w(f"\n## En çok CPU yiyen {a.top} işlev (kendi süresi)\n")
    w("| işlev | pay | en çok örnek |\n|---|---:|---|")
    for f, c in by_func.most_common(a.top):
        kk = by_func_kind[f].most_common(1)[0][0]
        w(f"| `{f}` | {100 * c / total:.2f}% | {KINDS.get(kk, kk)} |")
    w("\n## Java sınıfına göre (kendi süresi toplamı)\n")
    w("| sınıf | pay |\n|---|---:|")
    for f, c in by_class.most_common(25):
        w(f"| `{f}` | {100 * c / total:.2f}% |")
    for k in sorted(by_kind, key=lambda x: -by_kind[x]):
        sub = collections.Counter()
        for f, kc in by_func_kind.items():
            if kc.get(k):
                sub[f] = kc[k]
        kt = by_kind[k]
        w(f"\n## {KINDS.get(k, k)}: ilk 15 işlev ({kt} örnek)\n")
        w("| işlev | pay (bu türün) |\n|---|---:|")
        for f, c in sub.most_common(15):
            w(f"| `{f}` | {100 * c / kt:.2f}% |")
    text = "\n".join(out) + "\n"
    print(text)
    if a.md:
        open(a.md, "w").write(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
