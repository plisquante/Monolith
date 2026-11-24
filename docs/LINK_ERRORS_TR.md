# Link error'lar: ne oldukları, neden bug olmadıkları, nasıl korunuyoruz

Ölçüm: Minecraft 26.3 + 66 kütüphane, JDK 25, `monolith.rustgen --no-lwjgl`, `MONO_VERBOSE=1` (2026-11-24).
Sonuç: 61 747 sınıf, 292 896 metot, 0 çevrilemeyen, 0 eksik native, **446 link error, hepsi açıklandı**.

## Link error nedir

Bir çağrı noktası, alan erişimi ya da catch maddesi, sınıf yolunda olmayan bir sınıfa ya da üyeye başvuruyor.
Üretilen kod orada çalışma zamanında `NoSuchMethodError`, `NoSuchFieldError` ya da `NoClassDefFoundError` fırlatır, ve yalnızca
o nokta çalışırsa. JVM de aynı yerde aynı hatayı verir (tembel bağlama). Yani link error bir çeviri hatası değil, "bu yol bu
kurulumda çalışmaz" bilgisidir.

## Dağılım (446)

| Grup | Adet | Neden | Karar |
|---|---|---|---|
| `log4j-core` | 293 | log4j-core'un opsiyonel appender/layout'ları: JMS, SMTP (`javax.mail`), ZeroMQ, Kafka, Disruptor, Jackson, commons-csv, OSGi. Kütüphaneleri Minecraft ile gelmiyor | beklenen; JVM'de de çalışmaz |
| `netty-log4j` | 84 | netty'nin opsiyonel log4j 1.x günlükçü bağdaştırıcısı (`org.apache.log4j.Logger/Level` yok) | beklenen |
| `commons-compress` | 37 | zstd (`com.github.luben`) ve xz (`org.tukaani`) codec'leri | beklenen |
| `lambdaform` | 21 | JDK'nın `LambdaForm`/`ClassSpecializer`/`BoundMethodHandle` düzeni. monolith'te `MethodHandle` bir nesne ağacıdır (`classlib/.../MethodHandle.java`), LambdaForm yok | tasarım gereği |
| `macos-objc` | 11 | `MacosUtil.disableCloseWindowMenuItem` ve `NarratorMac`: `ca.weblite.objc` köprüsü yalnızca macOS'ta indirilir | Minecraft'ın macOS'a özgü sınıfları; macOS desteklenmiyor, bu sınıflar çalıştırılmaz |

## macOS

macOS hedef platform olmaktan çıkarıldı (proje kararı). Oyun jar'ındaki macOS'a özgü sınıflar yine çevrilir ve
bu listede kalır; çalıştırılmazlar. macOS pencere, ses, yama ve CI kodu bu daldan kaldırıldı.

## Koruma

`tools/check_link_errors.py <transpile.err>` her link error satırını yukarıdaki gruplardan birine eşler; eşlenmeyen bir satır
çıkış kodu 1 ile build kapısını bozar. Girdi:

    MONO_VERBOSE=1 python3 -m monolith.rustgen ... 2> transpile.err
    python3 tools/check_link_errors.py transpile.err

Bu yüzden yeni bir link error ("`net/minecraft/...` içinde olmayan bir üye") ya açıklanır (gruba eklenir) ya düzeltilir.

## Eski sayı

`docs/STATUS.md` §2 "137 link error ... JDK 21'in java.base'i" yazıyor; o ölçüm farklı bir kütüphane ve JDK durumundandı. Bugünkü
446'nın her satırı yukarıdaki gruplara düşüyor, yani yeni bir bug sınıfı yok. Neden 137'den 446'ya çıktığı (farklı kütüphane kümesi,
JDK sürümü ya da çeviri kapsamı) eski ölçümün ayrıntısı olmadığı için doğrulanmadı.
