#!/usr/bin/env python3
"""Bölge (kopyalama bekçili) tahsisi için aday sınıfların uygunluğunu denetler (docs/PIPELINE_VE_YOL_HARITASI_TR.md §18).

Bir sınıf uygundur, eğer:
  1. kendisinin ve süper sınıflarının (Object hariç) hiçbir örnek alanı `final` olmaktan çıkmıyorsa (tüm alanlar final),
  2. her `<init>` metodunda alıcı (`this`) başka bir metoda ya da bir alana kaçmıyorsa (kaçış analizi özetinden, `params_escape[0]`).
Kaçış özeti `monolith/escape.py` dökümünden okunur (MONO_ESCAPE_DUMP ile üretilmiş `escape_report.json`).

Kullanım: python3 tools/region_eligible.py <escape_report.json> <classpath: jar:jar:...> <sınıf> [<sınıf> ...]
Sınıf adları iç biçimde (`net/minecraft/world/phys/Vec3`).
"""
import json
import subprocess
import sys


def fields_of(cp: str, cls: str):
    out = subprocess.run(["javap", "-p", "-cp", cp, cls.replace("/", ".")], capture_output=True, text=True).stdout
    sup = None
    inst = []
    for line in out.splitlines():
        line = line.strip().rstrip(";")
        if line.startswith(("public ", "final ", "abstract ", "class ", "interface ")) and " class " in " " + line + " ":
            toks = line.replace("{", " ").split()
            if "extends" in toks:
                sup = toks[toks.index("extends") + 1].split("<")[0].replace(".", "/")
        elif "(" not in line and line and not line.startswith(("Compiled", "}")):
            toks = line.split()
            if "static" in toks:
                continue
            inst.append(("final" in toks, toks[-1]))
    return sup, inst


def main() -> int:
    if len(sys.argv) < 4:
        print(__doc__)
        return 2
    rep = json.load(open(sys.argv[1]))["dump"]
    cp = sys.argv[2]
    bad = 0
    for cls in sys.argv[3:]:
        reasons = []
        c = cls
        while c and c != "java/lang/Object":
            sup, inst = fields_of(cp, c)
            for fin, name in inst:
                if not fin:
                    reasons.append(f"{c.split('/')[-1]}.{name} final degil")
            c = sup
        inits = {k: v for k, v in rep.items() if k.startswith(cls + ".<init>")}
        if not inits:
            reasons.append("<init> kaçış özeti yok (analiz kapsamı dışı)")
        for k, v in inits.items():
            if v["params_escape"] and v["params_escape"][0]:
                reasons.append(f"{k.split('.', 1)[1]}: this kaçıyor")
        print(("UYGUN   " if not reasons else "UYGUN DEGIL ") + cls + ("" if not reasons else "  <- " + "; ".join(reasons[:3])))
        bad += bool(reasons)
    return 0


if __name__ == "__main__":
    sys.exit(main())
