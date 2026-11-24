# Yerel (native) metotlar: envanter ve plan (B1)

`tools/native_report.py <gen-dir> --md <dosya>` ile, Minecraft 26.3 istemcisinin üretilmiş shard'larından (`--no-lwjgl`, 32 shard) üretildi.
Statik envanter: üretilen koddaki çağrı noktaları; çalışma zamanı sayıları değil.

| kategori | yerel metot | çağrı noktası | rt içinde gerçekleştirilmiş | eksik |
|---|---:|---:|---:|---:|
| JNA (`com.sun.jna.Native`) | 56 | 56 | 56 | 0 |
| netty/JNI (epoll, unix, kqueue taşıyıcısı) | 174 | 174 | 174 | 0 |
| JDK (`java/`, `jdk/`, `sun/`) | 556 | 556 | 556 | 0 |
| grafik/ses (`org/lwjgl/`, `mono/rhi/`) | 162 | 162 | 162 | 0 |
| oyun/diğer | 28 | 28 | 28 | 0 |
| **toplam** | **976** | | **976** | **0** |

## Bunun anlamı

* Programın çağırdığı her yerel metot Rust ile gerçekleştirilmiş. Hiçbiri eksik değil; üretilen program, gerçekleştirilmemiş herhangi biri için yüksek sesle durur.
* "Rust yerel" demek "JNI/JNA yok" demek değildir: JNA ve Netty yerel metotları, değiştirilmemiş Java sınıflarının (JNA, Netty) çalışması için bir JNI/libffi kütüphanesini *öykünen* Rust kodudur. Bunları kaldırmak, Rust fonksiyonlarını değil, Java çağıranlarını kaldırmak demektir.
* **Netty (174):** Netty kullanılabilir olduğunda epoll/kqueue seçer. `io.netty.transport.noNative=true` ayarlanırsa NIO taşıyıcısını kullanır (düz Java, yerel kod yok). Çalışma zamanı artık bu varsayılanı ayarlıyor (`monolith/rt/rt.rs`, `user_props`). Kontrol: `monolith/netty_tests.py` (HotSpot ve Rust, `Epoll.isAvailable()` konusunda aynı sonucu verir). Bu, epoll/kqueue yerel metotlarını sıcak yoldan çıkarır; 174 fonksiyon, onlara referans veren üretilmiş kod kalmayana kadar ağaçta kalır (bir sonraki yeniden üretimde).
* **JNA (56):** hepsi `com.sun.jna.Native` içinde. Çağıranlar oshi (sistem bilgisi), anlatıcı ve benzerleri. Plan: bu çağıranlara doğrudan yanıt veren bir Rust yaması (oshi için sistem özellikleri) ya da isteğe bağlı özelliklerin kaldırılması. Yapılacak: çağıranları bulmak.
* **JDK (556) ve grafik/ses (162):** çalışma zamanının kendi yerel yüzeyi; Rust olarak kalırlar. Kalan Java kısmı, Java kaplamalarıdır (`classlib/`, `runtime/java_nolwjgl/`) (WS7).

## Çalışma zamanı sayımı (henüz yapılmadı)

`MONO_TRACE_NATIVES=1` (her yerel çağrının çalışma zamanı sayısı), üretilen kodda her yerel çağrı noktası için bir sayaç gerektirir. Bu bir kod üreteci değişikliğidir; bu yüzden bir sonraki tam yeniden üretimde, diğer üreteç değişiklikleriyle birlikte yapılacak.
