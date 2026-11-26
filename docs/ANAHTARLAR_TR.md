# Anahtarlar (ortam değişkenleri)

Son güncelleme: 27 Kasım 2026. Kaynak: koddaki `MONO_*` okumaları (`monolith/*.py`, `monolith/rt/*.rs`,
`android/touchcore`). Kural: her optimizasyon bir anahtarla açılır; eski yol yerinde kalır ve anahtar yoksa o çalışır.
Ölçümle kanıtlanan anahtar varsayılan yapılır ve burada "varsayılan" diye yazılır.

İki tür anahtar var:

- **Üretim zamanı:** derlerken verilir (`MONO_X=1 ./build.sh ...`). Üretilen Rust kodunu ya da derleyici bayraklarını
  değiştirir. Telefonda değiştirilemez; yeni motor derlemesi gerekir.
- **Çalışma zamanı:** motor açılırken okunur. Masaüstünde `MONO_X=1 ./run.sh`, Android'de başlatıcının
  "Ek Ortam Değişkenleri" alanı (yalnızca `MONO_*` anahtarları geçer, `EngineResources.applyExtraEnv`). Yeni derleme gerekmez.

## 1. Üretim zamanı (çevirici ve derleyici)

| anahtar | değerler | varsayılan | ne yapar | kod |
|---|---|---|---|---|
| `MONO_FRAMES` | `fast`, `dwarf` | kapalı (eski defter) | `fast`: ucuz gölge çerçeve (işaretçi + satır) ve birleşik yığın denetimi. `dwarf` (C9): çerçeve kaydı yok, yığın izi hata anında makine yığınından; deneysel, satır numaraları HotSpot'la aynı değil | `rustgen.py` |
| `MONO_ICACHE` | `1` | kapalı | arayüz çağrısı, `checkcast`, `instanceof` için çağrı yeri önbelleği | `rustgen.py`, `rt.rs` |
| `MONO_INTRINSICS` | `1` | kapalı | `Math` metotları doğrudan Rust işlemi | `rustgen.py` |
| `MONO_INIT_ELIDE` | `1` | kapalı | gereksiz sınıf başlatma denetimlerini kaldırır | `rustgen.py` |
| `MONO_NULL_ELIDE` | `1` | kapalı | `this` ve yeni nesne üzerindeki null denetimlerini kaldırır | `rustgen.py` |
| `MONO_LINE_LAZY` | `1` | kapalı | satır numarası yazımını erteler | `rustgen.py` |
| `MONO_SHARD` | `pkg` | kapalı (sınıf sırasıyla bölme) | paket bazlı parçalama, aynı parçada sınıflar arası doğrudan çağrı | `rustgen.py` |
| `MONO_CONCAT` | `exact` | kapalı | dizgi birleştirmede tam boy tek dizi | `indy.py` |
| `MONO_REGION_CLASSES` | virgüllü sınıf listesi | boş | bu sınıfların nesneleri bölgeden ayrılır (GC'siz yol). Kanıtlı 5 sınıf: `Vec3, AABB, Optional, Pair, DataResult$Success` | `rustgen.py`, `region.rs` |
| `MONO_MIR` | dolu = açık | kapalı | MIR ara katmanı ile üretim | `rustgen.py` |
| `MONO_MIR_NO` | virgüllü ad listesi | boş | MIR'de adı verilen geçişleri kapatır | `mir_opt.py` |
| `MONO_NATIVE_TLS` | `0` | **açık (Android)** | çerçeve yığını gerçek yerel TLS'te (`#[thread_local]`, TLSDESC). `0` = eski `thread_local!` (pthread anahtarı). Motor yalnızca Android 10+ (API 29) yüklenir. S26: FPS 29,8 -> 60,8 | `rustgen.py`, `rt.rs` |
| `MONO_TLS_SLOT` | `1` | kapalı | (yerel TLS açıkken) çerçeve yığını adresi bionic'in `TLS_SLOT_APP` yuvasında: her Java çağrısında bağlayıcıya giden TLSDESC çağrısı kalkar. API 29+. Varsayım: süreçte bu yuvayı başka kod kullanmıyor | `rustgen.py`, `rt.rs` |
| `MONO_SYM_CACHE` | `1` | kapalı | (yerel TLS açıkken) GL ve TouchCore işlev adresleri çağrı yeri başına bir kez `dlsym` ile aranır; eskisi her çağrıda arar (S26 v9: CPU'nun %7,5'i) | `rustgen.py`, `desk/mod.rs` |
| `MONO_PATCH_MANIFEST` | dosya yolu | `patches/rust/manifest.json` | elle yazılmış Rust yamalarının listesi | `rustgen.py` |
| `MONO_COUNT_NATIVES` | `1` | kapalı | yerel metot çağrılarını sayan kod üretir (çalışma zamanında tablo yazılır) | `rustgen.py`, `ncount.rs` |
| `MONO_SHARD_CACHE_GB` | sayı | `8` | parça nesne önbelleğinin üst sınırı | `rustgen.py` |
| `MONO_JDK_HOME` | dizin | - | JDK 25 yeri (zorunlu; JAVA_HOME 21 olabilir ve sessizce yanlış çıktı verir) | `classpath.py` |
| `MONO_NO_BUILTIN` | `1` | kapalı | yerleşik Java köprüsünü (`runtime/java`, `runtime/java_nolwjgl`) derlemez | `classpath.py` |
| `MONO_VERBOSE` | `1` | kapalı | bağlantı hatalarını listeler | `rustgen.py` |
| `MONO_ESCAPE_DUMP` | dosya öneki | kapalı | kaçış çözümlemesi sonuçlarını bu önekle dosyaya döker | `escape.py` |

Android motoru şu an bu ayarlarla derleniyor (`build/minecraft-26.3-android-v3`, v10):
`MONO_NATIVE_TLS=1 MONO_TLS_SLOT=1 MONO_SYM_CACHE=1 MONO_FRAMES=fast MONO_ICACHE=1 MONO_INTRINSICS=1 MONO_INIT_ELIDE=1
MONO_SHARD=pkg MONO_CONCAT=exact MONO_NULL_ELIDE=1 MONO_LINE_LAZY=1 MONO_REGION_CLASSES=<5 sınıf>`, `--opt 3 --shards 48`.

## 2. Çalışma zamanı: GC ve bellek

| anahtar | değerler | varsayılan | ne yapar | kod |
|---|---|---|---|---|
| `MONO_GC_CONCURRENT` | `0` / `1` | **Android açık**, masaüstü kapalı | eşzamanlı işaretleme (duraklamalar kısalır) | `gc.rs` |
| `MONO_GC_LAZY` | `0` / `1` | **Android açık**, masaüstü kapalı | tembel süpürme (dilimler tahsis sırasında süpürülür) | `gc.rs` |
| `MONO_GC_PHASES` | `0` / `1` | **Android açık**, masaüstü kapalı | her toplamada faz süreleri satırı (`[mono:gc_phases]`) | `gc.rs` |
| `MONO_GC_MB` | MB | `32` | toplamayı tetikleyen ayırma eşiği | `gc.rs` |
| `MONO_GC` | `off` | açık | otomatik toplama yok (yalnızca ölçüm için) | `gc.rs` |
| `MONO_GC_POISON` | `1` | kapalı | serbest bırakılan yuvalar `0xDD` ile dolar; bayat referans hemen çöker (tanı) | `gc.rs` |
| `MONO_ARENA` | `0` | açık | iş parçacığı arenası | `gc.rs` |
| `MONO_ARENA_CHECK` | `1`, `abort` | kapalı | arena ihlallerini raporlar / ilk ihlalde durdurur | `gc.rs` |
| `MONO_RECYCLE` | `0` | açık | for-each yineleyici nesnelerini geri dönüştürür | `gc.rs` |
| `MONO_REGION` | `0` | açık (derlemede sınıf listesi varsa) | bölge ayırmayı kapatır | `region.rs` |
| `MONO_REGION_QUARANTINE` | `1` | kapalı | serbest bırakılan bölge parçası erişime kapanır; bayat bölge referansı anında SIGSEGV (tanı) | `region.rs` |

## 3. Çalışma zamanı: profil ve tanı

| anahtar | değerler | varsayılan | ne yapar | kod |
|---|---|---|---|---|
| `MONO_CPU_PROFILE` | dosya yolu; Android'de `1` | kapalı | SIGPROF örnekleyici, JSON + `.classes`. Android'de `1` -> `Android/data/.../files/cpu_profile.json`, rapor zip'ine girer. Çözümleme: `tools/cpu_report.py` | `cpuprof.rs` |
| `MONO_CPU_HZ` | 20..1000 | `100` | örnekleme sıklığı | `cpuprof.rs` |
| `MONO_CPU_DUMP_SECS` | saniye | `10` | profil dosyasının yazılma aralığı | `cpuprof.rs` |
| `MONO_ALLOC_PROFILE` | dosya yolu; Android'de `1` | kapalı | ayırma profili (sınıf başına sayım). Çözümleme: `tools/alloc_report.py` | `prof.rs` |
| `MONO_ALLOC_SAMPLE` | bayt (en az 1024) | - | örnekleme aralığı | `prof.rs` |
| `MONO_ALLOC_DUMP_SECS` | saniye | `10` | profil yazma aralığı | `prof.rs` |
| `MONO_ALLOC_SNAPSHOTS` | dolu = açık | kapalı | her dökümün ayrı kopyasını saklar | `prof.rs` |
| `MONO_TRACE_EX` | dolu = açık | kapalı | istisna izleme | `rt.rs` |
| `MONO_TRACE_INIT` | dolu = açık | kapalı | sınıf başlatma beklemelerini yazar | `rt.rs` |
| `MONO_TRACE_PARK` | dolu = açık | kapalı | monitör, bekleme, park ve GC durdurma anlarında geri izleme | `sync.rs`, `gc.rs` |
| `MONO_PATCH_VERIFY` | `1` | kapalı | yamalı metotlar orijinalle karşılaştırılır, ilk farkta durur | `patch.rs` |
| `MONO_JNA_TRACE` | dolu = açık | kapalı | JNA çağrılarını yazar | `natives_jni.rs` |
| `MONO_AUDIO_TRACE` | dolu = açık | kapalı | ses karıştırıcı günlüğü | `audio.rs` |
| `MONO_GL_STATS` | `1` | kapalı | saniyede bir `[mono:gl_stats]` satırı: kare, çizim çağrısı, indis, yüklenen KB, eşleme / başarısız eşleme, bozulmuş arabellek (unmap false), atlanan çizim, GL hata sayısı. Unix ms damgalı (frametime.csv ile aynı saat) | `natives_rhi.rs` |
| `MONO_GL_CHECK` | `1` | kapalı | her çizim ve arabellek çağrısından sonra `glGetError`; hata çağrı adıyla yazılır (`[mono:gl]`, ilk 200) | `natives_rhi.rs` |
| `MONO_CRASH_DIR` | dizin | - | yerel çökme raporunun yazılacağı yer (`mono-crash-<pid>.txt`) | `crash.rs` |

## 4. Çalışma zamanı: platform

| anahtar | değerler | varsayılan | ne yapar | kod |
|---|---|---|---|---|
| `MONO_WORKER_NICE` | 1..19 | kapalı | (Android) "Worker-" iş parçacıklarının önceliğini düşürür; dünya üretimi çizimi boğmasın | `sync.rs` |
| `MONO_DISPLAY_BACKEND` | `x11`, `wayland` | otomatik | Linux pencere sistemi seçimi | `desk/linux.rs` |
| `MONO_WINDOW_SIZE` | `GxY` (ör. `1280x720`) | - | başlangıç pencere boyutu | `desk/*.rs` |
| `MONO_WAYLAND_DEBUG`, `MONO_WAYLAND_LEGACY` | dolu = açık | kapalı | Wayland tanı / eski yol | `desk/wayland.rs` |
| `MONO_AUDIO_DEVICE` | aygıt adı | `default` | ses çıkış aygıtı | `audio_dev.rs` |
| `MONO_AUDIO_SILENT` | dolu = açık | kapalı | sessiz çıkış | `audio_dev.rs` |
| `MONO_NATIVES` | dizin | - | yerel kütüphanelerin aranacağı yer (başlatıcı verir) | `desk/mod.rs` |

## 5. Android başlatıcısının kendisi verdikleri

`EngineResources.java` motoru açmadan önce bunları ayarlar; kullanıcı değiştirmez: `MONO_RESOURCES`, `MONO_JAVA_HOME`,
`MONO_NATIVES`, `MONO_RHI=1`, `MONO_NO_LWJGL=1`, `MONO_RHI_BACKEND=headless`.

**Okunmayanlar:** başlatıcı `MONO_HEAP_MAX_MB`, `MONO_GC_MIN_MB`, `MONO_GC_FACTOR`, `MONO_GC_MAX_GARBAGE_MB`,
`MONO_GC_VERBOSE` de verir, ama bugünkü motorda bunları okuyan kod yok (27 Kasım 2026'da arandı). Etkisizdirler; GC
eşiği `MONO_GC_MB` ile değişir.

## 6. Testler

`MONO_TEST_ARGS` (test argümanları), `MONO_NETTY_DIR`, `MONO_TEST_MIN_AVAIL_GIB` / `MONO_MIN_AVAIL_GIB` (bellek alt sınırları,
`docs/HARDWARE.md`).
