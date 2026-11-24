# Bellek mimarisi planı: durdur-dünya GC'den boşta çalışan toplayıcıya, sonra toplayıcısız duruma

> **Not (2026-11-24):** hedef mimari artık `docs/mimari/00_ANA_MIMARI.md`; çelişen bölümler orada listelenir (bölüm 8).

Hedef (sahibinin sözleriyle): GC sıfır, sızıntı sıfır, görünür bir takılma asla olmasın. Yol: önce toplayıcıyı görünmez kılmak (her zaman açık, küçük, ayarlı), yaptığı işi azaltmak (daha az çöp, sahipli veri), ve ancak ölçülmüş bir sızıntı bütçesi güvenli dediğinde kapatmak. Aşağıdakilerden "bitti" denmeyenler henüz uygulanmadı.

Güncel ölçümler (tahsis ve kök noktaları, `Stack1` üzerinde kaçış analizi açık): `docs/GC_REMOVAL_INVENTORY.md`.

## 0. Bitmiş sayılma tanımı

| Ölçüt | Bugün | Hedef (boşta toplayıcı) | Son durum (toplayıcı kapalı) |
|---|---|---|---|
| Oyun iş parçacığındaki en uzun GC duraklaması | ölçülmedi (tüm heap üzerinde STW) | < 1 ms (yalnızca kök anlık görüntüsü) | yok |
| Yönetilen heap'e tahsis, kararlı durum | 36,5 MiB/sn (prof4, satır içi/kaçış derlemesinden önce) | < 5 MiB/sn | yalnızca artık |
| Mutator üzerindeki bariyer maliyeti | yok | < kare süresinin %3'ü | yok |
| Toplayıcı CPU'su | patlamalar halinde | bir çekirdeğin <= %10'u, kısılmış | 0 |
| Sızıntı bütçesi: kararlı durumda yönetilen canlı bayt büyümesi | bilinmiyor | ısınmadan sonra düz | 60 dk soak'ta < 1 MiB/dk |

Hedefler, sonuçlar değil. Her satırın faz 0'da bir ölçüm aracı olur ve sonraki her derlemede kontrol edilir.

## 1. Nerededeyiz (koddan gerçekler)

* `monolith/rt/gc.rs`: ayrık dilimli (segregated slab) tahsis edici (64 KiB dilim, 14 boyut sınıfı 16..2048 B, büyük nesneler bir haritada), iş parçacığına özel etkin dilimler, durdur-dünya işaretle-süpür, muhafazakâr yığın ve yazmaç taraması, `ref_offsets` ile kesin heap işaretleme, 32 MiB'de tetikleyici (`MONO_GC_MB`), sabitlenmiş referanslar, kayıtlı statik yuvalar.
* Yapıldı: kaçmayan nesnelerin çerçeve (yığın) tahsisi, kapsam bağlı yineleyici geri dönüşümü, DataFixer yaması (canlı heap -%61), satır içi + yineleme-yerel kaçış analizi, `tickChunk` yamasi, durum farkı içeren tahsis profilleyicisi.
* Yapılmadı: artımlı ya da eşzamanlı toplama, herhangi bir yazma bariyeri, sahipli (elle serbest bırakılan) nesneler.
* `MONO_GC=off` (faz 0 anahtarı): yapıldı. Otomatik toplamalar hiç çalışmaz; `System.gc()` yine toplar. Testler bununla geçiyor (`GcStress Oop Threads`). Oyunda ölçüldü: 16 GB'da 10 dakika hareketsiz beklerken 13,3 GB resident; durduruldu, toplayıcı kalır (`docs/VERIFICATION.md`).
* Sızıntı kapısı, ilk geçiş (toplayıcı açık): 60 dk hareketsiz, 1800 sn ısınmadan sonra eğim -0,054 MiB/dk, bütçe 1 MiB/dk, yazıldığı gibi geçiyor. Heap tüm koşuda 7,9'dan 593,5 MiB'a büyüdü, en uzun duraklama 1473 ms. Ayrıntılar `docs/VERIFICATION.md`'de.
* Faz 2 kanıtı (test düzeneği, oyun değil): `tests/run_tests.py GcStress Oop Threads`, `MONO_GC_MB=1 MONO_GC_POISON=1 MONO_ARENA_CHECK=abort` ile geçiyor; `GcStress Oop Threads Lambdas Strings Collections1` aynı ayarlarla ve ayrıca `MONO_GC_CONCURRENT=1` ile geçiyor. Bu koşular yalnızca izleyici toplayıcıyı ve eşzamanlı mod anahtarını çalıştırır; SATB işaretleyicisi ve tembel süpürme henüz yapılmadı.

## 2. Dört kademe

Her tahsis tam olarak bir kademeye düşer. Bir kademe kanıtla (analiz) ya da bir yamayla (ilan edilmiş sahip) seçilir, asla tahminle değil.

1. **Çerçeve**: metottan uzun yaşamaz. Yığın yuvası. Dönüşte serbest. (yapıldı; satır içi/kaçış kapsamı arttıkça büyüyor)
2. **Bölge**: bilinen bir kapsamın sonuna kadar yaşar (tick, kare, paket). Bump arena, tek adımda serbest. (arena var; kaçmayan diziler zaten metot başına `rt::ArenaGuard` ile kullanıyor; nesneler henüz kullanmıyor. Bir turu aşan döngü tahsisleri, arenaya girmeden önce tur başına ya da kapsam başına sıfırlama gerektirir; aksi hâlde uzun bir döngü, metot dönene kadar her turun nesnesini canlı tutar. tick/kare için kapsam işaretçileri hâlâ eklenecek; kaçış ihlali denetleyicisiyle doğrulanacak)
3. **Sahipli**: tanımlı bir serbest bırakma noktası olan tek bir sahibi var (yonga, bölüm, blok varlığı, varlık, GPU tarafı sarmalayıcılar). Süpürme kümesi dışında tahsis edilir, yine de içinden izlenir (yönetilen nesnelere referans tutar) ama asla süpürülmez; sahibi öldüğünde `rt::release` ile serbest bırakılır. Yamalar sahipliği ilan eder; hata ayıklama modu serbest bırakılan nesneleri zehirler ve karantinaya alır, böylece serbest bırakıldıktan sonra kullanımı yakalar.
4. **Yönetilen**: artık kalan kısım. Sızıntı bütçesi izin verene kadar boşta toplayıcı tarafından toplanır.

Kademe 1-3, kademe 4'ü küçültür. Kademe 4 güvenlik ağıdır ve bir soak testi geçene kadar derlemede kalır (`gc` özelliği).

## 3. Boşta toplayıcı (kademe 4)

Boşta bir motor gibi: hep biraz döner, kısılmıştır, yalnızca bellek baskısı arttığında daha fazla pay alır.

* **Arka plan iş parçacığında eşzamanlı işaretleme**, başlangıç anlık görüntüsüyle (snapshot-at-the-beginning, SATB). Oyun iş parçacıkları asla işaretlemez.
  * Bir döngünün başlangıcı: tek kısa durma (yalnızca kökler): yığınlar ve yazmaçlar (bugünkü gibi muhafazakâr), kayıtlı statikler, sabitlenmiş referanslar. Tek duraklama budur; yığın derinliğiyle orantılıdır, heap boyutuyla değil.
  * İşaretleme: atomik işaret bitleri (nesne bayraklarında dönüşümlü epoch biti, böylece temizleme geçişi gerekmez); işaretleyici iş parçacığında iş listesi.
  * Döngü sırasında tahsis edilen nesneler doğuştan işaretli gelir.
  * Yazma bariyeri (SATB): bir döngü aktifken, bir referans yazımı üzerine yazılan referansı iş parçacığına özel bir tampona iter. Döngü dışında bariyer, genel bir bayrağın tek yüklemesi ve alınmayan bir dal.
  * İşaretlemenin sonu: iş parçacığına özel tamponları boşaltmak için kısa bir el sıkışma (SATB altında yığınların yeniden taranmasına gerek yok).
* **Tembel süpürme**: süpürme duraklaması yok. Bir dilim, tahsis edici ondan yeniden doldurduğunda süpürülür (ve işaretleyici boşta olduğunda). Boş dilimler boş havuza döner; boyut sınıfı serbest listeleri dilim başına yeniden kurulur.
* **Ayarlama**: bir döngü, yönetilen canlı bayt `tetikleyici = max(taban, son_canlı * 1,25)` değerini aştığında başlar. İşaretleyici bir görev yüzdesiyle çalışır (iş parçacıkları arasında uyku/verim) ve bir çekirdeğin <= %10'unu hedefler. Tahsis işaretlemeden hızlı giderse (heap `soft_limit` üstünde), görev yüzdesi yükselir, ardından mutator'lar sınırlı parçalarla yardım eder (tahsis yavaş yolu başına <= 200 us). `hard_limit`'te bir kez tam durdur-dünya toplama çalışır, yüksek sesle kaydedilir ve sızıntı bütçesinde bir başarısızlık sayılır.
* **İş parçacığı modeli**: oyun iş parçacıkları bugünkü gibi kayıt olur. İşaretleyici, her iş parçacığının yığın aralığını yalnızca başlangıç anlık görüntüsü için ister.
* **Taşıma yok, sıkıştırma yok.** Dilimler boyut sınıflarını korur; parçalanma, kopyalamayla düzeltilmez, bir metrikle (dilim doluluk histogramı) izlenir.
* Nesilsel toplama planlanmadı. İkinci bir bariyer ve bir hatırlanan küme gerektirir; kademe 1-3'ten sonraki profil, buna değip değmeyeceğine karar verir.

## 4. Üretilen kod ve çalışma zamanı üzerinde bariyerin etkisi

Çevirici her referans yazımının sahibidir, bu yüzden bariyer sonradan yamalanmaz, üretilir.

* Yayıldığı yerler: referansların `putfield`/`aastore` komutları, referansların `putstatic` komutları, toplu kopyalar (`System.arraycopy`, `Arrays.copyOf` ailesi, `clone`) aralık bariyeri olarak.
* Çalışma zamanı, yerel ve yama kodu referansları ham yazımlarla yazmamalıdır. `rt::set_ref`/`rt::set_ref_array` tek API olur ve bariyeri taşır. `monolith/patch_tests.py` içindeki grep tabanlı denetim, bir yama ya da yerel kod bir referans alanını ham yazarsa derlemeyi başarısız kılar.
* Çerçeve/bölge nesnelerine ve o döngüde doğan nesnelere yapılan yazımlar bariyeri atlar (analiz sonucu); taze bir nesneye yazan kurucular da atlar.
* Bariyer üretilen kodu değiştirdiği için diğer kod üretimi değişiklikleriyle birlikte tek bir tam yeniden üretimde toplanır (bölüm 9).

## 5. Sahipli bellek (kademe 3)

* API: `rt::alloc_owned(class)`, `rt::release(ref)`; yamalarda sahip tipleri (manifestteki bağlamada `owner: "chunk"`), böylece serbest bırakma noktaları yamanın yanında ilan edilir.
* Beklenen kazanca göre sıralı aday sahipler (önce ölç, tahmin etme): yonga bölümleri ve paletler (`PalettedContainer`, `BitStorage`, ışık verisi), yonga/bölüm haritaları, varlık listeleri, blok varlıkları, paket tamponları, ağ (mesh) tamponları, doku/atlas verisi.
* Serbest bırakma noktaları: yonga boşaltma, varlık kaldırma, bölüm yeniden kurma, bağlantı kapatma.
* Güvenlik: `MONO_OWNED_CHECK=1` serbest bırakılan belleği zehirler ve N kare karantinada tutar; zehirlenmiş bir nesneye erişim, tahsis noktasıyla birlikte durdurur. Serbest bırakılan ama başka yerde hâlâ referanslı bir nesne ana risktir; denetim modu ve bir erişilebilirlik denetimi (serbest bırakıldıktan sonra hâlâ erişilebilen sahipli nesneleri bildiren bir hata ayıklama tam işaretlemesi) bunu kapsar.

## 6. Çöp azaltma birikimi (kademe 1-2), profile göre sıra

1. Satır içi + kaçış + `tickChunk` derlemesini ölçmeyi bitirin (sürüyor). Tam oyun derlemesinde ölçüldü: `--escape --inline` boşta tahsisi 19,0'dan 26,8 MB/sn'ye çıkardı; bu yüzden bayraklar isteğe bağlı kalır ve çözüm değildir (`docs/VERIFICATION.md`). Etkinleştirmeden önce açık: yalnızca satır içi alındığında görünen 6 JDK method-handle link hatası; ulaşılıp ulaşılmadığını kontrol edin.
   Güncel derlemede boşta profil (toplayıcı kapalı, 10 dk, `docs/VERIFICATION.md`): yaklaşık 19 MB/sn tahsis, baskın sınıf yok; en üstteki kalemler aşağıdaki 2 ve 5 numaralı maddelerin geçicileri.
2. Değer gibi davranan ama hâlâ heap'te tahsis edilen geçiciler: `tickChunk` dışındaki `Vec3`, `AABB`, `BlockPos` fabrikaları, `MutableBlockPos` kopyaları, `VoxelShape` zincirleri. Boşta ölçüldü: `Vec3` bayt tahsisinin %3,0'ı, `BlockPos` %2,8 (`docs/VERIFICATION.md`).
3. Dizgiler: `StringBuilder`, `char[]`/`byte[]` büyümesi (`Arrays.copyOf`), `String.format`/`concat`; sıcak yolları, yeniden kullanılan bir bölge tamponuna yazan Rust yamalarıyla değiştirin.
4. Sıcak çağrı noktalarındaki lambdalar ve yakalanan kapanımlar: kaçmıyorsa çerçevede tahsis edin (analiz genişletmesi), yakalamayan örnekleri yeniden kullanın.
5. fastutil `clone`, koleksiyon yeniden boyutlandırma, tick yollarındaki akış boru hatları; DFU artıkları. Boşta ölçüldü: `DataResult$Success` %2,5, `Optional` %2,3, `ArrayList$Itr` %3,0, `RegularEnumSet$EnumSetIterator` %2,3, `ImmutableCollections$ListItr` %2,5 (`docs/VERIFICATION.md`).
6. Her kalemden sonra: yeni kararlı durum profili, `docs/ALLOC_BASELINE.md` güncellenir.

## 7. Doğrulama ve araçlar (toplayıcıdan önce yapılır)

* **Duraklama/heap izleyicisi** (faz 0): toplama başına duraklama, en uzun duraklama, kademe başına saniyede tahsis edilen bayt, zaman içinde canlı bayt, dilim doluluğu. Çıktı profil JSON'una eklenir.
* **Sızıntı bütçesi aracı**: betikli oyun (`tools/headless_play.sh`), ısınmadan sonra her 10 sn'de canlı yönetilen baytları örnekler, bir eğim uydurur, bütçeyi aşarsa başarısız olur. `MONO_GC=off` aynı betiği toplayıcı kapalıyken çalıştırır; kademe 1-3'ün tek başına ne başardığını gösterir.
* **Toplayıcı testleri** (hızlı, Minecraft yok): `monolith/tests/gc/` içinde küçük bayt kodu programları: bir döngü çalışırken değiştirilen bağlı yapılar, çok iş parçacıklı tahsis fırtınaları, finalizasyonsuz döngüler, büyük diziler, statik kökler, derin yığınlar. Döngüler sürekli olsun diye çok küçük bir tetikleyiciyle çalıştırılır.
* **Doğrulama modu** `MONO_GC_VERIFY=1`: her eşzamanlı döngüden sonra, durdur-dünya tam işaretlemesi çalışır ve iki canlı küme karşılaştırılır; STW işaretlemesinin ulaştığı ama eşzamanlı süpürmenin serbest bıraktığı her nesne, nesnenin sınıfı ve tahsis noktasıyla birlikte programı durdurur.
* **Bölge/çerçeve denetleyicisi**: `MONO_ARENA_CHECK=abort` tüm oyun koşularında açık kalır.

## 8. Fazlar ve çıkış ölçütleri

| Faz | İş | Çıkış ölçütü |
|---|---|---|
| 0 | Duraklama/heap izleyicisi, sızıntı bütçesi aracı, `MONO_GC=off` anahtarı, güncel derlemede temel ölçüm | sayılar `docs/ALLOC_BASELINE.md`'ye kaydedildi |
| 1 | Satır içi + kaçış + `tickChunk` derlemesinin oyunda doğrulanması | 10 dk'lık koşuda arena ihlali yok, çökme yok, tahsis oranı prof4 ile karşılaştırılıp belgelendi |
| 2 | Toplayıcı testleri + doğrulama modu iskeleti; kademe 4 iç yapısı yeniden düzenlendi (epoch işaret biti, tembel süpürme), mevcut STW toplayıcısı üzerinde | testler yeşil, oyunda davranış değişikliği yok |
| 3 | Kod üretiminde yazma bariyeri, çalışma zamanı API'si, yerel/yama denetimi; SATB eşzamanlı işaretleyici, ayarlama, yardım, sert sınır | 30 dk'lık koşuda doğrulama modu temiz, duraklama < 1 ms, bariyer maliyeti < %3 |
| 4 | Çöp birikimi (bölüm 6), tick/kare için bölge kapsamları | yönetilen kararlı durum tahsisi < 5 MiB/sn |
| 5 | Profildeki en büyük sahipler için sahipli bellek | her sahip için yönetilen canlı heap ve toplayıcı CPU'su ölçülebilir biçimde düşer |
| 6 | `MONO_GC=off` soak'ı (60 dk betikli oyun; ilk koşu 10 dk sonra 13,3 GB RSS'te durduruldu) | kalan büyüme < 1 MiB/dk; sağlanırsa toplayıcı varsayılan olarak kapalı, `gc` özelliği arkasında güvenlik ağı olarak tutulur; sert sınır olayı = başarısızlık |
| 7 | Son çalışma zamanında platformlar: Android `.so`, Windows/macOS pencereleme | her biri sızıntı betiğini çalıştırır |

## 9. Derleme lojistiği (sırayı bu belirler)

Tam bir Minecraft yeniden üretimi yaklaşık 10 dk analiz ve yaklaşık 55 dk derleme (32 shard, 2 paralel iş) sürer; bir çalışma zamanı değişikliği, `--reuse-objs` geçerli olmadıkça (yalnızca iç yapılar, aynı ABI ve aynı üretilmiş kod) tüm shard nesnelerini geçersiz kılar. Bu nedenle:

* Toplayıcı iç yapıları (faz 2 ve faz 3'ün işaretleyicisinin çoğu) tam oyun yerine test programları ve küçük test derlemeleri üzerinde geliştirilir.
* Üretilen kodu değiştiren her şey (bariyer, bölge kapsamı işaretçileri, sahipli tahsis çağrıları, yeni yamalar) mümkün olduğunca az tam yeniden üretimde toplanıp uygulanır.
* Üretim, derleme ve oyun testleri hiçbir zaman aynı anda çalıştırılmaz (bellek sınırı).

## 10. Bu planın sabitlediği kararlar

* Toplayıcı tasarımı: arka plan iş parçacığında eşzamanlı SATB işaretleme, tembel taşımasız süpürme, şimdilik nesilsel şema yok.
* Duraklama bütçesi: yalnızca kök anlık görüntüsü; geri kalan her şey eşzamanlı ya da amortismandır.
* Sahiplik yamalarda ilan edilir, hata ayıklamada zehirlemeyle zorlanır ve asla sessizce çıkarılmaz.
* Toplayıcı, geçen bir soak'tan sonra varsayılan yoldan çıkarılır; güvenlik ağı olarak bir özellik arkasında ağaçta kalır.
* Sıra: ölç, sonra çöpü azalt, sonra güvenlik ağını görünmez yap; bariyer ve işaretleyici tek bir yeniden üretimde girer.

## 11. Riskler

* Eksik bir bariyer (yerel ya da yamada ham referans yazımı) SATB altında canlı nesneleri kaybettirir: tek depolama API'si, grep denetimi ve doğrulama modu ile azaltılır.
* Muhafazakâr yığın taraması bazı çöpleri canlı tutar: sınırlıdır, sızıntı aracında görünür.
* Sahipli serbest bırakma hataları belleği bozar: zehirleme/karantina modu ve erişilebilirlik denetimi; sahipli tipler birer birer devreye alınır.
* Sıkıştırma olmadan parçalanma: dilim doluluk metriği; profilden boyut sınıfı ayarı.
* Android: işaretleyici iş parçacığı önceliği ve big.LITTLE zamanlaması bir cihazda ölçülmeli (faz 7).

## Önceden kaydedilmiş sızıntı kapısı (ölçümden önce sabitlendi)

- Senaryo: kaydedilmiş dünyayı aç, hareketsiz dur (yürüme yok, yeni yonga yok), 60 dakika (`tools/leak_soak.sh --minutes 60`).
- Isınma: ilk 30 dakika yok sayılır (oyuncunun çevresindeki dünya bu pencerede oturur; ilk 60 dakikalık soak'ta ölçüldü).
- Kapı: 30-60. dakikalar arasında yönetilen canlı baytların en küçük kareler eğimi, `tools/leak_budget.py --warmup-s 1800 --budget-mb-per-min 1` ile.
- Geçer: eğim <= 1 MiB/dk. Kalır: üzerindeki her şey. Pencere koşudan sonra değiştirilmez.
