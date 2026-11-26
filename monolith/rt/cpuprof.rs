//! İşlemci (CPU) örnekleme profilcisi. Ortam değişkeni: `MONO_CPU_PROFILE=<dosya>`.
//!
//! Amaç: zamanın nerede harcandığını görmek. Tahsis profilcisi (`prof.rs`) yalnızca ne kadar çöp üretildiğini söyler; hangi
//! kodun işlemciyi yediğini söylemez. Süreç CPU süresine bağlı bir zamanlayıcı (ITIMER_PROF, SIGPROF) saniyede `MONO_CPU_HZ`
//! kez (varsayılan 100) çalışan iş parçacığını yakalar ve o anki komut adresini kaydeder: bir iş parçacığı ne kadar çok CPU
//! yerse o kadar çok örnek düşer. Sinyal işleyici yalnızca bir halka tampona atomik yazar (tahsis, kilit, sembol çözme yok).
//! Dosya (JSON) `MONO_CPU_DUMP_SECS` saniyede bir (varsayılan 10) ve çıkışta yazılır; öldürülen bir süreç bile veri bırakır.
//! `tools/cpu_report.py` adresleri sembollere (Java metot adlarına) çevirip özetler.
//!
//! Değişken ayarlı değilse hiçbir maliyeti yoktur (zamanlayıcı ve işleyici kurulmaz).
//!
//! * `MONO_CPU_PROFILE=<dosya>`   etkinleştirir (dosya yanında `<dosya>.classes` sınıf listesi de yazılır)
//! * `MONO_CPU_HZ=<n>`            örnekleme sıklığı, 20..1000 (varsayılan 100)
//! * `MONO_CPU_DUMP_SECS=<n>`     döküm periyodu (varsayılan 10)
//!
//! İş parçacığı türleri (örnekle birlikte kaydedilir): 0 ana iş parçacığı ve Java dışı yerel iş parçacıkları (çizim iş
//! parçacığı burada), 2 "Server thread", 3 "Worker-" ile başlayanlar, 4 diğer Java iş parçacıkları.
use crate::rt::{self, Ref};

/// Etkin mi? Başka modüller (iş parçacığı başlatma) yalnızca açıkken ad okumak için sorar.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn on() -> bool {
    imp::ON.load(std::sync::atomic::Ordering::Relaxed)
}
#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn on() -> bool {
    false
}

/// Yeni başlayan Java iş parçacığının içinden çağrılır: örneklerin hangi türe yazılacağını belirler.
pub fn tag_thread(t: Ref) {
    if !on() {
        return;
    }
    let name = rt::str_to_rust(rt::get_ref(t, "name"));
    let kind = if name == "Server thread" {
        2
    } else if name.starts_with("Worker-") {
        3
    } else {
        4
    };
    set_kind(kind);
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn set_kind(k: u8) {
    imp::KIND.with(|c| c.set(k));
}
#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn set_kind(_k: u8) {}

/// Çalışma zamanı başlarken bir kez çağrılır (`rt::init_hooks`).
pub fn init() {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    imp::init();
}

#[cfg(any(target_os = "linux", target_os = "android"))]
mod imp {
    use crate::rt;
    use std::cell::Cell;
    use std::collections::HashMap;
    use std::os::raw::{c_int, c_void};
    use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
    use std::sync::OnceLock;
    use std::time::Instant;

    /// En çok örnek sayısı (8 MiB). 100 Hz'de yaklaşık 3 saat; dolunca yeni örnekler atılır.
    const CAP: usize = 1 << 20;
    #[allow(clippy::declare_interior_mutable_const)]
    const ZERO: AtomicU64 = AtomicU64::new(0);
    static BUF: [AtomicU64; CAP] = [ZERO; CAP];
    static N: AtomicUsize = AtomicUsize::new(0);
    pub static ON: AtomicBool = AtomicBool::new(false);
    static PATH: OnceLock<String> = OnceLock::new();
    static START: OnceLock<Instant> = OnceLock::new();
    static HZ: AtomicU64 = AtomicU64::new(100);

    thread_local! {
        /// Bu iş parçacığının türü (const başlatma: sinyal işleyicide güvenle okunur).
        pub static KIND: Cell<u8> = const { Cell::new(0) };
    }

    // `ucontext_t` içinde kesilen komutun adresi (PC). Yerleşim aarch64'te glibc ve bionic için aynı (NDK başlıklarına
    // karşı doğrulandı: uc_mcontext 176. bayt, pc +264 = 440); x86_64'te uc_mcontext.gregs[REG_RIP] = 168.
    #[cfg(target_arch = "aarch64")]
    const PC_OFF: usize = 440;
    #[cfg(target_arch = "x86_64")]
    const PC_OFF: usize = 168;
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    const PC_OFF: usize = 0;

    /// bionic `struct sigaction` (LP64): bayraklar önce, sonra işleyici, sonra 8 baytlık maske, sonra restorer (32 bayt).
    #[cfg(target_os = "android")]
    #[repr(C)]
    struct SigAction {
        sa_flags: c_int,
        sa_handler: usize,
        sa_mask: u64,
        sa_restorer: usize,
    }
    /// glibc `struct sigaction` (Linux x86_64 ve aarch64): işleyici, 128 baytlık maske, bayraklar, restorer.
    #[cfg(not(target_os = "android"))]
    #[repr(C)]
    struct SigAction {
        sa_handler: usize,
        sa_mask: [u64; 16],
        sa_flags: c_int,
        sa_restorer: usize,
    }
    #[repr(C)]
    struct Timeval {
        tv_sec: i64,
        tv_usec: i64,
    }
    #[repr(C)]
    struct ITimerVal {
        it_interval: Timeval,
        it_value: Timeval,
    }
    extern "C" {
        fn sigaction(sig: c_int, act: *const u8, oldact: *mut u8) -> c_int;
        fn setitimer(which: c_int, new_value: *const ITimerVal, old_value: *mut ITimerVal) -> c_int;
        fn atexit(cb: extern "C" fn()) -> c_int;
    }
    const SIGPROF: c_int = 27;
    const ITIMER_PROF: c_int = 2;
    const SA_SIGINFO_RESTART: c_int = 0x1000_0004; // SA_RESTART | SA_SIGINFO

    /// SIGPROF işleyici: yalnızca atomik yazma (async-signal-safe).
    extern "C" fn on_sample(_sig: c_int, _info: *mut c_void, uc: *mut c_void) {
        let i = N.fetch_add(1, Ordering::Relaxed);
        if i >= CAP {
            N.store(CAP, Ordering::Relaxed);
            return;
        }
        let pc = unsafe { *((uc as *const u8).add(PC_OFF) as *const u64) };
        let kind = KIND.try_with(|c| c.get()).unwrap_or(0);
        BUF[i].store((pc & 0x00FF_FFFF_FFFF_FFFF) | ((kind as u64) << 56), Ordering::Relaxed);
    }

    pub fn init() {
        let Some(path) = std::env::var_os("MONO_CPU_PROFILE") else { return };
        let path = path.to_string_lossy().into_owned();
        if path.is_empty() || PATH.set(path).is_err() {
            return;
        }
        let hz = std::env::var("MONO_CPU_HZ").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(100).clamp(20, 1000);
        HZ.store(hz, Ordering::Relaxed);
        let _ = START.set(Instant::now());
        write_class_list();
        let sa = SigAction { sa_handler: on_sample as *const () as usize, sa_mask: Default::default(), sa_flags: SA_SIGINFO_RESTART, sa_restorer: 0 };
        unsafe {
            if sigaction(SIGPROF, (&sa as *const SigAction).cast(), std::ptr::null_mut()) != 0 {
                eprintln!("[mono:cpuprof] sigaction failed, profile off");
                return;
            }
            let us = (1_000_000 / hz) as i64;
            let it = ITimerVal { it_interval: Timeval { tv_sec: 0, tv_usec: us }, it_value: Timeval { tv_sec: 0, tv_usec: us } };
            if setitimer(ITIMER_PROF, &it, std::ptr::null_mut()) != 0 {
                eprintln!("[mono:cpuprof] setitimer failed, profile off");
                return;
            }
            atexit(at_exit);
        }
        ON.store(true, Ordering::Release);
        let secs = std::env::var("MONO_CPU_DUMP_SECS").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(10).max(1);
        mark();
        let _ = std::thread::Builder::new().name("mono-cpuprof".into()).spawn(move || {
            let mut t = 0u64;
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                mark();
                t += 1;
                if t % secs == 0 {
                    dump();
                }
            }
        });
        eprintln!("[mono:cpuprof] cpu profile -> {} ({} Hz)", PATH.get().unwrap(), hz);
    }

    extern "C" fn at_exit() {
        mark();
        dump();
    }

    /// Zaman işaretleri: saniyede bir (unix ms, o ana kadarki örnek sayısı). Örnekler tampona sırayla yazıldığı için iki işaret
    /// arasındaki örnekler o saniyeye aittir; rapor aracı böylece yalnızca yavaş karelerin olduğu anları (başlatıcının
    /// frametime.csv'si, aynı saat) ayrı çözümleyebilir.
    static MARKS: std::sync::Mutex<Vec<(u128, usize)>> = std::sync::Mutex::new(Vec::new());

    fn mark() {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
        let n = N.load(Ordering::Relaxed).min(CAP);
        if let Ok(mut m) = MARKS.lock() {
            m.push((t, n));
        }
    }

    fn write_class_list() {
        let Some(path) = PATH.get() else { return };
        let mut s = String::new();
        for c in rt::hooks().all_classes {
            s.push_str(c.name);
            s.push('\n');
        }
        let _ = std::fs::write(format!("{path}.classes"), s);
    }

    /// (tür, adres) -> örnek sayısı olarak JSON yazar. Önce geçici dosyaya yazılıp yeniden adlandırılır (yarım dosya kalmasın).
    fn dump() {
        let Some(path) = PATH.get() else { return };
        let n = N.load(Ordering::Relaxed).min(CAP);
        let mut hist: HashMap<u64, u32> = HashMap::new();
        for slot in BUF.iter().take(n) {
            *hist.entry(slot.load(Ordering::Relaxed)).or_insert(0) += 1;
        }
        let elapsed = START.get().map_or(0, |s| s.elapsed().as_millis());
        let mut rows: Vec<(u64, u32)> = hist.into_iter().collect();
        rows.sort_unstable_by(|a, b| b.1.cmp(&a.1));
        let mut j = String::with_capacity(rows.len() * 28 + 4096);
        j.push_str(&format!(
            "{{\"version\":2,\"hz\":{},\"elapsed_ms\":{},\"samples\":{},\"maps\":{},\"hist\":[",
            HZ.load(Ordering::Relaxed),
            elapsed,
            n,
            crate::prof::maps_json()
        ));
        for (i, (k, c)) in rows.iter().enumerate() {
            if i > 0 {
                j.push(',');
            }
            j.push_str(&format!("[{},{},{}]", k >> 56, k & 0x00FF_FFFF_FFFF_FFFF, c));
        }
        // saniyelik pencereler: [başlangıç unix ms, tür, adres, sayı] (sürüm 2)
        j.push_str("],\"marks\":[");
        let marks = MARKS.lock().map(|m| m.clone()).unwrap_or_default();
        for (i, (t, m)) in marks.iter().enumerate() {
            if i > 0 {
                j.push(',');
            }
            j.push_str(&format!("[{t},{m}]"));
        }
        j.push_str("],\"win\":[");
        let mut first = true;
        for w in marks.windows(2) {
            let (t0, a) = w[0];
            let b = w[1].1.min(n);
            if a >= b {
                continue;
            }
            let mut h: HashMap<u64, u32> = HashMap::new();
            for slot in BUF.iter().take(b).skip(a) {
                *h.entry(slot.load(Ordering::Relaxed)).or_insert(0) += 1;
            }
            for (k, c) in h {
                if !first {
                    j.push(',');
                }
                first = false;
                j.push_str(&format!("[{},{},{},{}]", t0, k >> 56, k & 0x00FF_FFFF_FFFF_FFFF, c));
            }
        }
        j.push_str("]}");
        let tmp = format!("{path}.tmp");
        if std::fs::write(&tmp, j).is_ok() {
            let _ = std::fs::rename(&tmp, path);
        }
    }
}
