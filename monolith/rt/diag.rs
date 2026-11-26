//! MONO_DIAG=1: tek anahtarla cihaz tanısı (varsayılan KAPALI; kapalıyken hiçbir iş parçacığı başlamaz, hiçbir şey yazılmaz).
//!
//! Açıkken:
//!   * `[mono:mem]` (2 sn'de bir): süreç RSS ve en yüksek RSS (VmHWM), GC yığını ve son toplamadaki canlı veri, bölge parça
//!     sayısı, iş parçacığı sayısı, sistemin kullanılabilir belleği (MemAvailable; Android düşük bellek katili buna bakar).
//!   * `[mono:threads]` (2 sn'de bir): iş parçacığı başına CPU yüzdesi (/proc/self/task/*/stat, son aralıktaki fark), en çok
//!     yiyen 10 tanesi. Java iş parçacıklarına işletim sistemi adı verilir (Java adının ilk 15 karakteri) ki satırda okunsun.
//!   * GPU sayaçları ve GL hata denetimi (natives_rhi.rs: MONO_GL_STATS + MONO_GL_CHECK ikisi birden açılır).
//! Bütün satırlar unix ms damgalıdır (`t=`): başlatıcının frametime.csv'si, GC satırları ve CPU profili pencereleriyle aynı saat.

use crate::rt::{self, Ref};
use std::sync::OnceLock;

pub fn on() -> bool {
    static V: OnceLock<bool> = OnceLock::new();
    *V.get_or_init(|| std::env::var("MONO_DIAG").is_ok_and(|v| !v.is_empty() && v != "0"))
}

fn now_ms() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn set_os_name(name: &str) {
    extern "C" {
        fn prctl(option: i32, arg2: usize, arg3: usize, arg4: usize, arg5: usize) -> i32;
    }
    const PR_SET_NAME: i32 = 15;
    // çekirdek adı en çok 15 bayt + NUL; boşluk /proc/.../stat ayrıştırmasını bozmasın diye '_' olur
    let mut b: Vec<u8> = name.bytes().filter(|&c| c != 0).map(|c| if c == b' ' { b'_' } else { c }).take(15).collect();
    b.push(0);
    unsafe { prctl(PR_SET_NAME, b.as_ptr() as usize, 0, 0, 0) };
}
#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn set_os_name(_name: &str) {}

/// Yeni başlayan Java iş parçacığının içinden: anahtar açıksa işletim sistemi adını Java adı yapar.
pub fn name_thread(t: Ref) {
    if !on() {
        return;
    }
    set_os_name(&rt::str_to_rust(rt::get_ref(t, "name")));
}

/// Program başında bir kez (rt::init_hooks). Anahtar kapalıysa hiçbir şey yapmaz.
pub fn init() {
    if !on() {
        return;
    }
    set_os_name("mono-main");
    eprintln!("[mono:diag] on: mem + threads every 2 s, GL stats + GL error check");
    let _ = std::thread::Builder::new().name("mono-diag".into()).spawn(|| {
        let mut prev: std::collections::HashMap<u32, (String, u64)> = std::collections::HashMap::new();
        let mut prev_t = std::time::Instant::now();
        loop {
            std::thread::sleep(std::time::Duration::from_secs(2));
            mem_line();
            prev_t = threads_line(&mut prev, prev_t);
        }
    });
}

/// /proc dosyasındaki "Anahtar:   123 kB" satırının değeri (kB).
fn kb(text: &str, key: &str) -> u64 {
    text.lines()
        .find(|l| l.starts_with(key))
        .and_then(|l| l[key.len()..].split_whitespace().next())
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

fn mem_line() {
    let st = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let mi = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    eprintln!(
        "[mono:mem] t={} rss_mb={} hwm_mb={} heap_mb={} live_mb={} region_chunks={} threads={} sys_avail_mb={}",
        now_ms(),
        kb(&st, "VmRSS:") / 1024,
        kb(&st, "VmHWM:") / 1024,
        crate::heap::heap_bytes() >> 20,
        crate::heap::live_bytes() >> 20,
        crate::region::chunks_in_use(),
        kb(&st, "Threads:"),
        kb(&mi, "MemAvailable:") / 1024
    );
}

/// Her iş parçacığının toplam CPU saati (utime + stime, saat tıkı) ve adı.
fn task_times() -> Vec<(u32, String, u64)> {
    let mut out = Vec::new();
    let Ok(dir) = std::fs::read_dir("/proc/self/task") else { return out };
    for e in dir.flatten() {
        let Ok(tid) = e.file_name().to_string_lossy().parse::<u32>() else { continue };
        let Ok(s) = std::fs::read_to_string(e.path().join("stat")) else { continue };
        // biçim: "tid (ad) durum ..." ; ad parantez ve boşluk içerebilir, son ')' sonrası alanlar sayılır
        let (Some(a), Some(b)) = (s.find('('), s.rfind(')')) else { continue };
        let name = s[a + 1..b].to_string();
        let f: Vec<&str> = s[b + 1..].split_whitespace().collect();
        // ')' sonrası alan 0 = durum (stat'ın 3. alanı); utime = 14. alan -> 11, stime = 15. alan -> 12
        if f.len() > 12 {
            let ut: u64 = f[11].parse().unwrap_or(0);
            let stt: u64 = f[12].parse().unwrap_or(0);
            out.push((tid, name, ut + stt));
        }
    }
    out
}

fn threads_line(prev: &mut std::collections::HashMap<u32, (String, u64)>, prev_t: std::time::Instant) -> std::time::Instant {
    let now = std::time::Instant::now();
    let dt = now.duration_since(prev_t).as_secs_f64().max(0.001);
    const HZ: f64 = 100.0; // Linux/Android USER_HZ (sysconf(_SC_CLK_TCK)) her iki platformda 100
    let cur = task_times();
    let mut rows: Vec<(f64, String)> = Vec::new();
    let mut next = std::collections::HashMap::new();
    for (tid, name, ticks) in cur {
        if let Some((_, p)) = prev.get(&tid) {
            let pct = (ticks.saturating_sub(*p)) as f64 / HZ / dt * 100.0;
            if pct >= 1.0 {
                rows.push((pct, format!("{name}#{tid}")));
            }
        }
        next.insert(tid, (name, ticks));
    }
    *prev = next;
    rows.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let total: f64 = rows.iter().map(|r| r.0).sum();
    let top: Vec<String> = rows.iter().take(10).map(|(p, n)| format!("{n}={p:.0}%")).collect();
    eprintln!("[mono:threads] t={} total={:.0}% {}", now_ms(), total, top.join(" "));
    now
}
