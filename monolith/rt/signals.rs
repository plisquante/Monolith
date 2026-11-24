//! SIGTERM, SIGINT and SIGHUP end a Java program through `Runtime.exit(128 + signal)`, so the shutdown hooks run first, as
//! they do on HotSpot. Minecraft saves the world and stops the integrated server in such a hook; without this a `kill`,
//! Ctrl-C in a terminal or a system shutdown stopped the process at once and the hooks never ran.
//!
//! The handler only writes the signal number into a pipe (async-signal-safe). The Java thread `Signal Dispatcher`
//! (`java.lang.MonoSignals`) blocks in `await0` on the read end and calls `Runtime.exit` when a number arrives.
//! Linux only for now: elsewhere `install0` reports 0 and the signals keep their default action.
use crate::rt::Res;

#[cfg(target_os = "linux")]
mod imp {
    use std::os::raw::{c_int, c_void};
    use std::sync::atomic::{AtomicI32, Ordering};

    static WFD: AtomicI32 = AtomicI32::new(-1);
    static RFD: AtomicI32 = AtomicI32::new(-1);

    /// glibc `struct sigaction` on Linux x86_64 and aarch64 (the layout crash.rs uses too).
    #[repr(C)]
    struct SigAction {
        sa_handler: usize,
        sa_mask: [u64; 16],
        sa_flags: c_int,
        sa_restorer: usize,
    }
    unsafe extern "C" {
        fn pipe(fds: *mut c_int) -> c_int;
        fn read(fd: c_int, buf: *mut c_void, count: usize) -> isize;
        fn write(fd: c_int, buf: *const c_void, count: usize) -> isize;
        fn fcntl(fd: c_int, cmd: c_int, ...) -> c_int;
        fn sigaction(sig: c_int, act: *const u8, oldact: *mut u8) -> c_int;
    }
    const F_SETFD: c_int = 2;
    const F_GETFL: c_int = 3;
    const F_SETFL: c_int = 4;
    const FD_CLOEXEC: c_int = 1;
    const O_NONBLOCK: c_int = 0o4000;
    const SA_RESTART: c_int = 0x1000_0000;
    const SIGNALS: [c_int; 3] = [1, 2, 15]; // HUP INT TERM

    extern "C" fn handler(sig: c_int) {
        let fd = WFD.load(Ordering::Relaxed);
        if fd >= 0 {
            let b = [sig as u8];
            // the write end is non-blocking: a full pipe (nobody reading) drops the byte instead of blocking in a handler
            unsafe {
                let _ = write(fd, b.as_ptr() as *const c_void, 1);
            }
        }
    }

    pub fn install() -> bool {
        if WFD.load(Ordering::Relaxed) >= 0 {
            return true;
        }
        let mut fds = [0 as c_int; 2];
        unsafe {
            if pipe(fds.as_mut_ptr()) != 0 {
                return false;
            }
            for fd in fds {
                fcntl(fd, F_SETFD, FD_CLOEXEC);
            }
            let fl = fcntl(fds[1], F_GETFL);
            fcntl(fds[1], F_SETFL, fl | O_NONBLOCK);
        }
        RFD.store(fds[0], Ordering::Relaxed);
        WFD.store(fds[1], Ordering::Relaxed);
        for sig in SIGNALS {
            let sa = SigAction { sa_handler: handler as *const () as usize, sa_mask: [0u64; 16], sa_flags: SA_RESTART, sa_restorer: 0 };
            unsafe {
                if sigaction(sig, (&sa as *const SigAction).cast(), std::ptr::null_mut()) != 0 {
                    return false;
                }
            }
        }
        true
    }

    /// Blocks until a signal number arrives; 0 when the pipe is gone.
    pub fn wait() -> i32 {
        let fd = RFD.load(Ordering::Relaxed);
        if fd < 0 {
            return 0;
        }
        let _blk = crate::heap::ScopedBlock::enter(); // a thread blocked here must not hold up a collection
        loop {
            let mut b = 0u8;
            let n = unsafe { read(fd, (&mut b as *mut u8).cast(), 1) };
            if n == 1 {
                return b as i32;
            }
            if n == 0 {
                return 0;
            }
            if std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
                return 0;
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod imp {
    pub fn install() -> bool {
        false
    }
    pub fn wait() -> i32 {
        0
    }
}

/// `MonoSignals.install0()`: 1 when SIGHUP, SIGINT and SIGTERM now go to the dispatcher thread, 0 when not supported.
pub fn n_java_lang_MonoSignals__install0() -> Res<i32> {
    Ok(imp::install() as i32)
}

/// `MonoSignals.await0()`: the number of the next signal (blocks), or 0 when signals are not delivered any more.
pub fn n_java_lang_MonoSignals__await0() -> Res<i32> {
    Ok(imp::wait())
}
