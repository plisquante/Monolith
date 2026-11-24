# Durum

Son güncelleme: 2026-11-15. Araç zinciri: rustc (stable), OpenJDK 25 (Temurin 25.0.5) sınıf kütüphanesi (JDK 21 de destekleniyor).

> **Kronolojik kayıt.** Bu dosyadaki "Zig" ifadeleri, Zig arka ucunun kaldırılmasından önceki durumu anlatır (arka uç
> kaldırıldı; güncel durum `docs/PROJE_OZETI.md` ve `.claude/ARCHITECTURE_MAP.md`'de). Güncel plan: `MASTER_PLAN_TR.md`.

## 1. Fark testi sonuçları

`python3 tests/run_tests.py`, her `tests/java/*.java` dosyasını javac ile derler, JVM üzerinde ve bir monolith yerel çalıştırılabilir dosyası olarak çalıştırır; stdout, ilk stderr satırı ve çıkış kodunun aynı olmasını ister. Son koşu: **JDK 25 sınıf kütüphanesine karşı 10/10 geçti** (Debug derlemeler, yani Zig güvenlik denetimleri açık).

| test | kapsadığı |
|---|---|
| `Arith` | int/long/float/double aritmetiğinin uç durumları, dönüşümler, NaN/±0/∞, `toString`/`parseDouble` üzerinden 2 000 rastgele double/float, `StrictMath` (fdlibm), `Integer`/`Long` bit işlemleri, `Math.*Exact` |
| `Strings` | birleştirme (indy), `StringBuilder`, arama/değiştirme/bölme/regex, Unicode dahil büyük-küçük harf eşleme, `intern`, string `switch`, metin blokları, karakter kümeleri, kalıplar |
| `Oop` | kalıtım, soyut/default/statik arayüz metotları, `super` çağrıları, alan gizleme, iç/yerel/anonim sınıflar, jenerikler + köprüler, record'lar, gövdeli enum'lar, `EnumMap/EnumSet`, sealed tipler, kalıp `switch` (`typeSwitch`), başlatma sırası |
| `Exceptions` | try/catch/finally anlamları, HotSpot mesajlarıyla çalışma zamanı istisnaları, try-with-resources + bastırılmış istisnalar, `ExceptionInInitializerError` → `NoClassDefFoundError`, derin özyineleme, `StackOverflowError`, satır numaralı yığın izleri, yakalanmamış istisna işleyicileri |
| `Lambdas` | tüm lambda/metot referansı biçimleri (statik, bağlı, bağsız, kurucu, dizi kurucusu), yakalamalar, kutulama/genişletme uyarlaması, serileştirilebilir lambdalar (`altMetafactory`), `Comparator` birleştiricileri, `Optional` |
| `Collections1` | `ArrayList/LinkedList/HashMap/LinkedHashMap/TreeMap/HashSet/TreeSet/ArrayDeque/PriorityQueue/BitSet`, `Arrays`, `Collections`, akışlar ve toplayıcılar, `Random` |
| `Threads` | `synchronized`, `wait/notify`, atomikler, `ReentrantLock`/`Condition` (Unsafe + park/unpark üzerinden AQS), `ConcurrentHashMap`, `ExecutorService`, `CountDownLatch`, `CompletableFuture`, kesmeler, `volatile`, `ThreadLocal`, daemon / daemon olmayan kapanış |
| `GcStress` | 1 ve 4 iş parçacığında tahsis dalgalanması (güvenli noktalı durdur-dünya GC), büyük diziler, `HashMap` büyümesi, zayıf referanslar |
| `Misc` | `tableswitch/lookupswitch`, etiketli döngüler, çok boyutlu diziler, `arraycopy` örtüşmesi, değişken argümanlar, aşırı yüklemeler, kutulama önbellekleri, `String.format`, `System.exit` durumu |
| `IoZip` | `FileInputStream/FileOutputStream/RandomAccessFile/File`, okuyucular/yazıcılar, `CRC32/Adler32`, `Deflater/Inflater`, `GZIP*Stream`, `Zip*Stream` |

## 2. Minecraft Java Edition 26.3 (istemci) analizi

Girdiler: `client.jar` (11 383 sınıf, sınıf dosyası sürümü 69 / Java 25) + 66 Linux kütüphanesi.
Bu dosyalardan türetilmiş hiçbir şey bu depoda saklanmaz.

| metrik | değer |
|---|---|
| client.jar içindeki sınıflar | 11 383 (1 793 arayüz), 98 739 metot, 1,91 M komut |
| `jsr/ret` (desteklenmiyor) | 0 |
| invokedynamic noktaları | 24 014 lambda, 5 941 record `ObjectMethods`, 2 201 dize birleştirme, 79 kalıp switch — hepsi `indy.py` tarafından ele alınıyor |
| yansıma çağrı noktaları (client.jar) | 30 |
| kütüphaneler | 31 789 sınıf, 3 903 yerel metot (yalnızca LWJGL OpenGL: 2 278) |
| **`net.minecraft.client.main.Main`'den RTA kapanışı** | **53 996 sınıf yüklendi, 38 885 örneklendi, 240 868 erişilebilir metot, 3,93 M komut** (50 sn) |
| tüm kapanış için üretilen Zig | 53 977 modül, 542 MB Zig, **0 kod üretim hatası** (108 sn) |
| erişilebilir yerel metotlar | 781, bunların 682'si henüz gerçekleştirilmedi |
| link hataları / eksik sınıflar | 137 / 20 — çoğu JDK 22–25 API'leri (`java.lang.classfile`, `Math.powExact`, …), çünkü derleme JDK 21'in `java.base`'ini kullandı; Minecraft 26.3 Java 25 gerektiriyor |
| desteklenmeyen `MethodHandle.invoke*` noktaları | 36 |

Alana göre eksik yerel metotlar: `org/lwjgl/opengl` 115, `sun/nio/ch` 95, `sun/nio/fs` 59,
`com/sun/jna` 56, `org/lwjgl/system/libffi` 39, `java/io` 38 (sonradan gerçekleştirildi), netty
epoll/kqueue/unix 70, `com/mojang/jtracy` 20, `java/util/zip` 17 (sonradan gerçekleştirildi), …

## 3. Minecraft'ın yerel olarak başlayabilmesi için gerekenler

1. Sınıf kütüphanesi girdisi olarak **JDK 25 `java.base`** (bir JDK 25 jmod indirin; boru hattı sürümden bağımsız).
2. **Üçüncü taraf yerel kütüphaneler için JNI köprüsü** (LWJGL, JNA, netty-epoll, jtracy, lz4):
   `Java_<sınıf>_<metot>` sembolleri için `extern` bildirimleri üretin ve `natives-linux` jar'larında gelen
   kütüphanelere bağlayın; `JNIEnv` fonksiyon tablosu Zig'de gerçekleştirilir
   (LWJGL çoğunlukla ilkel değerler/adresler geçirir; `GetDirectBufferAddress`, dize ve geri çağrı
   fonksiyonları kullandığı başlıca JNIEnv giriş noktalarıdır).
3. **NIO yerel metotları** (`sun.nio.ch` dosya kanalları / seçiciler, `sun.nio.fs` UnixNativeDispatcher):
   Minecraft kaynaklarını ve dünyalarını `java.nio.file` üzerinden okur.
4. `Class.forName`/`newInstance` ile erişilen sınıflar için **yansıma metaverisi**
   (Gson tip bağdaştırıcıları, `ServiceLoader`, log4j eklentileri) — derleme zamanı yapılandırması artı
   üretilmiş kurucu/alan erişimcileri.
5. Kütüphanelerin kullandığı **MethodHandle'lar** (36 nokta): üretilmiş bağdaştırıcı fonksiyonlarla desteklenen doğrudan metot tutamacı nesneleri.
6. **Derleme ölçeklendirme**: Zig, programı tek bir LLVM modülü olarak derler; 240 bin metot, optimize edilmiş derlemeleri yönetilebilir tutmak için programın ayrı derlenen birimlere bölünmesini (paketler arası C-ABI sınırları) gerektirir. Debug derlemeleri Zig'in hızlı kendi arka ucunu kullanır.
7. Kaynak erişimi: sınıf yükleyici kaplaması `getResourceAsStream` isteklerini çıkarılmış bir kaynak dizininden (`MONO_RESOURCES`) karşılar; asset'ler derleme zamanında çıkarılmalıdır.

## 4. Kapsam: her metodun çevrilmesi (yalnızca erişilebilir olanlar değil)

`python3 tests/transpile_all.py <arşivler>`, ön ucu ve Zig üretecini her sınıfın her metodu üzerinde çalıştırır:

| girdi | sınıf | metot | sonuç |
|---|---|---|---|
| JDK 21 `java.base.jmod` | 7 555 | 62 235 | 62 235 başarılı, 0 hata |
| Minecraft 26.3 `client.jar` + 66 kütüphane | 43 034 | 546 163 | 546 163 başarılı, 0 hata (138 sınıf yüklenemedi: Kafka/OSGi/JFR gibi isteğe bağlı bağımlılıklar sınıf yolunda yok) |

## 5. Performans (ilk ölçümler, optimizasyon aşaması başladı)

`GcStress` (1 + 4 iş parçacığında ≈44 M kısa ömürlü nesne, 300 bin girdili `HashMap`, zayıf referanslar),
ReleaseFast, 4 çekirdek:

| | süre | tepe RSS |
|---|---|---|
| HotSpot (OpenJDK 21, varsayılan G1) | 1,0 sn | 640–830 MB |
| monolith, nesne başına sistem tahsis edicisi | 8,6–10,6 sn | 710 MB |
| monolith, ayrık-uyumlu (segregated-fit) tahsis edici | 5,9–6,9 sn | 238 MB |

Uygulananlar: ayrık-uyumlu iş parçacığına özel tahsis edici, CHA devirtualizasyonu, atlanan sınıf başlatma denetimleri, yerinde süpürme, yalnızca `volatile` atomikler. Beklenen etki sırasına göre değerlendirilen sonraki adımlar:

1. **Gölge yığın maliyeti**: her çağrı bir çerçeve iter (TLS erişimi, yığın sınırı denetimi, güvenli nokta yoklaması) ve `R` referans dizisini sıfırlar; referanslar yazmaçlar yerine bellekte yaşar. Gölge yığını, ayrık-uyumlu heap üzerinde muhafazakâr yığın taramasıyla (blok/boyut sınıfına göre nesne başlangıcı araması) değiştirmek, LLVM'in referansları yazmaçlarda tutmasını sağlar ve çağrı başına muhasebeyi kaldırır. Tahsis edemeyen, çağrı yapamayan ya da fırlatamayan yaprak metotların hiç çerçeveye ihtiyacı yoktur.
2. Fidanlık (nursery) için **nesilsel / bump tahsisi** (çoğu nesne genç ölür).
3. Megamorfik görünen arayüz çağrıları için **satır içi önbellekler**; itable aramaları doğrusaldır.
4. **SIMD**: `Arrays.fill/equals/mismatch`, `String` taramaları ve `System.arraycopy` doğal `@Vector` adaylarıdır (HotSpot orada intrinsics kullanır).
5. Durdurulmuş dünyada **paralel işaretleme** (iş parçacıkları zaten kayıtlı/park edilmiş).
6. **Derleme ölçeklendirme**: programı ayrı derlenen birimlere bölerek LLVM'in 240 bin metotluk Minecraft kapanışını paralel optimize etmesini sağlamak.

## 6. Bilinen anlamsal farklar (belgelenmiş, kasıtlı)

* Yerel ayar (locale) verisi: yalnızca Locale.US / ROOT sembolleri (`DecimalFormatSymbols` kaplaması); CLDR verisi gömülü değil.
* `NullPointerException` mesajları "yardımcı" değil (HotSpot bunları çalışma zamanında bytecode'dan hesaplar).
* `Object.hashCode()` kimlik karmaları HotSpot'unkinden farklıdır (JLS'de belirtilmemiş).
* Sonlandırıcılar (`Object.finalize`) hiç çalışmaz; yumuşak referanslar zayıf referanslar gibi temizlenir.
* `System.gc()`, program `-Dmono.explicit.gc` ayarlamadıkça işlem yapmaz (Minecraft bunu her tick çağırır; bir toplama oyunu yaklaşık yarım saniye durdurur). Özellik ayarlıysa, dönmeden önce tam bir toplama çalıştırır ve WeakReference'lar HotSpot'taki gibi temizlenir (Rust arka ucu, her iki GC modu; `tests/java/GcStress.java` özelliği ayarlar).
  Zig arka ucunun `GcStress` testi hâlâ bir WeakReference hedefini canlı tutuyor: muhafazakâr yığın taraması ona bayat bir referans buluyor. Açık.
* `Class.getDeclared*` yansıması, dinamik sınıf tanımlama, gizli sınıflar, VarHandle'lar ve indirgenmiş invokedynamic bootstrap'leri dışındaki method handle'lar kullanılamaz.

## 7. Minecraft 26.3 yerel çalıştırma (kilometre taşı)

Yerel olarak derlenmiş istemci (Debug derleme, JDK 25 kapanışı, JVM yok) başlıyor, LWJGL/SDL3 üzerinden bir OpenGL bağlamı oluşturuyor, kaynakları yüklüyor ve Mesa llvmpipe ile Xvfb altında başlık/ilk çalıştırma ekranını çiziyor. Headless olarak yeniden üretmek için:

    Xvfb :97 -screen 0 1280x720x24+32 +extension GLX +render &
    SDL_VIDEO_FORCE_EGL=1 DISPLAY=:97 LIBGL_ALWAYS_SOFTWARE=1 MC_GRAPHICS=opengl ./run.sh

`SDL_VIDEO_FORCE_EGL=1` (libegl1/libegl-mesa0 gerekir) yalnızca Xvfb'de gereklidir: Minecraft bir sRGB çerçeve tamponu ister, Xvfb'nin GLX'i bunu sunmaz; gerçek X sunucuları ve Wayland buna ihtiyaç duymaz.
Ağ/ses olmadan beklenen günlük gürültüsü: Mojang servisleri kullanılamıyor, OpenAL aygıtı, anlatıcı (flite).
Vulkan arka ucu yalnızca sürücünün VK_KHR_surface sunduğu yerde seçilir.

## 8. Derleme modları ve ölçülen sayılar (Minecraft 26.3 kapanışı, 61 bin Zig dosyası, 904 MB kaynak)

| derleme | zig süresi | tepe derleyici RSS | başlık ekranı (Xvfb, llvmpipe) |
|---|---|---|---|
| Debug (kendi arka ucu, -fstrip) | ~3 dk | ~10 GB | ~5 dk |
| ReleaseFast, kendi arka ucu (`--backend native`) | ~3,3 dk | ~7 GB | ~3,4 dk |
| ReleaseFast, LLVM | 4,3 dk sonra >15 GB'da OOM öldürücü tarafından sonlandırıldı | >15 GB | yok |
| ReleaseSmall, LLVM | 4,2 dk sonra ~13 GB'da OOM öldürücü tarafından sonlandırıldı | >13 GB | yok |

LLVM tüm programı tek modül olarak optimize eder; 16 GB'lık bir makine bunu tutamaz. Bu yüzden `--backend auto` (varsayılan), LLVM derlemesini yalnızca makinede yeterli RAM varsa (10 000 üretilmiş dosya başına yaklaşık 4 GiB çarpı 1,6) seçer ve LLVM derlemesi öldürüldüğünde de kendi arka ucuna geri düşer. Küçük programlar (testler) her zaman LLVM kullanır: `GcStress` ReleaseFast 2,8 sn'de çalışır (HotSpot 0,94 sn, önceki monolith temeli 6,4 sn).

Bu turda uygulanan optimizasyonlar: yaprak metotlar (fırlatma/tahsis/çağrı/döngü yok) gölge yığın çerçevesini atlar; tahsis sayacı iş parçacığı başına toplu tutulur; küçük çalışma zamanı yardımcıları yalnızca kendi arka ucunda zorla satır içine alınır (LLVM altında zorlamak 3,5 kat daha yavaştı).

Neden ayrı derlenen nesnelere bölünmedi: hata birleşimleri döndüren Zig fonksiyonları nesneler arasında dışa aktarılamaz (`export` bir C çağrı kuralı gerektirir) ve sanal çağrılar Zig-ABI fonksiyon işaretçilerinin vtable'larından geçer. Bölme, C uyumlu bir metot ABI'si (hata birleşimleri yerine iş parçacığına özel bir bayrak olarak istisnalar) ve zayıf bağlı çalışma zamanı globalleri gerektirir; bu bir kod üretimi yeniden tasarımıdır, bir derleme betiği değişikliği değil.

## 9. Oynanış kilometre taşı

Yerel olarak derlenmiş Minecraft 26.3 istemcisi (JVM yok) artık: başlıyor, başlık ekranına ulaşıyor, Singleplayer'ı açıyor, bir dünya oluşturuyor (netty yerel kanalları üzerinden gömülü sunucu), yüklüyor ve Xvfb/llvmpipe altında (OpenGL 4.5 core, Mesa 25.2) oyun içinde çiziyor: arazi, ağaçlar, yaratıklar, HUD, eğitim bildirimleri, F3 kaplaması, klavye/fare girdisi. xdotool ile headless sürüldü (bkz. bölüm 7).

Yolda düzeltilenler (davranış JDK düzeyinde olduğu yerde hepsi fark testleriyle): default metotlu arayüz başlatma, hizasız Unsafe erişimi, http/https URL işleyicileri, adlandırılmış önyükleme modülleri (`java.base` derin yansımaya kapalı ama `java.*`/`javax.*` dışa aktarıyor, `jdk.unsupported` `sun.misc`'i açıyor), JNI `RegisterNatives`, yansıtmalı netty kanalı ve temizleyici sınıfları, JNA yapı tipleri.

Bu 4 çekirdekli, yalnızca llvmpipe olan makinede kendi arka ucuyla oyun içinde ölçüldü: yaklaşık 1 fps ve gömülü sunucu tick'leri ~390 ms (50 ms bütçeye karşı). Hem yazılım GL rasterleyicisi hem de optimize edilmemiş üretilmiş kod katkı yapıyor; gerçek bir GPU birincisini, LLVM Release ise ikincisinin çoğunu ortadan kaldırır (yeterli RAM'li bir makine gerektirir, bkz. bölüm 8).

## 10. Yama sistemi (patches/)

El yazımı Zig, hiçbir Mojang kodunu saklamadan sıcak metotları değiştirir: `patches/manifest.json` bir metodu (sınıf, metot, tanımlayıcı) ve `patches/*.zig` içindeki bir Zig fonksiyonunu adlandırır. Çevirici çevrilmiş orijinali `<ad>_orig` olarak tutar ve bir sarmalayıcı üretir; `MONO_PATCH_VERIFY=1` her ikisini de çalıştırır ve ilkel dönüşlerde bit düzeyinde bir fark olursa durdurur. Parçalı ve parçasız çalışır (yama dosyaları her shard'a kopyalanır; `P.C_*` referansları shard cephelerine eklenir). tests/java/PatchTarget ile doğrulandı (manifestteki yalnızca test yamaları, sınıf Minecraft derlemelerinde yok). Henüz doğrulanmayanlar: uyuşmazlık durdurma yolu, parçalı kopya yolu. Bkz. patches/README.md.

## 11. Performans çalışma günlüğü (bu dal)

Aynı 4 vCPU makinede, parçalı LLVM ReleaseFast, llvmpipe, dünya hazır (vanilla HotSpot: 21,7 sn):
43,0 sn (performans çalışmasının başlangıcı) -> 42,3 (tembel çerçeveli yapraklar) -> 36,5 (JDK intrinsics, SimpleBitStorage yaması)
-> 33,5 (satır içi null/sınır denetimleri) -> 33,3 (küçük nesne sıfırlama, sınıf arama karması) -> 33,7 (satır yalnızca fırlatan yolda saklanır)
-> ~31-33 (dünya yüksekliği yamaları, satır içi checkcast/itable). Koşular arası gürültü yaklaşık 1-2 sn.

Araçlar: `MONO_PROF=<dosya>` (süreç içi 1 kHz CPU örneklemesi), `tools/prof_report.py`, `tools/bench.py`, `tools/microbench/CallBench.java`, `MONO_PATCH_VERIFY=1`.

Bulgular: oyun profili düz (hiçbir metot ~%3'ün üstünde değil); çağrı ağırlıklı kod HotSpot'tan 2,7 ila 6,8 kat yavaş (docs/BENCHMARK.md); dünya üretimi sırasında 15 sn'de 140 bin futex çağrısı görülüyor (tek sıcak kilit sözcüğü, henüz tanımlanmadı); bellek yönetimi (tahsis, sıfırlama, GC işaretle ve süpür) CPU örneklerinin yaklaşık %10'u; yazılım görüntüleme iş parçacıkları (llvmpipe) yaklaşık %7 alıyor.

Açık yapısal seçenekler, hiçbiri başlamadı: metot başına tek etiketli switch yerine yazmaçta yaşayan yerel değişkenlerle yapılandırılmış döngüler; `R[]` bellek dizisi olmadan referans yuvaları (muhafazakâr yığın taraması; zayıf referans zamanlama farkları riski taşır); shard'lar arası satır içi (daha az, daha büyük shard ya da LTO gerekir, ikisi de 15 GiB derleme belleğiyle sınırlı); daha küçük nesne başlığı ve daha düşük RSS (vanilla için 2,3 GiB'a karşı 6,5-7 GiB).

## 12. GC'siz istemci çekirdeği, Sıfır-LWJGL RHI, dünya elemesi ve GPU örneklemesi

1. **GC'siz bellek (`runtime/mem_pool.zig`, `runtime/thread_pool.zig`, `runtime/rt.zig`)**:
   - Durdur-dünya işaretle-süpür GC'si (`collect`, `markAndSweep`, `traceParallel`, `sweepBlocks`), STW bariyerleri ve döngü güvenli nokta yükü kaldırıldı (`rt.safepoint()` artık bekleme gerektirmeyen bir no-op).
   - Yerine kilitsiz iş parçacığı başına bump `ArenaAllocator` (`pushArenaScope` / `popArenaScope`, `beginFrame` / `endFrame`) + 3 çağlı sakin ertelenmiş geri kazanımlı ayrık `SlabPool` ve düşük güçlü, talep üzerine uyanan `WorkerPool` getirildi.
   - JDK 25 `MethodHandleNatives` (`registerNatives`, `getNamedCon`, `init`, `expand`), `NativeImageBuffer.getNativeMap`, `MethodHandles.Lookup.IMPL_LOOKUP`/`checkAccess`, `MethodHandles` VarHandle birleştiricileri, `VarHandle` (`VarForm`, `AccessType`, `AccessMode`), `Class.getRecordComponents` ve Foreign `MemorySegment` `VarHandle` indirgemesi kapatıldı (`runtime/varhandle.zig`, `monolith/program.py`, `monolith/zigen.py`).
2. **Özel RHI (`runtime/rhi.zig`, `runtime/rhi_vk.zig`, `runtime/rhi_gl.zig`, `runtime/rhi_bridge.zig`, `runtime/java/mono/rhi/NativeRhi.java`, `tools/mcbuild.py`)**:
   - Dinamik Vulkan 1.1+ arka ucu (`libvulkan.so.1` yoklaması, `vkCmdDrawIndexed` / `vkCmdDrawIndexedIndirect` komut dağıtımı), katı OpenGL 3.3 Core'a otomatik geri düşüşle (`rhi_gl.zig`: üçlü tamponlu `GL_TEXTURE_BUFFER` TBO örnekleme + `glMapBufferRange` eşzamansız akış + Intel HD 5500 için `glDrawElementsInstancedBaseVertex`) ve headless komut doğrulama moduyla, ayrıca `tools/mcbuild.py` içinde `--no-lwjgl` ve `--graphics auto|vulkan|opengl` desteği.
3. **Vanilla'ya güvenli dünya çizim boru hattı (`runtime/world_render.zig`)**:
   - Değişmez `SectionSnapshot` eleme, vanilla tick/redstone/fizik/sunucu mantığına dokunmaz: 6 düzlemli Gribb-Hartmann frustum eleme, 6 yüzlü CaveCulling portal BFS (`computeSectionPortals`), muhafazakâr `HiZPyramid` örtme ve görüntü alanı piksel kapsamlı LOD eleme (`cullAndScheduleWorld`).
4. **GPU yönlendirmeli blok dışı örnekleme (`runtime/gpu_instancing.zig`)**:
   - Birleşik 64 baytlık `GpuInstance` / 32 baytlık `GpuParticle` sıkıştırma (`InstanceBatcher`); `BlockDisplay`, `ItemDisplay`, `TextDisplay`, yaratıklar, oyuncular, düşen eşyalar, sandıklar, tabelalar ve parçacıklar RHI dolaylı/örneklemeli çizim toplu işlerine aktarılır.
5. **Uçtan uca doğrulama (`tests/run_client_core_e2e.py`)**:
   - `python3 tests/run_client_core_e2e.py -O Debug --shards 16 --shard-jobs 1`: 594,6 sn'de geçti (Zig birim testleri `mem_pool` 2/2, `thread_pool` 1/1, `rhi` 3/3, `world_render` 4/4, `gpu_instancing` 6/6, 4 alt sistemli Zig E2E 8/8, `javac` + `monolith` 5901 sınıf / 31959 metot / 300 yerel metot, `0 missing-native`, `0 link-error`, `0 unsupported-varhandle` ve yerel Java'dan Zig'e E2E koşusu `JAVA_CLIENT_CORE_E2E_OK backend=3 vis=2 inst=6 batches=4 dtMs=5`).
   - Son `-O ReleaseFast` doğrulaması (`python3 tests/run_client_core_e2e.py -O ReleaseFast --shards 32 --shard-jobs 1`): tüm denetimler 2398,9 sn'de geçti (`JAVA_CLIENT_CORE_E2E_OK backend=3 vis=2 inst=6 batches=4 dtMs=2`, `0 missing-native`, `0 link-error`, `0 unsupported-varhandle`).
   - GC kaldırıldıktan sonra çok iş parçacıklı JVM fark gerilemesi (`MONO_TEST_SHARDS=16 python3 tests/run_tests.py -O Debug Threads`): `PASS` (`13 satır; jvm 0.78s, yerel 100.75s, derleme 534.6s`).
   - Ölçülen `-O ReleaseFast` alt sistem mikro kıyaslamaları (`tools/microbench/run_client_core_bench.py`): `ArenaAllocator` `6.68 ns/op` (`149.7 M ops/s`), `SlabPool` 3 çağlı `30.08 ns/op` (`0.0 ms` STW duraklaması), `WorldRender` 256 bölümlü 3 aşamalı eleme `30.0 us/kare`, `InstanceBatcher` 2048 varlık/parçacık eleme+toplu iş `264.4 us/kare`.

## 13. Kısıtlı donanımda doğrulama (2 vCPU / 1,9 GiB, 2026-11-16)

Bu checkout, 5–12. bölümlerin arkasındaki makineden çok daha küçük bir makinede (2 vCPU Intel Xeon @ 2,60 GHz, 1,94 GiB RAM, swap yok, 25 GB disk) denetlendi ve kısmen doğrulandı. Görev görev tam tablo için bkz. [`HARDWARE.md`](HARDWARE.md). Sonuçlar, dürüstçe:

* Burada doğrulananlar: tüm Python kaynaklarında `python3 -m py_compile` (temiz), tüm Python kaynaklarında `pyflakes` (temiz), `runtime/*.zig` üzerinde `zig ast-check` (temiz), tüm kabuk betiklerinde `bash -n` (temiz), `.claude/hooks/guard.py --self-test` (36/36) ve stdin/çıkış-2 protokolü, `tests/run_tests.py --min-avail-gib` reddetme yolu (3 ile çıkar, anında) ve bir tam fark testi: **Arith GEÇTİ** (JVM ve yerel aynı çıktı; 2 228 satır; jvm 0,16 sn, yerel 0,58 sn; zig Debug kendi arka uç derlemesi 495,0 sn; derleme sırasında yük ortalaması ~10, zig tepe RSS ~745 MB).
* Arith derlemesi sırasında makine fiilen doymuştu (etkileşimli komutlar zaman aşımına uğradı; kswapd aktif, `MemAvailable` en düşük ~700 MB'a indi). OOM öldürmesi olmadı — kendi arka ucu artı `-fstrip` onu ayakta tuttu. `docs/HARDWARE.md` ve yeni bellek tabanları (`MONO_TEST_MIN_AVAIL_GIB`, `MONO_MIN_AVAIL_GIB`, guard.py ağır iş tabanı) tam da bu hata modunu artık riske atmak yerine reddediyor.
* **Bu kutuda çalıştırılamaz, çalıştırılmadı**: fark paketinin geri kalanı, Client Core E2E, mikro kıyaslamalar, parçalı Minecraft derlemesi ve tam program Zig derlemesi. 5–12. bölümlerdeki tüm iddialar daha büyük makineden geliyor ve burada yeniden ölçülmedi. 1. bölümdeki "10/10 geçti" sonucu da bu denetimden öncesine ait.
* Bu denetimde düzeltilenler: eksik `.claude/hooks/guard.py` (gerçekleştirildi, kendi kendine test edildi), `tests/run_tests.py` bellek korumasından yoksundu (artık tabanın altında reddeder ve paketi durdurur), dokümanlar var olmayan bir `docs/learnings/` dizinine ve eski bir jmod gereksinimine başvuruyordu (düzeltildi), kullanılmayan Python içe aktarmaları/değişkenleri ve bir yer tutucu f-string (kaldırıldı; `pyflakes` temiz), `tests/__pycache__` ve bayat derleme ağaçları (kaldırıldı).

## 14. Açık denetimi ve sağlamlaştırma geçişi (2026-11-16, ikinci oturum)

*Açıklar* için depo çapında denetim (gizli boşluklar, özellik değil): güvenlik, taşınabilirlik, doğruluk. Tüm düzeltmeler yalnızca hafif denetimlerle doğrulandı; ağır kapılar (fark paketi, Client Core E2E, parçalı Minecraft derlemesi) bu kutuda donanım nedeniyle bloke — bkz. §13.

Bu geçişte düzeltilen ve doğrulananlar:

* **tools/mcbuild.py — ağdan alınan JDK hiçbir bütünlük denetimi olmadan çıkarılıyordu.** Artık: arşivin SHA-256'sı çıkarmadan *önce* doğrulanıyor (beklenen özet, Adoptium varlığının `.sha256.txt` eşinden; v3 `/checksums` API'si 404 döndürür ve kullanılmaz); uyuşmazlık → arşiv silinir ve koşu iptal edilir. `MONO_JDK_NO_CHECKSUM=1` belgelenmiş bir atlama yoludur. Tar çıkarma, Python ≥ 3.12'de `filter="data"` kullanır, aksi hâlde realpath-içerme + sembolik/sabit bağlantı hedefi denetimleri; zip çıkarma realpath içermeyi uygular (`_safe_extract_zip`). `extract_resources()` (istemci JAR'ı / yerel kaynaklar) bir zip-slip açığına sahipti: girdi adları `dest`'e karşı doğrulanır ve akışla kopyalanır. `fetch()` hata durumunda kısmi indirmeleri siler; sürüm manifesti önbelleğe alınır (`.tmp` yarışı yok); `ensure_jdk()` kurulu JDK'yı özyineleme yerine `<install>/bin/javac` ile bulur; yerel kütüphane taraması artık `.so` / `.dylib` / `.dll` kapsar; `read_jar_version()` ham bir traceback yerine açık bir mesajla başarısız olur; tüm ağ yardımcıları `User-Agent: monolith-mcbuild/<version>` gönderir (varsayılan python-urllib UA'sı Adoptium'un CDN'i tarafından 403 ile reddedilir).
  **Bu kutuda uçtan uca doğrulandı:** `ensure_jdk(25)` taze indirme → "JDK SHA-256 verified" → `~/.cache/monolith/temurin-25/` altında Temurin 25.0.5+8 (javac 25.0.5).
* **monolith/classpath.py — `jdk_home()` mcbuild ile kurulan JDK'yı bulamıyordu** (yalnızca `MONO_JDK_HOME` / `JAVA_HOME` / `/opt/jdk25` / `/usr/lib/jvm` aranıyordu), bu yüzden bir `mcbuild ensure-jdk` kurulumu `monolith build` için görünmezdi. Artık `~/.cache/monolith/temurin-*/**` de taranır. module-info ayrıştırma hataları artık sessizce başarısız olmak yerine stderr'e uyarı yazar.
* **monolith/__main__.py — otomatik arka uç seçimi.** Parçalı yol her zaman LLVM'i zorluyordu (Debug derlemeler OOM oluyordu); artık parçasız politikayı yansıtır (Debug/ReleaseSafe için kendi arka ucu ya da makinenin optimize eden derlemeler için LLVM'e sığmayacağı kanıtlandığında). Parçasız yol da artık Debug'ı LLVM'e sabitlemez; iki yol da aynı `_mem_total_gib()` tahmininden geçer. `fcntl` classlib kilidi artık isteğe bağlı (`msvcrt.locking` geri düşüşü, yoksa devam — çıktılar içerik adresli/değişmez).
* **runtime/mem_pool.zig — arena `alloc`, `align > 4096` için hizasız işaretçiler döndürüyordu** (artık `Allocator` sözleşmesine göre null döndürür); iki yolu da parçayı aynı şekilde serbest bırakan gereksiz `madvise(MADV_DONTNEED)` dalı kaldırıldı. `zig ast-check` temiz; `zig test -lc -O Debug` 2/2 geçti.
* **monolith/program.py** — ölü `synchronized` no-op'u kaldırıldı. **tools/bench.py** — `SIGKILL` artık varsayılmıyor (`SIGTERM`'e geri düşer). **tests/run_client_core_e2e.py** — artık bir makinenin JDK yolunu sabit kodlamıyor (env → `classpath.jdk_home()` → `~/.cache/monolith/temurin-*` ile türetir); adım başına zaman aşımı ekler (`MONO_E2E_TIMEOUT`, varsayılan 10800 sn).
* **Depo taşınabilirliği** — yazar makinesine ait sabit yollar ortadan kaldırıldı: `.claude/COMMON_MISTAKES.md` ve (yollar makineden bağımsız yapıldı: `/root/...` → `$HOME/...`, `MCOUT` bir `/tmp/claude-0/...` yolu yerine depo yerel `out/` ağacına geri düşer).
* **Hijyen taraması** — gizli bilgi kalıpları (PEM blokları, AWS/GitHub/Slack belirteçleri, JWT'ler, sabit kodlanmış parolalar): 0 eşleşme. Sabit kodlanmış kullanıcı makinesi yolu kalmadı.
* **Yeni `.github/workflows/ci.yml`** — sözdizimi/lint, `zig ast-check`, JSON geçerliliği, guard kendi testi, JDK 25 kurulumu ve bir fark testi (Arith). Ağır kapılar kasıtlı olarak CI'da değil; bkz. `docs/HARDWARE.md`. Yazıldı ama GitHub'da HENÜZ çalıştırılmadı (burada yerel çalıştırıcı yok).

Açıkça YAPILMAYAN / kalan boşluklar (büyük makine, GitHub CI ya da insan incelemesi gerektirir):

* Arith ötesindeki fark paketi, Client Core E2E, parçalı Minecraft derlemesi: bu kutuda çalıştırılamaz (§13). Bu oturumda Arith'i yeniden çalıştırma denemesi, zig Debug derlemesi sırasında `MemAvailable` ~34 MB'a düştüğünde **ajan tarafından iptal edildi** (zig tepe RSS ~1,5 GB); etkileşimli oturumu korumak için süreç öldürüldü ve `tests/build/` içindeki ~726 MB bayat derleme çıktısı silindi. §13'teki 745 MB tepe, mevcut koşullarda yeniden üretilemedi — ağır doğrulama daha büyük bir makineye ya da CI'a aittir.
* Ilkel dönüşlerin ötesinde yama doğrulaması (bkz. patches/README.md); parçalı yama kopya yolu hâlâ test edilmedi.
* `.claude/hooks/guard.py` o zaman bir `.claude/settings.json` içine bağlanmamıştı (kullanıcı kararına bırakıldı); sonraki geçişte açık kullanıcı isteğiyle bağlandı — bkz. §15 (madde 7).
* `tests/run_client_core_e2e.py` değişiklikleri yalnızca sözdizimi/lint doğrulamalı (`bash -n` sınıfı denetimler), çalıştırılmadı — test, belgelenmiş büyük makine profilini gerektirir.


## 15. Mimari ve entegrasyon açığı geçişi (2026-11-16, üçüncü oturum)

Amaç: yalnızca yapay testlerde/yalıtılmış modüllerde bulunan ama gerçek istemci akışından (`net.minecraft.client.*` — çizim/eleme/örnekleme/RHI/bellek yaşam döngüsü) erişilemeyen kodu bulmak ve bağlamak. Tüm analiz bu checkout'a karşı yapıldı; doğrulama yalnızca çeviri + statik denetimlerle (1,94 GiB kutu — tam derleme yok). Aşağıdakilerin hiçbiri oyun içi doğruluk iddia etmez; bkz. "Yapılmayanlar".

### Bulunan entegrasyon boşlukları (ve her biri için yapılanlar)

1. **Tüm Sıfır-LWJGL RHI köprüsüne hiçbir gerçek derlemeden erişilemiyordu.** `mono.rhi.NativeRhi` (`runtime/rhi_bridge.zig`'in Java JNI cephesi) yalnızca `tests/run_client_core_e2e.py` içinde derleniyordu; ne `monolith build` ne de `mcbuild.py --no-lwjgl` onu üretiyordu, bu yüzden `mono.rhi.NativeRhi` hiçbir istemciden bağlanamazdı — RHI/eleme/örnekleme yığını gerçek yolda ölü koddu.
   **Düzeltme:** `monolith/classpath.py: builtin_classes()`, `runtime/java/**/*.java` dosyalarını içerik anahtarlı bir önbelleğe derler (`~/.cache/monolith/builtin-<sha12>`, kilitli, damgalı, `MONO_NO_BUILTIN=1` devre dışı bırakır) ve `cmd_build` bunu uygulama sınıf yolunun başına ekler (JDK kaplamasından önce, böylece yerel metotlarımız her zaman çözülür). E2E düzeneği artık `ClientCoreE2E.java`'yı buna karşı javac ile derler (köprünün elle javac'ı yok).
   **Doğrulandı:** taze derleme + önbellek isabeti (~6 ms); `javap` tüm köprü yerel metotlarını gösterir; `ClientCoreE2E`'nin yalnızca çevirisi 5901 sınıf / 31959 metot / 300 yerel metot verir, **0 missing-native / 0 link-error** (değişiklik öncesi E2E defteriyle aynı sayılar), report.json'da `builtin_rhi: true`.
2. **Hiçbir şeyle eşleşmeyen manifest yamaları sessizce düşürülüyordu** — yazım hatalı bir yama girdisi sonsuza dek görünmez. **Düzeltme:** `monolith build` artık bunları olası nedenle `issues["unused-patch"]` olarak bildirir. **Doğrulandı:** `PatchTarget`'ın yalnızca çevirisi tam olarak 9 eşleşmeyen mojmap yamasını yazar; kullanılan 3 PatchTarget yaması listelenmez.
3. **Üretilen launcher, vanilla istemciyi tasarım gereği çökertiyordu**: `run.sh`/`run.bat` `--graphicsBackend <api>` geçiriyordu — vanilla `net.minecraft.client.main.Main` bilinmeyen argümanları reddeder. **Düzeltme:** arka uç `-Dmono.graphics.backend=<api>` olarak geçirilir (vanilla tarafından yok sayılır; mono farkında herhangi bir kod okur). Launcher'ın argüman listesi artık tam olarak vanillaninkidir.
4. **Arena kapsamı köprüsü hata modları sessizdi.** `NativeRhi.pushArenaScope()` 16 derinliğin ötesindeki kapsamları düşürüyordu ve `popArenaScope()` no-op'a taşıyordu — ikisi de uzun ömürlü bir istemcide yaşam döngüsü hatalarını gizler. **Düzeltme:** artık ikisi de açık bir mesajla `IllegalArgumentException` fırlatır. `submitSectionSnapshot` artık sıfırlama başına 65536 gönderimle sınırlı ve bir `droppedSections` sayacı var (köprüdeki tek sınırsız büyüme yolu); `shutdownClientRhi()` son bir sıfır-STW `rt.gc()` çağ geri kazanımı yapar.
5. **Uzun ömürlü nesne şişmesi için gözlemlenebilirlik yoktu.** Çalışma zamanında "slab havuzu / çerçeve arenası şu an ne kadar büyük?" sorusuna yanıt veren hiçbir şey yoktu; bu yüzden yavaşça sızdıran bir istemci Java'dan hata ayıklanamazdı. **Düzeltme:** altı yeni köprü yerel metodu, `NativeRhi.arenaBytes()` (çağıran iş parçacığının geçici arenası), `slabCommittedBytes()`, `slabReclaimedBytes()`, `frameEpoch()`, `threadCount()`, `droppedSections()` olarak sunulur.
   Not (önemli, tasarım gereği): çalışma zamanı *erişilemeyen* SlabPool nesnelerini güvenle serbest bırakamaz — bir GC olmadan hiçbir şey onların öldüğünü kanıtlayamaz ve `retireObject()`'in hiç çağıranı yok. Geçici Java işi `NativeRhi.pushArenaScope()/popArenaScope()` (O(1) geri sarma) içinde çalışmalıdır. Çağlar yalnızca `rt.beginFrame()` içinde ilerler; her iç içe kapsamda ilerletmek çok erken geri kazanırdı ve kasıtlı olarak BAĞLANMADI.
6. **İstemci oturum yaşam döngüsü kancaları yoktu** — köprü oyunla birlikte kurulup indirilemiyordu. **Düzeltme:** `patches/client_frame.zig` (+2 manifest girdisi): `net/minecraft/client/main/Main.main` ve `net/minecraft/client/Minecraft.run`, orig-geri düşüşle sarılır (öncesinde RHI başlat, sonrasında kapat, `MONO_RHI_STATS=1` ile istatistik çıktısı). **İsteğe bağlı:** `MONO_RHI=1` ayarlanmadıkça kancalar bir getenv'dir ve `orig`'i dokunmadan çağırır; RHI başlatma hatası bir yazdırılmış uyarıya düşer, asla durdurma olmaz. 26.x'te (gizlenmemiş üretim adları, mcbuild varsayılanı) `Main.main([Ljava/lang/String;)V` girişi eşleşir; eşleme adı uyuşmazlığında girdi etkisizdir ve artık `unused-patch` olarak bildirilir.
7. **`.claude/hooks/guard.py` hiç bağlanmamıştı** — hiçbir şey onu otomatik çalıştırmıyordu. **Düzeltme:** `.claude/settings.json` oluşturuldu: `Bash|Write|Edit|NotebookEdit` üzerinde `PreToolUse`, `python3 "$CLAUDE_PROJECT_DIR/.claude/hooks/guard.py"` çalıştırır (hata durumunda kapalı). Doğrulandı: kendi testi geçer; tam settings.json komutu `git status`'e izin verir (çıkış 0), `git push --force`'u engeller (çıkış 2), `out/` altına yazmaları engeller (çıkış 2), normal yazmalara izin verir (çıkış 0).

### Hâlâ bağlanmayanlar (ve neden — dürüstçe)

* **Oyun içi çizim döngüsü enjeksiyonu** (`LevelRenderer`/`GameRenderer` → `mono.rhi.NativeRhi`, her karede `SectionSnapshot`'ları ve örnekleri besleme): gerçek istemci jar'ının bir insanın önünde ya da Mojang eşlemeli bir istemcinin ve duman testi için bir GPU makinesinin olmasını gerektirir. Bu imzaların bu kutudan körü körüne yamalanması doğrulanamaz kod göndermek olurdu — işe yaramazdan beter. Köprü API'si, oturum kancaları ve istatistikler tam da bu iş için iskeledir.
* **`--no-lwjgl` oyun içi uçtan uca**: köprüye erişilebilse bile vanilla istemci `org.lwjgl.*`'i doğrudan çağırır (çizim yolundan tamamen erişilebilir); bu sınıflar `--no-lwjgl` ile ayıklanır ve RTA erişilemeyen/eksik referansları `NoSuchMethod` fırlatmalarına çevirir. Çalışan bir sıfır-LWJGL istemcisi, Java tarafında bir GL→RHI shim'ine (ya da `rhi.zig`'e yönlendiren Zig'de bir GL-ABI shim'ine) ihtiyaç duyar; yani istemcinin GL çağrılarını yeniden gerçekleştirmek — jar + GPU olan bir makinede başlı başına bir proje. Mevcut `--no-lwjgl` bayrağı bu yüzden çalıştırılabilir bir oyun değil, *yalnızca çeviri* iskelesi olarak belgelendi.
* **Android yüzey katmanı**: bu depoda hiçbir yerde yok (ANativeActivity/JNI yapıştırması yok, GL/EGL ya da Vulkan yüzey oluşturma yok). Birini körü körüne gerçekleştirmek düpedüz uydurma olurdu; yok olarak belgelendi. Gerçek bir taşıma ndk + bir cihaz/öykünücü + `rhi_vk.zig`/`rhi_gl.zig` için bir pencere/bağlam shim'i gerektirir.
* **RHI kareleri vanilla tarafından asla üretilmez**: istemcide hiçbir şey `NativeRhi.beginFrame` çağırmaz, bu yüzden çağ geri kazanımı ve `frameEpoch()` vanilla bir oturumda düz kalır (oturum kancaları yalnızca istemci ömrünü sarar). İstatistik yerel metotları bu durumu doğru bildirir; etkinlik uydurmazlar.
* **Tam derlemeler** (paket, Client Core E2E, parçalı Minecraft derlemesi): bu kutuda çalıştırılamaz (§13/§14). `tests/run_client_core_e2e.py` yerleşik köprüye göre sadeleştirildi ve pyflakes temiz; ÇALIŞTIRILMADI.

### Burada yapılan doğrulama

`py_compile` + `pyflakes` temiz (dokunulan tüm Python); `bash -n` temiz; `zig ast-check` temiz (`runtime/*.zig`, `patches/*.zig`); yama + çalışma zamanı grafiğinin tam `zig test -lc -O Debug`'ı: **11/11 geçti** (4 yeni `client_frame` yaşam döngüsü testi + önceki 7 çalışma zamanı testinin hepsi); 4 alt sistemli Client Core E2E (`tests/e2e_client_core_test.zig`, Debug): **8/8 geçti**; `rhi_bridge.zig` ve `client_frame.zig`'in gerçek çıktı düzenine karşı `zig build-obj`'i: temiz; JSON geçerli (`patches/manifest.json`, `.claude/settings.json`, `report.json`); `guard.py --self-test` GEÇTİ; `builtin_classes()` taze derleme + önbellek isabeti; yalnızca çeviri duman testleri: `PatchTarget` (unused-patch listesi tamam, yerleşik köprü bağlandı) ve `ClientCoreE2E` (`0 missing-native/link-error`, `builtin_rhi: true`); `MONO_NO_BUILTIN=1` ile kapatma anahtarı duman testi (raporda `builtin_rhi: false`; analiz tek bir yüksek sesli `link-error: class not found: mono/rhi/NativeRhi` ile durur — hata açık, asla sessiz değil).
Zig birim testlerinin ötesinde hiçbir yerel kod çalıştırılmadı — çevrilmiş çıktılar analiz edildi, çalıştırılmadı.

### Bu kutuda aritmetik fark testi (JVM'ye karşı yerel, 1 GiB tavan)

Görev: bu kutunun OOM tarafından öldürülememesi için sert 1 GiB bellek tavanıyla `tests/run_tests.py Arith`'i canlı çalıştırmak ve JVM ile yerel çıktıyı yan yana göstermek. Gerçekte yapılan ve ölçülen:

* cgroup v2 salt okunur ve `systemd-run --scope`/`--user` bu kutuda reddedilir, bu yüzden `memory.max` ayarlanamaz; `/usr/bin/prlimit` var. `RLIMIT_AS` (adres alanı) süreç başına bulunan tek sert tavandır ve OOM öldürücü seçimleri yerine temiz tahsis hatalarına dönüşür. `RLIMIT_RSS` modern Linux'ta no-op'tur.
* İlk deneme *tüm düzeneği* 1 GiB ile sınırladı (`MONO_TEST_BUILD_AS_GIB=1`): `tools/gen_jla.py` içinde başarısız oldu, çünkü `javap` <=1 GiB adres alanıyla başlayamaz (HotSpot sanal olarak >1 GiB ayırır). JVM'in kendisini aynı tavanla sarmak bu JDK'da uygulanabilir değil.
* Tavan bu yüzden yalnızca her `zig` çağrısına uygulandı (`MONO_ZIG_AS_GIB=1`, `monolith/__main__.py` içinde `preexec_fn` `RLIMIT_AS` olarak gerçekleştirildi, 6 zig çağrı yerinin hepsi; `tests/run_tests.py` düzeneği tavansıza geri alındı).
* Tavanla, çevirici (~40 sn, 5907 sınıf / 31964 metot / 288 yerel metot) ve her parçalı `zig build-obj` sorunsuz derlendi; **kendi bağlama adımı** (shard0 + tüm shard nesnelerinin `zig build-exe`'si) 1 GiB tavanı altında, hem Debug hem `ReleaseSmall`, 8 ve 16 shard ile tekrarlanabilir biçimde `error: OutOfMemory` ile öldü. GNU ld 474-640 MiB'lık nesneleri ~2 sn'de bağlar ama zig'in `extern threadlocal` referanslarını (NOTYPE UND) `shared.o` içindeki STT_TLS tanımlarına karşı reddeder.
* Gerçekleştirilen düzeltme: `_compile_sharded` içinde `MONO_ZIG_LD=cc`, zig'in kendi bağlayıcısı yerine sistem C araç zinciriyle bağlar; ondan önce `tools/fix_tls_syms.py`, üretilen nesnelerde bu dört tanımsız sembolü (`cur`, `processing_refs`, `throw_depth`, `attached`) `STT_TLS` olarak yeniden tiplendirir (yerleşim değişmez; her biri bir `st_info` baytı). Varsayılan olarak kapalı; zig bağlama hâlâ varsayılan yol.
* Canlı sonuç (son koşu, çıktı kaydedildi): `PASS Arith [2228 lines; jvm 0.13s, native 0.41s, build 127.0s]` — `1/1 geçti`, `MemAvailable` önce 1560 MB, sonra 1524 MB (hiçbir şey çökmedi). Yerel ikilinin tek başına `prlimit --as=1G` koşusu ve JVM stdout'una karşı `diff` bayt bayt aynı (0 satır), ikisinin de stderr'i boş.
* Yapılmayan / iddia edilmeyen: 1 GiB tavan *adres alanını* ölçer, RSS'i değil — kutunun ~1,5 GiB boş belleği hiç risk altında değildi; tavan makineyi korur, zig'in bağlayıcısının 1 GiB'a "sığdığının" kanıtı değildir (sığmaz — bu yüzden GNU-ld yolu). Parçasız `zig build-exe` yolu ve LLVM tavan altında imkânsız kalır. Diğer tüm fark testleri çalıştırılmadı. `tests/build` koşudan sonra silindi.

## 16. Hata düzeltme, bellek geri kazanımı, yazmaç dönüşü ve RHI sağlamlaştırma geçişi (2026-11-16)

v2 denetim anlık görüntüsü birleştirildikten sonra ortaya çıkan gizli hatalar ve darboğazlar denetlendi ve çözüldü:

* **SlabPool işletim sistemi bellek geri kazanımı ve arena güvenliği (`runtime/mem_pool.zig`, `runtime/rt.zig`)**:
  - Bağlantısız, asla işletim sistemine geri küçülemeyen 256 KiB'lık slab'lar, satır içi 128B başlıklı `128 KiB` hizalı `SlabBlock`'larla değiştirildi (`(ptr & ~(SLAB_SIZE - 1))` O(1) arama): bir slab'daki tüm yuvalar 3 çağ boyunca döndüğünde ya da emekli olduğunda, `MAX_RETAINED_EMPTY_SLABS` (4 = 512 KiB) üstündeki fazla boş slab'lar doğrudan işletim sistemine geri bırakılır (sıcak rezervde `madvise(MADV_DONTNEED)`; sıfır tutma RSS şişmesi). `"SlabPool returns completely empty slabs across epochs back to the OS"` birim testiyle doğrulandı.
  - İş parçacığı başına önbellek yeniden doldurma toplu işi (`max 32`) ve `scratch_arena` tutma (`max 2` parça = 512 KiB) ayarlandı.
  - `rt.intern`, `rt.classMirror` ve `rt.initClass` (`<clinit>`) çevresine `arenaSuspend`/`arenaResume` eklendi; böylece süreç çapı nesneler asla geçici çerçeve/arena kapsamları içinde tahsis edilmez.
  - Aritmetik taşma koruması, geri sarılan arena dilimlerinde `0xAA` hata ayıklama güvenlik zehiri, `resetRetainingCapacity` üzerinde aşırı büyük temel parça serbest bırakma ve `ArenaAllocator` içinde LIFO `vtFree` muhasebe düzeltmeleri eklendi.
* **Yazmaç-dönüş ABI'si (`rt.Res`) ve ölü gölge dizi eliminasyonu (`runtime/rt.zig`, `monolith/zigen.py`, `monolith/shard.py`)**:
  - `_r` sarmalayıcıları, vtable'lar, itable'lar, yansıma `reflectInvoke`, `Class.enumValues0`, `MemorySegment.address()` `VarHandle` indirgemesi ve shard'lar arası cepheler boyunca yazmaç paketli `rt.Res(T)` dönüşleri (`toR`/`unR` ile `u64`/`u128`) gerçekleştirildi.
  - Derlenmiş Java metotlarından ölü, yalnızca yazılan `var R: [K]rt.Ref` yığın dizisi ve kaçan `.refs = &R` işaretçisi elendi (`sv`/`lv` yazmaca terfi edilebilir yerel değişkenler `s*a`/`l*a` kullanır; `F.refs = null` varsayılan).
  - `_is_leaf2` içindeki yinelenen `invokestatic` denetimi kaldırıldı (`monolith/zigen.py`).
* **RHI, GL 3.3 Core, dünya elemesi, GPU örnekleme, çizim döngüsü kancaları ve `--no-lwjgl` (`runtime/rhi*.zig`, `patches/`, `runtime/java_nolwjgl/`, `tools/mcbuild.py`)**:
  - `runtime/rhi_gl.zig`: sabit sentetik boru hattı bağlama (`gl_default_prog` GLSL 330 gölgelendiricisi + gereken GL 3.3 Core VAO `gl_default_vao`), `has_base_instance == false` olduğunda `uFirstInstance` uniform geri düşüşü (Intel HD 5500), sıfır ofsetli orphan+eşzamansız TBO yüklemesi (`size_bytes`), `eglGetProcAddress` geri düşüşü ve tembel `tryAttachGlContext()`.
  - `runtime/world_render.zig`: CaveCulling portal BFS düzeltildi; elmas yolları her giriş yüzünü biriktirir (gerileyen test geçti); `runtime/gpu_instancing.zig`: opak çizimler yarı saydamlardan (`particle`/`text_display`) önce sıralandı ve kayıtsız ağlar GPU hazırlama yüklemelerinden ve hayatta kalan istatistiklerinden çıkarıldı.
  - `runtime/rhi_bridge.zig` ve `runtime/java/mono/rhi/NativeRhi.java`: köprüye ait `frame_arena`, açık doğrulama/OOM hataları, `submitSectionSnapshotEx`, `visibleSectionCount`/`visibleSectionField` ve GL komut shim'leri eklendi; `patches/client_frame.zig` ve `patches/level_renderer.zig` içinde tick/çizim başına yaşam döngüsü kancaları eklendi.
  - `monolith/__main__.py` ve `tools/mcbuild.py` içinde `--no-lwjgl` ile temiz bağlanan isteğe bağlı Sıfır-LWJGL shim'i (`runtime/java_nolwjgl/org/lwjgl/...`) eklendi (`--no-lwjgl` launcher özellikleri düzeltildi).
* **JNA Udev Proxy katlama ve `String.coder` (`monolith/program.py`, `classlib/src/java/lang/String.java`, `tests/java/JnaProxyFold.java`)**:
  - `_fold_proxy` (`monolith/program.py:637`), dizi `aastore` sınırlarında durması ve yalnızca arayüzlere filtrelemesi için düzeltildi (`Proxy.newProxyInstance(..., ifaces, ...)` sonrasında `Class.getClassLoader()` geldiğinde düzeltme).
  - `_fold_jna_proxy` daha derin pencereler ve sondaki seçenek haritaları için sağlamlaştırıldı (`Udev extends Library`).
  - `String.java` içine `private final byte coder = UTF16;` eklendi (mevcut hizalama dolgusunda 0 ek bayt).
* **Burada canlı doğrulanan (`aarch64` 10 çekirdekli Linux, 7,4 GB RAM):**
  - `ClientCoreE2E` `-O Debug`: **6/6** `mem_pool`, **1/1** `thread_pool`, **7/7** `rhi`, **9/9** `world_render`, **12/12** `gpu_instancing`, **14/14** 4 alt sistemli E2E, `1,092` sınıf / `5,932` metot (`0 missing-native, 0 link-error, 0 unsupported-varhandle`), derleme süresi `587.1s`'den `37.9s`'e düştü (**15,5 kat hızlı**).
  - Tam 27 testlik fark JVM paketi (`tests/run_tests.py`, `-O Debug`): **27/27 GEÇTİ** (`Arith`, `Strings`, `Oop`, `Exceptions`, `Lambdas`, `Collections1`, `Threads`, `GcStress`, `Misc`, `IoZip`, `NioFiles`, `NetIo`, `HttpFetch`, `IfaceInit`, `LazyLeaf`, `Intrinsics`, `PatchTarget`, `Reflect`, `MHandles`, `Annot`, `StackWalk`, `ModuleInfo`, `Platform`, `Jni`, `OptSuite`, `JnaProxyFold`, `ClientCoreE2E`).
  - `tools/microbench/run_client_core_bench.py` (`-O ReleaseFast`): `arena_alloc_64B` `11.71 ns/op` (`85.4 Mops/s`), `slab_epoch_64B` `72.30 ns/op` (`stw_pause_ms=0.0`), `worker_pool` `146.9 us/batch` (`36.73 ms` toplam, `3987.72 ms`'e göre **108,5 kat hızlı**), `world_cull_256` `40.1 us/kare`, `gpu_instancing_2048` `7` toplu işte `1499/2048` (`287.0 us/kare`).

## 17. Derin sistem denetimi, RTA kapanış küçültme ve S24 FE kararlılık geçişi (2026-11-16)

* **RTA kapanış şişkinliğinin giderilmesi (temel testlerde `5,902` -> `230` sınıf; 15x–40x hızlı derlemeler)**:
  - `classlib/src/java/lang/Module.java`, `Class.java`, `System.java`: `BootModule` / `UnsupportedModule`, `ModuleDescriptor.newOpenModule(...)`'dan ve `System.<clinit>` içindeki istekli `ModuleLayer.empty()`'den ayrıştırıldı; `Module.java`'dan `java.lang.classfile.*`, `ModuleBootstrap`, `ClassLoaders`, `BuiltinClassLoader`, `ServicesCatalog` ve `URL.openStream()` (`JarVerifier` -> `ProviderConfig` -> 1 500+ SSL/JCA/Crypto sınıfı) kaldırıldı; dinamik ana makine `os.arch` yerel metodu eklendi (`J_java_lang_System__arch0`: `aarch64` / `amd64`).
  - `classlib/src/java/lang/String.java`: `String.format` / `String.formatted`, `java.util.Formatter` ya da `BigDecimal` olmadan doğrudan gerçekleştirildi (tüm `Misc` ve `Oop` biçim testlerini geçerken JDK `.toString()` ve hata yollarından `Formatter`/`Locale`/`Calendar`/`java.time`/`Pattern`/`ICU` kapanış şişkinliğini ortadan kaldırır).
  - Hafif kaplamalar eklendi: `classlib/src/jdk/internal/loader/ClassLoaders.java`, `classlib/src/jdk/internal/util/ClassFileDumper.java`, `classlib/src/jdk/internal/util/ReferencedKeyMap.java`, `classlib/src/java/util/stream/Tripwire.java` ve `classlib/src/sun/reflect/generics/reflectiveObjects/ParameterizedTypeImpl.java`; `tools/gen_jla.py` ve `monolith/classpath.py` güncellendi (`services()` uygulama kaynaklarına `[:boot_start]` ile sınırlandı).
* **Çalışma zamanı, yansıma, VarHandle ve JNI düzeltmeleri**:
  - `runtime/thread_pool.zig`: tek mutex kilidi altında toplu kuyruk ekleme, kilit sonrası `cv.broadcast()` ve tek `wg.add(count)` (`15,950.9 us/batch` -> `146.9 us/batch`, **108,5 kat hızlanma**).
  - `runtime/varhandle.zig`: `Debug`/`ReleaseSafe` içinde hizalama panic'lerini önlemek için hizasız `MemorySegment` `getSeg`/`setSeg` düzeltildi (`*align(1)`).
  - `classlib/src/jdk/internal/reflect/MonoFieldAccessor.java`: `isVolatile` alanları `Unsafe.get*Volatile` / `put*Volatile` üzerinden yönlendirildi.
  - `runtime/natives.zig`: `findTarget` içine en belirgin default arayüz metodu araması eklendi.
  - `runtime/jni.zig` ve `runtime/jni_call.zig`: `findMethod` içinde arayüz metotları için statik-bayrak denetimi eklendi ve `DefineClass`, `FromReflectedField`, `ToReflectedMethod`, `ToReflectedField` ve `GetModule` gerçekleştirildi (0 gerçekleştirilmemiş JNIEnv yuvası).
  - `monolith/zigen.py`: ölü `_shadow_refs` ve `_compact_refs` yardımcıları kaldırıldı.
* **Mobil / Termux LMKD koruması (`monolith/__main__.py`, `tests/run_tests.py`, `tests/run_client_core_e2e.py`)**:
  - Çocuk `zig`/`javac`/`java` CPU ilişkisi (`sched_setaffinity({0, 1})`, `nice(10)`), JVM heap'i (`-Xmx192m`/`-Xmx256m -XX:+UseSerialGC`), 450 sınıfın üzerinde otomatik shard'lama ve `<4,5 GiB` boş RAM'de `cc` bağlama sınırlandı.

## 18. Monolith için TouchCore, Android `build-lib` boru hattı ve GLES3/GLFW yüzey köprüsü (2026-11-16)

* **Android `libminecraft_mono.so` üretim boru hattı (`monolith/__main__.py`, `tools/mcbuild.py`, `runtime/rt.zig`, `monolith/zigen.py`, `monolith/shard.py`)**:
  - Hem `monolith` hem `tools/mcbuild.py` için `build-lib` alt komutu ve `-dynamic` / `--dynamic` + `--target aarch64-linux-android.26` desteği eklendi (Termux/NDK clang + `ld.lld -shared -z max-page-size=16384` + `__emutls_get_address` için `libclang_rt.builtins-aarch64-android.a` kullanılarak).
  - Üretilen `main.zig`'den (parçalı ve parçasız) `mono_android_entry(...)` dışa aktarıldı; `rt.runMainAsync` çağırır, JVM ana iş parçacığını tam yığınla (`default_stack_size * 4`) başlatır ve hemen ayrılır (<=1 ms içinde `rc = 0`), böylece Android UI iş parçacığı asla bloke olmaz.
  - İşaretçiden-`i64`'e dönüşümler `@intCast`'ten `@bitCast`'e çevrilerek Android `aarch64` işaretçi etiketleme / TBI (`bit 63 = 1`) panic'leri düzeltildi: `monolith/zigen.py` (`mirrorSet`/`mirrorGet`), `rt.zig` (`eetop`), `natives.zig`, `varhandle.zig`, `io.zig` ve `nio.zig`; `runtime/jni_call.zig` içinde shard'lar arası `getModule` `rt.unR` tip işleme düzeltildi ve `gl_shim.zig`, `shard.py` `RUNTIME_FILES` listesine eklendi.
* **GL yol kararı ve doğrudan GL → `libGLESv3.so` dağıtımı (`runtime/gl_shim.zig`, `runtime/rhi_gl.zig`, `runtime/rhi_bridge.zig`, `runtime/java/mono/rhi/NativeRhi.java`, `runtime/java_nolwjgl/`)**:
  - Tüm durumlu GL boru hattını Java `NativeRhi` içinde yeniden gerçekleştirmek yerine, GL 3.3 Core → GLES 3.0 uyumluluk shim'leriyle (GLSL `#version 150`/`330 core`/`4xx` -> `#version 300 es`, `glClearDepth`->`glClearDepthf`, `glDepthRange`->`glDepthRangef`, güvenli `glPolygonMode` no-op) Android `libGLESv3.so`'ya doğrudan yerel dağıtım seçildi.
  - `runtime/rhi_gl.zig`, Android `libGLESv3.so` + `libEGL.so` yoklama geri düşüşü ve `ES 3.00` gölgelendiricileriyle güncellendi; `runtime/java_nolwjgl/org/lwjgl/` genişletildi (`GLFW`, geri çağrı arayüzleri, `GL11C`, `GL15C`/`GL15`, `GL20C`/`GL20`, `GL30C`/`GL30`).
  - `GLFW.glfwCreateWindow`, `glfwMakeContextCurrent`, `glfwSwapBuffers` ve `glfwPollEvents`, TouchCore'un `ANativeWindow` + `EGL` + `g_tc_input` yapısına `tc_android_has_window`, `tc_android_init_egl`, `tc_android_swap_buffers`, `tc_android_get_size` ve `tc_android_poll_snapshot` (`libtouchcore_mono.so`) ile bağlandı.
* **MJLauncher tarzı TouchCore APK (`android/touchcore/`, `releases/Monolith.apk`)**:
  - Başlangıç/oyun öncesi çökme tehlikeleri düzeltildi: `VkPhysicalDeviceProperties.limits_pad` 824 baytlık Vulkan ABI yapısına uyması için `[560]u8`'e genişletildi (`runtime/rhi_vk.zig`), 131 KB portal ara belleği pthread yığınından çıkarıldı (`touchcore_core.zig`), `android:extractNativeLibs="true"`, `gl_ready` + EGL derinlik/ES2 geri düşüşü + 4 MB çizim iş parçacığı yığını eklendi (`touchcore_android.c`) ve `mono_android_entry`, `ANativeWindow` bağlanana kadar UI iş parçacığından ertelendi.
  - UI ve kontroller yükseltildi (`HudRenderer.java`, `TouchOverlayView.java`, `ControlsEditorActivity.java`, `GameActivity.java`, `LauncherActivity.java`): buzlu cam vektör ikonlu düğmeler, bağlı `+` D-Pad ve Analog Çubuk, kaydırılabilir saldırı/kullanma kamera sürükleme, sanal imleç (`FARE`), yazılım klavyesi (`KB`), merkez artı imleç, izometrik 9 yuvalı hotbar, yatay önizleme ve ince itme padli görsel HUD düzenleyici, açık **"DEMO MODU (Zig RHI Voxel)"** etiketleme ve harici bir `libminecraft_mono.so` başarısız olduğunda açık hata iletişim kutusu (Demo Modu'na sessiz geri düşüş yok).
* **Bu ARM64 Android telefonda canlı doğrulanan:**
  - `python3 tools/build_touchcore_apk.py`: `releases/Monolith.apk` (ikili dosya, repoda yok; `tools/build_touchcore_apk.py` ile yerelde üretilir) derlendi (`0.98 MiB`), `apksigner verify` GEÇTİ (v2/v3), `zipalign -c -p 4` GEÇTİ, `nm -D` tüm `Java_com_monolith_touchcore_NativeBridge_*` ve `tc_android_*` sembollerini doğrular.
  - `/system/bin/linker64` üzerinden Bionic PIE yerel testleri: `libtouchcore_mono.so` doğrulandı (`tc_zig_init`, 10 tick, `tc_zig_shutdown`), tek shard'lı ve 4 shard'lı `--no-lwjgl` `-dynamic` `libminecraft_mono.so` (`mono_android_entry` `1 ms` içinde `0` döndürdü, `NO_SURFACE_EXPLICIT_THROW=true`, `.so` sınırı boyunca `KEY_W=1` okuyan `NativeRhi.pollOsInput`, `GL11C` GL→GLES3 dağıtımı, `EXIT=0`).
  - Gerileme paketleri: `tests/run_tests.py Arith IoZip` GEÇTİ; `tests/run_client_core_e2e.py` 4 E2E DENETİMİNİN HEPSİ GEÇTİ (`mem_pool 6/6`, `thread_pool 1/1`, `rhi 7/7`, `world_render 9/9`, `gpu_instancing 12/12`, `4-Subsystem E2E 14/14`, `ClientCoreE2E` yerel `JAVA_CLIENT_CORE_E2E_OK`).

## 19. TouchCore Mojang indirici, genişletilmiş `--no-lwjgl` yığını ve dürüst Android `client.jar` durumu

* **Resmi Mojang sürüm/asset indirici (`android/touchcore/src/com/monolith/touchcore/MojangDownloader.java`, `LauncherActivity.java`)**:
  - `https://piston-meta.mojang.com/mc/game/version_manifest_v2.json` dosyasını doğrudan Mojang CDN'lerinden alır (APK içinde paketlenmiş Mojang oyun dosyası ya da jar yok).
  - Seçilen sürümün `<id>.json`, `client.jar` (boyut denetimli) -> `<filesDir>/monolith/versions/<id>/client.jar` ve `<assetIndex.id>.json` -> `<assetsDir>/indexes/<assetIndex.id>.json` dosyalarını bir arka plan iş parçacığında indirir. *(Mojang CDN'lerinden canlı HTTPS indirmesi headless PRoot ortamında test edilmedi.)*
* **Genişletilmiş `--no-lwjgl` istemci shim kapsamı (`runtime/java_nolwjgl/org/lwjgl/`)**:
  - `system/`: `MemoryStack` (try-with-resources çerçeve tahsis edici), `Pointer`, `Configuration`, `Callback` eklendi.
  - `stb/`: `STBImage` eklendi (`stbi_info_from_memory`, `stbi_load_from_memory` gerçek PNG parçası + `java.util.zip.Inflater` RGBA çözücü + süzgeç yeniden kurulumu, `stbi_set_flip_vertically_on_load`, `stbi_image_free`).
  - `opengl/`: `GLCapabilities` (`GL.createCapabilities()` tarafından döndürülen `OpenGL11`..`OpenGL33`, ARB bayrakları), `GL31C`/`GL31`, `GL32C`/`GL32`, `GL33C`/`GL33` ve `GL30C` mipmap/renderbuffer/framebuffer yardımcıları eklendi.
  - `glfw/`: `Callbacks.glfwFreeCallbacks`, `GLFWVidMode`, `glfwGetPrimaryMonitor`, `glfwGetVideoMode`, `GLFWErrorCallbackI`, `GLFWCharCallbackI`, `GLFWScrollCallbackI`, `GLFWWindowFocusCallbackI`, `glfwSetClipboardString`/`glfwGetClipboardString`, `glfwSwapInterval` eklendi.
  - `openal/`: asgari `ALC`, `ALC10`, `AL`, `AL10`, `ALCCapabilities`, `ALCapabilities` taslakları eklendi.
* **Android çok mimarili C başlık düzeltmesi (`monolith/__main__.py`)**:
  - `_android_include_flags()` eklendi; `/data/data/com.termux/files/usr/include/aarch64-linux-android` (`asm/types.h`) yolunu `zig build-obj` ve `clang`'a geçirir, böylece `java.util.zip.*` / `STBImage` kullanan Android `--target aarch64-linux-android.26` `-dynamic` kütüphaneleri derlenirken `runtime/io.zig` içindeki `@cImport(@cInclude("zlib.h"))` temiz derlenir.
* **Bu güncellemelerden sonra doğrulananlar**:
  - `python3 tools/build_touchcore_apk.py`: `releases/Monolith.apk` (ikili dosya, repoda yok; `tools/build_touchcore_apk.py` ile yerelde üretilir) derlendi (`0.98 MiB`, 13 Java dosyası, 107 D8 sınıfı), `apksigner verify` GEÇTİ (v2/v3), `zipalign -c -p 4` GEÇTİ.
  - Doğrudan Android Bionic `/system/bin/linker64` üzerinde çalıştırılan 4 shard'lı Android `aarch64-linux-android.26` `-dynamic --no-lwjgl` derlemesi (`FullNoLwjglTest` -> `libminecraft_mono.so`, `26233 KiB`): `mono_android_entry` `0.82 ms` içinde `0` döndürdü, `STBIMAGE_PNG_OK 2x2 RGBA`, `GL_CAPS_OK OpenGL33=true`, `OPENAL_OK buf=1 src=2`, `GLFW_SURFACE_CHECK_OK vm=1280x720` (ayrıca ekli işletim sistemi yüzeyi olmadan açık `UnsupportedOperationException`), `FULL_NOLWJL_BIONIC_OK`, `Bionic exit=0`.
  - `zig test -lc runtime/rhi.zig`: `All 7 tests passed.`
  - `python3 tests/run_tests.py Arith IoZip`: `2/2 passed`.
* **Vanilla Mojang `client.jar`'ın telefonda %100 oynanabilir çalışmasından önce kalan engeller**:
  - Tam bir Mojang `client.jar`'dan bir Android `libminecraft_mono.so` üretimi henüz **çalıştırılmadı** (`client.jar` yerelde yok ve ~61 bin dosyalı Vanilla kapanışını LLVM ile AOT derlemek `§8`'e göre ~7–15 GiB RAM gerektirir; `monolith/__main__.py` ise `--backend native`'i `x86_64/AMD64` ile sınırlar, yani büyük vanilla AOT `.so` derlemeleri yüksek RAM'li bir ana makineden çapraz derlenmeli ya da daha fazla bölünmelidir).
  - Gerçek `client.jar` oynanışının canlı bir Android SurfaceFlinger GPU/ses oturumunda uçtan uca çalıştırılması (GLES3 altında Blaze3D/Minecraft gölgelendirici uniform uyumluluğu, `GameActivity` içinde klavye/dokunmatik oynanış döngüsü ve `§2–3`'te izlenen kalan gerçekleştirilmemiş JDK/üçüncü taraf yerel metotları) henüz cihazda yapılmadı.

## 20. Tam MC `26.3` `--no-lwjgl` SDL3/GLES3/OpenAL eşitliği, kaçış analizi ve SHA-1 indirici

* **Gerçek Minecraft `26.3` (`26.3-client.jar`, 41 483 720 bayt + 42 kütüphane) deneysel açık kapatma**:
  - Tam LWJGL 3.4 SDL3 yüzey/olay/girdi katmanı eklendi (`runtime/java_nolwjgl/org/lwjgl/sdl/` — `SDLVideo`, `SDLEvents`, `SDLKeyboard`, `SDLMouse`, `SDL_Event` + alt olaylar, `SDLInit`, `SDLTimer`, `SDLClipboard`, `SDLStdinc`, `SDLSurface`, `SDLPixels`, `SDLMessageBox`, `SDLHints`, `SDLLog`, `SDLKeycode`, `SDLScancode`, `SDLVulkan`) ve `NativeRhi`'ye bağlandı (`ANativeWindow` + `g_tc_input`).
  - Genişletilmiş OpenGL 3.3 Core -> GLES3 ve OpenAL köprüsü (`runtime/gl_shim.zig`, `runtime/al_shim.zig`, `runtime/rhi_bridge.zig`, `runtime/java/mono/rhi/NativeRhi.java`, `monolith/shard.py`): FBO blit, tampon kopyalama/aralık eşleme, örnekleyiciler, sorgular, `glVertexAttribDivisor`, `glFenceSync`/`glDeleteSync`/`glClientWaitSync`, `glDrawBuffer`, `ARBDirectStateAccess` DSA->bind bağdaştırıcıları, `GL33C` üzerine `ARBVertexAttribBinding` durum öykünmesi, temiz geri düşüşle dinamik `libopenal.so.1`/`libopenal.so` ses yoklaması, gerçek `STBImageResize` çift doğrusal RGBA ölçekleme, `STBImageWrite` Deflater PNG kodlama + `STBIWriteCallback.getData`, `StructBuffer.get()`, `Pointer.Default` ve tam `org/lwjgl/vulkan/`, `util/vma/`, `util/shaderc/`, `util/spvc/`, `util/freetype/` (`FT_Init_FreeType = -1` bitmap yazı tipi geri düşüşü) link kapanışı eklendi.
  - `MethodHandle.invokeBasic` imza-polimorfik indirgemesi düzeltildi (`monolith/program.py`, `classlib/src/java/lang/invoke/MethodHandle.java`) ve `net/minecraft/client/renderer/LevelRenderer.resetLevelRenderData()V`, `patches/manifest.json` dosyasına eklendi.
  - `26.3-client.jar`'a karşı RTA doğrulaması (`61,280` yüklenen sınıf, `278,867` erişilebilir metot), `org/lwjgl/*` link hatalarını **`345` -> `0`**'a indirdi (`resetLevelRenderData` dahil tüm yamalar `true` eşleşti; `91` LWJGL dışı link hatası kalıyor, çoğu isteğe bağlı Log4j appender'ları/`NarratorMac` + `9` `MethodHandle`/`Lookup` iç yapısı, `103` JDK yerel metodu ve `257` üçüncü taraf yerel metoduyla birlikte).
* **Metot içi kaçış analizi ve otomatik `ArenaScope` (`monolith/zigen.py`, `runtime/rt.zig`)**:
  - `runtime/rt.zig` içine `rt.arenaMark`, `rt.arenaRestore`, `rt.newObjectArena` ve `rt.newArrayArena` ile `monolith/zigen.py` içinde ileri DAG veri akışı kaçış analizi (`_analyze_local_allocs`, `_safe_ea_callee`) eklendi; döngüsüz olduğu kanıtlanmış, kaçmayan `new`/`newarray`/`anewarray` nesnelerini iş parçacığının `scratch_arena`'sında bump ile tahsis eder ve metottan çıkışta geri sarar.
* **TouchCore SHA-1 kütüphane + asset nesnesi indirici (`android/touchcore/src/com/monolith/touchcore/MojangDownloader.java`)**:
  - İndirici Linux işletim sistemi kural süzmesi, 4 iş parçacıklı `ExecutorService` ve `client.jar`, yerel olmayan kütüphane JAR'ları, `assetIndex` ve tek tek `resources.download.minecraft.net` asset nesneleri için SHA-1 + boyut doğrulamasıyla yükseltildi. *(Canlı HTTPS indirmesi çevrimdışı PRoot içinde hâlâ test edilmedi.)*
* **Düzenlemelerden sonra doğrulananlar**:
  - `python3 -m py_compile` / AST denetimleri / `.claude/hooks/guard.py --self-test` / `zig ast-check`: GEÇTİ; `runtime/java/` + `runtime/java_nolwjgl/` içindeki tüm `276` Java dosyası JDK 25 `javac` ile temiz derlendi (`rc=0`).
  - `tests/run_client_core_e2e.py`: `ALL 4 CLIENT CORE E2E CHECKS PASSED (Debug)` (`mem_pool 6/6`, `thread_pool 1/1`, `rhi 7/7`, `world_render 9/9`, `gpu_instancing 12/12`, `4-Subsystem E2E 14/14`, `JAVA_CLIENT_CORE_E2E_OK`, `0 missing-native`, `0 link-error`, `0 unsupported-varhandle`).
  - 4 shard'lı Android ARM64 `-dynamic --no-lwjgl` Bionic `/system/bin/linker64` testi (`BionicSdlCheck`, `17438 KiB` `.so`): `rt.arenaMark`/`newObjectArena`/`newArrayArena` üretti, `HOST_ENTRY_RC=0`, `EA_ARENA_OK=1002502500`, `SDL_NO_SURFACE_GUARD_OK=true`, `GL_SHIM_OK=true`, `OPENAL_SHIM_OK=true`, `STB_PNG_OK=true`, `FREETYPE_FALLBACK_OK=-1`, `BIONIC_SDL_E2E_PASS`, `BIONIC_EXIT=0` yazdırdı.
  - Fark testleri (`tests/run_tests.py`): `Arith` (`1/1 GEÇTİ`), `MHandles` (`1/1 GEÇTİ`); `GcStress`, GC'siz `SlabPool` altında `WeakReference` `weak cleared=false` döndürürken `churn`/`big arrays`/`threads`/`map 200000 262501012965` ile eşleşti.
  - TouchCore APK (`python3 tools/build_touchcore_apk.py`): `releases/Monolith.apk` (ikili dosya, repoda yok; `tools/build_touchcore_apk.py` ile yerelde üretilir) derlendi (`0.98 MiB`, 13 Java dosyası, 109 D8 sınıfı), `zipalign -c -p 4` GEÇTİ, `apksigner verify` v2/v3 GEÇTİ.
* **Kalan dış/donanım engelleri**:
  - Tam `26.3-client.jar` LLVM `.so` derlemesi (~`7–15 GiB` RAM), GitHub Actions ([`build-so.yml`](../.github/workflows/build-so.yml)) ya da bir `x86_64` ana makinesinde çalıştırılmalı; canlı cihaz içi `SurfaceFlinger` çizimi + gölgelendirici uyumluluğu + HTTPS Mojang indirmeleri, derlenmiş `.so`'nun TouchCore APK'ya yüklendiği fiziksel telefonda test edilmeyi bekliyor.


## 2026-11-18: Android launcher ve motor doğrulaması

- Ortam: Zig 0.15.2, Android NDK r27c, JDK 25.0.5, Android SDK platform 34 / build-tools 34.0.0, 4 çekirdek, 15 GiB RAM.
- Tam `libminecraft_mono.so` (Minecraft 26.3, ReleaseFast, `aarch64-linux-android.26`, 16 shard) derlendi: 437 MB ayıklanmış, 16 KB sayfa hizalı, `mono_android_entry` dışa aktarılmış.
  Bir shard 7,5 GiB'a kadar ister, bu yüzden makine bir seferde bir shard derler. Yarıda kesilen bir derleme, Minecraft derleme aracının `--only-shards K,K` seçeneğiyle devam eder.
- Düzeltilen çökme nedenleri: `exit()`, HWUI/GPU sürücüsü yıkıcılarıyla yarışıyordu (artık Android'de `exit_group`); motor stdio'su görünmezdi (artık logcat etiketi `MonolithEngine`'e yönlendiriliyor);
  glibc'ye özgü `__errno_location`, `copy_file_range` ve `posix_spawn*` içe aktarmaları bionic'te `dlopen`'ı başarısız kılıyordu (errno `std.c._errno` ile, ham sistem çağrısı, zayıf içe aktarmalar);
  EGL bağlamı her Surface değişikliğinde yok ediliyordu (bağlam korunur, yalnızca pencere yüzeyi yeniden bağlanır).
- Launcher: oyun HUD'unun üzerine çizilen yinelenen hotbar ve artı imleç kalktı; dokunma yuvaları gerçek hotbar'a eşlenir.
- Fark testleri: 27 varsayılan testin 26'sı geçiyor. `GcStress` (`System.gc()` sonrası WeakReference temizlenmedi) açık. `ClientCoreE2E`'nin kendi çalıştırıcısı var.
- D8 8.2.2 (build-tools 34) JDK 25 sınıf dosyalarında çöker: `~/.cache/monolith/android-sdk/r8.jar` olarak R8 9.5.33 kullanın.
- Burada doğrulanmayan: `.so`'nun bir cihazda ya da öykünücüde çalıştırılması.

### 2026-11-19: x86_64'te motor başlangıcı, Android ana makinesinin masaüstü sahtesiyle doğrulandı

- `tools/mock_android_host.c`, `libtouchcore_mono.so`'nun yerini tutar (EGL pbuffer, Mesa üzerinden GLES 3): pencere, GL bağlamı ve
  tüm `Minecraft.<init>` yolu bir telefon olmadan çalışır. Debug motor başlangıcı gölgelendirici yüklemeye ulaşır.
- Yolda düzeltilenler: STBImage hata nedeni (null, "ok" değil), `ModuleLayer.boot()`, zaten bir ThreadState'i olan iş parçacıkları için JNI `GetEnv`/`AttachCurrentThread` (JNA kilitlenmesi), RTA'da `class$` forName yardımcısı, VarHandle sıradan metotları korundu, yerel ayar sağlayıcıları olmadan NumberFormat, oyunun yansıttığı LWJGL dışı Vulkan/GL/bellek taslakları.
- **Açık (çizimi engelliyor): gölgelendirici derleme.** Minecraft 26.3, GLSL'i shaderc ile SPIR-V'ye derler ve GL arka ucu için SPIRV-Cross ile çevirir (`renderpearl` `GlslCompiler.compileToSpv`). `runtime/java_nolwjgl/org/lwjgl/util/shaderc` ve `.../spvc` taslaktır; "shaderc disabled in --no-lwjgl build" bildirir, bu yüzden ilk kritik boru hattı (`minecraft:pipeline/gui`) başarısız olur ve oyun bir çökme raporuyla biter.
  Gerekenler: shaderc (glslang, SPIRV-Tools; kaynaklar NDK ile `sources/third_party/shaderc` altında gelir) ve aarch64 için derlenmiş SPIRV-Cross, `renderpearl`'ın yaptığı çağrılar için Java yerel metotları, GLSL ES 3.x çıktısı ve `libminecraft_mono.so` içine bağlama.

## 21. Bu makinede sürüm kapısı doğrulaması (2026-11-24)

- Ortam: 4 vCPU, 16 GiB RAM, JDK 25.0.5.1+2 (`~/.cache/monolith/temurin-25`), rustc/cargo, Python 3.13. Cihaz yok, GPU yok, ekran yok.
- Geçenler (listelenen yerlerde komutlar `docs/RELEASE.md` bölüm 2'deki gibi):
  - `pyflakes monolith tools tests/run_tests.py`: çıktı yok. `monolith tools tests` üzerinde `compileall`: çıkış 0.
  - `rustc --test tests/rust/audio_main.rs`: 12/12.
  - `monolith.crc_tests`, `netty_tests`, `recycle_tests`, `patch_tests`, `stack_tests`: her biri `ALL PASSED`.
  - `MONO_GC_MB=1 MONO_GC_POISON=1 MONO_ARENA_CHECK=abort` ile `tests/run_tests.py GcStress Oop Threads Lambdas Strings Collections1`: 6/6, normal ve `MONO_GC_CONCURRENT=1`.
  - `tests/run_tests.py Arith`: 1/1.
  - `tools/rt_typecheck.py`: GEÇTİ.
  - `android/touchcore/native` `cargo test`: 29/29.
- Düzeltme: `GcStress` artık açık değil. Bununla ilgili §18 ve §16 satırları tarihîdir; bu koşu her iki GC modunu da geçti.
- Burada çalıştırılmayan (donanım ya da zaman sınırı): tam Minecraft derlemesi, `tools/headless_play.sh` oyun açılışı, 60 dk sızıntı soak'ı, Android cihaz koşusu, Client Core E2E.

## 22. 2026-11-24 kod değişiklikleri ve hâlâ doğrulanmayanlar

Yapılanlar, çalıştırılan denetimle birlikte:
- `monolith/rt/crash.rs`: panic kancası ve Linux ölümcül sinyal işleyicileri (SEGV, BUS, ILL, FPE, ABRT) `mono-crash-<pid>.txt` yazar (dizin: `MONO_CRASH_DIR`). `rt.rs` içindeki yalnızca SIGSEGV'li stderr işleyicisinin yerini alır. `tests/rust/crash_main.rs`: 5/5 (alt süreçlerde gerçek panic ve gerçek SIGSEGV). `tools/rt_typecheck.py`: GEÇTİ.
- `tools/check_link_errors.py` ve `docs/LINK_ERRORS_TR.md`: Minecraft 26.3 çevirisinin 446 link hatasının tamamı açıklanmış beş gruba düşüyor; yeni açıklanmamış bir hata kapıyı başarısız kılar (olumlu ve olumsuz durum çalıştırıldı).
- Yama parmak izleri (`monolith/patches.py`, `tools/patch_fingerprints.py`): 10 yamanın hepsi metot gövdesini sabitler; `monolith.patch_fp_tests` 9/9.
- WS7: `runtime/java_nolwjgl` içinden 34 kullanılmayan Java kaplama dosyası silindi (277'den 243'e); kalan ağaç, boru hattının kullandığı aynı `javac` çağrısıyla derleniyor (295 sınıf). 35. silme (`SDL_LogOutputFunction`) yanlıştı: hiçbir kaplama kaynağı ve hiçbir üretilmiş sembol onu adlandırmıyordu, `javac` geçti, ama Minecraft'ın kendi bytecode'u (`SdlDebug.<clinit>`) onu adlandırıyor. Link hatası kapısı (`tools/check_link_errors.py`), silmeden sonraki ilk tam boru hattı koşusunun tek açıklanmamış link hatası olarak bunu yakaladı; dosya geri yüklendi. `docs/WS7_ENVANTER_TR.md`.
- `docs/JNI_ENVANTER_TR.md`: gruba göre 259 JNI/JNA öykünme yerel metodu; Tracy yerel metotları normal oyunda çağrılmaz (bytecode denetlendi).
- Zig artıkları: ölü `touchcore_core.zig` kaldırıldı, yorumlardaki bayat `.zig` dosya referansları Rust modüllerine işaret ediyor.
- Çalışma zamanı hataları düzeltildi (`monolith/rt/sync.rs`, `gc.rs`, `rt.rs`), her biri çalıştırılan bir testle: `Object.wait` içinde kaybolan bir kesme (`WaitInterrupt`, 3000 tur); `interrupt` ile zaman aşımlı `wait` arasında bir kilit sırası kilitlenmesi; yeniden kullanılan bir adreste yeni bir `Thread`'in miras aldığı bayat kesme bayrakları ve park izinleri; monitör oluşturan `holdsLock` ve `notify`; ölü nesnelerin hiç serbest bırakılmayan monitörleri (artık işaretleme ile süpürme arasında toplayıcı tarafından düşürülüyor, `MonitorChurn`); her istisna ya da monitör beklemesinde bir kez yerine her seferinde okunan `MONO_TRACE_EX` ve `MONO_TRACE_PARK`. `MONO_GC_POISON=1 MONO_ARENA_CHECK=abort` ile her iki GC modu, her biri 12 tekrar: hata yok.
- `rustgen.py`: derlenmiş nesne önbelleği `/tmp/monolith_shard_cache` sınırsızdı ve 42 GB'a ulaştı (disk fark paketinin ortasında doldu, 14 test koddan değil `No space left on device` ile başarısız oldu). Artık 8 GiB ile sınırlı, en eski önce (`MONO_SHARD_CACHE_GB`); `monolith.cache_tests` 7/7. Sınırdan sonra 14 test yeniden çalıştırıldı: 14/14.
- Fark paketi `tests/run_tests.py`: 27 geçti, 3 atlandı (`ClientCoreE2E`, `HttpFetch`, `Jni`), her atlama ölçülmüş nedeniyle. `NetIo` çalışıyor ve geçiyor (atlaması eskimişti).
- `MONO_COUNT_NATIVES` (`monolith/rt/ncount.rs`): JNI/JNA = 0 hedefi için sayaç; JNA tarzı yerel metotları çağıran küçük bir programda denetlendi (7 çağrı, 3 yerel metot doğru sayıldı); `tests/rust/ncount_main.rs`.
- Sinyaller (`monolith/rt/signals.rs`, `classlib/.../MonoSignals.java`, `Runtime.addShutdownHook`): SIGTERM, SIGINT ve SIGHUP'un işleyicisi yoktu, bu yüzden bir `kill`, Ctrl-C ya da sistem kapanışı süreci anında bitiriyordu ve kapanış kancaları hiç çalışmıyordu (Minecraft dünyayı kancasında kaydeder). Artık HotSpot'taki gibi bir `Signal Dispatcher` iş parçacığı üzerinden `Runtime.exit(128 + sinyal)` çalıştırıyorlar. `SignalExit` fark testi: HotSpot ile aynı çıktı ve 143 çıkış durumu. Yalnızca Linux; Windows ve macOS varsayılan eylemi korur.
- Yukarıdakilerin hepsiyle tam boru hattı koşusu (26.3, 16 shard, bir iş, 4 vCPU / 16 GB'da yaklaşık 70 dakika): 61 749 sınıf, 446 link hatası (`tools/check_link_errors.py`: açıklanmamış 0), 0 eksik yerel metot, parmak izleriyle 10 yama uygulandı. Ardından Xvfb altında yazılım GL ile boş bir oyun dizininden `tools/headless_play.sh --seconds 60 --clean-exit`: başlık, ilk çalıştırma ekranı, Singleplayer, Create New World (Creative), 60 sn yürüme, SIGTERM; kapanış kancaları çalıştı, dünya kaydedildi, çıkış durumu 143, `mono-crash` dosyası yok. O koşu sırasında `MONO_COUNT_NATIVES`: 139 JNI/JNA öykünme çağrısı, hepsi JNA ve Flite anlatıcısı (`docs/JNI_ENVANTER_TR.md`). Çalıştırılmayan: 60 dakikalık sızıntı soak'ı, oyunla eşzamanlı GC, çok oyunculu, gerçek GPU.
- 60 dakikalık sızıntı soak'ı (`tools/leak_soak.sh`, aynı derleme, oyun dünyada boşta, normal durdur-dünya GC): çökme yok, çökme raporu yok; `tools/leak_budget.py --warmup-s 1800 --budget-mb-per-min 1`: ısınmadan sonra canlı büyüme **+0,180 MiB/dk, bütçe içinde**. Aynı profil açık sorunu gösteriyor: 100 toplama, ortalama duraklama **1,1 sn**, en uzun **1,8 sn** (sonda canlı heap 625 MiB, resident set yaklaşık 5,8 GiB ve ilk 25 dakikadan sonra düz). Duraklama, bir oyuncunun hissedeceği takılmadır; azaltılması WS1'dir (GC_PLAN), başlanmadı.
- İndirme sağlamlaştırma (kod okunarak bulundu, sonra test edildi): TouchCore indiricide (`MojangDownloader.java`, `DownloadGuard.java`, 48 denetim) ve Python araçlarında (`mcbuild_common.py`, `fetch_client_jar.py`, `monolith.fetch_guard_tests`) bir manifestten gelen sürüm kimliği, kütüphane yolu ya da asset karması oyun dizinlerinin dışındaki bir dosyayı adlandırabiliyordu, her URL şeması kabul ediliyordu, Android tarafında `client.jar` SHA-1 olmadan kabul ediliyordu ve masaüstü tarafında sağlama toplamsız bir ayna jar'ı kabul ediliyordu. Artık: yalnızca https (Android: yalnızca mojang.com ve minecraft.net ana makineleri), yollar ve kimlikler kendi dizinlerinde kalmalı, karmalar 40 onaltılık hane olmalı, bir istemci jar'ının her zaman SHA-1'i olmalı, başarısız indirmeler `.part` dosyası bırakmaz. `MojangDownloader`, taslak `org.json`'a karşı `--release 8` ile derlendi; bir cihazda çalıştırılmadı.
- Dokümanlar: `docs/KULLANICI_REHBERI_TR.md` (ayarlar çalışma zamanı koduna karşı denetlendi), `docs/PLAN_4_GUN_TR.md`, `docs/JNI_ENVANTER_TR.md`.

Doğrulanmayan (tam bir Minecraft boru hattı koşusu ve bir oyun açılışı gerektirir, istek üzerine sona bırakıldı):
- Kaplama silme sonrasındaki çevirinin hâlâ 446 link hatası, 0 açıklanmamış ve 0 eksik yerel metot bildirdiği.
- Yeni çalışma zamanından başlatılan bir oyunun (çökme raporlayıcı `init_hooks` içinde kurulu) eskisi gibi davrandığı.
- Boru hattının 10 parmak izini kabul ettiği (araç ve `validate()` aynı sınıflar üzerinde aynı fonksiyonu kullanıyor; boru hattının kendisi değişiklikten sonra çalıştırılmadı).
- 16 shard'lı masaüstü koşusu, makineyi kod çalışması için boşaltmak üzere istek üzerine 16 shard'ın 7'sinde durduruldu.
