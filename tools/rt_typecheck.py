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

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, ROOT)


def main() -> int:
    from monolith.rustgen import rt_deps_flags

    rt_deps_flags()  # builds rt_deps (crc32fast, cfg-if) offline first; the check below reuses those artifacts
    # çalışma dizini rt_deps: cargo, gömülü kaynak yapılandırmasını (rt_deps/.cargo) çalışma dizininden okur
    r = subprocess.run(["cargo", "check", "--release", "--offline", "--lib", "--manifest-path", "check/Cargo.toml",
                        "--target-dir", "target"], cwd=os.path.join(ROOT, "rt_deps"), capture_output=True, text=True)
    if r.returncode != 0:
        sys.stderr.write(r.stderr)
        print("FAIL: runtime type-check (cargo check of monolith/rt)")
        return 1
    print("PASS: runtime type-checks for this machine (cargo check of monolith/rt)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
