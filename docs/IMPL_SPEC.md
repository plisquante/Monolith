# Uygulama şartnamesi (kodu yazacak olan için)

Bu dosya `MASTER_PLAN_TR.md` ve `GC_PLAN.md` planlarının uygulanabilir halidir. Her görev: dosyalar, arayüz, kabul testi, boyut.
Plan ile bu dosya çelişirse bu dosya geçerlidir. Bir görevde bu dosyada yazmayan bir karar gerekirse: dur, kararı bu dosyaya ekle (gerekçesiyle), sonra kodla.

## 0. Çalışma kuralları (her görev için)

* Dal: oturumun verdiği dal. Commit yazarı `cem-ali152`. Commit mesajında Claude/AI, `Co-Authored-By`, `Claude-Session` yok. Asla force-push. `git push` ayrı komut (kanca, aynı komutta `mc`/`build` gibi sözcükler geçerse reddeder).
* Commitlenmez: `*.jar`, `mc/`, `out/`, `build/`, `tests/build`, shard çıktıları.
* Ortam: `MONO_JDK_HOME=/root/.cache/monolith/temurin-25/jdk-25.0.5.1+2` (testler javac ister). Python testleri: `python3 -m monolith.<test_adı>` (örn. `recycle_tests`, `stack_tests`, `patch_tests`).
* Bellek sınırı: çeviri (~5 GB) + derleme (2 paralel rustc) + oyun testi **aynı anda çalıştırılmaz**. Tam oyun derlemesi ~65 dk; yalnızca görev açıkça isterse başlatılır. Çoğu görev küçük test programlarıyla doğrulanır (`monolith/tests/*`).
* Çalışma zamanı (`monolith/rt/*.rs`) her değişince tüm shard nesneleri geçersiz olur (`rt_hash`). Yalnızca yeni fonksiyon/modül eklemek `--reuse-objs` ile ucuz derlenir; üretilen kodu değiştiren iş toplu yapılır (§3, Hat A).
* Hata durumunda sessiz geçiştirme yok: desteklenmeyen şey yüksek sesle başarısız olur. Stub, TODO, sahte başarı yok.
* Her görev sonunda: kabul testi çıktısı commit mesajına/PR notuna, ilgili doküman güncellemesi, commit + push.
* Ölçümsüz performans iddiası yok: sayı yazılacaksa araçla ölçülmüş olmalı.

## 1. Kararlar (kapalı)

| # | Karar |
|---|---|
| D1 | Tek arka uç Rust. Zig arka ucu (`runtime/*.zig`, `zigen.py`) kaldırıldı; Rust fark süiti yeşil (24/24, 4 atlandı). |
| D2 | `mono_rt` rustc ile doğrudan derlenmeye devam eder (`rustgen.py` ~satır 2299). Üçüncü parti crate'ler `rt_deps/` (Cargo workspace, `Cargo.lock`, `vendor/`) içinde cargo ile `--release` rlib olarak derlenir; `rustgen` `--extern ad=yol` ve `-L dependency=dizin` geçirir. Doğrulandı: cargo/rustc 1.97 kurulu, crates.io erişilebilir. Sürümler `Cargo.lock` + `cargo vendor` ile sabit, `rust-toolchain.toml` ile rustc sabit. |
| D3 | macOS: OpenGL 3.3 core. Gerekçe: bağlam zaten 3.3 core isteniyor (`natives_rhi.rs`), oyunun tüm gölgelendiricileri `#version 330`, macOS 4.1 core'a kadar destekler. Vulkan/MoltenVK yok. |
| D4 | Android: GLES 3.0+ ve gölgelendirici başlık dönüştürme (`#version 330 core` -> `#version 300 es` + precision) metin geçişi, Zig `gl_shim.zig` mantığı Rust'a taşınır (B6). naga/SPIRV yok. |
| D5 | Ses: OpenAL alt kümesi Rust'ta (aşağıdaki 23 fonksiyon, `monolith/rt/audio.rs`, bağımlılıksız). Aygıt çıkışı: Linux'ta `libasound` çalışma zamanında dlopen ile (`cpal` denendi: `alsa-sys` ALSA geliştirme başlıklarını derleme zamanında ister, bu da 'sistem paketi bağımlılığı sıfır' kuralına aykırı; karar değişti), Windows'ta WASAPI, macOS'ta CoreAudio, Android'de AAudio. Ogg çözme: oyun **JOrbis** kullanıyor (saf Java, zaten Rust'a çevriliyor); Rust Vorbis çözücü şimdilik gerekmez. |
| D6 | Pencere/giriş/ses aygıtı: Linux mevcut X11 kodu kalır. Windows/macOS/Android için iki aday yan yana denenir (B4): (a) `winit`+`glutin`(+`cpal`), (b) SDL3 (C, tek bağımlılık: pencere, GL/GLES bağlamı, giriş, oyun kolu, ses aygıtı, Android desteği). Kural: dört platformda spike'ı geçen seçilir; ikisi de geçerse Android desteği daha az bağımlılıkla olan (öncelikle SDL3) seçilir. Aşağıdaki ayrıntı (a) için geçerli: Windows/macOS: `winit` (pump_events ile oyun döngüsünden) + `glutin` (WGL/CGL), önce küçük test programı (B4). Oyunun ana iş parçacığı = render iş parçacığı = process ana iş parçacığı (macOS gereği). |
| D7 | TLS: önce çevrilmiş JSSE (`sun.security.ssl`) soketlerimizin üzerinde çalıştırılır (B5 spike). Gerçek uç noktaya (HTTPS GET) başarıyla bağlanırsa D7a: JSSE kalır. Bağlanamazsa D7b: sınıf seviyesi yama ile rustls. Karar B5 çıktısıyla bu tabloya yazılır. |
| D8 | Tahsis edici: kendi slab'ımız kalır. Üçüncü parti tahsis edici yalnızca ölçüm gerektirirse. |
| D9 | GC: eşzamanlı SATB işaretleme (arka plan thread'i), tembel süpürme, taşıma yok, nesil yok (§2). Kapatma yalnızca soak geçerse. |
| D10 | Launcher v1: `launcher.py`+`ui.html`, PyInstaller ile paketlenir. Rust'a yeniden yazım sürüm sonrası. |
| D11 | Her sürüm tek Minecraft sürümü (26.3). |
| D12 | CI: GitHub Actions yalnızca hafif doğrulama (lint, birim, küçük test programları). Ağır derleme için ≥16 GB RAM, 8 çekirdek, 100 GB disk bir Linux makine gerekir (kaynak kararı kullanıcıya ait; yoksa derleme bu oturum ortamında yapılır). |
| D13 | Hukuk: ertelendi; herkese açık dağıtım öncesi kapı (WS13). Özel test sürümleri serbest. |

## 2. GC ve bellek: teknik şartname (Hat A)

Mevcut yapı (kodda doğrulandı): `Obj { class: &Class, hash: u32, gc: u32 }` (16 B), `gc` bitleri `GC_HEAP=1, GC_MARK=2, GC_ARENA=4`;
slab 64 KiB, 14 boyut sınıfı; `collect()` durdur-tara-süpür; `safepoint()` = `GC_STOPPING` bayrağını yoklar; kökler: iş parçacığı yığınları+yazmaçlar (muhafazakâr), `static_slots`, `pinned_refs`; kesin yığın işaretleme `Class.ref_offsets`.

### 2.1 Tasarım ayrıntıları

* **Mark bitleri (epoch):** `gc` alanının bit 8..15'i `mark_epoch` (0 = işaretsiz). Bir nesne bu çevrimde işaretli ⇔ `(gc >> 8) & 0xFF == EPOCH`. Çevrim başında `EPOCH = (EPOCH % 255) + 1`. İşaretleme `AtomicU32::compare_exchange` ile. İşaret temizleme geçişi yok. Süpürülmemiş slab kalmışken yeni çevrim başlamaz (kalan slab'lar önce süpürülür).
* **Doğuşta siyah:** işaretleme (MARKING) sırasında yapılan her tahsisin `gc`'si `EPOCH<<8` ile doğar.
* **Kök anlık görüntüsü (tek duraklama):** `GC_STOPPING` ile tüm thread'ler safepoint'te durur; yığın+yazmaç taraması (mevcut kod), `static_slots`, `pinned_refs`, **canlı arena/frame parçaları** (thread başına `ArenaChunk`'lar; yığındaki frame nesneleri zaten yığın taramasında) işaretlenir; sonra `MARKING=1`, thread'ler bırakılır.
* **SATB bariyeri:** referans alan her yazmadan **önce** `if MARKING.load(Relaxed) { satb_push(eski_değer) }` (eski değer null değilse). İş parçacığı yerel tampon (1024 giriş), dolunca global kuyruğa boşaltılır.
* **Bariyer eklenecek yerler (hepsi, eksiksiz):**
  1. `rustgen.py` `putfield` ve `putstatic` (referans türlü alanlar: descriptor `L...;` veya `[`), satır ~519–552.
  2. `rt::aastore_ref` (`rt.rs`, ~satır 826).
  3. `System.arraycopy` referans dizileri (`natives.rs:71`): hedef aralığın **eski** değerleri için aralık bariyeri; taze (bu çevrimde doğan) hedefe kopya bariyer gerektirmez.
  4. `Unsafe` referans yazan yöntemler (`putReference*`, `compareAndSetReference`, `compareAndExchangeReference`, `getAndSetReference`, `natives.rs:325` civarı), `vh.rs` (VarHandle referans yazımı), yansıma `Field.set`, `rt::set_ref` (`rt.rs:341`).
  5. Yamalar (`patches/rust`) ve natives referans yazmak için yalnızca `rt::set_ref_field`/`rt::set_ref_array` kullanır; `monolith/patch_tests.py`'ye ham referans yazımını (`as *mut Ref` + atama) reddeden bir denetim eklenir.
  6. Atlanabilir: taze nesnenin kurucusundaki ilk yazımlar (eski değer her zaman null), frame/arena nesnelerine yazımlar (yığın anlık görüntüsüyle kapsanır), analizin kanıtladığı yerler.
* **İşaretçi iş parçacığı:** işçi kuyruğu, her nesne için `ref_offsets` ya da dizi elemanları; `CAS` ile işaretle; SATB global kuyruğunu boşalt. Görev döngüsü (duty cycle) ile ≤%10 tek çekirdek; sınır aşımında hız artar (§2.2).
* **Bitirme el sıkışması:** işaretçi kuyrukları boş görünce `DRAINING=1`; her mutator bir sonraki safepoint'te SATB tamponunu boşaltır (kısa duraklama); tüm tamponlar boş + işaretçi boş ⇒ **zayıf referans işleme** (§2.3) ⇒ `MARKING=0`, süpürme başlar.
* **Tembel süpürme:** slab, tahsisçi o slab'ı doldurmak için aldığında süpürülür (yalnızca işaretsiz ve `GC_HEAP` nesneler serbest). Boşta işaretçi thread'i kalan slab'ları süpürür. Büyük nesneler (`large` kaydı): çevrim sonunda arka planda küçük gruplar halinde.
* **Doğrulama modu `MONO_GC_VERIFY=1`:** her çevrim sonunda ikinci, tam durdur-işaretle (eski algoritma) çalışır; eşzamanlı çevrimin serbest bıraktığı ama durdur-işaretin erişebildiği her nesne için `abort` + sınıf adı + tahsis noktası.

### 2.2 Hız ayarı (pacing)

`trigger = max(32 MiB, live_son * 1.25)` (`MONO_GC_MB` taban). Çevrim sürerken `alloc_hız > işaretleme_hızı` ise görev döngüsü %10 → %30 → %60; `heap > soft_limit (live_son * 1.6)` olursa mutator yardımı (tahsis yavaş yolunda ≤200 µs işaretleme). `heap > hard_limit (live_son * 2.2 veya 2 GiB tavanı, hangisi küçükse)`: tek seferlik tam STW toplama, stderr'e büyük harfli uyarı, sayaç `hard_limit_events` artar (sürüm testinde 0 olmalı). Bu sayılar başlangıç değerleridir; faz 0/3 ölçümüyle ayarlanır ve bu dosyaya yazılır.

### 2.3 Zayıf referanslar (şu an eksik, öncelikli)

Kodda doğrulandı: `gc.rs` `java.lang.ref.Reference` için özel işlem yapmıyor (`referent` normal alan olarak izleniyor), yani `WeakReference/SoftReference/PhantomReference` referent'i **hiç temizlenmiyor** (WeakHashMap, ThreadLocalMap, Cleaner, önbellekler sızar). Şartname:
* `Reference.referent` alanı işaretlemede **izlenmez**. İşaretleme bitince bulunan `Reference` nesneleri (sınıf kalıtımı ile) türüne göre işlenir: Weak: referent işaretsizse temizle; Soft: yalnız `heap > soft_limit` iken temizle; Phantom/Final: referent işaretsizse temizle + kuyruğa al. Temizlenenler `ReferenceQueue`'ya `monoNext` listesiyle iletilir; bunu boşaltan bir "reference handler" thread'i (Java tarafı mevcut `Reference` kaplamasında) çalışır.
* Test: `monolith/tests/gc/WeakRefs.java` (WeakHashMap boşalır, SoftReference baskı altında temizlenir, ReferenceQueue'ya düşer), HotSpot ile çıktı karşılaştırması.

### 2.4 Katmanlar ve sahipli bellek

* **Frame/region:** mevcut. Eklenecek: tick/kare kapsam işaretleri (`rt::scope_begin/end`), kaçış kanıtı bulunan allocation'lar için `alloc_arena`.
* **Owned:** `rt::alloc_owned(class) -> Ref` (slab'dan, `gc` bayrağı `GC_OWNED=16`; süpürme **atlar**, işaretleme **içinden geçer**), `rt::release(Ref)` (serbest liste), `MONO_OWNED_CHECK=1`: serbest bırakılan bellek `0xDD` ile doldurulur, N=120 kare karantinada bekler, erişim `abort`. Yama manifest'inde `"owner": {"kind": "...", "release": "<metot>"}` alanı. Yalnız profil gösterdiği sahipler (en büyük: `PalettedContainer`, `BitStorage` dizileri, `[J` 64 MiB, `[B` 52 MiB) ve ölçümle onaylandıktan sonra.

### 2.5 Faz 0 araçları (önce yapılır)

* `prof.rs`: frame (`stack_obj`) tahsis sayacı (sınıf başına, `MONO_ALLOC_PROFILE` açıkken), duraklama histogramı (her çevrim: duraklama ms, kök-anlık-görüntü ms), heap/canlı eğrisi.
* `tools/leak_budget.py`: `headless_play.sh` ile oynat, ısınma sonrası 10 sn'de bir canlı yönetilen bayt örnekle, doğrusal eğim hesapla, `--budget-mib-per-min` aşılırsa çıkış kodu 1.
* `MONO_GC=off`: toplama tetiklenmez (hard limit hariç), sızıntı eğimi ölçülür.

## 3. Görev listesi

Etiket: **[H]** = sınırları net, küçük modelle yapılabilir; **[S]** = derin hata ayıklama/eşzamanlılık/kod üretimi, daha güçlü model önerilir. Hat A işleri tam yeniden üretim gerektirir, toplu yapılır.

### Hat B (platform; oyunu beklemeden)

| ID | Görev | Dosya | Kabul testi | Etiket |
|---|---|---|---|---|
| B0 | `rt_deps/` workspace, `rust-toolchain.toml`, `vendor/`, `rustgen` `--extern` aktarımı, `docs/THIRD_PARTY.md` | `rt_deps/`, `monolith/rustgen.py` | küçük bir crate (örn. `crc32fast`) bağımlılığı `mono_rt` içinde kullanılır, ağsız (`CARGO_NET_OFFLINE=true`) derlenir, `monolith/tests` bir programı çalışır | [H] |
| B1 | Native envanteri: `MONO_TRACE_NATIVES=1` her native çağrı için sayaç (tablo: sınıf.metot, çağrı sayısı, JNI/JNA/Rust-native ayrımı) çıkışta yazar; `tools/native_report.py` | `monolith/rt/natives*.rs`, `tools/` | `headless_play.sh` ile 60 sn: rapor üretilir, JNI/JNA satırları listelenir; test programında sayaçlar HotSpot beklentisiyle tutar | [H] |
| B2 | Ses çekirdeği: `monolith/rt/audio.rs` — OpenAL alt kümesi, yazılım karıştırıcı. Fonksiyonlar (23): `alcOpenDevice alcCloseDevice alcCreateContext alcDestroyContext alcMakeContextCurrent alcGetString alcGetIntegerv alGenSources alDeleteSources alGenBuffers alDeleteBuffers alBufferData alSourcei alSourcef alSource3f alListener3f alSourcePlay alSourcePause alSourceStop alSourceQueueBuffers alSourceUnqueueBuffers alGetSourcei alGetError`. Davranış: PCM 8/16 bit mono/stereo, kuyruk, `AL_BUFFERS_PROCESSED`, `AL_SOURCE_STATE`, `AL_LOOPING`, pitch, gain, mesafe azalması (linear/inverse, `alDistanceModel` yok: LWJGL ek uzantıları mevcut kaplamada), dinleyici konumu. | `monolith/rt/audio.rs` | `cargo test`/rustc testi: ton karıştırma doğru örnek değerleri, kuyruk/kuyruktan çıkarma semantiği, hata kodları; WAV dosyasına çıkış arka ucu ile 1 sn deterministik ton | [H] |
| B3 | Ses bağlama: `natives_rhi.rs` `alc*/al*` çağrıları `audio.rs`'ye gider; `MONO_AL=system` ortam değişkeni eski `libopenal` yoluna düşer. `cpal` aygıt arka ucu. | `natives_rhi.rs`, `audio.rs` | headless oyun: ses aygıtı yok ortamında hata yerine sessiz çıkış (null backend) ve `SoundSystem` hatası yok; aygıtı olan makinede (kullanıcı) kulakla doğrulama notu | [H] |
| B4 | `Platform` trait + Windows/macOS spike: `winit` `pump_app_events` + `glutin` ile pencere, GL 3.3 core bağlam, swap, klavye/fare/pencere olayları → mevcut X11 yolundaki aynı olay modeli | `monolith/rt/platform/` | Linux'ta spike çalışır (X11 üstünde winit); CI'da Windows (`windows-latest`) ve macOS (`macos-latest`) koşucularında pencere açılıp kapanan test programı yeşil | [S] |
| B5 | TLS spike: küçük Java programı `HttpsURLConnection` ile `https://sessionserver.mojang.com/` (ya da herhangi bir genel HTTPS uç noktası) GET; çevrilmiş JSSE ile. Sonuç D7'ye yazılır | `monolith/tests/tls/` | program 200/ geçerli yanıt döndürür (D7a) ya da raporlu başarısızlık (D7b'ye geçiş) | [S] |
| B6 | GLES gölgelendirici dönüştürme: `330 core` → `300 es` metin geçişi (Rust, `monolith/rt/shader_es.rs`), `glslangValidator` (apt `glslang-tools`) ile doğrulama | `monolith/rt/shader_es.rs`, `tools/check_shaders_es.py` | oyun kaynaklarındaki 72 gölgelendiricinin hepsi ESSL doğrulamasından geçer (ya da istisna listesi gerekçeli) | [H] |
| B7 | Paketleme: PyInstaller ile launcher, `release` GitHub iş akışı iskeleti (Linux), SHA-256 manifest | `launcher/`, `.github/workflows/` | temiz dizinde paket çalışır; manifest doğrulanır | [H] |
| B8 | Belgeleri düzenle: eski `STATUS.md`/`ARCHITECTURE.md` "tarihî" başlığı, kullanıcı rehberi (kurulum/sorun giderme), `THIRD_PARTY.md` | `docs/` | bağlantılar çalışır, güncel plan ana sayfadan bulunur | [H] |

### Hat A (çekirdek; toplu yeniden üretim)

| ID | Görev | Kabul testi | Etiket |
|---|---|---|---|
| A1 | Faz 0 araçları (§2.5) | araçlar çalışır; mevcut build'in duraklama histogramı ve eğrisi `docs/ALLOC_BASELINE.md`'ye yazılır | [H] |
| A2 | Epoch mark biti + tembel süpürme, **hâlâ durdur-işaretle** olarak (davranış değişmez) | `monolith/tests/gc/*` ve `GcStress` HotSpot ile aynı çıktı; oyunda 4 dk, ihlal yok; duraklama artmaz | [S] |
| A3 | Zayıf referanslar (§2.3) | `WeakRefs` testi; `ThreadLocal`/`WeakHashMap` kaçağı yok | [S] |
| A4 | SATB bariyeri (kod üretimi + `rt` API + yama denetimi, §2.1 madde 1–5) | bariyer kapalıyken (`MARKING=0`) üretilen kod davranışı aynı (HotSpot karşılaştırma); bariyer maliyeti mikro ölçüm <%3 | [S] |
| A5 | Eşzamanlı işaretçi + el sıkışma + pacing + yardım + sert limit (§2.1–2.2) | `monolith/tests/gc/` stres (çok thread, bağlı yapılar, küçük eşik); `MONO_GC_VERIFY=1` ile 30 dk oyun temiz; en uzun oyun-thread duraklaması <1 ms (kök anlık görüntü) | [S] |
| A6 | Sınıf seviyesi yama: manifest `class` girdisi, `patches.py` doğrulayıcı + codegen | yama testi: bir sınıfın birkaç metodu ve yeni alanı değişir, orijinale düşme ve verify modu çalışır | [S] |
| A7 | Çöp azaltma yamaları (profile göre, sıra): `AbstractObjectList.listIterator` geri dönüşüm, `VoxelShape.move/collideX`, `forEachBlockIntersectedBetween`, `Shapes.create [D`, `NaturalSpawner.spawnForChunk`, `Entity.checkInsideBlocks` | her yama: yama testi + `MONO_PATCH_VERIFY=1` + oyunda tahsis profili farkı | [S]/[H] karma |
| A8 | Sahipli bellek (§2.4) + ilk sahipler | `MONO_OWNED_CHECK=1` oyun testi temiz; sızıntı eğimi düşer (A1 aracı) | [S] |
| A9 | `MONO_GC=off` soak (60 dk) ve karar | `leak_budget.py` bütçe içinde ⇒ GC varsayılan kapalı | [H] (ölçüm) |

### Kalan iş akışları (B ve A bitince)

* WS2: B1 raporundaki her JNI/JNA girişi için Rust-native karşılık; sıfır hedefi. [H] her giriş.
* WS7: Java kaplamalarını Rust'a taşıma: sıra `java_nolwjgl/opengl` (kullanılanlar), `openal`, `sdl/glfw`, sonra `classlib`; kullanılmayan Vulkan dosyaları silinir. [H] dosya başına.
* WS12 (Android), WS3 gerçek GPU testleri, WS11 dağıtım: donanım/cihaz gerektirir; kullanıcının cihazlarında doğrulama.

## 4. Sıra

1. Hat B: B0, B1, B2 (paralel yapılabilir, birbirine bağımlı değil). B3, B6, B7, B8. B4 ve B5 spike'ları.
2. Hat A: A1 → A2 → A3 → A4 → A5 (A4 ve A5 tek tam yeniden üretimle gelir) → A7/A6 → A8 → A9.
3. Tam yeniden üretim yalnızca: (a) bekleyen iterator düzeltmesi + A7 yamaları birlikte, (b) A4/A5, (c) A8. Her biri sonrası oyun testi + tahsis profili + belge.

## 5. Açık kalanlar (kod yazmakla kapanmaz)

* CI makinesi (D12): kaynak sağlanması kullanıcıya ait.
* Hukuk kapısı (D13): herkese açık dağıtım öncesi karar.
* Gerçek GPU/cihaz testleri: kullanıcı cihazlarında veya erişilebilir donanımda.
* D7 (TLS) ve D6 (winit modeli) spike sonuçlarına bağlıdır; her iki durum için de kabul testi ve yol yazılıdır.


## 6. Durum (her commit'te güncellenir)

| Görev | Durum | Kanıt |
|---|---|---|
| B0 rt_deps (vendored, çevrimdışı derleme, rustgen bağlantısı) | tamam | `monolith/crc_tests.py` (CRC32 HotSpot ile eşleşiyor); `863e99d` |
| B1 native envanteri (976 native, hepsi rt'de) | tamam (statik; çalışma zamanı sayacı sonraki tam üretimde) | `docs/NATIVES.md`, `tools/native_report.py` |
| Netty NIO varsayılanı (174 epoll/kqueue native'i yükten çıkarır) | tamam | `monolith/netty_tests.py`; `1a3120f` |
| B2 ses çekirdeği (OpenAL alt kümesi, karıştırıcı, WAV çıkış) | tamam (çekirdek; aygıt yok) | `rustc --test monolith/rt/audio.rs`: 7/7 |
| B3 ses bağlama + aygıt arka uçları | kısmen: OpenAL natives Rust karıştırıcısına bağlı (23 fonksiyon), ALSA dlopen + sessiz yedek çalışıyor, `MONO_AUDIO_TRACE` ile hata izi. Oyunda 60 sn'de ses hatası 0 (öncesi 379). WASAPI/CoreAudio/AAudio açık | `monolith/rt/audio_dev.rs`, `tests/rust/audio_main.rs` (CI), `278461f`, `aee7d36` |
| CI lint | düzeltildi: pyflakes 20 bulgu → 0; ses testleri ve rt_deps çevrimdışı derleme CI'a eklendi | `4bfb0cf` |
| B5 TLS spike | **sonuç D7a: JSSE korunur.** Çevrilmiş çalışma zamanından HTTPS GET 200 (HTTP/2). Bulunan engeller ve çözümler: sağlayıcı sınıfları yansımayla oluşur (tüm sağlayıcı paketleri yansıma listesine alındı), X.509 uzantıları, ICU veri dosyaları (`MONO_RESOURCES`), güven deposu (`javax.net.ssl.trustStore`, `run.sh` ve `resources/cacerts`). Hepsi `tools/jdk_runtime_data.py` ve `tools/mcbuild_rust.py` içinde | `monolith/tests/tls/TlsProbe.java`, `tools/jdk_runtime_data.py` |
| B4 pencere spike (winit/glutin, SDL3) | açık | Windows/macOS koşucusu gerekir |
| B6 GLES gölgelendirici dönüşümü | açık | |
| B7 paketleme | açık | |
| B8 belgeler | kısmen (NATIVES.md, THIRD_PARTY.md) | |
| A1–A9 (GC, yama, sahiplik, soak) | açık | GC_PLAN / IMPL_SPEC §2 |
| WS7 Java kaplamaları → Rust | açık | |
| WS12 Android cihaz, WS3 gerçek GPU, WS10 CI makinesi | açık, donanım/kaynak gerektirir | |


## 7. Bekleyen tanılama ve kararlar (oturum notları)

- **`--reuse-objs` tehlikesi (kanıtlandı):** `rustgen._older_obj` kaynağı aynı olan herhangi bir eski shard nesnesini,
  hangi çalışma zamanıyla derlendiğine bakmadan geri koyar. Bir native'in gövdesi değişip shard içine satır içi
  alınmışsa eski davranış sessizce kalır (JNA `initIDs`: yeni kod hiç çağrılmadı). Karar: gövde değişen herhangi bir
  değişiklikten sonra tam yeniden derleme (`--resume` ve `--reuse-objs` yok). `--reuse-objs` yalnızca yeni fonksiyon
  eklemek gibi değişikliklerde ve bilinçli olarak kullanılır.
- **Önceki ses doğrulamaları** `--reuse-objs` ile derlendi; tam yeniden derleme sonrası yeniden doğrulanacak.
- **JNA `initIDs`:** `natives_jni.rs` içinde FFI tip alanlarını dolduran gövde yazıldı; narrator hatasının kapanması
  tam derlemeden sonra doğrulandı: `initIDs` çalışıyor, FFI/narrator/ses hatası 0 (20 sn headless). Kalan üç ERROR
  satırı dış ağa çıkamayan ortamdan geliyor (Realms, kullanıcı özellikleri).


## 8. GC ölçümleri (bu oturum, temiz çalışma, profil kapalı)

Kaynak: `MONO_GC_PHASES=1`, 150 sn headless oyun, 33 toplama.

| canlı heap | bekleme | kök | işaretleme | süpürme | toplam duraklama |
|---|---|---|---|---|---|
| 10 MB | 1 µs | 1,5 ms | 11 ms | 5 ms | 18 ms |
| 63 MB | 3 µs | 2,6 ms | 117 ms | 36 ms | 155 ms |
| 145 MB | 10 ms | 4,9 ms | 268 ms | 54 ms | 337 ms |
| 495 MB | 3 µs | 6,4 ms | 869 ms | 254 ms | **1 130 ms** |

- Duraklama canlı heap ile doğrusal büyür: işaretleme ~1,8 ms/MB, süpürme ~0,5 ms/MB. Kök taraması ihmal edilebilir.
- En uzun toplam duraklama 1,14 sn. Önceki "profil kaynaklı" iddiası YANLIŞTI; 1,3–1,8 sn rakamları gerçekti, ağır heap'te ortaya çıkıyor.
- Canlı heap 150 sn içinde 10 MB'dan 495 MB'a çıktı. Bu, oyun içinde yükleme (chunk) dışında bir büyüme olabilir: sızıntı bütçesi aracı ile ayrılmalı (A1).
- Bariyer mikro ölçümü (`bench.rs`, bu oturumda): bariyer kapalıyken yazma 1,46 ns, açıkken (tampona itme dahil) 5,63 ns; yani işaretleme penceresinde ~3,9×. Pencere kısa olduğu sürece ortalama maliyet düşük, ama oyunda ölçülmeden %3 hedefi doğrulanmış sayılmaz.

## 9. GC panic "fresh slab has slot" (kanıtlandı ve düzeltildi)

- Oyun 26 dk'da çöktü (`gc.rs`, alloc_small_slow). Kök neden: kısmi (partial) listedeki bir slab'in boş yeri olmayabiliyordu;
  thread çıkışında dolu slab'ler de listeye geri konuyordu, tahsis yolu ise boş yer varsayıyordu.
- Düzeltme (`5fc6a64`): kısmi listeden yalnızca boş yeri olan slab alınır; thread çıkışında yalnızca boş yeri olanlar
  geri konur; tahsis dolu bir slab bulursa yeniden dener (panic yok).
- Kanıt: `monolith/tests/GcSlabStress.java`, `MONO_GC_MB=1`. Düzeltme öncesi (`39d3c16`): 3/3 çökme (çıkış 134).
  Düzeltmeli: 3/3 geçti, checksum HotSpot ile aynı (276071380).

## 10. GC kilitlenmesi: thread bitişi sırasında toplama (kanıtlandı ve düzeltildi)

- `GcSlabStress` (`MONO_GC_MB=1`) 10 koşuda 2 hang. gdb: toplayıcı tüm thread'lerin durmasını bekliyor; biten thread
  `unregister_thread` içinde toplamanın bitmesini bekliyor ve `paused` bayrağını set etmiyor. İkisi de sonsuza kadar bekler.
- Düzeltme (`e3cca06`): `unregister_thread` güvenli nokta gibi davranır: yığın ve yazmaçları kaydeder, beklerken `paused` set eder.
- Kanıt (düzeltilmiş ikili): 20/20 koşu, checksum HotSpot ile aynı, hang 0.

## 11. Kayıtlı sızıntı kapısı: geçti (tek koşu)

- Ölçüm: tam yeniden derlenmiş oyun, düzeltmeli GC (`e3cca06` ve önceki düzeltmeler), 60 dk sabit durum (`tools/leak_soak.sh`).
- Kural (`docs/GC_PLAN.md`, önceden kaydedildi): ısınma 30 dk, eğim 30–60. dakika, bütçe 1 MiB/dk.
- Sonuç: eğim **+0,013 MiB/dk**, 43 ölçüm noktası, canlı heap son değer 224 MiB, en uzun duraklama 806 ms.
- Sınır: tek koşu. Kapının "geçti" sayılması bu tekrarlanabilirliği kanıtlamaz; ikinci bir koşu ve farklı bir senaryo gerekir.

## 12. SATB algoritma prototipi (kanıtlandı, gc.rs'e dokunulmadı)

- `tests/rust/satb_proto.rs`: eşzamanlı işaretleme, mutatörler işaretleme sürerken referans değiştirir, her yazma bariyerden geçer.
- Sonuç: 50 deneme geçti; aynı test bariyer kapalıyken ilk denemede canlı bir nesneyi kaçırıyor (`lost`). Yani bariyer gerekli ve bu
  tasarım yeterli (bu model için). `gc.rs`'e entegrasyon ayrı, toplu bir yeniden derleme işi.

## 13. Eşzamanlı işaretleme (MONO_GC_CONCURRENT=1), ilk ölçümler

- Uygulama: `gc.rs` (kök anlık görüntüsü STW, işaretleyici thread, SATB günlüğü, son toplama işaretleyici tarafından STW).
  Varsayılan kapalı; kapalıyken davranış değişmedi. Yeni nesneler döngü sırasında siyah doğar.
- `tests/GcConcurrentStress.java` (4 thread, 2500 tur, liste ters çevirme): düz ve eşzamanlı modda toplam 2261257500000 = HotSpot;
  panic yok; işaretleyici thread 62 örneğin 10'unda görüldü (döngüler gerçekten çalıştı).
- Son toplama duraklaması (küçük stres yığını, yalnız STW kısmı): eşzamanlı maks 7,4 ms / ort 2,2 ms; düz maks 9,0 ms / ort 5,1 ms.
  Bu bir küçük yığın ölçümü; 500 MB'lık oyun yığınında kazanç henüz ölçülmedi. Başlangıç (kök) duraklaması ölçülmedi.
- Sınır: bu, gerçek oyun ikilisinde (tam yeniden derleme + sızıntı kapısı benzeri soak) doğrulanmadı.
