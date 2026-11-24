# WS7 envanter: Java kaplamaları (2026-11-24)

Yöntem: Minecraft 26.3 + 66 kütüphane `monolith.rustgen --no-lwjgl` ile çevrildi (`MONO_VERBOSE=1`); üretilen shard'larda her
`CLS_<sınıf>` sembolü sayıldı (8 820 JDK/LWJGL sınıfı). Bir kaplama dosyası "kapalı dünyada" sayılır, eğer kendisi ya da bir iç sınıfı
üretilen kodda geçiyorsa. Silme adayı = kapalı dünyada olmayan dosya, ve `javac` (`monolith/classpath.py` ile aynı çağrı) geriye
kalan ağacı derliyorsa silinir; derlemeyi bozan dosya geri konur (kalan dosyalar sınıf imzalarında başvuruyor).

## Sonuç

| | önce | sonra | not |
|---|---|---|---|
| `runtime/java_nolwjgl` | 277 dosya | **243** | 34 kullanılmayan dosya silindi (35'i silinmişti, biri geri alındı, aşağıya bak); ~6 400 satır |
| `classlib/src` | 56 dosya | 56 | 55'i kapalı dünyada; 13 257 satır (`String`, `Thread`, `Class`, ...). Asıl iş burada |

Silinenler: `glfw/*` (11, Minecraft 26.3 pencereyi SDL3 ile açıyor; kapalı dünyada GLFW sınıfı hiç yok), `opengl/GL11, GL15, GL20,
GL30..GL33, EXTTextureFilterAnisotropic` (yalnız sabit kapları, javac sabitleri satır içi koymuş), `openal` EXT/SOFT eklentileri (6),
`vulkan` çoklu çizim ve taşınabilirlik eklentileri (8), `util/spvc/Spv`.

## Bir hata ve dersi: `SDL_LogOutputFunction` geri alındı

`SDL_LogOutputFunction` kapalı dünyada sembol olarak görünmüyordu ve kalan Java kaynakları ona başvurmuyordu, bu yüzden `javac`
silmeyi onayladı. Silmeden sonraki tam çeviride `tools/check_link_errors.py` bir **açıklanmamış** link error verdi:
`com/mojang/blaze3d/platform/SdlDebug.<clinit>()V: class org/lwjgl/sdl/SDL_LogOutputFunction not found`. Minecraft'ın kendi bayt kodu bu
sınıfa başvuruyordu (statik başlatıcıda), yani oyunda `SdlDebug` sınıf başlatması hata verirdi. Dosya geri konuldu.
Ders: "sembol yok" ve "javac geçiyor" yetmez; **silme ancak çeviri sonrası link error kapısı 0 açıklanmamış verince geçerlidir.**
Kalan 34 silme için aynı kapı yalnızca bu bir hatayı gösterdi (başka açıklanmamış link error yok).

## Kalan 243 dosya

| paket | dosya | kapalı dünyada | not |
|---|---|---|---|
| `vulkan` | 128 | 124 | Minecraft'ın Vulkan arka ucu sınıfları bunlara bağlanıyor (kullanılmasa da link için gerekli). Kaldırmak ancak oyun Vulkan sınıflarını hiç yüklemezse mümkün |
| `sdl` | 38 | 18 | 20'si kapalı dünyada değil ama başka dosyaların imzasında ya da Minecraft'ın bayt kodunda (`SDL_LogOutputFunction`) başvuruluyor |
| `opengl` | 23 | 23 | |
| `system` | 17 | 17 | |
| `util` | 20 | 15 | freetype, shaderc, spvc, vma |
| `openal` | 10 | 10 | |
| `stb`, `BufferUtils`, `PointerBuffer`, `Version` | 7 | 7 | |

MASTER_PLAN'daki "kullanılmayan Vulkan taslakları silinir" varsayımı yalnızca kısmen doğru: 136 Vulkan dosyasının 124'ü
kapalı dünyada, çünkü oyunun Vulkan sınıfları var ve bunlara başvuruyor.

## Doğrulama durumu

- Yapıldı: `javac` ile kalan ağaç derleniyor, dışarıdan başvuru taraması boş, **silmeden sonra tam çeviri ve link error kapısı**
  (447 link error, bir açıklanmamış: `SDL_LogOutputFunction`, geri alındı; 446 bekleniyor).
- Yapıldı (geri almadan sonra yeniden): 61 747 sınıf, 446 link error, **0 açıklanmamış**, 0 eksik native, 10 yama uygulandı
  (parmak izleri kabul edildi).
- **Yapılmadı:** oyun açılışı (build sürüyor).

## Sonraki adım

`classlib/src` (13 k satır) asıl taşınacak parça: dosya başına karar (Rust native, sınıf seviyesi yama, ya da JDK'nın kendi sınıfı).
Bu, WS6'nın sınıf seviyesi yamalarına bağlıdır.
