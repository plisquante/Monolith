//! Çökme raporlayıcı. Bir Rust panic'i (çalışma zamanı panic=abort ile derlenir, yani her panic süreci sonlandırır) ve Linux'ta
//! öldürücü bir sinyal (SIGSEGV, SIGBUS, SIGILL, SIGFPE, SIGABRT) tek bir rapor dosyası yazar: `mono-crash-<pid>.txt`.
//! Dosya `MONO_CRASH_DIR` içine yazılır (varsayılan: çalışma dizini) ve süreç çıkar. Rapor çökme türünü, iş parçacığını,
//! mesajı ya da hata adresini ve yerel bir geri izlemeyi adlandırır. Yalnızca bir süreçteki ilk çökme dosyayı yazar.
//!
//! Sinyal işleyici yalnızca sinyal-güvenli çağrılar kullanır (`open`, `write`, `backtrace_symbols_fd`, `close`, `_exit`)
//! ve işleyici çalışmadan önce hazırlanmış bir yol kullanır. Yığın taşması durumunda işleyicinin çalışabilmesi için
//! bir iş parçacığının alternatif sinyal yığınına (`sigaltstack`, iş parçacığı başınadır) ihtiyacı vardır. Rust std, başlattığı
//! her iş parçacığına bunu verir; `init_thread` yalnızca hiç olmayan bir iş parçacığına ekler (std ile başlamayan sürecin ana iş parçacığı, yabancı iş parçacıkları).
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

/// Raporu ilk yazan taraf (panic kancası ya da sinyal işleyici) tarafından ayarlanır.
static WRITTEN: AtomicBool = AtomicBool::new(false);
/// NUL ile biten rapor yolu; `install` sırasında sızdırılır ve sinyal işleyici okur. `install`'dan önce null.
static PATH: AtomicPtr<u8> = AtomicPtr::new(std::ptr::null_mut());

/// Çökme raporu dosyasının yolu; `install` çalıştıktan sonra.
pub fn report_path() -> Option<String> {
    let p = PATH.load(Ordering::Acquire);
    if p.is_null() {
        return None;
    }
    let mut n = 0usize;
    unsafe {
        while *p.add(n) != 0 {
            n += 1;
        }
        Some(String::from_utf8_lossy(std::slice::from_raw_parts(p, n)).into_owned())
    }
}

/// Panic kancasını ve Linux'ta çağıran (ana) iş parçacığı için öldürücü sinyal işleyicilerini kurar. Erken, bir kez çağrılır.
pub fn install() {
    if !PATH.load(Ordering::Acquire).is_null() {
        return;
    }
    let dir = std::env::var_os("MONO_CRASH_DIR").map(std::path::PathBuf::from).unwrap_or_else(|| std::path::PathBuf::from("."));
    let mut path = dir.join(format!("mono-crash-{}.txt", std::process::id())).to_string_lossy().into_owned().into_bytes();
    path.retain(|b| *b != 0);
    path.push(0);
    PATH.store(Box::leak(path.into_boxed_slice()).as_mut_ptr(), Ordering::Release);

    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if !WRITTEN.swap(true, Ordering::AcqRel) {
            let t = std::thread::current();
            let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            let text = format!(
                "Monolith crash report\nkind: panic\nunix time: {}\nos: {} {}\nthread: {}\n{}\nbacktrace:\n{}\n",
                secs,
                std::env::consts::OS,
                std::env::consts::ARCH,
                t.name().unwrap_or("<unnamed>"),
                info,
                std::backtrace::Backtrace::force_capture()
            );
            if let Some(p) = report_path() {
                let _ = std::fs::write(p, text);
            }
        }
        prev(info);
    }));

    #[cfg(target_os = "linux")]
    linux::install_handlers();
    init_thread();
}

/// Çağıran iş parçacığına alternatif bir sinyal yığını verir; böylece yığın taşması sinyal işleyicisini yine de çalıştırabilir.
/// Yığın, iş parçacığı bitince serbest bırakılır. Sinyallerin işlenmediği yerlerde işlem yapmaz.
pub fn init_thread() {
    #[cfg(target_os = "linux")]
    linux::init_thread();
}

#[cfg(target_os = "linux")]
mod linux {
    use super::{PATH, WRITTEN};
    use std::os::raw::{c_char, c_int, c_uint, c_void};
    use std::sync::atomic::Ordering;

    #[repr(C)]
    struct StackT {
        ss_sp: *mut c_void,
        ss_flags: c_int,
        ss_size: usize,
    }
    /// Linux x86_64 ve aarch64 üzerinde glibc `struct sigaction`.
    #[repr(C)]
    struct SigAction {
        sa_handler: usize,
        sa_mask: [u64; 16],
        sa_flags: c_int,
        sa_restorer: usize,
    }
    unsafe extern "C" {
        fn write(fd: c_int, buf: *const c_void, count: usize) -> isize;
        fn open(path: *const c_char, flags: c_int, mode: c_uint) -> c_int;
        fn close(fd: c_int) -> c_int;
        fn sigaltstack(ss: *const StackT, old_ss: *mut StackT) -> c_int;
        fn sigaction(sig: c_int, act: *const u8, oldact: *mut u8) -> c_int;
        fn backtrace(buffer: *mut *mut c_void, size: c_int) -> c_int;
        fn backtrace_symbols_fd(buffer: *const *mut c_void, size: c_int, fd: c_int);
        fn _exit(status: c_int) -> !;
    }
    const SS_DISABLE: c_int = 2;
    const SA_ONSTACK_SIGINFO: c_int = 0x0800_0004; // SA_ONSTACK | SA_SIGINFO
    const O_WRONLY_CREAT_TRUNC: c_int = 0x1 | 0x40 | 0x200;
    const ALT_STACK_BYTES: usize = 64 * 1024;
    const SIGNALS: [c_int; 5] = [11, 7, 4, 8, 6]; // SEGV BUS ILL FPE ABRT

    fn name(sig: c_int) -> &'static [u8] {
        match sig {
            11 => b"SIGSEGV",
            7 => b"SIGBUS",
            4 => b"SIGILL",
            8 => b"SIGFPE",
            6 => b"SIGABRT",
            _ => b"signal",
        }
    }

    fn put(fd: c_int, b: &[u8]) {
        unsafe {
            let _ = write(fd, b.as_ptr() as *const c_void, b.len());
        }
    }

    fn put_hex(fd: c_int, v: usize) {
        let hex = b"0123456789abcdef";
        let mut out = [0u8; 18];
        out[0] = b'0';
        out[1] = b'x';
        for i in 0..16 {
            out[2 + i] = hex[(v >> ((15 - i) * 4)) & 0xf];
        }
        put(fd, &out);
    }

    fn put_dec(fd: c_int, mut v: u32) {
        let mut buf = [0u8; 10];
        let mut i = buf.len();
        loop {
            i -= 1;
            buf[i] = b'0' + (v % 10) as u8;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        put(fd, &buf[i..]);
    }

    /// Rapor gövdesini `fd`'ye yazar: tür, hata adresi, yerel geri izleme. Sinyal-güvenlidir.
    fn body(fd: c_int, sig: c_int, addr: usize) {
        put(fd, b"Monolith crash report\nkind: signal ");
        put(fd, name(sig));
        put(fd, b" (");
        put_dec(fd, sig as u32);
        put(fd, b")\nfault address: ");
        put_hex(fd, addr);
        put(fd, b"\nbacktrace:\n");
        let mut buf = [std::ptr::null_mut::<c_void>(); 48];
        unsafe {
            let n = backtrace(buf.as_mut_ptr(), 48);
            backtrace_symbols_fd(buf.as_ptr(), n, fd);
        }
    }

    extern "C" fn handler(sig: c_int, info: *const usize, _ctx: *const u8) {
        // si_addr, 64 bit Linux'ta siginfo_t'nin üçüncü makine sözcüğüdür
        let addr = if info.is_null() { 0 } else { unsafe { *info.add(2) } };
        if !WRITTEN.swap(true, Ordering::AcqRel) {
            body(2, sig, addr);
            let p = PATH.load(Ordering::Acquire);
            if !p.is_null() {
                let fd = unsafe { open(p as *const c_char, O_WRONLY_CREAT_TRUNC, 0o644) };
                if fd >= 0 {
                    body(fd, sig, addr);
                    unsafe {
                        close(fd);
                    }
                }
            }
        }
        unsafe { _exit(128 + sig) }
    }

    pub fn install_handlers() {
        for sig in SIGNALS {
            let sa = SigAction { sa_handler: handler as *const () as usize, sa_mask: [0u64; 16], sa_flags: SA_ONSTACK_SIGINFO, sa_restorer: 0 };
            unsafe {
                let _ = sigaction(sig, (&sa as *const SigAction).cast(), std::ptr::null_mut());
            }
        }
    }

    /// Bu iş parçacığının alternatif yığınının sahibi; iş parçacığı bitince onu devre dışı bırakır ve serbest bırakır.
    struct AltStack(Option<Box<[u8]>>);
    impl Drop for AltStack {
        fn drop(&mut self) {
            if let Some(mem) = self.0.take() {
                let off = StackT { ss_sp: std::ptr::null_mut(), ss_flags: SS_DISABLE, ss_size: 0 };
                unsafe {
                    let _ = sigaltstack(&off, std::ptr::null_mut());
                }
                drop(mem); // freed only after the kernel no longer knows it
            }
        }
    }
    thread_local! {
        static ALT: std::cell::RefCell<AltStack> = const { std::cell::RefCell::new(AltStack(None)) };
    }

    pub fn init_thread() {
        let mut cur = StackT { ss_sp: std::ptr::null_mut(), ss_flags: 0, ss_size: 0 };
        if unsafe { sigaltstack(std::ptr::null(), &mut cur) } == 0 && cur.ss_flags & SS_DISABLE == 0 {
            return; // this thread already has one
        }
        let _ = ALT.try_with(|a| {
            let mut a = a.borrow_mut();
            if a.0.is_none() {
                let mut mem = vec![0u8; ALT_STACK_BYTES].into_boxed_slice();
                let ss = StackT { ss_sp: mem.as_mut_ptr() as *mut c_void, ss_flags: 0, ss_size: mem.len() };
                if unsafe { sigaltstack(&ss, std::ptr::null_mut()) } == 0 {
                    a.0 = Some(mem);
                }
            }
        });
    }
}
