//! Monitors (synchronized, wait/notify), threads, sleep, interrupt, park/unpark.
use crate::rt::{self, Obj, Ref, Res};
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

static NEXT_TID: AtomicU64 = AtomicU64::new(1);
thread_local! {
    static TID: u64 = NEXT_TID.fetch_add(1, Ordering::Relaxed);
    static CUR: Cell<Ref> = const { Cell::new(Ref::NULL) };
}
pub fn tid() -> u64 {
    TID.with(|t| *t)
}
pub fn current_thread() -> Ref {
    CUR.with(|c| c.get())
}
pub fn set_current(r: Ref) {
    CUR.with(|c| c.set(r))
}

/// MONO_TRACE_PARK=1 prints a backtrace at every blocking monitor enter, wait and park. Read once: these are hot paths.
fn trace_park() -> bool {
    static V: OnceLock<bool> = OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MONO_TRACE_PARK").is_some())
}

// ------------------------------------------------------------------ monitors
struct MonState {
    owner: u64,
    count: u32,
}
pub struct Mon {
    st: Mutex<MonState>,
    enter_cv: Condvar,
    wait_cv: Condvar,
}
const SHARDS: usize = 64;
type ShardTable = Vec<Mutex<HashMap<usize, Arc<Mon>>>>;
fn table() -> &'static ShardTable {
    static T: OnceLock<ShardTable> = OnceLock::new();
    T.get_or_init(|| (0..SHARDS).map(|_| Mutex::new(HashMap::new())).collect())
}
fn mon(o: *mut Obj) -> Arc<Mon> {
    let a = o as usize;
    let mut t = table()[(a >> 4) % SHARDS].lock().unwrap();
    t.entry(a)
        .or_insert_with(|| Arc::new(Mon { st: Mutex::new(MonState { owner: 0, count: 0 }), enter_cv: Condvar::new(), wait_cv: Condvar::new() }))
        .clone()
}
pub fn enter(r: Ref) -> Res<()> {
    let o = rt::nn(r)?;
    let m = mon(o);
    let me = tid();
    if let Ok(mut st) = m.st.try_lock() {
        if st.owner == 0 {
            st.owner = me;
            st.count = 1;
            return Ok(());
        }
        if st.owner == me {
            st.count += 1;
            return Ok(());
        }
    }
    let _blk = crate::heap::ScopedBlock::enter();
    let mut st = m.st.lock().unwrap();
    loop {
        if st.owner == 0 {
            st.owner = me;
            st.count = 1;
            return Ok(());
        }
        if st.owner == me {
            st.count += 1;
            return Ok(());
        }
        if trace_park() {
            eprintln!("[mono:monitor_enter] owner={} me={}\n{}", st.owner, me, std::backtrace::Backtrace::force_capture());
        }
        st = m.enter_cv.wait(st).unwrap();
    }
}
pub fn exit(r: Ref) -> Res<()> {
    let o = rt::nn(r)?;
    let m = mon(o);
    let mut st = m.st.lock().unwrap();
    if st.owner != tid() {
        return rt::throw("java/lang/IllegalMonitorStateException", String::new());
    }
    st.count -= 1;
    if st.count == 0 {
        st.owner = 0;
        drop(st);
        m.enter_cv.notify_one();
    }
    Ok(())
}
/// Called by the collector between marking and sweeping: drops the monitors of dead objects. Without it every object that was
/// ever locked (synchronized, wait, notify) would keep a table entry for the life of the process, and a later object at the
/// same address would find it. A monitor that is owned or still referenced by a thread (blocked in `enter`, waiting) stays;
/// such an object is reachable anyway. `is_dead(address)` must be true only for heap objects that were not marked.
pub fn prune_dead_monitors(is_dead: impl Fn(usize) -> bool) -> usize {
    let mut dropped = 0;
    for shard in table() {
        let mut t = shard.lock().unwrap();
        let before = t.len();
        t.retain(|&a, m| {
            if !is_dead(a) || Arc::strong_count(m) > 1 {
                return true;
            }
            let st = m.st.lock().unwrap();
            st.owner != 0 || st.count != 0
        });
        dropped += before - t.len();
    }
    dropped
}

/// The monitor of `o` if one exists. Unlike `mon` it creates nothing: an object that was never locked has no owner.
fn peek(o: *mut Obj) -> Option<Arc<Mon>> {
    let a = o as usize;
    table()[(a >> 4) % SHARDS].lock().unwrap().get(&a).cloned()
}
pub fn holds_lock(r: Ref) -> Res<bool> {
    let o = rt::nn(r)?;
    Ok(match peek(o) {
        Some(m) => m.st.lock().unwrap().owner == tid(),
        None => false,
    })
}
pub fn notify(r: Ref, all: bool) -> Res<()> {
    let o = rt::nn(r)?;
    let Some(m) = peek(o) else {
        return rt::throw("java/lang/IllegalMonitorStateException", "current thread is not owner".to_string());
    };
    let st = m.st.lock().unwrap();
    if st.owner != tid() {
        return rt::throw("java/lang/IllegalMonitorStateException", "current thread is not owner".to_string());
    }
    if all { m.wait_cv.notify_all() } else { m.wait_cv.notify_one() }
    Ok(())
}
/// Object.wait: millis/nanos == 0 waits without a timeout.
pub fn wait(r: Ref, millis: i64, nanos: i32) -> Res<()> {
    let o = rt::nn(r)?;
    let m = mon(o);
    let me = tid();
    let ts = tstate(current_thread());
    if ts.interrupted.swap(false, Ordering::SeqCst) {
        return rt::throw("java/lang/InterruptedException", String::new());
    }
    if trace_park() {
        eprintln!("[mono:wait] millis={} nanos={}\n{}", millis, nanos, std::backtrace::Backtrace::force_capture());
    }
    let _blk = crate::heap::ScopedBlock::enter();
    let mut st = m.st.lock().unwrap();
    if st.owner != me {
        return rt::throw("java/lang/IllegalMonitorStateException", "current thread is not owner".to_string());
    }
    let saved = st.count;
    st.owner = 0;
    st.count = 0;
    m.enter_cv.notify_one();
    *ts.waiting.lock().unwrap() = Some(m.clone());
    // An interrupt that came after the check at the top but before `waiting` was set found nobody to wake. It stored the
    // flag first, so the flag is visible here: do not start waiting, take the monitor back and report the interrupt.
    if ts.interrupted.swap(false, Ordering::SeqCst) {
        *ts.waiting.lock().unwrap() = None;
        st.owner = me;
        st.count = saved;
        return rt::throw("java/lang/InterruptedException", String::new());
    }
    if millis == 0 && nanos == 0 {
        st = m.wait_cv.wait(st).unwrap();
    } else {
        let d = Duration::from_millis(millis.max(0) as u64) + Duration::from_nanos(nanos.max(0) as u64);
        st = m.wait_cv.wait_timeout(st, d).unwrap().0;
    }
    *ts.waiting.lock().unwrap() = None;
    while st.owner != 0 {
        st = m.enter_cv.wait(st).unwrap();
    }
    st.owner = me;
    st.count = saved;
    drop(st);
    if ts.interrupted.swap(false, Ordering::SeqCst) {
        return rt::throw("java/lang/InterruptedException", String::new());
    }
    Ok(())
}

// ------------------------------------------------------------------ per-thread state
pub struct TState {
    pub interrupted: AtomicBool,
    permit: Mutex<bool>,
    park_cv: Condvar,
    sleep_m: Mutex<()>,
    sleep_cv: Condvar,
    waiting: Mutex<Option<Arc<Mon>>>,
}
fn new_tstate() -> Arc<TState> {
    Arc::new(TState {
        interrupted: AtomicBool::new(false),
        permit: Mutex::new(false),
        park_cv: Condvar::new(),
        sleep_m: Mutex::new(()),
        sleep_cv: Condvar::new(),
        waiting: Mutex::new(None),
    })
}
/// Interrupt flag, park permit and wait state of each thread object, keyed by the object's address.
fn tstates() -> &'static Mutex<HashMap<usize, Arc<TState>>> {
    static T: OnceLock<Mutex<HashMap<usize, Arc<TState>>>> = OnceLock::new();
    T.get_or_init(|| Mutex::new(HashMap::new()))
}
fn tstate(t: Ref) -> Arc<TState> {
    tstates().lock().unwrap().entry(t.0 as usize).or_insert_with(new_tstate).clone()
}
/// A thread object that is about to start gets fresh state. Objects are not moved, and the address of a dead Thread can be
/// handed to a new one: without this a stale `interrupted` flag or park permit of the dead thread would fire on the new one.
/// (An interrupt before `start` has no effect, as in the JDK.)
fn reset_tstate(addr: usize) {
    tstates().lock().unwrap().insert(addr, new_tstate());
}
fn drop_tstate(addr: usize) {
    tstates().lock().unwrap().remove(&addr);
}
pub fn interrupt(t: Ref) -> Res<()> {
    let ts = tstate(rt::nn_ref(t)?);
    ts.interrupted.store(true, Ordering::SeqCst);
    {
        let _g = ts.permit.lock().unwrap();
        ts.park_cv.notify_all();
    }
    {
        let _g = ts.sleep_m.lock().unwrap();
        ts.sleep_cv.notify_all();
    }
    // The `waiting` guard must be gone before the monitor lock is taken: `wait` takes `st` first and `waiting` second, and
    // a guard kept alive by an `if let` scrutinee would make this the opposite order (deadlock when a timed wait ends
    // while the thread is interrupted).
    let waiting = ts.waiting.lock().unwrap().clone();
    if let Some(m) = waiting {
        let _g = m.st.lock().unwrap();
        m.wait_cv.notify_all();
    }
    Ok(())
}
pub fn is_interrupted(t: Ref, clear: bool) -> Res<bool> {
    let ts = tstate(rt::nn_ref(t)?);
    Ok(if clear { ts.interrupted.swap(false, Ordering::SeqCst) } else { ts.interrupted.load(Ordering::SeqCst) })
}
pub fn sleep(nanos: i64) -> Res<()> {
    let ts = tstate(current_thread());
    let end = Instant::now() + Duration::from_nanos(nanos.max(0) as u64);
    let _blk = crate::heap::ScopedBlock::enter();
    let mut g = ts.sleep_m.lock().unwrap();
    loop {
        if ts.interrupted.swap(false, Ordering::SeqCst) {
            return rt::throw("java/lang/InterruptedException", "sleep interrupted".to_string());
        }
        let now = Instant::now();
        if now >= end {
            return Ok(());
        }
        g = ts.sleep_cv.wait_timeout(g, end - now).unwrap().0;
    }
}
pub fn park(absolute: bool, time: i64) {
    let ts = tstate(current_thread());
    if trace_park() {
        eprintln!("[mono:park] abs={} time={}\n{}", absolute, time, std::backtrace::Backtrace::force_capture());
    }
    let _blk = crate::heap::ScopedBlock::enter();
    let mut p = ts.permit.lock().unwrap();
    if *p {
        *p = false;
        return;
    }
    if ts.interrupted.load(Ordering::SeqCst) {
        return;
    }
    if absolute {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
        if time <= now {
            return;
        }
        p = ts.park_cv.wait_timeout(p, Duration::from_millis((time - now) as u64)).unwrap().0;
    } else if time == 0 {
        p = ts.park_cv.wait(p).unwrap();
    } else if time > 0 {
        p = ts.park_cv.wait_timeout(p, Duration::from_nanos(time as u64)).unwrap().0;
    }
    *p = false;
}
pub fn unpark(t: Ref) {
    if t.is_null() {
        return;
    }
    let ts = tstate(t);
    let mut p = ts.permit.lock().unwrap();
    *p = true;
    ts.park_cv.notify_all();
}

// ------------------------------------------------------------------ threads
pub fn start(t: Ref, stack: i64) -> Res<()> {
    let addr = rt::nn_ref(t)?.0 as usize;
    let ss = if stack > 0 { (stack as usize).max(1 << 20) } else { 64 << 20 };
    reset_tstate(addr);
    let r = std::thread::Builder::new().stack_size(ss).spawn(move || {
        crate::crash::init_thread();
        let _gc_guard = crate::heap::ThreadGuard::enter();
        let t = Ref(addr as *mut Obj);
        set_current(t);
        crate::heap::set_thread_obj(t);
        let _ = (rt::hooks().thread_run)(t);
        drop_tstate(addr);
    });
    match r {
        Ok(_) => Ok(()),
        Err(e) => rt::throw("java/lang/OutOfMemoryError", format!("unable to create native thread: {}", e)),
    }
}
