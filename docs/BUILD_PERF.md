# Derleme hızı: shard'lar ve paralel işler

Soru: üretilen Rust kodunu paralel derlemek (daha fazla `rustc` shard'ı, daha fazla eşzamanlı iş), çok çekirdekli bir makinede derlemeyi hızlandırır mı?

## Ölçüm (her ayar için bir koşu, aynı girdi)

Girdi: `monolith/tests/stack/Stack1.java` javac ile derlenir, ardından `python3 -m monolith.rustgen --main Stack1 --opt 1 --compile`. Sınıf kütüphanesi zaten derlenmiş (önbellekte) olduğu için sayılar yalnızca program derlemesidir.

| shard | shard işi | duvar saati |
|---|---|---|
| 1 | 1 | 217.6 sn |
| 4 | 4 | 70.5 sn |

Hızlanma: 4 çekirdekte yaklaşık 3.1 kat (çıktıda 1358 sınıf, 7036 metot).

Makine: 4 vCPU, 15 GB RAM, Linux. Her ayar için bir koşu; sayılar ortalaması alınmamış.

## Nasıl tekrarlanır

    tools/bench_build.sh <sınıflar dizini> <ana sınıf> <shard> <iş>

Yeni bir çıktı dizini ile tek bir derlemenin duvar saati süresini yazdırır. Ayar başına iki kez çalıştırıp karşılaştırın.

## Notlar

* Varsayılanlar çekirdek sayısını izler: `tests/run_tests.py` ve `monolith/stack_tests.py` `min(4, çekirdek)` shard ve `çekirdek` iş kullanır. `tools/mcbuild_rust.py` (Minecraft derlemesi) hâlâ `--shards 32 --shard-jobs 3` kullanır; bu makinede ölçülmedi.

## Henüz ölçülmedi

* Minecraft derlemesi (varsayılanlarla yaklaşık 65 dakika ve 12 GB). Client jar'ı ve sakin bir makine gerektirir; Minecraft sınıfları üzerinde `tools/bench_build.sh` çalıştırın, `--shards 4 --shard-jobs 4` ve varsayılanlarla birlikte, ikisini de buraya kaydedin.
* Tekrarlı koşular: ayar başına tek koşu, hız hakkında bir iddia için yeterli değil. En az üç koşu gerekir.
* Bellek: eşzamanlı her `rustc` kendi payını alır. 4x4 koşusu Stack1 için 15 GB'ın çok altında kaldı; bu, Minecraft derlemesinden çok daha küçük.
