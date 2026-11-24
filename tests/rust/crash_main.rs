// Birim testleri: çökme raporlayıcı (crash.rs). Her test aynı test ikilisini çocuk süreç olarak çalıştırır; çocuk gerçekten
// panic eder ya da SIGSEGV alır, ebeveyn rapor dosyasını kontrol eder.
// Çalıştırma: rustc --edition 2021 --test tests/rust/crash_main.rs -o /tmp/crash_tests && /tmp/crash_tests
#[path = "../../monolith/rt/crash.rs"]
pub mod crash;

use std::path::PathBuf;
use std::process::Command;

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("mono-crash-test-{}-{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Runs the named child test in a separate process and returns (exit code, report text if one was written).
fn run_child(test: &str, tag: &str) -> (Option<i32>, Option<String>) {
    let dir = temp_dir(tag);
    let out = Command::new(std::env::current_exe().unwrap())
        .args([test, "--exact", "--nocapture"])
        .env("MONO_CRASH_CHILD", "1")
        .env("MONO_CRASH_DIR", &dir)
        .output()
        .unwrap();
    let report = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .find(|e| e.file_name().to_string_lossy().starts_with("mono-crash-"))
        .and_then(|e| std::fs::read_to_string(e.path()).ok());
    let _ = std::fs::remove_dir_all(&dir);
    (out.status.code(), report)
}

fn is_child() -> bool {
    std::env::var_os("MONO_CRASH_CHILD").is_some()
}

#[test]
fn child_panic() {
    if !is_child() {
        return;
    }
    crash::install();
    panic!("deliberate test panic 4711");
}

#[test]
fn child_segv() {
    if !is_child() {
        return;
    }
    crash::install();
    unsafe { std::ptr::null_mut::<u8>().write_volatile(1) };
}

#[test]
fn panic_writes_report() {
    if is_child() {
        return;
    }
    let (_, report) = run_child("child_panic", "panic");
    let r = report.expect("no crash report file after a panic");
    assert!(r.starts_with("Monolith crash report\nkind: panic\n"), "{}", r);
    assert!(r.contains("deliberate test panic 4711"), "{}", r);
    assert!(r.contains("thread: child_panic"), "{}", r);
    assert!(r.contains("backtrace:"), "{}", r);
}

#[cfg(target_os = "linux")]
#[test]
fn segv_writes_report_and_exits_with_signal_status() {
    if is_child() {
        return;
    }
    let (code, report) = run_child("child_segv", "segv");
    assert_eq!(code, Some(128 + 11), "exit status of the crashed child");
    let r = report.expect("no crash report file after SIGSEGV");
    assert!(r.starts_with("Monolith crash report\nkind: signal SIGSEGV (11)\n"), "{}", r);
    assert!(r.contains("fault address: 0x0000000000000000\n"), "{}", r);
    assert!(r.contains("backtrace:\n"), "{}", r);
}

#[test]
fn report_path_is_set_after_install() {
    if is_child() {
        return;
    }
    assert!(crash::report_path().is_none());
    crash::install();
    let p = crash::report_path().unwrap();
    assert!(p.contains("mono-crash-"), "{}", p);
    assert!(p.ends_with(".txt"), "{}", p);
}
