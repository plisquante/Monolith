# Monolith: Mimari ve Risk Listesi

Hedef: Minecraft Java Edition **26.3** istemcisini (sunucu değil) Rust'a dönüştürmek ve davranışını vanilla ile birebir aynı tutmak. GC'siz çalışma zamanı. Hedef platformlar: PC ve Android (arm64, `.so`).

## Yaklaşım: çeviri + yama

Sıfırdan yeniden yazmak yerine:

1. **Decompile:** 26.3 client jar'ı (obfüskasyonsuz) Java kaynağına çevrilir (Vineflower 1.12.0, Maven Central, sha1 doğrulamalı).
2. **Çeviri betiği:** Java kaynağını Rust'a dönüştüren araç (`tools/transpile/`). Girdi: kullanıcının kendi jar'ı. Çıktı: `OUT_DIR` veya `generated/` (gitignore'da), depoya girmez.
3. **Yama katmanı:** Repodaki elle yazılmış Rust modülleri, çeviri çıktısındaki belirli sınıf/metot imzalarını değiştirir (`patches/`). GC'ye bağlı desenler, sıcak yollar ve platform kodu burada düzeltilir.
4. **Optimizasyon:** Her performans yaması vanilla çıktısıyla karşılaştırılarak kabul edilir.

Repo kuralı: Mojang kodu veya varlıkları commit edilmez. Depoda yalnızca betik, yamalar, test altyapısı ve `mono-*` yardımcı crate'leri olur.

## Doğrulananlar (26.3 istemci jar'ından, depoya girmedi)

- Sürüm manifestinde `26.3` release, `26.4-snapshot-10` en son snapshot. İstemci jar SHA-1 manifestle eşleşti.
- Jar obfüskasyonsuz: `net/minecraft/...` sınıf adları okunabilir (11266 sınıf). Eşleme (mapping) dosyası gerekmiyor.
- Overworld: `min_y = -64`, `height = 384`, `sea_level = 63`. Kaynak: `data/minecraft/dimension_type/overworld.json` ve `data/minecraft/worldgen/noise_settings/overworld.json`.
- Overworld `legacy_random_source = false`: worldgen modern RandomSource (Xoroshiro tabanlı) kullanıyor. `java.util.Random` yalnızca legacy yollarda gerekebilir.
- Sunucu manifesti Java 25 istiyor (`javaVersion.majorVersion = 25`). Bu makinede JDK 21 var.

## Crate yapısı

```
Monolith/
  Cargo.toml                 çalışma alanı, unsafe varsayılan olarak yasak
  crates/
    mono-core/               java.util.Random yeniden uygulaması, matematik, sabitler (unsafe yok)
    mono-world/              chunk düzeni, blok durum kimlikleri (unsafe yok)
    mono-verify/  (sonraki)  vanilla çıktısı ile karşılaştırma harness'i
    mono-render/  (sonraki)  wgpu katmanı, tek unsafe sınırı
    mono-platform/ (sonraki) winit, girdi, Android köprüsü (JNI / NativeActivity), ses
    mono-app/     (sonraki)  ikili (PC) ve cdylib (Android)
  tools/oracle/              vanilla çıktısı üreten Java araçları (depoya yalnızca bizim kodumuz)
```

Bağımlılık yönü tek yönlü: `platform, render -> world -> core`. `core` ve `world` hiçbir grafik/OS crate'ine bağlanmaz.

## Veri düzeni (derleme zamanı)

- Chunk: 16 x 384 x 16, blok başına `BlockStateId(u16)`. Boyut `const` ile hesaplanır, `const _: () = assert!(...)` ile derleme zamanında kilitlenir. Sabit boyutlu `Box<[T; N]>`, çalışma zamanında boyut kararı yok.
- Yerel indeks: `((y - MIN_Y) * 16 + z) * 16 + x`. Erişim `Option` döndürür, panik yok.
- Blok/varlık kayıt defterleri: 26.3 jar'ındaki kayıt verisinden `build.rs` ile üretilecek. Girdi olarak kullanıcının kendi jar'ı verilecek, çıktı `OUT_DIR`'e yazılacak, depoya girmeyecek.
- Shader'lar: WGSL, derleme zamanında doğrulanıp gömülecek (araç seçimi henüz yapılmadı, doğrulanmadı).

## Grafik, ses, girdi, Android

- Grafik: `wgpu` (crates.io'da `30.0.2` var, kontrol edildi). Bağımlılık henüz eklenmedi. `unsafe` yalnızca yüzey/buffer sınırında.
- Pencere: `winit` (en yeni kararlı değil, `0.31.0-beta.4` var). Beta sürüm seçilirse risk kabul edilerek belgelenecek.
- Ses: henüz seçilmedi. Android için oboe veya AAudio adayı, doğrulanmadı.
- Android: `aarch64-linux-android` hedefi, `cdylib` çıktısı `.so`. NDK kurulumu ve JNI/NativeActivity seçimi açık. Doğrulanmadı.

## Doğrulama stratejisi

- Her alt sistem için vanilla ile karşılaştırma testi: aynı tohum, aynı çıktı. Test yoksa alt sistem "bitmiş" sayılmaz.
- `java.util.Random` karşılaştırması: JDK 21 çıktısı (`tools/oracle/JavaRandomOracle.java`) sabit fixture olarak depoda. Bu, JDK davranışını test eder. Vanilla `LegacyRandomSource` ile eşdeğerlik ayrıca doğrulanmadı.
- Vanilla sunucu oracle'ı: Java 25 gerekiyor. Adoptium API erişilebilir (HTTP 200), JDK 25 kurulumu denenmedi.
- Referans dump'lar (vanilla çıktısı) depoya girmez, `MONOLITH_REF_DIR` ortam değişkeninden okunur. Yoksa ilgili testler `#[ignore]` ile görünür biçimde atlanır.

## Riskler

1. **Yasal (en yüksek):** Çevrilmiş kod Mojang'ın telifli kodunun türevidir. Kişisel yerel kullanım ile dağıtım (özellikle Android `.so` paketleri) farklı değerlendirilmeli. Hukuki görüş gerekli. Bu karar kullanıcıya ait.
1b. **Sınırlar:** Decompile edilmiş Java'nın tam anlamıyla derlenebilir olması garanti değil. Lambda, generic, iç sınıf, reflection ve Java `float` semantiği çeviride ayrı ele alınmalı.
2. **Sürüm hareketliliği:** 26.x snapshot'ları sık değişiyor. Hedef sürüm 26.3 olarak sabitlendi. Güncelleme bilinçli yapılacak.
3. **Matematik uyumu:** Java `StrictMath` (fdlibm) ile Rust `libm`/`f64::ln` son bit farkı verebilir. Noise ve `nextGaussian` bu yüzden kayabilir. Çözüm: fdlibm portu ve bit-düzeyinde test. Doğrulanmadı.
4. **Sıralama bağımlılığı:** Vanilla'nın hash tabanlı koleksiyon sıraları worldgen'i etkiliyorsa birebir eşleşme için sıra bilinmeli. Bytecode incelemesi gerekecek.
5. **Blok durum sayısı:** 26.3'te durum sayısı `u16` sınırına yaklaşıyor olabilir. Kayıt defteri üretildiğinde kontrol edilecek. Doğrulanmadı.
6. **Oracle kararsızlığı:** Sunucu tick zamanlaması çıktıyı değiştirebilir. Tohum ve zaman sabitlenmeli.
7. **Android:** Grafik sürücüsü (Vulkan/GLES), `.so` yükleme ve JNI köprüsü cihaz bazında test edilmeden bitmiş sayılmaz. Fiziksel cihaz yok.
8. **Ağ protokolü:** 26.3 paket düzeni ve sıkıştırma/şifreleme henüz incelenmedi.
9. **Kullanıcı varlıkları:** Varlıklar depoda değil. Build sırasında kullanıcının kendi jar'ından okunacak. Varlık yolu olmadan derleme veri kayıt defteri olmadan tamamlanamaz.
10. **Horizon referansı:** Bu oturumda GitHub erişimi yalnızca `plisquante/monolith` ile sınırlı. `horizon-rust/horizon` okunmadı, lisansı kontrol edilmedi.

## Yol haritası ve durum

| Adım | İçerik | Durum |
|---|---|---|
| 0 | Mimari ve risk | Yazıldı |
| 1 | `java.util.Random` + chunk düzeni + doğrulama fixture'ı | Commit `fb41322` |
| 2 | Decompile (Vineflower 1.12.0, sha1 doğrulandı, client jar eşleşti) | İzin reddedildi, bekliyor |
| 3 | Çeviri betiği `tools/transpile/` ilk dilim (tek sınıf) | Bekliyor |
| 4 | Derlenen çeviri + pencere + boş dünya, tek chunk vanilla karşılaştırması | Bekliyor |
| 5 | Dünya üretimi, kayıt defterleri (build.rs) | Bekliyor |
| 6 | Render, girdi, ağ, ses, Android | Bekliyor |
