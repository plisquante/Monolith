# JNI/JNA taklit yüzeyi: envanter (WS2)

Kaynak: `monolith/rt/natives_jni.rs` (1225 satır, 259 native fonksiyon). Sayım: `grep -oE 'pub fn n_[A-Za-z0-9_]+__'`.
Amaç: MASTER_PLAN §8 "çalışma zamanında JVM, JNI, JNA, libffi yok". Her grup için karar ve riski burada yazılır; kaldırma sonrası
çalışma ancak tam build + `headless_play.sh` ile doğrulanır (testler en sonda).

| Grup | Fonksiyon | Kim kullanıyor | Karar | Risk |
|---|---|---|---|---|
| `io.netty.channel.unix.Socket` | 48 | netty yerel (epoll) taşıma | kaldır: epoll'u kullanılamaz yap; netty'nin NIO'ya düştüğü varsayımı henüz doğrulanmadı (Minecraft `Connection` seçimi okunacak) | NIO yolu (sun.nio.ch, `natives_nio.rs`) ağ yükü altında doğrulanmadı |
| `io.netty.channel.epoll.LinuxSocket` | 43 | aynı | aynı | aynı |
| `io.netty.channel.epoll.Native` | 18 | aynı | aynı | aynı |
| `io.netty.channel.epoll.NativeStaticallyReferencedJniMethods` | 10 | aynı | aynı | aynı |
| `io.netty.channel.kqueue.*` | 31 | macOS taşıması | kaldır: Linux/Windows'ta hiç yüklenmez | yok (macOS'ta NIO) |
| `io.netty.channel.unix.{Errors,Limits}StaticallyReferencedJniMethods, FileDescriptor, Buffer` | 25 | epoll taşıması | epoll ile birlikte | aynı |
| `com.sun.jna.Native` (+ `read`, `write`) | 56 | oshi, narrator, Minecraft yerel çağrıları (JNA) | WS2: oshi'yi yama ile kaldır, narrator'ı kaldır ya da Rust'a taşı; kalan çağrı sayacı raporla ölçülür | oshi sistem bilgisi çağrıları; kaldırılırsa donanım bilgisi ekranı boş olur |
| `com.mojang.jtracy.TracyBindings` | 21 | profil aracı (Tracy) | **normal oyunda çağrılmaz**: `Main` yalnızca komut satırında profil seçeneği verilirse `TracyBootstrap.setup()` çağırıyor (bayt kodu doğrulandı); `TracyClient` her çağrıda `loaded` bayrağını kontrol ediyor. Rust tarafı yalnız o seçenek için. Şimdi silmek `natives missing` sayacını 0'dan 21'e çıkarır (build bozulmaz, `rt::missing_native` yüksek sesle hata verir); sınıf seviyesi yama (WS6) gelince boş uygulamayla silinir | düşük; çalışma zamanı sayacına girmez |
| `com.mojang.text2speech.NarratorLinux$FliteLibrary` | 7 | anlatıcı (erişilebilirlik) | WS2 kararı: Rust FFI ile flite ya da özellik olarak kapalı (açıkça belgeli) | erişilebilirlik özelliği kaybı |

## Sıra (küçükten büyüğe)

1. Tracy (21): WS6 sınıf seviyesi yama bekliyor (yukarıdaki not); çalışma zamanı sayacını etkilemez.
2. kqueue (31): Linux/Windows build'lerinde hiç yüklenmiyor; macOS ayrı karar.
3. Flite (7): kapat ya da taşı.
4. JNA (56): çağıranları tek tek çıkar (gerçek çağrıları sayan araç henüz yok, aşağıdaki "Ölçü" bölümüne bak).
5. Netty epoll + unix (~150): en büyük grup; NIO'ya düşme yolu doğrulanınca topluca silinir.

## Ölçü

Hedef: 30 dk oynanışta JNI/JNA giriş sayısı 0 (`MASTER_PLAN` WS2 çıkışı). `MASTER_PLAN`'ın söylediği `MONO_TRACE_NATIVES`
runtime'da yoktu; yerine `MONO_COUNT_NATIVES=1` (ya da `=<dosya>`) yazıldı (`monolith/rt/ncount.rs`): `natives_jni.rs`
içindeki her native'in stub'ı çağrıyı sayar, süreç çıkarken tablo yazılır (en çok çağrılan başta). Kapalıyken çağrı başına
tek `Relaxed` yükleme. Doğrulama: `com.sun.jna.Native` taklidini çağıran küçük bir programda 7 çağrı / 3 native doğru
sayıldı; birim testi `tests/rust/ncount_main.rs`.

### İlk gerçek ölçüm (2026-11-24, Linux x86_64, Xvfb, yazılım GL, 26.3, açılış ekranında 75 sn)

130 çağrı, 18 farklı native. Hepsi iki gruptan:

| Grup | Çağrı | Ne |
|---|---|---|
| `com.sun.jna.Native` | 118 | `getShort` 26, `registerMethod` 14, `_getPointer` 13, `getLong` 13, `malloc` 13, `setMemory` 13, `findSymbol` 10, `open` 10, `sizeof` 6, `invokeInt` 2, `getNativeVersion`, `initIDs` |
| `NarratorLinux$FliteLibrary` (anlatıcı) | 12 | `flite_init`, `register_cmu_us_kal16`, `flite_synth_text`, `utt_wave`, `play_wave`, `delete_utterance` |

Netty (epoll/kqueue/unix) ve Tracy çağrısı **yok** (açılışta ağ ve profil aracı kullanılmıyor). Yani ilk 75 sn'de kaldırma hedefi:
JNA'yı anlatıcı (Flite) çağıran taraf kullanıyor; anlatıcı kapatılırsa ya da Rust'a alınırsa bu ölçümdeki çağrıların tamamı gider.
### İkinci ölçüm: açılıştan dünyaya, yürüyüşe ve kapanışa (aynı gün, son build)

`tools/headless_play.sh --seconds 60 --clean-exit`, boş oyun dizini: ana menü, yeni dünya (Creative), 60 sn yürüyüş, `SIGTERM`
ile kapanış (kancalar çalıştı, dünya kaydedildi, çıkış kodu 143). **139 çağrı, aynı 18 native**: JNA 129, Flite 10. Dünya
yüklemek ve yürümek ek JNI/JNA çağrısı getirmedi (ilk ölçümden fark: `registerMethod` 14 → 21, Flite 12 → 10). Netty ve Tracy yine yok.

Sonuç: Linux masaüstünde tek oyunculu oturumda JNI/JNA girişlerinin **tamamı anlatıcıdan (Flite, JNA üzerinden) ve JNA'nın
kendi kaydından** geliyor. Anlatıcıyı çıkarmak ya da Rust'a almak bu ölçümde sayacı 0'a indirir (hedef WS2). Henüz ölçülmeyen:
çok oyunculu (netty), uzun oynanış, Windows/macOS/Android.
