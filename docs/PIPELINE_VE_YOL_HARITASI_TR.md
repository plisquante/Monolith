# Pipeline ve yol haritası (basit Türkçe)

Bu belge iki şeyi anlatır: (1) Monolith'in Java'dan çalışan oyuna giden yolu, yani "pipeline", (2) bundan sonra yapılacaklar, yani yol haritası.
Ayrıntılı ve bağlayıcı belgeler: [`mimari/00_ANA_MIMARI.md`](mimari/00_ANA_MIMARI.md), [`mimari/06_YOL_HARITASI.md`](mimari/06_YOL_HARITASI.md), [`GC_PLAN.md`](GC_PLAN.md).

## 1. Pipeline: Java'dan çalışan oyuna

Fikir basit: Minecraft Java bytecode'u olarak gelir. Biz onu önceden Rust'a çeviririz, Rust'ı derleriz ve ortaya JVM gerektirmeyen tek bir program çıkar.

| Adım | Ne olur | Nerede |
|---|---|---|
| 1. Jar'ı al | Minecraft istemci jar'ı indirilir (`client.jar`). | `tools/fetch_client_jar.py` |
| 2. Sınıfları oku | Jar'daki ve kütüphanelerdeki `.class` dosyaları okunur (yaklaşık 60 bin sınıf, 286 bin metot). | `monolith/classfile`, `monolith/world.py` |
| 3. Ulaşılabilirlik analizi (RTA) | Hangi metotların gerçekten çağrılabileceği bulunur. Çağrılamayanlar atılır, böylece program küçülür. | `monolith/world.py` |
| 4. Rust üret | Her metot Rust koduna çevrilir. Kod 16 parçaya (shard) bölünür. | `monolith/rustgen.py` |
| 5. Yamaları uygula | El yazması hızlı Rust kodları, çevrilmiş metotların yerine konur (şu an 22 yama). | `patches/rust/manifest.json`, `monolith/patches.py` |
| 6. Rust'ı derle | Çalışma zamanı kütüphanesi (`monolith/rt`) ve 16 shard `rustc` ile derlenir. Aynı kaynak daha önce derlendiyse önbellekten alınır. | `tools/mcbuild_rust.py` |
| 7. Paketle | Çalıştırılabilir program, kaynak dosyalar ve `run.sh` çıktı klasörüne konur. | `build/minecraft-26.3/` |
| 8. Oyunu dene | Ekransız (Xvfb) ortamda oyun açılır, dünya oluşturulur, yürünür, temiz kapatılır. | `tools/headless_play.sh` |

Komutlar:

- Tam derleme: `./build.sh build/client.jar -o build/minecraft-26.3 --opt 1 --shards 16 --shard-jobs 2 --reuse-objs`
- Oyun testi: `MC_GRAPHICS=opengl LIBGL_ALWAYS_SOFTWARE=1 xvfb-run -a -s "-screen 0 1280x720x24" sh tools/headless_play.sh build/minecraft-26.3 --seconds 60 --clean-exit`
- Link hatası kapısı: `MONO_VERBOSE=1` ile derle, hata çıktısını `python3 tools/check_link_errors.py <dosya>` ile denetle.

Bilmen gerekenler:

- `--reuse-objs`: Üretilen Rust kaynağı birebir aynıysa eski derleme çıktısı yeniden kullanılır. Derleme çok hızlanır (ilk derleme yaklaşık 95 dk, önbellekli derleme çok daha kısa).
- Bellek: Her `rustc` yaklaşık 5 GB ister. 16 GB'lık makinede `--shard-jobs 2` güvenlidir. Derleme ile test paketi aynı anda çalıştırılmaz.
- Link hatası: Bir çağrı, sınıf yolunda olmayan bir sınıfa gidiyorsa "link hatası" sayılır. Şu an 444 tane var ve hepsi açıklanmış (log4j eklentileri, netty log4j, commons-compress, LambdaForm, macOS sınıfları). Yeni bir hata çıkarsa kapı kırmızı olur.
- Ortam: Oyunun GLSL derleyebilmesi için sistemde `libshaderc1` ve `libspirv-cross-c-shared0` kurulu olmalı. Yoksa oyun "Couldn't parse GLSL" ile açılışta çöker.

## 2. Pipeline'ın 25 Kasım 2026 durumu

| Kapı | Sonuç |
|---|---|
| Tam Minecraft 26.3 derlemesi | Geçti (5 koşu, hepsi çıkış 0) |
| Oyun testi (60 sn, temiz kapanış, dünya kaydı) | Geçti |
| Link hatası kapısı | Geçti: 444 hata, açıklanmamış 0 |
| `MONO_PATCH_VERIFY=1` ile oyun (60 sn) | Çökme yok, "patch mismatch" yok. Doğrulama modu yavaş olduğu için betiğin tıklamaları geç kaldı, dünya kaydı doğrulanamadı; bu koşu yalnızca "uyuşmazlık yok" kanıtıdır. Not: bu mod yalnızca ilkel değer döndüren yamaları denetler, sysinfo/narrator/netty bunun dışındadır. |
| Tracy yamaları (33 yama) | Çıkarıldı: oyunu açılışta çökertiyordu. Yeniden eklemek için `patches/rust/tracy.rs` başındaki nota bak. |
| Sızıntı soak'ı (60 dk, GC açık) | Oyun 3600 sn çökmeden çalıştı. RSS: 10. dakikada 4,4 GB, 30. dakikada 4,9 GB, 60. dakikada 5,0 GB. 15. dakikadan sonra eğim yaklaşık 6,6 MB/dk: bütçe (1 MiB/dk altı) AŞILDI, yavaş büyüme sürüyor. Çıkışta dünya kaydı doğrulanamadı (betik zamanlaması). Neden araştırılacak. |
| Windows, Android, gerçek GPU | Bu ortamda denenemez |

## 3. Yol haritası

Sıra: önce güvenilirlik, sonra bellek, sonra platformlar.

| Sıra | İş | Neden | Bittiğini nasıl anlarız |
|---|---|---|---|
| 1 | Test edilmemiş yamaları oyunda doğrula (sysinfo, narrator, netty) | Açılış testinden geçtiler ama ayrıntılı denenmediler. | Her grup tek tek kapatılıp açılarak davranış karşılaştırılır. |
| 2 | Tracy yamalarını doğru kurup geri getir | Tasarım eksikti: `RTA` bazı sınıfları ve metotları atıyordu. Düzeltme: `news` bağlaması ve `init_X()?` çağrıları. | Oyun açılır, Tracy çağrıları çalışır. |
| 3 | 60 dakikalık sızıntı soak'ı | Uzun oturumda belleğin sabit kaldığı kanıtlanmadı. | Büyüme 1 MiB/dk altında. |
| 4 | GC planı faz 0-3 (`GC_PLAN.md`): ölçüm araçları, yazma bariyeri, eşzamanlı işaretleyici | Şu anki GC durdur-ve-süpür; oyunda takılma yapar. | Duraklama 1 ms altında, bariyer maliyeti %3 altında. |
| 5 | GC planı faz 4-5: çöp azaltma ve sahipli bellek (palet/chunk) | GC'yi kapatabilmek için çöp üretimini düşürmek gerekir. | Kararlı durum tahsisi 5 MiB/sn altında. |
| 6 | GC planı faz 6: `MONO_GC=off` soak'ı | İlk deneme 10 dakikada 13,3 GB'a çıktı. Büyüme hâlâ çok yüksek. | Kalan büyüme 1 MiB/dk altında. Sonra GC varsayılan kapalı, güvenlik ağı olarak kalır. |
| 7 | Derleyici kazançları (MIR, satır içi, kaçış analizi; `06_YOL_HARITASI.md` C1-C9) | Hız ve bellek: tahsis oranını düşürür. | `mir_fallback == 0`, tahsis oranı hedefte. |
| 8 | Kütüphane temizliği (JNA, JNI, LWJGL Java dosyalarını silme) | Java mantığını sıfıra indirir. | Envanterde sınıflanmamış kalem 0. |
| 9 | Platformlar: Android `.so` ve APK, Windows | Gerçek cihaz testi gerekir. | Her biri sızıntı betiğini çalıştırır. |
| 10 | C dosyalarının yorum çevirisi | Yorumların tamamı Türkçe olsun. | Taramada İngilizce yorum yok. |

Önemli nokta: GC kaldırma 4, 5 ve 6. adımların birleşik adıdır. Doğrudan kapatılamaz, çünkü önce çöp üretimi ve sahipli bellek hazır olmalı.

## 4. Bu belgeyi güncel tutmak

- Bir kapı geçildiğinde 2. bölümü, bir iş bittiğinde 3. bölümü güncelle.
- Ayrıntılı kronoloji `STATUS.md` içindedir, buraya taşınmaz.

## 5. Güncel tahsis profili (25 Kasım 2026, GC açık, 242 sn oyun)

- Toplam 11.950 MiB tahsis, saniyede 49,5 MiB (hedef: 5 MiB/sn altı). %84,5'i ömrü kısa (TEMP). 29 toplama, toplam duraklama 16 sn, en uzun 1,5 sn.
- En büyük çöp sınıfları: `[C` 1.284 MiB, `[I` 1.110 MiB, `Object[]` 755 MiB, `[J` 590 MiB, `[B` 584 MiB, `DataResult$Success` 506 MiB, `Optional` 478 MiB, `ArrayList$Itr` 295 MiB, `Vec3` 278 MiB, `AABB` 233 MiB.
- En büyük tek nokta: `PalettedContainer.pack` (yonga kaydı/gönderimi) `int[]` ayırıyor, örneklerin %7,6'sı.
- Kolay yol yok: çöpün çoğu Codec (`DataResult`, `Optional`, `Pair`) ve fizik (`Vec3`, `AABB`) kodundan geliyor, tek tek yama yerine derleyicinin kaçış analizi (`monolith/escape.py`, çerçeve ve arena katmanları) bu sınıfları kapsayacak şekilde genişletilmeli. Geri dönüştürülen yineleyiciler zaten %6,5'i yığın dışında karşılıyor.
- Sonuç: GC şimdilik kapatılamaz. Kapatma kapısı (`MONO_GC=off` ile 60 dk, büyüme 1 MiB/dk altı) geçilmedi.

## 6. Kaçış analizi ölçümü (25 Kasım 2026, `--escape --inline`, yalnızca analiz)

- 243.594 tahsis noktasının yalnızca 17.608'i (%7,2) metottan dışarı kaçmıyor; döngü dışında kullanılabilir olan 16.153.
- Kaçış nedenleri: başka metoda argüman olarak verilmiş 107.443, `areturn` 59.327, `via owner` 28.606, çözülemeyen çağrı 14.362, `putfield` 11.146.
- `Vec3` ve `AABB` fabrikalarının hepsi "taze nesne döndürür" (`ret_fresh`) olarak işaretli, ama çağıranlar sonucu çoğunlukla başka bir metoda verir ya da alana yazar. Taze nesne alan 76.486 çağrının yalnızca 5.386'sında çağıran tarafta yığına alınabilir.
- Sonuç: yalnızca derleyici kaçış analiziyle tahsisin küçük bir bölümü kalkar; `Vec3`, `AABB`, `Optional`, `DataResult$Success` çöpünün çoğu kısa ömürlü ama gerçekten metot dışına çıkıyor. Bunlar için kapsam (bölge) tabanlı tahsis ya da skaler değiştirme gerekir (`GC_PLAN.md` kademe 2-3). GC kapatma kapısı hâlâ açık.

## 7. Sızıntı ayrımı (25 Kasım 2026, 15 dk koşu, `MONO_GC_PHASES=1`)

- Yönetilen yığında toplama sonrası canlı veri: ilk toplamalarda hızlı artış (dünya yükleme), 29. toplamadan sonra 555 MiB'de düz kalıyor (29-42. toplamalar: 554-556 MiB), 43-44. toplamada 573-581 MiB.
- Yani yönetilen yığında sızıntı görünmüyor. Soak'taki yaklaşık 6,6 MB/dk büyümenin kaynağı yönetilen yığın dışı: yazılım GL (llvmpipe), doku/ara bellek ya da bölünme. Bu ortamda ayırt edilemez; gerçek GPU'da ayrıca ölçülmeli.
- Asıl ürün sorunu duraklama: 555 MiB canlı veride her durdur-ve-süpür toplama yaklaşık 1,4 sn sürüyor. Bütçe 1 ms altı. Bu yüzden eşzamanlı işaretleyici (`GC_PLAN.md` faz 1-3) ve sahipli bellek (yonga/palet) öncelikli.

## 8. Eşzamanlı işaretleyici ölçümü (25 Kasım 2026, `MONO_GC_CONCURRENT=1`, 240 sn)

- Çalışma zamanında SATB eşzamanlı işaretleyici zaten var ve açılabiliyor. Oyun 240 sn çökmeden çalıştı, 52 toplama.
- Duraklama dağılımı (canlı yaklaşık 600 MiB): kök tarama 7-9 ms, işaretleme duraklaması yaklaşık 1 ms (eskiden yaklaşık 1000 ms), süpürme 260-335 ms. Toplam duraklama yaklaşık 270-350 ms (eskiden yaklaşık 1400 ms).
- Kalan duraklamanın tamamı süpürmeden geliyor: `monolith/rt/gc.rs` toplamanın 5. ve 6. adımları durdur-ve-süpür içinde dilimleri tek tek tarıyor. Sıradaki iş tembel/eşzamanlı süpürme (epoch işaret biti). Hedef 1 ms altı.
- Bu varsayılan değil: doğrulama modu ve 30 dk'lık temiz koşu (`GC_PLAN.md` faz 3 ölçütü) yapılmadan açılmaz.

## 9. Paralel süpürme (25 Kasım 2026)

- `monolith/rt/gc.rs`: sayım (profil) kapalıyken yığın dilimleri 4 iş parçacığıyla paralel süpürülüyor (`sweep_slab_nocensus`). Dilimler birbirinden bağımsız, dünya zaten durmuş. Yalnızca çalışma zamanı değişti; `--resume --reuse-objs` ile yeniden derleme birkaç dakika sürdü.
- Doğrulama: `GcStress` testi geçti (1/1). `MONO_GC_CONCURRENT=1` ile 240 sn oyun: çökme yok, 53 toplama. Süpürme 260-335 ms'den en çok 110-120 ms'ye indi; son toplamalarda toplam duraklama 150-160 ms (önce 270-350 ms). Dünya kaydı doğrulaması bu betikte hep başarısız (bilinen betik sorunu), bu yüzden kayıt kapsanmadı.
- Hâlâ hedefin (1 ms altı) çok üstünde. Kalan: tembel/eşzamanlı süpürme (epoch bit) ve kök tarama (7 ms).

## 10. Tembel süpürme tasarımı (henüz kodlanmadı)

Hedef: kalan yaklaşık 110 ms süpürme duraklamasını durdur-dünya dışına almak. `gc.rs` okunarak çıkarılan plan:

1. İşaret biti kutbu her döngüde döner (`G.sense` = 0 ya da `GC_MARK`). "İşaretli" demek `gc & GC_MARK == sense`. Süpürme artık biti silmez. Yeni nesneler (`new_obj_bits`) her zaman `GC_HEAP | sense` alır: hem sonraki döngüde otomatik "işaretsiz" olur, hem eşzamanlı döngüde "siyah" doğar.
2. Kutup, döngünün en başında bir kez döner: tek aşamalı toplamada `collect_impl` başında, eşzamanlı toplamada `begin_only` yolunda (bitiş aşaması `collect_impl(false)` tekrar döndürmez). Güncellenecek işaret bakış noktaları: `gc.rs` satır 264, 273, 943, 955, 1346 ve süpürme.
3. Döngü sonunda dilimler süpürülmez; `h.unswept[sc]` listelerine konur. `alloc_small_slow` önce `partial`, sonra `unswept` listesinden bir dilim alıp kilit altında süpürür (`sweep_slab_nocensus`). Süpürülmemiş dilime tahsis yapılmaz.
4. Sonraki döngünün başında, kutup dönmeden önce, kalan süpürülmemiş dilimler paralel süpürülür (en kötü durum bugünkü duraklama).
5. Açık sorular: `live_bytes` ve `gc_threshold` bir döngü gecikmeli hesaplanır; boş dilimlerin işletim sistemine geri verilmesi (şu an `to_dealloc`) ertelenmiş süpürmede ayrıca ele alınmalı; büyük nesneler (`h.large`) için aynı kutup kuralı.
6. Doğrulama: `GcStress`, `MONO_GC_POISON=1`, `MONO_GC_CONCURRENT=1` ile 30 dk oyun, duraklama ölçümü.

## 11. Tembel süpürme uygulandı (25 Kasım 2026, `MONO_GC_LAZY=1`, varsayılan kapalı)

- `monolith/rt/gc.rs`: işaret biti kutbu her döngüde döner (`SENSE`), süpürme tahsis sırasında dilim dilim yapılır (`lazy_sweep_one`), yeni döngünün başında kalan dilimler paralel bitirilir (`finish_lazy_sweep`). Canlı boyut işaretleme sırasında sayılır (`MARKED_BYTES`).
- Öğrenilen iki ders: (1) Kutup, `G.finishing` ile değil "yeni işaretleme başlıyor" (`begin_only || !G.cycle`) ile döndürülmeli. (2) `Globals` yapısına alan eklemek `--reuse-objs` ile yeniden kullanılan shard nesnelerini bozar, çünkü satır içi yazma bariyerleri `Globals` düzenini gömer. Yeni durum ayrı statiklerde tutulur. Bu ilk denemede oyunu çökertti, düzeltildi.
- Ölçüm (120 sn oyun, aynı derleme, yazılım GL):

| kip | toplama | ort. süpürme | ort. toplam duraklama | en uzun |
|---|---:|---:|---:|---:|
| varsayılan (durdur-süpür) | 29 | 34 ms | 473 ms | 1243 ms |
| `MONO_GC_CONCURRENT=1` | 23 | 47 ms | 64 ms | 170 ms |
| `MONO_GC_LAZY=1` | 29 | 3 ms | 470 ms | 1198 ms |
| `MONO_GC_LAZY=1 MONO_GC_CONCURRENT=1` | 26 | 4 ms | 16 ms | 35 ms |

- Hedef 1 ms altı hâlâ geçilmedi (kök tarama yaklaşık 7 ms). `GcStress`, `MonitorChurn`, `Threads` testleri `MONO_GC_MB=1` ile tembel+eşzamanlı+zehirli kipte geçti.
- Varsayılan yapılması için `GC_PLAN.md` faz 3 ölçütü gerekir: doğrulama modunda 30 dk temiz koşu ve bariyer maliyeti ölçümü. Henüz yapılmadı.

## 12. 30 dk doğrulama koşusu (25 Kasım 2026, tembel+eşzamanlı, zehir ve arena denetimi açık)

- Kip: `MONO_GC_LAZY=1 MONO_GC_CONCURRENT=1 MONO_GC_POISON=1 MONO_ARENA_CHECK=abort`, 1800 sn betikli oyun, yazılım GL.
- Sonuç: oyun 1800 sn çökmeden çalıştı ("game still running after 1800 s: OK"); yeni çökme raporu ve yerel çökme dosyası yok; arena ihlali ve `panic`/`SIGSEGV` yok. 56 toplama döngüsü, ortalama duraklama 19 ms, en uzun 62 ms (canlı yaklaşık 154 MiB). Faz 3'ün "30 dk doğrulama modu temiz" ölçütü karşılandı; duraklama ölçütü (1 ms altı) ve bariyer maliyeti ölçütü (%3 altı) karşılanmadı/ölçülmedi.
- RSS: 4,5 GB (15. dk) ve 5,1 GB (30. dk), zehir dolgusu belleği şişirdiği için önceki soak ile doğrudan karşılaştırılamaz.
- Dünya kaydı yine betikte doğrulanamadı (bilinen betik sorunu); kayıt yolu bu koşuda kapsanmadı.
- Karar: tembel+eşzamanlı kip varsayılan yapılmadı. Önce kök taramayı (yaklaşık 7 ms) ve duraklamayı 1 ms altına indirme işi ile bariyer maliyeti ölçümü yapılmalı.

## 13. Kök tarama ölçümü (25 Kasım 2026)

- `MONO_GC_PHASES=1` artık `[mono:gc_roots]` satırı da yazıyor. Ölçüm (60.630 sınıf, 34 bin statik yuva, 28 bin sabitlenmiş referans, 22-24 iş parçacığı): iş parçacığı yığınları yaklaşık 0,3 ms; sınıf aynası ve statik döngüsü yaklaşık 6 ms (seri); geri kalan statik yuvalar ve sabitlenmiş referanslar yaklaşık 2,7 ms.
- Sınıf döngüsü 4 iş parçacığına bölündü (`gc.rs`, dünya durmuşken salt okunur). Ortalama sınıf döngüsü yaklaşık 2,6 ms; kök toplamı yaklaşık 5,7 ms'den yaklaşık 4-5 ms'ye indi. `GcStress`, `MonitorChurn`, `Threads` geçti; 90 sn tembel+eşzamanlı oyun çökmeden çalıştı.
- Kalan seri kısım (`static_slots`, `pinned_refs`, yaklaşık 62 bin giriş) benzer şekilde bölünebilir; ama 4 çekirdekli bu ortamda kök taramasını 1 ms altına indirmek için bu yetmez. 1 ms hedefi, kökleri durdur-dünya dışında (örneğin statikleri yazma bariyeriyle izleyerek) taramayı gerektirir.
- Deneme: `static_slots` ve `pinned_refs` döngüleri de paralelleştirildi; kalan seri kısım yalnızca yaklaşık 2,7 ms'den 2,1 ms'ye indi (kök toplamı yaklaşık 4,6 ms), kazanç gürültü düzeyinde olduğu için geri alındı. Kök taramasını 1 ms altına indirmenin yolu paralellik değil, taramayı durdur-dünya dışına almaktır.

## 14. `MONO_GC=off` ölçümü, güncel derleme (25 Kasım 2026)

- Betikli oyun, `MONO_GC=off`, yazılım GL, 16 GB makine. RSS: 11:24:04'te 1,1 GB, 11:24:44'te 3,5 GB, 11:25:34'te 6,3 GB, 11:26:54'te 11,3 GB. Güvenlik için 11 GB'ı geçince süreç öldürüldü (yaklaşık 3 dk). Büyüme yaklaşık 3,6 GB/dk, yani yaklaşık 60 MB/sn.
- Sonuç: GC kapalıyken bellek tüketimi 60 dk soak bütçesinin (1 MiB/dk altı) yaklaşık 3.600 katı. Daha önceki ölçümle (10 dk'da 13,3 GB) ve tahsis profiliyle (saniyede 49,5 MiB) tutarlı. GC varsayılan olarak kapatılamaz; önce tahsis oranı yaklaşık 10 kat azalmalıdır (`GC_PLAN.md` faz 4-5).

## 15. Değerlendirme: "GC'yi kaldırmak" gerçekçi mi? (25 Kasım 2026)

Bu bölüm ölçümlerden çıkan bir yorumdur, karar kullanıcıya aittir.

- Ölçülen gerçekler: tahsis saniyede 49,5 MiB; GC kapalıyken büyüme yaklaşık 3,6 GB/dk; kaçış analizi tahsis noktalarının yalnızca %7,2'sini yığına alabiliyor; çöpün büyük kısmı kısa ömürlü ama metot dışına çıkıyor (Codec ve fizik nesneleri `putfield`, `areturn`, `arg` ile akıyor).
- Bölge (tick) tabanlı tahsis, nesnelerin tick sonunda toplu bırakılabileceğini kanıtlamayı ya da kaçanları tick sonunda yığına taşımayı gerektirir. Taşıma (kopyalama) için kesin (precise) yığın haritaları gerekir; mevcut toplayıcı yığını tutucu (conservative) taradığı için nesneleri taşıyamaz. Kanıtlama ise kaçış analizinin ulaşamadığı yerdir. Yani bölge tabanlı tahsis, bugünkü mimaride güvenli biçimde yapılamaz; önce kesin yığın haritaları ya da Rust'ta sahiplik modeli (`GC_PLAN.md` kademe 3, faz 5) gerekir.
- Önerilen hedef revizyonu: "GC sıfır" yerine "GC duraklaması bütçe altında". Tembel + eşzamanlı toplayıcı bugün ortalama 16-25 ms, en uzun 35-217 ms duraklama veriyor (yazılım GL). 60 FPS için bütçe kare başına yaklaşık 2 ms'nin altında olmalı. Kalan iş: kökleri durdur-dünya dışına almak (statikler için yazma bariyeri) ve bariyer maliyetini (%3 altı) ölçmek. Bu, sızıntı bütçesinden (`MONO_GC=off`) çok daha ulaşılabilir ve ürün için aynı kullanıcı etkisini (takılmama) verir.
- `MONO_GC=off` yolu, sahipli bellek ve kesin yığın haritaları tamamlanana kadar kapalı kalır.

## 16. Yol 1 ön çalışması: değer benzeri sınıflar (25 Kasım 2026)

Amaç: Vec3, AABB, Optional, DataResult$Success, Pair gibi sınıfların tahsisini azaltmak. Bölüm 15'teki "bölge + kaçışta kopyalama" tasarımı bu sınıflar için denenecek. Aşağıdakiler ölçüm ve okumadır, kod yazılmadı.

- Değişmezlik (bayt kodundan okundu): `AABB` (altı alan `final`), `Pair` (`first`, `second` `final`) ve `DataResult$Success` (`record`, `final` sınıf) uygun. `BlockPos` uygun değil: `Vec3i` alanları (`x`, `y`, `z`) `final` değil ve `BlockPos.MutableBlockPos` onları değiştiriyor; bu sınıf aday listesinden çıkarıldı. `Vec3` ve `Optional` için alan bayrakları ayrıca doğrulanacak.
- Kaçış analizi, çağıran taraf ayrıntısı (`--escape --inline`, yalnızca analiz): Vec3, AABB, Optional, DataResult, Pair, Vector3f, BlockPos fabrikalarının sonucunu alan 4.049 statik çağrı noktasından 493'ü çağıranda yığına alınabilir (LIFTABLE). Kalanların nedenleri: argüman olarak başka metoda verilmiş 2.621 (en çok `ImmutableList.of` 692, `Applicative.ap*` yaklaşık 670, `List.of` 116, `ServerLevel.gameEvent` 145), `via owner` 570, `areturn` 143, çözülemeyen çağrı 106, `putfield` 59. Bunlar statik sayımdır, çalışma sıklığıyla ağırlıklandırılmamıştır.
- Tahsis profili: bu sınıfların tahsisi tek bir sıcak noktada toplanmıyor. En çok tahsis yapan ilk 400 noktada Vec3 yalnızca yaklaşık 41 MB, AABB yaklaşık 85 MB, Optional yaklaşık 161 MB, DataResult$Success yaklaşık 131 MB (toplam 12 GB içinde). Yani uzun bir kuyruk; tek tek yama yerine genel bir mekanizma (bekçili bölge) gerekir. Not: `profile_sites.py` çağrı yığını adları güvenilir değil (çerçeve işaretçisiz yığınlar), bu yüzden hangi metodun tahsis ettiği buradan kesin okunamaz.
- Sonuç: yol 1, tek tek yama ile değil, bölge (arena) + referans yazımlarında kopyalama bekçisi + kimliksizlik kanıtı ile yapılabilir. Ön koşullar: (a) aday sınıfların kimliğinin (`==`, `synchronized`, `identityHashCode`) kullanılmadığının tüm program üzerinde kanıtı, (b) kod üretiminde alan/dizi/statik/dönüş/lambda yakalama noktalarına bekçi, (c) derin kopyalama, (d) bekçi maliyeti ölçümü (hedef %3 altı). Her biri kod üretimine dokunur; tam yeniden derleme (yaklaşık 95 dk) gerektirir.

## 17. Yol 1: kimlik kanıtının sınırı (25 Kasım 2026)

Kimlik kullanımını (`if_acmpeq/ne`, `monitorenter`, `identityHashCode`) kanıtlamak için `escape.py` yeterli değil: orası yalnızca metot içinde `new` ile doğan değerleri izliyor, statik tür taşımıyor. Kanıt için bir tür benzetimi gerekir (`stackmap.frames` dallanma noktalarında doğrulayıcı türlerini veriyor, aradaki doğrusal kod benzetilebilir; `ARITH/CONV/IF1/IF2` tabloları `escape.py`'den yeniden kullanılabilir).

Asıl sorun analizden önce gelir: `Object` türünde `==` her yerde (koleksiyonlar, `HashMap.getNode`...). Bunların bir kısmı yalnızca hızlı yoldur (`a == b || a.equals(b)`), ama bir kısmı kimlik anlamı taşır. İki yol var:

1. **Yalnızca kanıtlananlar:** Kimlik kullanımı hiç görülmeyen sınıflara dokunmak. `Object` türünde `==` olan her yerde aday sınıf kanıtlanamaz; pratikte hiçbir sınıf kanıtlanamaz.
2. **Değer eşitliği (Valhalla gibi):** Aday sınıfların örnekleri için `==` alan alan karşılaştırır (`rt::ref_eq` ile). Böylece asıl nesne ile kopyası "aynı" sayılır. Sonuç: iki ayrı ama alanları eşit `new Vec3(0,0,0)` Java'da `!=` iken burada `==` olur. Kodun buna dayanıp dayanmadığı elle ya da testle doğrulanmalı. Maliyet: her referans karşılaştırmasında sınıf bakışı.

Bu yüzden yol 1 yalnızca bir analiz değil, dil anlamını değiştiren bir karardır (`GC_PLAN.md`'nin ve davranış eşdeğerliği hedefinin dışına çıkar). Karar kullanıcıdadır; verilmeden kod üretimine dokunulmayacak.

## 18. Yol 1 tasarımı: bölge + kopyalama bekçisi + iletme işaretçisi (henüz kodlanmadı)

§17'deki kimlik sorununun çözümü: değer eşitliği gerekmez, kimlik **iletme (forwarding) işaretçisiyle** korunur. Java davranışı değişmez. Aşağıdaki tasarım `gc.rs`, `rt.rs` ve `rustgen.py` okunarak çıkarıldı; hiçbiri uygulanmadı.

**Fikir.** Seçilen sınıflar (önce `Vec3`, `AABB`, `Pair`, `DataResult$Success`, `Optional`) yığına değil, iş parçacığına özel bir bölgeye (bump arena) tahsis edilir. Bölge, sanal adres olarak tek bir ayrılmış aralıktır (ör. 32 GiB, `mmap` ile, dokunulmadıkça bellek yemez); bir adresin bölgede olup olmadığı iki karşılaştırmayla anlaşılır.

**Güvenlik kuralı (heap'ten bölgeye referans yok).** Bölge nesnesi uzun yaşayan bir yere yazılacağı anda kopyalanır:
- Referans yazımları zaten tek noktadan geçiyor: alan ve statik için `gc::satb_store`, dizi için `rt::aastore_ref` (`rustgen.py` satır 530, 554, 622). Bunların bölge farkında kopyaları (`_r`) üretilir; bölge kapalıyken üretilen kod değişmez.
- Bölge nesnesi yazılacaksa heap'e kopyalanır (derin: içindeki bölge referansları da), kopyanın adresi yazılır ve asıl nesnenin başlığına iletme adresi konur. Aynı nesne ikinci kez yazılırsa aynı kopya kullanılır.
- Kimlik: `if_acmpeq/ne`, `monitorenter/exit`, `identityHashCode`, varsayılan `hashCode` önce iletmeyi izleyip (`canon`) kopyaya çevirir. Böylece asıl ve kopya her zaman "aynı nesne" sayılır. `Obj` başlığındaki `hash` alanı kopyayla taşınır.
- Ek yazım yolları kapatılmalı: `System.arraycopy` (kaynak çerçeve ya da arena dizisi ise), Unsafe/VarHandle/yansıma yazımları (`rt.rs` satır 307, 759, `vh.rs`), lambda yakalama (`indy.py`), yerel metotlar. Her biri bir testle doğrulanır.

**Uygunluk.** Sınıf değişmez olmalı (tüm alanlar `final`, kurucudan sonra yazım yok) ve kurucu `this`'i dışarı kaçırmamalı (`aload_0` yalnızca `putfield` alıcısı ve üst `<init>` çağrısında). Bayt kodundan denetlenir. `BlockPos` uygun değil (`Vec3i` alanları değişebilir).

**Bölge toplama (küçük toplama).** Bölge parçalara (64 KiB) bölünür. Belirli aralıklarla (ör. her 64 MiB bölge tahsisinde) kısa bir durdur-dünya yapılır: yalnızca iş parçacığı yığınları, arenaları ve `recycled` listeleri taranır (ölçüm: yığınlar yaklaşık 0,3 ms). Bir yığın kelimesi bölge nesnesini gösteriyorsa o nesnenin parçası canlı sayılır ve nesne içindeki bölge referansları izlenir. Hiçbir canlı nesne göstermeyen parçalar topluca serbest bırakılır. Heap'ten bölgeye referans olmadığı için kök kümesi budur. Beklenen duraklama yaklaşık 1 ms.

**Ana toplayıcıyla ilişki.** Bölge nesneleri heap toplayıcısınca süpürülmez ama içinden geçilir: bir bölge nesnesi `String` gibi bir heap nesnesine referans veriyorsa, ana işaretleme o nesneyi canlı tutmalıdır. Bu yüzden işaretleme bölge adreslerini de izler (işaret biti kutbu gerekir, yani tembel kip: §11). Ana işaretleme sürerken bölge toplama parça serbest bırakmaz.

**Maliyet ve kapsam.** Her referans yazımına ve karşılaştırmasına iki adres karşılaştırması eklenir (hedef: %3 altı, ölçülmedi). Tahsis payı tahminim: aday sınıflar toplam tahsisin yaklaşık %17'si. Gerçek kazanç, bölgede kalan (kopyalanmayan) oran ile ölçülecek. Kod üretimi, çalışma zamanı ve tam yeniden derleme (yaklaşık 95 dk) gerektirir; küçük Java test programlarıyla fark testi (`tests/run_tests.py`) ilk adım olmalıdır.
