# 02. Bellek mimarisi: V, F, R, O, I, S

Etiketler `00_ANA_MIMARI.md` ile aynıdır (KOD, ÖLÇÜ, KARAR, ÖNERİ, AÇIK). Hedef E1/E2: oynanışta yönetilen yığına (S) tahsis ≈ 0.

## 1. Ölçülen problem (ÖLÇÜ 2026-11-24, 26.3, boş dünya, 62 dk)

| ölçü | değer |
|---|---|
| tahsis | 114 855 MiB, 2,52 milyar nesne (30,7 MiB/sn) |
| ömür | TEMP %95,8, MIXED %3,2, LONG %0,3 |
| toplayıcı | 100 toplama, ort. 1,1 sn, en uzun 1,8 sn, canlı 625 MiB |
| toplayıcıya hiç girmeyen (arena, geri dönüşüm) | %19,1 |

Çöpün büyük kısmı değer tipi (`Vec3` 9,6 GB, `AABB` 6,1 GB, `BlockPos` 5,1 GB), geçici koleksiyon (`Object[]`, `long[]`, `double[]`, iterator) ve
çizim komutlarıdır (`GpuBufferSlice`, `RenderPass$Draw`). Uzun ömürlü veri küçüktür ve sahipli olmaya uygundur (palet `long[]` 103 MiB, `Object[]` 61 MiB,
`byte[]` 56 MiB).

Kaçış analizi sonucu (ÖLÇÜ `escape_hot.py`): `Vec3` 450 noktanın 135'i çerçeveye alınabilir; kaçanların nedeni `arg` 178 ve `areturn` 108.
Yani **metot sınırını aşan değer tipi** sorunu: metot kapsamlı analiz yetmez, çağrı sınırını aşan bir analiz ve çağrı sözleşmesi (ABI) gerekir.

## 2. Katman V: sanal nesne (kısmi kaçış analizi + ölçek ABI)

**KARAR D19.** Sınıf beyaz listesi yok. Her `new C` noktası için kanıt: nesnenin kimliği (`==`, `identityHashCode`, `synchronized`, `wait/notify`, `IdentityHashMap`
anahtarı, `System.arraycopy` hedefi değil) gözlenmiyor ve alanları yalnızca alan okuma/yazma ile kullanılıyor ise nesne **sanaldır**: hiç ayrılmaz; alanları yerel
değişkenlere (ölçek) açılır.

### 2.1 Kısmi kaçış analizi (KKA)
- Girdi: MIR (belge 01). Akış duyarlı, alan duyarlı, çağrı grafı üzerinde parametre özetleri (`escape.py` özetleri `ret_fresh, rsites, cparams` MIR'a taşınır).
- Her `new` için durum: `Sanal` (hiç kaçmıyor), `Kısmi` (yalnızca bazı yollarda kaçıyor; kaçış yolunda **gerçekleştirme** (materialize) konur), `Kaçar`.
- Gerçekleştirme noktası: nesne kimlik gerektiren yola girmeden hemen önce, ölçeklerden gerçek nesne tahsisi (katman S ya da R). Böylece `Vec3.add` zinciri
  kimlik kullanılmadıkça hiç tahsis etmez.
- Kimlik gözlemleri **yapısal olarak** korunur: sanal nesnenin kimliği gözlenirse analiz onu `Sanal` yapmaz. Değer tipi sınıf varsayımı (Copy struct) gerekmez.

### 2.2 Ölçek ABI'si (çağrı sınırı)
- Özet: `f(Vec3 a)` metodunun parametresi `a` için "yalnızca alanları okunur, kaçmaz, kimliği gözlenmez" ise metodun **ölçek özelleştirmesi** `f$s(a_x, a_y, a_z)` üretilir.
- `areturn` için: `Vec3 add(...)` metodunun dönüşü her yolda taze ve kaçmayan nesne ise `add$s -> (f64,f64,f64)` üçlüsü döner (Rust demeti, yazmaçlarda).
- Çağıran, çağrılanın özelleştirmesini seçer; özelleştirme yoksa ya da sanal çağrı hedefleri farklıysa **gerçek nesneyi** geçirir. Çift giriş noktası (`f` ve `f$s`) vardır; her ikisi de aynı gövde özetinden üretilir.
- Sanal çağrı (devirtualization yok): hedef kümesi RTA ile küçükse sınıf başına özelleştirme tablosu; aksi halde gerçekleştir.
- Sınır: en fazla 8 ölçek parametre (aşılırsa gerçekleştir), özyinelemeli özet sabit nokta ile (en kötü durumda `Kaçar`).

### 2.3 Kabul
| ölçü | bugün | hedef | araç |
|---|---|---|---|
| `Vec3` tahsisi (kararlı durum) | 209 M nesne/62 dk | −%90 | `alloc_report.py` |
| `AABB` | 100 M | −%80 | aynı |
| `BlockPos` | 166 M | −%70 (MutableBlockPos kimlik kullanır, tutucu) | aynı |
| davranış | | fark süiti + `Vec3`, `AABB`, `BlockPos` özel fark testi (kimlik, null, equals/hashCode, istisna yolu) | `tests/run_tests.py` |

Risk: gerçekleştirme sırasında alan sırası ve istisna semantiği. Doğrulama modu: `MONO_VERIFY_KKA=1` her sanal nesneyi yine de ayırır ve alanları ölçeklerle karşılaştırır (belge 05).

## 3. Katman F: çerçeve nesneleri

**KOD** (`escape.py`, `rt.rs` `stack_obj`): bugün var; kaçmayan nesne yığın yuvasında başlıklı blok olarak yaşar. KKA sonrası F, yalnızca **adresi gereken** (alan tablosu, dizi, `Object`'e geçirilen) ama kaçmayan nesneler içindir.
- Döngü kuralı: döngü içinde `new` ve yineleme sonunda ölüyorsa her yinelemede aynı yuva (bugün `loop_pcs`/`iter_pcs`).
- Boyut sınırı: 4 KiB üstü diziler F'e girmez (yığın taşması); `stack_check` bütçesine yazılır.
- GC kökü: F nesnelerinin referans alanları taramada gözetilmelidir (bugün frame tarama var, KOD). O ve R'ye giden referans bu yolla S'ye ulaşabilir; sınırlar bölüm 8'de.

## 4. Katman R: bölge (arena)

**KARAR D20**: iki aşama.

### 4.1 Elle yazılmış bölgeler (önce)
- `Region` (Rust): `Bump` + Drop. API `mono_abi::region_enter()/region_exit()`; tick, kare, görev, chunk yükleme başına.
- Bölgeye yalnızca Rust yamaları (belge 04) tahsis eder: çarpışma hattı (`BlockCollisions`, `VoxelShape.toAabbs`), çizim komut tamponu (`RenderPass$Draw`, `GpuBufferSlice`).
- **Borç denetleyicisi kanıt verir**: bölgeden alınan referans yaşam süresi `'r` ile bağlıdır; bölge dışına çıkan değer açıkça `to_managed()` ile S'ye kopyalanır. Derleyici hata verir, çalışma zamanı kuralı yok.

### 4.2 Otomatik bölge çıkarımı (araştırma hattı)
- Girdi: KKA'dan `Kaçar` çıkan ama "yalnızca çağıran metot (ya da tick/kare) döngüsü bitene kadar yaşar" kanıtlanan noktalar.
- Kanıt: çağrı grafında "bölge sahibi" metot (tick, render, chunk görevi) ve değerin yalnızca bölgenin içinden erişilen alanlara/yerellere yazıldığı.
- Kuralı sağlamayan bölge çıkarımı **açılmaz**: sağlamlık test paketi (belge 05) ve `MONO_REGION_CHECK=abort` (bölge bitince bölgeden gelen referansı tarayan doğrulama) geçmeden varsayılan yapılmaz. Durum: AÇIK araştırma; V+O+elle R bu hedefi tek başına karşılayabilir (bölüm 9).

### 4.3 Arena bugünü
**KOD**: `rt.rs` kapsam arenaları (`MONO_ARENA_CHECK`) var ve toplayıcıya girmeyen %19,1'in kaynağı; bölge API'si bu mekanizmayı genelleştirir.

## 5. Katman O: sahipli yerel depolama

**KARAR D21.** Uzun ömürlü büyük veri Rust yapısı olur, sınıf yaması (belge 04 §5) ile Java sınıfının alanı ve yöntemleri Rust'a taşınır.

| aday | boyut | yaklaşım |
|---|---|---|
| `PalettedContainer` + `long[]` veri | 103 MiB + 13 MiB | `Storage` Rust `Box<[u64]>`, chunk gidince `Drop` |
| `LevelChunkSection` `byte[]`, `Object[]` | 56 + 61 MiB | sahipli dilim |
| `ThreadingDetector` (18 MiB) | | tembel + küçük |
| `StructureBlockInfo` | 17 MiB | S'de kalır (düşük değer) |

- Java tarafı görünümü: `long[]` gibi görünen bir **tutamaç** nesnesi (S'de küçük, 16 bayt başlık + sahipli işaretçi); `Drop`'u toplayıcı sonlandırıcısı değil, **sahibin** açıkça `free()` çağırması (chunk yaması) ya da S'nin zayıf sonlandırma kuyruğudur (yedek).
- GC'ye görünürlük: O'daki depolama referans içermez (yalnızca sayılar) ya da referanslar tutamaç aracılığıyla S'ye kök bildirilir. İlk aşamada **yalnızca referans içermeyen** depolar (sayı dizileri) O olur.
- Kabul: palet verisi tahsisi S'den görünmez; chunk boşaltmada RSS iner (soak `leak_budget`).

## 6. Katman S: yönetilen yığın (yedek)

**KOD**: mevcut durdur-işaretle-süpür toplayıcı (`gc.rs`), slab + arena + frame nesneleri korunur. **KARAR**: eşzamanlı GC (D9) iptal; S yalnızca yükleme, menü, acil durum ve V/F/R/O kanıtı çıkmayan nesneler içindir.
- GC kapatma kapısı (G-S): oynanışta (dünya yüklü, 10 dk) tahsis ≤ 1 MiB/dk ise toplayıcı **tetiklenmez** (`MONO_GC=lazy`: yalnızca yığın eşiği aşılırsa).
- Acil: eşik aşılırsa yine toplar; bu duraklama raporlanır (`gc.pause` sayacı), ve kapı yeşil sayılmaz.

## 7. Katman I: görüntü yığını

**ÖNERİ D22** (varsayım: başlatma izleyicisi sınıf başlatıcılarının ≥ %60'ını yan etkisiz çıkarabilir; arm64 ve x64 düzen aynı).
- Derleme zamanı: `<clinit>` yan etkisiz ve kapalı dünyada saf ise (yalnızca sabit, dizi, `Map.of`, enum değerleri) Python yorumlayıcısıyla çalıştırılır, sonuç nesne grafı `static` veri olarak yayılır (`.rodata`/`.data`), sınıf başlatma bayrağı "başlatıldı" olur.
- Yan etkili başlatıcı (`System.nanoTime`, dosya, thread) çalışma zamanında kalır.
- Kazanç: açılış süresi (E7), `init_X().res()?` çağrılarının silinmesi (AB7), ölümsüz veri GC dışı.
- Kabul: ana menü süresi; görüntü yığını nesneleri GC taramasından hariç (ölçülen tarama süresi düşer).
- Risk: yorumlayıcı JVM semantiğini taklit eder; yalnızca küçük, kanıtlı alt küme (`MONO_IMAGE_VERIFY=1` çalışma zamanında yeniden başlatıp karşılaştırır).

## 8. Katmanlar arası referans kuralları

| kaynak → hedef | izin | kural |
|---|---|---|
| V → herhangi | gerçekleştirme ile | kimlik gereken noktada S ya da R tahsisi |
| F → S, R, O | evet | GC kökü: çerçeve taraması |
| R → S | evet | bölge nesnesi S'ye yazılırsa bölge o noktada `to_managed` kopyalar (4.1) |
| S → R | **hayır** | S nesnesi bölge nesnesini tutamaz (borç denetleyicisi ya da `MONO_REGION_CHECK`) |
| S → O | tutamaç aracılığıyla | |
| O → S | yalnızca sayı depolarında yok | referans içeren O ilk aşamada yasak |
| I → S | hayır (I değişmez) | I nesnesinin alanı S'ye yazılırsa derleme zamanı hata (yazılabilir alan ya da yazma bariyeri ile "I'dan çıkar") |

Yazma bariyeri **yok**. Bu yüzden S → R ve O → S yasağı derleme zamanı kuralıdır.

## 9. GC kapatma planı ve kapılar

| kapı | koşul | ölçü |
|---|---|---|
| G1 | KKA + ölçek ABI açık | S tahsisi ≤ 12 MiB/sn (−%60) |
| G2 | + elle R (çarpışma, çizim) | ≤ 5 MiB/sn |
| G3 | + O (palet, chunk) | ≤ 2 MiB/sn, canlı yığın ≤ 300 MiB |
| G-S | + iterator/koleksiyon işi (dilim üstü `for`, `ArrayList` tahsisini V/F'ye) | ≤ 1 MiB/dk, oynanışta 0 toplama |

Eşik 5 MiB/sn üstünde kalırsa (AÇIK): D9'un yeniden değerlendirilmesi (eşzamanlı toplayıcı, tasarım `GC_PLAN.md` §3) ve bölge çıkarımının otomatiği (4.2).

## 10. Bilinçli olarak yapılmayanlar

| seçenek | neden hayır |
|---|---|
| Referans sayımı (Rc) (KARAR D26) | döngüsel grafikler (Mojang kodunda yaygın), her kopyada maliyet, `unsafe` ham `Ref` ile uyumsuz |
| Tüm nesneler için Rust sahiplik çıkarımı | Java'da paylaşımlı değişebilir grafikler yaygın; kanıt yoksa S'ye düşmek P3 |
| Sınıf beyaz listesiyle `Copy` struct | kimlik kullanan nadir yolu kaçırırsa sessiz davranış değişir (P4); KKA aynı kazancı kanıtlı verir |
| Eşzamanlı SATB GC | tasarım ve bariyer maliyeti büyük; oynanışta 0 toplama hedefi bunu gereksiz kılıyor |
| Başlık küçültme (16 → 8 bayt) | V/R/O sonrası ölçülür (D25 AÇIK) |

## 11. Çalışma zamanı API (mono_abi, belge 03)

```
mono_abi::alloc_s(cls, size)            // S: bugünkü tahsis, hızlı yol satır içi
mono_abi::alloc_frame(cls, slot)        // F
mono_abi::region_enter() -> RegionId    // R
mono_abi::region_alloc(r, cls, size)
mono_abi::region_exit(r)
mono_abi::owned_new(kind, len) -> Handle  // O
mono_abi::owned_free(h)
mono_abi::materialize(cls, fields...)   // V -> S/R
```

Hepsi **tahsis noktasının** kararıdır; bellek nesne düzeni (`Obj` 16 bayt) tüm katmanlarda aynıdır, böylece S taraması ve yansıma değişmez.
