#!/usr/bin/env python3
"""Retype the undefined cross-shard TLS symbols (cur, processing_refs, throw_depth, attached) from
STT_NOTYPE to STT_TLS in the generated ELF64 objects of a sharded build.

The zig self-hosted backend emits `extern threadlocal` references as NOTYPE UND symbols, which the
zg self-hosted linker tolerates but GNU ld rejects ("TLS definition ... mismatches non-TLS
reference"). When J2Z_ZIG_LD=cc is used to link a sharded build with the system linker, those four
symbols must be marked STT_TLS for ld to bind them to the definitions in shared.o. The symbol table
layout is untouched — exactly four bytes change (one st_info per matching symbol)."""
from __future__ import annotations
import struct
import sys

WANT = {"cur", "processing_refs", "throw_depth", "attached"}
SHN_UNDEF = 0
STT_TLS = 6
SHT_SYMTAB = 2


def patch(path: str) -> int:
    with open(path, "rb") as fh:
        data = bytearray(fh.read())
    if data[:4] != b"\x7fELF" or data[4] != 2 or data[5] != 1:
        raise SystemExit(f"{path}: not ELF64 little-endian")
    e_shoff = struct.unpack_from("<Q", data, 0x28)[0]
    e_shentsize = struct.unpack_from("<H", data, 0x3A)[0]
    e_shnum = struct.unpack_from("<H", data, 0x3C)[0]
    e_shstrndx = struct.unpack_from("<H", data, 0x3E)[0]
    secs = [struct.unpack_from("<IIQQQQIIQQ", data, e_shoff + i * e_shentsize) for i in range(e_shnum)]

    def sec_name(i):
        sh = secs[e_shstrndx]
        off = sh[4] + secs[i][0]
        return bytes(data[off:data.index(b"\0", off)]).decode()

    n = 0
    for i, sh in enumerate(secs):
        if sh[1] != SHT_SYMTAB:
            continue
        sh_offset, sh_size, sh_entsize, str_idx = sh[4], sh[5], sh[9], sh[6]
        str_off = secs[str_idx][4]
        for off in range(sh_offset, sh_offset + sh_size, sh_entsize):
            st_name, st_info, st_other, st_shndx = struct.unpack_from("<IBBH", data, off)
            if st_shndx != SHN_UNDEF:
                continue
            start = str_off + st_name
            name = bytes(data[start:data.index(b"\0", start)]).decode()
            if name in WANT:
                new_info = ((st_info >> 4) << 4) | STT_TLS
                if new_info != st_info:
                    data[off + 4] = new_info
                    n += 1
    with open(path, "wb") as fh:
        fh.write(bytes(data))
    return n


def main() -> int:
    args = sys.argv[1:]
    if not args:
        print("usage: fix_tls_syms.py OBJ ...", file=sys.stderr)
        return 2
    for path in args:
        print(f"{path}: {patch(path)} symbols retyped to STT_TLS")
    return 0


if __name__ == "__main__":
    sys.exit(main())
