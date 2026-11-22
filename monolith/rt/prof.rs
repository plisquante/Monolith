//! Allocation and lifetime profiler, `MONO_ALLOC_PROFILE=<file>`.
//!
//! Purpose: decide, with data, how to leave the garbage collector. For every class it records how much is allocated,
//! how long the objects live (the age of each object when it dies or is still alive: number of collections survived,
//! 0 to 3+) and, by sampling, which code allocates it. `tools/alloc_report.py` turns the file into the plan: which
//! allocations are temporaries (arena or stack candidates), which are long-lived data (owned by a chunk, a world or
//! the program) and which methods to patch first.
//!
//! Costs nothing when the variable is unset (one relaxed load per allocation). Written every `MONO_ALLOC_DUMP_SECS`
//! seconds (default 10) and at exit, so a killed process still leaves data.
//!
//! * `MONO_ALLOC_PROFILE=<file>`   enable, write JSON to <file> (and the class list to <file>.classes once)
//! * `MONO_ALLOC_SAMPLE=<bytes>`   one allocation site sample per this many allocated bytes per thread (default 65536)
//! * `MONO_ALLOC_DUMP_SECS=<n>`    dump period
//! * `MONO_ALLOC_SNAPSHOTS=1`      also keep every periodic dump as <file>.<elapsed seconds>, so two of them can be subtracted
//!                                 (`alloc_report.py --base`): start-up versus steady state
use crate::rt::{self, Class, Obj};
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

pub static ON: AtomicBool = AtomicBool::new(false);
static INTERVAL: AtomicUsize = AtomicUsize::new(65536);

/// Frames kept per allocation site (the first ones belong to the allocator and are stripped by the report).
const DEPTH: usize = 14;
const MAX_SITES: usize = 400_000;
pub const AGE_SHIFT: u32 = 8;
pub const AGE_MASK: u32 = 3 << AGE_SHIFT;

#[derive(Default)]
struct Counters {
    n: AtomicU64,
    b: AtomicU64,
    arena_n: AtomicU64,
    arena_b: AtomicU64,
}

struct Census {
    dead_n: Vec<[u64; 4]>,
    dead_b: Vec<[u64; 4]>,
    live_n: Vec<[u64; 4]>,
    live_b: Vec<[u64; 4]>,
    gcs: Vec<GcRec>,
}

struct GcRec {
    n: u64,
    t_ms: u64,
    live: usize,
    heap: usize,
    since: usize,
    pause_us: u64,
}

/// Class pointer -> dense index, immutable after `init` (lock-free reads on the allocation path).
struct Table {
    mask: usize,
    keys: Box<[usize]>,
    idx: Box<[u32]>,
    classes: Vec<&'static Class>,
}

impl Table {
    fn build(classes: Vec<&'static Class>) -> Table {
        let cap = (classes.len() * 2 + 16).next_power_of_two();
        let mut t = Table { mask: cap - 1, keys: vec![0; cap].into(), idx: vec![0; cap].into(), classes };
        for i in 0..t.classes.len() {
            let k = t.classes[i] as *const Class as usize;
            let mut h = Self::hash(k) & t.mask;
            while t.keys[h] != 0 && t.keys[h] != k {
                h = (h + 1) & t.mask;
            }
            t.keys[h] = k;
            t.idx[h] = i as u32;
        }
        t
    }
    #[inline(always)]
    fn hash(k: usize) -> usize {
        (k >> 4).wrapping_mul(0x9E37_79B9_7F4A_7C15u64 as usize) >> 20
    }
    #[inline(always)]
    fn get(&self, c: &Class) -> Option<usize> {
        let k = c as *const Class as usize;
        let mut h = Self::hash(k) & self.mask;
        loop {
            let kk = self.keys[h];
            if kk == k {
                return Some(self.idx[h] as usize);
            }
            if kk == 0 {
                return None;
            }
            h = (h + 1) & self.mask;
        }
    }
}

struct State {
    table: Table,
    counters: Box<[Counters]>,
    other: Counters, // classes the table does not know (never expected)
    start: Instant,
    path: String,
    census: Mutex<Census>,
    sites: Mutex<HashMap<(u32, [usize; DEPTH]), (u64, u64)>>,
    threads: Mutex<HashMap<String, (u64, u64)>>,
    dropped_sites: AtomicU64,
}

static STATE: OnceLock<State> = OnceLock::new();

#[inline(always)]
pub fn on() -> bool {
    ON.load(Ordering::Relaxed)
}

pub fn init() {
    let Some(path) = std::env::var_os("MONO_ALLOC_PROFILE") else { return };
    let path = path.to_string_lossy().into_owned();
    if path.is_empty() || STATE.get().is_some() {
        return;
    }
    let h = rt::hooks();
    let mut classes: Vec<&'static Class> = h.all_classes.to_vec();
    classes.extend(h.arrays.iter().copied());
    classes.push(h.object_class);
    let n = classes.len();
    if let Some(v) = std::env::var("MONO_ALLOC_SAMPLE").ok().and_then(|v| v.parse::<usize>().ok()) {
        INTERVAL.store(v.max(1024), Ordering::Relaxed);
    }
    let table = Table::build(classes);
    let st = State {
        counters: (0..n).map(|_| Counters::default()).collect::<Vec<_>>().into(),
        other: Counters::default(),
        table,
        start: Instant::now(),
        path,
        census: Mutex::new(Census {
            dead_n: vec![[0; 4]; n], dead_b: vec![[0; 4]; n], live_n: vec![[0; 4]; n], live_b: vec![[0; 4]; n], gcs: Vec::new(),
        }),
        sites: Mutex::new(HashMap::new()),
        threads: Mutex::new(HashMap::new()),
        dropped_sites: AtomicU64::new(0),
    };
    if STATE.set(st).is_err() {
        return;
    }
    write_class_list();
    ON.store(true, Ordering::Release);
    let secs = std::env::var("MONO_ALLOC_DUMP_SECS").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(10).max(1);
    let _ = std::thread::Builder::new().name("mono-prof".into()).spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(secs));
        dump();
    });
    unsafe { atexit(at_exit) };
    eprintln!("[mono:prof] allocation profile -> {} (sample every {} B)", STATE.get().unwrap().path, INTERVAL.load(Ordering::Relaxed));
}

extern "C" {
    fn atexit(cb: extern "C" fn()) -> i32;
}
extern "C" fn at_exit() {
    dump();
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
extern "C" {
    fn backtrace(buf: *mut *mut std::ffi::c_void, size: i32) -> i32;
}

thread_local! {
    static ACC: Cell<usize> = const { Cell::new(0) };
}

/// Called by the allocator for every object. `arena`: the allocation site was proven non-escaping (it would live in a
/// scope arena once that is enabled) but today still comes from the heap.
#[inline(always)]
pub fn on_alloc(class: &'static Class, size: usize, arena: bool) {
    if ON.load(Ordering::Relaxed) {
        on_alloc_slow(class, size, arena);
    }
}

#[inline(never)]
fn on_alloc_slow(class: &'static Class, size: usize, arena: bool) {
    let Some(st) = STATE.get() else { return };
    let idx = st.table.get(class);
    let c = match idx {
        Some(i) => &st.counters[i],
        None => &st.other,
    };
    let sz = ((size.max(16)) + 15) & !15;
    c.n.fetch_add(1, Ordering::Relaxed);
    c.b.fetch_add(sz as u64, Ordering::Relaxed);
    if arena {
        c.arena_n.fetch_add(1, Ordering::Relaxed);
        c.arena_b.fetch_add(sz as u64, Ordering::Relaxed);
    }
    let interval = INTERVAL.load(Ordering::Relaxed);
    let due = ACC.with(|a| {
        let v = a.get() + sz;
        if v >= interval {
            a.set(v % interval);
            Some((v / interval) * interval)
        } else {
            a.set(v);
            None
        }
    });
    if let Some(weight) = due {
        sample(st, idx.map_or(u32::MAX, |i| i as u32), weight as u64);
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn capture() -> [usize; DEPTH + 4] {
    let mut buf = [std::ptr::null_mut::<std::ffi::c_void>(); DEPTH + 4];
    let n = unsafe { backtrace(buf.as_mut_ptr(), (DEPTH + 4) as i32) } as usize;
    let mut out = [0usize; DEPTH + 4];
    for i in 0..n.min(DEPTH + 4) {
        out[i] = buf[i] as usize;
    }
    out
}
#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
fn capture() -> [usize; DEPTH + 4] {
    [0; DEPTH + 4]
}

fn sample(st: &State, class_idx: u32, weight: u64) {
    let raw = capture();
    // drop the profiler's own frames (sample, on_alloc_slow, on_alloc): the report strips the allocator frames
    let mut frames = [0usize; DEPTH];
    frames.copy_from_slice(&raw[3..3 + DEPTH]);
    {
        let mut sites = st.sites.lock().unwrap();
        if sites.len() >= MAX_SITES && !sites.contains_key(&(class_idx, frames)) {
            st.dropped_sites.fetch_add(1, Ordering::Relaxed);
        } else {
            let e = sites.entry((class_idx, frames)).or_insert((0, 0));
            e.0 += weight;
            e.1 += 1;
        }
    }
    let th = std::thread::current();
    let name = th.name().unwrap_or("?");
    let mut t = st.threads.lock().unwrap();
    if let Some(e) = t.get_mut(name) {
        e.0 += weight;
        e.1 += 1;
    } else {
        t.insert(name.to_string(), (weight, 1));
    }
}

// ------------------------------------------------------------------ collector hooks

/// Held for the duration of one sweep: counts the dead and live objects per class and ages the survivors.
pub struct Sweep {
    st: &'static State,
    guard: std::sync::MutexGuard<'static, Census>,
    t0: Instant,
}

/// Starts the census of a collection. Returns None when profiling is off.
pub fn sweep_begin() -> Option<Sweep> {
    if !on() {
        return None;
    }
    let st = STATE.get()?;
    let mut guard = st.census.lock().unwrap();
    for v in guard.live_n.iter_mut() { *v = [0; 4]; }
    for v in guard.live_b.iter_mut() { *v = [0; 4]; }
    Some(Sweep { st, guard, t0: Instant::now() })
}

impl Sweep {
    /// A surviving object of `size` bytes: counted at its age, then one collection older (capped at 3).
    #[inline]
    pub unsafe fn live(&mut self, o: *mut Obj, size: usize) {
        let age = (((*o).gc & AGE_MASK) >> AGE_SHIFT) as usize;
        if let Some(i) = self.st.table.get((*o).class) {
            self.guard.live_n[i][age] += 1;
            self.guard.live_b[i][age] += size as u64;
        }
        if age < 3 {
            (*o).gc += 1 << AGE_SHIFT;
        }
    }
    /// An object that is being freed: counted at the age it reached. Call before the slot is reused.
    #[inline]
    pub unsafe fn dead(&mut self, o: *mut Obj, size: usize) {
        let age = (((*o).gc & AGE_MASK) >> AGE_SHIFT) as usize;
        if let Some(i) = self.st.table.get((*o).class) {
            self.guard.dead_n[i][age] += 1;
            self.guard.dead_b[i][age] += size as u64;
        }
    }
    pub fn finish(mut self, gc_no: u64, live: usize, heap: usize, since: usize, stw_start: Instant) {
        let t_ms = self.st.start.elapsed().as_millis() as u64;
        let pause_us = stw_start.elapsed().as_micros() as u64;
        let _ = self.t0;
        self.guard.gcs.push(GcRec { n: gc_no, t_ms, live, heap, since, pause_us });
    }
}

// ------------------------------------------------------------------ output

fn esc(s: &str, out: &mut String) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 32 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn arr(a: &[u64; 4]) -> String {
    format!("[{},{},{},{}]", a[0], a[1], a[2], a[3])
}

fn write_class_list() {
    let Some(st) = STATE.get() else { return };
    let mut s = String::new();
    for c in rt::hooks().all_classes {
        s.push_str(c.name);
        s.push('\n');
    }
    let _ = std::fs::write(format!("{}.classes", st.path), s);
}

/// Executable mappings of this process (start, end, file offset, path): the report maps sample addresses to symbols.
fn maps_json() -> String {
    let mut out = String::from("[");
    if let Ok(txt) = std::fs::read_to_string("/proc/self/maps") {
        let mut first = true;
        for line in txt.lines() {
            let mut it = line.split_whitespace();
            let (Some(range), Some(perms), Some(off)) = (it.next(), it.next(), it.next()) else { continue };
            let path = it.nth(2).unwrap_or("");
            if !perms.contains('x') || !path.starts_with('/') {
                continue;
            }
            let Some((a, b)) = range.split_once('-') else { continue };
            if !first {
                out.push(',');
            }
            first = false;
            out.push_str(&format!("[\"{a}\",\"{b}\",\"{off}\","));
            esc(path, &mut out);
            out.push(']');
        }
    }
    out.push(']');
    out
}

pub fn dump() {
    let Some(st) = STATE.get() else { return };
    let mut j = String::with_capacity(1 << 20);
    let elapsed = st.start.elapsed().as_millis();
    j.push_str(&format!("{{\"version\":1,\"interval\":{},\"elapsed_ms\":{},\"maps\":{},", INTERVAL.load(Ordering::Relaxed), elapsed, maps_json()));
    // collections
    {
        let cen = st.census.lock().unwrap();
        j.push_str("\"gcs\":[");
        for (i, g) in cen.gcs.iter().enumerate() {
            if i > 0 { j.push(','); }
            j.push_str(&format!("{{\"n\":{},\"t_ms\":{},\"live\":{},\"heap\":{},\"since\":{},\"pause_us\":{}}}", g.n, g.t_ms, g.live, g.heap, g.since, g.pause_us));
        }
        j.push_str("],\"classes\":[");
        let mut first = true;
        let (mut tn, mut tb, mut an, mut ab) = (0u64, 0u64, 0u64, 0u64);
        for (i, c) in st.counters.iter().enumerate() {
            let n = c.n.load(Ordering::Relaxed);
            if n == 0 { continue; }
            let b = c.b.load(Ordering::Relaxed);
            let (arn, arb) = (c.arena_n.load(Ordering::Relaxed), c.arena_b.load(Ordering::Relaxed));
            tn += n; tb += b; an += arn; ab += arb;
            if !first { j.push(','); }
            first = false;
            j.push_str("{\"name\":");
            esc(st.table.classes[i].name, &mut j);
            j.push_str(&format!(",\"an\":{},\"ab\":{},\"arn\":{},\"arb\":{},\"dn\":{},\"db\":{},\"ln\":{},\"lb\":{}}}", n, b, arn, arb,
                arr(&cen.dead_n[i]), arr(&cen.dead_b[i]), arr(&cen.live_n[i]), arr(&cen.live_b[i])));
        }
        j.push_str(&format!("],\"total\":{{\"n\":{tn},\"b\":{tb},\"arena_n\":{an},\"arena_b\":{ab}}},"));
    }
    // sites: the heaviest by sampled bytes
    {
        let sites = st.sites.lock().unwrap();
        let mut v: Vec<(&(u32, [usize; DEPTH]), &(u64, u64))> = sites.iter().collect();
        v.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
        j.push_str(&format!("\"dropped_sites\":{},\"sites\":[", st.dropped_sites.load(Ordering::Relaxed)));
        for (i, (k, w)) in v.iter().take(8000).enumerate() {
            if i > 0 { j.push(','); }
            j.push_str("{\"c\":");
            match st.table.classes.get(k.0 as usize) {
                Some(c) => esc(c.name, &mut j),
                None => j.push_str("\"?\""),
            }
            j.push_str(&format!(",\"b\":{},\"s\":{},\"f\":[", w.0, w.1));
            let n = k.1.iter().position(|&a| a == 0).unwrap_or(DEPTH);
            for (x, a) in k.1[..n].iter().enumerate() {
                if x > 0 { j.push(','); }
                j.push_str(&format!("\"{a:x}\""));
            }
            j.push_str("]}");
        }
        j.push_str("],");
    }
    {
        let t = st.threads.lock().unwrap();
        j.push_str("\"threads\":[");
        for (i, (name, (b, s))) in t.iter().enumerate() {
            if i > 0 { j.push(','); }
            j.push_str("{\"name\":");
            esc(name, &mut j);
            j.push_str(&format!(",\"b\":{b},\"s\":{s}}}"));
        }
        j.push_str("]}");
    }
    let tmp = format!("{}.tmp", st.path);
    if std::fs::write(&tmp, j).is_ok() {
        let _ = std::fs::rename(&tmp, &st.path);
        if std::env::var_os("MONO_ALLOC_SNAPSHOTS").is_some() {
            let _ = std::fs::copy(&st.path, format!("{}.{}", st.path, elapsed / 1000));
        }
    }
}
