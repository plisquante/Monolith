# Monolith

Monolith, Java programlarını Java olmadan çalışan bir programa çevirir.

Minecraft Java 26.3 ilk hedef.

## Ne işe yarar?

1. Java dosyalarını (bytecode) okur.
2. Kullanılan kodu bulur.
3. Kodu Rust diline çevirir.
4. Çalışan bir program üretir.

Bu programı çalıştırmak için Java kurmak gerekmez.

## Kendi Java kodunuzu Rust'a çevirmek

Bu proje Java'dan Rust'a çeviriye yoğunlaşır. Minecraft yalnızca ilk hedeftir.

Çevirici (`monolith/`) ve çalışma zamanı (`monolith/rt/`) Minecraft'a özel değildir. Java ile yazdığınız kendi
programınızı da Rust'a çevirip Java olmadan çalışan bir programa dönüştürebilirsiniz.

Amacınız buysa projenin bu bölümüne bakın:

```sh
# depo kök klasöründe (Monolith/) çalıştırın; MONO_JDK_HOME bir JDK 25 klasörünü göstermeli
export MONO_JDK_HOME=/yol/jdk-25
javac -d /yol/classes Main.java
python3 -m monolith.rustgen --cp /yol/classes --main Main -o /yol/out --shards 4 --compile
/yol/out/prog
```

- `--cp`: derlenmiş `.class` dosyalarının klasörü
- `--main`: `main` metodu olan sınıf
- Sonuç: `out/` içinde üretilen Rust kodu ve `out/prog` programı

Örnek programlar `tests/java/` klasöründedir. `python3 tests/run_tests.py` her birini hem Java ile hem çevrilmiş
haliyle çalıştırır ve çıktıların aynı olduğunu denetler.

Bilmeniz gerekenler:

- Üretilen Rust kodu elle yazılmış gibi değildir. Java'nın davranışını (çöp toplama, null, hatalar, sanal çağrılar)
  birebir korur.
- Program bir bütün olarak çevrilir: çalışırken yeni sınıf yüklenemez.
- Uygulamanın kendi JNI kütüphaneleri desteklenmez.
- Gereksinimler aşağıdaki "Nasıl kullanılır?" bölümüyle aynıdır (JDK 25, Rust, Python). Küçük programlar birkaç
  dakikada derlenir: liste, sıralama ve stream kullanan küçük bir örnek 4 çekirdekte yaklaşık 2,5 dakika sürdü, çünkü
  kullanılan JDK sınıfları da (bu örnekte 1 371 sınıf) birlikte çevrilir.

## Durum

Proje henüz bitmiş değil.

Ayrıntılar: [docs/PROJE_OZETI.md](docs/PROJE_OZETI.md)

## Nasıl kullanılır? (Linux)

Gerekenler:

- Python 3.10 veya üstü
- JDK 25
- Rust (stable)
- Yaklaşık 12 GB RAM
- Derleme yaklaşık 1 saat sürer

Adımlar:

```sh
python3 tools/fetch_client_jar.py --version 26.3 --out client.jar
python3 tools/mcbuild_rust.py client.jar -o build/minecraft-26.3
build/minecraft-26.3/run.sh
```

## Klasörler

- `monolith/` : çevirici ve çalışma zamanı
- `patches/` : elle yazılmış Rust kodları
- `tools/` : yardımcı programlar
- `docs/` : belgeler

## Lisans

Bu depo Minecraft dosyalarını içermez. Minecraft'ı bu depo ile dağıtmayın.
