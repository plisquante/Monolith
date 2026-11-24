# Java kalıntısı (residue)

Çalışan programın ne kadarının hâlâ çevrilmiş Java bytecode'u olduğu. `python3 -m monolith.residue` ile ölçüldü
(Minecraft 26.3, `--no-lwjgl`, masaüstü derlemesi); başlangıç değeri `residue-baseline.json` dosyasında.

| Köken | Java gövdesi olan metotlar | Bytecode |
|---|---:|---:|
| Minecraft (istemci arşivi, 9 824 sınıf) | 88 385 | 3 812 KiB (%44) |
| JDK (java.base 2 175 KiB, java.xml 419, java.net.http 118, ...) | 139 023 | 2 914 KiB (%33) |
| Kütüphaneler (icu4j 479, fastutil 405, log4j-core 208, guava 118, oshi 98, netty ~125, joml 64, ...) | 49 760 | 1 921 KiB (%22) |
| Bizim Java kaplamamız (102 sınıf) | 1 339 | 51 KiB (%0,6) |
| **Toplam** | **278 507** (91 066 derlenmiş içinden) | **8 699 KiB** |

En büyük oyun paketleri: world/level 732 KiB, world/entity 470, util/datafix 333, client/gui 274, client/renderer 247,
world/item 179.

Çalışan program JVM ve bytecode içermez: bu metotların hepsi zaten bytecode'dan üretilmiş Rust'tır. "Kalıntı", yazılmış değil üretilmiş Rust demektir. Bir metot, bir Rust yaması onu değiştirdiğinde (`patches/rust/manifest.json`) ve orijinali artık çağrılmadığında kalıntıdan çıkar.
