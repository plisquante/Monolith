---
name: project-eta
description: Projeyi analiz ederek kalan işi, mevcut AI-destekli geliştirme hızını (git geçmişinden) ve final aşamasına kalan gerçekçi süreyi hesaplar. Kullanıcı "ne zaman biter", "final ne kadar kaldı", "ilerleme durumu" veya "ETA" sorduğunda kullan.
argument-hint: "[günlük çalışma saati veya hedef kapsam, örn: günde 2 saat, testler dahil]"
allowed-tools: Read, Glob, Grep, Bash(git log:*), Bash(git status:*), Bash(git rev-list:*), Bash(git diff:*)
---

# Gerçekçi AI-Destekli Proje Bitiş Tahmini (ETA)

**ASLA geleneksel insan geliştirici / ekip takvimi (adam-gün, sprint, 3-4 hafta gibi şablon süreler) kullanma.** Proje Claude Code ile geliştiriliyor; tahmin reponun gerçek üretim hızına ve kalan teknik kapsamın karmaşıklığına dayanır. Çıktı Türkçe, kısa, emoji ve em-dash yok (`caveman`).

Kullanıcının ek notu/kısıtı: `$ARGUMENTS`

## Adım 1: Gerçek hızı (velocity) ölç
- `git log --date=iso --pretty=format:"%ad | %h | %s" -n 50`
- `git log --shortstat -n 30`
- `git status --short`

Çıkar:
1. **Oturumlar:** aralarında 2 saatten fazla boşluk olan commit'ler ayrı oturum. Oturum sayısı ve toplam aktif saat.
2. **Gerçek üretim hızı:** oturum başına tamamlanan modül/özellik.
3. Git geçmişi yoksa ya da 3'ten az commit varsa bunu açıkça yaz; hız varsayımı yerine yalnızca tur/oturum sayısıyla tahmin et.

Çıktıyı ham basma: `| head`/`| tail` ile süz.

## Adım 2: Durumu ve final kapsamını tara
1. Kapsam dosyaları: `CLAUDE.md`, `docs/mimari/06_YOL_HARITASI.md` (iş paketleri, kapılar), `docs/mimari/00_ANA_MIMARI.md` (hedef E1-E7), `docs/MASTER_PLAN_TR.md`, `docs/RELEASE.md` (engeller). `docs/STATUS.md` büyük: yalnızca `grep`.
2. Eksik yerler: `TODO`, `FIXME`, `XXX`, `HACK`, `NotImplementedError`, boş fonksiyon, geçici mock. `monolith/`, `runtime/`, `classlib/` altında ara; üretilmiş çıktıyı ve 100 KB üstü dosyaları okuma.
3. Test, derleme, paketleme ve hata yönetimi durumu: `tests/run_tests.py` sonuçları, `docs/RELEASE.md` kapıları.
4. Final tanımı net değilse iki hedef ver: **MVP (çalışan çekirdek)** ve **Tam Final (test + hata ayıklama + paketleme/dokümantasyon)**. Bu repoda tam final = `docs/mimari/05_DOGRULAMA.md` Q1-Q12 kapılarının hepsi yeşil.

## Adım 3: Kalan işi AI-birimleriyle boyutlandır
- **S:** 1-3 prompt turu (~10-20 dk). Tek dosya, net bug fix.
- **M:** 4-8 tur (~30-60 dk). Çoklu dosya, yeni modül, test yazımı.
- **L:** 1-2 odaklı oturum (~1,5-3 saat). Mimari değişiklik, harici API/donanım, karmaşık state/eşzamanlılık.

**Tampon:** saf geliştirme süresine riske göre %25-%40 debug/refactor payı ekle.

**Bu repoya özel süre kalemi:** tam Minecraft derlemesi ~65-70 dk ve ~10-12 GB bellek; aynı anda yalnızca bir ağır iş. Her derleme/soak koşusunu aktif süreye değil, ayrı "bekleme süresi" olarak yaz ve yeniden derleme gereken paketlere (çalışma zamanı değişikliği, dönüşüm açma) koşu sayısını ekle. Donanım gerektiren işleri (Windows, macOS, Android cihaz) ayrı işaretle: bu ortamdan yapılamaz, kullanıcı makinesi ya da CI koşucusu gerekir.

## Adım 4: Çıktı formatı

### 1. Mevcut İlerleme Özeti
- **Tahmini Tamamlanma:** %X (hangi ölçüte göre, tek cümle; örn. kapanan iş paketi / toplam, geçen Q kapısı / 12)
- **Ölçülen Hız:** X oturum / ~Y aktif saatte tamamlanan ana bileşen sayısı

### 2. Kalan İşler Tablosu
| Öncelik | Görev / Eksik Modül | Boyut (S/M/L) | Tahmini Aktif Süre | Risk / Belirsizlik |
| :--- | :--- | :---: | :---: | :--- |

### 3. Final Aşaması Tahmini (ETA)
- **İyimser:** X aktif saat (Y oturum)
- **Gerçekçi (debug payıyla):** X aktif saat (Y oturum)
- **Kötümser (mimari/entegrasyon engeli):** X aktif saat (Y oturum)
- **Bekleme süresi (derleme, soak, cihaz):** ayrı satır
- **Takvim karşılığı:** kullanıcı günlük süre verdiyse ona göre; vermediyse "günde 2 saat aktif çalışma" varsayımıyla gün sayısı.

### 4. En Çok Uzatabilecek 2-3 Darboğaz
Belirsiz ya da en riskli teknik noktalar, madde madde. Kapsam belirsizse tahmini keskinleştirmek için en fazla 2 kısa soru sor.

## Kurallar
- Rakam uydurma: her sayının kaynağı git çıktısı, belge ya da ölçüm olmalı; kaynaksız sayıya "varsayım" yaz.
- Yalnızca raporla; kod ya da belge değiştirme.
