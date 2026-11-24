# Monolith

Monolith, Java programlarını Java olmadan çalışan bir programa çevirir.

Minecraft Java 26.3 ilk hedef.

## Ne işe yarar?

1. Java dosyalarını (bytecode) okur.
2. Kullanılan kodu bulur.
3. Kodu Rust diline çevirir.
4. Çalışan bir program üretir.

Bu programı çalıştırmak için Java kurmak gerekmez.

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
