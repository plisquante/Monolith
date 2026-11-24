# 05. Doğrulama mimarisi: kapılar, testler, bütçeler

Etiketler `00_ANA_MIMARI.md` ile aynıdır. İlke P4 ve P6: doğrulanmamış dönüşüm varsayılan olmaz; ölçülmemiş hedef iddia edilmez.

## 1. Test sınıfları

| sınıf | araç | durum |
|---|---|---|
| T1 fark (HotSpot ile) | `tests/run_tests.py` | 27/27 (3 atlandı: ClientCoreE2E, HttpFetch, Jni) KOD |
| T2 yerel/yama birim | `crc_tests`, `netty_tests`, `recycle_tests`, `stack_tests`, `patch_tests`, `patch_fp_tests` (9/9), `cache_tests` (7/7), `fetch_guard_tests` | KOD |
| T3 Rust birim | `tests/rust/audio_main.rs` (12), `crash_main.rs` (5) | KOD |
| T4 MIR birim | `mir_tests.py`: MIR yorumlayıcısı ile bayt kodu karşılaştırması | ÖNERİ (C1) |
| T5 KKA doğrulama | `MONO_VERIFY_KKA=1` (sanal nesneyi yine de ayır, alanları karşılaştır) + `Vec3/AABB/BlockPos` kimlik testleri | ÖNERİ (M1) |
| T6 bölge doğrulama | `MONO_REGION_CHECK=abort` + sağlamlık süiti | ÖNERİ (M3) |
| T7 oyun | `tools/headless_play.sh --clean-exit` (Creative, SIGTERM çıkışı, yerel çökme kontrolü) | KOD, doğrulandı |
| T8 sızıntı/tahsis | `leak_soak.sh`, `leak_budget.py`, `alloc_report.py`, `profile_sites.py`, `escape_hot.py` | KOD |
| T9 bağlantı | `check_link_errors.py` (açıklanmamış bağlantı hatası 0) | KOD |
| T10 stres | `GcStress Oop Threads Lambdas Strings Collections1` + `MONO_GC_MB=1 MONO_GC_POISON=1 MONO_ARENA_CHECK=abort` | KOD |
| T11 kare süresi | `tools/frame_time.py`: oyun içi kare süresi p50/p99 (headless, sabit dünya tohumu, sabit kamera yolu) | ÖNERİ (E6) |
| T12 HotSpot karşılaştırması | aynı dünya/yol ile JVM ve Monolith kare süresi, RSS | ÖNERİ (E6) |

## 2. Dönüşüm doğrulama kuralları

| dönüşüm | zorunlu kanıt |
|---|---|
| MIR kurucu | T4 + T1 yeşil; `mir_fallback` sayısı raporda |
| devirt / satır içi | T1 + hedef kümesi tutarlılık kontrolü (çalışma zamanı sayacı: tahmin edilen sınıf dışı sınıf görülürse `abort`, doğrulama modunda) |
| KKA | T5 |
| bölge | T6 |
| sınıf yaması O | `verify` testi + T8 (RSS düşüşü) |
| görüntü yığını | `MONO_IMAGE_VERIFY=1` |
| PC tablosu iz | `StackWalk`, `Exceptions` testleri + gerçek çökme raporu eşleşmesi |
| koşullu erişilebilirlik | T1 + T7 + `MONO_COUNT_NATIVES` düşüşü |

## 3. Kapılar (sürüm engeli)

| kapı | koşul | araç |
|---|---|---|
| Q1 statik | `pyflakes` temiz | |
| Q2 birim | T2, T3 hepsi geçer | |
| Q3 fark | T1 yeşil (hem eski hem MIR yolu geçiş süresince) | |
| Q4 bağlantı | T9: 0 açıklanmamış | |
| Q5 oyun | T7: dünya yüklenir, yürür, kaydeder, çıkar; çökme raporu 0; `Sound engine ERROR` yok | |
| Q6 sızıntı | `leak_budget.py` eğim ≤ 1 MiB/dk (30 dk ısınma sonrası) | |
| Q7 GC | G-S: oynanışta (dünya yüklü, 10 dk) toplama sayısı 0 ya da tahsis ≤ 1 MiB/dk | |
| Q8 performans | kare süresi p99 ≤ HotSpot'unki × 1,25; RSS ≤ HotSpot × 1,5 | T11, T12 |
| Q9 native | `Taklit == 0`, çalışma zamanı JNI çağrısı 0 | `MONO_COUNT_NATIVES` |
| Q10 kaynak | depoda elle yazılmış Java mantığı 0 | `lib_inventory.py` |
| Q11 derleme süresi | metot yaması sonrası < 10 dk; çalışma zamanı değişikliği < 5 dk (shard önbelleği ile) | |
| Q12 platform | her platform: önyükleme + dünya yükleme duman testi donanımda | belge 06 P işleri |

Aşama kapıları (G1, G2, G3, G-S) belge 02 §9; derleyici kapıları (C0-C9) belge 01 §12.

## 4. Metrik bütçeleri

| metrik | ölçü | bütçe (Q) |
|---|---|---|
| yönetilen tahsis (S) | MiB/sn, 10 dk | ≤ 0,02 (≈ 1 MiB/dk) |
| GC duraklaması | oynanışta | 0 toplama; yükleme ekranında ≤ 2 sn |
| kare süresi p99 | ms | ≤ HotSpot × 1,25 |
| açılış (ana menü) | sn | ≤ 15 (masaüstü) |
| derleme (tam) | dk | ≤ 90 (bugün ~65-70); ≤ 25 hedef (shard yakınlığı ve önbellek) |
| tam derleme belleği | GB | ≤ 12 (bugün ~10-12) |
| ikili boyutu | MB | ölçülür, bütçe AÇIK |
| RSS (boş dünya, 30 dk sonra) | MB | ≤ 2000 |

## 5. Alet zinciri

| araç | durum |
|---|---|
| `tools/release_check.py` (kapı toplayıcı) | KOD; Q kapılarına genişler |
| `tools/alloc_report.py` | KOD; `alloc_by_tier` (V/F/R/O/S) sütunu eklenir (M1) |
| `tools/frame_time.py` | yeni (T11) |
| `tools/lib_inventory.py` | yeni (belge 04 §2) |
| `monolith/mir_tests.py` | yeni (T4) |
| `tools/native_registry.py` | yeni (belge 03 §6) |

## 6. Regresyon kuralları

- Her dönüşüm bayrakla kapatılabilir (`--no-pass=NAME`); regresyon bulunursa ikili aramayla bayrak çifti ayrılır.
- Yeni dönüşüm "kapalı" gelir, T4-T6 geçince "açık" olur, bir sürüm sonra bayrak kaldırılır.
- CI: GitHub Actions bugün 3-4 sn'de hesap/runner tarafında düşüyor (KOD, sahip kararı); kapılar yerel çalıştırılabilir ve `release_check.py` ile toplanır. Düzeltme sahip işi (belge 06 risk R5).
