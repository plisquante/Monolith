//! Counts the calls of the JNI/JNA emulation natives (`natives_jni.rs`: netty epoll and kqueue, JNA, Tracy, the narrator).
//! The goal of docs/MASTER_PLAN_TR.md WS2 is zero such calls in 30 minutes of play; this is the meter for it.
//!
//! `MONO_COUNT_NATIVES=1` prints the table to stderr when the process exits, `MONO_COUNT_NATIVES=<file>` writes it there.
//! Off, a call costs one relaxed load. The table is written by an `atexit` handler: a process that ends by `_exit`, a
//! crash or a kill does not print it.
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

static ON: AtomicBool = AtomicBool::new(false);
static COUNTS: Mutex<BTreeMap<&'static str, u64>> = Mutex::new(BTreeMap::new());
static TARGET: Mutex<Option<String>> = Mutex::new(None);

/// Called by the generated stub of every emulation native, with its Rust name.
#[inline(always)]
pub fn count(name: &'static str) {
    if ON.load(Ordering::Relaxed) {
        count_slow(name);
    }
}

#[cold]
#[inline(never)]
fn count_slow(name: &'static str) {
    let mut c = COUNTS.lock().unwrap_or_else(|e| e.into_inner());
    *c.entry(name).or_insert(0) += 1;
}

/// The table: total, then one line per native, most called first.
pub fn render() -> String {
    let c = COUNTS.lock().unwrap_or_else(|e| e.into_inner());
    let total: u64 = c.values().sum();
    let mut rows: Vec<(&&str, &u64)> = c.iter().collect();
    rows.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    let mut s = format!("native calls through the JNI/JNA emulation: {} calls, {} distinct natives\n", total, c.len());
    for (name, n) in rows {
        s.push_str(&format!("{:>12}  {}\n", n, name));
    }
    s
}

/// Starts counting; the table goes to `target` (a file) or to stderr when `None`.
pub fn enable(target: Option<String>) {
    *TARGET.lock().unwrap_or_else(|e| e.into_inner()) = target;
    ON.store(true, Ordering::Relaxed);
}

fn write_report() {
    let text = render();
    let target = TARGET.lock().unwrap_or_else(|e| e.into_inner()).clone();
    match target {
        Some(path) => {
            let _ = std::fs::write(path, text);
        }
        None => eprint!("{}", text),
    }
}

#[cfg(unix)]
extern "C" fn at_exit() {
    write_report();
}

#[cfg(unix)]
unsafe extern "C" {
    fn atexit(cb: extern "C" fn()) -> std::os::raw::c_int;
}

/// Reads MONO_COUNT_NATIVES and, when it is set, starts counting and registers the table for process exit.
pub fn install() {
    let Some(v) = std::env::var_os("MONO_COUNT_NATIVES") else { return };
    let v = v.to_string_lossy().into_owned();
    if v.is_empty() || v == "0" {
        return;
    }
    enable(if v == "1" { None } else { Some(v) });
    #[cfg(unix)]
    unsafe {
        atexit(at_exit);
    }
    #[cfg(not(unix))]
    {
        let _ = write_report; // no atexit binding off unix: the table is not printed there
    }
}
