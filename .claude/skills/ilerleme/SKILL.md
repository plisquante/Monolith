---
name: ilerleme
description: Projenin ilerleme durumunu bir Artifact sayfasında gösterir (yüzde, iş paketleri, kalan işler). %100 = Claude'un yapabileceği iş bitti, geriye yalnızca kullanıcının yapacağı testler kaldı. Kullanıcı "ilerleme", "durum sayfası", "yüzde kaç", "ne kadar kaldı" dediğinde ya da bir iş paketi bittiğinde kullan.
argument-hint: "[not, örn: sadece güncelle]"
---

# İlerleme sayfası (Artifact)

Kullanıcı notu: `$ARGUMENTS`

Amaç: tek bir Artifact sayfası, her güncellemede aynı URL. Sayfa iki yüzde gösterir ve **%100'ün anlamını açıkça yazar**:
"%100 = Claude'un yapabileceği işin tamamı bitti; geriye yalnızca sizin yapacağınız testler kaldı."

## 1. Veri kaynağı (uydurma yok)

- İş paketleri: `docs/mimari/06_YOL_HARITASI.md` §1 tablosu (id, paket, boyut S/M/L, kabul kapısı).
- Durum: aynı dosyanın §7 "İlerleme" tablosu (`bitti` / `sürüyor` / `başladı` / satır yok = başlamadı). Sayfadan önce §7'yi gerçek
  duruma göre güncelle (commit ve test çıktısı kanıt olarak yazılı olmalı); kanıtsız paket `bitti` sayılmaz.
- Kapılar: `docs/mimari/05_DOGRULAMA.md` §3 (Q1-Q12) ve son ölçümler (`docs/STATUS.md` içinde `grep`, tamamını okuma).

## 2. Sınıflandırma

Her paketi iki gruptan birine koy:

| grup | ne | örnek |
|---|---|---|
| **Claude işi** | bu ortamda kodlanıp test edilebilen her şey: kod, birim/fark testleri, Linux derleme, headless oyun, soak | C0-C9, R1-R3, M1-M6, L1-L4, N1-N5, P4, V1, V2, D1 |
| **Kullanıcı testi** | bu ortamda yapılamayan: gerçek donanım ve cihaz, oyunu elle oynama, GitHub Actions/CI hesabı | P1 Windows, P2 macOS, P3 Android cihaz, Q12 donanım koşuları, CI düzeltmesi |

Bir paketin Claude kısmı bittiyse (kod yazıldı, bu ortamdaki testleri geçti) ama donanım testi kaldıysa: Claude işi olarak `bitti`,
donanım testi ayrı satır olarak kullanıcı testine yazılır.

## 3. Yüzde hesabı

- Ağırlık: S=1, M=2, L=4.
- Paket puanı: `bitti`=1, `sürüyor`=0,5, `başladı`=0,25, başlamadı=0.
- **Claude yüzdesi** = Σ(ağırlık × puan) / Σ ağırlık, yalnızca Claude işi paketleri üzerinden. Tam sayıya aşağı yuvarla.
  Bir paket bile `bitti` değilse sonuç %100 yazılmaz (en çok %99).
- **Kapı yüzdesi** = yeşil Q kapısı / 12 (ayrı gösterilir, Claude yüzdesine karışmaz).
- %100 olduğunda sayfanın en üstüne büyük harfle: "CLAUDE İŞİ TAMAM: kalan yalnızca sizin testleriniz" ve kullanıcı testlerinin
  listesi (ne, nasıl, hangi komut, beklenen sonuç).

## 4. Sayfa

- Önce `artifact-design` skill'ini yükle (Artifact aracının kuralı). Sade, telefon genişliğinde çalışan tek HTML sayfası.
- İçerik sırası: (1) büyük Claude yüzdesi ve ilerleme çubuğu, altında %100'ün anlamı; (2) kapı yüzdesi; (3) iş paketleri tablosu
  (id, ad, boyut, durum, kanıt); (4) kullanıcı testleri listesi; (5) son güncelleme tarihi ve commit kısa SHA'sı.
- Dosya: `<scratchpad>/ilerleme.html`. İlk yayında `icon: "chart"`. URL'yi `.claude/skills/ilerleme/URL.txt` dosyasına yaz ve commit et.
- Sonraki güncellemeler: `URL.txt` varsa önce `Artifact action: "read"` ile oku, sonra aynı `url` ile yayınla (yeni sayfa açma).

## 5. Yanıt

Tek satır: yüzde, kapı sayısı, link. %100 ise ek satır: "Benim işim bitti; kalan testler sayfada."
