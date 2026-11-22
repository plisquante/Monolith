// Birim testleri: ses karıştırıcısı (audio.rs) ve aygıt katmanı (audio_dev.rs).
// Çalıştırma: rustc --edition 2021 --test tests/rust/audio_main.rs -o /tmp/audio_tests && /tmp/audio_tests
#[path = "../../monolith/rt/audio.rs"]
pub mod audio;
#[path = "../../monolith/rt/audio_dev.rs"]
pub mod audio_dev;
