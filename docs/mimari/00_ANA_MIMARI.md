# Monolith hedef mimarisi: ana belge

Durum: **bağlayıcı mimari belge** (2026-11-24). Bu belge ve `docs/mimari/01..06` birlikte tek geçerli mimaridir. Eski belgelerle
çelişen yerde bu belge geçerlidir (bölüm 8 hangi bölümlerin geçersiz kaldığını sayar). Her iddia şu etiketlerden birini taşır:

| etiket | anlam |
|---|---|
| **KOD** | koddan doğrulandı (dosya adı yazılı) |
| **ÖLÇÜ** | araçla ölçüldü (tarih ve araç yazılı) |
| **KARAR** | bu belgeyle kesinleşti |
| **ÖNERİ** | tasarım, kanıtlanması gereken varsayım var (hangisi yazılı) |
| **AÇIK** | karar verilmedi, ölçüm ya da sahip kararı bekliyor |

## 1. Amaç: "Rust ruhu" ne demek

Monolith bugün Java bayt kodunu **Java'nın nesne modelini Rust'ta taklit eden** bir programa çevirir: her nesne yığında bir başlıklı blok,
her referans ham işaretçi, her dönüş değeri `Result`, bellek tek bir izleyici çöp toplayıcıyla (GC) yönetilir. Hedef, aynı davranışı veren
ama **Rust gibi çalışan** bir program: küçük değerler yığında ya da yazmaçta, geçici veri bölge (arena) ömrüyle, büyük veri tek sahipli ve
`Drop` ile serbest, çalışma zamanında karar yok, çöp toplama oynanışta çalışmaz.

Ölçülebilir son durum (ürün tanımı, 1.0 için):

| # | Ölçü | Bugün | Son durum |
|---|---|---|---|
| E1 | Oynanışta GC toplaması (boş olmayan dünya, 60 dk) | 100 toplama, ortalama duraklama 1,1 sn, en uzun 1,8 sn (ÖLÇÜ 2026-11-24, `leak_budget.py`) | **0** (GC yalnızca yükleme ekranında ve acil durumda) |
| E2 | Yönetilen yığına (katman S) kararlı durum tahsisi | 30,7 MiB/sn (ÖLÇÜ, `alloc_report.py`) | **1 MiB/dk altı** (sızıntı bütçesi) |
| E3 | Elle yazılmış Java kaynağı (kaplamalar) | 243 + 56 dosya (KOD `runtime/java_nolwjgl`, `classlib/src`) | **0 satır mantık**; yalnızca üretilmiş bildirim kabukları |
| E4 | Çalışma zamanında JNI/JNA girişi | 139 çağrı / 18 native, açılıştan kapanışa (ÖLÇÜ, `MONO_COUNT_NATIVES`) | **0** |
| E5 | HotSpot ile davranış eşitliği | fark süiti 27/27 (3 atlandı) | tüm fark testleri + oyun betiği yeşil |
| E6 | Kare süresi p99 ve RSS | ÖLÇÜLMEDİ (HotSpot karşılaştırması yok) | HotSpot'tan kötü değil |
| E7 | Ana menüye kadar süre (masaüstü) | ~50 sn (headless, yazılım GL) | 15 sn altı |

E1 ve E2 bellek mimarisinin (belge 02) tek sınavıdır; E3 ve E4 belge 04 ve 03'ün; E5 ve E6 belge 05'in.

## 2. Bugünkü durum: mimari borç (hepsi koddan ya da ölçümden kanıtlı)

| # | Borç | Kanıt | Çözüldüğü belge |
|---|---|---|---|
| AB1 | **Ara temsil yok.** `MethodGen.gen_insn`, bayt kodunu doğrudan Rust metnine çeviriyor. Analizler (kaçış, satır içi) bayt kodu üzerinde, dönüşümler metin üstünde; değer tipi, bölge ve sahiplik dönüşümleri bu yüzden yazılamıyor | KOD `monolith/rustgen.py` (2 715 satır), `escape.py`, `inline.py` | 01 |
| AB2 | **Referans türü bilgisi çıkarılmıyor.** `StackMapTable` hiç ayrıştırılmıyor; derleyici yalnızca "tür türü" (`i32/i64/f32/f64/ref`) biliyor | KOD `grep StackMapTable monolith/*.py` boş; `rustgen.kind_of` | 01 |
| AB3 | **Sınıflar arası çağrı satır içine alınamıyor.** `shards > 1` iken doğrudan `r_` çağrısı yalnızca aynı sınıf içinde; başka sınıfa her çağrı `extern "C"` `m_*` + `CRes` dönüşümü. LLVM sınıf sınırını aşamaz | KOD `rustgen.py:1305-1314` (`rust_callable`: `cur_cn == dc.name`) | 01 |
| AB4 | **Shard dağıtımı sınıf adının hash'i**; çağıran ve çağrılan farklı shard'larda. Çalışma zamanının herhangi bir değişikliği tüm shard nesnelerini geçersiz kılıyor (`rt_hash`): ~70 dk | KOD `rustgen.py:2160`, `rt_hash` | 01, 03 |
| AB5 | **Her metot gölge yığın çerçevesi itiyor, her deyim `set_line` yazıyor** (thread-local `RefCell<Vec>`). Yalnızca yığın izleri için ödenen sürekli maliyet | KOD `rt.rs:1285-1322`, üretilen `Vec3.add` | 01 |
| AB6 | Her çağrıda `stack_check()` (thread-local okuma) | KOD `rt.rs:1198`, üretilen kod | 01 |
| AB7 | Statik erişimde ve `new`'de sınıf başlatma çağrısı (`init_X().res()?`) | KOD üretilen `Vec3.add` | 01, 02 (görüntü yığını) |
| AB8 | `invokeinterface`: sınıfın itable'ında ikili arama | KOD `rt.rs:503` `itab_find` | 01 |
| AB9 | `synchronized`: nesne adresine bağlı hash tablosu (64 parçalı kilit); başlıkta kilit alanı yok | KOD `sync.rs` `mon()` | 03 |
| AB10 | **Bellek: tek model.** Her nesne yığında; kaçış analizi yalnızca metot kapsamında, ve ölçülen çöpün %1,5'ini kaldırabiliyor | ÖLÇÜ 2026-11-24 `escape_hot.py`; AOT_ARCHITECTURE §4.4.1 ölçümü | 02 |
| AB11 | 299 elle yazılmış Java kaplama dosyası; 13 294 satır `classlib` | KOD | 04 |
| AB12 | 259 JNI/JNA taklit native'i (netty epoll/kqueue ~150, JNA 56, Tracy 21, anlatıcı 7, ...) | KOD `natives_jni.rs`, `docs/JNI_ENVANTER_TR.md` | 03, 04 |
| AB13 | Platform: yalnızca Linux/X11 gerçek oyunla doğrulandı; Windows, macOS, Android çalıştırılmadı | KOD, `docs/STATUS.md` | 03 |
| AB14 | Bellek planı üç belgede parçalı ve çelişkili: `GC_PLAN` (görünmez eşzamanlı GC), `AOT_ARCHITECTURE` (bölge+görüntü yığını), `IMPL_SPEC` D9 | KOD docs/ | bu belge (bölüm 6) |

## 3. Hedef mimari: genel görünüm

```
                    ┌────────────────────────── DERLEME ZAMANI (Python, kapalı dünya) ──────────────────────────┐
 client.jar + libs  │  Ön yüz            Orta yüz (MIR)                                        Arka yüz        │
 classlib kabukları ─►  classfile        MIR kurucu (StackMapTable ile tipli)                  Rust yayıcı      │
 JDK jmods          │  world/program    ► analizler: çağrı grafı, null, kaçış, kimlik        ► shard'lar      │
 patches/ (Rust)    │  indy/RTA            dönüşümler: devirt, satır içi, KKA (kısmi kaçış),   ► iz tablosu     │
                    │                      bölge, ölçek özelleştirme, koşullu erişilebilirlik ► görüntü yığını │
                    └────────────────────────────────────────────┬───────────────────────────────────────────┘
                                                                 │ shard'lar (yalnız mono_abi'ye bağlı)
        ┌────────────────────────── ÇALIŞMA ZAMANI (Rust) ──────┴────────────────────────────────┐
        │ mono_abi (kararlı, satır içi)   mono_rt (uygulama)        platform / grafik / ses / ağ    │
        │ nesne düzeni, tahsis hızlı yol  katmanlar V F R O I S     Platform trait, RHI, mixer,      │
        │ kontrol, iz, bölge API          iş parçacığı, kilit,      soket+TLS, dosya, zaman          │
        │                                 istisna, yansıma, sinyal  yerel metot kaydı (sınıflı)      │
        └───────────────────────────────────────────────────────────────────────────────────────────┘
                                   │
                         başlatıcı (TouchCore / launcher.py) ── paketleme ── CI/kapılar (belge 05)
```

Katman eşlemesi (`MASTER_PLAN_TR` L0-L6 → bu belge): L0 çevirici = ön yüz + MIR + arka yüz (belge 01); L1 çalışma zamanı = `mono_abi` + `mono_rt`
(belge 03); L2 platform = platform/grafik/ses/ağ (belge 03); L3 kaplamalar = üretilmiş kabuklar + Rust sınıf yamaları (belge 04);
L4 yamalar = metot ve sınıf yamaları (belge 04); L5 launcher, L6 dağıtım (belge 06).

## 4. Tasarım ilkeleri (ihlal edilirse tasarım yanlıştır)

| # | İlke |
|---|---|
| P1 | **Kapalı dünya.** Derleme anında bütün kod bilinir; yansıma, `ServiceLoader`, `MethodHandle` hedefleri açık listeyle verilir. Listede olmayan çalışma zamanında yüksek sesle başarısız olur |
| P2 | **Kararı derleme zamanında ver.** Çalışma zamanında yapılan her işin derleme zamanında yapılamama nedeni belgede yazılı olmalı |
| P3 | **Kanıtla ya da güvenli tarafa düş.** Nesne ömrü ya da kimlik kullanımı kanıtlanırsa V/F/R/O/I katmanı; kanıtlanmazsa S (yönetilen). Kanıt kuralı: şüphede S. Sessiz bir tahmin yok |
| P4 | **Davranış HotSpot ile aynı.** Her dönüşüm fark testi ve gerekiyorsa doğrulama moduyla gelir. Doğrulanmamış dönüşüm varsayılan olmaz |
| P5 | **Desteklenmeyen şey yüksek sesle başarısız olur.** Stub, sahte başarı, sessiz geçiş yok |
| P6 | **Ölçmeden iddia yok.** Her hedef bir araca, her araç bir kapıya bağlıdır (belge 05) |
| P7 | **Kararlı ABI.** Üretilen kod ile çalışma zamanı arasındaki arayüz küçük, sürümlü ve satır içi parçalar `mono_abi`'de; çalışma zamanı iç değişikliği shard'ları geçersiz kılmaz |
| P8 | **Her aşama çalışır halde kalır.** Eski ve yeni yol yan yana (metot başına geri düşüşle) ilerler; büyük patlama geçişi yok |
| P9 | **Rust'ta yazılabilen veri Rust'ta yazılır.** Sıcak ve büyük veri yapıları Java nesnesi olarak değil, sınıf yamasıyla Rust yapısı olarak sahiplenilir |

## 5. Bellek katmanları (ayrıntı belge 02)

| katman | ad | ne | ömür/serbest bırakma | kim karar verir |
|---|---|---|---|---|
| **V** | sanal | kısmi kaçış analiziyle ölçeklere ayrılmış nesne (hiç ayrılmaz) | derleme zamanı | derleyici (KOD'da kanıtlı) |
| **F** | çerçeve | yığın yuvasındaki nesne (adres gerekli, kaçmaz) | metot/yineleme sonu | derleyici |
| **R** | bölge | dinamik ömürlü arena (tick, kare, görev) | bölge sonunda toplu | elle Rust (borç denetleyicisi) ya da otomatik çıkarım |
| **O** | sahipli | Rust yapısı, tek sahip, `Drop` | sahibi gidince | sınıf yaması (elle) |
| **I** | ölümsüz | görüntü yığını, derleme zamanında başlatılmış veri | hiç | derleme zamanı başlatma |
| **S** | yönetilen | geri kalan, izleyici GC | toplama | çalışma zamanı (yedek) |

Son durum: oynanışta S'ye tahsis ≈ 0; S yedek olarak derlemede kalır ve yalnızca yükleme/menü/acil durumda çalışır.

## 6. Kararlar

### 6.1 Önceki kararlardan değişenler

| eski | yeni |
|---|---|
| `IMPL_SPEC` D9: eşzamanlı SATB işaretleme, tembel süpürme, bariyer | **İPTAL (KARAR).** Bariyer ve eşzamanlı işaretçi yazılmayacak. S katmanı mevcut durdur-işaretle-süpür toplayıcıdır; oynanışta çalışmaz. Gerekçe: belge 02 §9. Koşullu yedek: V+R+O yapıldıktan sonra S tahsisi oynanışta 5 MiB/sn üstünde kalırsa (AÇIK, ölçüm kapısı G-S) eşzamanlı toplayıcı yeniden değerlendirilir; tasarımı `GC_PLAN.md` §3'te saklı |
| `GC_PLAN` hedefi "GC görünmez ve her zaman açık" | **Hedef: oynanışta GC yok.** "Görünmez GC" yalnızca yedek plan |
| `MASTER_PLAN` WS7 "classlib ve java_nolwjgl içinde Java kaynağı sıfır" | **Kesinleşti:** hiç **elle yazılmış mantık** yok; üretilmiş bildirim kabukları (alanlar ve imzalar, mantık yok) kalabilir (belge 04) |

### 6.2 Yeni kararlar

| # | Karar | durum |
|---|---|---|
| D14 | Bellek mimarisi: katmanlar V, F, R, O, I, S; oynanışta S tahsisi hedefi ≈ 0; eşzamanlı GC iptal | KARAR |
| D15 | Bayt kodu → **MIR** (tipli, yapılandırılmış) → Rust. Tüm analiz ve dönüşümler MIR üzerinde. Geçiş metot başına çift yolla (belge 01 §9) | KARAR |
| D16 | `mono_abi` (kararlı, satır içi) ile `mono_rt` (uygulama) ayrılır; shard'lar yalnızca `mono_abi`'ye bağlıdır | KARAR |
| D17 | Sıfır maliyetli yığın izi: gölge yığın ve `set_line` kalkar, çerçeve işaretçisi + PC tablosu gelir (hata ayıklama için eski yol bayrakla kalır) | ÖNERİ (varsayım: DWARF + çerçeve işaretçisiyle HotSpot ile aynı iz çıkar; kapı: `StackWalk` ve `Exceptions` testleri) |
| D18 | Shard dağıtımı çağrı grafı yakınlığına göre; aynı shard içinde sınıflar arası doğrudan çağrı; shard'lar arası için ThinLTO ya da `#[inline]` ölçülür | ÖNERİ (varsayım: kare süresi ve derleme süresi ölçümü) |
| D19 | Değer tipi = **kısmi kaçış analizi (KKA) + ölçek ABI'si**; sınıf beyaz listesi değil. Kimlik semantiği yapı gereği korunur | KARAR |
| D20 | Bölge: önce elle Rust bölgeleri (borç denetleyicisi kanıtlar), otomatik çıkarım ayrı araştırma hattı ve ancak sağlamlık test paketiyle açılır | KARAR |
| D21 | Büyük uzun ömürlü veri (chunk bölümü, palet) sınıf yamasıyla Rust sahipli yapı olur | KARAR |
| D22 | Görüntü yığını (derleme zamanı başlatma) katman I'dir | ÖNERİ (varsayımlar: başlatma izleyicisiyle çıkarılabilen sınıf oranı, x64/arm64 düzen eşitliği; belge 02 §7) |
| D23 | Koşullu erişilebilirlik: yama ile sabit yapılan öngörü metotları (`Epoll.isAvailable() == false`) RTA içinde katlanır, ölü dal ve sınıflar kapanıştan çıkar | KARAR |
| D24 | Yerel metotlar sınıflı kayıt tablosundan (`Gerçek | Taklit | Hata ayıklama`); taklit sayısı kapıdır | KARAR |
| D25 | Başlık düzeni v1 (16 bayt) kalır; 8 baytlık v2 değerlendirmesi V ve R'den sonra, ölçümle | AÇIK |
| D26 | Gelişmiş: referans sayımı (Rc) S yerine kullanılmayacak (belge 02 §10) | KARAR |

## 7. Hedef ile bugünün farkı (bileşen bazında)

| bileşen | bugün | hedef | iş (belge 06) |
|---|---|---|---|
| ön yüz (`classfile`, `world`, `program`, `indy`) | var, 61 749 sınıf çevriliyor | + `StackMapTable` | C0 |
| MIR | yok | tipli MIR, çift yol | C1-C3 |
| kaçış analizi | var (`escape.py`), sonucu sınırlı kullanılıyor | MIR üstünde, KKA'nın girdisi | C5, C6 |
| satır içi | opt-in, +%41 tahsis ölçüldü | MIR üstünde, KKA ile birlikte | C4, C6 |
| shard'lar | hash, `extern "C"` çağrı | yakınlık, doğrudan çağrı | C8 |
| çalışma zamanı ABI | tek crate, `rt_hash` | `mono_abi` + `mono_rt` | R1 |
| yığın izi | gölge yığın | PC tablosu | R2 |
| bellek | V yok, F var, R var (yalnız metot kapsamı), O yok, I yok, S var | V, F, R, O, I, S | M1-M6 |
| kaplamalar | 299 elle Java dosyası | üretilmiş kabuk + Rust | L1-L6 |
| JNI/JNA | 259 native | 0 | N1-N5 |
| platform | Linux | + Windows, macOS, Android doğrulanmış | P1-P4 |

## 8. Eski belgelerle ilişki

| belge | durum |
|---|---|
| `GC_PLAN.md` | bölüm 3 (eşzamanlı toplayıcı) koşullu yedek tasarım olarak kalır; hedef ve bölüm 8 faz tablosu **geçersiz**. Bölüm 2 (dört katman) bu belgedeki altı katmanla değişti |
| `AOT_ARCHITECTURE.md` | görüntü yığını (§4.1, §11.3, §16) ve kaynak önişleme (§4.2, §4.3, §11.5) bu mimarinin girdisidir ve geçerli; bölge kuralları (§5.2) belge 02'de yeniden yazıldı; Zig dosya adları tarihsel; yol haritası (§9) **geçersiz** |
| `IMPL_SPEC.md` | D1-D8, D10-D13 geçerli; D9 iptal; §2 (GC şartnamesi) **geçersiz**, A4/A5 görevleri iptal; B ve diğer görevler belge 06'ya taşındı |
| `MASTER_PLAN_TR.md` | ürün tanımı, WS13 ve platform akışları geçerli; WS1 ve WS7 tanımları bu belgeyle değişti |
| `RUST_RUHU_TR.md` | ölçüler geçerli; iş sırası belge 06 ile değişti |
| `ARCHITECTURE.md`, `MONOLITH.md` | ön yüz ve kod üretim şeması anlatımı büyük ölçüde geçerli (KOD); gölge yığın, GC anlatımları bu belgeyle değişti |

## 9. Belge haritası

| belge | konu |
|---|---|
| `00_ANA_MIMARI.md` | bu belge: amaç, borç, ilkeler, kararlar |
| `01_DERLEYICI.md` | MIR, tür çıkarımı, analizler, dönüşümler, ABI, shard, iz tablosu, geçiş |
| `02_BELLEK.md` | katmanlar V F R O I S, kanıt kuralları, çalışma zamanı API, GC kapatma ölçütleri |
| `03_CALISMA_ZAMANI.md` | crate'ler, nesne düzeni, iş parçacığı ve kilit, istisna, yerel metot kaydı, platform, grafik, ses, ağ |
| `04_KUTUPHANE_VE_YAMA.md` | kaplama sınıflandırması, sınıf seviyesi yama, kabuk üretici, kaynak hattı |
| `05_DOGRULAMA.md` | kapılar, test sınıfları, ölçü ve bütçeler, araçlar |
| `06_YOL_HARITASI.md` | iş paketleri, bağımlılıklar, kabul testleri, riskler, açık kararlar |
