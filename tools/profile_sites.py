#!/usr/bin/env python3
"""Top allocation sites of an allocation profile, with the native frames resolved to function names.

    python3 tools/profile_sites.py <profile.json> [--binary PATH] [--top 20] [--frames 4]

The profile (MONO_ALLOC_PROFILE, tools/leak_soak.sh) records, per allocation site, the class, the bytes, the count and
up to 14 return addresses of the native stack (`f`). The `maps` entry says where the program was loaded (start, end,
file offset, path). An address inside the program's own mapping becomes an address in the file:
    address_in_file = address - map_start + map_file_offset
and the ELF program headers (PT_LOAD) turn that file offset into the virtual address `addr2line -f -C -e <binary>` expects:
    virtual = file_offset - p_offset + p_vaddr      (the PT_LOAD segment that contains the file offset)
Bu dönüşüm şarttır: lld ile bağlanan programda kod bölümünün dosya ofseti sanal adresinden 0x1000 küçüktür; dönüşüm
yapılmazsa her ad komşu bir işleve kayar (25 Kasım 2026'ya kadarki site adları bu yüzden yanlıştı). The binary must be the
one that wrote the profile (the same build): another build has other offsets. Frames outside the program (libc, LLVM, ...) are skipped.

Output: one block per site, sorted by bytes. The Java class comes from the profile; the function names show which
generated Rust function allocated it (generated names contain the Java class and method, mangled).
Exit status 1 when the binary is missing or addr2line is not installed.
"""
import argparse
import json
import os
import shutil
import struct
import subprocess
import sys


def program_map(maps, binary):
    """(start, end, file_offset) of the first mapping of the binary, or None."""
    for start, end, off, path in maps:
        if path == binary:
            return int(start, 16), int(end, 16), int(off, 16)
    return None


def load_segments(binary):
    """[(p_offset, p_filesz, p_vaddr)] of the PT_LOAD segments of a 64-bit little-endian ELF file."""
    with open(binary, "rb") as fh:
        hdr = fh.read(64)
        if hdr[:4] != b"\x7fELF" or hdr[4] != 2 or hdr[5] != 1:
            raise SystemExit(f"{binary}: not a 64-bit little-endian ELF file")
        phoff, = struct.unpack_from("<Q", hdr, 0x20)
        phentsize, phnum = struct.unpack_from("<HH", hdr, 0x36)
        fh.seek(phoff)
        table = fh.read(phentsize * phnum)
    segs = []
    for i in range(phnum):
        p_type, _flags, p_offset, p_vaddr, _paddr, p_filesz = struct.unpack_from("<IIQQQQ", table, i * phentsize)
        if p_type == 1:  # PT_LOAD
            segs.append((p_offset, p_filesz, p_vaddr))
    return segs


def to_vaddr(segs, off):
    """Virtual address of a file offset, or None when no PT_LOAD segment contains it."""
    for p_offset, p_filesz, p_vaddr in segs:
        if p_offset <= off < p_offset + p_filesz:
            return off - p_offset + p_vaddr
    return None


def resolve(binary, offsets):
    """Function name for each file offset, one addr2line call for all of them."""
    if not offsets:
        return {}
    cmd = ["addr2line", "-f", "-C", "-e", binary] + [hex(o) for o in offsets]
    out = subprocess.run(cmd, capture_output=True, text=True).stdout.splitlines()
    names = {}
    for i, off in enumerate(offsets):
        fn = out[2 * i] if 2 * i < len(out) else "??"
        names[off] = fn
    return names


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("profile")
    ap.add_argument("--binary", help="the program that wrote the profile (default: the path in the profile's maps)")
    ap.add_argument("--top", type=int, default=20)
    ap.add_argument("--frames", type=int, default=4, help="native frames printed per site")
    a = ap.parse_args()

    if shutil.which("addr2line") is None:
        print("addr2line not found (binutils)", file=sys.stderr)
        return 1
    prof = json.load(open(a.profile))
    maps = prof.get("maps") or []
    binary = a.binary
    if binary is None:
        binary = next((p for _s, _e, _o, p in maps if p.endswith("/prog")), None)
    if binary is None:
        print("no program mapping in the profile; pass --binary", file=sys.stderr)
        return 1
    if not os.path.isfile(binary):
        print(f"binary not found: {binary} (rebuild the same build)", file=sys.stderr)
        return 1
    pm = program_map(maps, binary)
    if pm is None:
        print(f"{binary} is not mapped in the profile", file=sys.stderr)
        return 1
    start, end, foff = pm
    segs = load_segments(binary)

    sites = sorted(prof.get("sites", []), key=lambda r: -r.get("b", 0))[: a.top]
    wanted = set()
    per_site = []
    for r in sites:
        offs = []
        for hx in r.get("f", []):
            addr = int(hx, 16)
            if start <= addr < end:  # inside the program mapping
                off = to_vaddr(segs, addr - start + foff)
                if off is None:
                    continue
                offs.append(off)
                if len(offs) >= a.frames:
                    break
        per_site.append((r, offs))
        wanted.update(offs)
    names = resolve(binary, sorted(wanted))

    total = prof.get("total", {}).get("b", 0) or 1
    for r, offs in per_site:
        print(f"{r.get('b', 0) / 1e6:9.1f} MB {100 * r.get('b', 0) / total:5.1f}%  n={r.get('s', 0):>7}  {r.get('c', '?')}")
        for off in offs:
            print(f"      {names.get(off, '??')}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
