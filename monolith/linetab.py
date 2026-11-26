"""C9 yan tablosu: üretilen Rust parçasının her satırının hangi Java metoduna ve satırına ait olduğu.

docs/PIPELINE_VE_YOL_HARITASI_TR.md bölüm 27. Hata anında makine yığını yürünür; her dönüş adresi DWARF satır tablosuyla
(dosya, Rust satırı) zincirine çevrilir (monolith/rt/dwarf.rs). Bu modülün ürettiği tablo da (dosya, Rust satırı) -> Java
(sınıf, metot, tanımlayıcı, kaynak dosya, satır) çevirisini yapar.

Salağa anlatır gibi:
  * Derleyici her Java metodu için `static FI: rt::FrameInfo = ...` (sınıf/metot/dosya bilgisi) ve Java satırı değiştikçe
    `rt::set_line_s(N);` yazar. Bu modül metni baştan sona okur; her satırda "şu an hangi metottayım, hangi Java
    satırındayım" bilgisini takip eder ve değiştiği yerleri tabloya yazar.
  * `strip=True` iken (C9 kipi) bu çalışma anı kayıtları koddan silinir: yerlerine boş bir açıklama satırı konur, böylece
    satır numaraları kaymaz ve tablo DWARF'ın gördüğü satırlarla birebir eşleşir. `enter_frame_s` yerine yalnızca yığın
    taşması denetimi (`rt::stack_check()?`) kalır.
  * Java metodu olmayan fonksiyonlar (dışa açık sarmalayıcılar, yamalar) "Java değil" diye işaretlenir; yığın izinde atlanır.

Tablo biçimi (küçük sonlu):
  b"MLT1", u32 kayıt sayısı, her kayıt: u32 Rust satırı (1'den başlar), u32 metot indeksi (0xffffffff: Java değil),
  i32 Java satırı (-1: bilinmiyor); ardından u32 metot sayısı, her metot için 4 dizgi (sınıf, metot, tanımlayıcı, dosya),
  her biri u16 uzunluk + UTF-8 bayt.
"""
from __future__ import annotations

import re
import struct

_FI = re.compile(r'^\s*static FI: rt::FrameInfo = rt::FrameInfo \{ class: (".*?"), method: (".*?"), desc: (".*?"), file: (".*?") \};\s*$')
_ENTER = re.compile(r'^(\s*)let _frame = rt::enter_frame_s\(&FI\)\?;\s*$')
_LINE = re.compile(r'^(\s*)rt::set_line_s\((-?\d+)\);\s*$')
# yeni bir fonksiyonun başlangıcı (sütun 0): `fn`, `#[inline] fn`, `pub fn`, `#[no_mangle] pub extern "C" fn` ...
_FN_START = re.compile(r'^(?:#\[[^\]]*\]\s*)*(?:pub(?:\([^)]*\))?\s+)?(?:unsafe\s+)?(?:extern\s+"C"\s+)?fn\s')

NONE = 0xFFFFFFFF


def _unq(s: str) -> str:
    """rustgen.rstr'nin tersi: çift tırnaklı Rust dizgisini çözer (yalnızca \\" \\\\ \\n \\t \\u{..} kaçışları)."""
    s = s[1:-1]
    out, i = [], 0
    while i < len(s):
        c = s[i]
        if c == "\\" and i + 1 < len(s):
            n = s[i + 1]
            if n == "u" and s[i + 2:i + 3] == "{":
                j = s.index("}", i)
                out.append(chr(int(s[i + 3:j], 16)))
                i = j + 1
                continue
            out.append({"n": "\n", "t": "\t", "r": "\r", "0": "\0"}.get(n, n))
            i += 2
            continue
        out.append(c)
        i += 1
    return "".join(out)


def build(text: str, strip: bool = False) -> tuple[str, bytes]:
    """Parça metnini okur; (yeni metin, tablo baytları) döndürür. strip=False iken metin aynen döner."""
    lines = text.split("\n")
    methods: list[tuple[str, str, str, str]] = []
    index: dict[tuple, int] = {}
    entries: list[tuple[int, int, int]] = []
    cur, cur_line = NONE, -1

    def mark(ln: int):
        if not entries or entries[-1][1:] != (cur, cur_line):
            entries.append((ln, cur, cur_line))

    for i, ln in enumerate(lines):
        no = i + 1
        if _FN_START.match(ln):
            # yeni fonksiyon: Java metodu olduğunu FI satırı söyleyene kadar "Java değil"
            cur, cur_line = NONE, -1
            mark(no)
            continue
        m = _FI.match(ln)
        if m:
            key = tuple(_unq(g) for g in m.groups())
            if key not in index:
                index[key] = len(methods)
                methods.append(key)
            cur, cur_line = index[key], -1
            mark(no)
            if strip:
                lines[i] = "    // C9: FI"
            continue
        m = _ENTER.match(ln)
        if m:
            if strip:
                lines[i] = f"{m.group(1)}rt::stack_check()?;"
            continue
        m = _LINE.match(ln)
        if m:
            cur_line = int(m.group(2))
            mark(no)
            if strip:
                lines[i] = f"{m.group(1)}// L{m.group(2)}"
            continue
    out = bytearray(b"MLT1")
    out += struct.pack("<I", len(entries))
    for no, mi, jl in entries:
        out += struct.pack("<IIi", no, mi, jl)
    out += struct.pack("<I", len(methods))
    for t in methods:
        for s in t:
            b = s.encode("utf-8")
            out += struct.pack("<H", len(b)) + b
    return ("\n".join(lines) if strip else text), bytes(out)


def lookup(table: bytes, rust_line: int):
    """Test ve tanı için Python tarafı arama: Rust satırının (sınıf, metot, tanımlayıcı, dosya, Java satırı) ya da None."""
    n = struct.unpack_from("<I", table, 4)[0]
    ents = [struct.unpack_from("<IIi", table, 8 + 12 * k) for k in range(n)]
    p = 8 + 12 * n
    nm = struct.unpack_from("<I", table, p)[0]
    p += 4
    methods = []
    for _ in range(nm):
        t = []
        for _ in range(4):
            ln = struct.unpack_from("<H", table, p)[0]
            t.append(table[p + 2:p + 2 + ln].decode("utf-8"))
            p += 2 + ln
        methods.append(tuple(t))
    best = None
    for no, mi, jl in ents:
        if no <= rust_line:
            best = (mi, jl)
        else:
            break
    if best is None or best[0] == NONE:
        return None
    return methods[best[0]] + (best[1],)
