> **Tarihî belge (Zig dönemi).** Güncel plan: [`MASTER_PLAN_TR.md`](MASTER_PLAN_TR.md). Bellek planı: [`GC_PLAN.md`](GC_PLAN.md).

# MONOLITH Client Yol Haritası: %100 Çalışır Hale Gelmek ve JVM Minecraft'ı Geçmek İçin Eksikler

Bu doküman, repodaki güncel kod tabanı ve doğrulanmış ölçümler esas alınarak hazırlanmıştır. Eski `.md` bölümlerindeki tarihî notlarla karışmaması için yalnızca **şu an kodda gerçekten var olanlar**, **doğrulananlar** ve **somut eksikler** ayrılmıştır.

---

## 1. Güncel Durum Özeti (Ne Çalışıyor, Ne Eksik?)

| Bileşen / Hedef | Şu Anki Gerçek Durum | Kanıt / Konum |
|---|---|---|
| **Java → Zig Transpiler & Çekirdek Testler** | Çalışıyor (`Arith PASS`, `MHandles PASS`, `ClientCoreE2E 4/4 PASS`, `rhi 7/7`, `mem_pool 6/6`, `gpu_instancing 12/12`, `world_render 9/9`) | `tests/run_tests.py`, `tests/run_client_core_e2e.py`, `docs/STATUS.md` §20 |
| **Otomatik Escape Analysis (`ArenaScope`)** | Eklendi ve doğrulandı — döngüsüz metodlarda kaçmayan `new`/`newarray`/`anewarray` nesneleri `rt.newObjectArena` / `rt.arenaMark` ile sıfır-malloc tahsis edilir | `monolith/zigen.py` (`_analyze_local_allocs`), `runtime/rt.zig` |
| **`26.3-client.jar` `--no-lwjgl` LWJGL Uyumu** | **0 LWJGL link hatası** (`61,280` sınıf / `278,867` metot RTA analizi ile `345` hatadan `0`'a indirildi); `SDL3` (`org.lwjgl.sdl.*`) + `GL33C` + `ARB*` + `OpenAL` + `STBImageResize`/`Write` + `MemoryUtil` tamamlandı | `runtime/java_nolwjgl/org/lwjgl/`, `runtime/gl_shim.zig`, `runtime/al_shim.zig` |
| **Android TouchCore APK + `build-lib -dynamic`** | APK (`0.98 MiB`, 109 D8 sınıf) imzalı ve hazır; `SHA-1` çoklu kütüphane + asset nesnesi indirme eklendi; 4-shard `--no-lwjgl` SDL3/GL33C/OpenAL/STB/EA `.so` Bionic `/system/bin/linker64` üzerinde çalıştırıldı (`BIONIC_EXIT=0`) | `android/touchcore/`, `releases/Monolith.apk`, `docs/STATUS.md` §20 |
| **GitHub Actions CI + Bulut Cross-Build** | `.github/workflows/ci.yml` (`rhi.zig`, `Arith`, APK build+zipalign, Android 4-shard `--no-lwjgl` smoke) ve `.github/workflows/build-so.yml` (`ANDROID_NDK_HOME`, `mono_android_entry`) hazır | `.github/workflows/ci.yml`, `.github/workflows/build-so.yml` |
| **PC Masaüstü `--no-lwjgl` (Pencere + Oyun)** | **Eksik** — X11/Wayland/Win32 pencere adaptörü yok (PC Linux tam LWJGL ile `x86_64`'te çalışır) | `runtime/java_nolwjgl/org/lwjgl/sdl/SDLVideo.java`, `glfw/GLFW.java` |
| **Android Üzerinde Tam Vanilla `26.3-client.jar` Oynanışı** | **Donanım/ortam sınırı bekliyor** — `61k` sınıf LLVM derlemesi cihazın ~2.6 GiB boş RAM'ine sığmadığı için `.so` bulutta ([`build-so.yml`](../.github/workflows/build-so.yml)) üretilmeli ve canlı telefon ekranında test edilmelidir | `docs/STATUS.md` §8, §20 |
| **RAM ve FPS'te JVM (HotSpot)'u Geçmek** | **Kısmen tamamlandı** — GC kaldırılıp `SlabPool` + `ArenaAllocator` + Escape Analysis eklendi; ancak kalıcı nesne başlığı (`16B`), UTF-16 `String`, kalıcı heap geri kazanımı ve Vanilla chunk veri bağlamaları hâlâ eksiktir | `runtime/rt.zig`, `docs/BENCHMARK.md` §2 |

---

## 2. Client'ı %100 Çalışır Hale Getirmek İçin Yapılanlar ve Kalan Eksikler

### A. PC (Linux / Windows) İçin Kalanlar
1. **Masaüstü `--no-lwjgl` Pencere / Context Köprüsü (`SDLVideo.java`, `GLFW.java`, `rhi_gl.zig`)**:
   * `SDL_CreateWindow()` ve `glfwCreateWindow()` şu an Android TouchCore `ANativeWindow` yüzeyine veya `MONO_HEADLESS_WINDOW=1` moduna bağlıdır; yoksa `0L` / `UnsupportedOperationException` döner.
   * PC'de `--no-lwjgl` pencere açmak için X11/Wayland/Win32 pencere + EGL/GLX/WGL yüzey bağlayıcısı eklenmelidir (tam LWJGL modu masaüstünde mevcuttur).
2. **Gerçek Fiziksel GPU Üzerinde Blaze3D Shader & GLSL Uyumluluk Testi**:
   * `gl_shim.zig` GLSL `#version 150`/`330 core` başlıklarını `#version 300 es`'e dönüştürür, `glVertexAttribDivisor`, `glFenceSync`, `glBlitFramebuffer`, `glDrawBuffers` ve `ARBDirectStateAccess` / `ARBVertexAttribBinding` adaptörlerini taşır; ancak fiziksel Adreno/Mali GLES3 ve Vulkan üzerinde gerçek `26.3-client.jar` shader'ları henüz canlı ekranda çizdirilmemiştir.
3. **Kalan Üçüncü Parti / JDK Native Çağrıları (`docs/STATUS.md` §2–§3, §20)**:
   * `26.3-client.jar` analizinde `org/lwjgl/*` link hataları **0**'a indirilmiştir; ayrıca `MethodHandle.invokeBasic` çok-biçimli çağrı indirmesi ve `LevelRenderer.resetLevelRenderData()V` yaması eklenmiştir.
   * Bunun yanında `91` adet LWJGL-dışı link hatası (çoğu isteğe bağlı `log4j` Kafka/JMS/Mail/Jackson/Disruptor eklentileri ve `NarratorMac`, artı `9` dahili `MethodHandle`/`Lookup` imzası), `103` JDK native ve `257` üçüncü parti native (`netty` epoll/kqueue/unix, `jna`, `jtracy`, `text2speech`) çalışma zamanında çağrılmaları hâlinde karşılık beklemektedir.

### B. Android (`TouchCore` APK) İçin Yapılanlar ve Kalanlar
1. **Gerçek `26.3-client.jar` → `libminecraft_mono.so` Bulut Derlemesi (`.github/workflows/build-so.yml`)**:
   * Vanilla `26.3` kapanışı (`61,280` sınıf, `278,867` metot) LLVM ile ~`7–15 GiB RAM` gerektirir (`docs/STATUS.md` §8) ve `monolith/__main__.py` içindeki `--backend native` şu an `x86_64/AMD64` ile sınırlıdır.
   * Bu telefon ortamında (7.2 GiB toplam / ~2.6 GiB boş RAM) tam `26.3-client.jar` `.so` derlemesi fiziksel olarak sığmaz; ancak **0 LWJGL link hatası**, **`MethodHandle.invokeBasic`**, **`LevelRenderer.resetLevelRenderData` yaması**, **`SDL3` + `GLES3` + `OpenAL` + `STB`** ve **`ci.yml` / `build-so.yml` NDK + `--skip-assets` düzeltmeleri** hazırdır. Kullanıcı GitHub Actions `build-so.yml` iş akışıyla `libminecraft_mono.so`'yu yüksek RAM'li sunucuda derleyebilir.
2. **Canlı Cihaz Ekranı (`SurfaceFlinger` + `GameActivity`) Doğrulaması**:
   * `BionicSdlCheck` (`17,438 KiB` 4-shard `.so`) gerçek Android `/system/bin/linker64` üzerinde `EA_ARENA_OK`, `SDL_NO_SURFACE_GUARD_OK`, `GL_SHIM_OK`, `OPENAL_SHIM_OK`, `STB_PNG_OK`, `FREETYPE_FALLBACK_OK`, `BIONIC_EXIT=0` ile doğrulanmıştır.
   * [`releases/Monolith.apk`](../releases/Monolith.apk) (`0.98 MiB`, 109 D8 sınıf) derlenip `zipalign` + `apksigner` v2/v3 ile doğrulanmıştır.
   * **Kalan adım:** Bulutta (`build-so.yml`) derlenen gerçek `26.3` `libminecraft_mono.so` dosyasının telefona kurulan TouchCore APK içinden yüklenip gerçek `SurfaceFlinger` ekranında test edilmesi ve `MojangDownloader` HTTPS indirmesinin gerçek cihaz ağında denenmesidir.

---

## 3. Normal JVM Minecraft'ı RAM ve FPS'de Geçmek İçin Yapılanlar ve Kalanlar

`docs/BENCHMARK.md` §2'deki tarihsel tam oyun ölçümünde **Vanilla HotSpot `2255–2297 MiB` RSS** kullanırken, eski GC'li MONOLITH **`6603–7803 MiB` RSS** ve **`49 ms` vs `7.6 ms` server tick** gösteriyordu. Güncel durum:

1. **Tamamlananlar**:
   * STW GC tamamen kaldırıldı; yerine 3-epoch `SlabPool` + iş parçacığı başına `ArenaAllocator` (`mem_pool.zig`, `rt.zig`) getirildi.
   * **Yeni: Derleme Zamanı Escape Analysis + Otomatik `ArenaScope` (`monolith/zigen.py`, `runtime/rt.zig`)** — döngüsüz DAG metodlarında kaçmadığı ispatlanan `new`, `newarray`, `anewarray` nesneleri otomatik olarak `rt.arenaMark()` / `rt.arenaRestore()` korumasında `newObjectArena`/`newArrayArena` ile sıfır-malloc tahsis edilir.
2. **Hâlâ Eksik Olanlar (JVM'i Tam Geçmek İçin)**:
   * **Kaçan Kalıcı Nesnelerin Geri Kazanımı**: `SlabPool` sabit kalıcı tahsisleri otomatik toplamaz (yalnızca `ArenaScope`/`endFrame` ve açık `retireObject`); uzun oyun oturumları için eşzamanlı/RC veya bölge bazlı kalıcı geri kazanım gerekir (`GcStress`'te `WeakReference`'ın `weak cleared=false` kalmasının sebebi budur).
   * **16 Byte Nesne Başlığı & 64-Bit Referanslar**: Compressed OOPs (4B `Ref`) henüz yok (`rt.Object` = 16B).
   * **Compact Strings (`Latin-1`)**: `classlib/src/java/lang/String.java` hâlâ UTF-16 (`2 byte/karakter`) kullanır.
   * **Vanilla Chunk/Mesh + `WorkerPool` + `world_render.zig` Entegrasyonu**: `patches/manifest.json` artık `LevelRenderer.resetLevelRenderData()V` dahil Vanilla hedefleriyle eşleşmektedir, ancak Vanilla `PalettedContainer`/VBO belleğini tamamen Zig off-heap bloklarına ve `gpu_instancing.zig` batch yoluna taşıyan derin yamalar henüz yazılmamıştır.
   * **Shard-Arası İnlining / ThinLTO / Structured Loops**: 32-shard C-ABI sınırındaki çağrı yükü (`CallBench`) ve `RssFile` (~0.9 GiB) kod ayak izi iyileştirilmelidir.

---

## 4. Eski `.md` Dosyaları Neden Kafa Karıştırıyor?

* [`docs/STATUS.md`](STATUS.md): Kronolojik eklemelerle büyüdüğü için güncel referans **§18–§20**'dir; önceki bölümler eski kilometre taşlarını kaydeder.
* [`docs/HARDWARE.md`](HARDWARE.md): Küçük RAM'li cihazlarda nelerin derlenemeyeceğini açıklar.
* [`README.md`](../README.md), [`docs/ARCHITECTURE.md`](ARCHITECTURE.md) ve [`docs/INDEX.md`](INDEX.md) içindeki bazı erken dönem ifadeleri eski sürümlerden kalmadır.
