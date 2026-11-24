# MONOLITH

Monolith, Java bytecode'unu JVM olmadan çalışan yerel bir programa çevirir.

Sınıf dosyalarını okur, erişilebilir kodu bulur, Rust'a çevirir ve çalıştırılabilir dosya üretir.
İlk hedef: Minecraft Java Edition 26.3 istemcisi.

## Durum

Ayrıntılı ve güncel durum: [docs/PROJE_OZETI.md](docs/PROJE_OZETI.md)

Henüz bitmiş bir ürün değil.

## Hızlı başlangıç (Linux x86_64)

Gereksinimler: Python 3.10+, JDK 25 (`MONO_JDK_HOME`), stable rustc, yaklaşık 12 GB RAM, yaklaşık 1 saat.

```sh
python3 tools/fetch_client_jar.py --version 26.3 --out client.jar
python3 tools/mcbuild_rust.py client.jar -o build/minecraft-26.3
build/minecraft-26.3/run.sh
```

## Testler

```sh
python3 tools/release_check.py
```

Diğer testler ve kapılar: [docs/RELEASE.md](docs/RELEASE.md)

## Dizinler

- `monolith/`: çevirici ve Rust çalışma zamanı
- `patches/rust/`: elle yazılmış Rust yamaları
- `tools/`: derleme ve yardımcı araçlar
- `docs/`: plan, şartname, ölçümler ve belgeler

## Lisans notu

Bu depo Minecraft ikili dosyaları ya da onlardan türetilmiş kod içermez. Minecraft EULA'sı gereği, Minecraft'ı bu depo üzerinden dağıtmayın.
