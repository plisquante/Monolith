# Pipeline ve yol haritası (basit Türkçe)

Bu belge iki şeyi anlatır: (1) Monolith'in Java'dan çalışan oyuna giden yolu, yani "pipeline", (2) bundan sonra yapılacaklar, yani yol haritası.
Ayrıntılı ve bağlayıcı belgeler: [`mimari/00_ANA_MIMARI.md`](mimari/00_ANA_MIMARI.md), [`mimari/06_YOL_HARITASI.md`](mimari/06_YOL_HARITASI.md), [`GC_PLAN.md`](GC_PLAN.md).

## 1. Pipeline: Java'dan çalışan oyuna

Fikir basit: Minecraft Java bytecode'u olarak gelir. Biz onu önceden Rust'a çeviririz, Rust'ı derleriz ve ortaya JVM gerektirmeyen tek bir program çıkar.

| Adım | Ne olur | Nerede |
|---|---|---|
| 1. Jar'ı al | Minecraft istemci jar'ı indirilir (`client.jar`). | `tools/fetch_client_jar.py` |
| 2. Sınıfları oku | Jar'daki ve kütüphanelerdeki `.class` dosyaları okunur (yaklaşık 60 bin sınıf, 286 bin metot). | `monolith/classfile`, `monolith/world.py` |
| 3. Ulaşılabilirlik analizi (RTA) | Hangi metotların gerçekten çağrılabileceği bulunur. Çağrılamayanlar atılır, böylece program küçülür. | `monolith/world.py` |
| 4. Rust üret | Her metot Rust koduna çevrilir. Kod 16 parçaya (shard) bölünür. | `monolith/rustgen.py` |
| 5. Yamaları uygula | El yazması hızlı Rust kodları, çevrilmiş metotların yerine konur (şu an 22 yama). | `patches/rust/manifest.json`, `monolith/patches.py` |
| 6. Rust'ı derle | Çalışma zamanı kütüphanesi (`monolith/rt`) ve 16 shard `rustc` ile derlenir. Aynı kaynak daha önce derlendiyse önbellekten alınır. | `tools/mcbuild_rust.py` |
| 7. Paketle | Çalıştırılabilir program, kaynak dosyalar ve `run.sh` çıktı klasörüne konur. | `build/minecraft-26.3/` |
| 8. Oyunu dene | Ekransız (Xvfb) ortamda oyun açılır, dünya oluşturulur, yürünür, temiz kapatılır. | `tools/headless_play.sh` |

Komutlar:

- Tam derleme: `./build.sh build/client.jar -o build/minecraft-26.3 --opt 1 --shards 16 --shard-jobs 2 --reuse-objs`
- Oyun testi: `MC_GRAPHICS=opengl LIBGL_ALWAYS_SOFTWARE=1 xvfb-run -a -s "-screen 0 1280x720x24" sh tools/headless_play.sh build/minecraft-26.3 --seconds 60 --clean-exit`
- Link hatası kapısı: `MONO_VERBOSE=1` ile derle, hata çıktısını `python3 tools/check_link_errors.py <dosya>` ile denetle.

Bilmen gerekenler:

- `--reuse-objs`: Üretilen Rust kaynağı birebir aynıysa eski derleme çıktısı yeniden kullanılır. Derleme çok hızlanır (ilk derleme yaklaşık 95 dk, önbellekli derleme çok daha kısa).
- Bellek: Her `rustc` yaklaşık 5 GB ister. 16 GB'lık makinede `--shard-jobs 2` güvenlidir. Derleme ile test paketi aynı anda çalıştırılmaz.
- Link hatası: Bir çağrı, sınıf yolunda olmayan bir sınıfa gidiyorsa "link hatası" sayılır. Şu an 444 tane var ve hepsi açıklanmış (log4j eklentileri, netty log4j, commons-compress, LambdaForm, macOS sınıfları). Yeni bir hata çıkarsa kapı kırmızı olur.
- Ortam: Oyunun GLSL derleyebilmesi için sistemde `libshaderc1` ve `libspirv-cross-c-shared0` kurulu olmalı. Yoksa oyun "Couldn't parse GLSL" ile açılışta çöker.

## 2. Pipeline'ın 25 Kasım 2026 durumu

| Kapı | Sonuç |
|---|---|
| Tam Minecraft 26.3 derlemesi | Geçti (5 koşu, hepsi çıkış 0) |
| Oyun testi (60 sn, temiz kapanış, dünya kaydı) | Geçti |
| Link hatası kapısı | Geçti: 444 hata, açıklanmamış 0 |
| Tracy yamaları (33 yama) | Çıkarıldı: oyunu açılışta çökertiyordu. Yeniden eklemek için `patches/rust/tracy.rs` başındaki nota bak. |
| Sızıntı soak'ı (60 dk) | Yapılmadı |
| Windows, Android, gerçek GPU | Bu ortamda denenemez |

## 3. Yol haritası

Sıra: önce güvenilirlik, sonra bellek, sonra platformlar.

| Sıra | İş | Neden | Bittiğini nasıl anlarız |
|---|---|---|---|
| 1 | Test edilmemiş yamaları oyunda doğrula (sysinfo, narrator, netty) | Açılış testinden geçtiler ama ayrıntılı denenmediler. | Her grup tek tek kapatılıp açılarak davranış karşılaştırılır. |
| 2 | Tracy yamalarını doğru kurup geri getir | Tasarım eksikti: `RTA` bazı sınıfları ve metotları atıyordu. Düzeltme: `news` bağlaması ve `init_X()?` çağrıları. | Oyun açılır, Tracy çağrıları çalışır. |
| 3 | 60 dakikalık sızıntı soak'ı | Uzun oturumda belleğin sabit kaldığı kanıtlanmadı. | Büyüme 1 MiB/dk altında. |
| 4 | GC planı faz 0-3 (`GC_PLAN.md`): ölçüm araçları, yazma bariyeri, eşzamanlı işaretleyici | Şu anki GC durdur-ve-süpür; oyunda takılma yapar. | Duraklama 1 ms altında, bariyer maliyeti %3 altında. |
| 5 | GC planı faz 4-5: çöp azaltma ve sahipli bellek (palet/chunk) | GC'yi kapatabilmek için çöp üretimini düşürmek gerekir. | Kararlı durum tahsisi 5 MiB/sn altında. |
| 6 | GC planı faz 6: `MONO_GC=off` soak'ı | İlk deneme 10 dakikada 13,3 GB'a çıktı. Büyüme hâlâ çok yüksek. | Kalan büyüme 1 MiB/dk altında. Sonra GC varsayılan kapalı, güvenlik ağı olarak kalır. |
| 7 | Derleyici kazançları (MIR, satır içi, kaçış analizi; `06_YOL_HARITASI.md` C1-C9) | Hız ve bellek: tahsis oranını düşürür. | `mir_fallback == 0`, tahsis oranı hedefte. |
| 8 | Kütüphane temizliği (JNA, JNI, LWJGL Java dosyalarını silme) | Java mantığını sıfıra indirir. | Envanterde sınıflanmamış kalem 0. |
| 9 | Platformlar: Android `.so` ve APK, Windows | Gerçek cihaz testi gerekir. | Her biri sızıntı betiğini çalıştırır. |
| 10 | C dosyalarının yorum çevirisi | Yorumların tamamı Türkçe olsun. | Taramada İngilizce yorum yok. |

Önemli nokta: GC kaldırma 4, 5 ve 6. adımların birleşik adıdır. Doğrudan kapatılamaz, çünkü önce çöp üretimi ve sahipli bellek hazır olmalı.

## 4. Bu belgeyi güncel tutmak

- Bir kapı geçildiğinde 2. bölümü, bir iş bittiğinde 3. bölümü güncelle.
- Ayrıntılı kronoloji `STATUS.md` içindedir, buraya taşınmaz.
