//! C9: yığın izi çağrı başına kayıt tutmadan, yalnızca istendiğinde (hata nesnesi kurulurken, StackWalker) makine
//! yığınından çıkarılır. Tasarım: docs/PIPELINE_VE_YOL_HARITASI_TR.md bölüm 27.
//!
//! Salağa anlatır gibi:
//!  1. Derleyici her fonksiyonda "bir önceki çerçevenin adresi" zincirini korur (`-C force-frame-pointers=yes`). Bu zinciri
//!     yukarı doğru izleyince her çerçevenin dönüş adresini, yani "çağrı hangi satırdan yapıldı"yı buluruz.
//!  2. Her dönüş adresini, programın içindeki DWARF tablosuyla (dwarf.rs) "hangi .rs dosyasının hangi satırı" zincirine
//!     çeviririz. Satır içine alınmış fonksiyonlar zincirde ayrı halkalar olarak çıkar.
//!  3. Her (dosya, satır) halkasını, derleyicinin ürettiği yan tabloyla (monolith/linetab.py) Java'nın (sınıf, metot,
//!     dosya, satır) bilgisine çeviririz. Java'ya ait olmayan halkalar (çalışma zamanı, Rust kütüphanesi) atlanır.
//!
//! Kip, üretilen program açılışta tabloları kaydettiğinde (`register`) açılır; aksi halde eski çerçeve kaydı kullanılır.
//! Sınırlama: Rust standart kütüphanesi çerçeve işaretçisiz derlenmiştir; Java çerçeveleri arasına standart kütüphane
//! girerse (ör. Java karşılaştırıcısını geri çağıran bir sıralama) zincir orada kopabilir ve iz kısalır. Yürütücü her adımda
//! zinciri denetler (yığın sınırları içinde, artan, hizalı); bozulunca durur, yanlış adres okumaz.

use crate::dwarf::{elf_sections, Dwarf};
use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_void, CStr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

/// Kip açık mı (üretilen program tabloları kaydetti mi)?
static ON: AtomicBool = AtomicBool::new(false);
/// Rust kaynak dosyası adı (ör. "shard_05.rs") -> o dosyanın yan tablosu (linetab.py biçimi).
static TABLES: OnceLock<HashMap<&'static str, Table>> = OnceLock::new();

/// Bir dosyanın yan tablosu, aramaya hazır.
struct Table {
    /// kayıtlar: (Rust satırı, metot indeksi, Java satırı), Rust satırına göre artan
    entries: Vec<(u32, u32, i32)>,
    /// metotlar: (sınıf, metot, tanımlayıcı, kaynak dosya)
    methods: Vec<(&'static str, &'static str, &'static str, &'static str)>,
}

const NONE: u32 = 0xFFFF_FFFF;

fn parse_table(b: &'static [u8]) -> Option<Table> {
    let u32_at = |p: usize| -> Option<u32> { Some(u32::from_le_bytes(b.get(p..p + 4)?.try_into().ok()?)) };
    if b.get(0..4)? != b"MLT1" {
        return None;
    }
    let n = u32_at(4)? as usize;
    let mut entries = Vec::with_capacity(n);
    for k in 0..n {
        let p = 8 + 12 * k;
        entries.push((u32_at(p)?, u32_at(p + 4)?, u32_at(p + 8)? as i32));
    }
    let mut p = 8 + 12 * n;
    let nm = u32_at(p)? as usize;
    p += 4;
    let mut methods = Vec::with_capacity(nm);
    for _ in 0..nm {
        let mut t = [""; 4];
        for slot in t.iter_mut() {
            let ln = u16::from_le_bytes(b.get(p..p + 2)?.try_into().ok()?) as usize;
            *slot = std::str::from_utf8(b.get(p + 2..p + 2 + ln)?).ok()?;
            p += 2 + ln;
        }
        methods.push((t[0], t[1], t[2], t[3]));
    }
    Some(Table { entries, methods })
}

/// Üretilen program açılışta çağırır: her Rust parçasının adı ve yan tablosu. Bundan sonra yığın izleri bu kipten gelir.
pub fn register(tables: &'static [(&'static str, &'static [u8])]) {
    let mut m = HashMap::new();
    for &(name, b) in tables {
        if let Some(t) = parse_table(b) {
            m.insert(name, t);
        }
    }
    if TABLES.set(m).is_ok() {
        ON.store(true, Ordering::Release);
    }
}

pub fn enabled() -> bool {
    ON.load(Ordering::Acquire)
}

// ------------------------------------------------------------------ kendi ikili dosyamız (Android'de .so)

#[repr(C)]
struct DlInfo {
    dli_fname: *const c_char,
    dli_fbase: *mut c_void,
    dli_sname: *const c_char,
    dli_saddr: *mut c_void,
}

extern "C" {
    fn dladdr(addr: *const c_void, info: *mut DlInfo) -> c_int;
    // region.rs ile aynı imza (-1 adresi hata demektir)
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, off: i64) -> *mut u8;
}

struct Image {
    /// yükleme tabanı: dönüş adresi - taban = dosyadaki sanal adres (konumdan bağımsız program ve .so için)
    base: usize,
    dwarf: Dwarf<'static>,
}

static IMAGE: OnceLock<Option<Image>> = OnceLock::new();

/// Bu modülün içinde olduğu dosyayı (program ya da .so) bulur, belleğe eşler ve DWARF başlıklarını okur. Bir kez yapılır.
fn image() -> Option<&'static Image> {
    IMAGE
        .get_or_init(|| unsafe {
            let mut info: DlInfo = std::mem::zeroed();
            if dladdr(image as *const () as *const c_void, &mut info) == 0 || info.dli_fname.is_null() {
                return None;
            }
            let mut path = CStr::from_ptr(info.dli_fname).to_string_lossy().into_owned();
            // bazı sistemlerde program için yalnızca ad döner: /proc/self/exe kullanılır
            if !path.contains('/') {
                path = std::fs::read_link("/proc/self/exe").ok()?.to_string_lossy().into_owned();
            }
            let f = std::fs::File::open(&path).ok()?;
            let len = f.metadata().ok()?.len() as usize;
            use std::os::fd::AsRawFd;
            // PROT_READ, MAP_PRIVATE: dosya salt okunur eşlenir; dokunulmayan sayfa bellek yemez
            let p = mmap(std::ptr::null_mut(), len, 1, 2, f.as_raw_fd(), 0);
            if p as isize == -1 || p.is_null() {
                return None;
            }
            let data: &'static [u8] = std::slice::from_raw_parts(p as *const u8, len);
            let s = elf_sections(data)?;
            Some(Image { base: info.dli_fbase as usize, dwarf: Dwarf::parse(s) })
        })
        .as_ref()
}

// ------------------------------------------------------------------ yığın yürütücü

/// Çerçeve işaretçisi zincirindeki dönüş adresleri, en içten başlayarak (bu fonksiyonun kendi çerçevesi hariç).
#[inline(never)]
fn return_addresses(max: usize) -> Vec<usize> {
    let mut fp: usize;
    #[cfg(target_arch = "x86_64")]
    unsafe {
        std::arch::asm!("mov {}, rbp", out(reg) fp, options(nomem, nostack, preserves_flags))
    };
    #[cfg(target_arch = "aarch64")]
    unsafe {
        std::arch::asm!("mov {}, x29", out(reg) fp, options(nomem, nostack, preserves_flags))
    };
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        fp = 0;
    }
    let marker = 0u8;
    let lo = &marker as *const u8 as usize;
    let top = crate::heap::cur_stack_top();
    // kayıtlı değilse (beklenmez) 8 MiB'lik bir pencereyle sınırla
    let hi = if top > lo { top } else { lo + (8 << 20) };
    let mut out = Vec::new();
    while out.len() < max {
        // çerçeve işaretçisi yığının içinde ve 8 bayta hizalı olmalı; değilse zincir bitti ya da bozuk
        if fp < lo || fp + 16 > hi || fp & 7 != 0 {
            break;
        }
        let (next, ret) = unsafe { (*(fp as *const usize), *((fp + 8) as *const usize)) };
        if ret == 0 {
            break;
        }
        out.push(ret);
        // bir üst çerçeve her zaman daha yüksek adrestedir; değilse zincir bozuk
        if next <= fp {
            break;
        }
        fp = next;
    }
    out
}

/// Java çerçeveleri, en içten başlayarak: (sınıf iç adı, metot, tanımlayıcı, kaynak dosya, satır).
/// Kip kapalıysa ya da program kendi DWARF bilgisini okuyamıyorsa None (çağıran eski yola döner).
pub fn frames() -> Option<Vec<crate::rt::FrameRow>> {
    if !enabled() {
        return None;
    }
    let tables = TABLES.get()?;
    let img = image()?;
    let mut out = Vec::new();
    for ret in return_addresses(4096) {
        // dönüş adresi çağrıdan SONRAKİ komutu gösterir; çağrının kendisi için 1 eksiği aranır
        let pc = (ret.wrapping_sub(1)).wrapping_sub(img.base) as u64;
        let Some(chain) = img.dwarf.locate(pc) else { continue };
        for (file, line) in chain {
            let Some(t) = tables.get(file) else { continue };
            // satır <= line olan son kayıt
            let i = t.entries.partition_point(|e| e.0 <= line);
            if i == 0 {
                continue;
            }
            let (_, mi, jl) = t.entries[i - 1];
            if mi == NONE {
                continue;
            }
            if let Some(&(c, m, d, f)) = t.methods.get(mi as usize) {
                out.push((c, m, d, f, jl));
            }
        }
    }
    Some(out)
}
