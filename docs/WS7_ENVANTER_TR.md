# WS7 envanter: Java kaplamaları (2026-11-24)

Yöntem: Minecraft 26.3 + 66 kütüphane `monolith.rustgen --no-lwjgl` ile çevrildi (`MONO_VERBOSE=1`); üretilen shard'larda her
`CLS_<sınıf>` sembolü sayıldı (8 820 JDK/LWJGL sınıfı). Bir kaplama dosyası "kapalı dünyada" sayılır, eğer kendisi ya da bir iç sınıfı
üretilen kodda geçiyorsa. Silme adayı = kapalı dünyada olmayan dosya, ve `javac` (`monolith/classpath.py` ile aynı çağrı) geriye
kalan ağacı derliyorsa silinir; derlemeyi bozan dosya geri konur (kalan dosyalar sınıf imzalarında başvuruyor).

## Sonuç

| | önce | sonra | not |
|---|---|---|---|
| `runtime/java_nolwjgl` | 277 dosya | **242** | 35 kullanılmayan dosya silindi; ~6 400 satır |
| `classlib/src` | 56 dosya | 56 | 55'i kapalı dünyada; 13 257 satır (`String`, `Thread`, `Class`, ...). Asıl iş burada |

Silinenler: `glfw/*` (11, Minecraft 26.3 pencereyi SDL3 ile açıyor; kapalı dünyada GLFW sınıfı hiç yok), `opengl/GL11, GL15, GL20,
GL30..GL33, EXTTextureFilterAnisotropic` (yalnız sabit kapları, javac sabitleri satır içi koymuş), `openal` EXT/SOFT eklentileri (6),
`vulkan` çoklu çizim ve taşınabilirlik eklentileri (8), `sdl/SDL_LogOutputFunction`, `util/spvc/Spv`.

## Kalan 242 dosya

| paket | dosya | kapalı dünyada | not |
|---|---|---|---|
| `vulkan` | 128 | 124 | Minecraft'ın Vulkan arka ucu sınıfları bunlara bağlanıyor (kullanılmasa da link için gerekli). Kaldırmak ancak oyun Vulkan sınıflarını hiç yüklemezse mümkün |
| `sdl` | 37 | 18 | 19'u kapalı dünyada değil ama başka dosyaların imzasında |
| `opengl` | 23 | 23 | |
| `system` | 17 | 17 | |
| `util` | 20 | 15 | freetype, shaderc, spvc, vma |
| `openal` | 10 | 10 | |
| `stb`, `BufferUtils`, `PointerBuffer`, `Version` | 7 | 7 | |

MASTER_PLAN'daki "kullanılmayan Vulkan taslakları silinir" varsayımı yalnızca kısmen doğru: 136 Vulkan dosyasının 124'ü
kapalı dünyada, çünkü oyunun Vulkan sınıfları var ve bunlara başvuruyor.

## Doğrulama durumu

- Yapıldı: `javac` ile kalan ağaç derleniyor (63 adaydan 35'inin silinmesi güvenli bulundu, 28'i imzalarda başvurulduğu için kaldı), dışarıdan başvuru taraması boş.
- **Yapılmadı (test aşamasında):** silmeden sonra tam transpile ve `tools/check_link_errors.py` ile link error kümesinin değişmediğinin
  kontrolü, ardından oyun açılışı. Beklenen: 446 link error, 0 açıklanmamış.

## Sonraki adım

`classlib/src` (13 k satır) asıl taşınacak parça: dosya başına karar (Rust native, sınıf seviyesi yama, ya da JDK'nın kendi sınıfı).
Bu, WS6'nın sınıf seviyesi yamalarına bağlıdır.
