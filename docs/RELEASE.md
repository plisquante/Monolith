# Sürüm süreci (Monolith)

Bir sürüm, bir Minecraft sürümünün (26.3) test edilmiş, kapıları geçmiş ve kanıtları kaydedilmiş bir derlemesidir. Her kapıyı geçmeyen bir derlemeden hiçbir sürüm çıkarılmaz.

## 1. Derleme

    python3 tools/mcbuild_rust.py <client.jar> -o build/minecraft-26.3     # masaüstü, Linux x86_64

Rustc, JDK 25 (`MONO_JDK_HOME` ya da aracın indirdiği), 3 shard için yaklaşık 12 GB RAM ve yaklaşık 65 dakika gerekir.
Daha az belleği olan bir makinede `--shard-jobs 1` kullanın. Çalışma zamanı gövdesi değiştikten sonra nesneleri asla yeniden kullanmayın (`--reuse-objs` yalnızca eklemeli değişiklikler içindir; bkz. docs/IMPL_SPEC.md bölüm 7).

## 2. Kapılar (hepsi geçmeli)

| kapı | nasıl | geçme koşulu |
|---|---|---|
| Lint | `pyflakes monolith/*.py tools/*.py tests/*.py` | çıktı yok |
| Çalışma zamanı birim testleri | `rustc --edition 2021 --test tests/rust/audio_main.rs` ve `tests/rust/crash_main.rs` | hepsi geçer (ses 12, çökme raporlayıcı 5) |
| Yerel fark testleri | `python3 -m monolith.crc_tests`, `netty_tests`, `recycle_tests`, `stack_tests`, `patch_tests` (MONO_JDK_HOME ayarlı) | hepsi `ALL PASSED` |
| Link hataları açıklanmış | `MONO_VERBOSE=1 python3 -m monolith.rustgen ... 2> t.err`, ardından `python3 tools/check_link_errors.py t.err` (`docs/LINK_ERRORS_TR.md`) | açıklanmamış 0 |
| GC stresi, iki mod | `tests/run_tests.py GcStress Oop Threads Lambdas Strings Collections1`, `MONO_GC_MB=1 MONO_GC_POISON=1 MONO_ARENA_CHECK=abort` ile; normal ve `MONO_GC_CONCURRENT=1` ile | HotSpot ile aynı çıktı, panic yok, her modda altıdan altı |
| Oyun açılışı, normal | `tools/headless_play.sh build --seconds 60` | oyun çalışıyor, çökme raporu yok, `Sound engine ERROR` yok, `FFI types not initialized` yok |
| Oyun açılışı, eşzamanlı GC | `MONO_GC_CONCURRENT=1 tools/headless_play.sh ...` | normal modla aynı (bir headless koşu başlık ekranına ulaştı ve 120 sn ayakta kaldı, bkz. `docs/VERIFICATION.md`; tekrar koşular gerekli) |
| Sızıntı kapısı | `tools/leak_soak.sh build --minutes 60`, ardından `tools/leak_budget.py --warmup-s 1800 --budget-mb-per-min 1` | eğim <= 1 MiB/dk (toplayıcı açıkken bir koşuda eğim -0,054 MiB/dk, bkz. `docs/VERIFICATION.md`; tekrar gerekli; toplayıcı kapalı koşu 10 dakikada 13,3 GB residentta durduruldu, bkz. `docs/VERIFICATION.md`) |

## 3. Paketleme

    python3 tools/make_release.py build/minecraft-26.3 --out dist --name Monolith-Linux-x86_64 --version <x.y.z>

`dist/<ad>.tar.gz`, `dist/<ad>.sha256` ve `dist/<ad>.manifest.json` üretir (sürüm, kaynak commit, her dosyanın SHA-256'sı). Oyunun kendi dosyaları (kayıtlar, ayarlar, günlükler, çökme raporları) hiçbir zaman paketlenmez.

## 4. Bilinen engeller (bunlar varken sürüm çıkarılamaz)

- Eşzamanlı GC: önceki bir derleme ön-önyükleme aşamasında çöktü (DataFixers içinde ClassCastException). Mevcut derlemede `MONO_GC_CONCURRENT=1` ile iki headless koşu kayıtlı bir dünyayı yükledi, yürüdü, kaydetti ve çıktı; ClassCastException, panic ya da ölümcül satır çıkmadı (`docs/VERIFICATION.md`). İki koşu yetmez: eşzamanlı modun varsayılan olabilmesi için 60 dakikalık sızıntı soak'ı gerekir. O zamana kadar eşzamanlı mod varsayılan olarak kapalı kalır ve sürüm durdur-dünya toplamayı kullanır.
- Durdur-dünya duraklaması, 500 MB canlı heap ile yaklaşık 1,1 sn'ye ulaşıyor (ölçüldü, normal mod).
- Toplayıcı henüz kapatılamıyor: `MONO_GC=off` ile oyun, 16 GB'lık bir makinede 10 dakika hareketsiz beklerken 13,3 GB residente ulaştı (ölçüldü, durduruldu). Profil, boşta yaklaşık 19 MB/sn kısa ömürlü çöp gösteriyor; sızıntı değil (`docs/VERIFICATION.md`). `GC_PLAN.md` kademe 1-3 önce tahsisi azaltmalı.
- Android ses ve pencere arka uçları bir cihazda test edilmedi. Windows arka ucu var ve duman testi CI koşucularında geçiyor; oyun orada CI'da açılıyor (`.github/workflows/game.yml`); henüz gerçek donanım koşusu yok.
- Java kaplamaları (`classlib/`, `runtime/java_nolwjgl/`) ve JNA/Netty öykünmesi hâlâ yerinde.
- Android cihaz testi yapılmadı.
- Sızıntı kapısı iki koşuda geçti: önceki koşu ve 2026-11-24 (30 dakikalık ısınmadan sonra +0,180 MiB/dk, 60 dakikalık soak, normal GC, `docs/STATUS.md` §22). İkisi de boşta bir dünya soak'ı; yürüyüş, başka dünyalar ve soak sırasında eşzamanlı GC kapsanmadı. Aynı profilde ortalama durdur-dünya duraklaması 1,1 sn (en uzun 1,8 sn); iyi bir oyuncu deneyimi için bu hâlâ bir engel.

## 5. Platformlar

Şimdiye kadar yalnızca Linux x86_64 paketleniyor. Windows derlemeleri `tools/mcbuild_rust.py` ile kendi sistemlerinde üretilir (run.bat, prog.exe); onların paketlenmesi ve cihaz testleri, Android gibi ayrı sürümlerdir (docs/MASTER_PLAN_TR.md, M3 ve M4 kilometre taşları).
