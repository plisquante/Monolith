<p align="center"><img src="logo.png" alt="Monolith" width="720"></p>

# MONOLITH

Monolith, Java bytecode'unu **JVM olmadan** çalışan yerel bir programa çevirir. Bir uygulamanın ve kullandığı kütüphanelerin sınıf dosyalarını okur, hangi kodun erişilebilir olduğunu analiz eder, bu kodu Rust'a çevirir ve bir çalıştırılabilir dosya üretir. Hiçbir şey tersine derlenmez ve elle yazılmaz; çeviri doğrudan bytecode'dan yapılır.

İlk hedef Minecraft Java Edition 26.3 istemcisidir.

## Durum (dürüst)

| Alan | Durum |
|---|---|
| Çeviri (bytecode'dan Rust'a), sınıf ve metot kapsamı | çalışıyor: istemci kapanışında 6 196 sınıf, 0 çevrilemeyen |
| Linux x86_64 masaüstü oyunu (menü, dünya, girdi, ses) | düz modda çalışıyor; headless testler geçiyor. Headless koşu (Xvfb, yazılım GL, 26.3 istemcisi): başlık ekranı, menü, dünya oluşturma ve yükleme, yürüme, kaydetme ve çıkış, komut dosyalı bir koşuda yeniden yükleme başarılı, bkz. [`docs/VERIFICATION.md`](VERIFICATION.md). Çok oyunculu, Realms ve uzun oyun doğrulanmadı |
| Bilinen link boşlukları | Çeviri sırasında 446 link hatası (CI günlüğü, 26.3 istemcisi). Hepsi oyunun açılış yolunda kullanmadığı isteğe bağlı bağımlılık yollarında: log4j appender'ları (ZeroMQ, JMS, mail, Jackson, CSV, zstd), OSGi ve netty günlüğü ve Minecraft'ın yalnızca macOS'a özgü kodunun `ca.weblite` köprüsü. Bunlara çalışma zamanında çağrı yapılırsa hata fırlatırlar. macOS desteklenen bir hedef değil. |
| Çöp toplayıcı | durdur-dünya (stop-the-world) işaretle ve süpür (500 MB canlı veride duraklama yaklaşık 1 sn'ye kadar çıkabilir); eşzamanlı mod (`MONO_GC_CONCURRENT=1`, varsayılan kapalı) SATB düzeltmelerinden sonra 120 sn'lik Linux açılışını geçiyor: bariyersiz referans üzerine yazmaları (Unsafe put/CAS, VarHandle store/CAS, `recycle`) artık eski değeri günlüğe alıyor. `MONO_GC_POISON=1` serbest bırakılan yuvaları `0xDD` ile doldurarak serbest bırakıldıktan sonra erişimi yakalar |
| Ses | Linux ALSA (`libasound`, çalışma zamanında yüklenir), Windows WASAPI; aygıt yoksa sessiz yedek |
| Windows | pencere (Win32+WGL), girdi ve ses arka uçları yazıldı; CI arka uç duman testini ve tam oyun açılışını çalıştırıyor (`.github/workflows/game.yml`). Gerçek donanımda henüz test edilmedi |
| Android | launcher APK'sı var; yerel motor bir cihazda test edilmedi |
| Java sınıf kütüphanesi kaplamaları ve JNA/Netty öykünmesi | hâlâ kullanımda; değiştiriliyor. Bugün: `runtime/java_nolwjgl` içinde 243, `classlib` içinde 56 Java dosyası ([`docs/WS7_ENVANTER_TR.md`](WS7_ENVANTER_TR.md)), 259 JNI/JNA öykünme native'i ([`docs/JNI_ENVANTER_TR.md`](JNI_ENVANTER_TR.md)). `MONO_COUNT_NATIVES=1` çalışma zamanında çağrılarını sayar (gerçek bir oyun ölçümü henüz yok) |
| Sürekli entegrasyon (CI) | bu deponun GitHub Actions koşuları, hiçbir adım çalışmadan 3-4 saniyede hatayla bitiyor; bu, 2026-11-24 öncesi commit'ler için de geçerli (iş günlüğü yok). İş var ama günlüğü yok; bu, iş akışı dosyalarından çok hesap ya da çalıştırıcı (kota, faturalama, çalıştırıcı başlatma) sorununa işaret ediyor, ama bu doğrulanmadı. Düzeltilene kadar hiçbir CI sonucuna güvenilmemeli. `tools/release_check.py` kapıları yerelde çalıştırır |

Proje bitmiş bir ürün değil. Bir sürümden önce neyin geçmesi gerektiği için [`docs/RELEASE.md`](RELEASE.md), kararlar ve ölçümler için [`docs/IMPL_SPEC.md`](IMPL_SPEC.md) dosyasını okuyun.

## Hızlı başlangıç (Linux x86_64)

Gereksinimler: Python 3.10 veya üstü, bir JDK 25 (`MONO_JDK_HOME` ona işaret etmeli), rustc (stable), `jq` gerekmez, üç shard'lık derleme için yaklaşık 12 GB RAM ve derleme için yaklaşık bir saat.

    python3 tools/fetch_client_jar.py --version 26.3 --out client.jar     # Mojang manifestinden, SHA-1 doğrulamalı
    python3 tools/mcbuild_rust.py client.jar -o build/minecraft-26.3
    build/minecraft-26.3/run.sh      (Windows: build\minecraft-26.3\run.bat)

CI oyun işinin aynı adımlarını kendi makinenizde (POSIX kabuklarda), adım başına bir günlükle çalıştırmak için:

    sh tools/ci_local.sh --out build/ci-local        # indirme, derleme, 120 sn açılış, Linux'ta eşzamanlı GC açılışı

İlk açılışta bir dünya gerekir: Singleplayer'da bir tane oluşturun. Derleme seçenekleri: `--opt`, `--shards`, `--shard-jobs`, `--escape` ve `--inline` (isteğe bağlı; bu bayraklar bir ölçülmüş koşuda boşta tahsisi artırdı, bkz. `docs/VERIFICATION.md`). Az belleği olan bir makinede `--shard-jobs 1` kullanın. 4 çekirdekli bir makinede `--shards 4 --shard-jobs 4`, küçük bir test programını tek shard'a göre yaklaşık 3,1 kat hızlı derledi (ölçüldü, bkz. `docs/BUILD_PERF.md`); Minecraft varsayılanları henüz ölçülmedi.

## Testler

    rustc --edition 2021 --test tests/rust/audio_main.rs -o /tmp/audio_tests && /tmp/audio_tests
    rustc --edition 2021 --test tests/rust/crash_main.rs -o /tmp/crash_tests && /tmp/crash_tests    # çökme raporlayıcı
    python3 -m monolith.crc_tests          # ayrıca netty_tests, recycle_tests, stack_tests, patch_tests, patch_fp_tests (MONO_JDK_HOME ayarlı olmalı)
    python3 -m pyflakes monolith/*.py tools/*.py tests/*.py
    python3 tests/run_tests.py [Ad ...]    # tests/java içindeki fark testleri (HotSpot ile çevrilmiş programı karşılaştırır)
    python3 tools/release_check.py         # docs/RELEASE.md kapılarının tümü tek koşuda

Fark testleri, çevrilmiş programın çıktısını HotSpot ile karşılaştırır. Bir çevirinin link hataları, `tools/check_link_errors.py` ile açıklanmış gruplara karşı denetlenir (`docs/LINK_ERRORS_TR.md`); yamalanmış her metodun bytecode'u bir parmak izi ile sabitlenir (`tools/patch_fingerprints.py`, `patches/README.md`).

Bir çökme (Rust panic'i ya da Linux'ta SIGSEGV/SIGBUS/SIGILL/SIGFPE/SIGABRT) çalışma dizinine ya da `MONO_CRASH_DIR` içine `mono-crash-<pid>.txt` yazar; iş parçacığını, mesajı ya da hata adresini ve yerel bir geri izlemeyi içerir.

## Depo yapısı

- `monolith/` — çevirici (`rustgen.py`, `inline.py`, `escape.py`, `patches.py`, ...) ve Rust çalışma zamanı (`monolith/rt/`).
- `patches/rust/` — seçili metotlar için Rust yerine koyma kodları (DataFixer, tickChunk, bit depolama, dünya yüksekliği).
- `classlib/`, `runtime/` — sınıf kütüphanesinin parçaları için Java kaplamaları (değiştiriliyor).
- `tools/` — derleme, paketleme (`make_release.py`), ölçüm (`alloc_report.py`, `leak_soak.sh`, `leak_budget.py`).
- `docs/` — planlar, şartname, ölçümler, sürüm süreci.

## Android (TouchCore launcher)

`android/touchcore/`, dokunmatik HUD ve demo modu olan yerel bir `arm64-v8a` launcher APK'sıdır (`com.monolith.touchcore`). Motor kütüphanesini (`libminecraft_mono.so`) yükler; bu kütüphane `python3 tools/mcbuild_rust.py --android <client.jar>` ile üretilir (Android NDK ve `rustup target add aarch64-linux-android` gerekir). APK, `python3 tools/build_touchcore_apk.py` ile derlenir; bu derleme NDK 26.3 ile geçiyor ve yerel çekirdeğin host testleri geçiyor (`docs/VERIFICATION.md`). Motor henüz bir cihazda çalıştırılmadı.

## Launcher'lar

| Platform | Başlatma | Notlar |
|---|---|---|
| Linux | `./launch.sh` | python3 >= 3.10 gerekir |
| Windows | derleme dizinindeki `run.bat` | arka uç yazıldı, yalnızca CI ile doğrulandı |

Masaüstü launcher'ı (`launcher/launcher.py`, yalnızca standart kütüphane) tarayıcıda yerel bir sayfa açar; derlemenin yazdığı `run.sh`'ı başlatır. CLI: `./launch.sh --cli --play --username Steve`.

## Lisans notu

Bu depo yalnızca çeviriciyi, onun çalışma zamanını ve sınıf kütüphanesinin özgün kaplamalarını içerir. Minecraft ikili dosyalarını ya da onlardan türetilmiş kodu içermez ve bunları dağıtmak için kullanılmamalıdır (Minecraft EULA). Derleme çıktıları yerel dizinlerde kalır; `.gitignore` `*.jar`, `build/` ve `out/` dosyalarını dışarıda tutar.
