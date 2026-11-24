# Monolith: Rust arka ucu

Monolith'in tek arka ucu Rust'tır (Zig arka ucu kaldırıldı). Ön uç aynı kaldı (sınıf yükleme, erişilebilirlik, indy desugar, sınıf kütüphanesi kaplaması), çıktı Rust. Giriş noktası: `python3 -m monolith.rustgen --cp <dizin> --main <Sınıf> -o <çıktı> [--run] [--strict]`.
Testler: `python3 monolith/run_tests.py` (her `monolith/tests/*.java` HotSpot'ta ve Rust olarak çalışır; stdout ve çıkış kodu aynı olmalı).

## Yerleşim
- `monolith/rustgen.py`: bytecode'dan Rust'a. Tüm dünya: giriş noktası uygulamanın, kaplamanın ve JDK'nın erişilebilir her sınıfını ve metodunu çeker. Akış kontrolü `loop { match pc { .. } }`, istisnalar `?` ile `Result`, işleyiciler hata veren pc'ye göre yeniden dağıtılır.
- `monolith/rt/rt.rs`: nesne modeli (16 bayt başlık, `Class` tabloları: vtable, itab, arayüzler, alan tablosu), diziler, dizgiler, sınıf aynaları, kilitli sınıf başlatma, fırlatma kancaları.
- `monolith/rt/natives.rs`: kaplamanın yerel metotları (`n_<sınıf>__<metot>`).
- `monolith/rt/vh.rs`: VarHandle erişim modları. `monolith/rt/sync.rs`: monitörler, iş parçacıkları, park, uyku, kesme.

## Doğrulananlar (HotSpot ile birebir aynı)
Aritmetik ve akış kontrolü, nesneler, kalıtım, arayüzler ve default metotlar, diziler, statik başlatma, istisnalar (catch, finally, iç içe, nedenler), sınıf kütüphanesinden gerçek `String`/`StringBuilder`/`PrintStream`, indy dize birleştirme, lambdalar, record'lar, enum'lar, sealed switch, koleksiyonlar, akışlar, `String.format`, sayı ayrıştırma, iş parçacıkları, `synchronized` (blok ve metotlar), wait/notify, interrupt. Merhaba dünya 187 sınıf / 793 metot derliyor; bir koleksiyon/akış programı 1308 sınıf / 6031 metot.

## Bilinen boşluklar (üreteç çevrilemeyen kodu ve eksik yerel fonksiyonları stderr'e yazar; bunlara ulaşılırsa süreç 70 çıkış koduyla ve bir mesajla durur, hiçbir şey sessizce atlanmaz)
- Bellek: hiçbir şey serbest bırakılmaz (henüz GC ya da bölge yok). Uzun bir oyun oturumu için kullanılamaz.
- Yansıma: `Class.newField0`, `fields0`, `methods0`, açıklamalar, sabit havuzu; `getCallerClass` çağrı noktası başına bir ipucuyla yaklaşıktır (yalnızca bunu çağıran metotlar için). `ExecutorService`, `CompletableFuture`, `ConcurrentHashMap` birleştirme yollarını engeller (bkz. `monolith/pending/Mono7.java`).
- Yığın izleri: `Throwable.backtrace0` null döndürür (çerçeve yok).
- Referans dizileri tek bir çalışma zamanı sınıfını paylaşır: `ArrayStoreException` yok, `T[]` için `checkcast` gevşek.
- ServiceLoader sağlayıcı tablosu, MemorySegment VarHandle'ları, JNI, NIO, dosyalar, soketler, zip, yönetim yerel fonksiyonları.
- Grafik/ses katmanı (RHI, GL shim, SDL shim) ve Android launcher girişi taşınmadı.
- Eski Zig çalışma zamanının sınıf yükleyici, modül ve kaynak yönetimi Rust çalışma zamanına taşınmadı.

## Taşınacak yerel yüzey
Orijinal (Zig dönemi) çalışma zamanında 613 yerel giriş noktası var; yaklaşık 130'u Rust'ta yapıldı. En büyük gruplar: `NativeRhi` 159, UnixNativeDispatcher 49, `sun.nio.ch.Net` 40, Class 25, Unsafe 24 (çoğu yapıldı), UnixFileDispatcherImpl 19, UnixFileSystem 14, zip 20.

## Android kütüphanesi (`--android`)
`python3 -m monolith.rustgen --cp <jar'lar> --main <Sınıf> --no-lwjgl --android --shards 32 --shard-jobs 3 --opt 2 -o <dizin>`
`aarch64-linux-android` için `libminecraft_mono.so` derler: `mono_rt.rlib`, shard başına bir nesne, `libmc_main.rlib` (`pub fn mono_run`) ve `mono_android_entry(user, gameDir, assetsDir, input)` fonksiyonunu dışa aktaran üretilmiş bir `link_stub.rs` (cdylib kökü). Giriş, oyunu kendi 256 MiB iş parçacığında başlatır ve hemen döner; böylece Android UI iş parçacığı asla bloke olmaz. TouchCore ana makinesi (`libtouchcore_mono.so`), EGL, girdi ve yüzeyi, `natives_rhi.rs`'nin çözdüğü `tc_android_*` sembolleri üzerinden sağlar. JNA, libffi gerektirir; bu, resmi sürümden (sağlama toplamı sabitlenmiş) bir kez statik kütüphane olarak derlenir. `tools/mcbuild_rust.py --android`, jar, kütüphane ve asset indirmesini bu komutla birlikte sarar.

## Yamalar (oyun mantığı Rust'ta)
`patches/rust/manifest.json`, çevrilmiş metotları el yazımı Rust ile değiştirir, bkz. `patches/README.md`. 7 yama gönderiliyor (`SimpleBitStorage` get/set/cellIndex ve dünya yüksekliği zinciri); `python3 monolith/patch_tests.py` mekanizmayı HotSpot'a karşı, `MONO_PATCH_VERIFY` modunda denetler ve yanlış bir yamanın yakalandığını doğrular.

## Tahsis profili (GC'den çıkış)
`MONO_ALLOC_PROFILE=<dosya> ./run.sh`, sınıf başına ne kadar tahsis edildiğini ve ne kadar yaşadığını kaydeder (atlatılan toplama sayısı: 0, 1, 2, 3+), ayrıca örneklenen tahsis noktalarını. `python3 tools/alloc_report.py <dosya> --exe <program>` her sınıfı TEMP (genç ölür), MIXED veya LONG olarak sınıflandırır ve en çok tahsis eden metotları listeler. `docs/ALLOC_BASELINE.md` ilk ölçümdür (4 dakika oyun, Minecraft 26.3). Sınırlar: örnekleyici fiziksel çerçeveleri kaydeder; derleyicinin satır içine aldığı bir metodun tahsisi çağıranına yazılır; tahsis kancası, profilleme kapalıyken tek bir gevşek yüklemedir.

## Toplayıcı olmadan bellek: durum
4 dakikalık oyun üzerinde ölçüldü (bkz. `docs/ALLOC_BASELINE.md`, kararlı durum için `tools/alloc_report.py --base`): oyunun tahsis ettiği baytların %98'i kısa ömürlü. Her adım HotSpot'a karşı doğrulandı (`monolith/*_tests.py`) ve oyunda ölçüldü:

| Adım | Oyunda ölçülen etki |
|---|---|
| `DataFixers.optimize` atlandı (yama `datafix.rs`) | canlı heap 1 387 -> 543 MiB, en uzun GC duraklaması 8,1 sn -> 2,1 sn |
| for-each yineleyicileri döngü bitince geri verilir (`find_iterator_scopes`, `gc.rs` geri dönüşümü) | yineleyici heap tahsisi %89-98 azaldı, kararlı tahsis 44,0 -> 36,5 MiB/sn |
| BlockPos çöpü olmadan `ServerLevel.tickChunk` (yama `tick_chunk.rs`) | BlockPos hacminin yaklaşık beşte biri |
| `--inline --escape`: küçük çağrılanlar satır içine alınır, kaçmayan nesneler çağıranın çerçevesinde tutulur (`inline.py`, `escape.py`, `rt::stack_obj`) | bir sonraki ölçümden sonra `docs/ALLOC_BASELINE.md`'ye bakın |

Güvenlik ağları: `MONO_ARENA_CHECK=abort` her toplamada, hiçbir heap nesnesinin ya da statiğin bir çerçeveye ya da arenaya işaret etmediğini doğrular; `MONO_ARENA=0` ve `MONO_RECYCLE=0` karşılaştırma için mekanizmaları kapatır; `MONO_PATCH_VERIFY=1` yamaları denetler.
