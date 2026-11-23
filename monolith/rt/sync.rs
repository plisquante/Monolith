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
fn table() -> &'static Vec<Mutex<HashMap<usize, Arc<Mon>>>> {
    static T: OnceLock<Vec<Mutex<HashMap<usize, Arc<Mon>>>>> = OnceLock::new();
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
    let _blk = crate::gc::ScopedBlock::enter();
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
        if std::env::var_os("MONO_TRACE_PARK").is_some() {
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
pub fn holds_lock(r: Ref) -> Res<bool> {
    let o = rt::nn(r)?;
    let m = mon(o);
    let st = m.st.lock().unwrap();
    Ok(st.owner == tid())
}
pub fn notify(r: Ref, all: bool) -> Res<()> {
    let o = rt::nn(r)?;
    let m = mon(o);
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
    if std::env::var_os("MONO_TRACE_PARK").is_some() {
        eprintln!("[mono:wait] millis={} nanos={}\n{}", millis, nanos, std::backtrace::Backtrace::force_capture());
    }
    let _blk = crate::gc::ScopedBlock::enter();
    let mut st = m.st.lock().unwrap();
    if st.owner != me {
        return rt::throw("java/lang/IllegalMonitorStateException", "current thread is not owner".to_string());
    }
    let saved = st.count;
    st.owner = 0;
    st.count = 0;
    m.enter_cv.notify_one();
    *ts.waiting.lock().unwrap() = Some(m.clone());
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
fn tstate(t: Ref) -> Arc<TState> {
    static T: OnceLock<Mutex<HashMap<usize, Arc<TState>>>> = OnceLock::new();
    let mut m = T.get_or_init(|| Mutex::new(HashMap::new())).lock().unwrap();
    m.entry(t.0 as usize)
        .or_insert_with(|| {
            Arc::new(TState {
                interrupted: AtomicBool::new(false),
                permit: Mutex::new(false),
                park_cv: Condvar::new(),
                sleep_m: Mutex::new(()),
                sleep_cv: Condvar::new(),
                waiting: Mutex::new(None),
            })
        })
        .clone()
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
    if let Some(m) = ts.waiting.lock().unwrap().clone() {
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
    let _blk = crate::gc::ScopedBlock::enter();
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
    if std::env::var_os("MONO_TRACE_PARK").is_some() {
        eprintln!("[mono:park] abs={} time={}\n{}", absolute, time, std::backtrace::Backtrace::force_capture());
    }
    let _blk = crate::gc::ScopedBlock::enter();
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
    let r = std::thread::Builder::new().stack_size(ss).spawn(move || {
        let _gc_guard = crate::gc::ThreadGuard::enter();
        let t = Ref(addr as *mut Obj);
        set_current(t);
        crate::gc::set_thread_obj(t);
        let _ = (rt::hooks().thread_run)(t);
    });
    match r {
        Ok(_) => Ok(()),
        Err(e) => rt::throw("java/lang/OutOfMemoryError", format!("unable to create native thread: {}", e)),
    }
}
