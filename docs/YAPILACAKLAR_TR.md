# Yapılacaklar listesi (25 Kasım 2026)

Kural: her iş bir anahtarla açılır, eski yol yerinde kalır. Test (JVM'e karşı) + oyun koşusu + ölçümü geçen iş `build.sh`'nin
varsayılanı olur ve burada "varsayılan" diye işaretlenir. Her iş bütün platformlar için düşünülür (Linux, Windows, Android).
Ölçümler ve ayrıntı: `docs/PIPELINE_VE_YOL_HARITASI_TR.md` §22-24.

Her büyük değişiklikten sonraki oyun derlemesinde ekran görüntüleri (`tools/headless_play.sh` -> `<build>/shots`: ana menü,
dünyaya giriş, yürüyüş) kullanıcıya gönderilir.

Durum: `bitti` / `sürüyor` / `bekliyor` / `ertelendi` / `yapılmayacak`.

## A. GC'siz / Rust bellek yönetimi

| # | iş | anahtar | durum |
|---|---|---|---|
| A0 | Profil aracı adres dönüşümü | - | bitti |
| A1 | Bölge: 8 sınıf (Vec3, AABB, Optional, Pair, DataResult$Success, BlockPos, CubePointRange, OffsetDoubleList) | `MONO_REGION_CLASSES` | sürüyor: lambdasız temiz; oyun derlemesinde ölçülecek |
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
| C9 | Defteri tamamen kaldırmak: yığın izi yalnızca hata anında, makine yığınından (çerçeve işaretçileri) + satır içine alınmış çağrı bilgisi (DWARF) + üretilen `.rs` satırı -> Java sınıf/metot/satır tablosuyla. HotSpot da böyle yapar. Ölçüm: defter yokken fib(40) opt 3 265 ms (HotSpot 401) | - | bekliyor: büyük iş, en büyük hız kalemi |
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
| D6 | Samsung S26 / Android başlangıcı | bekliyor (gün sonu) |
| D7 | Geçen anahtarları `build.sh` varsayılanı yapmak | her iş geçtikçe |
