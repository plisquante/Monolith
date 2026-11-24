# 01. Derleyici mimarisi: ön yüz, MIR, analizler, arka yüz

Etiketler `00_ANA_MIMARI.md` ile aynıdır.

## 1. Bugünkü boru hattı (KOD)

`classfile.py` (sınıf dosyası) → `world.py` (sınıf dünyası, mangle) → `program.py` (RTA, `indy` açılımı, `insns()`) → `rustgen.py` (`MethodGen.gen_insn`:
bayt kodu → Rust metni, ara temsil yok) → shard'lar + `mono_rt` → `rustc`. Yan analizler `escape.py`, `inline.py`, `patches.py` bayt kodu üzerinde çalışır.

Sınırlar (belge 00 AB1-AB8): ara temsil yok, `StackMapTable` kullanılmıyor, sınıflar arası çağrı satır içine alınamıyor, her metot gölge yığın çerçevesi itiyor.

## 2. Hedef boru hattı

```
classfile → world → program (RTA, koşullu erişilebilirlik) → MIR kurucu → [analiz + dönüşüm geçişleri] → Rust yayıcı → shard'lar
                                                                   ▲
                                                      StackMapTable (C0), patches (belge 04)
```

## 3. MIR tasarımı (KARAR D15)

| özellik | karar |
|---|---|
| biçim | metot başına temel bloklar, **SSA** (blok parametreli), açık değişken türleri |
| türler | `i32 i64 f32 f64`, `ref(sınıf kümesi, null_olabilir)`, `ref[]`, ölçek grupları; **tür `StackMapTable` + bayt kodu doğrulama kurallarıyla** çıkarılır |
| kontrol akışı | blok sonlandırıcıları (`br`, `br_cond`, `switch`, `ret`, `throw`, `invoke` + istisna kenarı); `jsr/ret` yok (Java 25 sınıf dosyasında zaten yok) |
| istisna | her çağrı/tuzaklı komut için açık "normal" ve "istisna" kenarı; işleyici blok parametreyle istisnayı alır |
| çağrı | `call_static`, `call_special`, `call_virtual(hedef kümesi)`, `call_interface(hedef kümesi)`, `call_native` |
| bellek | `load_field`, `store_field`, `new`, `new_array`, `alloc_kind` (S/F/R/O karar alanı), `materialize` |
| yan etki | her komutun etkisi (`pure`, `reads_heap`, `writes_heap`, `may_throw`, `may_sync`) etiketlidir (satır içi/katlama bundan karar verir) |
| kaynak konumu | her komut `pc` ve satır taşır (iz tablosu için, §8) |

MIR kurucu (C1): bayt kodu → SSA: yığın simülasyonu ile (mevcut `gen_insn` benzeri), ama değer adlarıyla; blok başı dönüşümleri `StackMapTable` çerçeveleriyle doğrulanır.
Doğrulama: her metot için MIR → bayt kodu eşdeğerliği değil, **MIR yorumlayıcısı** (Python) ile ufak fark testi (belge 05: `mir_tests.py`).

## 4. StackMapTable (C0)

- `classfile.py` Code niteliği içindeki `StackMapTable` ayrıştırılır: `same`, `same_locals_1_stack_item`, `chop`, `append`, `full` çerçeveleri → blok başında yerel + yığın türleri.
- Kullanım: referans tür çıkarımı (`ref` için sınıf üst sınırı), `null` bilgisi (yalnızca `null` türü), sanal çağrı hedef kümesi daraltma (CHA/RTA ∩ tür üst sınırı), ölçek özelleştirme kararı (§6).
- Kabul: Minecraft kapanışındaki tüm metotlar için ayrıştırma hatasız ve çıkarılan tür her komutta bayt kodu doğrulayıcısıyla (JDK `-Xverify:all` karşılaştırması) uyumlu.

## 5. Analizler (MIR üstünde)

| analiz | girdi | çıktı | bugün |
|---|---|---|---|
| çağrı grafı (RTA + CHA + tür üst sınırı) | MIR + StackMapTable | `call_virtual` hedef kümeleri | `program.py` RTA (KOD), tür sınırsız |
| null analizi | MIR | `non_null` işaretleri (null kontrolü silme) | yok |
| kimlik gözlemi | MIR | `==`, `synchronized`, `identityHashCode`, `IdentityHashMap`/`Set` kullanım işareti | yok |
| kaçış (kısmi) | MIR + özetler | `Sanal/Kısmi/Kaçar` + gerçekleştirme noktaları | `escape.py` (metot kapsamlı + özetler, KOD) |
| etki analizi | MIR | `pure`, `reads_heap`, `writes_heap`, `may_throw` | yok |
| yan etkisiz `<clinit>` | MIR | görüntü yığını adayı (belge 02 §7) | yok |
| koşullu erişilebilirlik | MIR + sabit katlama | ölü dal/sınıf (belge 04 §6) | yok |

## 6. Dönüşümler (sıra önemlidir)

1. **Sabit katlama + ölü kod** (koşullu erişilebilirlik için de gerekli): yamalı sabit metotlar (`Epoll.isAvailable()`) çağrı yerinde katlanır, RTA yeniden çalışır (sabit nokta, en çok 3 tur).
2. **Devirtualization**: hedef kümesi tek ise `call_virtual → call_special`; küçük küme (≤3) ise tür testiyle dallan; kalan `itab` ya da `vtable`.
3. **Satır içi (MIR)**: sığ ve sıcak (kod boyutu ve ölçülen çağrı sayısına göre) metotlar; yamalı metotlar (`patches`) satır içine girmez (parmak izi sözleşmesi); `synchronized`, `native`, istisna tuzağı çok olan metotlar girmez.
4. **KKA + ölçek özelleştirme** (belge 02 §2): satır içinden sonra çalışır.
5. **Bölge ve tahsis türü seçimi** (belge 02 §3, §4): `alloc_kind` alanı.
6. **Boş kontrol silme**, **sınır kontrol silme** (basit döngüler: `for (i=0;i<a.length;i++)`; indüksiyon değişkeni kanıtı).
7. **Statik başlatma kaldırma**: görüntü yığınında başlatılmış sınıflar için `init_X()` silinir (AB7).
8. **Çağrıda `stack_check` birleştirme**: yaprak ve sığ metotlarda kaldırılır; döngü içinde yinelemede yalnızca yeterli yığın bütçesi yoksa kontrol (statik yığın kullanım tahmini).

Her dönüşüm bayrakla kapatılabilir (`--no-pass=NAME`), tek tek fark testi çalıştırılabilir (belge 05).

## 7. ABI (çağrı sözleşmesi)

| konu | bugün (KOD) | hedef |
|---|---|---|
| çift ABI | `r_*` (iç, `Result`) ve `m_*` (`extern "C"`, `CRes`) | korunur; `m_*` yalnızca dışa dönük (yerel kayıt, yansıma, thread girişi) ve **shard'lar arası** değil |
| dönüş | `Result<T, Exc>` | aynı; `Exc` işaretçi boyutu, `Ok` yolu sıcak |
| ölçek | yok | `f$s` özelleştirmeleri (belge 02 §2.2) |
| shard'lar arası çağrı | `extern "C" m_*` + `CRes` | doğrudan `r_*` çağrısı (`pub(crate)` + aynı derleme birimi için bağlantı); **aynı shard** içinde satır içi (D18) |
| `Ref` | ham işaretçi | aynı |
| istisna | `Result` yayılımı, her çağrıda `?` | aynı (sıfır maliyetli istisna **yapılmaz**: rustc `-C panic=abort` ve unwinding yok) |

## 8. Sıfır maliyetli yığın izi (ÖNERİ D17)

Bugün (AB5, KOD `rt.rs:1285-1322`): her metot girişinde gölge yığına `Frame` iter, her deyimde `set_line`, çıkışta pop. Maliyet: thread-local `RefCell<Vec>` erişimi.

Hedef:
- Çerçeve işaretçisi (`-C force-frame-pointers=yes`) + PC tablosu (`.mono_pc`): `(pc_aralığı → metot_id, satır_tablosu_ofseti)`; derleme zamanında shard'larda üretilir (satır eşlemesi bugünkü `set_line` kaynağından alınır).
- Yığın yürüme (`Throwable.fillInStackTrace`, `StackWalker`, `Thread.getStackTrace`): çerçeve işaretçi zinciri → PC → metot + satır.
- Satır doğruluğu: PC aralığı yalnızca çağrı noktalarında kesin (çağrı sitesinde `pc` kaydı). Çağrı olmayan istisna yolları (null, sınır, bölme) yine çağrı sitesi sayılır (panik yerine `Exc` yayma çağrısı).
- Eski yol (`--shadow-stack`): hata ayıklama için bayrakla korunur; fark testi iki yolu karşılaştırır.
- Varsayım (AÇIK, kapı): LLVM satır içi yaparken PC aralığı **sanal çerçeveleri** (satır içine alınmış metotlar) için satır tablosu üretilir (MIR satır içi bunu bilir ve tabloya "iç çerçeve" kayıtları yazar). Doğrulama `StackWalk`, `Exceptions`, `Throwable` fark testleri ve Minecraft çökme raporu eşleşmesi.
- Kazanç hedefi: kare süresi p50 ve p99 (belge 05) ölçülür; hedef kazanç ≥ %10 CPU.

## 9. Çift yollu geçiş (P8, C-iş paketleri)

Geçiş büyük patlama değildir:
1. `rustgen` bugünkü yol (**eski yol**) korunur.
2. MIR yolu metot başına açılır: `MethodGen` bir metot için MIR kurucuyu dener; **başarısız ya da yayıcıda desteklenmeyen komut** olursa eski yola düşer ve `mir_fallback` sayacına yazılır (sessiz değil, rapora girer).
3. Kapı: `mir_fallback == 0` olunca eski yol silinir (C9).
4. Dönüşümler (§6) yalnızca MIR yolundaki metotlarda çalışır.
5. Çift yol çıktısı fark testi: aynı metot iki yoldan da üretilip davranış karşılaştırması (`tests/run_tests.py --path=old|mir`).

## 10. Shard ve derleme (ÖNERİ D18)

| konu | bugün | hedef |
|---|---|---|
| shard anahtarı | sha1(sınıf adı) % n (KOD `rustgen.py:2160`) | çağrı grafı yakınlığı: sık çağıran-çağrılan çiftleri aynı shard (ağırlıklı grafik bölme, sınıf dağılımı yineleme) |
| çapraz-shard çağrı | `extern "C"` (satır içi yok) | doğrudan `r_*`; ThinLTO ya da `#[inline]` ölçülür |
| çalışma zamanı değişikliği | `rt_hash` → tüm shard'lar geçersiz (~70 dk) | `mono_abi` sürümü değişmedikçe shard'lar geçerli (D16); `mono_rt` değişikliği yalnızca `mono_rt` crate'ini yeniden derler |
| önbellek | `/tmp/monolith_shard_cache` 8 GiB LRU (KOD) | aynı + `mono_abi` sürüm anahtarı |

Kabul: tek metot yaması sonrası yeniden derleme süresi (belge 05 `build_time` kapısı): çalışma zamanı değişikliği < 5 dk, metot yaması < 10 dk.

## 11. Rust yayıcı kuralları

- Her MIR komutunun Rust karşılığı tek yerde (`emit.py`), `unsafe` kapsamı en küçük blok.
- Taşma, `Integer.MIN_VALUE / -1`, `float → int` dönüşüm kuralları `mono_abi` yardımcılarında (bugünkü `rt.rs` içinde; KOD) ve satır içi.
- `-C overflow-checks=off`, `-C debug-assertions=off` sürüm yapısında; fark testleri hata ayıklama yapısında da çalışır.
- Üretilen kodda panik **yok**: panik = çalışma zamanı hatası → `crash.rs` raporu (KOD).

## 12. Kapılar

| kapı | tanım |
|---|---|
| C0 | StackMapTable ayrıştırma + tür doğrulayıcı uyumu |
| C1 | MIR kurucu: tüm kapanış, `mir_fallback` sayısı raporlanır |
| C3 | MIR yolu `tests/run_tests.py` yeşil (27/27) |
| C5-C6 | KKA + ölçek: belge 02 §2.3 |
| C8 | shard yakınlığı + doğrudan çağrı: kare süresi ve derleme süresi ölçümü |
| C9 | eski yol silindi |
