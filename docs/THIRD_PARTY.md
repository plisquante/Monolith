# Monolith içindeki üçüncü taraf kodlar

İlke (docs/MASTER_PLAN_TR.md §7): bir kütüphane, olgun olduğunda ve kendi yazacağımızdan iyi olduğunda kullanılır. Her girdi sabitlenir, depoya gömülür (vendored) ve burada listelenir.

| Crate / kütüphane | Sürüm | Lisans | Konum | Kullanan | Neden |
|---|---|---|---|---|---|
| crc32fast | 1.4.2 | MIT OR Apache-2.0 | rt_deps/vendor/crc32fast | monolith/rt/natives_io.rs (java.util.zip.CRC32) | CRC32 için zlib çağrısını kaldırır; saf Rust, mümkün olan yerde SIMD |
| cfg-if | 1.0.x (geçişli) | MIT OR Apache-2.0 | rt_deps/vendor/cfg-if | crc32fast | geçişli bağımlılık |

Hâlâ sistem kütüphaneleri (değiştirilecek ya da gömülecek, bkz. docs/IMPL_SPEC.md): zlib (Inflater/Deflater, adler32), OpenAL, shaderc/SPIRV-Cross, libffi (JNA), X11/GLX, libc.

Bağımlılık setinin derlenmesi `monolith/rustgen.py` içinde (`rt_deps_flags`) otomatik olarak, çevrimdışı ve rt_deps/ üzerinden yapılır. Başarısız bir derleme üretimi durdurur; hiçbir şey sessizce geri düşmez.
Güncelleme: rt_deps/Cargo.toml dosyasını düzenleyin, Cargo.lock'u yenileyin, rt_deps/vendor içine yeniden gömün, farkı commit edin ve lisansları gözden geçirin.
