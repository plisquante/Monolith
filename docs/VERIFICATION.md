# Headless oyun doğrulaması (Linux, 26.3 istemcisi)

Kapsam: Rust derlemesinin (`tools/mcbuild_rust.py`) betikli bir koşusunun bu makinede gösterdiği sonuçlar. Bu, gerçek pencereden geçen bir duman testidir, test paketi değildir. Girdi xdotool ile verilir; basit bir tıklama ya da tuş basımı güvenilir değildi, bu yüzden fare düğmeleri ve tuşlar en az 0,3 sn basılı tutuldu.

Kurulum: GLX'li Xvfb `:99` 1280x720x24, `MC_GRAPHICS=opengl`, `LIBGL_ALWAYS_SOFTWARE=1`, derleme dizininde `./run.sh`. Seçenekler: `onboardAccessibility:false`, `narrator:0`, `skipMultiplayerWarning:true`.

| Adım | Sonuç |
|---|---|
| Başlık ekranına açılış | geçti (4 vCPU, yazılım GL ile yaklaşık 70 sn) |
| Tek oyunculu, dünya listesi | geçti: kaydedilmiş dünya listelendi |
| Kaydedilmiş dünyayı yükleme | geçti: "Loading terrain" ardından dünya çizildi |
| İleri yürüme (W 3 sn basılı) | geçti: eğitim "Move with W, A, S and D" adımından "Look around" adımına ilerledi |
| Duraklatma menüsü (Escape) | geçti: "Save and Quit to Title" içeren Oyun Menüsü |
| Kaydet ve başlığa dön | geçti: başlık ekranına dönüldü, çökme yok |
| Kaydedilmiş dünyayı yeniden yükleme | geçti: kayıttan önceki ile aynı görünüm ve HUD |

Günlük: koşu günlüğünde üç hata satırı var, hepsi ağla ilgili: Mojang kullanıcı özelliklerinin alınması ve Realms (`Couldn't connect to realms`; sandbox proxy sertifikası oyunun güven deposunda olmadığı için bir PKIX hatası). Hiçbir istisna ya da panic kaydedilmedi.

Eşzamanlı GC (`MONO_GC_CONCURRENT=1`), ilk açılış: başlık ekranına ulaşıldı, süreç 120 sn'de hâlâ çalışıyordu; günlükte ClassCastException, panic ya da ölümcül satır yok. Normal koşuyla aynı üç yalnızca ağ hatası satırı.

Eşzamanlı GC, ikinci açılış, aynı betik: kaydedilmiş dünya aynı görünümle yüklendi, yürüme eğitimi "Look around" adımına ilerletti, kaydet ve çık başlık ekranına döndü. Çökme, panic ya da ölümcül satır yok; aynı üç yalnızca ağ hatası.
İki eşzamanlı mod koşusunun ikisi de geçti; varsayılan olması için 60 dakikalık sızıntı soak'ı hâlâ gerekli.

## Android yerel çekirdeği (derleme ve bağlama, cihaz yok)

| Adım | Sonuç |
|---|---|
| `android/touchcore/native` içinde `cargo test --offline` | geçti: 28 host testinden 28'i |
| `ANDROID_NDK_HOME` = NDK 26.3.11579264 ile `tools/build_touchcore_apk.py` | geçti: `aarch64-linux-android` (release) için Rust çekirdeği, C köprüsü, APK imzalı, 5.58 MiB |
| APK içindeki `libtouchcore_mono.so` | AArch64 paylaşımlı nesne; `tc_init`, `tc_tick_frame`, `tc_shutdown` dışa aktarır; yalnızca `libandroid`, `liblog`, `libc`, `libm`, `libdl` gerektirir |

Kapsam dışı: motorun bir cihazda ya da öykünücüde çalıştırılması, GPU arka uçları (`rhi.rs`, Vulkan ve GL 3.3 için `UnsupportedBackend` döndürür), APK içindeki Minecraft motor kütüphanesi (`libminecraft_mono.so`).

## Sızıntı kapısı, 60 dakika hareketsiz (toplayıcı açık, normal mod)

Koşu: `tools/leak_soak.sh /home/user/mc_build --minutes 60` (kaydedilmiş dünya, hareket yok), ardından `tools/leak_budget.py soak-prof.json --warmup-s 1800 --budget-mb-per-min 1`.

| Ölçüm | Sonuç |
|---|---|
| 60 dakika sonra oyun hâlâ çalışıyor, çökme raporu yok | evet |
| Toplamalar, toplam / 1800 sn ısınmadan sonra | 111 / 42 |
| İlk toplamadan sonra canlı / son | 7,9 MiB / 593,5 MiB |
| Isınmadan sonra canlı büyüme (en küçük kareler) | -0,054 MiB/dk, bütçe 1,00, bütçe içinde, araç çıkışı 0 |
| Duraklama, en uzun / ortalama | 1473 ms / 1045 ms |

Bu neyi gösteriyor: ısınmadan sonra canlı yığın düz, yani bu kapı yazıldığı gibi geçiyor. Neyi göstermiyor: yığın koşu boyunca 7,9'dan 593,5 MiB'a büyüdü; ısınma penceresi bunu gizliyor ve duraklamalar yaklaşık 1 sn, `GC_PLAN.md`'deki < 1 ms hedefinin çok üstünde. Koşu izleyici (tracing) toplayıcıyı kullandı, toplayıcı kapalı modu kullanmadı; bu yüzden toplayıcının kaldırılması hakkında henüz bir şey söylemiyor. Yalnızca bir koşu.

## Toplayıcı kapalı anahtarı (`MONO_GC=off`), yalnızca test programları

`tests/run_tests.py GcStress Oop Threads`, `MONO_GC=off` ile geçiyor.

`MONO_GC=off` ile oyun koşusu (yeniden derlenmiş ikili, kaydedilmiş dünya, hareketsiz): bu 16 GB'lık makinede yaklaşık 10 dakika sonra süreç 13,3 GB residente ulaştı. Bellek yetersizliği ile öldürülmemesi için koşu orada durduruldu; bu yüzden 60 dakikalık kapı tamamlanmadı. Toplayıcı kapalıyken yığın sınırlı değil; toplayıcı şimdilik derlemede kalıyor.

13 GB nereden geliyor (o koşunun tahsis profili, `soak-prof.json`): 624 sn'de 11,85 GB tahsis, yaklaşık 19 MB/sn, 226 M nesne. Hiçbir sınıf baskın değil: `char[]` %10,4, `Object[]` %5,8, `byte[]` %4,6, `long[]` %4,4, `Vec3` %3,0, `ArrayList$Itr` %3,0, `BlockPos` %2,8, `ImmutableCollections$ListItr` %2,5, `DataResult$Success` %2,5, `Optional` %2,3, `RegularEnumSet$EnumSetIterator` %2,3. Bunlar kısa ömürlü geçici nesneler. Toplayıcı açık koşuda ısınmadan sonra canlı küme düzdü; yani büyüme toplanmamış çöp, sızıntı değil. Boşta tahsis oranı, `GC_PLAN.md` kademe 1-3'ün hedefidir.

## Oyun derlemesinde kaçış analizi ve satır içi (ölçüldü, etkin değil)

Derleme: `tools/mcbuild_rust.py ... --escape --inline` (32 shard, 2 iş). Tamamlandı. Derleme 525 link hatası yazdı (bayraksız temel derleme: 446). Ayrıntılı döküm (`MONO_VERBOSE=1`) 525 hatayı şöyle ayırıyor: 374 `NoSuchMethodError`, 90 `NoClassDefFoundError`, 48 `NoSuchFieldError`, 13 catch-tipi hatası. Temel koşu (446, ayrıntılı), 525'in kesin bir alt kümesi: temel hatalardan hiçbiri kaçış derlemesinde eksik değil; fazla 79 hata aynı başarısızlıkların yeni çağrı noktaları. Bu 79'un 55'i log4j core (SMTP, JMS, asenkron Disruptor, JSON, Jackson), 12'si commons-compress, 6'sı netty günlüğü, ve 6'sı JDK method-handle iç yapısı (`java/lang/invoke`: `InvokerBytecodeGenerator`, `SimpleMethodHandle`, `NativeMethodHandle`, `ClassSpecializer`). Kontrol edilmesi gereken 6 JDK girdisi: bunlar isteğe bağlı bağımlılık değil ve gerçek bir yolda çalışıp çalışmadığı henüz bilinmiyor. Varsayılan derleme bu bayrakları kullanmaz.

Boşta tahsis, 10 dakika hareketsiz, aynı kaydedilmiş dünya, toplayıcı açık:

| Derleme | Tahsis | Oran | Temele göre en büyük değişiklikler |
|---|---|---|---|
| bayraksız (`docs/VERIFICATION.md` sızıntı bölümü, toplayıcı kapalı, 624 sn) | 11,85 GB | 19,0 MB/sn | referans |
| `--escape --inline` ile (721 sn) | 19,37 GB | 26,8 MB/sn | `ImmutableCollections$ListItr` 294'ten 939 MB'a, `int[]` 1,6 GB yeni, `Vec3` 355'ten 753 MB'a |

Yukarıdaki oranlar arena baytlarını da sayar. Arena baytları metot dönüşünde serbest bırakılır ve asla toplayıcıya ulaşmaz; bu yüzden toplama baskısı için önemli olan, yalnızca heap oranıdır: temelde 17,0 MB/sn (11 851 MB eksi 1 255 MB arena, 624 sn), bayraklarla 24,0 MB/sn (19 368 MB eksi 2 069 MB arena, 721 sn). Bu hâlâ yaklaşık %40 fazla; bu yüzden bayraklar etkin değil, yalnızca ölçüm için isteğe bağlı kalıyor. İlk GC kaldırma adımı bu değişiklik değil.

Kapsam dışı: çok oyunculu, Realms, ses çıkışı, ikinci bir oturum, uzun oyun, tekrarlanan açılışlar, FPS ve bellek rakamları.
