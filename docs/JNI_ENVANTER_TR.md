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

Hedef: 30 dk oynanışta JNI/JNA giriş sayısı 0 (`MASTER_PLAN` WS2 çıkışı). `MASTER_PLAN` bunu `MONO_TRACE_NATIVES` ile
ölçmeyi söylüyor, ama runtime'da böyle bir değişken yok; yalnız `MONO_JNA_TRACE` (JNA çağrılarını yazar) var. Sayaç henüz
yazılmadı ve envanter statik; çalışma zamanı verisi alınmadı.
