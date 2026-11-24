# 4 günlük bitirme planı

Kaynak: `MASTER_PLAN_TR.md` (WS1–WS14, bölüm 8 onay listesi). Bu belge onu 4 güne böler. Hedef: 1.0 onay listesinin tamamı.
Durum tarihi: 2026-11-24. Bu belgedeki "bitti" yalnızca ölçülmüş çıktıyla yazılır (`docs/RELEASE.md` kuralı).

## 0. Kısıtlar (planın dayandığı gerçekler)

- Tam yeniden üretim + derleme ~65 dk, ~12 GB bellek. Aynı anda tek ağır iş (guard zorunlu kılar). 4 günde en fazla ~20 tam build var. Bu yüzden çevirici değişiklikleri toplu yapılır, runtime-yalnız değişiklikler `--reuse-objs` ile yapılır.
- Bu ortamda yok: Android NDK/SDK, adb, gerçek GPU, Windows, macOS, telefon. Bunlara bağlı maddeler "Kullanıcıdan gerekenler" bölümünde, gün gün istenir.
- Bugün ölçülmüş: lint, Rust birim testleri, crc/netty/recycle/patch/stack testleri, GcStress 6/6 (iki mod), rt_typecheck, TouchCore 29/29 (`STATUS.md` §21). Tam build bir kez çalışıyor (16 shard, `--shard-jobs 1`); 4 shard ile rustc tek shardda 12,7 GB'a çıkıp OOM oldu.
- Son transpile: 446 link error (eski kayıt 137). Neden açıklanmadan 1.0 olmaz; Gün 1'in ilk işi.

## 1. Paralel hatlar

| Hat | İş | Ağır iş? |
|---|---|---|
| A Çekirdek | çevirici/runtime değişiklikleri, tam build, oyun testi | evet (tek slot) |
| B Kaplama taşıma | WS7: Java → Rust, dosya dosya; WS2 JNI/JNA sayacı | hayır (kod yazma, birim test) |
| C Platform | WS3/WS4/WS12: Windows, macOS, Android, GPU | kullanıcı donanımı |
| D Dağıtım | WS10, WS11, WS14: CI, paket, launcher, belgeler | CI koşucusu |

Hat A'nın build slotu gece boyunca dolu tutulur; B ve D build beklerken ilerler.

## 2. Gün 1: doğrulama temeli ve ölçü (M0)

Çıkış: Linux'ta 30 dk betikli oynanış, çökme yok, `v0.1` etiketi.

1. 446 link error: `report.json` `issues.link-error` kategorilere ayrılır (JDK 22–25 API eksikleri, opsiyonel bağımlılık, gerçek bug). Gerçek bug olanlar düzeltilir, kalanlar belgelenir. (Hat A, çevirici; build gerektirmez)
2. Çalışan 16 shard'lı build tamamlanır → `headless_play.sh` ile 60 sn, sonra 30 dk oynanış. Dünya "New World" bir kez elle yaratılır. (A)
3. Gate seti sabitlenir: lint, birim, 5 fark modülü, GcStress (iki mod), rt_typecheck, `headless_play`. Hepsi tek komutla (`tools/release_check.py` genişletilir). (D)
4. Ölçü tablosu (MASTER_PLAN §9) ilk kez doldurulur: menüye süre, RSS, kare süresi p50/p99, HotSpot karşılığı. Başlangıç noktası olmadan hedef tutturulamaz. (A)
5. WS7 envanteri: `classlib/` (58) ve `java_nolwjgl/` (~270) için dosya başına durum tablosu: kullanılıyor/kullanılmıyor, Rust karşılığı var/yok. Kullanılmayan Vulkan taslakları silinir. (B)
6. `touchcore_core.zig` silinir; Zig kalıntıları temizlenir. (B)

Kullanıcıdan gereken: yok. (Hukuk kararı WS13 hâlâ ertelenmiş; Gün 4'te sorulur.)

## 3. Gün 2: bellek ve doğruluk (M2 ilk yarı)

Çıkış: GC duraklaması <1 ms ya da GC kapalı + soak başladı; JNI/JNA sayacı raporu var.

1. WS1 çöp azaltma (`GC_PLAN.md` tier 1–3): Vec3/BlockPos/iterator/Optional/DataResult geçicileri (ALLOC_BASELINE'daki ilk 10 site). `--escape --inline` ölçümde 525 link error ve +%41 tahsis verdi; açılmaz, yerine tek tek yama. (A)
2. Rölanti toplayıcı: concurrent mod varsayılan adayı; 60 dk soak (`leak_soak.sh`) gece koşusu. Bütçe: <1 MiB/dk büyüme. (A, gece)
3. WS2: `MONO_TRACE_NATIVES` ile 30 dk oynanışta her JNI/JNA girişi sayılır; sınıflandırma tablosu; sayacı sıfırlayan ilk toplu (oshi, narrator, JNA dispatch). (B)
4. WS7 ilk toplu: `java_nolwjgl` içinden en çok kullanılan paketler (glfw 11, opengl 31, stb) → Rust native + Java dosyası silinir. Her dosya: test + `tests/run_tests.py` yeşil. (B)
5. WS6 sınıf seviyesi yama: manifest `class` girdisi, verify modu, `patch_tests`. WS7'nin `String`, `Reference` gibi classlib dosyaları buna dayanır. (B)

Kullanıcıdan gereken: gerçek GPU'lu bir Linux/Windows makine ya da ekran görüntüsü karşılaştırması için erişim.

## 4. Gün 3: platformlar, ses, ağ (M1 + M3 + M4)

Çıkış: TLS ile sunucuya katılma, sesli oynanış, Windows/macOS/Android ilk koşu.

1. WS4: OpenAL alt kümesi Rust'ta, `libopenal` bağımlılığı olmadan; underrun sayacı; Linux (ALSA/Pulse), WASAPI, CoreAudio, AAudio aygıtları. (B)
2. WS5: TLS. IMPL_SPEC D7 kuralı: önce JSSE yolu denenir; çalışmazsa rustls sınıf yaması. Sunucuya katılma + oturum doğrulama. (B + A build)
3. WS3: shaderc/SPIRV-Cross yerine `naga` (Android'de gölgelendirici derleme bloğu bu). Önce masaüstü GL yolunda doğrulanır, sonra GLES. (B)
4. WS12: NDK + SDK kurulur (bulut/CI'da), `libminecraft_mono.so` yeniden derlenir, APK imzalanır. Cihaz testi kullanıcıda: Adreno + Mali, `report-metrics` raporu. (C)
5. WS10: GitHub hosted runner 12 GB'a yetmez; self-hosted ya da bulut derleyici kararı + kurulum. Windows ve macOS koşucularında `rt_typecheck` + smoke (CI zaten var). (D)

Kullanıcıdan gereken: Android cihazlar (2), Windows ve macOS makineleri ya da CI koşucusu, çevrimiçi test hesabı/sunucusu.

## 5. Gün 4: kalan Java, paket, kabul

Çıkış: bölüm 8 onay listesinin her kutusu ölçülmüş; `1.0-rc`.

1. WS7 bitiş: kalan `classlib/` ve `java_nolwjgl/` Java kaynağı sıfır ya da gerekçeli istisna listesi. JNI/JNA sayacı 0, `libffi` kalktı. (B)
2. Tam build, tam gate seti, 30 dk oynanış x her platform (kullanıcı cihazlarıyla), soak sonucu. (A, C)
3. WS11: paketler (tar/AppImage, zip+kurucu, .app/.dmg, APK), SHA-256 manifest, imza, güncelleme kontrolü, Python'sız launcher. "İndir → aç → oyna" temiz makinede. (D)
4. WS9: çökme raporlayıcı (panic kancası + sinyal işleyici); vanilla dünya yükle/kaydet; kaynak paketi, ayarlar. (A)
5. WS14: belgeler (kurulum, sorun giderme, yama yazma), eski belgeler arşive, `STATUS.md` güncel. (D)
6. WS13 hukuk kapısı: karar gerekir; verilmezse sürüm "özel test sürümü" kalır, herkese açık dağıtım yapılmaz. (kullanıcı)

## 6. Onay listesi eşlemesi (MASTER_PLAN §8)

| Kutu | Gün | Doğrulama |
|---|---|---|
| 4 platformda 30 dk oynanış | 1 (Linux), 3–4 (diğer) | `headless_play` + cihaz raporu |
| JVM/JNI/JNA/libffi yok, Java kaynağı yok | 2–4 | sayaç raporu + `find classlib runtime/java_nolwjgl -name '*.java'` |
| GC <1 ms ya da kapalı + soak <1 MiB/dk | 2 başla, 3–4 bitir | `leak_budget.py` |
| Ses tam, TLS | 3 | underrun sayacı, sunucuya katılma |
| Menü <15 sn, kare/RSS ≥ HotSpot | 1 ölç, 2–4 iyileştir | ölçü tablosu |
| Vanilla dünya + kaynak paketi | 4 | elle + betik |
| Fark testleri yeşil | her gün | `tests/run_tests.py` |
| Paketler, imza, temiz makine | 4 | temiz VM |
| Launcher Python'sız, güncelleme | 4 | paket testi |
| Belgeler, çökme raporlayıcı | 4 | gözden geçirme |
| Hukuk kapısı | 4 | kullanıcı kararı |

## 7. Riskler (planı bozabilecekler)

1. **Bellek ölçülmeden hedef kaybolur**: 13,3 GB RSS (`MONO_GC=off`) ve 1,1 s duraklama bugün ölçüm. Gün 2'de tier 1–3 işlemezse <1 ms hedefi tutmaz; yedek: concurrent GC varsayılan + duraklama ölçüsü belgelenir.
2. **Build maliyeti**: bir hatalı çevirici değişikliği 65 dk. Çevirici değişiklikleri küçük birimlerle `stack_tests`/`patch_tests` ile önce doğrulanır.
3. **Donanım erişimi**: Android/Windows/macOS/gerçek GPU kullanıcıda. Gün 3'te yoksa o kutular 4 günde kapanmaz; açıkça "doğrulanmadı" yazılır, sahte başarı yok.
4. **TLS**: JSSE çevirisi ağır ve kırılgan (plan WS5). rustls yaması yedeği Gün 3'te hazır tutulur.
5. **Gölgelendirici derleme (naga)**: Minecraft'ın shaderc/SPIR-V yolu `renderpearl` içinde; `naga` ile GLSL 330/ES çıktısı tutmazsa Android render açılmaz. Gün 3 öğleden önce masaüstünde kanıtlanır.
6. **Hukuk**: WS13 kapalı kalırsa dağıtım "özel test".

## 8. Günlük kapanış

Her gün sonunda: gate seti, `docs/STATUS.md`'ye tarihli bölüm (yalnız ölçülen), commit + push. Çalıştırılmayan gate "çalıştırılmadı" yazılır.
