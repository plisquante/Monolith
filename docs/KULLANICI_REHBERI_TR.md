# Kullanıcı rehberi (Linux x86_64)

Bu rehber yalnızca bu depoda doğrulanan davranışı anlatır. Durum tablosu için `README.md`, sürüm kapıları için `docs/RELEASE.md`.
Proje henüz bitmiş bir ürün değildir; bilinen sınırlar aşağıda.

## 1. Gereksinimler

- Python 3.10 veya yenisi, JDK 25 (`MONO_JDK_HOME`), `rustc` (stable).
- Derleme için yaklaşık 12 GB bellek ve yaklaşık bir saat. Az bellekte `--shards 16 --shard-jobs 1` kullan: 4 shard ile tek bir
  `rustc` işlemi 12,7 GB'a çıkıp bellek yetersizliğinden öldürüldü; 16 shard ile ilk 7 shard bellek sorunu olmadan derlendi
  (16 GB'lık makinede, 2026-11-24). Tam 16 shard'lık derlemenin sonu bu makinede henüz alınmadı.
- Çalıştırmak için ekran (X11) ve OpenGL; ekransız makinede `xvfb-run` ve `LIBGL_ALWAYS_SOFTWARE=1`.

## 2. Kurulum

    python3 tools/fetch_client_jar.py --version 26.3 --out client.jar        # Mojang'ın manifestinden, SHA-1 doğrulanır
    python3 tools/mcbuild_rust.py client.jar -o build/minecraft-26.3 --shards 16 --shard-jobs 1
    build/minecraft-26.3/run.sh

İlk açılışta bir dünya yoktur: Tek oyunculu'da yeni dünya yarat. Masaüstü başlatıcı için `./launch.sh`
(`./launch.sh --cli --play --username Steve`).

## 3. Ayarlar (ortam değişkenleri)

Hepsi çalışma zamanı kodunda okunur (`monolith/rt/`); belirtilmeyen varsayılandır.

| Değişken | Etkisi |
|---|---|
| `MONO_WINDOW_SIZE=1280x720` | ilk pencere boyutu |
| `MONO_AUDIO_DEVICE=<ad>` | ses aygıtı (varsayılan `default`) |
| `MONO_AUDIO_SILENT=1` | sesi kapat (sessiz çıkış) |
| `MONO_AUDIO_TRACE=1` | tanınmayan ses parametrelerini yaz |
| `MONO_GC_MB=<n>` | çöp toplama eşiği, MB (en az 1) |
| `MONO_GC_CONCURRENT=1` | eşzamanlı toplayıcı (varsayılan kapalı) |
| `MONO_GC=off` | toplayıcıyı kapat. **Önerilmez**: 10 dk sonra 13,3 GB'a çıkan ölçüm var (`docs/RELEASE.md` §4) |
| `MONO_GC_PHASES=1` | toplama evrelerinin sürelerini yaz |
| `MONO_GC_POISON=1` | boşaltılan yuvaları `0xDD` ile doldur (bellek hatası avı) |
| `MONO_ARENA=0`, `MONO_RECYCLE=0` | arena tahsisini / yineleyici geri dönüşümünü kapat |
| `MONO_ARENA_CHECK=abort` | arenadan kaçan nesneyi bulunca durdur (`1`: yalnız bildir) |
| `MONO_ALLOC_PROFILE=<dosya>` | tahsis profilini yaz (`tools/profile_sites.py` okur) |
| `MONO_PATCH_VERIFY=1` | her yamayı orijinal metotla karşılaştır (yavaş) |
| `MONO_CRASH_DIR=<dizin>` | çökme raporunun yazılacağı dizin (varsayılan: çalışma dizini) |
| `MONO_TRACE_EX=1` | her istisnada yığın izi (`err`: `ClassNotFoundException` hariç) |
| `MONO_TRACE_INIT=1`, `MONO_TRACE_PARK=1` | sınıf başlatma ve bloklanma izleri |
| `MONO_JNA_TRACE=1` | JNA kaydı izleri |
| `MONO_COUNT_NATIVES=1` (ya da `=<dosya>`) | çıkışta JNI/JNA taklit native'lerinin çağrı tablosunu stderr'e (ya da dosyaya) yaz; `_exit`, çökme ve öldürme ile bitişte yazılmaz |
| `MONO_JAVA_HOME=<dizin>` | `java.home` (conf/ ve lib/tzdb.dat burada) |
| `-Dad=değer` | yürütülebilire verilen argümanlar sistem özelliği olur |

## 4. Sorun giderme

**Oyun çöktü.** Çalışma dizininde (ya da `MONO_CRASH_DIR`'de) `mono-crash-<pid>.txt` yazılır: tür (`panic` ya da sinyal adı),
iş parçacığı, mesaj ya da hata adresi, yerel yığın izi. Bu dosya bir hata bildirimine eklenecek asıl kanıttır.

**`native method not implemented: ...` ile duruyor (çıkış kodu 70).** Çevrilen kod, çalışma zamanında karşılığı olmayan bir
yerel metoda ulaştı. Bilerek sessizce geçilmez; metodun adı mesajda yazar. Uygulama JNI kütüphaneleri bu çalışma zamanında
desteklenmez (hedef: çalışma zamanında JNI yok).

**`NoClassDefFoundError` / `NoSuchMethodError` bazı yollarda.** Çeviri sırasında 446 "link error" bildirilir; hepsi oyunun
kullanmadığı isteğe bağlı yollardadır (log4j eklentileri, macOS köprüsü, JDK'nın LambdaForm düzeni). Açıklaması:
`docs/LINK_ERRORS_TR.md`.

**Ses yok.** Linux'ta `libasound` çalışma zamanında yüklenir; yoksa sessiz çıkışa düşer. Zorla sessiz: `MONO_AUDIO_SILENT=1`.
Aygıt seçmek için `MONO_AUDIO_DEVICE`.

**Bellek büyüyor.** Durma duraklaması 500 MB canlı bellekte yaklaşık 1 sn'ye çıkabilir (`README.md`). Eşiği `MONO_GC_MB` ile
ayarla; ölçüm için `MONO_GC_PHASES=1` ve `MONO_ALLOC_PROFILE`.

**Ekransız sunucuda.** `LIBGL_ALWAYS_SOFTWARE=1 xvfb-run -a -s "-screen 0 1280x720x24" build/minecraft-26.3/run.sh`
(yazılım GL; yavaştır).

## 5. Bilinen sınırlar

- Windows, macOS ve Android'de gerçek donanımda test yok (`README.md`).
- Çevrimiçi sunucuya katılma, Realms ve uzun oyun doğrulanmadı; TLS (WS5) tamamlanmadı.
- Java sınıf kütüphanesi kaplamaları ve JNA/Netty taklidi hâlâ kullanılıyor (`docs/JNI_ENVANTER_TR.md`, `docs/WS7_ENVANTER_TR.md`).
- Minecraft içeriği bu depoda ve paketlerde dağıtılmaz; yasal karar (WS13) verilmemiştir, sürümler özel test sürümüdür.
