#!/usr/bin/env python3
"""Tip denetimi: Rust çalışma zamanı (monolith/rt) bu makinenin hedefi için derlenebiliyor mu?

    python3 tools/rt_typecheck.py

rt_deps kasasını cargo ile çevrimdışı derler (monolith/rustgen.py rt_deps_flags), cargo'nun ürettiği tüm rlib'leri
rustc'ye verir ve yalnızca meta veriyi üretir (--emit=metadata). Hata varsa 1 ile çıkar. CI'da her işletim sistemi
bu betiği çalıştırır; yerelde aynı komut aynı sonucu verir.
"""
import os
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, ROOT)


def main() -> int:
    from monolith.rustgen import rt_deps_flags

    flags = rt_deps_flags()
    out = os.path.join(tempfile.mkdtemp(prefix="mono_rt_"), "mono_rt.rmeta")
    cmd = ["rustc", "--edition", "2021", "--crate-type", "lib", "--crate-name", "mono_rt", "--emit=metadata",
           *flags, os.path.join(ROOT, "monolith", "rt", "lib.rs"), "-o", out]
    r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
    errors = [l for l in r.stderr.splitlines() if l.startswith("error")]
    if r.returncode != 0:
        sys.stderr.write(r.stderr[-6000:])
        print(f"FAIL: runtime type-check ({len(errors)} errors)")
        return 1
    print(f"PASS: runtime type-checks for this machine ({os.path.relpath(out, ROOT)})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
