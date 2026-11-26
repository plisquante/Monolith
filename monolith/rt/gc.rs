//! Durdur-dünya (stop-the-world) ayrık-dilimli (segregated slab) İşaretle-Süpür çöp toplayıcı, artı iş parçacığına özel kaçmayan bump arena.
//! Acemi notu: İşaretle-Süpür, canlı nesneleri önce işaretler, sonra işaretlenmemiş (ölü) nesneleri geri kazanır.
use crate::rt::{self, Arr, Class, FLoc, Obj, Ref, Res};
use std::alloc::{alloc_zeroed, dealloc, Layout};
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};

pub const GC_HEAP: u32 = 1;
pub const GC_MARK: u32 = 2;
pub const GC_ARENA: u32 = 4;

/// Tüm iş parçacıklarının paylaştığı toplayıcı durumu. Tek bir değer, `G`; her alan, yerini aldığı değişkendir.
struct Globals {
    /// Eşzamanlı işaretleme (SATB) açık mı? Şu an hiçbir yer açmıyor; açıldığında yazmalar eski değeri kaydeder.
    satb_on: AtomicBool,
    satb_log: Mutex<Vec<usize>>,
    /// Bu döngünün işaretlemesinde bulunan zayıf referanslar (canlı olabilirler ya da henüz canlı olduğu bilinmeyebilirler).
    weak_seen: Mutex<Vec<usize>>,
    gc_stopping: AtomicU32,
    bytes_since_gc: AtomicUsize,
    /// Eşzamanlı bir döngü çalışıyor (başlangıcı ile son toplaması arasında). İşaretleyici son toplamayı ister;
    /// döngü sırasında yalnızca o toplama çalışabilir.
    cycle: AtomicBool,
    finishing: AtomicBool,
    conc: Mutex<Option<ConcCycle>>,
    recycle_classes: [AtomicUsize; RECYCLE_SLOTS],
}

impl Globals {
    const fn new() -> Globals {
        Globals {
        satb_on: AtomicBool::new(false),
        satb_log: Mutex::new(Vec::new()),
        weak_seen: Mutex::new(Vec::new()),
        gc_stopping: AtomicU32::new(0),
        bytes_since_gc: AtomicUsize::new(0),
        cycle: AtomicBool::new(false),
        finishing: AtomicBool::new(false),
        conc: Mutex::new(None),
        recycle_classes: [const { AtomicUsize::new(0) }; RECYCLE_SLOTS],
        }
    }
}

static G: Globals = Globals::new();

// Tembel süpürme durumu `Globals` içinde DEĞİL, ayrı statiklerde tutulur: `Globals` düzeni satır içi (`inline(always)`) yazma
// bariyerleri aracılığıyla shard nesnelerine gömülüdür; düzeni değiştirmek `--reuse-objs` ile yeniden kullanılan eski
// nesneleri sessizce bozar.
/// İşaret bitinin "işaretli" anlamına gelen değeri (0 ya da GC_MARK). Tembel süpürmede (MONO_GC_LAZY) her döngüde döner;
/// böylece süpürme biti silmek zorunda kalmaz. Normal kipte sabit GC_MARK'tır (işaretli = bit set, süpürme bitleri siler).
static SENSE: AtomicU32 = AtomicU32::new(GC_MARK);
/// Bu döngüde işaretlenen nesnelerin bayt toplamı (tembel süpürmede canlı boyut tahmini).
static MARKED_BYTES: AtomicUsize = AtomicUsize::new(0);

const SLAB_SIZE: usize = 65536;
const SLAB_MASK: usize = !(SLAB_SIZE - 1);
const SLAB_HDR: usize = 1024;
const SLAB_MAGIC: u64 = 0x4D4F_4E4F_534C_4142;

const NUM_CLASSES: usize = 14;
const SIZE_CLASSES: [usize; NUM_CLASSES] = [
    16, 32, 48, 64, 96, 128, 192, 256, 384, 512, 768, 1024, 1536, 2048,
];

#[inline(always)]
fn size_class(sz: usize) -> Option<usize> {
    if sz <= 64 {
        Some(sz.max(16).div_ceil(16) - 1)
    } else if sz <= 2048 {
        for (i, &c) in SIZE_CLASSES.iter().enumerate().skip(4) {
            if sz <= c {
                return Some(i);
            }
        }
        None
    } else {
        None
    }
}

#[repr(C)]
pub struct Slab {
    magic: u64,
    slot_size: u16,
    sc: u8,
    in_use: bool,
    n_slots: u16,
    next_bump: u16,
    live_count: u16,
    free_head: i32, // -1 if empty, else slot index stored inside dead slot word 0
    alloc_bits: [u64; 64],
}

impl Slab {
    fn new(sc: usize) -> *mut Slab {
        let layout = Layout::from_size_align(SLAB_SIZE, SLAB_SIZE).unwrap();
        let p = unsafe { alloc_zeroed(layout) } as *mut Slab;
        unsafe { Self::init_at(p, sc) };
        p
    }
    unsafe fn init_at(p: *mut Slab, sc: usize) {
        let slot_size = SIZE_CLASSES[sc];
        (*p).magic = SLAB_MAGIC;
        (*p).slot_size = slot_size as u16;
        (*p).sc = sc as u8;
        (*p).in_use = true;
        (*p).n_slots = ((SLAB_SIZE - SLAB_HDR) / slot_size) as u16;
        (*p).next_bump = 0;
        (*p).live_count = 0;
        (*p).free_head = -1;
        (*p).alloc_bits = [0; 64];
    }
    #[inline(always)]
    unsafe fn slot_ptr(&self, idx: u16) -> *mut u8 {
        (self as *const Slab as *mut u8).add(SLAB_HDR + idx as usize * self.slot_size as usize)
    }
    #[inline(always)]
    fn alloc_bit(&self, bit_idx: usize) -> bool {
        (self.alloc_bits[bit_idx >> 6] >> (bit_idx & 63)) & 1 != 0
    }
    #[inline(always)]
    fn set_bit(&mut self, bit_idx: usize) {
        self.alloc_bits[bit_idx >> 6] |= 1u64 << (bit_idx & 63);
    }
    #[inline(always)]
    fn clear_bit(&mut self, bit_idx: usize) {
        self.alloc_bits[bit_idx >> 6] &= !(1u64 << (bit_idx & 63));
    }
    #[inline(always)]
    /// Dilimin boş bir yuvası var mı? (Serbest bırakılmış ya da hiç kullanılmamış bir yuva.)
    fn has_free(&self) -> bool {
        self.free_head >= 0 || self.next_bump < self.n_slots
    }

    unsafe fn alloc_slot(&mut self) -> Option<*mut Obj> {
        if self.free_head >= 0 {
            let idx = self.free_head as u16;
            let p = self.slot_ptr(idx);
            self.free_head = *(p as *const i32);
            std::ptr::write_bytes(p, 0, self.slot_size as usize);
            let off = SLAB_HDR + idx as usize * self.slot_size as usize;
            self.set_bit(off >> 4);
            self.live_count += 1;
            return Some(p as *mut Obj);
        }
        if self.next_bump < self.n_slots {
            let idx = self.next_bump;
            self.next_bump += 1;
            let p = self.slot_ptr(idx);
            let off = SLAB_HDR + idx as usize * self.slot_size as usize;
            self.set_bit(off >> 4);
            self.live_count += 1;
            return Some(p as *mut Obj);
        }
        None
    }
}

pub struct ArenaChunk {
    ptr: *mut u8,
    cap: usize,
    used: usize,
}
impl Drop for ArenaChunk {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { dealloc(self.ptr, Layout::from_size_align(self.cap, 16).unwrap()) };
        }
    }
}

#[derive(Clone, Copy)]
pub struct ArenaMark {
    chunk_idx: usize,
    used: usize,
}

pub struct ThreadReg {
    tid: u64,
    stack_top: usize,
    stack_bot: AtomicUsize,
    paused: AtomicBool,
    blocked: AtomicU32,
    active: [*mut Slab; NUM_CLASSES],
    arena: Vec<ArenaChunk>,
    arena_idx: usize,
    java_thread: AtomicUsize,
    alloc_acc: usize,
    /// For-each döngüsünün sonunda geri verilen yineleyici nesneleri (bkz. `recycle`), geri dönüştürülebilir sınıf yuvası başına.
    recycled: [Vec<*mut Obj>; RECYCLE_SLOTS],
}

unsafe impl Send for ThreadReg {}

struct RegCell(UnsafeCell<*mut ThreadReg>);
thread_local! {
    static TLS_REG: RegCell = const { RegCell(UnsafeCell::new(std::ptr::null_mut())) };
}

#[inline(always)]
fn cur_reg() -> *mut ThreadReg {
    TLS_REG.with(|c| unsafe { *c.0.get() })
}

/// Bu iş parçacığının kayıtlı yığın üst sınırı (yığın aşağı doğru büyür; en yüksek adres). Kayıtlı değilse 0.
/// C9 yığın yürütücüsü (jtrace.rs) çerçeve işaretçisi zincirini bu sınırın dışına çıkmadan izler.
pub fn cur_stack_top() -> usize {
    let r = cur_reg();
    if r.is_null() { 0 } else { unsafe { (*r).stack_top } }
}


/// Referans yazma bariyeri (snapshot-at-the-beginning): işaretleme açıksa, yazılacak yerin eski değerini kaydeder.
#[inline(always)]
/// # Güvenlik
/// `slot`, bir `Ref` alanına ya da dizi elemanına geçerli ve hizalı bir işaretçi olmalıdır. `v` geçerli bir referans ya da null olmalıdır.
pub unsafe fn satb_store(slot: *mut Ref, v: Ref) {
    unsafe {
        if G.satb_on.load(Ordering::Relaxed) {
            satb_log((*slot).0 as usize);
        }
        *slot = v;
    }
}

// ------------------------------------------------------------------ Zayıf referanslar
//
// Bir WeakReference, kendi hedefini (referent) canlı tutmaz. İşaretleyici taradığı her WeakReference'ın `referent`
// alanını atlar ve referansı not eder.
// İşaretlemeden sonra (süpürmeden önce), işaretlenmemiş hedefi olan canlı bir WeakReference temizlenir.
// HotSpot da zayıf referanslar için aynısını yapar. SoftReference ve PhantomReference güçlü referans gibi izlenir;
// ReferenceQueue doldurulmaz.

/// WeakReference sınıfı (program yüklemezse 0) ve Reference.referent alanının ofseti.
fn weak_info() -> (usize, u32) {
    static W: OnceLock<(usize, u32)> = OnceLock::new();
    *W.get_or_init(|| match rt::find_class("java/lang/ref/WeakReference") {
        Some(wc) => match wc.find_field("referent", false) {
            Some((_, f)) => match f.loc {
                FLoc::Off(o) => (wc as *const Class as usize, o),
                FLoc::Static(_) => (0, 0),
            },
            None => (0, 0),
        },
        None => (0, 0),
    })
}


/// Hedefi izlenmemesi gereken bir WeakReference (ya da alt sınıfı) örneği için true.
#[inline(always)]
fn weak_object(c: &'static Class) -> bool {
    let (wc, ro) = weak_info();
    if wc == 0 || !c.ref_offsets.contains(&ro) {
        return false;
    }
    let mut k: Option<&'static Class> = Some(c);
    while let Some(x) = k {
        if x as *const Class as usize == wc {
            return true;
        }
        k = x.super_;
    }
    false
}

/// İşaretlemeden sonra: işaretlenmiş bir WeakReference'ın hedefi işaretlenmemişse, hedefini kaybeder.
fn clear_dead_referents() {
    let (wc, ro) = weak_info();
    if wc == 0 {
        return;
    }
    let seen = std::mem::take(&mut *G.weak_seen.lock().unwrap());
    for o in seen {
        unsafe {
            let obj = o as *mut Obj;
            if (*obj).gc & GC_MARK != cur_sense() {
                continue; // the WeakReference itself is garbage
            }
            let slot = (obj as *mut u8).add(ro as usize) as *mut Ref;
            let r = *slot;
            if r.0.is_null() {
                continue;
            }
            let rg = (*r.0).gc;
            if rg & GC_HEAP != 0 && rg & GC_MARK != cur_sense() {
                *slot = Ref::NULL;
            }
        }
    }
}

/// Atomik bir işlemle (Unsafe CAS/put, VarHandle store/CAS) üzerine yazılacak bir referans yuvası için yazma bariyeri.
/// İşaretleme açıksa, yuvanın şu anki değeri SATB günlüğüne gider.
#[inline(always)]
/// # Güvenlik
/// `slot`, üzerine yazılacak, referans boyutunda ve hizalı bir alanın geçerli işaretçisi olmalıdır.
pub unsafe fn satb_overwrite(slot: *mut *mut Obj) {
    if G.satb_on.load(Ordering::Relaxed) {
        satb_log(unsafe { *slot } as usize);
    }
}

#[cold]
#[inline(never)]
fn satb_log(old: usize) {
    if old != 0 {
        G.satb_log.lock().unwrap().push(old);
    }
}

/// Bir bellek aralığının (eski) içeriğini, işaretleme açıksa SATB günlüğüne alır. Her 8 baytlık sözcük bir referans
/// olabilir. Sözcük bir nesne adresi değilse işaretleyici onu yok sayar; yani bu yaklaşım muhafazakâr ve güvenlidir.
/// Yazma bariyeri olmayan toplu yazımlar (arraycopy, Unsafe kopyası, VarHandle) bunu çağırır.
/// olabilir: sözcük bir nesne adresi değilse işaretleyici onu yok sayar, yani bu konservatif ve güvenlidir.
/// Yazma bariyeri olmayan toplu yazımlar (arraycopy, Unsafe kopyası, VarHandle) bunu çağırır.
/// # Güvenlik
/// `p..p + bytes` okunabilir bellek olmalıdır.
pub unsafe fn satb_log_range(p: *const u8, bytes: usize) {
    if !G.satb_on.load(Ordering::Relaxed) {
        return;
    }
    let mut v = Vec::new();
    let mut off = 0usize;
    while off + 8 <= bytes {
        let w = unsafe { std::ptr::read_unaligned(p.add(off) as *const usize) };
        if w != 0 {
            v.push(w);
        }
        off += 8;
    }
    if !v.is_empty() {
        G.satb_log.lock().unwrap().extend(v);
    }
}

/// İşaretleme sırasında kaydedilen eski değerleri alır (işaretleyici bunları tüketir).
pub fn satb_take() -> Vec<usize> {
    std::mem::take(&mut *G.satb_log.lock().unwrap())
}


struct HeapState {
    threads: Vec<*mut ThreadReg>,
    slabs: Vec<usize>,
    slab_min: usize,
    slab_max: usize,
    partial: [Vec<*mut Slab>; NUM_CLASSES],
    empty: Vec<*mut Slab>,
    /// Tembel süpürme: son döngüde işaretlenmiş ama henüz süpürülmemiş dilimler (sınıf başına). Bunlara tahsis yapılmaz.
    unswept: [Vec<*mut Slab>; NUM_CLASSES],
    large: HashMap<usize, usize>,
    large_min: usize,
    large_max: usize,
    pinned_refs: Vec<usize>,
    static_slots: Vec<*mut Ref>,
    array_classes: Vec<usize>,
    live_bytes: usize,
    heap_bytes: usize,
    gc_threshold: usize,
    gc_count: u64,
    collecting: bool,
}
unsafe impl Send for HeapState {}

impl HeapState {
    fn new() -> Self {
        Self {
            threads: Vec::new(),
            slabs: Vec::new(),
            slab_min: usize::MAX,
            slab_max: 0,
            partial: Default::default(),
            empty: Vec::new(),
            unswept: Default::default(),
            large: HashMap::new(),
            large_min: usize::MAX,
            large_max: 0,
            pinned_refs: Vec::new(),
            static_slots: Vec::new(),
            array_classes: Vec::new(),
            live_bytes: 0,
            heap_bytes: 0,
            gc_threshold: std::env::var("MONO_GC_MB")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .map(|mb| mb.max(1) * 1024 * 1024)
                .unwrap_or(32 * 1024 * 1024),
            gc_count: 0,
            collecting: false,
        }
    }
    fn note_slab(&mut self, p: *mut Slab) {
        let a = p as usize;
        self.slabs.push(a);
        self.slab_min = self.slab_min.min(a);
        self.slab_max = self.slab_max.max(a + SLAB_SIZE);
        self.heap_bytes += SLAB_SIZE;
    }
}

fn heap_sync() -> &'static (Mutex<HeapState>, Condvar) {
    static H: OnceLock<(Mutex<HeapState>, Condvar)> = OnceLock::new();
    H.get_or_init(|| (Mutex::new(HeapState::new()), Condvar::new()))
}

#[inline(always)]
fn capture_regs() -> [usize; 11] {
    let mut r = [0usize; 11];
    #[cfg(target_arch = "x86_64")]
    unsafe {
        std::arch::asm!(
            "mov {0}, rbx",
            "mov {1}, rbp",
            "mov {2}, r12",
            "mov {3}, r13",
            "mov {4}, r14",
            "mov {5}, r15",
            out(reg) r[0], out(reg) r[1], out(reg) r[2],
            out(reg) r[3], out(reg) r[4], out(reg) r[5],
            options(nomem, nostack, preserves_flags)
        );
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        std::arch::asm!(
            "mov {0}, x19", "mov {1}, x20", "mov {2}, x21", "mov {3}, x22",
            "mov {4}, x23", "mov {5}, x24", "mov {6}, x25", "mov {7}, x26",
            "mov {8}, x27", "mov {9}, x28", "mov {10}, x29",
            out(reg) r[0], out(reg) r[1], out(reg) r[2], out(reg) r[3],
            out(reg) r[4], out(reg) r[5], out(reg) r[6], out(reg) r[7],
            out(reg) r[8], out(reg) r[9], out(reg) r[10],
            options(nomem, nostack, preserves_flags)
        );
    }
    r
}

#[inline(never)]
fn flush_regs_and_sp() -> usize {
    let mut x: usize = 0;
    std::hint::black_box(&mut x as *mut usize as usize)
}

pub fn register_thread(stack_marker: usize) {
    let reg = Box::into_raw(Box::new(ThreadReg {
        tid: crate::sync::tid(),
        stack_top: stack_marker,
        stack_bot: AtomicUsize::new(stack_marker),
        paused: AtomicBool::new(false),
        blocked: AtomicU32::new(0),
        active: [std::ptr::null_mut(); NUM_CLASSES],
        arena: Vec::new(),
        arena_idx: 0,
        java_thread: AtomicUsize::new(0),
        alloc_acc: 0,
        recycled: Default::default(),
    }));
    TLS_REG.with(|c| unsafe { *c.0.get() = reg });
    let (m, _) = heap_sync();
    let mut h = m.lock().unwrap();
    h.threads.push(reg);
}

pub fn set_thread_obj(t: Ref) {
    let reg = cur_reg();
    if !reg.is_null() {
        unsafe { (*reg).java_thread.store(t.0 as usize, Ordering::Relaxed) };
    }
}

pub fn unregister_thread() {
    let reg = cur_reg();
    if reg.is_null() {
        return;
    }
    // Güvenli nokta (safepoint) gibi: yığını ve yazmaçları kaydeder ve toplama beklerken "duraklatıldı" der.
    // Bu bayrak olmadan toplayıcı bu iş parçacığını sonsuza kadar bekler (bir stres testinde görülen kilitlenme:
    // hem toplayıcı hem iş parçacığı birbirini bekliyordu).
    let _regs = std::hint::black_box(capture_regs());
    let sp = flush_regs_and_sp();
    unsafe { (*reg).stack_bot.store(sp, Ordering::Release) };
    TLS_REG.with(|c| unsafe { *c.0.get() = std::ptr::null_mut() });
    let (m, cv) = heap_sync();
    let mut h = m.lock().unwrap();
    while h.collecting {
        unsafe { (*reg).paused.store(true, Ordering::SeqCst) };
        cv.notify_all();
        h = cv.wait(h).unwrap();
    }
    unsafe { (*reg).paused.store(true, Ordering::SeqCst) };
    h.threads.retain(|&p| p != reg);
    unsafe {
        let b = Box::from_raw(reg);
        for s in b.active {
            // Yalnızca hâlâ boş yeri olan dilimler: kısmi listedeki dolu bir dilim, ayırıcının panic yapmasına yol açıyordu
            if !s.is_null() && (*s).has_free() {
                h.partial[(*s).sc as usize].push(s);
            }
        }
    }
    cv.notify_all();
}

pub struct ThreadGuard;
impl ThreadGuard {
    pub fn enter() -> Self {
        let m = flush_regs_and_sp();
        register_thread(m);
        ThreadGuard
    }
}
impl Drop for ThreadGuard {
    fn drop(&mut self) {
        unregister_thread();
    }
}

pub struct ScopedBlock {
    _regs: [usize; 11],
}
impl ScopedBlock {
    #[inline(never)]
    pub fn enter() -> Self {
        let regs = std::hint::black_box(capture_regs());
        let reg = cur_reg();
        if !reg.is_null() {
            let sp = flush_regs_and_sp();
            unsafe {
                (*reg).stack_bot.store(sp, Ordering::Release);
                (*reg).blocked.fetch_add(1, Ordering::SeqCst);
            }
            if G.gc_stopping.load(Ordering::Relaxed) != 0 {
                let (_, cv) = heap_sync();
                cv.notify_all();
            }
        }
        ScopedBlock { _regs: regs }
    }
}
impl Drop for ScopedBlock {
    #[inline(always)]
    fn drop(&mut self) {
        std::hint::black_box(&self._regs);
        let reg = cur_reg();
        if !reg.is_null() {
            unsafe {
                (*reg).blocked.fetch_sub(1, Ordering::SeqCst);
            }
            // SeqCst ŞART (Acquire yetmez): bu, GC'nin `gc_stopping=1` yazıp `blocked` okumasıyla bir Dekker çiftidir.
            // İki taraf da "önce kendi yazımı, sonra karşının değişkeni" yapar; ikisinin de 0 görmesi (GC: blocked hâlâ 1,
            // biz: gc_stopping hâlâ 0) ancak yüklemeler SeqCst ise imkânsızdır. arm64'te Acquire yükleme LDAPR olabilir ve
            // önceki STLR'nin önüne geçebilir: iş parçacığı GC "durdu" sanarken Java koduna döner, yığını ve yeni nesneleri
            // taranmaz, canlı nesneler süpürülür, bellek başka nesneye verilir (S26'da aralıklı ClassCastException /
            // AbstractMethodError: "java/lang/String has no method for selector 118"). x86'da kilitli işlem tam bariyer
            // olduğu için masaüstünde hiç görülmedi.
            if G.gc_stopping.load(Ordering::SeqCst) != 0 {
                safepoint_slow();
            }
        }
    }
}

#[inline(always)]
pub fn safepoint() {
    if G.gc_stopping.load(Ordering::Relaxed) != 0 {
        safepoint_slow();
    }
}

#[inline(never)]
pub fn safepoint_slow() {
    let _regs = std::hint::black_box(capture_regs());
    let reg = cur_reg();
    if reg.is_null() {
        return;
    }
    let sp = flush_regs_and_sp();
    unsafe { (*reg).stack_bot.store(sp, Ordering::Release) };
    let (m, cv) = heap_sync();
    let mut h = m.lock().unwrap();
    if G.gc_stopping.load(Ordering::Acquire) != 0 {
        unsafe { (*reg).paused.store(true, Ordering::SeqCst) };
        cv.notify_all();
        while G.gc_stopping.load(Ordering::Acquire) != 0 {
            h = cv.wait(h).unwrap();
        }
        unsafe { (*reg).paused.store(false, Ordering::SeqCst) };
    }
    std::hint::black_box(&_regs);
}

pub fn pin_ref(r: Ref) {
    if r.is_null() {
        return;
    }
    let (m, _) = heap_sync();
    let mut h = m.lock().unwrap();
    h.pinned_refs.push(r.0 as usize);
}

pub fn register_static_slot(slot: *mut Ref) {
    let (m, _) = heap_sync();
    let mut h = m.lock().unwrap();
    h.static_slots.push(slot);
}

pub fn register_array_class(c: &'static Class) {
    let (m, _) = heap_sync();
    let mut h = m.lock().unwrap();
    h.array_classes.push(c as *const Class as usize);
}

// ------------------------------------------------------------------ Tahsis (allocation)
pub fn alloc_obj(class: &'static Class, size: usize) -> *mut Obj {
    crate::prof::on_alloc(class, size, false);
    alloc_obj_inner(class, size)
}

#[inline(always)]
fn alloc_obj_inner(class: &'static Class, size: usize) -> *mut Obj {
    let sz = size.max(16);
    let aligned = (sz + 15) & !15;
    let reg = cur_reg();
    if let Some(sc) = size_class(aligned) {
        if !reg.is_null() {
            unsafe {
                let s = (*reg).active[sc];
                if !s.is_null() {
                    if let Some(o) = (*s).alloc_slot() {
                        (*o).class = class;
                        (*o).gc = new_obj_bits();
                        (*reg).alloc_acc += SIZE_CLASSES[sc];
                        if (*reg).alloc_acc >= 65536 {
                            flush_alloc_acc(reg);
                        }
                        return o;
                    }
                }
            }
            return alloc_small_slow(reg, sc, class);
        }
    }
    alloc_large(reg, class, aligned)
}

#[inline(never)]
fn flush_alloc_acc(reg: *mut ThreadReg) {
    let add = unsafe {
        let v = (*reg).alloc_acc;
        (*reg).alloc_acc = 0;
        v
    };
    let prev = G.bytes_since_gc.fetch_add(add, Ordering::Relaxed);
    if prev + add >= 32 * 1024 * 1024 {
        let thresh = {
            let (m, _) = heap_sync();
            m.lock().unwrap().gc_threshold
        };
        if prev + add >= thresh {
            collect();
        }
    }
}

#[inline(never)]
fn alloc_small_slow(reg: *mut ThreadReg, sc: usize, class: &'static Class) -> *mut Obj {
    safepoint();
    let (m, _) = heap_sync();
    let mut need_gc = false;
    let slab: *mut Slab = {
        let mut h = m.lock().unwrap();
        let old = unsafe { (*reg).active[sc] };
        if !old.is_null() {
            unsafe { (*reg).active[sc] = std::ptr::null_mut() };
        }
        // Kısmi listeden bir dilim yalnızca gerçekten boş yuvası varsa kullanılır. Dolu olan burada atılır;
        // süpürme alan açıldığında onu listeye geri koyar.
        let found = loop {
            match h.partial[sc].pop() {
                Some(s) if unsafe { (*s).has_free() } => break s,
                Some(_) => continue,
                None => {
                    if let Some(u) = lazy_sweep_one(&mut h, sc) {
                        break u;
                    }
                    break if let Some(es) = h.empty.pop() {
                        unsafe {
                            Slab::init_at(es, sc);
                            std::ptr::write_bytes((es as *mut u8).add(SLAB_HDR), 0, SLAB_SIZE - SLAB_HDR);
                        }
                        es
                    } else {
                        // Yeni dilim istemeden önce başka sınıfların süpürülmemiş dilimlerinden boş olan var mı bak.
                        let mut got: Option<*mut Slab> = None;
                        'scan: for k in 1..NUM_CLASSES {
                            let sc2 = (sc + k) % NUM_CLASSES;
                            let mut tries = 0;
                            while tries < 32 && !h.unswept[sc2].is_empty() {
                                tries += 1;
                                let u = h.unswept[sc2].pop().unwrap();
                                unsafe {
                                    sweep_slab_nocensus(u as usize);
                                    if (*u).live_count == 0 {
                                        (*u).in_use = false;
                                        got = Some(u);
                                        break 'scan;
                                    }
                                    if (*u).has_free() {
                                        h.partial[sc2].push(u);
                                    }
                                }
                            }
                        }
                        if let Some(es) = got {
                            unsafe {
                                Slab::init_at(es, sc);
                                std::ptr::write_bytes((es as *mut u8).add(SLAB_HDR), 0, SLAB_SIZE - SLAB_HDR);
                            }
                            es
                        } else {
                        if G.bytes_since_gc.load(Ordering::Relaxed) >= h.gc_threshold {
                            need_gc = true;
                        }
                        let s = Slab::new(sc);
                        h.note_slab(s);
                        s
                        }
                    };
                }
            }
        };
        unsafe { (*reg).active[sc] = found };
        found
    };
    if need_gc {
        collect();
        // GC'den sonra active[sc] temizlenmiş olabilir; alloc_obj üzerinden bir kez temiz biçimde yinelenir:
        return alloc_obj_inner(class, SIZE_CLASSES[sc]);
    }
    unsafe {
        // Yukarıda seçilen bir dilimin boş yuvası vardır. Yoksa (hiç beklenmez) ayırıcıya yeniden dönülür.
        let o = match (*slab).alloc_slot() {
            Some(o) => o,
            None => {
                (*reg).active[sc] = std::ptr::null_mut();
                return alloc_small_slow(reg, sc, class);
            }
        };
        (*o).class = class;
        (*o).gc = new_obj_bits();
        (*reg).alloc_acc += SIZE_CLASSES[sc];
        o
    }
}

#[inline(never)]
fn alloc_large(reg: *mut ThreadReg, class: &'static Class, aligned: usize) -> *mut Obj {
    if !reg.is_null() {
        safepoint();
        let thresh = {
            let (m, _) = heap_sync();
            m.lock().unwrap().gc_threshold
        };
        let total = G.bytes_since_gc.fetch_add(aligned, Ordering::Relaxed) + aligned;
        if total >= thresh {
            collect();
        }
    }
    let layout = Layout::from_size_align(aligned, 16).unwrap();
    let p = unsafe { alloc_zeroed(layout) } as *mut Obj;
    unsafe {
        (*p).class = class;
        (*p).gc = new_obj_bits();
    }
    let (m, _) = heap_sync();
    let mut h = m.lock().unwrap();
    let a = p as usize;
    h.large.insert(a, aligned);
    h.large_min = h.large_min.min(a);
    h.large_max = h.large_max.max(a + 1);
    h.heap_bytes += aligned;
    p
}

// ------------------------------------------------------------------ Kaçmayan bump arena
pub struct ArenaGuard {
    mark: ArenaMark,
}
impl ArenaGuard {
    #[inline(always)]
    pub fn enter() -> Self {
        let reg = cur_reg();
        let mark = if reg.is_null() {
            ArenaMark { chunk_idx: 0, used: 0 }
        } else {
            unsafe {
                let r = &*reg;
                let idx = r.arena_idx;
                let used = if idx < r.arena.len() { r.arena[idx].used } else { 0 };
                ArenaMark { chunk_idx: idx, used }
            }
        };
        ArenaGuard { mark }
    }
}
impl Drop for ArenaGuard {
    #[inline(always)]
    fn drop(&mut self) {
        let reg = cur_reg();
        if !reg.is_null() {
            unsafe {
                let r = &mut *reg;
                let max = r.arena.len();
                for i in (self.mark.chunk_idx + 1)..max {
                    r.arena[i].used = 0;
                }
                if self.mark.chunk_idx < r.arena.len() {
                    r.arena[self.mark.chunk_idx].used = self.mark.used;
                }
                r.arena_idx = self.mark.chunk_idx;
            }
        }
    }
}

/// Kaçış analizinin kaçmadığını kanıtladığı tahsis noktası: iş parçacığının kapsam arenasında bump ile ayrılır
/// ve metot dönünce serbest bırakılır (`ArenaGuard`).
/// `MONO_ARENA=0` bunu kapatır (o nesneler yığından gelir). Bu, çalıştırmaları karşılaştırmak ve analiz
/// bir gün yanlış çıkarsa geri dönüş yolu olarak kullanılır.
/// `MONO_ARENA_CHECK=1` her toplamada, hiçbir yığın nesnesinin ya da statik alanın arenaya işaret etmediğini denetler.
pub fn alloc_arena(class: &'static Class, size: usize) -> Ref {
    crate::prof::on_alloc(class, size, true);
    if !arena_enabled() {
        return Ref(alloc_obj_inner(class, size));
    }
    alloc_arena_bump(class, size)
}

/// Ortam anahtarları; ilk kullanımda bir kez okunur (tanılama ve A/B karşılaştırma anahtarları).
struct Knobs {
    arena: bool,
    /// 0: kapalı, 1: rapor et, 2: ilk ihlalde durdur.
    arena_check: u8,
    /// MONO_GC_POISON=1: serbest bırakılan yuvalar 0xDD ile doldurulur; böylece serbest bırakılmış belleğe erişim hemen görünür.
    poison: bool,
    concurrent: bool,
    /// MONO_GC_LAZY=1: tembel süpürme (dilimler tahsis sırasında süpürülür; durdur-dünya süpürmesi kalkar).
    lazy: bool,
    recycle: bool,
    /// MONO_GC=off: otomatik toplama yok (toplayıcı kapalıyken yığının ne yaptığını ölçer).
    gc_off: bool,
}

fn knobs() -> &'static Knobs {
    static K: OnceLock<Knobs> = OnceLock::new();
    K.get_or_init(|| Knobs {
        arena: std::env::var("MONO_ARENA").map_or(true, |v| v != "0"),
        arena_check: match std::env::var("MONO_ARENA_CHECK").as_deref() {
            Ok("abort") => 2,
            Ok(v) if !v.is_empty() && v != "0" => 1,
            _ => 0,
        },
        poison: std::env::var_os("MONO_GC_POISON").is_some_and(|v| !v.is_empty() && v != "0"),
        // Android'de eşzamanlı işaretleme ve tembel süpürme varsayılan olarak AÇIK (telefonda oyunun duraklamaları
        // kısalsın diye; masaüstü oyun koşularında ikisi birlikte sınandı). Masaüstünde varsayılan eskisi gibi kapalı.
        // Her iki platformda da değişken açıkça verilirse o geçerlidir: "0" kapatır, başka bir değer açar.
        concurrent: env_switch("MONO_GC_CONCURRENT", cfg!(target_os = "android")),
        lazy: env_switch("MONO_GC_LAZY", cfg!(target_os = "android")),
        recycle: std::env::var("MONO_RECYCLE").map_or(true, |v| v != "0"),
        gc_off: std::env::var("MONO_GC").is_ok_and(|v| v == "off"),
    })
}

/// Açık/kapalı ortam değişkeni: tanımlı değilse `default`; boş ya da "0" kapalı; başka her değer açık.
/// (Eskiden MONO_GC_CONCURRENT için "tanımlı olması" yetiyordu, "0" bile açıyordu; artık "0" kapatır.)
/// Unix zamanı (ms). Toplama günlük satırlarına damga olarak yazılır; başlatıcının kare süresi dosyasıyla (frametime.csv, aynı
/// saat) karşılaştırılıp her takılmanın toplamadan mı başka bir şeyden mi geldiği ayrılır.
fn unix_ms() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

fn env_switch(name: &str, default: bool) -> bool {
    match std::env::var_os(name) {
        None => default,
        Some(v) => !v.is_empty() && v != "0",
    }
}

/// Toplama fazlarının günlüğü (MONO_GC_PHASES): her toplamada bir satır (duraklama süreleri, canlı veri). Android'de
/// varsayılan AÇIK: telefondaki takılmaların nedeni cihaz raporunda görülebilsin diye (toplama başına bir satır, hacim düşük).
/// Masaüstünde varsayılan kapalı (eski davranış). "0" her yerde kapatır. Bir kez okunur.
pub fn phases_on() -> bool {
    static P: OnceLock<bool> = OnceLock::new();
    *P.get_or_init(|| env_switch("MONO_GC_PHASES", cfg!(target_os = "android")))
}

fn arena_enabled() -> bool {
    knobs().arena
}

fn arena_check_mode() -> u8 {
    knobs().arena_check
}

fn alloc_arena_bump(class: &'static Class, size: usize) -> Ref {
    let sz = ((size.max(16)) + 15) & !15;
    let reg = cur_reg();
    if reg.is_null() {
        return Ref(alloc_obj_inner(class, size));
    }
    unsafe {
        let r = &mut *reg;
        if r.arena.is_empty() {
            let cap = 65536;
            let ptr = alloc_zeroed(Layout::from_size_align(cap, 16).unwrap());
            r.arena.push(ArenaChunk { ptr, cap, used: 0 });
            r.arena_idx = 0;
        }
        loop {
            let c = &mut r.arena[r.arena_idx];
            if c.used + sz <= c.cap {
                let p = c.ptr.add(c.used) as *mut Obj;
                std::ptr::write_bytes(p as *mut u8, 0, sz);   // bytes, not Obj-sized units
                c.used += sz;
                (*p).class = class;
                (*p).gc = GC_ARENA;
                return Ref(p);
            }
            if r.arena_idx + 1 < r.arena.len() {
                r.arena_idx += 1;
                r.arena[r.arena_idx].used = 0;
            } else {
                let cap = 65536usize.max(sz);
                let ptr = alloc_zeroed(Layout::from_size_align(cap, 16).unwrap());
                r.arena.push(ArenaChunk { ptr, cap, used: 0 });
                r.arena_idx = r.arena.len() - 1;
            }
        }
    }
}

// ------------------------------------------------------------------ İşaretle ve Süpür toplayıcısı
//
// Eşzamanlı işaretleme (MONO_GC_CONCURRENT=1). Bir döngü, kısa bir durdur-dünya kök anlık görüntüsüyle başlar;
// işaretleme, program çalışırken bir işaretleyici iş parçacığında yapılır (SATB bariyeri, `satb_store`, üzerine yazılan
// her referansı günlükler). Son toplama (durdur-dünya: yine kökler, SATB günlüğü ve süpürme) iş kalmayınca işaretleyici tarafından yapılır.
// Döngü sırasında ayrılan nesneler doğuştan işaretli gelir, böylece süpürme onları tutar.
// Döngü sırasında gelen toplama istekleri yok sayılır: döngü kendini bitirir.
// Değişken ayarlanmamışsa burada hiçbir şey çalışmaz ve toplayıcı değişmez.


/// Bir döngü başladığında işaretleyici iş parçacığının devraldığı veriler.
struct ConcCycle {
    work: Vec<usize>,
    slabs: Vec<usize>,
    slab_min: usize,
    slab_max: usize,
    large: HashMap<usize, usize>,
    large_min: usize,
    large_max: usize,
}

fn poison_enabled() -> bool {
    knobs().poison
}

#[inline(always)]
fn lazy_enabled() -> bool {
    knobs().lazy
}

#[inline(always)]
fn cur_sense() -> u32 {
    SENSE.load(Ordering::Relaxed)
}

fn concurrent_enabled() -> bool {
    knobs().concurrent
}

/// Yeni ayrılmış bir nesnenin GC bitleri: döngü sırasında siyah (o döngünün süpürmesinden sağ çıkar).
#[inline(always)]
fn new_obj_bits() -> u32 {
    if lazy_enabled() {
        // Tembel kipte yeni nesne her zaman şu anki kutupla doğar: döngü sırasında siyah, döngüler arasında bir sonraki
        // dönüşte kendiliğinden "işaretsiz" olur.
        GC_HEAP | cur_sense()
    } else if G.satb_on.load(Ordering::Relaxed) {
        GC_HEAP | GC_MARK
    } else {
        GC_HEAP
    }
}

pub fn collect() {
    if knobs().gc_off {
        return; // MONO_GC=off: automatic collections never run; System.gc() still does (collect_explicit)
    }
    if G.cycle.load(Ordering::Acquire) && !G.finishing.load(Ordering::Acquire) {
        return; // a cycle is running and finishes itself
    }
    let begin = concurrent_enabled() && !G.finishing.load(Ordering::Acquire);
    collect_impl(begin);
}

/// System.gc(): dönmeden önce tam bir toplama yapar (HotSpot gibi), böylece zayıf referanslar çağıran devam etmeden temizlenmiş olur.
/// Çalışan bir eşzamanlı döngü önce beklenir; bekleme güvenli noktalarda yapılır, çünkü o döngünün son toplaması bu iş parçacığını durdurmalıdır.
pub fn collect_explicit() {
    while G.cycle.load(Ordering::Acquire) {
        safepoint();
        std::thread::sleep(std::time::Duration::from_micros(200));
    }
    // İlk çağrı yalnızca başka bir iş parçacığının bitirmekte olduğu bir toplamayı bekleyebilir (collect_impl o zaman hemen döner).
    // İkincisi bu çağrının istediği toplamadır. Bu döngüde siyah doğan bir nesnenin işareti süpürmeyle silinir;
    // hiçbir şey ona referans vermiyorsa ikinci toplama onu çöp olarak görür.
    collect_impl(false);
    collect_impl(false);
}

#[inline(always)]
// Sıcak yol: heap sınırları değerle geçirilir; böylece işaretleyici onları yazmaçlarda tutar.
#[allow(clippy::too_many_arguments)]
fn try_push(
    addr: usize,
    slabs_snap: &[usize],
    slab_min: usize,
    slab_max: usize,
    large_map: &HashMap<usize, usize>,
    large_min: usize,
    large_max: usize,
    work: &mut Vec<*mut Obj>,
) {
    if addr == 0 || (addr & 0xF) != 0 {
        return;
    }
    if addr >= slab_min && addr < slab_max {
        let base = addr & SLAB_MASK;
        let off = addr - base;
        if off >= SLAB_HDR && slabs_snap.binary_search(&base).is_ok() {
            unsafe {
                let s = &*(base as *const Slab);
                if s.magic == SLAB_MAGIC && s.alloc_bit(off >> 4) {
                    let o = addr as *mut Obj;
                    let g = (*o).gc;
                    if g & GC_HEAP != 0 && g & GC_MARK != cur_sense() {
                        (*o).gc = (g & !GC_MARK) | cur_sense();
                        MARKED_BYTES.fetch_add(s.slot_size as usize, Ordering::Relaxed);
                        work.push(o);
                    }
                }
            }
            return;
        }
    }
    if crate::region::in_region(addr) {
        // bölge nesnesi: heap toplayıcısı onu süpürmez ama içinden geçer (heap'e verdiği referanslar canlı kalmalı)
        crate::region::note(addr, work);
        return;
    }
    if addr >= large_min && addr < large_max {
        if let Some(&lsz) = large_map.get(&addr) {
            unsafe {
                let o = addr as *mut Obj;
                let g = (*o).gc;
                if g & GC_HEAP != 0 && g & GC_MARK != cur_sense() {
                    (*o).gc = (g & !GC_MARK) | cur_sense();
                    MARKED_BYTES.fetch_add(lsz, Ordering::Relaxed);
                    work.push(o);
                }
            }
        }
    }
}

/// İşaretleyici için bir nesneyi tarar: referans verdiği nesneleri yığına iter (durdur-dünya boşaltmasıyla aynı iş).
fn marker_scan(obj: *mut Obj, cyc: &ConcCycle, work: &mut Vec<*mut Obj>) {
    unsafe {
        let c = (*obj).class;
        let mp = c.mirror.0.load(Ordering::Relaxed);
        if !mp.is_null() {
            try_push(mp as usize, &cyc.slabs, cyc.slab_min, cyc.slab_max, &cyc.large, cyc.large_min, cyc.large_max, work);
        }
        if c.elem == b'A' {
            let n = (*(obj as *const Arr)).len.max(0) as usize;
            let data = (obj as *const u8).add(24) as *const Ref;
            for i in 0..n {
                try_push((*data.add(i)).0 as usize, &cyc.slabs, cyc.slab_min, cyc.slab_max, &cyc.large, cyc.large_min, cyc.large_max, work);
            }
        } else {
            let weak = weak_object(c);
            let skip = if weak {
                G.weak_seen.lock().unwrap().push(obj as usize);
                weak_info().1
            } else {
                u32::MAX
            };
            for &off in c.ref_offsets {
                if off == skip {
                    continue; // a weak referent is not traced
                }
                let r = *((obj as *const u8).add(off as usize) as *const Ref);
                try_push(r.0 as usize, &cyc.slabs, cyc.slab_min, cyc.slab_max, &cyc.large, cyc.large_min, cyc.large_max, work);
            }
        }
    }
}

/// İşaretleyici iş parçacığı: anlık görüntüden işaretler, SATB günlüğünü alır ve iş kalmayınca son toplamayı çalıştırır.
fn marker_main() {
    let cyc = match G.conc.lock().unwrap().take() {
        Some(c) => c,
        None => return,
    };
    let mut work: Vec<*mut Obj> = cyc.work.iter().map(|&a| a as *mut Obj).collect();
    loop {
        while let Some(obj) = work.pop() {
            marker_scan(obj, &cyc, &mut work);
        }
        let log = satb_take();
        if log.is_empty() {
            break;
        }
        for v in log {
            try_push(v, &cyc.slabs, cyc.slab_min, cyc.slab_max, &cyc.large, cyc.large_min, cyc.large_max, &mut work);
        }
    }
    drop(cyc);
    G.finishing.store(true, Ordering::Release);
    collect_impl(false);
    G.finishing.store(false, Ordering::Release);
}

/// Bir dilimi sayım olmadan süpürür: ölü yuvaları serbest listeye bağlar, canlıların işaret bitini siler ve canlı sayısını
/// `live_count` alanına yazar. Dilimler birbirinden bağımsız olduğundan farklı iş parçacıklarından çağrılabilir.
unsafe fn sweep_slab_nocensus(base: usize) {
    let s = &mut *(base as *mut Slab);
    let n = s.next_bump;
    let sz = s.slot_size as usize;
    let lazy = lazy_enabled();
    let sense = cur_sense();
    s.free_head = -1;
    let mut live_c = 0u16;
    for idx in (0..n).rev() {
        let off = SLAB_HDR + idx as usize * sz;
        let bit = off >> 4;
        if !s.alloc_bit(bit) {
            *((base + off) as *mut i32) = s.free_head;
            s.free_head = idx as i32;
            continue;
        }
        let o = (base + off) as *mut Obj;
        if ((*o).gc & GC_MARK) == sense {
            if !lazy {
                (*o).gc &= !GC_MARK;
            }
            live_c += 1;
        } else {
            s.clear_bit(bit);
            if poison_enabled() {
                std::ptr::write_bytes((base + off) as *mut u8, 0xDD, sz);
            }
            *((base + off) as *mut i32) = s.free_head;
            s.free_head = idx as i32;
        }
    }
    s.live_count = live_c;
}

/// Tembel süpürmeden kalan tüm dilimleri paralel süpürür ve sonuçlarını havuzlara dağıtır (boş: `empty`, yer var: `partial`).
/// Yeni döngü başlamadan önce durdur-dünya içinde çağrılır. Fazla boş dilimler işletim sistemine geri verilir.
fn finish_lazy_sweep(h: &mut HeapState) {
    let mut all: Vec<usize> = Vec::new();
    for sc in 0..NUM_CLASSES {
        for s in h.unswept[sc].drain(..) {
            all.push(s as usize);
        }
    }
    if !all.is_empty() {
        let workers = std::thread::available_parallelism().map_or(1, |n| n.get()).min(8);
        if workers > 1 && all.len() >= 64 {
            let chunk = all.len().div_ceil(workers);
            std::thread::scope(|sc| {
                for part in all.chunks(chunk) {
                    sc.spawn(move || {
                        for &b in part {
                            unsafe { sweep_slab_nocensus(b) };
                        }
                    });
                }
            });
        } else {
            for &b in &all {
                unsafe { sweep_slab_nocensus(b) };
            }
        }
        for &b in &all {
            unsafe {
                let s = &mut *(b as *mut Slab);
                if s.live_count == 0 {
                    s.in_use = false;
                    h.empty.push(s as *mut Slab);
                } else if s.has_free() {
                    h.partial[s.sc as usize].push(s as *mut Slab);
                }
            }
        }
    }
    // Boş dilim havuzu sınırlanır: fazlası geri verilir, dilim listesinden çıkarılır.
    const KEEP_EMPTY: usize = 16;
    if h.empty.len() > KEEP_EMPTY {
        let extra: Vec<*mut Slab> = h.empty.split_off(KEEP_EMPTY);
        let set: std::collections::HashSet<usize> = extra.iter().map(|&p| p as usize).collect();
        h.slabs.retain(|b| !set.contains(b));
        for p in extra {
            h.heap_bytes = h.heap_bytes.saturating_sub(SLAB_SIZE);
            unsafe { dealloc(p as *mut u8, Layout::from_size_align(SLAB_SIZE, SLAB_SIZE).unwrap()) };
        }
    }
}

/// Tahsis yolu için: süpürülmemiş bir dilimi süpürür. Yeri varsa dilimi döndürür; tamamen boşsa `empty` havuzuna koyar.
fn lazy_sweep_one(h: &mut HeapState, sc: usize) -> Option<*mut Slab> {
    while let Some(u) = h.unswept[sc].pop() {
        unsafe {
            sweep_slab_nocensus(u as usize);
            if (*u).live_count == 0 {
                (*u).in_use = false;
                h.empty.push(u);
                return None;
            }
            if (*u).has_free() {
                return Some(u);
            }
        }
    }
    None
}

#[inline(never)]
fn collect_impl(begin_only: bool) {
    if !G.finishing.load(Ordering::Acquire) {
        G.weak_seen.lock().unwrap().clear(); // a new marking starts: the weak references it finds are its own
    }
    let t_entry = std::time::Instant::now();
    let _regs = std::hint::black_box(capture_regs());
    let my_sp = flush_regs_and_sp();
    let reg = cur_reg();
    if !reg.is_null() {
        unsafe { (*reg).stack_bot.store(my_sp, Ordering::Release) };
    }

    let (m, cv) = heap_sync();
    let mut h = m.lock().unwrap();
    if h.collecting {
        if !reg.is_null() {
            unsafe { (*reg).paused.store(true, Ordering::SeqCst) };
        }
        cv.notify_all();
        while h.collecting {
            h = cv.wait(h).unwrap();
        }
        if !reg.is_null() {
            unsafe { (*reg).paused.store(false, Ordering::SeqCst) };
        }
        return;
    }
    if std::env::var_os("MONO_TRACE_PARK").is_some() {
        eprintln!("[mono:gc_stw] start\n{}", std::backtrace::Backtrace::force_capture());
    }
    h.collecting = true;
    G.gc_stopping.store(1, Ordering::SeqCst);

    // Diğer tüm etkin iş parçacıklarının bir güvenli noktada ya da ScopedBlock içinde durmasını bekle
    loop {
        let mut all_ready = true;
        for &tp in &h.threads {
            if tp == reg {
                continue;
            }
            unsafe {
                // SeqCst: ScopedBlock::drop ile Dekker çifti (oradaki açıklamaya bakın)
                if !(*tp).paused.load(Ordering::SeqCst) && (*tp).blocked.load(Ordering::SeqCst) == 0 {
                    all_ready = false;
                    break;
                }
            }
        }
        if all_ready {
            break;
        }
        h = cv.wait_timeout(h, std::time::Duration::from_micros(50)).unwrap().0;
    }

    let stw_start = std::time::Instant::now();
    // Güvenli nokta beklemesi: girişten, diğer iş parçacıklarının durmasının tamamlanmasına kadar
    let t_wait = stw_start.duration_since(t_entry);
    h.gc_count += 1;
    // İş parçacığına özel etkin dilimleri genel havuzlara geri ver; böylece her şey temiz biçimde süpürülür
    for i in 0..h.threads.len() {
        let tp = h.threads[i];
        unsafe {
            for sc in 0..NUM_CLASSES {
                let s = (*tp).active[sc];
                if !s.is_null() {
                    h.partial[sc].push(s);
                    (*tp).active[sc] = std::ptr::null_mut();
                }
            }
        }
    }

    // Yeni bir işaretleme başlıyorsa (eşzamanlı başlangıç ya da tek aşamalı toplama) kutup dönmelidir. Eşzamanlı döngünün
    // bitiş toplaması (`G.cycle` açık) başlangıçta dönen kutbu sürdürür. `G.finishing` buna yetmez: bitiş sürerken başka bir
    // iş parçacığı `collect()` çağırırsa o da `finishing` açıkken gelir ve bağımsız bir toplamadır; döndürülmezse önceki
    // döngünün işaretli nesneleri "zaten işaretli" sayılır, çocukları taranmaz ve canlı nesneler süpürülür.
    let new_cycle = begin_only || !G.cycle.load(Ordering::Acquire);
    let t_pre = std::time::Instant::now();
    if new_cycle {
        crate::region::cycle_begin(); // bölge parçaları "kanıtlanmamış" olur, iş parçacıkları parça bırakır
    }
    if lazy_enabled() && new_cycle {
        // Yeni döngü: önceki döngüden kalan süpürülmemiş dilimler, kutup dönmeden önce bitirilir (aksi halde eski ve yeni
        // işaretler karışırdı). Sonra kutup döner: tüm nesneler bir anda "işaretsiz" olur.
        finish_lazy_sweep(&mut h);
        SENSE.fetch_xor(GC_MARK, Ordering::Relaxed);
        MARKED_BYTES.store(0, Ordering::Relaxed);
    }
    if new_cycle && phases_on() {
        // döngü başındaki işler (kalan tembel süpürme, boş dilim bırakma, bölge hazırlığı) kök taramadan önce, duraklamanın içindedir
        eprintln!("[mono:gc_phases] pre gc={} finish_sweep_etc={}us t={}", h.gc_count, t_pre.elapsed().as_micros(), unix_ms());
    }
    h.slabs.sort_unstable();
    let slabs_snap = &h.slabs;
    let slab_min = h.slab_min;
    let slab_max = h.slab_max;
    let large_map = &h.large;
    let large_min = h.large_min;
    let large_max = h.large_max;

    let mut work: Vec<*mut Obj> = Vec::with_capacity(4096);


    macro_rules! push_ref {
        ($r:expr) => {
            try_push($r.0 as usize, slabs_snap, slab_min, slab_max, large_map, large_min, large_max, &mut work);
        };
    }

    // Fazların süreleri (MONO_GC_PHASES): kökler, işaretleme, arena denetimi, süpürme.
    let phases_on = phases_on();
    let t_start = std::time::Instant::now();
    // 1. İş parçacığı yığınları, iş parçacığı arenaları ve Java Thread nesnesi kökleri
    for &tp in &h.threads {
        unsafe {
            let bot = (*tp).stack_bot.load(Ordering::Acquire);
            let top = (*tp).stack_top;
            let (lo, hi) = if bot <= top { (bot, top) } else { (top, bot) };
            let mut p = (lo + 7) & !7;
            while p + 8 <= hi {
                let w = *(p as *const usize);
                try_push(w, slabs_snap, slab_min, slab_max, large_map, large_min, large_max, &mut work);
                p += 8;
            }
            for ch in &(*tp).arena {
                let alo = ch.ptr as usize;
                let ahi = alo + ch.used;
                let mut ap = alo;
                while ap + 8 <= ahi {
                    let w = *(ap as *const usize);
                    try_push(w, slabs_snap, slab_min, slab_max, large_map, large_min, large_max, &mut work);
                    ap += 8;
                }
            }
            for list in &(*tp).recycled {
                for &o in list {
                    try_push(o as usize, slabs_snap, slab_min, slab_max, large_map, large_min, large_max, &mut work);
                }
            }
            let jt = (*tp).java_thread.load(Ordering::Relaxed);
            if jt != 0 {
                try_push(jt, slabs_snap, slab_min, slab_max, large_map, large_min, large_max, &mut work);
            }
        }
    }

    let t_stacks = t_start.elapsed();
    // 2. Sınıf aynaları (Class nesneleri) ve statik referans alanları
    let hooks = rt::hooks();
    // 60 bin sınıfın aynası ve statikleri kök taramasının baskın kısmıdır (ölçüm: yığınlar ~0,2 ms, bu döngü ~6 ms). Dünya
    // durmuştur, okunanlar değişmez; bu yüzden sınıflar dilimlere bölünüp paralel taranır. İki iş parçacığı aynı nesneyi
    // işaretlerse iş yığınına iki kez girer, bu zararsızdır (işaretleme eş etkilidir).
    {
        let classes = hooks.all_classes;
        let workers = std::thread::available_parallelism().map_or(1, |n| n.get()).min(8);
        if workers > 1 && classes.len() >= 1024 {
            let chunk = classes.len().div_ceil(workers);
            let parts: Vec<Vec<usize>> = std::thread::scope(|sc| {
                let joins: Vec<_> = classes.chunks(chunk).map(|part| {
                    sc.spawn(move || {
                        let mut w: Vec<*mut Obj> = Vec::new();
                        for &c in part {
                            let mptr = c.mirror.0.load(Ordering::Relaxed);
                            if !mptr.is_null() {
                                try_push(mptr as usize, slabs_snap, slab_min, slab_max, large_map, large_min, large_max, &mut w);
                            }
                            for sp in c.static_refs {
                                let r = unsafe { *(sp.0 as *const Ref) };
                                try_push(r.0 as usize, slabs_snap, slab_min, slab_max, large_map, large_min, large_max, &mut w);
                            }
                        }
                        w.into_iter().map(|o| o as usize).collect::<Vec<usize>>()
                    })
                }).collect();
                joins.into_iter().map(|j| j.join().unwrap()).collect()
            });
            for v in parts {
                work.extend(v.into_iter().map(|a| a as *mut Obj));
            }
        } else {
            for &c in classes {
                let mptr = c.mirror.0.load(Ordering::Relaxed);
                if !mptr.is_null() {
                    try_push(mptr as usize, slabs_snap, slab_min, slab_max, large_map, large_min, large_max, &mut work);
                }
                for sp in c.static_refs {
                    let r = unsafe { *(sp.0 as *const Ref) };
                    push_ref!(r);
                }
            }
        }
    }
    for &c in &hooks.arrays {
        let mptr = c.mirror.0.load(Ordering::Relaxed);
        if !mptr.is_null() {
            try_push(mptr as usize, slabs_snap, slab_min, slab_max, large_map, large_min, large_max, &mut work);
        }
    }
    let t_classes = t_start.elapsed();
    for &c in &[
        &rt::CLS_P_BOOLEAN, &rt::CLS_P_BYTE, &rt::CLS_P_CHAR, &rt::CLS_P_SHORT,
        &rt::CLS_P_INT, &rt::CLS_P_LONG, &rt::CLS_P_FLOAT, &rt::CLS_P_DOUBLE, &rt::CLS_P_VOID,
    ] {
        let mptr = c.mirror.0.load(Ordering::Relaxed);
        if !mptr.is_null() {
            try_push(mptr as usize, slabs_snap, slab_min, slab_max, large_map, large_min, large_max, &mut work);
        }
    }
    for &ac_addr in &h.array_classes {
        let c = unsafe { &*(ac_addr as *const Class) };
        let mptr = c.mirror.0.load(Ordering::Relaxed);
        if !mptr.is_null() {
            try_push(mptr as usize, slabs_snap, slab_min, slab_max, large_map, large_min, large_max, &mut work);
        }
    }

    // 3. Sabitlenmiş referanslar (String.intern) ve literal yuvaları
    for &addr in &h.pinned_refs {
        try_push(addr, slabs_snap, slab_min, slab_max, large_map, large_min, large_max, &mut work);
    }
    for &slot in &h.static_slots {
        let r = unsafe { *slot };
        push_ref!(r);
    }

    let t_roots = t_start.elapsed();
    if phases_on {
        eprintln!("[mono:gc_roots] stacks={}us statics_etc={}us classes_loop={}us classes={} static_slots={} pinned={} threads={}", t_stacks.as_micros(), (t_roots - t_stacks).as_micros(), (t_classes - t_stacks).as_micros(), hooks.all_classes.len(), h.static_slots.len(), h.pinned_refs.len(), h.threads.len());
    }
    if begin_only {
        // Eşzamanlı döngünün başlangıcı: kökler anlık görüntüdür. İşaretleyici devralır, program devam eder.
        let mut slabs = h.slabs.clone();
        slabs.sort_unstable();
        let cyc = ConcCycle {
            work: work.iter().map(|&o| o as usize).collect(),
            slabs,
            slab_min,
            slab_max,
            large: large_map.clone(),
            large_min,
            large_max,
        };
        if phases_on {
            eprintln!("[mono:gc_phases] begin gc={} roots={}us t={}", h.gc_count, t_roots.as_micros(), unix_ms());
        }
        *G.conc.lock().unwrap() = Some(cyc);
        G.satb_on.store(true, Ordering::Release);
        G.cycle.store(true, Ordering::Release);
        h.collecting = false;
        G.gc_stopping.store(0, Ordering::Release);
        cv.notify_all();
        drop(h);
        if std::thread::Builder::new().name("mono-gc-mark".into()).spawn(marker_main).is_err() {
            // İşaretleyici iş parçacığı yok: döngüyü şimdi, bu iş parçacığında bitir (eşzamansız olmadığı gibi durdur-dünya)
            G.finishing.store(true, Ordering::Release);
            collect_impl(false);
            G.finishing.store(false, Ordering::Release);
        }
        std::hint::black_box(&_regs);
        return;
    }
    // 4. Kesin (precise) heap grafı işaretlemesi
    let check = arena_check_mode();
    let mut arena_ranges: Vec<(usize, usize)> = Vec::new();
    if check != 0 {
        for &tp in &h.threads {
            unsafe {
                for ch in &(*tp).arena { arena_ranges.push((ch.ptr as usize, ch.ptr as usize + ch.cap)); }
                // Bir çerçevede tutulan nesneler (rt::stack_obj) iş parçacığının yığınında yaşar. Onlara bir heap referansı
                // aynı ihlaldir.
                let bot = (*tp).stack_bot.load(Ordering::Acquire);
                let top = (*tp).stack_top;
                let (lo, hi) = if bot <= top { (bot, top) } else { (top, bot) };
                arena_ranges.push((lo, hi));
            }
        }
        arena_ranges.sort_unstable();
    }
    let in_arena = |a: usize| -> bool {
        if a == 0 || arena_ranges.is_empty() { return false; }
        let i = arena_ranges.partition_point(|&(lo, _)| lo <= a);
        i > 0 && a < arena_ranges[i - 1].1
    };
    let mut violations = 0u32;
    // Önce statikler: arena nesnesini tutan bir statik, kaçışın en açık biçimidir
    if check != 0 {
        for &c in hooks.all_classes {
            for sp in c.static_refs {
                let r = unsafe { *(sp.0 as *const Ref) };
                if in_arena(r.0 as usize) {
                    violations += 1;
                    if violations <= 20 { eprintln!("[mono:arena] escape violation: static of {} holds an arena object", c.name); }
                }
            }
        }
    }
    // Eşzamanlı bir döngü sırasında mutatörlerin üzerine yazdığı referanslar (eski değerleri anlık görüntüde canlıdır)
    for v in satb_take() {
        try_push(v, slabs_snap, slab_min, slab_max, large_map, large_min, large_max, &mut work);
    }
    while let Some(obj) = work.pop() {
        unsafe {
            let c = (*obj).class;
            let mp = c.mirror.0.load(Ordering::Relaxed);
            if !mp.is_null() {
                try_push(mp as usize, slabs_snap, slab_min, slab_max, large_map, large_min, large_max, &mut work);
            }
            if c.elem == b'A' {
                let n = (*(obj as *const Arr)).len.max(0) as usize;
                let data = (obj as *const u8).add(24) as *const Ref;
                for i in 0..n {
                    if check != 0 && in_arena((*data.add(i)).0 as usize) {
                        violations += 1;
                        if violations <= 20 { eprintln!("[mono:arena] escape violation: element {} of a {} holds an arena object", i, c.name); }
                    }
                    push_ref!(*data.add(i));
                }
            } else {
                let weak = weak_object(c);
                let skip = if weak {
                    G.weak_seen.lock().unwrap().push(obj as usize);
                    weak_info().1
                } else {
                    u32::MAX
                };
                for &off in c.ref_offsets {
                    if off == skip {
                        continue; // a weak referent is not traced
                    }
                    let r = *((obj as *const u8).add(off as usize) as *const Ref);
                    if check != 0 && in_arena(r.0 as usize) {
                        violations += 1;
                        if violations <= 20 {
                            let tgt = (*r.0).class.name;
                            eprintln!("[mono:arena] escape violation: {} (field at +{}) holds an arena {}", c.name, off, tgt);
                        }
                    }
                    push_ref!(r);
                }
            }
        }
    }

    if check != 0 && violations > 0 {
        eprintln!("[mono:arena] {} escape violation(s) at collection {}", violations, h.gc_count);
        if check == 2 { rt::abort("arena escape violation"); }
    }
    clear_dead_referents();
    // Ölü nesnelerin monitörleri (bir kez kilitlenen nesne, ölene kadar tablo girdisini tutar)
    crate::sync::prune_dead_monitors(|a| unsafe {
        let g = (*(a as *mut Obj)).gc;
        g & GC_HEAP != 0 && g & GC_MARK != cur_sense()
    });
    let t_mark = t_start.elapsed() - t_roots;
    // 5. Dilimleri süpür
    // Süpürme bölümünün alt faz sayaçları (MONO_GC_PHASES açıkken `sweep_detail` satırına yazılır): hangi adımın duraklamada
    // uzun sürdüğünü ayırmak için. Anahtar kapalıyken sonuçlar kullanılmaz; her biri tek bir saat okumasıdır.
    let t_sw0 = std::time::Instant::now();
    let since_gc = G.bytes_since_gc.load(Ordering::Relaxed);
    let mut census = crate::prof::sweep_begin();
    for sc in 0..NUM_CLASSES {
        h.partial[sc].clear();
    }
    let lazy = lazy_enabled();
    let sense = cur_sense();
    let defer = lazy && census.is_none();
    if defer {
        // Tembel süpürme: dilimler burada süpürülmez; sınıf listelerine konur, tahsis sırasında süpürülür.
        for sc in 0..NUM_CLASSES {
            h.unswept[sc].clear();
        }
        let in_use: Vec<(usize, usize)> = h.slabs.iter().filter_map(|&b| unsafe {
            let s = &*(b as *const Slab);
            if s.in_use { Some((b, s.sc as usize)) } else { None }
        }).collect();
        for (b, sc) in in_use {
            h.unswept[sc].push(b as *mut Slab);
        }
    }
    let t_sw1 = std::time::Instant::now(); // sınıf listeleri ve tembel süpürme listesi hazır
    let slabs_old = if defer { Vec::new() } else { std::mem::take(&mut h.slabs) };
    let mut new_live = 0usize;
    let mut new_slabs = Vec::with_capacity(slabs_old.len());
    let mut to_dealloc = Vec::new();
    // Sayım (profil) kapalıysa dilimlerin içi paralel süpürülür: her dilim bağımsızdır (kendi işaret bitleri, kendi serbest
    // listesi), dünya zaten durdurulmuştur. Sonuç `live_count` alanına yazılır; aşağıdaki sıralı döngü yalnızca hesap tutar.
    let pre_swept = census.is_none() && !defer;
    if pre_swept {
        let in_use: Vec<usize> = slabs_old.iter().copied().filter(|&b| unsafe { (*(b as *mut Slab)).in_use }).collect();
        let workers = std::thread::available_parallelism().map_or(1, |n| n.get()).min(8);
        if workers > 1 && in_use.len() >= 64 {
            let chunk = in_use.len().div_ceil(workers);
            std::thread::scope(|sc| {
                for part in in_use.chunks(chunk) {
                    sc.spawn(move || {
                        for &b in part {
                            unsafe { sweep_slab_nocensus(b) };
                        }
                    });
                }
            });
        } else {
            for &b in &in_use {
                unsafe { sweep_slab_nocensus(b) };
            }
        }
    }
    for &base in &slabs_old {
        unsafe {
            let s = &mut *(base as *mut Slab);
            if !s.in_use {
                new_slabs.push(base);
                continue;
            }
            let n = s.next_bump;
            let sz = s.slot_size as usize;
            let mut live_c = 0u16;
            if pre_swept {
                live_c = s.live_count;
                new_live += live_c as usize * sz;
            } else {
            s.free_head = -1;
            // Serbest listeyi tersten yeniden kur; böylece düşük indisler önce çıkar
            for idx in (0..n).rev() {
                let off = SLAB_HDR + idx as usize * sz;
                let bit = off >> 4;
                if !s.alloc_bit(bit) {
                    let p = (base + off) as *mut i32;
                    *p = s.free_head;
                    s.free_head = idx as i32;
                    continue;
                }
                let o = (base + off) as *mut Obj;
                if ((*o).gc & GC_MARK) == sense {
                    if !lazy {
                        (*o).gc &= !GC_MARK;
                    }
                    if let Some(c) = census.as_mut() { c.live(o, sz); }
                    live_c += 1;
                    new_live += sz;
                } else {
                    if let Some(c) = census.as_mut() { c.dead(o, sz); }
                    s.clear_bit(bit);
                    if poison_enabled() {
                        // Tanılama: serbest bırakılmış bir nesnenin bayat okuması artık makul veri yerine 0xDDDD... verir
                        std::ptr::write_bytes((base + off) as *mut u8, 0xDD, sz);
                    }
                    let p = (base + off) as *mut i32;
                    *p = s.free_head;
                    s.free_head = idx as i32;
                }
            }
            }
            s.live_count = live_c;
            if live_c == 0 {
                s.in_use = false;
                if h.empty.len() < 8 {
                    h.empty.push(s as *mut Slab);
                    new_slabs.push(base);
                } else {
                    to_dealloc.push(base);
                    h.heap_bytes = h.heap_bytes.saturating_sub(SLAB_SIZE);
                }
            } else {
                if s.free_head >= 0 || s.next_bump < s.n_slots {
                    h.partial[s.sc as usize].push(s as *mut Slab);
                }
                new_slabs.push(base);
            }
        }
    }
    if !defer {
        h.slabs = new_slabs;
    }
    let t_sw2 = std::time::Instant::now(); // dilim süpürme döngüsü bitti (tembel kipte boş)
    for base in to_dealloc {
        unsafe { dealloc(base as *mut u8, Layout::from_size_align(SLAB_SIZE, SLAB_SIZE).unwrap()) };
    }

    let t_sw3 = std::time::Instant::now(); // boş dilimlerin iadesi bitti
    // 6. Büyük nesneleri süpür
    let mut dead_large = Vec::new();
    // büyük nesnelerin gerçek `free` çağrıları (munmap) duraklama içinde değil, dünya yeniden başladıktan sonra yapılır
    let mut late_free: Vec<(usize, usize)> = Vec::new();
    for (&addr, &sz) in &h.large {
        unsafe {
            let o = addr as *mut Obj;
            if ((*o).gc & GC_MARK) == sense {
                if !lazy {
                    (*o).gc &= !GC_MARK;
                }
                if let Some(c) = census.as_mut() { c.live(o, sz); }
                new_live += sz;
            } else {
                if let Some(c) = census.as_mut() { c.dead(o, sz); }
                dead_large.push((addr, sz));
            }
        }
    }
    let n_dead_large = dead_large.len();
    for (addr, sz) in dead_large {
        h.large.remove(&addr);
        h.heap_bytes = h.heap_bytes.saturating_sub(sz);
        late_free.push((addr, sz));
    }

    if phases_on {
        // Süpürme bölümünün alt fazları: liste kurma, dilim süpürme (tembel kipte boş), boş dilim iadesi, büyük nesneler.
        eprintln!(
            "[mono:gc_phases] sweep_detail gc={} defer={} list={}us slabs={}us dealloc={}us large={}us slab_count={} large_count={} dead_large={}",
            h.gc_count,
            defer as u8,
            (t_sw1 - t_sw0).as_micros(),
            (t_sw2 - t_sw1).as_micros(),
            (t_sw3 - t_sw2).as_micros(),
            t_sw3.elapsed().as_micros(),
            h.slabs.len(),
            h.large.len(),
            n_dead_large
        );
        let t_sweep = t_start.elapsed() - t_roots - t_mark;
        eprintln!("[mono:gc_phases] gc={} wait={}us roots={}us mark={}us sweep={}us total={}us live={}B t={}", h.gc_count, t_wait.as_micros(), t_roots.as_micros(), t_mark.as_micros(), t_sweep.as_micros(), t_entry.elapsed().as_micros(), new_live, unix_ms());
    }
    if let Some(c) = census.take() {
        c.finish(h.gc_count, new_live, h.heap_bytes, since_gc, stw_start);
    }
    if defer {
        // Dilimler süpürülmediği için canlı boyut işaretleme sırasında sayılan baytlardır (siyah doğanlar hariç).
        new_live = MARKED_BYTES.load(Ordering::Relaxed);
    }
    h.live_bytes = new_live;
    h.gc_threshold = (new_live * 2).max(16 * 1024 * 1024);
    G.bytes_since_gc.store(0, Ordering::Relaxed);
    if std::env::var_os("MONO_TRACE_PARK").is_some() {
        eprintln!("[mono:gc_stw] done gc={} threads={} slabs={} large={} live={}B", h.gc_count, h.threads.len(), h.slabs.len(), h.large.len(), new_live);
        for (i, &tp) in h.threads.iter().enumerate() {
            unsafe { eprintln!("  t{}: bot={:#x} top={:#x} len={}", i, (*tp).stack_bot.load(Ordering::Relaxed), (*tp).stack_top, (*tp).stack_top.saturating_sub((*tp).stack_bot.load(Ordering::Relaxed))); }
        }
    }
    if G.cycle.load(Ordering::Acquire) {
        // Eşzamanlı bir döngünün son toplaması: program yeniden çalışmadan önce bariyer kapanır
        G.satb_on.store(false, Ordering::Release);
        G.cycle.store(false, Ordering::Release);
    }
    crate::region::cycle_end(); // yığın kelimeleriyle gösterilmeyen bölge parçaları serbest bırakılır
    h.collecting = false;
    G.gc_stopping.store(0, Ordering::Release);
    cv.notify_all();
    drop(h);
    for (addr, sz) in late_free {
        unsafe { dealloc(addr as *mut u8, Layout::from_size_align(sz, 16).unwrap()) };
    }
    std::hint::black_box(&_regs);
}

pub fn live_bytes() -> usize {
    heap_sync().0.lock().unwrap().live_bytes
}
pub fn heap_bytes() -> usize {
    heap_sync().0.lock().unwrap().heap_bytes
}

// ------------------------------------------------------------------ Kapsama bağlı nesnelerin geri dönüşümü
//
// `for (x : list)` döngüsü bir yineleyici oluşturur, onu yalnızca hasNext/next/remove ile kullanır ve döngü bitince bırakır.
// Çevirici bu biçimi tanır (monolith/rustgen.py, `find_iterator_scopes`) ve yineleyicinin kapsamı bittiği yerde `recycle` çağırır.
// Geri dönüştürülebilir bir sınıfın her `new`'ü önce `alloc_recyclable`'a sorar. Rust'taki yaklaşım budur: sahibi belli bir değerin
// kapsamı bitince serbest bırakılır; kimsenin onu sonradan aramasına gerek yoktur.
// Yalnızca `alloc_recyclable` ile ayrılan sınıflar geri dönüştürülür; başka bir sınıfın paylaşılan ya da önbelleğe alınmış örneğine dokunulmaz.
// `MONO_RECYCLE=0` bunu kapatır.

pub const RECYCLE_SLOTS: usize = 32;
const RECYCLE_KEEP: usize = 16;


fn recycle_enabled() -> bool {
    knobs().recycle
}

/// Geri dönüştürülebilir bir sınıfın `new`'ü: iş parçacığında o sınıftan geri dönüştürülmüş bir nesne varsa o, yoksa yeni bir heap nesnesi.
pub fn alloc_recyclable(class: &'static Class, slot: usize) -> Ref {
    if recycle_enabled() {
        G.recycle_classes[slot].store(class as *const Class as usize, Ordering::Relaxed);
        let reg = cur_reg();
        if !reg.is_null() {
            if let Some(o) = unsafe { (*reg).recycled[slot].pop() } {
                crate::prof::on_alloc(class, class.size, true);
                unsafe { (*o).gc = new_obj_bits(); }
                return Ref(o);
            }
        }
    }
    Ref(alloc_obj(class, class.size.max(16)))
}

/// `r`'nin kapsamı bitti ve başka hiçbir şey ona referans vermiyor: sıfırlanır ve sınıfının bir sonraki `new`'ü için saklanır.
pub fn recycle(r: Ref) {
    if r.is_null() || !recycle_enabled() {
        return;
    }
    let reg = cur_reg();
    if reg.is_null() {
        return;
    }
    unsafe {
        let class = (*r.0).class;
        let cp = class as *const Class as usize;
        for (slot, owner) in G.recycle_classes.iter().enumerate().take(RECYCLE_SLOTS) {
            if owner.load(Ordering::Relaxed) == cp {
                let list = &mut (*reg).recycled[slot];
                if list.len() < RECYCLE_KEEP && (*r.0).gc & GC_HEAP != 0 {
                    // Başlığı koru; nesnenin tuttuğu şeyleri bırak ki çöp toplayıcı bayat referansları izlemesin.
                    // Tuttuğu alanlar bariyersiz üzerine yazılır, bu yüzden eski değerleri önce SATB günlüğüne gitmelidir:
                    // anlık görüntüden işaretleyen bir döngü yine de onlara ulaşabilmelidir.
                    let body = class.size.max(16) - 16;
                    satb_log_range((r.0 as *const u8).add(16), body);
                    std::ptr::write_bytes((r.0 as *mut u8).add(16), 0, body);
                    list.push(r.0);
                }
                return;
            }
        }
    }
}
