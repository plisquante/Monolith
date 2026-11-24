# 03. Çalışma zamanı mimarisi

Etiketler `00_ANA_MIMARI.md` ile aynıdır.

## 1. Crate yapısı (KARAR D16)

| crate | içerik | değişim sıklığı | shard'ları geçersiz kılar mı |
|---|---|---|---|
| `mono_abi` | nesne düzeni (`Obj`, `Class`, `Ref`), tahsis hızlı yolu, `stack_check`, istisna tipi, `materialize`, bölge API, iz tablosu biçimi, taşma/dönüşüm yardımcıları. Hepsi `#[inline]` | nadir, sürümlü (`ABI_VERSION`) | **evet** (yalnızca sürüm artınca) |
| `mono_rt` | toplayıcı (S), iş parçacıkları, kilit, istisna yığın yürüme, yansıma, sinyal, kilit, `ncount`, yerel metot kaydı | sık | hayır (ayrı derlenir, shard'lar `extern` ile bağlanır) |
| `mono_platform` | `Platform` trait: pencere, girdi, pano, dosya sistemi yolları, saat, ses çıkışı, ağ (soket) | orta | hayır |
| `mono_gfx` | RHI: OpenGL/Vulkan arka yüzü (GLFW/OpenAL taklitleri) | orta | hayır |
| `mono_natives` | `Gerçek` ve `Taklit` yerel metot uygulamaları (belge §6) | orta | hayır |

Bugün (KOD): `monolith/rt/*.rs` tek crate (`rt.rs`, `gc.rs`, `sync.rs`, `natives*.rs`, `crash.rs`, `signals.rs`, `ncount.rs`); `rt_hash` tüm dosyaların karması, değişince tüm shard'lar yeniden derlenir.
Geçiş (R1): dosyalar `mono_abi` ve `mono_rt` olarak ayrılır; `ABI_VERSION` shard önbellek anahtarına girer.

## 2. Nesne düzeni

- v1: 16 bayt başlık `{class: *Class, hash: u32, gc: u32}` (KOD `rt.rs` `Obj`). Kilit alanı başlıkta yok (AB9).
- Dizi: başlık + `len: u32` + veri; bellek katmanları (S, F, R, O, I) aynı düzeni kullanır (belge 02 §11).
- **KARAR**: katman bilgisi `gc` alanındaki bitlerdedir (`S=0, F=1, R=2, I=3`; O nesneleri tutamaç ile S'de). GC taraması F/R/I nesnelerine dokunmaz (yalnızca içerdikleri referansları gezer).
- v2 (8 bayt, kilit bitleri içeride): AÇIK D25; V/R/O sonrası ölçülür.

## 3. İş parçacığı ve kilit

- `Thread` → OS iş parçacığı (KOD `rt.rs`); GC güvenli noktaları (`safepoint`) `stack_check` çağrısıyla birleşir.
- `synchronized`: bugünkü hash tablosu (64 parça, KOD `sync.rs`) kalır; **sıcak kilitler** için: kilit çözümü derleme zamanında `kimlik gözlemi` ile (belge 01 §5) kilidi gereksiz kılan yerel nesneler (V/F) için silinir.
- İyileştirme (R3, ÖNERİ): başlıkta ince kilit biti yoksa bile "sahipsiz kilit" yolunu thread-local önbellekle hızlandır. Ölçü: `MonitorChurn` testi ve oyun ana iş parçacığı kilit sayısı.
- Düzeltmeler (KOD, tamamlandı): lost-interrupt, `interrupt` guard süresi, `prune_dead_monitors`, `holdsLock/notify` için `peek`.
- Sinyaller (KOD): SIGTERM/INT/HUP → `Runtime.exit(128+sig)` (`signals.rs`), çökme raporu (`crash.rs`).

## 4. İstisna

- `Result<T, Exc>`; `?` ile yayılım; `try/catch` blok kenarları (MIR). Bugünkü yol korunur.
- Yığın izi: belge 01 §8 (PC tablosu).
- Panik: üretilen kodda yok; çalışma zamanında panik = `crash.rs` raporu + çıkış kodu 134.

## 5. Yansıma, ServiceLoader, MethodHandle

- Yansıma kümesi `reflect-classes.txt` (KOD `mcbuild_rust.py`) kapalı dünya girdisidir (P1); listesiz çağrı çalışma zamanında `NoClassDefFoundError`/`NoSuchMethodError` ile **yüksek sesle** düşer.
- `ServiceLoader`: derleme zamanında `META-INF/services` birleşir, sağlayıcı tablosu üretilir (KOD'da kısmen; kapsam belgelenecek, L-iş paketi).
- `MethodHandle`/`invokedynamic`: derleme zamanında açılım (KOD `indy`); açılamayan site derleme hatası, çalışma zamanı tahmini yok.

## 6. Yerel metot kaydı (KARAR D24)

| tür | anlam | örnek | sayaç |
|---|---|---|---|
| `Gerçek` | Rust'ta gerçek davranış (dosya, soket, zaman, zlib, LZ4) | `FileOutputStream.writeBytes`, `NetIo` | |
| `Taklit` | JNI ya da JNA arayüzünü taklit ediyor, **hedef 0** | netty epoll/kqueue (~150), JNA (56), Tracy (21), anlatıcı (7) | `MONO_COUNT_NATIVES`; kapı N: `Taklit == 0` |
| `Hata ayıklama` | yalnızca doğrulama/hata ayıklama (ör. poison) | | |

- Tablo bugünkü ad kuralından (`n_<sınıf>__<metot>`, KOD) üretilir: `native_registry.json` (`sınıf, metot, tür, sahip, kaynak`); derleme **kaydı olmayan native'i** reddeder (`issues.missing-native`).
- Taklit olanlar için koşullu erişilebilirlik (belge 04 §6) ve yeniden uygulama planı: netty epoll → kapanıştan çıkar (yamalı `isAvailable()`), JNA → doğrudan Rust çağrısı, Tracy → derleme dışı, anlatıcı (Flite) → platform konuşma servisi ya da kaldır.

## 7. Platform

| platform | bugün | hedef |
|---|---|---|
| Linux x86_64 | gerçek oyunla doğrulandı (KOD, STATUS) | birincil |
| Linux arm64 | | CI derleme + duman testi |
| Windows x86_64 | arka yüz var, CI'da önyükleme, donanımda çalıştırılmadı (RELEASE §4) | doğrulanmış sürüm |
| macOS arm64/x64 | aynı | doğrulanmış sürüm |
| Android arm64 | `android/touchcore` var, cihaz testi yok | cihaz doğrulaması (belge 05 cihaz matrisi) |

`Platform` trait: `window`, `input`, `clipboard`, `paths`, `clock`, `audio_out`, `net`; Android ve masaüstü aynı arayüzü uygular. Platforma özel kod yalnızca `mono_platform` içinde.

## 8. Grafik (RHI)

- Bugün: GLFW + OpenGL (+Vulkan) çağrıları natives ile taklit (`java_nolwjgl` kabukları). **Hedef**: kabuklar üretilmiş bildirimler (belge 04), mantık `mono_gfx` Rust crate'inde.
- Çizim komutu tamponu (`RenderPass$Draw`, `GpuBufferSlice`) elle R katmanı (belge 02 §4.1): kare başına bölge, kare sonunda toplu serbest.
- Kabul: kare süresi p99 (belge 05), headless GL duman testi (`headless_play.sh`).

## 9. Ses, ağ

- Ses: mixer (`audio_main.rs` birim testi 12/12, KOD) Rust'ta; OpenAL taklidi `mono_platform::audio_out` üstünde.
- Ağ: `NetIo` gerçek (KOD); TLS (rustls) ve `Epoll` taklidinin kapanıştan çıkarılması (JNI N-işi). Java `SocketChannel` yerleşik yolu kullanılır.

## 10. Tanılama

| araç | durum |
|---|---|
| `crash.rs` çökme raporu `mono-crash-<pid>.txt` | KOD, 5/5 |
| `MONO_COUNT_NATIVES` | KOD |
| `MONO_TRACE_EX` | KOD |
| `gc.pause` sayacı, bölge sayacı, tahsis katmanı sayacı (V/F/R/O/S) | ÖNERİ; belge 05 `alloc_by_tier` |
| `MONO_VERIFY_KKA`, `MONO_REGION_CHECK` | ÖNERİ, belge 02 |
