//! Küçük bir DWARF 4 okuyucu: bir makine kodu adresini (dönüş adresi) "hangi kaynak dosyanın hangi satırı" zincirine
//! çevirir; satır içine alınmış (inline) fonksiyonlar da ayrı halkalar olarak döner. C9 (docs/PIPELINE_VE_YOL_HARITASI_TR.md
//! bölüm 27) için: yığın izi çağrı başına kayıt tutmadan, yalnızca hata anında makine yığınından çıkarılır.
//!
//! Salağa anlatır gibi:
//!  * Derleyici (`-C debuginfo=line-tables-only`) programın içine iki tablo koyar:
//!      - satır tablosu (`.debug_line`): "şu adresten şu adrese kadar olan kod, şu dosyanın şu satırından geldi";
//!      - satır içi çağrı ağacı (`.debug_info`): "şu adres aralığı aslında X fonksiyonunun gövdesi; X buraya şu dosyanın
//!        şu satırından çağrılıp içe gömüldü".
//!  * Bir adres için önce satır tablosundan en içteki satırı buluruz. Sonra o adresi kapsayan satır içi çağrıları en
//!    derinden en sığa doğru dolaşırız: her biri bir üst halkanın (çağıranın) dosyasını ve satırını verir.
//!  * Sonuç, en içten başlayan bir (dosya, satır) listesidir. Hangi Java metoduna denk geldiğini yan tablo söyler.
//!
//! Üçüncü taraf kütüphane kullanılmaz. Yalnızca rustc'nin ürettiği biçimler (DWARF 4, 64 bit, küçük sonlu) desteklenir;
//! beklenmeyen bir biçimde ayrıştırma durur ve `None` döner (yanlış sonuç vermek yerine).

use std::collections::HashMap;

/// Okunan baytlar üzerinde ilerleyen basit bir imleç.
struct Rd<'a> {
    b: &'a [u8],
    p: usize,
}

impl<'a> Rd<'a> {
    fn new(b: &'a [u8], p: usize) -> Self {
        Rd { b, p }
    }
    fn eof(&self) -> bool {
        self.p >= self.b.len()
    }
    fn u8(&mut self) -> Option<u8> {
        let v = *self.b.get(self.p)?;
        self.p += 1;
        Some(v)
    }
    fn u16(&mut self) -> Option<u16> {
        let s = self.b.get(self.p..self.p + 2)?;
        self.p += 2;
        Some(u16::from_le_bytes([s[0], s[1]]))
    }
    fn u32(&mut self) -> Option<u32> {
        let s = self.b.get(self.p..self.p + 4)?;
        self.p += 4;
        Some(u32::from_le_bytes(s.try_into().ok()?))
    }
    fn u64(&mut self) -> Option<u64> {
        let s = self.b.get(self.p..self.p + 8)?;
        self.p += 8;
        Some(u64::from_le_bytes(s.try_into().ok()?))
    }
    /// İşaretsiz LEB128: her baytın 7 biti sayıya eklenir, en üst bit "devamı var" demektir.
    fn uleb(&mut self) -> Option<u64> {
        let mut v = 0u64;
        let mut sh = 0;
        loop {
            let x = self.u8()?;
            if sh < 64 {
                v |= ((x & 0x7f) as u64) << sh;
            }
            sh += 7;
            if x & 0x80 == 0 {
                return Some(v);
            }
        }
    }
    /// İşaretli LEB128: son baytın 0x40 biti sayının negatif olduğunu söyler.
    fn sleb(&mut self) -> Option<i64> {
        let mut v = 0i64;
        let mut sh = 0;
        loop {
            let x = self.u8()?;
            if sh < 64 {
                v |= ((x & 0x7f) as i64) << sh;
            }
            sh += 7;
            if x & 0x80 == 0 {
                if sh < 64 && x & 0x40 != 0 {
                    v |= -1i64 << sh;
                }
                return Some(v);
            }
        }
    }
    /// Sıfırla biten bayt dizisi (C dizgisi).
    fn cstr(&mut self) -> Option<&'a [u8]> {
        let start = self.p;
        let len = self.b.get(start..)?.iter().position(|&c| c == 0)?;
        self.p = start + len + 1;
        Some(&self.b[start..start + len])
    }
    fn skip(&mut self, n: usize) -> Option<()> {
        if self.p + n > self.b.len() {
            return None;
        }
        self.p += n;
        Some(())
    }
}

/// ELF dosyasındaki DWARF bölümleri (dosyanın içinden dilimler).
pub struct Sections<'a> {
    pub info: &'a [u8],
    pub abbrev: &'a [u8],
    pub line: &'a [u8],
    pub str_: &'a [u8],
    pub ranges: &'a [u8],
}

/// ELF64 küçük sonlu dosyadan bölümleri bulur. Bölüm yoksa ya da dosya beklenen biçimde değilse None.
pub fn elf_sections(f: &[u8]) -> Option<Sections<'_>> {
    if f.get(0..4)? != b"\x7fELF" || *f.get(4)? != 2 || *f.get(5)? != 1 {
        return None; // 64 bit ve küçük sonlu değil
    }
    let mut r = Rd::new(f, 0x28);
    let shoff = r.u64()? as usize;
    let mut r = Rd::new(f, 0x3a);
    let shentsize = r.u16()? as usize;
    let shnum = r.u16()? as usize;
    let shstrndx = r.u16()? as usize;
    // bölüm başlığı: ad (4), tür (4), bayraklar (8), adres (8), dosya konumu (8), boyut (8), ...
    let sec = |i: usize| -> Option<(u32, usize, usize)> {
        let mut r = Rd::new(f, shoff + i * shentsize);
        let name = r.u32()?;
        r.skip(4 + 8 + 8)?;
        let off = r.u64()? as usize;
        let size = r.u64()? as usize;
        Some((name, off, size))
    };
    let (_, stroff, strsize) = sec(shstrndx)?;
    let strtab = f.get(stroff..stroff + strsize)?;
    let mut out: HashMap<&[u8], &[u8]> = HashMap::new();
    for i in 0..shnum {
        let (name, off, size) = sec(i)?;
        let nm = Rd::new(strtab, name as usize).cstr()?;
        if nm.starts_with(b".debug_") {
            out.insert(nm, f.get(off..off + size)?);
        }
    }
    Some(Sections {
        info: out.get(&b".debug_info"[..]).copied()?,
        abbrev: out.get(&b".debug_abbrev"[..]).copied()?,
        line: out.get(&b".debug_line"[..]).copied()?,
        str_: out.get(&b".debug_str"[..]).copied().unwrap_or(&[]),
        ranges: out.get(&b".debug_ranges"[..]).copied().unwrap_or(&[]),
    })
}

// DWARF sabitleri (yalnızca kullanılanlar)
const TAG_COMPILE_UNIT: u64 = 0x11;
const TAG_INLINED: u64 = 0x1d;
const AT_LOW_PC: u64 = 0x11;
const AT_HIGH_PC: u64 = 0x12;
const AT_RANGES: u64 = 0x55;
const AT_STMT_LIST: u64 = 0x10;
const AT_CALL_FILE: u64 = 0x58;
const AT_CALL_LINE: u64 = 0x59;

/// Bir kısaltma (abbrev): bir girdinin etiketi, çocuğu olup olmadığı ve öznitelik listesi (öznitelik, biçim).
struct Abbrev {
    tag: u64,
    children: bool,
    attrs: Vec<(u64, u64, i64)>, // (öznitelik, biçim, implicit_const değeri)
}

fn parse_abbrevs(b: &[u8], off: usize) -> Option<HashMap<u64, Abbrev>> {
    let mut r = Rd::new(b, off);
    let mut m = HashMap::new();
    loop {
        let code = r.uleb()?;
        if code == 0 {
            return Some(m);
        }
        let tag = r.uleb()?;
        let children = r.u8()? != 0;
        let mut attrs = Vec::new();
        loop {
            let at = r.uleb()?;
            let form = r.uleb()?;
            if at == 0 && form == 0 {
                break;
            }
            let ic = if form == 0x21 { r.sleb()? } else { 0 };
            attrs.push((at, form, ic));
        }
        m.insert(code, Abbrev { tag, children, attrs });
    }
}

/// Bir özniteliğin değeri: sayı (adres, sabit, konum) ya da "biz kullanmıyoruz" (atlandı).
enum Val {
    U(u64),
    Skip,
}

/// Biçime göre özniteliği okur ya da atlar. Bilinmeyen biçimde None (ayrıştırma güvenle durur).
/// `version`: birimin DWARF sürümü. DWARF 2'de `ref_addr` adres boyutundadır (Android hedefi için rustc DWARF 2 üretir),
/// 3 ve sonrasında 4 bayttır (32 bit DWARF).
fn read_form(r: &mut Rd, form: u64, addr_size: usize, ic: i64, version: u16) -> Option<Val> {
    Some(match form {
        0x10 if version == 2 => Val::U(if addr_size == 8 { r.u64()? } else { r.u32()? as u64 }), // ref_addr (DWARF 2)
        0x01 => Val::U(if addr_size == 8 { r.u64()? } else { r.u32()? as u64 }), // addr
        0x0b | 0x11 => Val::U(r.u8()? as u64),                                    // data1, ref1
        0x05 | 0x12 => Val::U(r.u16()? as u64),                                   // data2, ref2
        0x06 | 0x13 | 0x17 | 0x0e | 0x10 => Val::U(r.u32()? as u64),              // data4, ref4, sec_offset, strp, ref_addr
        0x07 | 0x14 | 0x20 => Val::U(r.u64()?),                                   // data8, ref8, ref_sig8
        0x0f | 0x15 => Val::U(r.uleb()?),                                         // udata, ref_udata
        0x0d => Val::U(r.sleb()? as u64),                                         // sdata
        0x0c => Val::U(r.u8()? as u64),                                           // flag
        0x19 => Val::U(1),                                                        // flag_present
        0x21 => Val::U(ic as u64),                                                // implicit_const
        0x08 => {
            r.cstr()?;
            Val::Skip
        } // string
        0x18 | 0x09 => {
            let n = r.uleb()? as usize;
            r.skip(n)?;
            Val::Skip
        } // exprloc, block
        0x0a => {
            let n = r.u8()? as usize;
            r.skip(n)?;
            Val::Skip
        } // block1
        0x03 => {
            let n = r.u16()? as usize;
            r.skip(n)?;
            Val::Skip
        } // block2
        0x04 => {
            let n = r.u32()? as usize;
            r.skip(n)?;
            Val::Skip
        } // block4
        0x16 => {
            let f = r.uleb()?;
            return read_form(r, f, addr_size, ic, version);
        } // indirect
        _ => return None,
    })
}

/// Satır içi çağrı: [lo, hi) aralıklarındaki kod, (call_file, call_line) noktasından çağrılıp gömülmüş bir fonksiyondur.
struct Inl {
    ranges: Vec<(u64, u64)>,
    depth: u32,
    call_file: u32,
    call_line: u32,
}

/// Satır tablosunun bir satırı: `addr`'den bir sonraki satırın adresine kadar olan kod (file, line)'dan gelir.
#[derive(Clone, Copy)]
struct Row {
    addr: u64,
    file: u32,
    line: u32,
    end: bool,
}

/// Bir derleme birimi (bir Rust crate'inin bir parçası).
struct Unit {
    ranges: Vec<(u64, u64)>,
    inl: Vec<Inl>,
    rows: Vec<Row>,
    /// dosya numarası -> dosya adının son bileşeni (ör. "shard_05.rs")
    files: Vec<String>,
}

/// Bir birimin yalnızca başlığı: nerede başladığı ve hangi adresleri kapsadığı. Ayrıntılar (satır tablosu ve satır içi
/// ağaç) ilk sorguda bir kez ayrıştırılır (`detail`). 600 MB'lık motorda her şeyi baştan okumak saniyeler ve yüzlerce MB
/// bellek tutardı; böylece yalnızca yığın izinde gerçekten görünen birimler okunur.
struct UnitHead {
    off: usize,
    ranges: Vec<(u64, u64)>,
    detail: std::sync::OnceLock<Option<Unit>>,
}

/// Programın DWARF bilgisi, adres aramaya hazır.
pub struct Dwarf<'a> {
    sections: Sections<'a>,
    units: Vec<UnitHead>,
}

fn read_ranges(ranges: &[u8], off: usize, base: u64) -> Option<Vec<(u64, u64)>> {
    let mut r = Rd::new(ranges, off);
    let mut out = Vec::new();
    let mut base = base;
    loop {
        let a = r.u64()?;
        let b = r.u64()?;
        if a == 0 && b == 0 {
            return Some(out);
        }
        if a == u64::MAX {
            base = b; // taban adresi seçimi
            continue;
        }
        if a != b {
            out.push((base.wrapping_add(a), base.wrapping_add(b)));
        }
    }
}

/// DWARF 4 satır programını çalıştırıp satır listesini ve dosya adlarını üretir.
fn parse_line_program(b: &[u8], off: usize) -> Option<(Vec<Row>, Vec<String>)> {
    let mut r = Rd::new(b, off);
    let unit_len = r.u32()? as usize;
    if unit_len == 0xffff_ffff {
        return None; // 64 bit DWARF desteklenmiyor
    }
    let end = r.p + unit_len;
    let version = r.u16()?;
    if version < 2 || version > 4 {
        return None;
    }
    let hdr_len = r.u32()? as usize;
    let prog_start = r.p + hdr_len;
    let min_inst = r.u8()? as u64;
    if version >= 4 {
        r.u8()?; // komut başına en çok işlem (VLIW); kullanılmıyor
    }
    let _default_is_stmt = r.u8()?;
    let line_base = r.u8()? as i8 as i64;
    let line_range = r.u8()? as u64;
    let opcode_base = r.u8()?;
    let mut std_lens = vec![0u8; opcode_base as usize];
    for i in 1..opcode_base as usize {
        std_lens[i] = r.u8()?;
    }
    // klasör listesi (kullanılmıyor, yalnızca atlanır)
    loop {
        let d = r.cstr()?;
        if d.is_empty() {
            break;
        }
    }
    // dosya listesi: 1'den başlar (0 boş)
    let mut files = vec![String::new()];
    loop {
        let n = r.cstr()?;
        if n.is_empty() {
            break;
        }
        r.uleb()?;
        r.uleb()?;
        r.uleb()?;
        files.push(base_name(n));
    }
    r.p = prog_start;
    let mut rows = Vec::new();
    // satır makinesinin durumu
    let (mut addr, mut file, mut line) = (0u64, 1u32, 1i64);
    while r.p < end {
        let op = r.u8()?;
        if op >= opcode_base {
            // özel komut: adresi ve satırı birlikte ilerletir, bir satır ekler
            let adj = (op - opcode_base) as u64;
            addr = addr.wrapping_add((adj / line_range) * min_inst);
            line += line_base + (adj % line_range) as i64;
            rows.push(Row { addr, file, line: line.max(0) as u32, end: false });
        } else if op == 0 {
            // genişletilmiş komut
            let len = r.uleb()? as usize;
            let next = r.p + len;
            let sub = r.u8()?;
            match sub {
                1 => {
                    rows.push(Row { addr, file, line: line.max(0) as u32, end: true });
                    addr = 0;
                    file = 1;
                    line = 1;
                } // dizinin sonu
                2 => addr = r.u64()?, // adres ata
                3 => {
                    let n = r.cstr()?;
                    files.push(base_name(n));
                } // dosya tanımla
                _ => {}
            }
            r.p = next;
        } else {
            match op {
                1 => rows.push(Row { addr, file, line: line.max(0) as u32, end: false }), // satır kopyala
                2 => addr = addr.wrapping_add(r.uleb()? * min_inst),                       // adresi ilerlet
                3 => line += r.sleb()?,                                                    // satırı ilerlet
                4 => file = r.uleb()? as u32,                                              // dosya ata
                5 => {
                    r.uleb()?;
                } // sütun ata
                6 | 7 | 10 | 11 => {} // is_stmt, basic_block, prologue_end, epilogue_begin
                8 => addr = addr.wrapping_add(((255 - opcode_base as u64) / line_range) * min_inst), // sabit ekle
                9 => addr = addr.wrapping_add(r.u16()? as u64), // sabit adres ilerlet
                12 => {
                    r.uleb()?;
                } // isa
                _ => {
                    // bilinmeyen standart komut: bağımsız değişkenleri atlanır
                    for _ in 0..std_lens[op as usize] {
                        r.uleb()?;
                    }
                }
            }
        }
    }
    Some((rows, files))
}

fn base_name(p: &[u8]) -> String {
    let s = String::from_utf8_lossy(p);
    s.rsplit('/').next().unwrap_or("").to_string()
}

impl<'a> Dwarf<'a> {
    /// Birim başlıklarını okur (her birimin yalnızca ilk girdisi: adres aralıkları). Ayrıştırılamayan birim atlanır.
    pub fn parse(s: Sections<'a>) -> Dwarf<'a> {
        let mut units = Vec::new();
        let mut off = 0usize;
        while off + 11 <= s.info.len() {
            let mut r = Rd::new(s.info, off);
            let Some(len) = r.u32() else { break };
            if len == 0xffff_ffff {
                break;
            }
            let next = off + 4 + len as usize;
            if let Some(ranges) = Self::unit_ranges(&s, off) {
                if !ranges.is_empty() {
                    units.push(UnitHead { off, ranges, detail: std::sync::OnceLock::new() });
                }
            }
            off = next;
        }
        Dwarf { sections: s, units }
    }

    /// Birimin ilk girdisinden (derleme birimi) adres aralıklarını okur.
    fn unit_ranges(s: &Sections, off: usize) -> Option<Vec<(u64, u64)>> {
        let mut r = Rd::new(s.info, off);
        r.u32()?;
        let version = r.u16()?;
        if version < 2 || version > 4 {
            return None;
        }
        let abbr_off = r.u32()? as usize;
        let addr_size = r.u8()? as usize;
        let abbrevs = parse_abbrevs(s.abbrev, abbr_off)?;
        let code = r.uleb()?;
        let ab = abbrevs.get(&code)?;
        if ab.tag != TAG_COMPILE_UNIT {
            return None;
        }
        let (mut lo, mut hi, mut hi_is_off, mut rng) = (None, None, false, None);
        for &(at, form, ic) in &ab.attrs {
            if let Val::U(x) = read_form(&mut r, form, addr_size, ic, version)? {
                match at {
                    AT_LOW_PC => lo = Some(x),
                    AT_HIGH_PC => {
                        hi = Some(x);
                        hi_is_off = form != 0x01;
                    }
                    AT_RANGES => rng = Some(x as usize),
                    _ => {}
                }
            }
        }
        if let Some(o) = rng {
            return read_ranges(s.ranges, o, lo.unwrap_or(0));
        }
        Some(match (lo, hi) {
            (Some(l), Some(h)) => vec![(l, if hi_is_off { l + h } else { h })],
            _ => Vec::new(),
        })
    }

    fn parse_unit(s: &Sections, off: usize) -> Option<Unit> {
        let mut r = Rd::new(s.info, off);
        let len = r.u32()? as usize;
        let end = off + 4 + len;
        let version = r.u16()?;
        if version < 2 || version > 4 {
            return None;
        }
        let abbr_off = r.u32()? as usize;
        let addr_size = r.u8()? as usize;
        let abbrevs = parse_abbrevs(s.abbrev, abbr_off)?;
        let mut unit = Unit { ranges: Vec::new(), inl: Vec::new(), rows: Vec::new(), files: Vec::new() };
        let mut cu_base = 0u64;
        // her derinlikte açık girdinin türü (satır içi mi)
        let mut stack: Vec<bool> = Vec::new();
        while r.p < end {
            let code = r.uleb()?;
            if code == 0 {
                // çocuk listesi bitti: bir üst seviyeye çık
                stack.pop();
                continue;
            }
            let ab = abbrevs.get(&code)?;
            let (mut lo, mut hi, mut hi_is_off, mut rng, mut cf, mut cl, mut stmt) = (None, None, false, None, 0u32, 0u32, None);
            for &(at, form, ic) in &ab.attrs {
                let v = read_form(&mut r, form, addr_size, ic, version)?;
                if let Val::U(x) = v {
                    match at {
                        AT_LOW_PC => lo = Some(x),
                        AT_HIGH_PC => {
                            hi = Some(x);
                            hi_is_off = form != 0x01; // adres değilse "lo'ya göre uzunluk"
                        }
                        AT_RANGES => rng = Some(x as usize),
                        AT_CALL_FILE => cf = x as u32,
                        AT_CALL_LINE => cl = x as u32,
                        AT_STMT_LIST => stmt = Some(x as usize),
                        _ => {}
                    }
                }
            }
            // girdinin adres aralıkları: ya `.debug_ranges` listesi ya da tek bir [lo, hi) aralığı
            let ranges = |base: u64| -> Option<Vec<(u64, u64)>> {
                if let Some(o) = rng {
                    return read_ranges(s.ranges, o, base);
                }
                match (lo, hi) {
                    (Some(l), Some(h)) => Some(vec![(l, if hi_is_off { l + h } else { h })]),
                    _ => Some(Vec::new()),
                }
            };
            match ab.tag {
                TAG_COMPILE_UNIT => {
                    cu_base = lo.unwrap_or(0);
                    unit.ranges = ranges(cu_base)?;
                    if let Some(so) = stmt {
                        if let Some((rows, files)) = parse_line_program(s.line, so) {
                            unit.rows = rows;
                            unit.files = files;
                        }
                    }
                }
                TAG_INLINED => {
                    let rs = ranges(cu_base)?;
                    let d = stack.iter().filter(|&&x| x).count() as u32 + 1;
                    unit.inl.push(Inl { ranges: rs, depth: d, call_file: cf, call_line: cl });
                }
                _ => {}
            }
            if ab.children {
                stack.push(ab.tag == TAG_INLINED);
            }
        }
        // satırları adrese göre sırala (dizi sonları dahil): arama ikili aramayla yapılır
        unit.rows.sort_by_key(|x| (x.addr, !x.end));
        Some(unit)
    }

    /// Tanı için: adres aralığı olan birim sayısı ve ilk birimin ilk aralıkları.
    pub fn debug_summary(&self) -> String {
        let first = self.units.first().map(|u| format!("{:x?}", &u.ranges[..u.ranges.len().min(3)])).unwrap_or_default();
        format!("units={} first={}", self.units.len(), first)
    }

    /// `pc` adresindeki kodun kaynak zinciri, en içten başlayarak: [(dosya adı, satır), ...].
    /// İlk halka satır tablosundan, sonrakiler satır içi çağrıların çağrı noktalarından gelir.
    pub fn locate(&self, pc: u64) -> Option<Vec<(&str, u32)>> {
        let h = self.units.iter().find(|u| u.ranges.iter().any(|&(a, b)| pc >= a && pc < b))?;
        let u = h.detail.get_or_init(|| Self::parse_unit(&self.sections, h.off)).as_ref()?;
        // satır tablosu: adresi pc'den küçük-eşit son satır (dizi sonu değilse)
        let i = u.rows.partition_point(|x| x.addr <= pc);
        if i == 0 {
            return None;
        }
        let row = u.rows[i - 1];
        if row.end {
            return None;
        }
        let mut out = vec![(u.files.get(row.file as usize)?.as_str(), row.line)];
        // pc'yi kapsayan satır içi çağrılar, en derinden en sığa
        let mut inl: Vec<&Inl> = u.inl.iter().filter(|x| x.ranges.iter().any(|&(a, b)| pc >= a && pc < b)).collect();
        inl.sort_by_key(|x| std::cmp::Reverse(x.depth));
        for x in inl {
            out.push((u.files.get(x.call_file as usize).map(|s| s.as_str()).unwrap_or(""), x.call_line));
        }
        Some(out)
    }
}
