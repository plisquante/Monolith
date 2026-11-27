# Yapılacaklar listesi (25 Kasım 2026)

Kural: her iş bir anahtarla açılır, eski yol yerinde kalır. Test (JVM'e karşı) + oyun koşusu + ölçümü geçen iş `build.sh`'nin
varsayılanı olur ve burada "varsayılan" diye işaretlenir. Her iş bütün platformlar için düşünülür (Linux, Windows, Android).
Ölçümler ve ayrıntı: `docs/PIPELINE_VE_YOL_HARITASI_TR.md` §22-24.
Tüm anahtarların listesi: `docs/ANAHTARLAR_TR.md`.

Her büyük değişiklikten sonraki oyun derlemesinde ekran görüntüleri (`tools/headless_play.sh` -> `<build>/shots`: ana menü,
dünyaya giriş, yürüyüş) kullanıcıya gönderilir.

Durum: `bitti` / `sürüyor` / `bekliyor` / `ertelendi` / `yapılmayacak`.

## A. GC'siz / Rust bellek yönetimi

| # | iş | anahtar | durum |
|---|---|---|---|
| A0 | Profil aracı adres dönüşümü | - | bitti |
| A1 | Bölge: kanıtlı liste 5 sınıf (Vec3, AABB, Optional, Pair, DataResult$Success) | `MONO_REGION_CLASSES` | 5 sınıf temiz. 8 sınıf (+BlockPos, CubePointRange, OffsetDoubleList) seyrek çöküyor: bölge açık 9 koşuda 3 çökme (SIGSEGV, bozuk işaretçi), bölge kapalı (`MONO_REGION=0`) 7 koşuda 0. Aynı ikili, yeni hız anahtarları açık. Üç sınıftan hangisi, bulunmadı |
| A1a | Bölge + lambdalar çökmesi (bayat referans) | - | bekliyor: tanı kipi (parça boşaltılmadan önce ona işaret eden yığın alanını bul) |
| A1b | Bölgeyi Windows'a taşımak (`VirtualAlloc`) | - | bekliyor (varsayılan olmadan önce şart) |
| A2 | Dizgi birleştirme tam boy tek dizi | `MONO_CONCAT=exact` | sürüyor: testler geçti, oyunda ölçülecek |
| A3 | `--escape --inline` yeniden ölçüm | derleme bayrağı | bekliyor |
| A4 | Sık dizilere iş parçacığı tamponu (worldgen `[D`/`[F`, ışık kopyaları, `EnumSetIterator`) | - | bekliyor |
| A5 | Chunk verisi Rust sahipliğinde | - | bekliyor |
| A6 | Bölge parçalarını toplu iade (Linux `madvise`, Windows `VirtualFree`) | - | bekliyor |
| A7 | En yüksek RAM (VmHWM) ölçümü | - | sürüyor: oyun derlemesi koşusunda |
| A8 | Aynı kalıptaki birleştirme fonksiyonlarını tekilleştirmek (kod boyutu) | - | bekliyor |

## B. GC duraklamaları

| # | iş | durum |
|---|---|---|
| B1 | Statikleri eşzamanlı taramak | bekliyor |
| B2 | Tembel kipte canlı tahmine yeni ayrılanları katmak | bekliyor |
| B3 | Slab kırpma iadesini duraklama dışına ertelemek | bekliyor |
| B4 | Geç kalan iş parçacığı tanısı | bekliyor |

## C. Hız

| # | iş | anahtar | durum |
|---|---|---|---|
| C1 | Ucuz gölge çerçeve (işaretçi + satır, kilitsiz) | `MONO_FRAMES=fast` | bitti (anahtarlı); oyunda ölçülecek |
| C1a | Çerçeveyi daha da inceltmek (dairesel tampon, derinlik denetimi) | - | yapılmayacak: ölçümde yalnızca ~%20 (fib(35) yükte 112 -> ~88 ms); dairesel tampon 16K'dan derin özyinelemeden sonra yığın izini bozar. Asıl yol C9 |
| C2 | Yığın denetimi çerçeveyle birleşik | `MONO_FRAMES=fast` | bitti (anahtarlı) |
| C3 | Arayüz çağrısı çağrı yeri önbelleği | `MONO_ICACHE=1` | bitti (anahtarlı) |
| C4 | `checkcast`/`instanceof` önbellek + tam sınıf hızlı yolu | `MONO_ICACHE=1` | bitti (anahtarlı) |
| C5 | `Math` doğrudan Rust işlemi | `MONO_INTRINSICS=1` | bitti (anahtarlı) |
| C6 | Gereksiz sınıf başlatma denetimlerini kaldırmak | `MONO_INIT_ELIDE=1` | bitti (anahtarlı) |
| C7 | Hafif kilit (`synchronized`) | - | bekliyor: önce oyundaki sıklığı ölçülecek |
| C8 | Ayırma hızlı yolunu satır içine almak | - | yapılmayacak: kod şişer, kazanç küçük; asıl iş GC'de düz ayırma tamponu |
| C9 | Defteri tamamen kaldırmak: yığın izi yalnızca hata anında, makine yığınından (çerçeve işaretçileri) + satır içine alınmış çağrı bilgisi (DWARF) + üretilen `.rs` satırı -> Java sınıf/metot/satır tablosuyla. HotSpot da böyle yapar. Ölçüm: defter yokken fib(40) opt 3 265 ms (HotSpot 401) | - | kapatıldı (27 Kasım 2026): `MONO_FRAMES=dwarf` yalnızca elle açılır, varsayılan `fast`. Ölçüm: dwarf 157/188/144 ms, fast 183/160/163 ms (hız kazancı yok). Çıktı HotSpot ile aynı değil (satır numarası, fazladan `Fib.main` çerçevesi); `StackWalk` başarısız. Düzeltme yapılmadı |
| C10 | Paket bazlı parçalama + aynı parçada sınıflar arası doğrudan çağrı | `MONO_SHARD=pkg` | bitti (anahtarlı); test sürüyor |
| C11 | Hedef işlemci ayarı (taban seviye ya da çalışma anında seçim) | - | bekliyor |
| C12 | `--opt 3` + HotSpot ile yan yana hız ölçümü | - | bekliyor |
| C13 | Döngüleri gerçek Rust döngüsü olarak üretmek | - | bekliyor (büyük iş) |
| C14 | PGO | - | bekliyor |
| C15 | `ClassCastException` mesajına HotSpot'un modül/yükleyici ekini eklemek (davranış eşitliği) | - | bekliyor |

## D. Diğer

| # | iş | durum |
|---|---|---|
| D1 | mimalloc v3 | ertelendi (kullanıcı kararı) |
| D2 | LTO | ertelendi (kullanıcı kararı) |
| D3 | Bağlantı hatası sayacını sıfırlamak | yapılmayacak: kazancı yok |
| D4 | `String` tek baytlık sıkıştırılmış dizgi | yapılmayacak: Java kaplaması değişikliği gerektirir (Java kuralı) |
| D5 | Q9 (Flite) ve Q10 (Java kuralı) kararları | kullanıcıda |
| D6 | Samsung S26 / Android başlangıcı | derlendi ve dosya düzeyinde doğrulandı (API 37 araçları, NDK r30, 16 KB hiza); cihaz testi kullanıcıda. Ayrıntı: PIPELINE §26 |
| D7 | Geçen anahtarları `build.sh` varsayılanı yapmak | her iş geçtikçe |
| D8 | Ana menü sarı yazısı "işiniz rust gitsin :)" geçici: geri almak için `git revert 5689ba5` (tek dosya, `tools/mcbuild_common.py` `SPLASH_TEXT`); özgün yazılar jar'da duruyor, sonraki derlemede geri gelir | geri alınacak |
| D9 | Ana menü telif satırı "mojang yaptı, cemo ve plisquante modladı" geçici: geri almak için `git revert 1dd65a1` ve kısaltma commit'i (`tools/mcbuild_common.py` `CREDITS_TEXT`); özgün metin jar'daki dil dosyalarında duruyor, sonraki derlemede geri gelir | geri alınacak |
| D10 | Masaüstü başlatıcısı (`launcher/`): web arayüzü (`ui.html` + yerel HTTP sunucusu) ve Python betiği (`launcher.py`) kaldırılacak; başlatıcı tamamen Rust, arayüz Slint ile yazılacak. Aynı işlevler: `build/minecraft-*` derlemelerini bulma, kullanıcı adı, grafik API, oyun klasörü, ayarların kaydı (Linux `~/.config/monolith-launcher`, Windows `%APPDATA%`), OYNA (`run.sh`/`run.bat`), komut satırı kipi. Başlamadan önce Slint'in güncel sürümü, lisansı ve Windows/Linux desteği doğrulanacak | bekliyor (kullanıcı kararı) |
| D11 | Log4j'yi Rust'a taşımak: `slf4j` arkasına Rust yamalarıyla küçük bir günlükçü; `logs/latest.log` biçimi korunur. Kazanç: motor boyutu, açılış, bağlantı hatası 444 -> ~150; FPS kazancı beklenmiyor | sonra (kullanıcı kararı) |
| D12 | Wayland yüzeyi (`monolith/rt/desk/wayland.rs`) yazıldı ve başsız sway'de 14/14 geçti (PIPELINE §30). Kalan: GNOME ve KDE'de gerçek denemesi, Minecraft'la denemesi, yüksek çözünürlüklü ekran ölçeği (`wp_fractional_scale`), GNOME başlık çubuğu (libdecor), ölü tuşlar (compose), varsayılanı X11'den Wayland'a çevirme kararı (`linux.rs` `prefer_wayland`) | gerçek masaüstünde sınanacak |
| D13 | CI clippy adımı (`-D warnings`) 24 eski hatayla kırık: Türkçe çeviride `# Safety` başlıkları `# Güvenlik` olmuş (`missing_safety_doc`). Düzeltme: başlıkları `# Safety` yapmak ya da `lib.rs`'e `#![allow(clippy::missing_safety_doc)]` | bekliyor |
| D14 | Android'de çerçeve yığını için yerel TLS (`MONO_NATIVE_TLS=1`): CPU'nun yaklaşık %60'ı çerçeve kaydında (pthread_getspecific). PIPELINE §32 | S26'da ölçüldü (27 Kasım 2026, v8, global-dynamic): ortalama FPS 29,8 -> 60,8; 60. sn sonrası >100 ms kare gören pencere 22/76 -> 0/40; CPU'da Java kodu payı %25 -> %66. Kalan TLS maliyeti `[linker64]` %10,6 (TLSDESC çözücüsü). **varsayılan** (Android derlemesi; `MONO_NATIVE_TLS=0` eski yol, Android 9 ve öncesi için şart) |
| D15 | CPU profilinden sıradaki hedefler: `itab_find_ic_slow`, `init_enter`, `is_subclass`; dünya üretimi (DensityBuffer, OreFeature, Beardifier, PalettedContainer) | bekliyor: v8 profili sonrası |
| D16 | Dünya yükleme yavaşlığı (ilk ~55 sn: 1,7 sn donmalar, 6-8 FPS): CPU profili yalnızca yükleme penceresi için alınmalı | bekliyor |

## E. Sıcak alt sistemleri Rust'a taşımak (27 Kasım 2026 kararı)

Projenin amacı: Rust'ta daha hızlı çalışan Minecraft Java Edition. Oyunun tamamını başka bir motora taşımak yok (yeni bir
oyun olur, lisans ve uyumluluk sorunu). Yol: profilde en çok CPU yiyen alt sistemler tek tek elle yazılmış Rust yamasına
(`patches/rust/`) taşınır; hedef seviyeye gelince durulur.

Hedef seviye (Galaxy S26, Android, görüş mesafesi 12, simülasyon 8; kullanıcıyla kesinleşecek):
- yürürken ortalama 90 FPS ve 60. saniyeden sonra 100 ms'yi aşan kare olmaması,
- blok kırma/koymada FPS 60'ın altına düşmemesi,
- entegre sunucuda tick süresi (MSPT) 50 ms'nin altında (20 TPS),
- dünya yüklemede 1 sn'yi aşan donma olmaması.

Her yama için kural: Java ile aynı gözlemlenebilir davranış (istisnalar dahil; yalnızca yaygın durum, gerisi çevrilmiş
orijinale), bytecode parmak izi, `MONO_PATCH_VERIFY=1` ve fark testleri (`monolith/patch_tests.py`), telefonda önce/sonra
ölçümü (`MONO_DIAG=1 MONO_CPU_PROFILE=1`, `tools/cpu_report.py --slow`).

| # | alt sistem | iş parçacığı | durum |
|---|---|---|---|
| E1 | Blok durumu okuma: `PalettedContainer.get(I)`, `get(III)` | sunucu, işçi | yazıldı (a31bee5), sınanacak |
| E2 | `LevelChunk.getBlockState`, `LevelChunkSection.getBlockState` | sunucu | bekliyor |
| E3 | Çarpışma sorguları (`BlockCollisions.computeNext`) | sunucu | bekliyor |
| E4 | Işık motoru (blok ve gökyüzü ışığı yayılımı) | sunucu, çizim | bekliyor |
| E5 | Chunk bölüm örgüsü oluşturma (section compile) | işçi | bekliyor |
| E6 | Dünya üretimi: `DensityBuffer`, `Aquifer`, `GradientNoise`, `BiomeManager.getBiome`, `Climate` | işçi | bekliyor |
| E7 | GUI kesişim denetimleri (`GuiRenderState.hasIntersection`, `ScreenRectangle`) | çizim | bekliyor (F3 açıkken belirgin) |
| E8 | Yamalar için çalışma zamanı kapatma anahtarı (yeniden derlemeden devre dışı bırakmak, A/B ölçümü) | - | bekliyor |

## F. Fabric + shader yolu (27 Kasım 2026 kararı)

Sıra: (1) E bölümündeki sıcak alan yamaları, (2) F, (3) yeniden sıcak alan yamaları, (4) final.

| # | iş | durum |
|---|---|---|
| F1 | Fabric Loader + Mixin derleme anında uygulanır (çalışırken bytecode değişmez: AOT); dönüştürülmüş sınıflar Rust'a çevrilir. İlk hedef: Fabric API ile masaüstünde açılış | bekliyor |
| F2 | wgpu çizim arka ucu (Minecraft arka uç arayüzü Rust'ta); GL arka ucu yerinde kalır, anahtarla seçilir. Önce masaüstü Vulkan, sonra Android | bekliyor |
| F3 | Gölgelendirici çevirisi (GLSL -> naga -> wgpu) | bekliyor |
| F4 | Sodium/Iris benzeri mod ve bir shader paketi | bekliyor |

## G. Derleme anında seçilen kipler (27 Kasım 2026 kararı)

Sıra: E (sıcak alan yamaları) -> F (Fabric; sonucu ne olursa olsun) -> G. Kip derleme betiğine parametre olarak verilir;
kipe göre açılış bayrakları ve kısıtlama yamaları motora eklenir. Kullanıcı derlenmiş kipi değiştiremez.

| kip | derleme anında belirlenen | kullanıcının yapamayacağı |
|---|---|---|
| **Tam** (varsayılan, bugünkü hal) | - | kısıtlama yok |
| **Yalnızca çok oyunculu** | sunucu adresi; isteğe bağlı doku paketi ve yükleme ekranı | ana menüye dönmek, başka sunucuya ya da tek oyunculuya geçmek. Bağlantı koparsa ana menü yerine yeniden bağlanma ya da kapanma |
| **Yalnızca tek oyunculu** | dünya (derlemeye gömülür, ilk açılışta oyun klasörüne kopyalanır), doku paketi, yükleme ekranı | dünyadan çıkmak, yeni dünya açmak, doku paketini değiştirmek. İlerleme kaydedilir (dünya her açılışta sıfırlanmaz) |
| **Fabric olursa (her kipte)** | mod listesi ve shader paketi | mod ya da shader eklemek, çıkarmak, değiştirmek |

Teknik yol: Minecraft'ın Quick Play açılış bayrakları (`--quickPlayMultiplayer <adres:port>`,
`--quickPlaySingleplayer <dünya>`): bağlantı ve dünya yükleme açılış yükleme ekranının arkasında başlar. Kısıtlamalar Rust
yamalarıyla (Bağlantıyı kes / Kaydet ve çık düğmeleri, ana menü, doku paketi ve dünya seçim ekranları).

| # | iş | durum |
|---|---|---|
| G1 | Derleme parametresi: `--mode full|multiplayer|singleplayer` (+ sunucu adresi / dünya klasörü / doku paketi) | bekliyor |
| G2 | Yalnızca çok oyunculu: Quick Play + menüden çıkış engeli + kopmada yeniden bağlanma | bekliyor |
| G3 | Yalnızca tek oyunculu: dünyayı gömme, ilk açılışta kopyalama, Quick Play, çıkış ve dünya seçimi engeli | bekliyor |
| G4 | Doku paketi ve yükleme ekranının derlemeye sabitlenmesi | bekliyor |
| G5 | (F'ye bağlı) mod ve shader listesinin sabitlenmesi | bekliyor |
