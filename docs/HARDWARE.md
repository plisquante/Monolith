# Görev başına donanım gereksinimleri

Son güncelleme: 2026-11-16. Büyük makine için rakamlar, orijinal yazarın 4 çekirdekli, llvmpipe kullanan makinesinden alındı
(bkz. `docs/STATUS.md` bölüm 8–12). Kısıtlı makine sütunu, Kasım 2026'da bu depoyu denetleyen makinede ölçüldü.

## Bu depoyu denetleyen makine (2026-11-16)

| kaynak | değer |
|---|---|
| CPU | 2 vCPU, Intel Xeon @ 2.60 GHz |
| RAM | 1.94 GiB (`MemTotal: 2030416 kB`), **takas (swap) yok** |
| disk | ~25 GB, başlangıçta %17 dolu |
| çekirdek | 6.1.189+ x86_64 |

## Nerede ne sığar

| görev | asgari makul | 2 vCPU / 1.9 GiB kutusunda |
|---|---|---|
| `python3 -m py_compile`, `pyflakes`, `tools/rt_typecheck.py` | herhangi biri | Uygun (dosya başına < 1 sn) |
| `pyflakes`/`ast-check` tam geçiş | herhangi biri | Uygun |
| **Tek** fark testi, Debug, kendi arka ucumuz (`tests/run_tests.py Arith`) | ~2 GiB boş, 1 iş | **495 sn'de GEÇTİ** (derleme) + 0.58 sn yerel; makine ağır yük altında (loadavg ~10), etkileşim zayıf |
| `tests/java/` içindeki tüm fark paketi | ≥ 4 GiB boş, saatlerce CPU | takılma riski: çalıştırmayın; `MONO_TEST_MIN_AVAIL_GIB` (varsayılan 0.5 GiB) altında testler reddedilir |
| tam Minecraft derlemesi (`build.sh client.jar`) | Debug kendi arka ucu: ~10 GB tepe; bölünmemiş LLVM: > 15 GB (yazarın 16 GB makinesinde OOM ile öldürüldü) | imkânsız; buradaki `./build.sh` hiçbir şeyi başarıyla tamamlamaz |
| Client Core E2E (`tests/run_client_core_e2e.py`), 5 901 sınıfı çevirme + derleme | ≥ 8 GiB, onlarca dakika | imkânsız; burada çalıştırılmadı |
| microbench (`tools/microbench/run_client_core_bench.py`) | ReleaseFast, ~2–4 GiB | burada çalıştırılmadı (ReleaseFast LLVM bellek gereksinimi) |

## Betiklerin artık uyguladığı kurallar

- `tests/run_tests.py`, test derlemesini `--min-avail-gib` altında (varsayılan 0.5 GiB, ortam değişkeni `MONO_TEST_MIN_AVAIL_GIB`) başlatmayı reddeder; her testten önce yeniden denetler ve takılmak yerine çıkış koduyla 3 ile durdurur.
- `.claude/hooks/guard.py`, 1 GiB'ın altında boş belleğe sahip bir ağır işi başlatmayı reddeder (≥ 8 GiB'lık makinelerde 4 GiB; `MONO_MIN_AVAIL_GIB` ile değiştirilir), ikinci bir eşzamanlı ağır işi reddeder ve derleme çıktılarının commit edilmesini engeller.
- Aynı anda tek bir ağır iş. Büyük makinede de fark paketiyle asla birlikte çalıştırılmaz (CLAUDE.md); bu kutuda hiç çalıştırılmaz.

## Disk beklentileri

`tests/build/<Ad>` test başına yaklaşık 1–1,4 GB tutar (`--keep`); birden çok ad verildiğinde araç geçen testlerin ardından bunu siler. `tests/build` (rustc shard'ları ve ikili dosyalar) yüzlerce MB büyüyebilir. Minecraft derlemeleri onlarca GB ister ve küçük disklerin kapsamı dışındadır. Ağır koşular arasında `rm -rf tests/build` kullanın.
