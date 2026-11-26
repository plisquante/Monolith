//! Bölge (region) tahsisi: değişmez, kimliği önemsiz küçük nesneler (Vec3, AABB, Optional...) için, yığına (heap) değil iş
//! parçacığına özel bir bump alanına tahsis. Tasarım: docs/PIPELINE_VE_YOL_HARITASI_TR.md bölüm 18.
//!
//! Özet, salağa anlatır gibi:
//!  * Bölge nesneleri "taslak kâğıdı"na yazılır. Taslak kâğıtlar toplama sırasında, kimse bakmıyorsa topluca atılır.
//!  * Bir bölge nesnesi kalıcı bir yere (alan, dizi, statik) yazılacağı an bekçi (`promote`) onun fotokopisini yığına çıkarır
//!    ve kopyanın adresini yazar. Böylece yığından bölgeye hiçbir referans olmaz; bölgeyi yalnızca yığınlar (stack) gösterir.
//!  * Kimlik korunur: asıl nesnenin başlığına "kopyası şurada" yazılır (iletme işaretçisi). `==`, `monitorenter` ve
//!    `identityHashCode` önce bu iletmeyi izler (`canon`), böylece asıl ve kopya her zaman aynı nesne sayılır.
//!  * Bölge parçaları (64 KiB) yalnızca ana toplayıcı çalışırken, yığın kelimeleriyle (ve onların izlediği bölge nesneleriyle)
//!    gösterilmeyenler için serbest bırakılır.
//!
//! Her bölge nesnesinin önünde 16 baytlık bir önek vardır: ilk 8 bayt iletme adresi. `Obj` başlığı önekten sonra gelir.
//! Linux ve Android'de etkindir (`mmap` ile ayrılmış sanal aralık). Diğer platformlarda `enabled()` false döner ve
//! `alloc_region` normal yığın tahsisine düşer; hiçbir davranış değişmez.

use crate::rt::{Class, Obj, Ref};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

/// Başlıktaki bit: nesne bir bölge nesnesidir.
pub const GC_REGION: u32 = 0x10;
/// Başlıktaki bit: nesne yığına kopyalandı, önekteki iletme adresi geçerlidir.
pub const GC_FWD: u32 = 0x20;
/// Başlıktaki bit: bu toplama döngüsünde ziyaret edildi (kutup `RSENSE` ile her döngüde döner).
pub const GC_RVIS: u32 = 0x40;

// Sanal aralık; dokunulmayan sayfa bellek yemez. Android'de 8 GiB (telefon çekirdeklerinin sanal adres alanı daha dar
// olabilir ve uygulamaların toplam sanal bellek sınırı vardır), masaüstünde 32 GiB.
#[cfg(target_os = "android")]
const RANGE: usize = 8 << 30;
#[cfg(not(target_os = "android"))]
const RANGE: usize = 32 << 30;
const CHUNK: usize = 64 << 10;
const NCHUNKS: usize = RANGE / CHUNK;
const PREFIX: usize = 16;

/// Bölge aralığı [LO, HI). Etkin değilse LO = usize::MAX, HI = 0 (hiçbir adres aralıkta değildir).
static LO: AtomicUsize = AtomicUsize::new(usize::MAX);
static HI: AtomicUsize = AtomicUsize::new(0);
/// Hiç kullanılmamış parçaların bump sayacı (parça indeksi).
static NEXT: AtomicUsize = AtomicUsize::new(0);
/// 1 bit / 16 bayt: o adreste bir bölge nesnesi başlar mı? (tutucu taramada geçerlilik denetimi)
static BITS: AtomicUsize = AtomicUsize::new(0);
/// Parça başına bir bayt: 0 kullanılmıyor, 1 kullanımda, 2 bu döngüde canlı bulundu.
static FLAGS: AtomicUsize = AtomicUsize::new(0);
static FREE: Mutex<Vec<usize>> = Mutex::new(Vec::new());
/// Her toplama döngüsünde (ve parça boşaltılınca) artar: iş parçacıkları elindeki parçayı bırakıp yeni parça alır.
static EPOCH: AtomicUsize = AtomicUsize::new(1);
/// Bir toplama döngüsü sürüyor: bu sırada alınan yeni parçalar canlı sayılır (siyah tahsis).
static ACTIVE: AtomicBool = AtomicBool::new(false);
/// `GC_RVIS` kutbu; `cycle_begin` her döngüde çevirir.
static RSENSE: AtomicUsize = AtomicUsize::new(0);
/// Ölçüm sayaçları (MONO_GC_PHASES açıkken her döngü sonunda yazılır).
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static PROMOTED: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    // (bump imleci, parça sonu, dönem)
    static TL: Cell<(usize, usize, usize)> = const { Cell::new((0, 0, 0)) };
}

#[cfg(any(target_os = "linux", target_os = "android"))]
extern "C" {
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, off: i64) -> *mut u8;
    fn madvise(addr: *mut u8, len: usize, advice: i32) -> i32;
    fn mprotect(addr: *mut u8, len: usize, prot: i32) -> i32;
}

/// MONO_REGION_QUARANTINE=1 (tanı kipi): serbest bırakılan parça bir daha kullanılmaz ve erişime kapatılır (PROT_NONE).
/// Bayat bir bölge referansı (parçası serbest bırakılmış nesne) kullanıldığı anda, kullanan kodun içinde SIGSEGV olur ve
/// çökme raporundaki adres bölge aralığındadır; böylece referansı kimin tuttuğu bulunur. Kapalıyken davranış aynıdır.
fn quarantine() -> bool {
    static Q: OnceLock<bool> = OnceLock::new();
    *Q.get_or_init(|| std::env::var("MONO_REGION_QUARANTINE").is_ok_and(|v| !v.is_empty() && v != "0"))
}

fn map_zeroed(len: usize) -> usize {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    unsafe {
        // PROT_READ|PROT_WRITE, MAP_PRIVATE|MAP_ANONYMOUS|MAP_NORESERVE
        let p = mmap(std::ptr::null_mut(), len, 3, 0x2 | 0x20 | 0x4000, -1, 0);
        if p as isize == -1 {
            return 0;
        }
        p as usize
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        let _ = len;
        0
    }
}

/// Bölgeyi bir kez kurar. Başarısızsa (ya da Linux değilse) bölge kapalı kalır.
fn init() -> bool {
    static ONCE: OnceLock<bool> = OnceLock::new();
    *ONCE.get_or_init(|| {
        if std::env::var("MONO_REGION").is_ok_and(|v| v == "0") {
            return false;
        }
        let base = map_zeroed(RANGE);
        let bits = map_zeroed(RANGE / 16 / 8);
        let flags = map_zeroed(NCHUNKS);
        if base == 0 || bits == 0 || flags == 0 {
            return false;
        }
        BITS.store(bits, Ordering::Relaxed);
        FLAGS.store(flags, Ordering::Relaxed);
        HI.store(base + RANGE, Ordering::Release);
        LO.store(base, Ordering::Release);
        true
    })
}

pub fn enabled() -> bool {
    init()
}

/// Adres bölge aralığında mı? (Nesne olduğunu göstermez; yalnızca aralık denetimidir.)
#[inline(always)]
pub fn in_region(addr: usize) -> bool {
    addr >= LO.load(Ordering::Relaxed) && addr < HI.load(Ordering::Relaxed)
}

#[inline(always)]
fn chunk_of(addr: usize) -> usize {
    (addr - LO.load(Ordering::Relaxed)) / CHUNK
}

#[inline(always)]
unsafe fn flag(idx: usize) -> *mut u8 {
    (FLAGS.load(Ordering::Relaxed) as *mut u8).add(idx)
}

#[inline(always)]
unsafe fn bit_ptr(addr: usize) -> (*mut u8, u8) {
    let g = (addr - LO.load(Ordering::Relaxed)) >> 4;
    ((BITS.load(Ordering::Relaxed) as *mut u8).add(g >> 3), 1u8 << (g & 7))
}

/// `addr` bir bölge nesnesinin başlangıcı mı? (tutucu tarama için)
pub fn is_object(addr: usize) -> bool {
    if !in_region(addr) || addr & 15 != 0 {
        return false;
    }
    unsafe {
        let (p, m) = bit_ptr(addr);
        *p & m != 0
    }
}

fn new_chunk() -> usize {
    let reused = FREE.lock().unwrap().pop();
    let idx = match reused {
        Some(i) => i,
        None => {
            let i = NEXT.fetch_add(1, Ordering::Relaxed);
            if i >= NCHUNKS {
                return 0; // aralık doldu: çağıran yığına düşer
            }
            i
        }
    };
    let base = LO.load(Ordering::Relaxed) + idx * CHUNK;
    unsafe {
        // yeniden kullanılan parçada eski nesneler ve bitler kalmış olabilir
        std::ptr::write_bytes(base as *mut u8, 0, CHUNK);
        let (bp, _) = bit_ptr(base);
        std::ptr::write_bytes(bp, 0, CHUNK / 16 / 8);
        *flag(idx) = if ACTIVE.load(Ordering::Acquire) { 2 } else { 1 };
    }
    base
}

/// Bölgede bir `class` nesnesi tahsis eder (sıfırlanmış, başlığı yazılmış). Bölge kapalıysa ya da dolduysa yığına düşer.
pub fn alloc_region(class: &'static Class) -> Ref {
    let sz = (class.size.max(24) + 15) & !15;
    let total = PREFIX + sz;
    if total > CHUNK / 8 || !init() {
        return crate::rt::alloc(class);
    }
    let ep = EPOCH.load(Ordering::Acquire);
    let (mut cur, mut end, e) = TL.with(|c| c.get());
    if e != ep || cur + total > end {
        let base = new_chunk();
        if base == 0 {
            return crate::rt::alloc(class);
        }
        cur = base;
        end = base + CHUNK;
    }
    let obj = cur + PREFIX;
    ALLOCS.fetch_add(1, Ordering::Relaxed);
    TL.with(|c| c.set((cur + total, end, ep)));
    unsafe {
        let o = obj as *mut Obj;
        (*o).class = class;
        (*o).gc = GC_REGION;
        let (p, m) = bit_ptr(obj);
        *p |= m;
    }
    Ref(obj as *mut Obj)
}

/// İletme işaretçisini izler: `p` yığına kopyalandıysa kopyayı, aksi halde `p`'yi döndürür.
#[inline(always)]
pub unsafe fn canon(p: *mut Obj) -> *mut Obj {
    if in_region(p as usize) && (*p).gc & GC_FWD != 0 {
        return *((p as *mut u8).sub(PREFIX) as *const usize) as *mut Obj;
    }
    p
}

/// Kalıcı bir yere yazılacak bölge nesnesini yığına kopyalar (derin) ve kopyayı döndürür. Aynı nesne tekrar gelirse aynı kopya.
#[cold]
pub unsafe fn promote(p: *mut Obj) -> *mut Obj {
    if (*p).gc & GC_FWD != 0 {
        return *((p as *mut u8).sub(PREFIX) as *const usize) as *mut Obj;
    }
    let class = (*p).class;
    let size = class.size.max(24);
    PROMOTED.fetch_add(1, Ordering::Relaxed);
    let h = crate::heap::alloc_obj(class, size);
    // başlıktan (16 bayt) sonrasını kopyala; hash ve sınıf başlığı yeniden yazıldı
    std::ptr::copy_nonoverlapping((p as *const u8).add(16), (h as *mut u8).add(16), size - 16);
    (*h).hash = (*p).hash;
    // bölge nesneleri değişmezdir; içlerindeki bölge referansları da yığına çıkarılır
    for &off in class.ref_offsets {
        let slot = (h as *mut u8).add(off as usize) as *mut Ref;
        let v = (*slot).0;
        if in_region(v as usize) {
            *slot = Ref(promote(v));
        }
    }
    *((p as *mut u8).sub(PREFIX) as *mut usize) = h as usize;
    (*p).gc |= GC_FWD;
    h
}

/// Yazma bekçisi: `v` bölgedeyse yığın kopyasını döndürür, değilse `v`'yi.
#[inline(always)]
pub fn guard(v: Ref) -> Ref {
    if in_region(v.0 as usize) {
        Ref(unsafe { promote(v.0) })
    } else {
        v
    }
}

/// Bölge hiç kullanıldı mı? Kullanılmadıysa hiçbir bölge adresi yoktur ve tarama/kopyalama döngüleri atlanabilir.
#[inline(always)]
pub fn active() -> bool {
    LO.load(Ordering::Relaxed) != usize::MAX
}

/// `slot` adresine yazılacak `v` için bekçi: bölge nesnesinin kendi alanına yazılıyorsa (kurucu) olduğu gibi, aksi halde
/// kalıcı bir yer olduğundan `v` bölgedeyse yığına kopyalanır.
#[inline(always)]
pub fn guard_into(slot: usize, v: Ref) -> Ref {
    if in_region(v.0 as usize) && !in_region(slot) {
        Ref(unsafe { promote(v.0) })
    } else {
        v
    }
}

/// Yazılmış değeri beklenen değerle karşılaştırırken (CAS) beklenen değer bölge nesnesiyse kopyasına çevrilir.
#[inline(always)]
pub fn canon_ref(v: Ref) -> Ref {
    Ref(unsafe { canon(v.0) })
}

/// `==` (if_acmpeq/ne): iletmeyi izleyerek karşılaştırır, asıl ve kopya aynı nesnedir.
#[inline(always)]
pub fn ref_eq(a: Ref, b: Ref) -> bool {
    a.0 == b.0 || (in_region(a.0 as usize) || in_region(b.0 as usize)) && unsafe { canon(a.0) == canon(b.0) }
}

// ------------------------------------------------------------------ ana toplayıcı ile bütünleşme

/// Yeni bir toplama döngüsü başlıyor: ziyaret kutbu döner, parçalar "kanıtlanmamış" olur, iş parçacıkları parça bırakır.
pub fn cycle_begin() {
    if !active() {
        return; // bölge hiç kullanılmadı: ne ayrılmış aralık ne de iş var
    }
    RSENSE.fetch_xor(GC_RVIS as usize, Ordering::Relaxed);
    ACTIVE.store(true, Ordering::Release);
    EPOCH.fetch_add(1, Ordering::AcqRel);
    let n = NEXT.load(Ordering::Relaxed).min(NCHUNKS);
    unsafe {
        for i in 0..n {
            if *flag(i) == 2 {
                *flag(i) = 1;
            }
        }
    }
}

/// Toplayıcı bir bölge adresiyle karşılaştı (yığın kelimesi ya da bölge nesnesinin alanı). Geçerli bir nesneyse parçasını
/// canlı işaretler ve, daha önce ziyaret edilmediyse, içindekileri izlenmek üzere `work`'e iter.
#[inline]
pub fn note(addr: usize, work: &mut Vec<*mut Obj>) {
    if !is_object(addr) {
        return;
    }
    unsafe {
        *flag(chunk_of(addr)) = 2;
        let o = addr as *mut Obj;
        let sense = RSENSE.load(Ordering::Relaxed) as u32;
        if (*o).gc & GC_RVIS != sense {
            (*o).gc = ((*o).gc & !GC_RVIS) | sense;
            work.push(o);
        }
    }
}

/// Toplama bitti: canlı bulunmayan parçalar serbest bırakılır.
pub fn cycle_end() {
    if !active() {
        return; // bölge hiç kullanılmadı: ne ayrılmış aralık ne de iş var
    }
    ACTIVE.store(false, Ordering::Release);
    let n = NEXT.load(Ordering::Relaxed).min(NCHUNKS);
    let mut freed = Vec::new();
    unsafe {
        for i in 0..n {
            if *flag(i) == 1 {
                *flag(i) = 0;
                freed.push(i);
                // not: parçalar işletim sistemine burada geri verilmez (parça başına bir madvise, binlerce parçada duraklamayı
                // uzatır); yeniden kullanımda `new_chunk` sıfırlar. Bölgenin tepe kaplamı (birkaç yüz MB) RSS'te kalır.
            }
        }
    }
    let nfreed = freed.len();
    if quarantine() {
        // tanı kipi: parçalar yeniden kullanılmaz; erişime kapatılır, belleği işletim sistemine geri verilir
        #[cfg(any(target_os = "linux", target_os = "android"))]
        for &i in &freed {
            let base = LO.load(Ordering::Relaxed) + i * CHUNK;
            unsafe {
                // nesne bitleri silinir: yığında kalmış ölü bir kelime toplayıcıyı kapalı sayfaya götürmesin (yanlış alarm)
                let (bp, _) = bit_ptr(base);
                std::ptr::write_bytes(bp, 0, CHUNK / 16 / 8);
                madvise(base as *mut u8, CHUNK, 4); // MADV_DONTNEED
                mprotect(base as *mut u8, CHUNK, 0); // PROT_NONE
            }
        }
    } else if !freed.is_empty() {
        FREE.lock().unwrap().extend(freed);
    }
    if std::env::var_os("MONO_GC_PHASES").is_some() {
        eprintln!("[mono:region] allocs={} promoted={} chunks_freed={} chunks_in_use={}", ALLOCS.load(Ordering::Relaxed), PROMOTED.load(Ordering::Relaxed), nfreed, chunks_in_use());
    }
    EPOCH.fetch_add(1, Ordering::AcqRel);
}

/// Kullanımdaki bölge parçası sayısı (ölçüm ve test için).
pub fn chunks_in_use() -> usize {
    if !active() {
        return 0;
    }
    let n = NEXT.load(Ordering::Relaxed).min(NCHUNKS);
    unsafe { (0..n).filter(|&i| *flag(i) != 0).count() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rt::{ArrCache, Class, Mirror};

    // testler küresel bölge durumunu paylaşır: sırayla çalışsınlar
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    // 16 baytlık başlık + iki 8 baytlık alan: biri bir referans (ref_offsets), biri düz değer
    static REFS: [u32; 1] = [16];
    static PAIR: Class = Class {
        name: "test/Pair", super_: None, size: 32, vtable: &[], elem: 0, elem_class: None, array_cache: ArrCache::NEW, itab: &[],
        ifaces: &[], fields: &[], ref_offsets: &REFS, static_refs: &[], methods: &[], refl: None, mirror: Mirror::NEW, kind: 0,
        access: 0, init: None, enum_values: None,
    };

    #[test]
    fn region_alloc_forward_and_identity() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        if !enabled() {
            return; // Linux dışı: bölge kapalı
        }
        let a = alloc_region(&PAIR);
        let b = alloc_region(&PAIR);
        assert!(in_region(a.0 as usize) && in_region(b.0 as usize) && is_object(a.0 as usize));
        unsafe {
            *((a.0 as *mut u8).add(16) as *mut Ref) = b; // a.alan = b (bölge -> bölge)
            *((a.0 as *mut u8).add(24) as *mut i64) = 77;
            let h = promote(a.0);
            assert!(!in_region(h as usize), "kopya yığında olmalı");
            assert_eq!(*((h as *const u8).add(24) as *const i64), 77, "değer alanı kopyalandı");
            let inner = *((h as *const u8).add(16) as *const Ref);
            assert!(!in_region(inner.0 as usize), "içteki bölge referansı da yığına çıkarıldı");
            assert_eq!(promote(a.0), h, "aynı nesne ikinci kez aynı kopyayı verir");
            assert_eq!(canon(a.0), h, "iletme izlenir");
            assert!(ref_eq(a, Ref(h)), "asıl ve kopya aynı nesnedir");
            assert!(!ref_eq(a, b), "farklı nesneler eşit değildir");
        }
    }

    #[test]
    fn unreferenced_chunks_are_freed_and_referenced_kept() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        if !enabled() {
            return;
        }
        let keep = alloc_region(&PAIR);
        for _ in 0..5000 {
            alloc_region(&PAIR); // birkaç parça doldur
        }
        let before = chunks_in_use();
        assert!(before >= 3, "birden çok parça kullanımda olmalı ({before})");
        cycle_begin();
        let mut work = Vec::new();
        note(keep.0 as usize, &mut work); // yığın kelimesi `keep`'i gösteriyor
        assert_eq!(work.len(), 1);
        note(keep.0 as usize, &mut work); // ikinci kez ziyaret edilmez
        assert_eq!(work.len(), 1);
        cycle_end();
        let after = chunks_in_use();
        assert_eq!(after, 1, "yalnızca `keep`'in parçası kalır (önce {before}, sonra {after})");
        // serbest bırakılan parçalar yeniden kullanılır ve sıfırlanmış gelir
        let fresh = alloc_region(&PAIR);
        unsafe { assert_eq!(*((fresh.0 as *const u8).add(24) as *const i64), 0) };
    }
}
