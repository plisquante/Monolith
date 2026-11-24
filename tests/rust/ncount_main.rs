// Birim testleri: JNI/JNA taklit sayacı (ncount.rs). Genel durum tek süreçte paylaşıldığı için tek test, sırayla.
// Çalıştırma: rustc --edition 2021 --test tests/rust/ncount_main.rs -o /tmp/ncount_tests && /tmp/ncount_tests
#[path = "../../monolith/rt/ncount.rs"]
pub mod ncount;

#[test]
fn counts_only_when_enabled_and_renders_most_called_first() {
    // off: nothing is recorded
    ncount::count("n_a__never");
    assert!(ncount::render().starts_with("native calls through the JNI/JNA emulation: 0 calls, 0 distinct natives\n"));

    ncount::enable(None);
    for _ in 0..3 {
        ncount::count("n_com_sun_jna_Native__read");
    }
    ncount::count("n_io_netty_channel_unix_Socket__newSocketStream");
    for _ in 0..7 {
        ncount::count("n_com_mojang_jtracy_TracyBindings__markFrame");
    }
    let t = ncount::render();
    let lines: Vec<&str> = t.lines().collect();
    assert_eq!(lines[0], "native calls through the JNI/JNA emulation: 11 calls, 3 distinct natives");
    assert!(lines[1].trim_start().starts_with("7  n_com_mojang_jtracy_TracyBindings__markFrame"), "{}", t);
    assert!(lines[2].trim_start().starts_with("3  n_com_sun_jna_Native__read"), "{}", t);
    assert!(lines[3].trim_start().starts_with("1  n_io_netty_channel_unix_Socket__newSocketStream"), "{}", t);
    assert!(!t.contains("n_a__never"), "a call before enable must not be recorded");
}
