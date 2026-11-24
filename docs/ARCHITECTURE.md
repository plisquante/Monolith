# monolith — Java bytecode → Rust çevirici: mimari

```
 app.jar + libs ─┐
 classlib overlay ├─► ClassPath ─► World (hiyerarşi, JVMS çözümü, vtable/itable)
 java.base.jmod ─┘                   │
                                     ▼
                     Program: çözme + invokedynamic desugar + RTA
                                     │  (erişilebilir metotlar, örneklenmiş sınıflar)
                                     ▼
                     RustGen: shard başına bir Rust modülü + main.rs
                                     │
 monolith/rt/*.rs (çalışma zamanı; GC değiştiriliyor) ┴─► rustc ─► yerel çalıştırılabilir (JVM yok)
```

## Ön uç (`monolith/`)

| modül | görevi |
|---|---|
| `classfile.py` | sınıf dosyası ayrıştırıcı, sürümler 45–69 (Java 1.1 … 25): sabit havuz, alanlar, metotlar, Code, istisna tabloları, BootstrapMethods, Signature (jenerik metaverisi), InnerClasses, NestHost/Members, Record, PermittedSubclasses, LineNumberTable |
| `bytecode.py` | 202 opcode'un tamamının çözücüsü (wide, tableswitch/lookupswitch normalleştirilmiş); `jsr/ret` çözülür ve desteklenmediği bildirilir |
| `classpath.py` | dizinler, JAR'lar (çoklu sürüm farkında), JDK `.jmod` dosyaları |
| `world.py` | kapalı dünya sınıf hiyerarşisi: JVMS §5.4.3 alan/metot çözümü, §5.4.6 seçim, en belirgin default metotlar, vtable yerleşimi (miranda/default yuvalarıyla), itable içeriği, önek uyumlu örnek yerleşimleri |
| `indy.py` | **invokedynamic desugar**: sentetik bytecode sınıflarına çevirir: `LambdaMetafactory.metafactory/altMetafactory` (yakalamalar, köprüler, işaret arayüzleri, kutulama/açma/genişletme uyarlaması), `StringConcatFactory` (tarifler, sabitler), `ObjectMethods.bootstrap` (record `toString/hashCode/equals`, JDK'nın birebir formülleri ve karşılaştırma sırasıyla), `SwitchBootstraps.typeSwitch/enumSwitch`. Bilinmeyen bootstrap'ler bildirilir ve bir JVM gibi `BootstrapMethodError` fırlatır. |
| `program.py` | komut hazırlığı (indy yeniden yazımı, devre dışı `assert`lerin katlanması) ve her erişilebilir metot için `why` zinciriyle **Rapid Type Analysis** |
| `rustgen.py` | Rust kodu üretimi (aşağıda) |
| `analyze.py` | JAR istatistikleri: sürümler, opcode histogramı, indy bootstrap'leri, yansıma/Unsafe/VarHandle kullanımı, yerel metotlar |

## Kod üretim şeması

* **Tipler**: `int/short/char/byte/boolean → i32` işlenen yığınında; alanlar depolama genişliğini korur (`i8/u16/i16`), `long → i64`, `float/double → f32/f64`, referanslar → `rt.Ref = ?*rt.Object`.
* **İşlenen yığını ve yerel değişkenler**: CFG üzerinde soyut yorumlama, her blok girişindeki yığın şeklini hesaplar; her `(derinlik, tür)` yuvası ve `(yerel, tür)` çifti bir Rust yerel değişkeni olur. Referans yuvaları çağrı başına GC çerçeve dizisi `R` içinde yaşar (kesin kökler).
* **Akış kontrolü**: temel bloklar, etiketli bir `switch`'in kollarıdır; atlamalar `continue :sw <pc>` olur (rustc/LLVM bunu doğrudan dallara derler). Düz metotlar switch olmadan üretilir.
* **Aritmetik** JVMS'i birebir izler: sarmalı `+% -% *%`, `/ by zero` ve `MIN_VALUE / -1` ile `idiv/irem`, maskeli kaydırmalar, doyuran `f2i/d2l`, NaN duyarlı `fcmpl/fcmpg`.
* **İstisnalar**: her metot `rt.E!T` döndürür; fırlatılan nesne iş parçacığı durumunda bekler. Bir try aralığındaki fırlatan işlemler `catch continue :sw D` kullanır; `D` kolu işleyicileri istisna tablosu sırasıyla denetler. Çalışma zamanı denetimleri (null, sınır, cast, dizi yazımı, negatif boyut, bölme) HotSpot ile aynı istisna sınıflarını ve mesajlarını fırlatır.
* **Dağıtım (dispatch)**: `invokevirtual` → vtable yuvası; `invokeinterface` → sınıf başına itable; `invokespecial/static` → doğrudan. Hedef private/final ise, sınıf final ise ya da örneklenmiş sınıflar üzerinde **sınıf hiyerarşisi analizi** tek bir olası hedef kanıtlıyorsa çağrılar devirtualize edilir (kapalı dünyada sağlam; programda geç bir değişiklik tam yeniden üretimi tetikler).
* **Canlılık**: bir çağrının tükettiği referans yığın yuvaları sonra temizlenir; böylece bayat yuvalar çöpü (ya da zayıf referanslı nesneleri) canlı tutmaz.
* **Sınıf başlatma**: JVMS §5.5 (sınıf başına durum, başlatan iş parçacığının özyinelemeli istekleri, bekleyen iş parçacıkları, `ExceptionInInitializerError` sarmalama, JDK 21 neden zinciriyle `NoClassDefFoundError`). JVMS'nin başlatmayı garanti ettiği yerlerde denetimler atlanır.
* **Eşzamanlama**: `monitorenter/exit` ve `synchronized` metotlar, tembel tahsis edilen, yeniden girilebilir nesne başına monitör kullanır (mutex + koşul); `wait/notify/notifyAll` dahil.
* **volatile** alanlar sıralı tutarlı atomikler kullanır.
* **VarHandle**: `findVarHandle/findStaticVarHandle/arrayElementVarHandle`, (tür, ofset) tanımlayıcıları üretir; imza-polimorfik çağrılar (`get`, `set*`, `compareAndSet`, `compareAndExchange*`, `getAndSet*`, `getAndAdd*`, `getAndBitwise*`) çağrı noktasının tanımlayıcısını kullanarak `monolith/rt/vh.rs` içinde atomik bellek işlemlerine indirgenir.
* **Satır numaraları**: `F.line`, istisna fırlatabilecek komutlardan önce güncellenir; böylece yığın izleri JVM gibi `File.java:NN` taşır.

## Çalışma zamanı (`runtime/`)

* `mem_pool.zig` ve `thread_pool.zig`: kilitsiz, sayfa destekli `ArenaAllocator` (`ArenaMark`, `madvise(MADV_DONTNEED)`), 3 çağlı (epoch) ertelenmiş geri kazanımlı ayrık `SlabPool` (`16..32768` B; sıfır STW duraklaması) ve koşul değişkenli `WorkerPool` + `WaitGroup` (`%0` boşta CPU döngüsü).
* `rt.zig`: nesne başlığı, diziler, sınıf metaverisi, `SlabPool`/`ArenaAllocator` üzerinden tahsis, bekleme gerektirmeyen `safepoint()` no-op'u, çerçeve kapsamları (`beginFrame`/`endFrame`/`pushArenaScope`/`popArenaScope`), işletim sistemi iş parçacıkları (`std.Thread`) ve yığın taşması koruması (`StackOverflowError`).
* `rhi.zig`, `rhi_vk.zig`, `rhi_gl.zig`, `rhi_bridge.zig`: LWJGL'siz özel RHI (`auto` → Vulkan 1.1+ → katı OpenGL 3.3 Core TBO + Intel HD 5500 gibi eski entegre GPU'lar için `glMapBufferRange` → Headless komut doğrulaması) ve `NativeRhi.java` JNI/yerel giriş noktaları.
* `world_render.zig` ve `gpu_instancing.zig`: Vanilla'ya güvenli `SectionSnapshot` üç aşamalı eleme (6 düzlemli FOV frustum, 6 yüzlü CaveCulling portal BFS, muhafazakâr `HiZPyramid`, piksel kapsamlı LOD) ve birleşik `64B` `GpuInstance` / `32B` `GpuParticle` blok dışı örnekleme (ekran varlıkları, yaratıklar, oyuncular, eşyalar, sandıklar, tabelalar ve parçacıklar).
* `natives.zig`: sınıf kütüphanesinin yerel metotları (Object, Class, System, Runtime, Thread, Throwable geri izlemeleri, Float/Double bitleri, reflect.Array, `MethodHandleNatives`, `NativeImageBuffer` ve `jdk.internal.misc.Unsafe` bellek/CAS/park API'si).
* `vh.rs`: VarHandle erişim modları (alan, statik, dizi elemanı, `byteArrayViewVarHandle` ve JDK 25 Foreign `MemorySegment` görünümleri).
* `io.zig`: `java.io` dosya akışları, `RandomAccessFile`, `UnixFileSystem` ve `java.util.zip` (`CRC32`, `Adler32`, `Inflater`, `Deflater`); JDK'nın da kullandığı sistem zlib'i üzerinde çalışır, bu yüzden sıkıştırılmış çıktı bayt bayt aynıdır.

## Kapalı dünya hizmetleri

* `ServiceLoader`: sağlayıcılar, derleme zamanında uygulamanın `META-INF/services` dosyalarından okunur, RTA'ya kök olarak eklenir ve yapıcı thunk'larından oluşan bir tabloya derlenir.
* `Class.forName("sabit")`, sınıf literalleri ve `Lookup.ensureInitialized`, hedef sınıfı ve `<clinit>`'ini canlı tutar (yansıtıcı referansların sabit katlanması).
* `Class.getDeclaredField/getField`, alan metaverisi tablolarıyla desteklenen `Field` nesneleri üretir (bildiren sınıf, ad, tür, değiştiriciler); `Unsafe`/`VarHandle` ofsetleri için yeterli.

## Sınıf kütüphanesi stratejisi (`classlib/src`)

Gerçek JDK `java.base` olduğu gibi çevrilir; yalnızca uygulaması HotSpot iç yapılarına bağlı olan sınıflar hariç. Bunlar `javac --patch-module java.base=…` ile derlenen kaplama kaynaklarıyla değiştirilir (GraalVM tarzı ikameler): `Object`, `String` (UTF-16 `char[]`), `StringBuilder/StringBuffer`, `Class`, `System`, `Runtime`, `Thread`, `ThreadLocal`, `Throwable`, `StackTraceElement`, `Integer`, `Long`, `PrintStream`, `java.lang.ref.*`, `java.util.concurrent.atomic.Atomic{Integer,Long,Boolean,Reference}`, `java.lang.invoke.{MethodHandles,VarHandle}`, `jdk.internal.misc.{VM,CDS}`, `jdk.internal.util.Preconditions`, `java.lang.CharacterName`.
Geri kalan her şey (koleksiyonlar, akışlar, `java.util.concurrent`, `Math/StrictMath` (fdlibm), `Double/Float.toString` (Raffaello Giulietti'nin algoritması), regex, ...) JDK'nın kendi kodudur; sonuçların JVM ile bit bit aynı olmasını sağlayan da budur.
