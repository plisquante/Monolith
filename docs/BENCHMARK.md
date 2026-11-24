# Kıyaslamalar

> **Tarihî ölçümler (Zig dönemi).** Aşağıdaki sayılar Zig arka ucu ve `runtime/*.zig` modülleri içindir; bugünkü Rust çalışma zamanı ve TouchCore çekirdeği (`android/touchcore/native`) için yeniden ölçülmedi. Güncel yapı süresi ölçümü: [`BUILD_PERF.md`](BUILD_PERF.md).

## 1. GC'siz istemci çekirdeği alt sistemleri (`-O ReleaseFast`, `aarch64` 10 çekirdekli Linux)

`python3 tools/microbench/run_client_core_bench.py` (`tools/microbench/client_core_bench.zig`, Zig 0.15.1 `-O ReleaseFast`) ile bu makinede doğrudan ölçüldü (`aarch64`, 10 çekirdek, ~7,4 GB RAM). Durdur-dünya işaretle-süpür GC'si ve dış LWJGL bağımlılıkları, 4 alt sistemli İstemci Çekirdeği ile değiştirildikten sonra:

| Alt sistem / Kıyaslama | İş yükü | Ölçülen sonuç | Verim / kare maliyeti |
|---|---|---|---|
| `ArenaAllocator` (`runtime/mem_pool.zig`) | `200` karede `2,000,000` x `64 B` tahsis (`resetRetainingCapacity`) | toplam `23.42 ms` | **`11.71 ns/op` (`85.4 M ops/s`)**, `0.0 ms` STW duraklaması |
| `SlabPool` 3 çağlı + OS haritası kaldırma (`runtime/mem_pool.zig`) | `64,000` x `64 B` yuva (`refillBatch` + `retireDeferred` + `advanceAndReclaim`) | toplam `4.63 ms`, `4000 KiB` geri kazanıldı | **`72.30 ns/op`**, **`0.0 ms` STW duraklaması** |
| `WorkerPool` (`runtime/thread_pool.zig`) | `250` toplu iş x `64` görev (`16,000` görev, 4 işçi, `WaitGroup`, toplu kilit + kilit sonrası yayın) | toplam `36.73 ms` (önce `3987.72 ms`) | **64 görevlik toplu iş başına `146.9 us` (108,5 kat hızlı)**, `%0` boşta döngü |
| Dünya Render 3 Aşamalı Eleme (`runtime/world_render.zig`) | `256` yonga bölümü (`16x16x16`), `1920x1080`, FOV `70°`, `200` kare | `256` bölümden `127` görünür kaldı | **`40.1 us/kare` (`0.040 ms/kare`)** |
| GPU Yönlendirmeli Örnekleme (`runtime/gpu_instancing.zig` + `runtime/rhi.zig`) | `2,048` varlık/parçacık (`64B`/`32B`), `8` ağ, frustum+Hi-Z+piksel eleme, opak/yarı saydam sıralı, `200` kare | `2,048`'den `1,499` kaldı, `7` çizim toplu işinde | **`287.0 us/kare` (`0.287 ms/kare`)** |

### Java -> Zig uçtan uca ve fark paketi (`27/27 GEÇTİ`, RTA Kapanışı ve S24 FE Optimizasyon Geçişi)

- **RTA kapanışı ve derleme hızlanması (`aarch64` S24 FE)**:
  - `PatchTarget`: `230` sınıf / `855` metot (önce `5,902` sınıf / `31,968` metot), `derleme 14.7s, yerel 0.06s`.
  - `JnaProxyFold`: `523` sınıf / `2,543` metot, `derleme 21.9s` (önce `624.8s`; **28,5 kat hızlı**), `yerel 0.05s`.
  - `Threads`: `derleme 28.4s` (önce `527.4s`; **18,6 kat hızlı**), `yerel 0.54s` (önce `100.45s`; **186 kat hızlı**).
  - `ClientCoreE2E`: `1,092` sınıf / `5,932` metot (`0 eksik native, 0 link hatası, 0 desteklenmeyen varhandle`), `derleme 37.9s` (önce `587.1s`; **15,5 kat hızlı**), `yerel 0.15s`.
  - Tam 27 testlik fark paketi (`python3 tests/run_tests.py`): **27/27 GEÇTİ** (`Arith` 13.4s, `IfaceInit` 12.6s, `Intrinsics` 13.3s, `LazyLeaf` 13.7s, `PatchTarget` 14.7s, `GcStress` 18.6s, `Exceptions` 18.8s, `Jni` 19.3s, `Misc` 21.6s, `Oop` 21.8s, `JnaProxyFold` 21.9s, `MHandles` 24.4s, `Strings` 25.0s, `StackWalk` 25.2s, `ModuleInfo` 26.5s, `Reflect` 26.8s, `Annot` 28.0s, `Threads` 28.4s, `ClientCoreE2E` 37.9s, `IoZip` 37.9s, `OptSuite` 41.9s, `Collections1` 43.3s, `Lambdas` 44.3s, `Platform` 45.7s, `NioFiles` 71.1s, `NetIo` 76.2s, `HttpFetch` 132.4s).

Kıyaslamaları yerelde yeniden çalıştırmak için:

    python3 tools/microbench/run_client_core_bench.py
    python3 tests/run_client_core_e2e.py -O Debug --shards 16 --shard-jobs 1

Donanım GPU/güç karşılaştırması için `tools/bench.py` ile tam istemci (`LIBGL_ALWAYS_SOFTWARE` ve `SDL_VIDEO_FORCE_EGL` kaldırılmış):

    python3 tools/bench.py --target jvm    --client-jar client.jar --libs LIBDIR --java JAVA25 --mcout OUT --out r/jvm
    python3 tools/bench.py --target native --mcout OUT --out r/native --no-lwjgl --graphics auto

---

## 2. Tarihî temel: Minecraft 26.3 ile HotSpot'ta vanilla (durdur-dünya GC kaldırılmadan önce, 4 vCPU Xeon)

Makine: bulut VM, 4 vCPU Intel Xeon @ 2.10 GHz, 15 GiB RAM, **GPU yok** (/dev/dri yok, NVIDIA/AMD aygıtı yok). Görüntüleme: Mesa 25.2.8 llvmpipe (yazılım OpenGL 4.5 core), Xvfb, pencere 854x480, `SDL_VIDEO_FORCE_EGL=1 LIBGL_ALWAYS_SOFTWARE=1` ile. Her iki hedef de aynı argümanlarla, aynı kaydedilmiş dünyaya (`--quickPlaySingleplayer "New World"`), oyun dizininin aynı temiz kopyasından başlatıldı.
Düzenek: `tools/bench.py` (her saniye /proc örnekler; kare hızı/tick için F3 ekran görüntüleri). Hedef başına bir koşu.

*Not: Aşağıdaki tüm satırlar, durdur-dünya GC'si kaldırılmadan **önce** ve Sıfır-LWJGL RHI, Vanilla'ya güvenli eleme ve GPU yönlendirmeli örnekleme tanıtılmadan **önce** alındı.*

Hedefler: vanilla = `client.jar` + kütüphaneler, OpenJDK 25.0.5 üzerinde (G1, -Xmx4g, JIT). Yerel = monolith derlemesi, ReleaseFast, kendi Zig arka ucuyla (`--backend native`); tam oyunun LLVM ReleaseFast derlemesi bu makinenin 15 GiB'ından fazlasını gerektirdi ve OOM öldürücü tarafından sonlandırıldı, bu yüzden optimize edilmiş yerel derleme burada ölçülemedi.

| metrik | vanilla (HotSpot) | yerel (monolith, kendi arka uç) |
|---|---|---|
| pencereye kadar başlatma | 13 sn | 215 sn |
| integrated server'ın dünyayı açmasına kadar başlatma | 10,8 sn | 197 sn |
| dünya oynanabilir olana kadar ("Loading terrain" kaybolur) | 21,7 sn | 645,6 sn |
| dünya hazır olmadan harcanan CPU saniyesi | ~50 | ~1464 |
| dünya hazır olduktan sonra ortalama CPU (bir çekirdek / 4 çekirdek) | %239 / %60 | %373 / %93 |
| başlatmadan sonra RSS ortalama / tepe | 2255 / 2297 MiB | 7108 / 7803 MiB |
| iş parçacıkları (en fazla) | 56 | 33 |
| FPS (F3, yazılım GL) | 17 | 1 |
| integrated server tick (F3, bütçe 50 ms) | 7,6 ms | 315 ms (koşular arasında 246-390) |
| pencere içinde kaydedilen bölge verisi | 4 dosya / 12,2 MiB | 1 dosya / 0,01 MiB |
| GPU kullanımı / VRAM / güç | yok (GPU yok) | yok (GPU yok) |

Çekinceler: her hedef için tek koşu; FPS ve tick tekil F3 okumaları (zaman serisi değil); oyuncu farklı yerlerde bitti (vanilla arazinin içinde, yerel ağaç yapraklarının içinde), bu yüzden render yükü farklı; yazılım görüntüleme aynı 4 çekirdeği oyunla paylaşır (llvmpipe iş parçacıkları), bu da ikisine de zarar verir. Yonga yükleme hızı yalnızca dünya-hazır süresiyle ve kaydedilmiş bölge verisiyle temsil edilir; yonga/sn sayacı kaydedilmedi.

## GC payı (yerel, aynı koşu koşulları)

`MONO_GC_VERBOSE=1` ile yerel derleme, dünya oynanabilir olmadan önce 9 toplama ve toplam 22,7 sn durdur-dünya duraklaması bildirdi; yani başlangıçtan dünyaya 657 sn'nin yaklaşık %3,5'i. Minimum toplama tetikleyicisini 64 MiB'tan 256 MiB'a (`MONO_GC_MIN_MB`) yükseltmek dünya süresini değiştirmedi (önce 646 sn, sonra 657 sn: gürültü) ya da RSS'i (ortalama 7,1 GiB). Yani çöp toplama darboğaz değil; HotSpot'a (21,7 sn) olan fark, üretilmiş kodun çalışma hızı (kendi Zig arka ucu, çağrılar arasında satır içi ya da yazmaç tahsisi yok, gölge yığın çerçeveleri) ile daha büyük bir heap ayak izinden (9. toplamada canlı veri 1,5 GiB) geliyor.
Başlık ekranındaki bellek dökümü: RssAnon 3,8 GiB (heap), RssFile 0,9 GiB (çalıştırılabilir sayfalar).

## Parçalı LLVM ReleaseFast derlemesi (aynı makine, aynı koşullar)

`--shards 32`, programı 32 ayrı derlenmiş nesneye böler; bu sayede LLVM ReleaseFast derlemesi bu 15 GiB'lık makineye sığar (2 shard paralelken tepe 5 GB; tek birimlik derleme 15 GB'ın üstünde öldürüldü). Aynı kıyaslama, aynı dünya:

| metrik | vanilla (HotSpot) | yerel, kendi arka uç | yerel, 32 shard LLVM ReleaseFast |
|---|---|---|---|
| çalıştırılabilir boyut | (JVM) | 5,6 GB | 1,35 GB |
| pencereye kadar başlatma | 13 sn | 215 sn | 27,5 sn |
| dünya oynanabilir olana kadar | 21,7 sn | 645,6 sn | 43,0 sn |
| FPS (F3, llvmpipe) | 17 | 1 | 4 |
| integrated server tick (bütçe 50 ms) | 7,6 ms | 315 ms | 49 ms |
| RSS ortalama | 2255 MiB | 7108 MiB | 7395 MiB |
| dünya hazır olduktan sonra ortalama CPU | bir çekirdeğin %239'u | %373 | %350 |
| dünya hazır olana kadar GC duraklaması | yok | 22,7 sn | 18,7 sn (15 toplama) |

Başlatma süresinde (2 kat), sunucu tick'inde (6 kat) ve bellekte (3 kat canlı veri: 1,8 GiB, vanilla ~0,9 GiB) hâlâ vanilla'nın gerisinde; FPS bu makinede yazılım görüntülemesi tarafından belirleniyor, adil karşılaştırılamaz.

## Parçalı LLVM derlemesinde art arda iyileştirmeler (aynı makine, koşu başına bir; gürültü yaklaşık ±2 sn)

| derleme | pencere | dünya oynanabilir | oynanabilire kadar GC duraklaması | RSS ortalama | F3 sunucu tick (tekil okuma) |
|---|---|---|---|---|---|
| 32 shard, seri GC | 27,5 sn | 43,0 sn | 18,7 sn | 7395 MiB | 49 ms |
| + paralel işaretle ve süpür | 26,8 sn | 45,1 sn | 6,5 sn | 7045 MiB | okunmadı |
| + sahne başına itable/checkcast önbellekleri, yoğun gölge yığın çerçeveleri, GC işçi geri çekilmesi | 23,6 sn | 42,3 sn | 6,4 sn | 7274 MiB | 72 ms |

Okuma: paralel GC duraklamayı 3 kat azalttı ama başlatmayı kısaltmadı: süreç CPU'ya bağlı (4 çekirdeğin yaklaşık %376'sı, öncekiyle aynı), yani duraklama kritik yolda değildi. Sahne başına önbellekler ve yoğun çerçeveler, oynanabilire kadar süreyi yaklaşık %6 iyileştirdi; bu, tek koşunun gürültüden ayırabildiği aralıkta. Vanilla'ya olan fark (oynanabilir 21,7 sn, 17 fps, 7,6 ms tick, bu makinede 2,3 GiB RSS) hâlâ açık.

## Optimizasyon ilerlemesi, ikinci tur (aynı makine, parçalı LLVM ReleaseFast, llvmpipe)

| Derleme | Dünya hazır | RSS ortalama | GC duraklaması (12 GC) |
|---|---|---|---|
| tembel çerçeveli yaprak metotlar | 42,3 sn | 6603 MiB | 4,5 sn |
| + JDK intrinsics (Math/Integer/Long/Float/Double statikleri satır içi), SimpleBitStorage yaması | 36,5 sn | 6660 MiB | 4,0 sn |
| + satır içi null/sınır denetimi yardımcıları | 33,5 sn | 6832 MiB | 3,8 sn |
| + küçük nesne sıfırlamasını satır içine alma, daha fazla intrinsics, sınıf aramasını karma tablosuyla yapma | 33,3 sn | 6843 MiB | 4,5 sn |
| vanilla (HotSpot, aynı makine, önceki koşu) | 21,7 sn | 2255 MiB | yok |

Aynı derlemenin `MONO_PATCH_VERIFY=1` koşusu: `SimpleBitStorage.get/cellIndex`'te uyuşmazlık yok (set void döndürür ve kendi kendini denetlemez). Her satır bir koşu; yalnızca tembel çerçeve değişikliği dünya-hazırı 0,7 sn kaydırdı (gürültü).
Bu turdan önceki profil (gdb örneklemesi, 60 örnek, yalnızca meşgul iş parçacıkları): yonga üretimi baskın (NoiseBasedChunkGenerator buildTerrain/doFill, MaterialSystem.buildSurface, BiomeManager) ve sembol tablosu üzerinden çağrılan küçük erişimciler (tüm sınıf referanslarının %51'i bir shard sınırını aşıyor, bu yüzden LLVM bunları satır içine alamıyor). Sınıfları referans sayılarına göre kümelemek, çevrimdışı bir simülasyonda bu payı düşürmedi ve bırakıldı.

Son satır hiçbir ölçülebilir şeyi değiştirmedi (33,5 sn'den 33,3 sn'ye; koşular arası gürültü yaklaşık 1 sn). Son derlemenin başlatma aşamaları: oturum kilidi 16,1 sn (vanilla 10,8 sn), pencere +2,2 sn (vanilla +2,2 sn), dünya 15,0 sn (vanilla 8,7 sn).

## Çağrı maliyeti mikro kıyaslaması (tools/microbench/CallBench.java)

Parçalı (3 shard) LLVM ReleaseFast ile HotSpot 25, aynı makine, tek koşu: özyinelemeli `fib(40)`, 4e8 arayüz çağrısı (iki alıcı sınıfı), 8e8 alan atama ve getter yinelemesi.

| | HotSpot | Monolith | oran |
|---|---|---|---|
| fib(40) | 401 ms | 1066 ms | 2,7x |
| arayüz çağrı döngüsü | 309 ms | 1304 ms | 4,2x |
| atayıcı + getter döngüsü | 294 ms | 2000 ms | 6,8x |

Getter döngüsü monolith tarafından zaten satır içine alınmış, yine de yineleme başına ~2,5 ns tutuyor. Sökümde akümülatör ve satır sayacının yığın belleğinde yaşadığı görülüyor (yineleme başına okuma-değiştirme-yazma zinciri), ayrıca gölge yığın satır yazımı, `R[]` üzerinden referans yuvası kopyaları ve güvenli nokta yoklaması. Bu, üretilen kod biçiminin yapısal maliyeti (metot başına etiketli bir switch, bellek dizisinde referans yuvaları, çerçeveler); profilin düz olmasının nedeni de budur: tek bir metot baskın değil, her metot HotSpot'un derlenmiş kodundan 2-7 kat yavaş. Tek tek çekirdekleri yamalamak bunu kapatamaz; kod biçimini değiştirmek kapatabilir (yazmaçta yaşayan yerel değişkenlerle yapılandırılmış döngüler, yığın taraması ile gölge yığın olmadan referans yuvaları, shard'lar arası satır içi).

Satır-sakla-fırlatırken değişikliği (`rt.nnF` ve benzerleri): dünya hazır 33,7 sn (derlemenin hemen ardından ilk koşu 42,1 sn ölçtü, soğuk önbellek sapması; aksi hâlde koşu gürültüsü ~1 sn); yani oyunda ölçülebilir bir kazanç yok, halbuki satır yazımları elle kaldırıldığında çağrı maliyeti mikro kıyaslaması yaklaşık %20 iyileşmişti.

Tur 3 (satır içi checkcast/itable hızlı yolları, Level/ChunkAccess/LevelReader getMinY, Holder.value, Level.dimensionType için dünya yüksekliği yamaları): dünya hazır 30,8 sn (oturum kilidi 16,1 sn, pencere 18,3 sn), RSS 6959 MiB; `MONO_PATCH_VERIFY=1` koşusu: 0 uyuşmazlık. Vanilla: 21,7 sn.
