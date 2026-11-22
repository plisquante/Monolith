# AOT mimarisi: her şey derleme aşamasında

Durum: tasarım belgesi, kod yok. Amaç: çalışma zamanında yapılan her işi (bellek kararı, başlatma, kaynak
ayrıştırma, shader derleme, sanal çağrı çözümü) derleme aşamasına taşımak. Bu belge hedefleri, kuralları, eksikleri,
riskleri ve sırayı tek yerde toplar. Ölçülmemiş her şey "TAHMİN" veya "ÖLÇÜLMEDİ" diye işaretlidir.

## 1. Hedefler (ölçülebilir)

Hedefler cihaz sınıfına göre. S24 FE (7 GB, Xclipse 940) en zorlu referans, S26 (11 GB) rahat referans.

| Ölçü | Şimdi | Hedef | Not |
|---|---|---|---|
| Yüzey hazır, ilk kare sunuldu | ÖLÇÜLMEDİ | S24 FE: 150 ms altı | Android süreç başlangıcı ve Vulkan/ANGLE başlatma bu hedefin içinde, motor payı 30 ms altı |
| Motor hazır (ana menü çizilebilir) | ÖLÇÜLMEDİ (log'da yaklaşık 18 sn dünya öncesi) | 300 ms altı | Görüntü yığını (image heap) ile |
| Menüde canlı veri | 1,0 GB | 300 MB altı | Gerçek Minecraft menüde bunun yaklaşık 3'te biri (TAHMİN) |
| Oynanışta GC sayısı | S26'da 8 zorlama + ~7 hacim / 126 sn | 0 | Yükleme ekranı ve menüde serbest |
| Oynanışta en uzun duraklama | S26: 1,1 sn, S24: 1,5 sn | 50 ms altı | |
| FPS ortalama (dünyada) | S26: 66, S24: 23 | S24: 45 üstü, 1% düşük 25 üstü | |
| Varlık yüklemede çökme | S24 FE'de var | 0 | Sebep büyük ihtimalle heap tavanı (7.1) |
| `.so` boyutu (soyulmuş) | 587 MB | 150 MB altı | Ağaç budama + ölü kod silme |

"0,03 saniyede Mojang yazısına gelmeden açılsın" hedefi hakkında dürüst not: Android'de süreç başlatma (zygote fork)
ve sürücü başlatma tek başına yüzlerce ms sürebilir (TAHMİN, ölçülmedi). Motorun kendi payını 30 ms altına indirmek
mümkün, çünkü başlangıç işini derleme aşamasına taşıyacağız (bölüm 4). Toplam 30 ms değil, motor payı 30 ms.

## 2. Bugünkü mimari ve bulunan hatalar

Hat: Minecraft jar -> monolith (Python) -> Zig kaynağı (101 birim) -> `libminecraft_mono.so`; başlatıcı
(TouchCore APK, Java + C) motoru yükler. Çalışma zamanı: `runtime/rt.zig` (nesneler, GC, iş parçacıkları),
`mem_pool.zig` (slab havuzu).

### 2.1 Bellek ve GC (koddan ve loglardan kanıtlı)
| # | Hata / eksik | Kanıt | Düzeltme (bölüm) |
|---|---|---|---|
| B1 | Heap tavanına dayanınca `no free slab` ve OOM riski. Dilimler boyut sınıfına bağlı; kısmen dolu dilim başka boyuta verilemez. Varlık (entity) yüklenince yeni boyut sınıfı yeni dilim ister. | Alper raporu: heap 3279 MiB = tavan, toplama nedeni `no free slab` | 5.3, 5.5 |
| B2 | Minecraft her tick `System.gc()` çağırıyor (126 sn'de ~1350 çağrı). Şu an çalışma zamanında hız sınırlı (10 sn'de bir). | Cem raporu: 8 gerçek, 1342 atlanan | Java overlay'de çağrıyı derleme zamanında sil (5.6) |
| B3 | Canlı veri menüde 1 GB. İşaretleme süresi buna bağlı (S26 0,4-0,8 sn, S24 1,4 sn). | `gc #N` satırları | Görüntü yığını + küçültme (4.1, 4.3, 5.4) |
| B4 | İşaretleme iş parçacıkları büyük/küçük çekirdek ayrımı yapmıyor (10 çekirdek, farklı hızlar). S24 işaretleme S26'dan 2-3 kat yavaş. | Aynı log'lar | Büyük çekirdeklere sabitle (5.3), ölçülmedi |
| B5 | Tembel süpürme bitmeden yeni toplama başlayınca 100-400 ms ek bekleme (`finish-sweep`). | Eski loglar | Arka plan süpürücü (5.3) |
| B6 | Muhafazakâr yığın taraması yanlış canlı tutabilir. | Tasarım gereği | Kesin yığın haritası (5.5) |
| B7 | Büyük nesneler (32 KB üstü) `malloc` ile ayrı; parçalanma ve sayım ayrı. | `rt.zig` rawAlloc | Büyük nesne bloklarını havuza al (5.3) |
| B8 | Ayırma profilleyicisi yalnız yığın ayırmalarını sayıyor, arena'yı saymıyor. | `allocObject` | Ömür profilleyicisi (bölüm 6) |
| B9 | Doğrudan bellek (NIO ByteBuffer) hesabı GC'den ayrı. | `native_mb` | Tek bütçe (5.4) |

### 2.2 Başlatma (loglardan)
| # | Bulgu | Kanıt |
|---|---|---|
| S1 | Açılışta toplam ~7-8 GB ayırma yapılıyor. | Cem/Alper profil çıktısı |
| S2 | En büyük kaynaklar: DataFixer (`Func`, `Optional`, `RewriteResult`, `View`; toplamın %15-20'si), JSON okuma (`[C` BufferedReader/Arrays.copyOf/String.<init>; %12), PNG çözme, DensityBuffer, VoxelShape birleştirme. | Aynı |
| S3 | Shader derleme çalışma zamanında (shaderc + SPIRV-Cross, `libmonoshader.so`). | `tools/build_shader_lib.py` |
| S4 | `.so` APK içinde sıkıştırılmış (xz) taşınıp kullanıcı elle içe aktarıyor; `extractNativeLibs=true`. | Manifest, launcher akışı |

### 2.3 Diğer hatalar
| # | Hata | Durum |
|---|---|---|
| D1 | Çıkışta `Cannot close a surface while it is acquired` (kapanış çökme raporu). | Açık |
| D2 | Çalışma zamanı değişince tam derleme 100 dk; VM yeniden başlayınca takipsiz derleme ölüyor. | Süreçle çözüldü (takipli görev), mimari çözüm: shard sınırlarını runtime'dan ayır (7.4) |
| D3 | `patches/README.md` eski Frame kuralını anlatıyor. | Açık, belgeyi düzelt |
| D4 | Testler atlanıyor; davranış eşitliği doğrulanmıyor. | Bölüm 8 |
| D5 | Yüzey kaybı (uygulama arka plana gidip gelince) işlenmiyor. | Açık |

## 3. İlkeler

1. **Kapalı dünya (closed world):** Bütün sınıflar derleme anında bilinir. Yansıma, ServiceLoader, MethodHandle hedefleri
   açık listeyle bildirilir (`reflect-classes.txt` zaten var). Listede olmayan sınıf çalışma zamanında bulunamaz ve
   yüksek sesle hata verir (sessiz yedek yok).
2. **Çalışma zamanında karar yok:** Bir iş derleme zamanında yapılabiliyorsa yapılır. Yapılamıyorsa nedeni bu belgede
   yazılıdır (7.2 listesi).
3. **Kanıtlanabilir olan statik, kanıtlanamayan güvenli yedekle:** Nesne ömrü derleme zamanında kanıtlanırsa bölge/yığın,
   kanıtlanamazsa GC yığını. Yedek GC oynanışta çalışmaz, yükleme ekranı ve acil durumda çalışır.
4. **Her dönüşüm doğrulanır:** HotSpot ile çıktı karşılaştırma. Doğrulanmamış dönüşüm yayına girmez.
5. **Ölçmeden optimizasyon yok:** Her aşamanın bitiş ölçütü rapordaki sayıdır.

## 4. Derleme aşamasına taşınacaklar

### 4.1 Görüntü yığını (build-time init + heap snapshot)
GraalVM Native Image'ın yaptığı şey (bilinen teknik): seçili sınıfların statik başlatıcıları (`<clinit>`) derleme anında
çalıştırılır, ortaya çıkan nesne grafı `.so` içine **veri olarak** gömülür. Çalışma zamanında bu nesneler hiç
oluşturulmaz, sayfalar mmap ile tembel yüklenir (dosya destekli, çekirdek önbellekten paylaşır, özel kirli bellek olarak
sayılmaz).
- **Ne taşınır:** kayıt defterleri (Registry, Block/Item/Entity türleri), sabit tablolar, dil dosyaları, model/blockstate
  JSON'unun ayrıştırılmış hali, DataFixer kuralları (en azından güncel sürüm için), CLDR verisi, sınıf kalıpları.
- **Güvenli başlatma sınıflandırması (zorunlu):** Bir sınıfın başlatıcısı yalnız şunları yapıyorsa derleme zamanında
  çalıştırılır: saf hesaplama, jar kaynaklarını okuma, sabit koleksiyon kurma. Şunlar olursa **çalışma zamanına bırakılır**:
  saat (`System.nanoTime`, `currentTimeMillis`), rastgele tohum (`Random()` tohumsuz), ortam değişkeni/sistem özelliği,
  dosya/ağ I/O (jar dışı), iş parçacığı başlatma, yerel (native) tutamak, `Thread` yerel değişkenleri. Sınıflandırma otomatik
  (çağrı grafı taraması) + elle liste (patches/manifest benzeri `init-policy.json`).
- **Kimlik özetleri (identityHashCode):** Görüntüde sabitlenir. `HashMap` yineleme sırası bunlara bağlıysa HotSpot'tan
  farklı sıra çıkar; bu bir davranış farkıdır. Karşı önlem: kimlik özeti kullanan koleksiyonları derleme zamanında
  tespit edip başlatma listesinden çıkar veya sıra bağımsızlığını testle kanıtla (bölüm 8).
- **Yazılabilir kısım:** Görüntü iki bölgedir: salt okunur (varsayılan) ve yazılabilir. Salt okunur bölge `mprotect` ile
  korunur; yazma olursa SIGSEGV işleyicisi o sayfayı yazılabilir yapar ve **kirli sayfa** olarak kaydeder (MMU tabanlı
  yazma bariyeri). GC yalnız kirli sayfaları tarar. Böylece 1 GB canlı verinin büyük kısmı taranmaz ve derleyiciye bariyer
  kodu eklemek gerekmez. Dikkat: mevcut kilitlenme (crash) işleyicisiyle zincirlenmeli (tek SIGSEGV sahibi), 16 KB sayfa
  boyutlu cihazlar için sayfa boyutu çalışma zamanında okunmalı (şu cihazlar 4096). ÖLÇÜLMEDİ: kirli sayfa oranı.
- **Beklenen kazanç:** Açılışta ayırmaların büyük kısmı ve DataFixer/JSON çalışması yok olur (S1, S2). Menüde canlı
  veri düşer (B3).

### 4.2 Kaynak önişleme
- JSON modeller, blockstates, tarifler, dil dosyaları: ikili biçime çevrilmiş halde taşınır (ayrıştırma yok).
- Doku atlası: derleme zamanında birleştirilir (stitching), çalışma zamanında yalnız GPU'ya yüklenir. PNG çözme (stb) yok.
  Kaynak paketi/mod desteği varsa çalışma zamanı yolu yedek olarak kalır (ayrı mod).
- Ses, yazı tipi: önceden çözülmüş.

### 4.3 Shader önderleme
Vanilla shader'ları derleme zamanında SPIR-V'ye çevrilir (shaderc yok çalışma zamanında). Vulkan boru hattı önbelleği
(pipeline cache) ilk çalışmada diske yazılır, sonraki açılışlarda yüklenir. Çalışma zamanı derleyicisi yalnız özel shader
(shader paketi) için yedek.

### 4.4 Derleyici dönüşümleri
Koda bakınca düzeltilen not: bunların bir kısmı **zaten var**, planda "yapılacak" yazmıştım.

| Dönüşüm | Durum (koddan) | Kalan iş |
|---|---|---|
| Ulaşılabilirlik / ağaç budama | **Var.** `monolith/program.py`: `Program.run` kökten başlayıp RTA ile ulaşılabilir metot, örneklenen sınıf ve sanal çağrı kümesini çıkarıyor. Yansıma kökleri, ServiceLoader, JNI geri çağrıları ele alınıyor. | `.so` boyutunun neden 587 MB olduğunu ölçmek (JDK payı, MC payı, Zig kod şişmesi) |
| Sanal çağrı çözümü (CHA) | **Var.** `zigen.py`: tek hedefli çağrı doğrudan, 2 hedefli çağrı korumalı satır içi, kalanı itable/vtable. | Yok (ölçümle bak) |
| Yaprak (leaf) satır içine alma | **Var.** `_can_inline_leaf` | Yaprak olmayan küçük metotlara genişletmek |
| Metot içi kaçış analizi | **Var ama dar.** `_analyze_local_allocs`: yalnız döngüsüz, istisna işleyicisiz, tek metot; çağrılan metoda geçen nesne, kendi kurucusu hariç, kaçmış sayılıyor. Kaçmayanlar arena'ya gidiyor (parçalama yok). | Metotlar arası özetler, döngüler, alan akışı, nesne parçalama |
| Güvenli nokta yoklamaları | Kısmen (yaprak metotlarda yok) | Sınırlı döngülerde atlama |
| Sabit katlama | Zig/LLVM yapıyor | PGO |

### 4.4.1 Metotlar arası kaçış analizi (tasarım)
- Birim: metot özeti. Her referans parametre (alıcı dahil) için: `NoEscape` (alana, statik alana, diziye yazılmaz;
  dönmez; kaçan bir çağrıya geçmez; monitör olarak kullanılmaz) veya `Escapes`.
- Çağrı hedefleri: `Program.vcalls` ve CHA ile çözülen hedef kümesi. Küme boş değilse ve her hedefin parametresi
  `NoEscape` ise geçen nesne kaçmaz. Küme çözülemiyorsa (arayüz, lambda, yansıma) `Escapes`.
- Yerel (native) metotlar, yansıma, `Thread`, monitör: `Escapes`.
- Sabit nokta: iyimser başlangıç, yükselen küme; yakınsama bütçesi aşılırsa yakınsamayan tüm özetler `Escapes`.
- Döngüdeki ayırmalar bu aşamada arena'ya gitmez (arena büyür); yalnız döngü dışı ve nesne parçalama ile kaldırılabilenler.
- Çıktı: `karar.json` (ayırma noktası, katman, sebep) ve insan okunur rapor.
- **Ölçüm (21 Kasım, `monolith/escape.py`, gerçek Minecraft):** 427.350 metot 56 sn'de analiz edildi. 110.015 ayırma noktasının
  yalnız 8.639'u (%7,9) kaçmıyor, döngü dışı olanlar 8.029 (%7,3). Kaçma sebepleri: başka metoda argüman %43, `areturn`
  %35, `putfield` %6, çözülemeyen çağrı %3, `aastore` %2,5. `net/minecraft/world`: 20.377 noktanın 607'si (%3).
  Ölçülen çöp profiliyle eşleştirince (profildeki ilk 15 ayırıcının MiB toplamı): **%95,5'i tüm noktaları kaçan metotlarda,
  %1,5'i kullanılabilir, %3'ü JDK/analiz dışı.** Sonuç: metotlar arası kaçış analizi tek başına çöp hacminin en çok
  %1,5-5'ini kaldırır. Çöp hacmi dönen (areturn) ve başka nesnede saklanan (argüman, alan) nesnelerden geliyor.
  Bu yüzden ana yol bölge modeli (5.2), "çağıranın bölgesinde ayır" (dönen nesneler için), sıcak yol yamaları (4.5) ve
  görüntü yığınıdır (11.3). Kaçış analizi tamamlayıcı kalır.

### 4.5 Veri düzeni
- **String:** `char[]` yerine `byte[]` (Latin-1) + bayrak. Beklenen: metin belleğinin yarısı (TAHMİN, ölçülecek).
- **Alan paketleme:** sınıf başına alan sırası boyut azalan; boolean/byte birleştirme.
- **Sıcak yollar için düz yapılar (SoA):** PalettedContainer/BitStorage, gürültü tamponları, mesh vertex verisi, chunk
  bölümü; nesne başına başlık yok. `patches/` sistemiyle, `MONO_PATCH_VERIFY=1` ile orijinal Java ile karşılaştırılır.

## 5. Bellek modeli

### 5.1 Dört katman
| Katman | İçerik | Kim karar verir | Silinme |
|---|---|---|---|
| A. Yığın / skalar | Kaçmayan küçük nesneler | Derleyici (kaçış + nesne parçalama), kanıtlı | Metot sonu |
| B. Bölge (arena) | Görev/tick/kare bazlı geçici nesneler | Derleyici + açıklama (annotation), kanıtlı | Bölge sonunda topluca |
| C. Görüntü yığını | Kalıcı başlangıç verisi | Derleme zamanı (4.1) | Hiç / yazılan sayfalar GC taranır |
| D. GC yığını | Kalan her şey | Çalışma zamanı | Yükleme/menü/acil GC |

Hedef dağılım (TAHMİN, 6. bölümdeki profilleyici ile gerçek sayı çıkacak): açılışta ayırmaların %70+ C ve A'ya, dünya
yüklemede %60+ B ve A'ya düşer; oynanışta D yalnız uzun ömürlü değişkenler (chunk, varlık).

### 5.2 Bölge kuralları
- Bölge türleri: `ChunkTask` (üretim, mesh), `Tick`, `Frame`, `Load`.
- Bir bölgede ayrılan nesne bölge dışına **sızamaz**: statik alana, sınıf/başka bölge nesnesine, iş parçacığı
  paylaşılan koleksiyona yazılamaz. Derleyici sızma analizi yapar (metotlar arası, alan akışı). Analiz "emin değilim"
  derse nesne D katmanına gider (yedek, kanıt aranmaz).
- Hata ayıklama modu: bölge sonunda belleği zehirle (0xDD) ve bölge dışından içeriye işaretçi taraması yap
  (tavan: yalnız `MONO_REGION_CHECK=1`).
- Notlar: iş parçacığı sahipliği (bir bölge tek iş parçacığına ait), iç içe bölge (iç, dış bölgeye sızabilir, tersi hayır).

### 5.3 Yığın (D katmanı) iyileştirmeleri
- Dilim parçalanması (B1): boş dilimler boyut sınıfları arasında paylaşılır (şu an var), yetmeyen durumda **kısmen taşıyan
  (mostly-copying)** toplama: yığın işaretçileri kesin olduğu için (ref_offsets), muhafazakâr kökler yalnız işaret ettiği
  sayfaları sabitler (pin), kalan sayfalar taşınabilir. Bu, parçalanmayı ve nesil tabanlı genç nesli mümkün kılar. Bayt
  işaretçi güncelleme maliyeti ve kesin yığın haritası şartı var (5.5). Araştırma işi, A/B/C yetmezse.
- Arka plan süpürücü: tembel süpürmeyi bir iş parçacığı bitirir (B5).
- İşaretleme iş parçacıklarını büyük çekirdeklere bağla (B4), ölçülecek.
- Büyük nesneler (B7): havuzdan blok al, `malloc` bırak.

### 5.4 Bütçeler
Tek bütçe: yığın + doğrudan bellek + yerel (GL, doku) + kod. Cihaz sınıfına göre sabit bütçeler (APK ayarı),
`onTrimMemory` ve düşük bellek sinyallerinde küçül. Tavan, GC yerine bütçe politikasıyla korunur: bütçeyi aşan ayırma
isteği OOM değil, "ağır toplama + bölge boşaltma" yolunu dener, olmazsa temiz OOM.

### 5.5 Kesin yığın haritası
Derleyici her güvenli noktada hangi yığın yuvalarının referans olduğunu yazsın (şu an muhafazakâr). Kazanç: yanlış canlı
yok (B6), taşıyan toplama mümkün. Maliyet: üretilen kod yığın haritası tabloları (`.so` boyutu artar). Ancak A/B/C
çalışınca gerek kalmayabilir, karar noktası (bölüm 9).

### 5.6 GC'nin yeni rolü
- Oynanışta tetiklenmez. Tetik yalnız: yükleme ekranı, duraklatma menüsü, bütçe acil durumu.
- Oynanışta ayırma bütçesi: bölge dışı ayırmaların sayacı; eşiği aşarsa log + (debug) çökme (böylece sızıntı kaynağı bulunur).
- `System.gc()` Java overlay'de silinir (B2). Gerçek istek gerekiyorsa yalnız menüde.

## 6. Ölçüm altyapısı (her şeyin ön şartı)
1. Yığın dökümü: sınıf başına canlı bayt, en büyük 50 sınıf, her fazda (açılış, menü, dünya, oynanış).
2. Ömür profilleyicisi: her ayırma noktası için ömür sınıfı (kalıcı, görev, tick, uzun) ve hayatta kalma oranı; sonuç
   tablosu katman A/B/C/D dağılımını verir ve hangi noktaya bölge/yama gerektiğini söyler.
3. Zaman çizelgesi: süreç başı, motor yükleme, kayıt defterleri, kaynaklar, shader, ilk kare, menü (ms).
4. CPU örnekleyici: hangi Java metodunda ne kadar süre (sıcak yol bulmak için).
5. Rapor sistemi (metrics.csv, performans sayfası) genişletilir; cihaz sınıfı başına kabul tablosu.

## 7. Eksikler, riskler, açık noktalar

### 7.1 Doğruluk riskleri
| Risk | Etki | Önlem |
|---|---|---|
| Yanlış kaçış/bölge analizi | Rastgele çökme, veri bozulması | Kanıt yoksa D'ye; diferansiyel test; bölge zehir modu |
| Görüntü yığını başlatma yan etkisi | Davranış farkı | Başlatma sınıflandırması, kimlik özetleri, testle |
| Yansıma listesi eksik | `NoClassDefFound` çalışma zamanında | Liste üretimi otomatik + yüksek sesle hata |
| SIGSEGV yazma bariyeri | Çökme işleyicisiyle çakışma, yavaşlık | Tek işleyici, kirli sayfa oranını ölç, yazılabilir bölgeyi küçük tut |

### 7.2 Derleme zamanında yapılamayacaklar (nedeniyle)
- Kullanıcı kaynak paketi, mod, dünya verisi, ağ, saat, rastgele (tohumsuz), yerel (native) tutamaklar.
- Kimlik özetine bağlı koleksiyon sırası (tutarlı kılınmadıkça).
- Tam sahiplik çıkarımı genel Java için (çözülemeyen problem). Bu yüzden D katmanı yedek olarak kalır.

### 7.3 Platform
- Android yaşam döngüsü: yüzey kaybı, arka plan, `onTrimMemory`, 16 KB sayfa (Android 15+).
- `.so` dağıtımı: `extractNativeLibs=false` ve hizalı, sıkıştırmasız APK içinde, doğrudan mmap (kopyasız). Şu an xz'li
  harici içe aktarım (S4); hedef: APK içinde (boyut 150 MB altı olunca).
- Çoklu mimari yok (yalnız arm64-v8a).

### 7.4 Derleme sistemi
- Çalışma zamanı değişince tam derleme (100 dk) çok pahalı. Hedef: çalışma zamanı ayrı bir birim/ayrı `.a`, shard'lar
  yalnız arayüze bağlı, çalışma zamanı iç değişiklikleri shard'ları geçersiz kılmasın.
- Ağaç budama derleme süresini kısaltır (kod azalır).
- Uzun derlemeler takipli arka plan görevi.

## 8. Doğrulama
- **Diferansiyel test:** Aynı girdi, HotSpot ve yerel çıktı karşılaştırması. Küçük Java programları + Minecraft sahneleri
  (dünya üretimi tohumdan, aynı tohumda aynı chunk çıktısı karşılaştır).
- **Yama doğrulama:** `MONO_PATCH_VERIFY=1` (var).
- **GC stres testi:** (var, GcStress) her sürümde.
- **Bölge testleri:** zehir modu + sızma testleri.
- **Cihaz raporu:** 2 cihaz (S24 FE, S26) her aşamada, kabul tablosu bölüm 1.
- Bu makinede testler bellek yüzünden çalıştırılamıyor (CLAUDE.md). Testler ayrı bir CI makinesine taşınmalı (açık karar, bölüm 9).

## 9. Yol haritası ve kapılar
Süre tahminleri TAHMİN, hafta olarak (tek kişi + AI). Her aşama bir derleme + iki cihaz raporuyla biter.

| # | Aşama | İçerik | Süre | Kapı (geçilmezse sonraki aşama yok) |
|---|---|---|---|---|
| 0 | Hepsini tek seferde kapatan "kararlılık paketi" | B1 (tavan politikası), B2 (System.gc sil), B4/B5 ölçüm, D1 (kapanış), D3 | 0,5-1 hf | S24 FE'de varlık yüklemede çökme yok |
| 1 | Ölçüm altyapısı (bölüm 6) | Yığın dökümü, ömür ve zaman çizelgesi, CPU örnekleyici | 1 hf | Katman dağılım tablosu çıktı |
| 2 | Veri küçültme ve hızlı kazançlar | 4.5 String, DataFixer tembel, doku CPU kopyaları | 1 hf | Menüde canlı 500 MB altı |
| 3 | Kaçış analizi v2 + nesne parçalama (4.4) | Çevirici işi, diferansiyel testli | 2-3 hf | Ayırma sayısı ölçülen X% azaldı, testler yeşil |
| 4 | Derleme zamanı kaynak ve shader (4.2, 4.3) | Önişleme hattı | 2 hf | Açılış ayırması %50 altı |
| 5 | Görüntü yığını (4.1) | Başlatma sınıflandırması, anlık görüntü, MMU kirli sayfa | 3-5 hf | Motor hazır 300 ms altı |
| 6 | Bölge modeli (5.2) + sıcak yol yamaları (4.5) | ChunkTask, Tick, Frame bölgeleri; PalettedContainer vb. | 3-4 hf | Dünya yüklemede GC sayısı ~0 |
| 7 | GC'yi güvenlik ağına çevir (5.6), bütçe politikası | Oynanışta GC 0 | 1 hf | Oynanışta GC sayısı 0 (rapor) |
| 8 | Karar noktası | A-D yetmezse: kesin yığın haritası + taşıyan GC (5.3, 5.5) veya bariyerli nesil tabanlı GC dalı | – | Yalnız ölçülen veriyle |

Sıra gerekçesi: 0-2 hızlı, riski düşük, ölçülebilir; 3-6 asıl mimari ve en riskli; 7 hedefi doğrular.
2'den sonra durup S24 FE'de 30 FPS'e ulaşıp ulaşmadığına bakarız. Ulaşırsa 5-6 ertelenebilir.

## 10. Açık kararlar (kullanıcıdan)
1. Test makinesi: diferansiyel ve GC stres testleri için bellekli ayrı bir CI makinesi kurulabilir mi? Yoksa testler telefon raporuyla mı doğrulansın?
2. `.so`'yu APK içine koymaya (S4, 7.3) ne zaman geçelim? Boyut hedefi için ağaç budamayı mı bekleyelim?
3. Kaynak paketi/mod desteği şimdilik gerekli mi? Gerekmiyorsa 4.2'de çalışma zamanı yedeği kaldırılır (daha hızlı, daha az kod).

## 11. Nasıl yapılır: mekanizmalar (koda dayalı)

Bu bölüm "derleme zamanında nasıl" sorusunun cevabıdır. Dayanak: `Object` başlığı 16 bayt (`class*`, `hash`, `gc_bits` ve
monitör indeksi, `runtime/rt.zig`), `client.jar` içinde 11383 sınıf, 4371 doku, 4195 model, 1289 blockstate, 93 shader ve
10449 veri paketi dosyası var. Yani vanilla'nın kodu, dokuları, modelleri, shader'ları ve veri paketi **jar'ın içinde**;
yalnız sesler ve bazı diller asset index'ten (cihazda indirilen) geliyor.

### 11.1 Üç aşama
| Aşama | Nerede | Ne yapılır |
|---|---|---|
| Derleme (build) | Geliştirici makinesi/CI | Kod dönüşümleri (11.2), görüntü yığını (11.3), shader/model/doku önişleme (11.5) |
| Kurulum (install) | İlk açılışta cihazda bir kez, dex2oat gibi | Asset index'ten gelenlerin (ses, diller) ve kullanıcı paketlerinin önişlenmesi, `files/` altında önbellek |
| Çalışma (run) | Cihaz | Yalnız mmap + oynanış. Ayrıştırma, derleme, başlatma, GC (oynanışta) yok |

Kurulum aşaması önemli: Mojang varlıklarını `.so`'ya gömmeyi gerektirmeden "çalışma zamanında hiçbir şey yapma" hedefine
ulaşılır.

### 11.2 Bellek kararları derleme zamanında (çevirici, `monolith/zigen.py`)
Bugün: `_analyze_local_allocs` yalnız **döngüsüz, istisna işleyicisiz, tek metot** içinde, soyut yorumlama ile
"nesne metottan kaçıyor mu" bakıyor ve kaçmayanları arena'ya koyuyor (döndürülen nesne kaçmış sayılıyor). Genişletme:
1. **Metotlar arası özetler:** Çağrı grafı alt-yukarı (SCC sırasıyla) her metot için her parametre/dönüş için bir özet
   üretilir: "parametre i: kaçmaz / sadece döner / yığına kaçar". Çağıran, çağrılanın özetiyle analizi sürdürür.
   Kapalı dünya (11.4) sayesinde sanal çağrılar CHA ile çözülür; çözülemeyenler "kaçar" sayılır (güvenli yön).
2. **Döngüler:** Sabit nokta (fixpoint) çalışma listesi. Bütçe aşılırsa "kaçar" (güvenli yön).
3. **Alan akışı:** `a.f = b` ile b, a'nın kaçış durumunu miras alır (Andersen tarzı basit işaret grafı).
4. **Nesne parçalama:** Kaçmayan ve yalnız alan okuma/yazma ile kullanılan nesne, alanları yerel değişkene açılır
   (`Vector3f`, `BlockPos` değişebilir, `Optional`, kutulu `Integer`, `Iterator`). Hiç bellek ayrılmaz.
5. **Bölge çıkarımı:** Bölge kökü bir metot (ör. chunk üretim görevi). Kökten erişilen çağrı ağacında ayrılan ve
   (a) statik alana, (b) kök dışında yaşayan nesneye, (c) başka iş parçacığına, (d) kök dönüşüne yazılmayan her ayırma
   o bölgeye gider. Kanıtlanamayan D'de (GC yığını) kalır. Kök listesi `regions.json` (patches/manifest gibi) ile verilir,
   yani elle seçilir, analiz doğrular.
6. **Doğruluk emniyeti:** Her dönüşümün ürettiği karar bir rapor dosyasına yazılır (ayırma noktası -> katman). Hata ayıklama
   derlemesinde bölge sonu zehirleme (0xDD) ve dışarıdan içeri işaretçi taraması vardır.

### 11.3 Görüntü yığını (registry'ler, tablolar) derleme zamanında
Yöntem: **derleme makinesinde gerçek başlatmayı çalıştırıp bellek görüntüsünü almak.** Bu makinede x64 sürümün başsız
çalıştığını zaten gördük (`world_test.sh`), yani çalıştırabiliriz.
1. x64 "yapıcı" ikilisi `Bootstrap.bootStrap()` ve gerekli statik başlatıcıları çalıştırır (pencere ve cihaz öncesi).
2. **Başlatma izleyicisi (taint):** Çalışma zamanı her sınıf başlatmasında şu olayları izler: saat, tohumsuz `Random`,
   ortam/sistem özelliği, jar dışı dosya/ağ I/O, iş parçacığı başlatma, yerel tutamak, `identityHashCode` kullanımı. Bu
   olayı gören sınıf (ve başlatması onu tetikleyen sınıflar) görüntüden **çıkarılır**, çalışma zamanında lazy başlatılır.
3. Görüntüye giren sınıfların statik alanlarından erişilen nesne grafı yazılır: nesne = 16 bayt başlık + alanlar. Nesnelerde
   fonksiyon işaretçisi yok (sanal çağrı sınıf vtable'ından, o da `.so` kodu), yani **sınıf işaretçisi sınıf kimliğine
   (`c.id`) çevrilerek taşınabilir**. Monitör indeksi 0, `gc_bits` yalnız ALLOC.
4. arm64 derlemesi aynı düzeni üretiyorsa (64 bit, aynı alan sırası) x64 görüntüsü arm64'te geçerlidir. Derleme, iki
   hedefin sınıf düzeni özetini (hash) karşılaştırır, farklıysa durur.
5. Çalışma zamanında: görüntü dosyası `MAP_FIXED_NOREPLACE` ile sabit adrese mmap edilir (sınıf işaretçileri tek seferlik
   düzeltme tablosuyla; HotSpot CDS arşivi gibi, adres doluysa yeniden konumlandırma yedeği). Dosya destekli olduğu için
   sayfalar tembel yüklenir ve özel kirli belleğe sayılmaz.
6. GC bu bölgeyi **kök olarak** görür ama yalnız MMU ile yazıldığı kaydedilen (kirli) sayfaları tarar (4.1). Yazılmayan
   sayfa hiç taranmaz: 1 GB canlı verinin taranan kısmı kirli sayfalara düşer.

### 11.4 Kapalı dünya ve ağaç budama
Zaten var (`Program.run`, RTA). Kökler: `Main`, `reflect-classes.txt`, RT kökleri, ServiceLoader sağlayıcıları, JNI geri
çağrıları. Yapılacak: görüntü yığınına giren sınıfları köke ekle ve ölçüm (hangi paketler `.so`'nun kaç MB'ı).
Yansıma/MethodHandle hedefi listede yoksa çalışma zamanında yüksek sesle hata verir.

### 11.5 Kaynak, shader, atlas önişleme
- **Derleme:** jar içindeki shader'lar SPIR-V'ye (shaderc derleme makinesinde), modeller/blockstates/items/tarifler/loot/tag
  ikili biçime, doku atlası birleştirilmiş (stitching) ve GPU'ya hazır sıkıştırılmış doku (ASTC) olarak üretilir.
- **Kurulum:** asset index'ten gelen ses/dil ve kullanıcı kaynak paketi ilk açılışta aynı hatla işlenir, önbelleğe yazılır.
- **Çalışma:** önişlenmiş dosyalar mmap, GPU'ya doğrudan yükleme.
- Dikkat: doku atlası kaynak paketine bağlı. Paket değişirse önbellek geçersiz olur (kurulum aşaması yeniden çalışır).

### 11.6 Kapı: neyin gerçekten ölçüldüğü
Bu mekanizmalar tasarım. Kanıtlanması gereken varsayımlar: (1) başlatma izleyicisinin görüntüye alabileceği sınıf oranı,
(2) kirli sayfa oranı (MMU bariyeri işe yarar mı), (3) arm64/x64 düzen eşitliği, (4) kaçış analizinin ayırmaların kaçını
yok ettiği, (5) önişlenmiş kaynakların açılış süresine katkısı. Her biri 1. aşama ölçümüyle (bölüm 6) sayıya çevrilir.

## 12. Yeni bulgu (Alper, 16:56, 36 sn): işlem sessizce öldü
S24 FE: çökme raporu yok, yerel (native) çökme dosyası yok, `OutOfMemoryError` yok. Ölçüm son satırı 36. saniyede
PSS 4355 MB, sistemde 1176-1351 MB boş; log'un sonu `gc #7: heap 3495 MiB, next in 3069 MiB`. Debug açık olduğu için
uygulamanın kendi bellek korumacısı kapalı. **Büyük ihtimalle Android'in düşük bellek öldürücüsü (LMK) işlemi öldürdü**
(sinyal ve raporsuz bu yüzden). Sebep: çöp aralığı (GC faktörü 300) S24 FE'de heap'i 3,5 GB'a kadar şişiriyor.
Kanıt eksik: Android'in çıkış sebebini (`ApplicationExitInfo`) rapora yazmıyoruz. Aşama 0'a eklenir:
(1) sonraki açılışta çıkış sebebini rapora yaz (LOW_MEMORY, SIGNALED, vb.), (2) çöp aralığını cihaz RAM'ine göre
sınırla (S24 FE: canlı + en çok 1 GB), (3) heap tavanını sistemde boş RAM'e göre dinamik azalt.

## 13. Sahiplik modu (Rust'ın yaptığını modül modül yapmak)

Rust'ta sahiplik, programcı kodu öyle yazdığı için var. Bizim kodu değiştirebildiğimiz için (çevirici, Java overlay,
`patches/`) aynı garantiyi **seçili modüllerde** kendimiz kurabiliriz. Orijinal Java kodunda sahipliği çıkarmak genel
durumda çözülemez; ama kodu sahiplik kurallarına uyacak şekilde değiştirip bir denetleyiciyle doğrulamak mümkündür.

1. **Sahiplik denetleyicisi (çevirici içinde):** Seçili sınıf kümesi (`owned-modules.json`) için bytecode veri akışı:
   - Her ayırma tek sahibe bağlanır. Sahip yerel değişken, alan veya parametre olabilir.
   - Taşıma (move): sahiplik başka sahibe geçince eskisi kullanılamaz (kullanırsa derleme hatası).
   - Ödünç alma (borrow): bir çağrı süresince salt okunur veya tek yazan; çağrı dışına sızamaz.
   - Sızma: statik alana, paylaşılan koleksiyona, başka iş parçacığına yazma yasak (hata).
2. **Hata raporu:** İhlal edilen kural, sınıf, metot, satır numarası. Biz o noktayı yamalarız (Java overlay'de yeniden yazarız veya Zig'de elle).
3. **Sonuç:** Denetleyiciyi geçen modülde sahip kapsamdan çıkınca silme kodu derleme zamanında üretilir (`defer free` gibi);
   bu nesneler GC yığınına hiç girmez, GC tarafından görülmez.
4. **Geçemeyen kod:** GC yığınında kalır (yedek). Modül listesi büyüdükçe GC'nin işi azalır.
5. **Sıra:** Ömür profilleyicisinden (bölüm 6) çıkan en çok ayıran modüller önce. Örnek adaylar (ölçüm doğrulayacak):
   gürültü/yoğunluk fonksiyonları, `PalettedContainer`/`BitStorage`, VoxelShape birleştirme, mesh üretimi, `Optional`/`Iterator` kalıpları.
6. **Doğrulama:** Dönüştürülen her modül için orijinal Java ile çıktı karşılaştırma (aynı tohum, aynı chunk) şart.
   Doğrulanmayan modül sahiplik moduna alınmaz.
7. **Bitiş durumu:** Hedef kodun %90-95'i sahiplik modunda veya bölgede, kalanı GC yığınında. %100 sahiplik uzun vadede
   modül modül mümkün; hemen ya da otomatik değil.

Bu bölüm 5.1'deki A ve B katmanlarının "elle taşınan" yoludur: analiz kanıtlayamıyorsa kodu kanıtlanabilir hale getiririz.


## 14. Derleme kapısı (kaldırıldı)
Tam derleme öncesi testleri zorunlu kılan kapı (betik, işaret dosyası ve koruma betiğindeki kuralı) kullanıcı kararıyla
silindi. Koruma betiği artık yalnızca şunları yapıyor: derleme çıktılarının commit edilmesini engeller, güvensiz git
işlemlerini reddeder, aynı anda tek ağır iş ve bellek tabanı. Kodlama bitince testler ve hata ayıklama aşaması başlar.


## 15. Ölçülen çöp kaynakları ve işlerin sırası (21 Kasım, gerçek profil + kaçış analizi)
İki oturumun profili (6951 MiB, çoğu açılış) ve `escape.py` sonuçları birleştirildi. Kapsama (iç içe sahiplik) analizi
eklendi ama kaçmayan nokta sayısını değiştirmedi (8639 -> 8653), çöp hacmine etkisi yok. Sebep: kaçışların %43'ü başka
metoda argüman, %35'i `areturn`; bunlar nesneyi başka koda veriyor.

| Hacim | Ayıran | Ne | Çözüm yolu |
|---|---|---|---|
| %12,2 | `[C` <- `Arrays.copyOf` | metin tamponları büyütme | önceden ayrıştırılmış kaynak, Latin-1 String |
| %7,5 | `[C` <- `BufferedReader.<init>` | okuyucu başına 8192 karakterlik tampon (16 KB) | önceden ayrıştırılmış kaynak; tek tampon yeniden kullanımı |
| %6,1 | `[C` <- `String.<init>` | metin kopyaları | Latin-1 String |
| %15,6 | DataFixer: `Func`, `Id`, `RewriteResult`, `BitSet`, `View` | açılışta kurulan kural ağacı | görüntü yığını (derleme zamanında) veya tembel kurulum |
| %8,5 | `Optional.of/ofNullable` | değer nesnesi | satır içi + nesne parçalama |
| %5,3 | iteratörler (`ArrayList`, `EnumSet`) | `for-each` | çağıranın bölgesinde ayır / satır içi |
| %5,0 | `[F` <- `DensityBuffer.<init>` | dünya üretimi tamponu | havuz / bölge |
| %3,8 | `[B` <- `HeapByteBuffer` | NIO tamponu | havuz |

Sonuç: ayırma hacminin yarısından fazlası **açılışta** (JSON okuma + DataFixer) ve derleme zamanında önceden hazırlanabilir
türden. Bu yüzden sıra: (1) görüntü yığını ve kaynak önişleme, (2) DataFixer'ı derleme zamanına veya tembel kuruluma
almak, (3) Latin-1 String, (4) iteratör/Optional için satır içi ve çağıran bölgesi, (5) dünya üretimi için havuz ve bölge.


## 16. DataFixer bulgusu ve görüntü yığını uygulama tasarımı (21 Kasım)

### 16.1 DataFixer: koddan çıkanlar
- `DataFixers.<clinit>` hem `DATA_FIXER` hem `FILE_FIXER`'ı **hevesli** kuruyor: yüzlerce `addSchema`/`addFixer`, sonra
  `DataFixerBuilder.build()` ve `FileFixerUpper$Builder.build(...)`. İlk erişimde (`getDataFixer`, `getFileFixer`,
  `optimize`) çalışıyor. Başlatıcı `Main` ve `LevelStorageSource` bunlara başlangıçta dokunuyor.
- `DataFixers.optimize(Set)` yalnızca asenkron bir ön ısıtma. `Main.main` bunu açılışta çağırıyor.
- `DataFixerUpper.update(...)` sürümler eşitse hiçbir kural çalıştırmadan girdiyi döndürüyor (bytecode: `if (version >= newVersion) return input`).
  Yani yeni dünyada kurulan kural ağacı hiç kullanılmıyor.
- `FileFixerUpper` bir sınıf (arayüz değil), `DataFixerBuilder$Result` ister: vekil (lazy proxy) yazmak kolay değil.
- Sonuç: DataFixer'ı **tembel yapmak** işi ötelemekten öteye gitmez (maliyet ilk dünya açışına kayar, S24'te dünya
  üretimiyle çakışıp tepe belleği artırır). Gerçek çözüm: `DataFixers.<clinit>`'i **derleme zamanında çalıştırıp** sonucu
  görüntü yığınına koymak. Bu yüzden DataFixer ayrı bir iş değil, görüntü yığınının (11.3) ilk müşterisi.

### 16.2 Görüntü yığını: gereken parçalar
Bilinenler (koddan): nesne başlığı 16 bayt (`class*`, `hash`, `gc_bits`); `Class.id` = `all_classes` indeksi (işaretçi
<-> kimlik çevrimi var); `Class.static_refs` sınıfın referans statiklerini listeliyor (GC bunları kök sayıyor);
`intern_map` internlenmiş Stringleri tutuyor; küçük nesneler boyut sınıflı slab'larda, büyük nesneler `malloc`ta.

1. **Yazıcı (x64 yapıcı ikilisinde, `runtime/image.zig`):** İstenen sınıflar başlatıldıktan sonra köklerden (seçilen sınıfların
   `static_refs` değerleri, intern tablosu, sınıf aynaları) nesne grafını gezer. Her nesne için (boyut, sınıf kimliği,
   gövde) yazar; referanslar dosya içi ofsete çevrilir. Dışlanan türler (iş parçacığı, monitörlü nesne, yerel tutamak,
   `identityHashCode` kullanılmış ve sırası önemli koleksiyon) bulunursa o sınıf görüntüden çıkarılır (başlatma izleyicisi, 11.3).
2. **Yükleyici (`image.zig`, motor başlarken):** Dosyayı sabit adrese `mmap`ler (MAP_FIXED_NOREPLACE, olmazsa yeniden
   konumlandırma), sınıf kimliklerini işaretçiye çevirir (tek geçiş, yalnız sınıf alanı), görüntüdeki sınıfları "başlatıldı"
   işaretler ve statik referans alanlarını doldurur, intern tablosuna ekler.
3. **GC ile ilişki:** Görüntü bölgesi kök olarak taranır, ancak yalnızca yazılan sayfalar (4.1, `mprotect` + SIGSEGV ile
   kirli sayfa takibi) taranır. İlk sürümde (basit): tüm bölge kök, taranır ama görüntü bölgesi **hiç boşaltılmaz** (süpürme yok).
   Bu bile açılıştaki 7-8 GB ayırmayı yok eder; tarama maliyetini sonra MMU yöntemiyle düşürürüz.
4. **Çevirici tarafı:** `init-policy.json` (hangi sınıflar derleme zamanı başlatılır), üretilen sınıf başlatıcısında "görüntüden
   geldi" yolu (`clinit` atlanır), statik alanların görüntüden doldurulması. Statik ilkel alanlar da görüntüye girer.
5. **Derleme hattı:** Aşama 1: x64 yapıcı derle (aynı kaynak). Aşama 2: yapıcıyı çalıştır -> `image.bin`. Aşama 3: arm64 `.so`
   derle (sınıf düzeni özetini yapıcıyla karşılaştır). APK `image.bin`'i assets'e koyar, motor `mmap` eder.

### 16.3 Uygulama sırası (her biri kendi başına doğrulanabilir)
1. `image.zig` yazıcı + yükleyici, x64'te boşta çevrim testi (nesne grafı yaz, aynı süreçte yükle, yapısal eşitlik).
2. Sınıf başlatma izleyicisi (taint) ve `init-policy.json` üretimi.
3. Çeviricide başlatma atlama ve statik doldurma.
4. İki geçişli derleme betiği.
5. DataFixer + kayıt defterleri ilk müşteri, ölçüm (açılış süresi, ayırma hacmi, canlı veri).
