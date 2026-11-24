# ai-speakings: Claude + Antigravity (Gemini) ortak plan

Tarih: 2026-11-24. Dal: `ccr-6ce47c51-wazur1`. Ana plan: `docs/MASTER_PLAN_TR.md` (WS1–WS14, M0–M5). Bu dosya o planın eksik envanteri ve iki AI arasındaki iş dağılımıdır.

## 0. Hedef ve sınır

Hedef: "projeyi product ready hale getir". Tanım: `docs/MASTER_PLAN_TR.md` bölüm 8'deki 1.0 onay listesinin tamamı, her madde ölçülmüş kanıtla.

Bu oturumun sınırı (dürüst durum):
- Bölüm 8'in çoğu donanım ister: gerçek GPU, Android cihaz, Windows/macOS makinesi, 60 dk soak, tam Minecraft üretimi (~65–70 dk, ~12 GB). Bu ortamda (4 vCPU / 16 GB, GPU yok) bunların bir kısmı çalıştırılabilir, bir kısmı çalıştırılamaz.
- Antigravity/Gemini bu ortamdan çağrılamıyor: `gemini`, `antigravity`, `ag` CLI'ları yok ve Gemini'ye erişen bir araç bağlı değil. Ortak çalışma, dosya üzerinden ve kullanıcı aracılığıyla yapılır (bkz. bölüm 3).
- "Gemini 4 hydrogen high" model kimliği bu oturumda doğrulanamadı. Kullanılacak model ID'si kullanıcıdan teyit edilmeli.

## 1. Ortak protokol

1. Tek kaynak bu dosyadır. Her iş bir ID taşır (G-xx). Durum: `[ ]` açık, `[~]` yapılıyor, `[x]` kanıtla kapandı.
2. Kapanış kanıtı: çalıştırılan komut + filtreli çıktı + tarih. "Yazıldı" kanıt değildir.
3. Dosya sahipliği çakışmaz. Bir dosyayı yalnızca sahibi düzenler; diğeri önerisini bu dosyanın "Öneriler" bölümüne yazar (bölüm 8).
4. Ağır işler (tam üretim, oyun testi, soak) aynı anda yalnızca bir tane çalışır (`MASTER_PLAN_TR.md` bölüm 6, bellek sınırı).
5. Commit ve push yalnızca kullanıcı istediğinde yapılır. Bu dosyanın ilk sürümü `85ae2ef` ile commit ve push edildi. Sonraki düzeltmeler commit edilmedi.

## 2. Eksik envanteri

Kaynak sütunu: dosya:bölüm. "Doğrulandı" sütunu: bu oturumda çalıştırılıp görülen sonuç.

### 2.1 Yayın kapıları (`docs/RELEASE.md` bölüm 2)

| ID | Kapı | Durum | Kanıt / kaynak | Sahip | Not |
|---|---|---|---|---|---|
| G-01 | Lint `pyflakes` | [x] | 2026-11-24 `pyflakes monolith/*.py tools/*.py tests/*.py`, çıktı yok, çıkış 0 | Claude | tekrar gerekir: her değişiklikten sonra |
| G-02 | Rust runtime unit: audio | [x] | `tests/rust/audio_main.rs` 12 passed | Claude | |
| G-03 | Rust runtime unit: crash | [x] | `tests/rust/crash_main.rs` 5 passed | Claude | |
| G-04 | Native diff: crc, netty, recycle, stack, patch | [ ] | Bu oturumda kesildi, çalıştırılmadı. JDK: `/root/.cache/monolith/temurin-25/jdk-25.0.5.1+2` | Claude | Süit çalışırken başlatılmaz (G-05) |
| G-05 | Diferansiyel süit `tests/run_tests.py` | [~] | Bu oturumda başka bir süreç çalışıyor (PID 13606, `MONO_MIR=1`, log boş). Önceki sonuç: 27 pass / 3 skip (§22) | Claude | Süreç sahibi doğrulanmadan öldürülmez |
| G-06 | Link error açıklaması | [ ] | §22 tam üretim raporu: 446 hata, 0 açıklanmamış. Aynı §22'nin "Doğrulanmadı" listesi overlay silindikten sonraki 446/0 sonucunu yeniden istiyor | Claude | Tam üretim gerekir (~70 dk) |
| G-07 | GC stres, iki mod | [ ] | §21: 6/6 plain ve concurrent. Overlay/çevirici değişikliğinden sonra tekrar | Claude | |
| G-08 | Oyun başlatma, plain | [ ] | §22: bir koşu geçti (`headless_play.sh --seconds 60 --clean-exit`, SIGTERM, çıkış 143). Tam üretimden sonra tekrar | Claude | Tam üretim gerekir |
| G-09 | Oyun başlatma, concurrent GC | [ ] | `docs/RELEASE.md` bölüm 4: iki koşu başarılı, 120 sn'lik bir koşu `docs/VERIFICATION.md`'de (bu dosya bu oturumda okunmadı). Yeni koşu yok | Antigravity (ölçüm kaydı) | Tam üretim gerekir |
| G-10 | Kaçak kapısı 60 dk | [~] | §22: +0.180 MiB/dk, bütçe içinde, tek koşu, idle dünya. Tekrar ve yürüyüş/başka dünya yok | Antigravity (koşu + rapor) | Tam üretim + 60 dk |
| G-11 | Paketleme `tools/make_release.py` | [ ] | Betik var, bu oturumda çalıştırılmadı (üretilmiş `build/` yok) | Claude | Tam üretim sonrası |

### 2.2 Çalışma zamanı ve GC (`MASTER_PLAN_TR.md` WS1, WS8)

| ID | Eksik | Durum | Kanıt | Sahip | Not |
|---|---|---|---|---|---|
| G-12 | GC duraklaması ort. 1.1 s, en uzun 1.8 s (625 MiB canlı) | [ ] | §22 soak profili | Claude | Hedef <1 ms. WS1 faz 0 başlamadı. XL |
| G-13 | Kapatılamayan GC: `MONO_GC=off` 13.3 GB RSS | [ ] | `docs/RELEASE.md` bölüm 4 | Claude | Önce çöp azaltma (GC_PLAN katman 1–3) |
| G-14 | Eşzamanlı GC varsayılan değil | [ ] | RELEASE bölüm 4 | Claude | G-09 ve G-10 bekler |
| G-15 | Kare süresi, RSS ve menü süresi ölçüsü boş | [ ] | `MASTER_PLAN_TR.md` bölüm 9, tablo boş | Antigravity (HotSpot karşılaştırması) | "Boş hücre = ölçülmedi" kuralı |

### 2.3 Java ve JNI/JNA (WS2, WS6, WS7)

| ID | Eksik | Durum | Kanıt | Sahip | Not |
|---|---|---|---|---|---|
| G-16 | `classlib/` ve `runtime/java_nolwjgl/` içinde Java kaynağı | [~] | WS7: 34 dosya silindi (277→243). Kalan 243 | Claude | Kural: sıfır Java, istisnalar yazılı |
| G-17 | JNI/JNA emülasyon natifleri | [~] | `docs/JNI_ENVANTER_TR.md`: 259 native. Oyun koşusunda 139 çağrı, hepsi JNA ve Flite | Antigravity (envanter gözden geçirme) | WS2 hedefi: sayaç sıfır |
| G-18 | Sınıf seviyesi yama (JSSE) | [ ] | WS5 kararı: önce çevrilmiş JSSE denenir, sonra rustls | Claude | |
| G-19 | Eksik `TLS` | [ ] | `MASTER_PLAN_TR.md` WS5, "Yok" listesi | Claude | Gerçek sunucu testi gerekir |

### 2.4 Platform (WS3, WS4, WS5, WS10, WS12)

| ID | Eksik | Durum | Kanıt | Sahip | Not |
|---|---|---|---|---|---|
| G-20 | Gerçek GPU testi | [ ] | Yalnız llvmpipe | Kullanıcı (donanım) | Bu ortamda GPU yok |
| G-21 | Ses çıkışı, underrun kontrolü | [~] | Rust ses testleri 12/12. Cihaz/çıkış testi yok | Claude (kod) / Kullanıcı (donanım) | |
| G-22 | Windows ve macOS | [ ] | `.github/workflows/game.yml` yazıldı (§14), GitHub'da çalıştırılmadı | Antigravity (CI kanıtı, GitHub erişimi varsa) | M3 |
| G-23 | Android cihaz | [ ] | APK derleniyor, `.so` sembolleri var. Cihazda çalışmadı (§18, §19) | Kullanıcı (cihaz) + Claude (kod) | M4 |
| G-24 | CI/derleyici makinesi kararı | [ ] | WS10 | Kullanıcı | |
| G-25 | Launcher Python'sız paket, güncelleme kontrolü | [ ] | Doğrulanmadı | Claude | WS11 |
| G-26 | Dağıtım imzası, temiz makinede "indir→aç→oyna" | [ ] | Yok | Claude + Kullanıcı | 1.0 onayı |

### 2.5 Belgeler ve temizlik (WS14, bu oturum)

| ID | Eksik | Durum | Kanıt | Sahip | Not |
|---|---|---|---|---|---|
| G-27 | `CLAUDE.md` eski: "Zig back end, `monolith/` transpiler" diyor | [ ] | `ARCHITECTURE_MAP.md` satır 1: Zig arka ucu kaldırıldı | Kullanıcı (proje talimatı) | Değiştirilmedi. Onay gerekir |
| G-28 | `.zig` referansları kalıntı | [~] | §22: "stale .zig references in comments point at Rust modules". Tam tarama yapılmadı | Antigravity (tarama, öneri) | `monolith/` ve `tools/` yorumlarını Claude uygular |
| G-29 | Eski belgeleri işaretleme | [ ] | `MASTER_PLAN_TR.md` bölüm 11 madde 5 | Antigravity | |
| G-30 | Ürün kullanım kılavuzu, sürüm notları | [~] | `docs/KULLANICI_REHBERI_TR.md` var (§22) | Antigravity | Kılavuz ayarları kodla karşılaştırıldı |

### 2.6 Karar bekleyenler (kullanıcı)

| ID | Konu | Neden |
|---|---|---|
| D-01 | WS13 hukuk kapısı (dağıtım) | Sürümü durdurur (`MASTER_PLAN_TR.md` bölüm 10). Hukuki karar kullanıcıya ait |
| D-02 | Gemini/Antigravity erişimi ve model kimliği | Bu ortamdan çağrılamıyor. Model adı doğrulanamadı |
| D-03 | Donanım: GPU, Android cihaz, Windows/macOS | G-20, G-22, G-23 kapanmaz |
| D-04 | Bölüm 8 ve bu gözden geçirmenin düzeltmeleri | Commit edilmedi. Commit ve push izni gerekir |
| D-05 | CLAUDE.md güncellemesi (G-27) | Proje talimatı; kullanıcı onayı olmadan değişmez |

## 3. İş dağılımı

Claude (kod ve kapılar, dosya sahibi):
- `monolith/`, `monolith/rt/`, `tools/`, `tests/`: G-04, G-05, G-06, G-07, G-08, G-11, G-12, G-13, G-14, G-16, G-18, G-19, G-21 (kod kısmı), G-25, G-26.
- Her değişiklikten sonra G-01 (pyflakes) ve ilgili en dar test.

Antigravity / Gemini (ölçüm, envanter, belge; kod yazmaz ya da Claude'un onayıyla yazar):
- G-09, G-10: koşu ve ölçüm kaydı (`docs/VERIFICATION.md`).
- G-15: HotSpot karşılaştırma tablosu (`docs/BENCHMARK.md`), boş hücre kuralı.
- G-17: JNI/JNA envanteri gözden geçirme (`docs/JNI_ENVANTER_TR.md`).
- G-22: CI kanıtı (GitHub erişimi varsa).
- G-28, G-29, G-30: `.zig` kalıntı taraması, belge işaretleme, kılavuz doğrulaması.

Çakışma kuralı: Antigravity `monolith/` ve `tools/` dosyalarını değiştirmez. Önerilen değişiklik bu dosyada "öneri" başlığıyla yazılır, Claude uygular.

## 4. Sıra

1. G-05'in bitmesini bekle (PID 13606), sonra G-04 (native diff, ucuz).
2. G-06 ve G-08 için tam Minecraft üretimi tek koşu olarak (~70 dk, 12 GB). Bu bitmeden G-07, G-09, G-10, G-11 kapanmaz.
3. G-12 (WS1 faz 0): duraklama ölçüm aracı, sonra çöp azaltma. Uzun iş, birkaç tur.
4. G-16, G-18, G-19: Java kalıntısı ve TLS.
5. G-25, G-26: paketleme ve launcher.
6. Donanım gerektiren maddeler (G-20, G-22, G-23) kullanıcı cihazı gelince.

## 5. Bu oturumun doğrulamaları (2026-11-24)

- G-01: `pyflakes monolith/*.py tools/*.py tests/*.py`, çıkış 0, çıktı yok.
- G-02: `rustc --edition 2021 --test tests/rust/audio_main.rs`, 12 passed.
- G-03: `rustc --edition 2021 --test tests/rust/crash_main.rs`, 5 passed.
- G-04: başlatıldı, kesildi. Sonuç yok.
- G-05: başka süreç çalışıyor, sonuç yok.

## 6. Durum özeti

Kapalı: G-01, G-02, G-03 (bu oturum). Kısmi: G-05, G-10, G-16, G-17, G-21, G-28, G-30. Açık: geri kalan. Donanım ve karar bekleyen: D-01..D-05.

1.0 onay listesi (`MASTER_PLAN_TR.md` bölüm 8): 1 maddesi işaretli (fark testleri yeşil; önceki oturum). Kalanlar açık.

## 7. Gemini ilk görevi

Bu bölüm Gemini'nin ilk oturumu için. Gemini bu dosyayı okuyup şunları yapar, her madde için kanıt ister:

1. Model kimliğini ve erişim yolunu bildirir (bu oturum doğrulayamadı, D-02).
2. Bölüm 2'den iki alan seçer: G-15 (HotSpot ölçüm tablosu) ve G-17 (JNI/JNA envanter gözden geçirme). Seçtiği maddeleri bölüm 8'e yazar.
3. Bölüm 2'deki her `Antigravity` sahipli maddenin ilk adımını ve gereken donanımı yazar. Donanım yoksa "yapılamaz" der, tahmin yazmaz.
4. Bölüm 3'teki çakışma kuralını kabul eder ya da itiraz eder.

Çıktı formatı: bölüm 8 altında `### Gemini yanıtı (tarih)`, madde ID'leri ve kanıt. Kod değişikliği önermez, yalnızca öneri yazar.

## 8. Öneriler

Henüz kayıt yok. Gemini ve Claude buraya yazar. Format: `- [öneren] G-xx: öneri, kanıt, etki`.
