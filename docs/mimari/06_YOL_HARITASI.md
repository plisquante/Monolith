# 06. Yol haritası: iş paketleri, bağımlılıklar, kabul, riskler

Gün sınırı yok (sahip kararı). Sıra: bağımlılık ve değer. Her paket küçükten büyüğe, her paket kapısıyla biter. Test/derleme koşuları her paketin sonunda, paket içinde önce kod.

## 1. İş paketleri

| id | paket | bağımlı | kabul (kapı) | boyut |
|---|---|---|---|---|
| **C0** | `StackMapTable` ayrıştırma + tür doğrulayıcı uyumu | | belge 01 §4 | S |
| **C1** | MIR kurucu (SSA, istisna kenarları, etki etiketleri), `mir_tests.py` | C0 | T4, `mir_fallback` raporu | L |
| **C2** | Rust yayıcı MIR için (`emit.py`), çift yol | C1 | T1 MIR yolunda yeşil | L |
| **C3** | MIR yolu tüm kapanışta; eski yol geri düşüşü | C2 | `mir_fallback == 0` hedef, T1 yeşil | M |
| **C4** | MIR satır içi + devirt + sabit katlama + ölü kod | C3 | T1, tahsis ve kare süresi ölçümü | M |
| **C5** | KKA (kısmi kaçış) | C4 | T5 | L |
| **C6** | ölçek ABI (özelleştirme `f$s`) | C5 | G1 (≤ 12 MiB/sn) | L |
| **C7** | bölge ve tahsis türü alanı (`alloc_kind`) | C5 | T6 | M |
| **C8** | shard yakınlığı + doğrudan çapraz-shard çağrı | C3 | Q11 + kare süresi | M |
| **C9** | eski yolu sil, `gen_insn` kaldır | C3 + tüm Q yeşil | kod silindi | S |
| **R1** | `mono_abi` / `mono_rt` ayrımı, `ABI_VERSION` anahtarı | | Q11 (çalışma zamanı değişikliği < 5 dk) | M |
| **R2** | PC tablosu ve çerçeve işaretçisi ile iz; gölge yığın bayrakla | C3 | `StackWalk`, `Exceptions`, çökme raporu | L |
| **R3** | kilit hızlı yolu (thread-local önbellek) + kilit silme (V/F) | C5 | `MonitorChurn`, ana iş parçacığı kilit sayısı | S |
| **M1** | `alloc_by_tier` sayacı, KKA doğrulama modu | C5 | T5 | S |
| **M2** | elle R: çarpışma hattı ve çizim komutları bölgeleri | R1 | G2 (≤ 5 MiB/sn) | M |
| **M3** | bölge doğrulama + (araştırma) otomatik bölge çıkarımı | M2, C7 | T6 | M, araştırma |
| **M4** | sahipli O: palet/chunk sınıf yaması | L2 (sınıf yaması), R1 | G3 | L |
| **M5** | görüntü yığını (derleme zamanı başlatma) | C1 | menü süresi, `MONO_IMAGE_VERIFY` | L |
| **M6** | GC `lazy` modu, kapat kapısı | M2, M4, C6 | G-S, Q7 | S |
| **L1** | `lib_inventory.py` (K1-K4 sınıflama) | | sınıflanmamış 0 | S |
| **L2** | sınıf düzeyi yama şeması + araç (`patches/classes`) | | `patch_fp_tests` sınıf desteği | M |
| **L3** | `gen_shells.py` (K1 üretim) + LWJGL Java dosyalarının silinmesi | L1 | Q10 (LWJGL), Q4 | M |
| **L4** | K2 taşıması: `classlib` mantığı Rust'a ya da JDK orijinaline | L2 | Q10 (classlib) | L |
| **N1** | anlatıcı native'leri sıfır | L1 | `Taklit` düşer | S |
| **N2** | Tracy kapanıştan çıkar | C4 (sabit katlama) | | S |
| **N3** | JNA doğrudan Rust | L2 | | M |
| **N4** | netty epoll/kqueue koşullu erişilebilirlik | C4 | Q9 | M |
| **N5** | kalan native'ler | N1-N4 | `Taklit == 0` | M |
| **P1** | Windows donanım doğrulama + paket | | Q12 | M |
| **P2** | macOS donanım doğrulama + paket | | Q12 | M |
| **P3** | Android cihaz doğrulama + APK | | Q12 | M |
| **P4** | Linux arm64 | R1 | Q12 | S |
| **V1** | `frame_time.py` + HotSpot karşılaştırması (T11, T12) | | Q8 ölçüleri | M |
| **V2** | `release_check.py` Q kapılarını toplar | V1 | tek komut | S |
| **D1** | kullanıcı belgeleri ve sürüm süreci güncellemesi | V2 | `RELEASE.md` | S |

## 2. Kritik yol

`C0 → C1 → C2 → C3 → C4 → C5 → C6 → G1` (bellek kazancının çekirdeği) paralel `R1 → M2 → G2`; `L2 → M4 → G3`; sonra `M6 → G-S`.
Bağımsız küçükler (değer ve risk düşük, hemen): L1, N1, N2, M1, V1, R3, P4. Başlangıç sırası: **L1, N1, V1, R1, C0**.

## 3. Aşama planı

| aşama | içerik | çıktı |
|---|---|---|
| A (temel) | L1, N1, V1, R1, C0 | envanter, ilk native sıfırlama, kare süresi ölçüsü, ABI ayrımı, tür bilgisi |
| B (derleyici) | C1-C3 | MIR yolu eşdeğer; hiçbir davranış değişmez |
| C (kazanç) | C4-C8, R2, R3, M1 | satır içi, KKA, ölçek ABI; tahsis düşer (G1) |
| D (bellek) | M2-M5 | G2, G3 |
| E (kütüphane) | L2-L4, N3-N5 | Java mantığı 0, JNI 0 |
| F (kapanış) | M6, C9, V2, P1-P3, D1 | G-S, sürüm kapıları |

Aşamalar kısmen paralel yürür (D ve E bağımsız).

## 4. Riskler

| id | risk | etki | azaltma |
|---|---|---|---|
| R1 | MIR geçişi çok uzun sürer / `mir_fallback` sıfıra inmez | C-zinciri durur | çift yol (P8): eski yol çalışır kalır; fallback sebepleri sayılır ve en sık sebepten kapatılır |
| R2 | KKA yetersiz: kaçış nedenlerinin çoğu sanal çağrı ve genel koleksiyon | G1 kaçırılır | M2 (elle R) ve M4 (O) bağımsızdır; eşik aşılırsa D9 yeniden değerlendirilir (belge 02 §9) |
| R3 | PC tablosu satır doğruluğu HotSpot izlerinden sapar | iz testleri | eski yol bayrakla kalır; kapı: `StackWalk`, `Exceptions` |
| R4 | derleme belleği/süresi büyür (daha çok analiz, doğrudan çapraz-shard çağrı) | CI/yerel geliştirme | Q11 kapısı; shard önbelleği; yakınlık dengesi |
| R5 | GitHub Actions hesap/runner tarafı bozuk | kapılar CI'da koşmuyor | yerel kapılar `release_check.py`; sahip kararı |
| R6 | Minecraft güncellemesi yama parmak izlerini kırar | derleme durur | parmak izi kapısı yüksek sesle durur; yama yenileme iş akışı (belge 04 §5) |
| R7 | görüntü yığını düzeni x64/arm64 farkı | Android | düzen derleme zamanı hedef bilgisinden; `MONO_IMAGE_VERIFY` |
| R8 | donanım doğrulaması (Windows/macOS/Android) bu ortamdan yapılamaz | M3/M4 sürümü | sahip makinesi ya da CI koşucusu gerekir (AÇIK) |
| R9 | yanlış kaçış kanıtı sessiz davranış değişikliği | kritik | T5, `MONO_VERIFY_KKA`, şüphede S (P3) |

## 5. Açık kararlar (sahip ya da ölçüm bekliyor)

| id | soru | ne zaman |
|---|---|---|
| O1 | başlık 16 → 8 bayt (D25) | V, R, O sonrası ölçüm |
| O2 | eşzamanlı GC yedeği (D9) | G2/G3 ölçümünde S ≥ 5 MiB/sn kalırsa |
| O3 | otomatik bölge çıkarımı açılsın mı (4.2) | T6 sağlamlık süiti sonucu |
| O4 | `android/touchcore` Java başlatıcı Java kalsın mı (K4) | Android aşaması |
| O5 | ikili boyut bütçesi | V1 ölçümü |
| O6 | CI sağlığı (R5) | sahip |
| O7 | donanım doğrulama kanalı (R8) | sahip |

## 6. Ürün tanımı (1.0 çıkışı)

1.0: Q1-Q12 hepsi yeşil Linux x86_64'te; Windows, macOS ve Android Q12 geçmiş; E1-E7 ölçüleri belge 00 §1 tablosundaki hedefte ya da AÇIK karar sahibinin imzasıyla kabul edilmiş.

## 7. İlerleme (güncellenir)

Puan kuralı: bitti=1, sürüyor=0,5, başladı=0,25. "Kod bitti, derlenmedi" en fazla sürüyor sayılır (derlenip testi geçmeden bitti denmez).
Kapı sayısı: yalnızca mevcut build için kanıtlanmış Q kapıları sayılır.

| paket | durum | kanıt |
|---|---|---|
| L1 | bitti | `tools/lib_inventory.py`: 300 dosya, K1=68, K2=232 (`docs/KUTUPHANE_ENVANTER.json`) |
| C0 | bitti | `monolith/stackmap.py`, `stackmap_tests.py`: java.util 3654 metot, 9750 çerçeve |
| C1 | bitti | `monolith/mir.py`: java.base 13 059 sınıf, 111 407 metot, 0 hata; `mir_tests.py` ALL PASSED (yorumlayıcı 50 satır JVM ile aynı) |
| C2 | sürüyor | `MONO_MIR=1` ile Collections1 derlenip JVM çıktısıyla aynı; fark süiti MIR yolunda koşulmadı |
| C4 | başladı | `monolith/mir_opt.py` `inline_leaf`: Collections1'de 1682 çağrı satır içi; devirt yalnızca RTA'nın örneklediği sınıflar (`d3df2e6`) |
| N1 | sürüyor | `patches/rust/narrator.rs` + `narrator_tests.py` ALL PASSED (HotSpot ile aynı anlatıcı, JNA kaydı yok); oyunda ölçülmedi |
| N2 | sürüyor | `patches/rust/tracy.rs` (33 yama): kod yazıldı, derlenip koşulmadı |
| N3 | sürüyor | `patches/rust/sysinfo.rs` (oshi yerine /proc) + Linux dışı anlatıcı EMPTY: kod yazıldı, derlenmedi. Analiz: JNA native 56 → 0 |
| N4 | sürüyor | `patches/rust/netty.rs` (`remote()` her zaman NIO) + epoll kanalları yansıma listesinden çıktı: kod yazıldı, oyunda denenmedi. Analiz: netty native 173 → 0 |

Analiz (derleme değil, kapanış sayımı): native 976 → 726. Bu, Claude tarafının ölçtüğü tek kapanış sayısı.
Son derleme: `--escape` Minecraft derlemesi tamamlandı (`mcesc/minecraft-26.3/run.sh`); bu derleme yamalardan önce, MIR kapalı.
Varsayılan yolda fark süiti: koşuyor; şu ana kadar 16 PASS, 0 FAIL, 3 SKIP (ClientCoreE2E, HttpFetch, Jni: tasarım gereği).

Q kapıları (mevcut build için): 0/12 kanıtlı. Q3 fark süiti bitmedi; Q5 oyun ve Q6 sızıntı mevcut build ile koşulmadı.
