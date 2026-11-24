# Monolith ana planı (sürüm yoluna kadar)

Tek geçerli plan bu dosyadır; uygulanabilir görev listesi ve kapalı kararlar için `IMPL_SPEC.md`. `YOL_HARITASI_TR.md` (Zig dönemi) ve `STATUS.md` (kronolojik kayıt) tarihîdir. GC ve bellek ayrıntısı:
`GC_PLAN.md`. Bu plan sözdür: bir madde bitmiş sayılmak için "çıkış ölçütü" sütunu ölçülmüş olmalı.

## 1. Ürün tanımı

Monolith, Minecraft Java 26.3 istemcisini (bayt kodu) AOT olarak yerel koda çevirir. Çalışma zamanında JVM yok, GC duraklaması yok.
Sürümler "Monolith Rust+ Edition" adıyla çıkar. Hedef platformlar: Linux x86_64, Windows x86_64, macOS arm64, Android arm64.
Sürüm politikası: bir sürüm tek bir Minecraft sürümünü destekler (şu an 26.3). Yeni Minecraft sürümü = yeniden çeviri + yama doğrulaması.

"Java'dan kalma her şey Rust'a" şu anlama gelir (kapsam sınırı net olsun diye):
* Çalışma zamanında JVM, JNI, JNA, Java sınıf kütüphanesi **kaynağı** kalmaz: JDK/LWJGL yerine geçen elle yazılmış Java kaplamaları (`classlib/`, `runtime/java_nolwjgl/`, ~330 dosya) Rust yerel kod ve Rust yamalarıyla değiştirilir.
* Minecraft'ın kendi mantığı ve JDK'nın ihtiyaç duyulan kısımları bayt kodundan Rust'a çevrilmeye devam eder (bu çevirinin kendisidir). Çeviri aracı Python kalır (derleme aracı, çalışma zamanında yok).
* Launcher arayüzleri Java/Python/HTML kalabilir (kullanıcı kararı).

## 2. Hedef mimari (katmanlar)

| Katman | İçerik | Dil | Durum |
|---|---|---|---|
| L0 Çevirici | bayt kodu → Rust, satır-içi, kaçış analizi, yama bağlama, shard derleme | Python | çalışıyor |
| L1 Çalışma zamanı `mono_rt` | nesne modeli, bellek katmanları, iş parçacığı/kilit, istisna, yansıma, MethodHandle, yama API | Rust | çalışıyor; bellek yenilenecek (GC_PLAN) |
| L2 Platform | pencere+giriş, GL bağlamı, grafik RHI, ses, ağ+TLS, dosya, panoya erişim | Rust | yalnız Linux/X11+GL+kısmi ses(dlopen OpenAL) |
| L3 Kaplamalar | JDK/LWJGL/JNA yerine geçenler | Java→**Rust** | Java; taşınacak |
| L4 Yamalar | performans ve davranış değişiklikleri (metot ve sınıf seviyesinde) | Rust | metot seviyesi var; sınıf seviyesi yok |
| L5 Launcher | hesap girişi, indirme, başlatma | Python+HTML (masaüstü), Java TouchCore (Android) | var |
| L6 Dağıtım | paketleme, imza, güncelleme, CI | Python/YAML | eksik |

Çıkarıldı: Zig arka ucu (`runtime/*.zig`, `zigen.py`). Rust arka ucu fark testlerini (`tests/run_tests.py` kümesi) geçiyor,
Tek arka uç = tek doğruluk kaynağı.

## 3. Durum (kodda doğrulanan)

Çalışıyor: Linux masaüstü oyun (menü, dünya, giriş; Xvfb+llvmpipe), yama sistemi (9 yama), tahsis profil aracı, DataFixer yaması, kaçış analizi+satır-içi (birim testli; gerçek oyunda doğrulama sürüyor), iterator geri dönüşümü, başlatıcı, marka.
Yok veya doğrulanmadı: gerçek GPU testi (yalnız yazılım GL denendi), ses çıkışı, TLS/çevrimiçi, Windows, macOS, Android'in yeni çekirdekle cihazda çalışması, GC yenilemesi, JNI/JNA'nın kaldırılması, sınıf seviyesi yama, dağıtım hattı.

## 4. İş akışları (workstream)

Büyüklük: S ≈ bir tur, M ≈ birkaç tur, L ≈ çok tur, XL ≈ çok tur + cihaz/donanım testi. Çevirici/çekirdek değişikliği bir tam yeniden
üretim (~65 dk) maliyetlidir; çalışma zamanı-yalnız değişiklikler `--reuse-objs` ile ucuzdur. Bu yüzden iş akışları iki hatta ayrılır (bölüm 6).

### WS1 Bellek (L1) , XL
`GC_PLAN.md`: dört katman (frame, region, owned, managed), eşzamanlı SATB rölanti toplayıcı, sahipli bellek, kaçak bütçesi, GC kapatma soak testi.
Çıkış: duraklama <1 ms, yönetilen tahsis <5 MiB/sn, 60 dk soak büyüme <1 MiB/dk; geçerse GC varsayılan kapalı, özellik olarak kalır.

### WS2 Yerel sınır: JNI/JNA → Rust FFI (L1/L3), L
* Envanter: çalışma zamanında çağrılan her native metot (`MONO_TRACE_NATIVES` ile çıkarılır), sınıflandırma: Rust'ta gerçek, kaldırılabilir, kaplama.
* `natives_jni.rs` (JNI taklit katmanı) yalnızca hâlâ gereken yerlerde kalır, hedef sıfır; JNA/oshi/narrator çağrıları Rust yerel kodla ya da kaldırılarak değiştirilir.
* Her native `#[native]` kayıt tablosunda tek yerde; eksik native = yüksek sesle hata (mevcut kural).
Çıkış: oyunda 30 dk oynanışta JNI/JNA girişi **0** (sayaç raporu), `libffi` bağımlılığı kalktı.

### WS3 Grafik ve pencere (L2), XL
* Masaüstü: OpenGL 4.5 çekirdek (Linux/Windows), Android: GLES 3.2. Vulkan 1.0 sonrasına bırakıldı (kaplama taslakları var, ana yol değil).
* Gölgelendirici derleme: shaderc + SPIRV-Cross (sistem paketi) yerine saf Rust `naga` ya da gömülü derleyici; sistem bağımlılığı sıfır.
* Pencere/giriş: X11 (var, xdotool testi), Wayland (XWayland yeterli, yerli Wayland sürüm sonrası), Windows: Win32+WGL, macOS: Cocoa. Yerleşik ortak soyutlama `Platform` trait'i.
* **macOS (karar kapandı, bkz. IMPL_SPEC D3):** OpenGL 3.3 core. Bağlam zaten 3.3 core isteniyor, oyunun gölgelendiricileri `#version 330`, macOS 4.1 core'a kadar destekler. Vulkan/MoltenVK yok.
* Gerçek GPU: llvmpipe dışında en az bir gerçek GPU (Mesa/NVIDIA/AMD) ve bir mobil GPU (Adreno/Mali) ile görsel+kare süresi testi.
Çıkış: her platformda menü + dünya + 10 dk gezinme, gerçek GPU'da görüntü hatası yok, karşılaştırma ekran görüntüsü farkı eşik altında.

### WS4 Ses: Rust Audio (L2), L
* OpenAL API alt kümesini Rust'ta gerçekle (kaynak, tampon, dinleyici, mesafe modeli, akış tamponları, HRTF yok). Oyun yalnız bu alt kümeyi kullanıyor; kapsam envanterle çıkarılır.
* Çıkış aygıtları: Linux (ALSA/PulseAudio/PipeWire dlopen), Windows (WASAPI), macOS (CoreAudio), Android (AAudio).
* Ogg çözme: oyun JOrbis (saf Java, zaten çevriliyor) kullanıyor; Rust çözücü şimdilik gerekmez.
* Bağımlılık politikası: bölüm 7.
Çıkış: tüm ses olayları (müzik, blok, çevre, 3B konum) çalar, 30 dk oynanışta underrun sayacı ~0, ses kodu `libopenal` bağımlılığı olmadan çalışır.

### WS5 Ağ ve çevrimiçi (L2), M
* Soketler (`natives_net.rs`) var; eksik olan **TLS**. Karar kuralı (IMPL_SPEC D7): önce çevrilmiş JSSE soketlerimizin üzerinde denenir (B5), çalışmazsa rustls yaması. Oturum doğrulama, kaplama/doku indirme, Realms HTTPS ister. JDK'nın JSSE'si çevrilerek çalıştırmak ağır ve kırılgan; plan: sınıf seviyesi yamayla `SSLContext/SSLSocket` yerine Rust TLS (rustls sınıfı).
* Hesap girişi launcher'da (Microsoft akışı), oyuna yalnız erişim belirteci verilir.
Çıkış: gerçek sunucuya katılma testi (çevrimiçi mod), kaplama indirme, şifreli oturum.

### WS6 Yama sistemi (L4), M
* Sınıf seviyesi yama: manifest'te `class` girdisi (bir sınıfın metotlarını/alanlarını toptan değiştir, yeni alan ekle), mevcut `bindings` ile aynı güvence (orijinale düşme, verify modu).
* Yama kataloğu ve sahiplik bildirimi (`owner:`, bkz. WS1), yama başına test (`patch_tests.py`).
* Yama sürüm uyumu: Minecraft sürümüne bağlı bayt kodu karması, uyuşmazlıkta build yüksek sesle başarısız olur.
Çıkış: JDK/LWJGL kaplama Java dosyalarının tamamı yama/native ile karşılandı (WS7).

### WS7 Java kaplamalarını Rust'a taşıma (L3), L
Sıra: (1) `java_nolwjgl` (≈270 dosya: opengl 31, vulkan 136, sdl 38, openal 16, glfw 11…), önce kullanılanlar; kullanılmayan Vulkan taslakları silinir.
(2) `classlib` (58 dosya: `String` UTF-16 kaplaması vb.). Her dosya için: Rust yerel karşılığı, test, Java kaynağını sil.
Çıkış: `classlib/` ve `runtime/java_nolwjgl/` içinde Java kaynağı kalmadı (ya da yalnızca gerekçesi yazılı istisnalar).

### WS8 Performans (L0/L1), L
Hedef ölçüler HotSpot karşısında (aynı makine, aynı dünya): ilk menüye kadar süre, dünya açılışı, RSS, kare süresi p50/p99, tick süresi.
Mevcut kayıt: headless başlatma ~50 sn bekleme gerekiyor; masaüstü hedefi menü <15 sn.
İş listesi: GC_PLAN §6 çöp azaltma, 16B başlık/sıkıştırılmış referans (ölçüm sonrası karar), Latin-1 String, shard-arası satır-içi/ThinLTO, başlangıç profili.
Çıkış: bölüm 8'deki sürüm ölçütleri.

### WS9 Doğruluk ve test (hepsi), L
* Fark testleri (HotSpot ↔ Rust): `tests/run_tests.py` kümesi Rust arka uçta yeşil.
* Oyun testleri: `headless_play.sh` betiği (menü→dünya→gezinme), çökme raporu kontrolü, `MONO_ARENA_CHECK`, `MONO_PATCH_VERIFY`.
* Uyumluluk: vanilla dünya yükleme ve kaydetme (geri okunur), vanilla sunucuya katılma, kaynak paketi, ayarlar.
* Çökme raporlayıcı: yerel çökme (SIGSEGV/panic) yakalayıp rapor yazar (Rust panic kancası + sinyal işleyici).
Çıkış: her sürüm adayında yeşil test matrisi (bölüm 8).

### WS10 Derleme ve CI (L6), M
* Tam derleme ~10 dk çeviri + ~55 dk derleme, ~12 GB bellek. CI: GitHub barındırılan koşucu yetmeyebilir; seçenek: kendi barındırılan koşucu/bulut derleyici. Karar ve kurulum bu akışın ilk işi.
* Çıktılar önbelleğe alınır (shard önbelleği anahtarı: çalışma zamanı karması + kaynak karması). Tekrarlanabilir yapı (aynı girdi = aynı ikili).
* Çapraz derleme: Windows (x86_64-pc-windows-gnu/msvc), Android (NDK), macOS (macOS koşucusu).
Çıkış: bir etiket (tag) iterek tüm platformların yapıları otomatik üretilir.

### WS11 Launcher ve dağıtım (L5/L6), M
* Masaüstü: `launcher.py`+`ui.html`; sürümde Python'sız paketlenir (gömülü yorumlayıcı ya da küçük Rust sunucusu). Kararı: v1 paketlenmiş Python, v2 Rust'a yazılması sürüm sonrasına bırakıldı.
* Oyun dosyaları (asset'ler) ilk açılışta kullanıcının hesabıyla Mojang'dan indirilir, depoda ve paketlerde bulunmaz.
* Paketler: Linux (tar + AppImage), Windows (zip + kurucu), macOS (.app/.dmg), Android (APK). İmzalama ve sürüm numarası (`0.x` alfa/beta, `1.0`).
* Güncelleme: launcher yeni sürümü kontrol eder (sürüm manifesti, SHA-256).
Çıkış: temiz makinede "indir → aç → oyna" 5 dakika altında, el ile komut yok.

### WS12 Android (L2/L5), XL
* Yeni çekirdekle `libminecraft_mono.so` yeniden derlenir (bulutta/CI'da; telefonda derlenmez).
* TouchCore (Java arayüz) `.so`'yu yükler; dokunma kontrolleri; AAudio ses; GLES yolu.
* Cihaz testi: gerçek cihazda menü, dünya, 10 dk oynanış, bellek (LMK sınırları), ısınma; raporlar `report-metrics` hattına.
Çıkış: iki farklı cihazda (bir Adreno, bir Mali) oynanabilir; bellek tepe değeri ölçüldü ve belgelendi.

### WS13 Hukuk kapısı (karar; sürüm dışına çıkmadan), S
Şimdilik ertelendi (kullanıcı kararı). Ama bir kapıdır: **herkese açık dağıtımdan önce** karar verilmesi gerekir. Çünkü ikili, çevrilmiş Mojang kodu içerir ve
son kullanıcı 65 dk/12 GB derlemeyi çalıştıramaz. O zamana kadar sürümler "özel test sürümü"dür (belirli kişilere, açık dağıtım yok).

### WS14 Dokümantasyon, S
Eski belgeler (`STATUS.md`, `YOL_HARITASI_TR.md`, `ARCHITECTURE.md` eski bölümleri) arşive; güncel: bu plan, `GC_PLAN.md`, `MONOLITH.md`, kullanıcı rehberi (kurulum, sorun giderme), geliştirici rehberi (yama yazma, test, profil).

## 5. Sürüm yolu (kilometre taşları)

| Taşı | İçerik | Çıkış |
|---|---|---|
| **M0** iç alfa (Linux) | WS9 oyun testi + mevcut yapının doğrulanması, WS10 CI kurulumu | 30 dk betikli oynanış çökmeden; launcher ile başlıyor; etiket `v0.1` |
| **M1** ses + ağ | WS4 Rust ses, WS5 TLS, WS2 JNI/JNA sayacı sıfır | çevrimiçi sunucuya katıl + sesli oynanış; `v0.3` |
| **M2** bellek | WS1 faz 3–6 (rölanti toplayıcı, çöp azaltma, sahipli bellek), WS8 ölçümleri | duraklama <1 ms, soak geçti; `v0.5` |
| **M3** Windows + macOS | WS3 pencere/GL, WS4/WS5 aygıt arka uçları, WS10 çapraz derleme, macOS kararı | iki platformda M0 testi; `v0.7` |
| **M4** Android | WS12 | iki cihazda oynanış; `v0.9` |
| **M5** adaylık | WS6/WS7 Java kaynağı sıfır, WS8 hedefleri, WS9 matrisi, WS11 paketler, WS14, WS13 kararı | bölüm 8 kutuları; `1.0-rc` → `1.0` |

Sıra zorunlu değil, bağımlılık: M2 ile M1 paralel yürür (bölüm 6). M3/M4, M1'in ses/ağ API'sine dayanır; M5 hepsine.

## 6. Yürütme: iki hat

Tam yeniden üretim pahalı (~65 dk) olduğu için işler ikiye ayrılır:

* **Hat A (çekirdek):** çevirici, bellek katmanları, bariyer, satır-içi/kaçış, yamalar. Tam yeniden üretim ister; değişiklikler **toplu** uygulanır (bir üretimde bir dizi değişiklik). Küçük test programlarıyla önce doğrulanır.
* **Hat B (platform):** ses, TLS, pencere/GL, FFI, launcher, paketleme. Çalışma zamanı-yalnız (`--reuse-objs`) ya da ayrı test programlarıyla geliştirilir, tam oyunu beklemez.
* Aynı anda üretim+derleme+oyun testi çalıştırılmaz (bellek sınırı). Bir hat derlerken diğer hat kod yazar.
* Her adım sonunda: ölçüm kaydı (`docs/ALLOC_BASELINE.md` ve ilgili baseline), commit, push.

## 7. Bağımlılık ve araç politikası

Hedef JVM'den ve Java'dan çıkmaktır; harici kütüphane yasağı yoktur. İyi, olgun bir kütüphane varsa yeniden yazmak yerine kullanılır (Rust ya da C/C++ fark etmez).

* Seçim ölçütü sırası: (1) doğru ve olgun, (2) hedef platformların hepsinde derleniyor, (3) lisansı uyumlu, (4) ölçülmüş kazancı var, (5) bakım yükü düşük.
* Her bağımlılık içeri alınır (vendored), sürümü ve karması sabittir; yapı ağ gerektirmez. Liste ve lisanslar `docs/THIRD_PARTY.md`.
* Adaylar (kullanım ölçümle onaylanır): bellek: kendi tahsis edicimiz kalır (kaçak yakalama sınıf/tahsis noktası bilgisine dayanır); mimalloc yalnızca nesne dışı bellekte sistem `malloc` ölçümde sıcak çıkarsa denenir, aksi halde eklenmez, pencere: winit ya da doğrudan platform API'si, ses: cpal ya da doğrudan arka uçlar, TLS: rustls, gölgelendirici: naga, Vorbis çözücü, zlib/deflate (zlib-rs ya da zlib).
* `rustc` sürümü sabit (stable, belgelenmiş), çapraz hedefler `rustup target`. Önce crates.io erişimi ve derleme düzeni (rustc doğrudan mı, cargo mu) bir kerelik doğrulanır.
* Sistem paketi bağımlılığı (apt `libshaderc`, `libopenal`) sürümde sıfır olmalı: ya içeri alınır ya da kaldırılır.

## 8. Sürüm 1.0 onay listesi (hepsi ölçülmüş olmalı)

- [ ] 4 platformda: menü → yeni dünya → 30 dk betikli oynanış, çökme yok (her platformda en az bir gerçek GPU/cihaz).
- [ ] Çalışma zamanında JVM, JNI, JNA, libffi yok; `classlib/` ve `java_nolwjgl/` içinde Java kaynağı yok (istisnalar yazılı).
- [ ] GC: duraklama <1 ms ya da GC kapalı + 60 dk soak <1 MiB/dk büyüme.
- [ ] Ses tam, kopma/underrun yok; çevrimiçi sunucuya TLS ile katılma.
- [ ] Menü açılışı <15 sn (masaüstü), kare süresi p99 ve RSS HotSpot'tan kötü değil (hedef tablosu bölüm 9'daki ölçümlerle doldurulur).
- [ ] Vanilla dünya yükleme/kaydetme uyumlu; kaynak paketi ve ayarlar çalışıyor.
- [x] Fark testleri yeşil; Zig arka ucu silindi.
- [ ] Paketler üretiliyor, imzalı, "indir → aç → oyna" temiz makinede çalışıyor.
- [ ] Launcher Python'sız paketli; güncelleme kontrolü çalışıyor.
- [ ] Belgeler güncel; çökme raporlayıcı çalışıyor.
- [ ] Hukuk kapısı (WS13) karara bağlandı.

## 9. Ölçü tablosu (doldurulacak, M0'da başlar)

HotSpot karşılaştırması: aynı dünya, aynı makine; ilk menüye süre, dünya açılışı, RSS tepe/ortalama, kare süresi p50/p99, tick süresi, GC/yönetilen tahsis hızı.
Ölçü araçları: `tools/bench.py`, `tools/headless_play.sh`, `tools/alloc_report.py`, TouchCore raporları. Sonuçlar `docs/BENCHMARK.md` ve baseline dosyalarına yazılır. Boş hücre = ölçülmedi, tahmin yazılmaz.

## 10. Riskler

| Risk | Etki | Karşılık |
|---|---|---|
| Gerçek GPU'da görüntü/uyumluluk hatası (yalnız llvmpipe denendi) | yüksek | WS3'te erken gerçek GPU testi (kullanıcı cihazları dahil) |
| macOS GL 4.1 sınırı | orta/yüksek | probe + karar (WS3) |
| Derleme maliyeti (65 dk, 12 GB) | hız | iki hat, toplu değişiklik, önbellek, güçlü CI makinesi. Ölçüm: küçük programda 4 shard/4 iş 3,1x hızlı (`docs/BUILD_PERF.md`); Minecraft ölçülmedi |
| Eksik bariyer / yanlış sahiplik → bellek bozulması | yüksek | tek saklama API'si, verify modu, zehirleme modu, soak |
| TLS'nin JDK yerine Rust'a taşınması | orta | sınıf seviyesi yama, gerçek sunucu testi |
| Android bellek ve ısınma | yüksek | cihaz testi, ölçülen sınırlar, ayarlanabilir kalite |
| Hukuk (dağıtım) | sürümü durdurur | WS13 kapısı; o zamana kadar özel test sürümü |
| Minecraft sürümüne bağımlılık | bakım | tek sürüm/sürüm; yama uyum denetimi |

## 11. Hemen sıradaki işler (bu plan onaylı)

1. Çalışan build'i doğrula (M0): çökme/ihlal denetimi, tahsis profili, tickChunk etkisi.
2. WS1 faz 0: duraklama/heap izleme ve kaçak bütçesi aracı.
3. Hat B başlangıcı: WS2 native envanteri (`MONO_TRACE_NATIVES`), WS4 ses kapsam envanteri, crate/derleme düzeni doğrulaması (bölüm 7).
4. WS10: CI/derleyici makinesi kararı.
5. Eski belgeleri işaretleme (bu commit) ve her adımda `docs/` güncelleme.
