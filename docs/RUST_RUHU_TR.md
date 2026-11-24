# Java ruhlu Rust'tan Rust ruhuna: ölçüler ve yol haritası

Hedef: çevrilen oyun "Java nesne modelini Rust'ta taklit eden" bir program olmasın; GC'siz, sahiplik (ownership) temelli,
değer tipleri stack'te, ömrü belli veri arenada, uzun ömürlü veri tek sahipli ve `Drop` ile serbest kalan bir Rust programı olsun.
Katman planı `GC_PLAN.md`'de. Bu belge **bugünkü ölçüleri** ve onlardan çıkan iş sırasını tutar.

## 1. Ölçü: çöp nereden geliyor (2026-11-24, 26.3, boş dünya, 62 dk)

Kaynak: `tools/alloc_report.py` (soak profili), `tools/escape_hot.py` (kaçış analizi), `tools/profile_sites.py` (nokta çözümü).

| | değer |
|---|---|
| tahsis edilen | 114 855 MiB, 2,52 milyar nesne (30,7 MiB/sn, 1,8 GiB/dk) |
| ömür sınıfı | **TEMP %95,8** (genç ölür), MIXED %3,2, LONG %0,3 |
| toplayıcı | 100 toplama, toplam duraklama 111 sn, ortalama 1,1 sn, en uzun 1,8 sn; canlı yığın 625 MiB |
| toplayıcıya hiç girmeyen | %19,1 (geri dönüştürülen iterator'lar, kapsam arenaları) |

En çok baytı üreten sınıflar (hepsi TEMP): `Vec3` 9,6 GB (209 M nesne), `Object[]` 8,1 GB, `ListItr`/`ArrayList$Itr`/`EnumSetIterator`
(iterator'lar, 16 GB'ın büyük kısmı zaten arenadan), `AABB` 6,1 GB (100 M), `BlockPos` 5,1 GB (166 M), `long[]` 4,0 GB, `double[]` 2,6 GB,
şekil sınıfları (`ArrayVoxelShape`, `CubePointRange`, `OffsetDoubleList`, `BlockCollisions`) ~7 GB, `Optional`, `GpuBufferSlice`,
`RenderPass$Draw`. Çözülebilen noktalar (profil sembol çözümü gürültülüdür, bunlar ipucudur): `BlockCollisions.computeNext`,
`VoxelShape.toAabbs`, `Vec3.align`, `AABB.contract`, `ClientLevel.addEntity/doAddParticle`, `LevelRenderer` çizim komutları.

**Uzun ömürlü veri çok küçük** (canlı 625 MiB'ın büyüğü: `long[]` paletli bölüm verisi 103 MiB, `Object[]` 61 MiB, `byte[]` 56 MiB,
`ThreadingDetector` 18 MiB, `StructureBlockInfo` 17 MiB, `PalettedContainer` 13 MiB). Yani çöpün neredeyse tamamı değer tipi ve geçici
koleksiyon; uzun ömürlü veri zaten "sahipli" olmaya uygun.

## 2. Kaçış analizi neden yetmiyor (statik: tahsis noktaları, çalışma sıklığı ağırlıklandırılmamış)

| sınıf | nokta | frame'e alınabilir | kaçan | ana neden |
|---|---:|---:|---:|---|
| `Vec3` | 450 | 135 | 313 | `arg` 178, `areturn` 108 |
| `AABB` | 118 | 38 | 80 | `arg` 39, `areturn` 33 |
| `BlockPos` | 207 | 31 | 176 | `arg` 114, `areturn` 38 |
| `BlockPos$MutableBlockPos` | 134 | 1 | 133 | `arg` |
| `ChunkPos` | 52 | 9 | 43 | `arg` |
| `ArrayList` | 1488 | 33 | 1453 | çözülemeyen çağrı |

İki ana neden: (1) nesne, kaçabilen bir çağrıya argüman olarak veriliyor (`arg`); (2) nesne döndürülüyor (`areturn`): `Vec3.add(...)` gibi
**değer tipi fabrikaları**, Java'nın değer tipi eksikliğini telafi eden kalıp. Bunlar Rust'ta `Copy` struct olur ve stack'te yaşar;
bayt kodunda ise her işlem bir `new`.

## 3. Java kalıbı → Rust karşılığı

| Java'da | Rust ruhunda | katman |
|---|---|---|
| `Vec3`, `AABB`, `BlockPos`, `ChunkPos` (değişmez, kimliği kullanılmaz) | `#[derive(Copy)] struct`, değerle geçer | 1 (değer tipi) |
| for-each iterator'ı | dilim üzerinde `for` | 1 (var: %19'u zaten arenada) |
| yöntem içi geçici `ArrayList`/dizi | `Vec`/dizi, stack ya da kapsam arenası | 1-2 |
| tick/kare/chunk ömürlü nesneler | o ömrün arenası, toplu serbest bırakma | 2 |
| chunk bölümü, palet, `long[]` veri | tek sahipli `Box`/`Vec`, chunk gidince `Drop` | 3 |
| paylaşılan ya da döngüsel grafik | `Rc`/`Arc`, çok az; yedek olarak mevcut toplayıcı | 4 |

## 4. İş sırası (her adım ayrı bayrak, ölçüm tutmazsa geri alınır)

1. **Yalnızca `--escape` ölçümü** (çalışıyor): `--escape --inline` tahsisi 19,0'dan 26,8 MB/sn'ye çıkarmıştı (`docs/VERIFICATION.md`); hangi
   bayrağın yaptığı ayrılmadı. `--escape` tek başına, aynı soak, aynı rapor.
2. **Değer tipi dönüşümü** (en büyük kazanç): `Vec3`, `AABB`, `BlockPos`, `ChunkPos` için kimlik kullanımı yok mu (`==`, `synchronized`,
   `identityHashCode`, `IdentityHashMap`) kapalı dünyada kanıtlanır; kanıtlanan sınıf çağrı sınırında alanlarına açılır (argüman ve
   dönüş değeri çoklu değer), yığın dışına yalnızca silinmiş (erasure) kapsayıcılara girerken kutulanır. `arg` ve `areturn` nedenlerinin
   ikisini birden ortadan kaldırır.
3. **Sıcak hatları elle Rust'a (yama):** çarpışma hattı (`BlockCollisions`, `VoxelShape`, `Shapes`) ve çizim komutları (`RenderPass$Draw`,
   `GpuBufferSlice`). Mekanizma var (`patches/`, parmak izli, doğrulamalı); `f64` yerel değişkenlerle, tahsissiz yazılır.
4. **Bölge arenaları:** tick ve kare başına arena; `MIXED` sınıflar için.
5. **Sahipli veri:** palet/chunk verisi `Drop` ile; toplayıcı yalnızca yedek.
6. **Java kaplamaları ve JNI** (WS7, WS2): `java_nolwjgl` önce, `classlib` sonra; anlatıcı (Flite) ve JNA ilk.

Her adımın kapısı: fark testleri, `check_link_errors.py`, oyun açılışı (`headless_play.sh --clean-exit`), 60 dk soak ve **duraklama
süresi**. Hedef: duraklama <1 ms ya da GC kapalı + soak büyüme <1 MiB/dk.

## 5. Dürüst riskler

- Değer tipi dönüşümü kapalı dünya kanıtı ister; `null`, `instanceof`, `equals`, jenerik koleksiyonlar ve yansıma (reflection) her biri
  ayrı durumdur. Yanlış kanıt sessiz davranış değişikliği demektir: her sınıf için fark testi şart.
- Elle yazılan Rust, Mojang kodunun davranışını birebir taşımak zorunda; Minecraft güncellenince parmak izi kapısı build'i durdurur.
- GC'yi tamamen kapatmak ancak 2-5 adım ölçüm geçtikten sonra mümkün; o zamana kadar toplayıcı yedek olarak kalır.
