//! Stop-the-world segregated-slab Mark-Sweep GC + thread-local non-escaping bump arena.
use crate::rt::{self, Arr, Class, FLoc, Obj, Ref, Res};
use std::alloc::{alloc_zeroed, dealloc, Layout};
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};

pub const GC_HEAP: u32 = 1;
pub const GC_MARK: u32 = 2;
pub const GC_ARENA: u32 = 4;

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
        Some((sz.max(16) + 15) / 16 - 1)
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
    /// Iterator objects handed back at the end of a for-each loop (see `recycle`), per recyclable class slot.
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

pub static GC_STOPPING: AtomicU32 = AtomicU32::new(0);
static BYTES_SINCE_GC: AtomicUsize = AtomicUsize::new(0);

struct HeapState {
    threads: Vec<*mut ThreadReg>,
    slabs: Vec<usize>,
    slab_min: usize,
    slab_max: usize,
    partial: [Vec<*mut Slab>; NUM_CLASSES],
    empty: Vec<*mut Slab>,
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
    let p = std::hint::black_box(&mut x as *mut usize as usize);
    p
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
    TLS_REG.with(|c| unsafe { *c.0.get() = std::ptr::null_mut() });
    let (m, cv) = heap_sync();
    let mut h = m.lock().unwrap();
    while h.collecting {
        h = cv.wait(h).unwrap();
    }
    h.threads.retain(|&p| p != reg);
    unsafe {
        let b = Box::from_raw(reg);
        for s in b.active {
            if !s.is_null() {
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
            if GC_STOPPING.load(Ordering::Relaxed) != 0 {
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
            if GC_STOPPING.load(Ordering::Acquire) != 0 {
                safepoint_slow();
            }
        }
    }
}

#[inline(always)]
pub fn safepoint() {
    if GC_STOPPING.load(Ordering::Relaxed) != 0 {
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
    if GC_STOPPING.load(Ordering::Acquire) != 0 {
        unsafe { (*reg).paused.store(true, Ordering::SeqCst) };
        cv.notify_all();
        while GC_STOPPING.load(Ordering::Acquire) != 0 {
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

// ------------------------------------------------------------------ allocation
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
                        (*o).gc = GC_HEAP;
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
    let prev = BYTES_SINCE_GC.fetch_add(add, Ordering::Relaxed);
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
        let found = h.partial[sc].pop().unwrap_or_else(|| {
            if let Some(es) = h.empty.pop() {
                unsafe {
                    Slab::init_at(es, sc);
                    std::ptr::write_bytes((es as *mut u8).add(SLAB_HDR), 0, SLAB_SIZE - SLAB_HDR);
                }
                es
            } else {
                if BYTES_SINCE_GC.load(Ordering::Relaxed) >= h.gc_threshold {
                    need_gc = true;
                }
                let s = Slab::new(sc);
                h.note_slab(s);
                s
            }
        });
        unsafe { (*reg).active[sc] = found };
        found
    };
    if need_gc {
        collect();
        // After GC, our active[sc] might have been cleared, recurse cleanly once via alloc_obj:
        return alloc_obj_inner(class, SIZE_CLASSES[sc]);
    }
    unsafe {
        let o = (*slab).alloc_slot().expect("fresh slab has slot");
        (*o).class = class;
        (*o).gc = GC_HEAP;
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
        let total = BYTES_SINCE_GC.fetch_add(aligned, Ordering::Relaxed) + aligned;
        if total >= thresh {
            collect();
        }
    }
    let layout = Layout::from_size_align(aligned, 16).unwrap();
    let p = unsafe { alloc_zeroed(layout) } as *mut Obj;
    unsafe {
        (*p).class = class;
        (*p).gc = GC_HEAP;
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

// ------------------------------------------------------------------ Non-escaping Bump Arena
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

/// Allocation site proven non-escaping by the escape analysis: bump-allocated in the thread's scope arena, released
/// when the method returns (`ArenaGuard`). `MONO_ARENA=0` turns it off (those objects then come from the heap), for
/// comparing runs and as a rollback if the analysis is ever wrong; `MONO_ARENA_CHECK=1` verifies at every collection
/// that no heap object or static points into an arena.
pub fn alloc_arena(class: &'static Class, size: usize) -> Ref {
    crate::prof::on_alloc(class, size, true);
    if !arena_enabled() {
        return Ref(alloc_obj_inner(class, size));
    }
    alloc_arena_bump(class, size)
}

fn arena_enabled() -> bool {
    static V: OnceLock<bool> = OnceLock::new();
    *V.get_or_init(|| std::env::var("MONO_ARENA").map_or(true, |v| v != "0"))
}

/// 0 off, 1 report, 2 abort on the first violation.
fn arena_check_mode() -> u8 {
    static V: OnceLock<u8> = OnceLock::new();
    *V.get_or_init(|| match std::env::var("MONO_ARENA_CHECK").as_deref() {
        Ok("abort") => 2,
        Ok(v) if !v.is_empty() && v != "0" => 1,
        _ => 0,
    })
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

// ------------------------------------------------------------------ Mark & Sweep Collector
pub fn collect() {
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
    GC_STOPPING.store(1, Ordering::SeqCst);

    // Wait for all other active threads to park at a safepoint or be inside ScopedBlock
    loop {
        let mut all_ready = true;
        for &tp in &h.threads {
            if tp == reg {
                continue;
            }
            unsafe {
                if !(*tp).paused.load(Ordering::Acquire) && (*tp).blocked.load(Ordering::Acquire) == 0 {
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
    h.gc_count += 1;
    // Return thread-local active slabs to global pools so everything is swept cleanly
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

    h.slabs.sort_unstable();
    let slabs_snap = &h.slabs;
    let slab_min = h.slab_min;
    let slab_max = h.slab_max;
    let large_map = &h.large;
    let large_min = h.large_min;
    let large_max = h.large_max;

    let mut work: Vec<*mut Obj> = Vec::with_capacity(4096);

    #[inline(always)]
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
                        if ((*o).gc & (GC_HEAP | GC_MARK)) == GC_HEAP {
                            (*o).gc |= GC_MARK;
                            work.push(o);
                        }
                    }
                }
                return;
            }
        }
        if addr >= large_min && addr < large_max && large_map.contains_key(&addr) {
            unsafe {
                let o = addr as *mut Obj;
                if ((*o).gc & (GC_HEAP | GC_MARK)) == GC_HEAP {
                    (*o).gc |= GC_MARK;
                    work.push(o);
                }
            }
        }
    }

    macro_rules! push_ref {
        ($r:expr) => {
            try_push($r.0 as usize, slabs_snap, slab_min, slab_max, large_map, large_min, large_max, &mut work);
        };
    }

    // Fazların süreleri (MONO_GC_PHASES): kökler, işaretleme, arena denetimi, süpürme.
    let phases_on = std::env::var_os("MONO_GC_PHASES").is_some();
    let t_start = std::time::Instant::now();
    // 1. Thread stacks + thread arenas + Java Thread object roots
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

    // 2. Class mirrors and static reference fields
    let hooks = rt::hooks();
    for &c in hooks.all_classes {
        let mptr = c.mirror.0.load(Ordering::Relaxed);
        if !mptr.is_null() {
            try_push(mptr as usize, slabs_snap, slab_min, slab_max, large_map, large_min, large_max, &mut work);
        }
        for sp in c.static_refs {
            let r = unsafe { *(sp.0 as *const Ref) };
            push_ref!(r);
        }
    }
    for &c in &hooks.arrays {
        let mptr = c.mirror.0.load(Ordering::Relaxed);
        if !mptr.is_null() {
            try_push(mptr as usize, slabs_snap, slab_min, slab_max, large_map, large_min, large_max, &mut work);
        }
    }
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

    // 3. Pinned refs (String.intern) and literal slots
    for &addr in &h.pinned_refs {
        try_push(addr, slabs_snap, slab_min, slab_max, large_map, large_min, large_max, &mut work);
    }
    for &slot in &h.static_slots {
        let r = unsafe { *slot };
        push_ref!(r);
    }

    let t_roots = t_start.elapsed();
    // 4. Precise heap graph mark
    let check = arena_check_mode();
    let mut arena_ranges: Vec<(usize, usize)> = Vec::new();
    if check != 0 {
        for &tp in &h.threads {
            unsafe {
                for ch in &(*tp).arena { arena_ranges.push((ch.ptr as usize, ch.ptr as usize + ch.cap)); }
                // objects kept in a frame (rt::stack_obj) live on the thread's stack: a heap reference to one is the same violation
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
    // statics first: a static holding an arena object is the plainest escape
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
                for &off in c.ref_offsets {
                    let r = *((obj as *const u8).add(off as usize) as *const Ref);
                    if check != 0 && in_arena(r.0 as usize) {
                        violations += 1;
                        if violations <= 20 {
                            let tgt = unsafe { (*r.0).class.name };
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
    let t_mark = t_start.elapsed() - t_roots;
    // 5. Sweep slabs
    let since_gc = BYTES_SINCE_GC.load(Ordering::Relaxed);
    let mut census = crate::prof::sweep_begin();
    for sc in 0..NUM_CLASSES {
        h.partial[sc].clear();
    }
    let slabs_old = std::mem::take(&mut h.slabs);
    let mut new_live = 0usize;
    let mut new_slabs = Vec::with_capacity(slabs_old.len());
    let mut to_dealloc = Vec::new();
    for &base in &slabs_old {
        unsafe {
            let s = &mut *(base as *mut Slab);
            if !s.in_use {
                new_slabs.push(base);
                continue;
            }
            let n = s.next_bump;
            let sz = s.slot_size as usize;
            s.free_head = -1;
            let mut live_c = 0u16;
            // Rebuild free list in reverse so low indices pop first
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
                if ((*o).gc & GC_MARK) != 0 {
                    (*o).gc &= !GC_MARK;
                    if let Some(c) = census.as_mut() { c.live(o, sz); }
                    live_c += 1;
                    new_live += sz;
                } else {
                    if let Some(c) = census.as_mut() { c.dead(o, sz); }
                    s.clear_bit(bit);
                    let p = (base + off) as *mut i32;
                    *p = s.free_head;
                    s.free_head = idx as i32;
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
    h.slabs = new_slabs;
    for base in to_dealloc {
        unsafe { dealloc(base as *mut u8, Layout::from_size_align(SLAB_SIZE, SLAB_SIZE).unwrap()) };
    }

    // 6. Sweep large objects
    let mut dead_large = Vec::new();
    for (&addr, &sz) in &h.large {
        unsafe {
            let o = addr as *mut Obj;
            if ((*o).gc & GC_MARK) != 0 {
                (*o).gc &= !GC_MARK;
                if let Some(c) = census.as_mut() { c.live(o, sz); }
                new_live += sz;
            } else {
                if let Some(c) = census.as_mut() { c.dead(o, sz); }
                dead_large.push((addr, sz));
            }
        }
    }
    for (addr, sz) in dead_large {
        h.large.remove(&addr);
        h.heap_bytes = h.heap_bytes.saturating_sub(sz);
        unsafe { dealloc(addr as *mut u8, Layout::from_size_align(sz, 16).unwrap()) };
    }

    if phases_on {
        let t_sweep = t_start.elapsed() - t_roots - t_mark;
        eprintln!("[mono:gc_phases] gc={} roots={}us mark={}us sweep={}us live={}B", h.gc_count, t_roots.as_micros(), t_mark.as_micros(), t_sweep.as_micros(), new_live);
    }
    if let Some(c) = census.take() {
        c.finish(h.gc_count as u64, new_live, h.heap_bytes, since_gc, stw_start);
    }
    h.live_bytes = new_live;
    h.gc_threshold = (new_live * 2).max(16 * 1024 * 1024);
    BYTES_SINCE_GC.store(0, Ordering::Relaxed);
    if std::env::var_os("MONO_TRACE_PARK").is_some() {
        eprintln!("[mono:gc_stw] done gc={} threads={} slabs={} large={} live={}B", h.gc_count, h.threads.len(), h.slabs.len(), h.large.len(), new_live);
        for (i, &tp) in h.threads.iter().enumerate() {
            unsafe { eprintln!("  t{}: bot={:#x} top={:#x} len={}", i, (*tp).stack_bot.load(Ordering::Relaxed), (*tp).stack_top, (*tp).stack_top.saturating_sub((*tp).stack_bot.load(Ordering::Relaxed))); }
        }
    }
    h.collecting = false;
    GC_STOPPING.store(0, Ordering::Release);
    cv.notify_all();
    std::hint::black_box(&_regs);
}

pub fn live_bytes() -> usize {
    heap_sync().0.lock().unwrap().live_bytes
}
pub fn heap_bytes() -> usize {
    heap_sync().0.lock().unwrap().heap_bytes
}

// ------------------------------------------------------------------ recycling of scope-bound objects
//
// A `for (x : list)` loop creates an iterator, uses it only through hasNext/next/remove and drops it when the loop
// ends. The translator recognizes that shape (monolith/rustgen.py, `find_iterator_scopes`) and calls `recycle` where
// the iterator goes out of scope; every `new` of a recyclable iterator class asks `alloc_recyclable` first. That is
// the Rust way for a value with a clear owner: the end of its scope releases it, nobody has to find it later.
// Only classes that are allocated through `alloc_recyclable` are ever recycled, so a shared or cached instance of
// another class is never touched. `MONO_RECYCLE=0` turns it off.

pub const RECYCLE_SLOTS: usize = 32;
const RECYCLE_KEEP: usize = 16;

static RECYCLE_CLASSES: [AtomicUsize; RECYCLE_SLOTS] = [const { AtomicUsize::new(0) }; RECYCLE_SLOTS];

fn recycle_enabled() -> bool {
    static V: OnceLock<bool> = OnceLock::new();
    *V.get_or_init(|| std::env::var("MONO_RECYCLE").map_or(true, |v| v != "0"))
}

/// `new` of a recyclable class: a recycled object of that class when the thread has one, else a fresh heap object.
pub fn alloc_recyclable(class: &'static Class, slot: usize) -> Ref {
    if recycle_enabled() {
        RECYCLE_CLASSES[slot].store(class as *const Class as usize, Ordering::Relaxed);
        let reg = cur_reg();
        if !reg.is_null() {
            if let Some(o) = unsafe { (*reg).recycled[slot].pop() } {
                crate::prof::on_alloc(class, class.size, true);
                unsafe { (*o).gc = GC_HEAP; }
                return Ref(o);
            }
        }
    }
    Ref(alloc_obj(class, class.size.max(16)))
}

/// The scope of `r` ended and nothing else refers to it: zero it and keep it for the next `new` of its class.
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
        for slot in 0..RECYCLE_SLOTS {
            if RECYCLE_CLASSES[slot].load(Ordering::Relaxed) == cp {
                let list = &mut (*reg).recycled[slot];
                if list.len() < RECYCLE_KEEP && (*r.0).gc & GC_HEAP != 0 {
                    // keep the header, drop what the object held so the collector does not follow stale references
                    std::ptr::write_bytes((r.0 as *mut u8).add(16), 0, class.size.max(16) - 16);
                    list.push(r.0);
                }
                return;
            }
        }
    }
}
