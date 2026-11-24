# 04. Kütüphane, kaplama ve yama mimarisi

Etiketler `00_ANA_MIMARI.md` ile aynıdır. Hedef E3 (elle yazılmış Java mantığı sıfır) ve E4 (JNI/JNA girişi sıfır).

## 1. Bugünkü kaynaklar (KOD)

| yer | içerik | boyut |
|---|---|---|
| `classlib/src` | JDK sınıflarının Java kaplamaları (`java.lang.*`, `java.nio.*`, `MonoSignals`...), `javac --patch-module` ile derlenir | 13 294 satır |
| `runtime/java_nolwjgl` | LWJGL kabukları (GLFW, OpenGL, OpenAL, stb, ...) | 243 dosya (34'ü silindi, 2026-11-24) |
| `patches/` | metot düzeyi Rust yamaları (`orig`, `verify`, parmak izi) | 10 yama, `manifest.json` |
| `android/touchcore` | Android başlatıcı (Java) | ayrı ürün |

## 2. Sınıflandırma (her dosya bir sınıfa girer)

| sınıf | tanım | hedef |
|---|---|---|
| **K1 bildirim kabuğu** | yalnızca alan, imza, `native` bildirimi (mantık yok): LWJGL sınıfları | **üretilir** (bölüm 4) |
| **K2 mantıklı kaplama** | JDK davranışını yeniden yazan Java mantığı (`Runtime`, `Thread`, `FileSystem`...) | Rust'a taşı (sınıf yaması) ya da JDK orijinaline dön ve yalnızca native'leri sağla |
| **K3 test yardımcısı** | yalnızca test | kapsam dışı |
| **K4 Android Java** | başlatıcı | AÇIK: Kotlin/Java başlatıcıyı koru (platform arayüzü zorunlu); ürün başlatıcısı ayrı |

Envanter aracı (L1): `tools/lib_inventory.py` her dosyayı K1-K4'e atar (mantık = metot gövdesinde `invoke` ya da dal), sonucu `docs/KUTUPHANE_ENVANTER.json`.
Kabul: sınıflandırılmamış dosya 0.

## 3. Hedef kaynak hattı

```
JDK jmods ─┐
libs/Minecraft jar ─┼► program (RTA) ► MIR ► Rust
classlib (K1 üretim) ─┤
Rust sınıf yamaları ─┘ (patches/classes/*.rs)
```

Java **kaynağı** hattan çıkar: JDK sınıfları jmods'tan (derlenmiş bayt kodu) gelir; değiştirilmesi gereken davranış yalnızca Rust yamasıdır.

## 4. Üretilmiş kabuk (K1)

- `tools/gen_shells.py` LWJGL jar ya da önceki kaynak listesinden `public static native` imza kabukları üretir (mantık yok).
- Çıktı Java **kaynağı olarak depoya girmez**; derlemede üretilir (`build/shells`), `javac` ile derlenip program kapanışına girer.
- Üretim girdisi: `docs/lwjgl_surface.json` (hangi sınıf ve metotların Minecraft tarafından kullanıldığı, RTA çıktısından); kullanılmayan sınıf üretilmez (silinen 34 dosyanın dersi: bayt kodu referansı varsa sınıf olmalı; link kapısı `check_link_errors.py` yakalar).
- Kabul: depoda elle yazılmış LWJGL Java dosyası 0.

## 5. Sınıf düzeyi Rust yama şeması (KARAR)

Bugünkü yama metot düzeyindedir (`monolith/patches.py`, `patches/rust/manifest.json`). Katman O (belge 02 §5) ve K2 taşıması için **sınıf düzeyi** gerekir.

`patches/classes/<sınıf>.toml` (şema sürüm 1):

```toml
class = "net/minecraft/util/BitStorage"      # sınıf adı
fingerprint = "sha256:..."                    # orijinal sınıfın alan+imza parmak izi
kind = "owned"                                # owned | rewrite | shell
fields = ["data:long[]->owned_u64", "size:int"]   # alan -> sahipli Rust türü
methods = [ "get(I)I", "set(II)V", "getRaw()[J" ]   # Rust uygulaması gereken yöntemler
rust = "patches/classes/BitStorage.rs"
verify = "tests/java/BitStorageVerify.java"   # fark testi: orijinal ve yama aynı sonucu verir
```

- `kind = owned`: belirtilen alanlar Rust sahipli depolamaya taşınır; Java yöntemleri Rust uygulamasına gider; sınıfın diğer kısımları (kalan alanlar) bayt kodundan çevrilir.
- `kind = rewrite`: tüm sınıf Rust'ta (K2 taşıması).
- `kind = shell`: K1 üretimi (alan/imza listesi).
- Doğrulama: parmak izi (`patch_fp_tests.py` 9/9, KOD) alan/imza değişince derlemeyi durdurur; `verify` testi orijinal bayt kodu yolu ile yamalı yolu karşılaştırır (`MONO_PATCH_VERIFY=1`).
- Kayıt tutarı: sınıf yamaları Minecraft sürümü başına `manifest.json` içinde; sürüm güncellenirse parmak izi kapısı durdurur (RUST_RUHU riski).

## 6. Koşullu erişilebilirlik (KARAR D23)

Amaç: ölü kodu ve ona bağlı JNI'yi kapanıştan çıkarmak (AB12: netty epoll ~150 native, 56 JNA...).
- Yama ile sabit yapılan öngörü metotları (`io.netty.channel.epoll.Epoll.isAvailable() = false`, `Native.isAvailable`...) `patches/constants.toml`.
- RTA (belge 01 §6.1): sabit katlama sonrası ölü dal içindeki çağrılar erişilebilirlik grafından çıkar; bir sınıfa **yalnızca** ölü dalla erişiliyorsa sınıf kapanıştan çıkar.
- Kabul: `MONO_COUNT_NATIVES` ve `report.json` `native-total`: taklit sayısı düşer; kaldırılan sınıfa çalışma zamanı referansı `NoClassDefFoundError` ile yüksek sesle düşer (P5).
- Risk: yanlış sabit katlama yaygın bir davranışı kapatabilir; fark testi (ağ yolu, dosya yolu) ve oyun betiği.

## 7. JNI/JNA sıfırlama planı (N işleri)

| native grubu | sayı (KOD `JNI_ENVANTER_TR.md`) | yaklaşım | sıra |
|---|---|---|---|
| anlatıcı (Flite) | 7 | platform konuşma servisi ya da kaldır (erişilebilirlik ayarı yok sayılırsa kapat) | N1 (en küçük) |
| Tracy | 21 | derleme dışı (profil sınıfları RTA'dan çıkar) | N2 |
| JNA | 56 | doğrudan Rust çağrısı (kullanılan işlevler: sistem bilgisi, kütüphane yolu) | N3 |
| netty epoll/kqueue | ~150 | koşullu erişilebilirlik (bölüm 6) ya da NIO select | N4 |
| kalan | ~25 | tek tek | N5 |

Sayaç: `MONO_COUNT_NATIVES` (açılıştan kapanışa 139 çağrı / 18 native, ÖLÇÜ). Hedef çağrı 0 ve kayıt `Taklit == 0` (belge 03 §6).

## 8. Mojang yama alanları (K2 dışı)

Elle Rust yamaları (metot düzeyi, `patches/rust`):
- çarpışma hattı (`BlockCollisions`, `VoxelShape.toAabbs`, `Shapes`),
- çizim komutları (`RenderPass$Draw`, `GpuBufferSlice`),
- chunk paleti (sınıf yaması O).
Her yama: parmak izi, `verify`, fark testi, bütçe kapısı (tahsis azalması ölçülür, belge 05).

## 9. Kabul

| ölçü | bugün | hedef |
|---|---|---|
| elle yazılmış Java mantığı (K2) | classlib 13 294 satır | 0 |
| depodaki LWJGL Java dosyası | 243 | 0 (üretilir) |
| `Taklit` native sayısı | 259 kayıtlı | 0 |
| yama parmak izi kapsamı | 10/10 | %100 (sınıf yamaları dahil) |
