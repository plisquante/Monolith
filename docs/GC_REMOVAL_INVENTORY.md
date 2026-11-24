# GC kaldırma: envanter

Durum: yalnızca analiz. Davranış değişmedi. İzleyici (tracing) toplayıcı hâlâ `monolith/rt/gc.rs` (yaklaşık 1490 satır), `monolith/rt/heap.rs` üzerinden erişiliyor.

## Nesnelerin nerede tahsis edildiği (çalışma zamanı, GC heap giriş noktaları)

`alloc_obj`, `alloc_arena`, `alloc_recyclable`, `new_str`, `new_array`, `new_obj_array`, `alloc_zeroed` çağrı noktaları:

| dosya | nokta sayısı |
|---|---|
| `rt.rs` | 28 (nesne ve dizi oluşturma, dizge literalleri, sınıf aynaları) |
| `natives_sys.rs` | 22 (süreç, zaman, sistem özellikleri) |
| `gc.rs` | 19 (toplayıcının kendisi) |
| `natives_net.rs` | 18 |
| `natives_reflect.rs` | 15 |
| `natives_io.rs` | 14 |
| `natives.rs` | 10 |
| `natives_jni.rs` | 8 |
| `natives_rhi.rs` | 5 |
| `natives_nio.rs` | 2 |
| toplam | 141 |

Yerel metotların çoğu, dönüş değeri için bir Java `String` ya da dizi tahsis eder. Bu değerlerin sahibi çağıranıdır ve onunla birlikte ölür; sahiplik modeli için kolay durum budur. Zor durumlar, başka nesnelerin içinde ya da statiklerde saklanan nesnelerdir.

## Toplayıcının taradığı kökler

- Statik yuvalar: `register_static_slot` (rt.rs:632), referans tipli her Java statik alan için bir tane.
- Dizi sınıfları: `register_array_class` (rt.rs:724); eleman sınıfları canlı kalsın diye.
- Sabitlenmiş referanslar: `pin_ref` (sınıf statikleri için rt.rs:615; `init_failure` bildirilene kadar yaşamak zorunda olan bir gölge yığın istisnası için rt.rs:1314).
- İş parçacığı yığınları: `register_thread` ile `unregister_thread` arasında muhafazakâr biçimde taranır (gc.rs:424, 451); iş parçacığı nesneleri `set_thread_obj` ile ayarlanır (rt.rs:1024, sync.rs:259).

## Bir yerine koyma neye karar vermek zorunda

1. Java nesnelerinin sahipliği: bir alandan referans verilen ama artık hiçbir yığından referans verilmeyen bir nesneyi kim serbest bırakır?
   Seçenekler: `Arc` benzeri tutamaçlarla referans sayımı (döngüler, açıkça kırılmazsa sızar), sınıf yükleyici ömrü başına bir arena (asla serbest bırakılmaz ama program boyutuyla sınırlı), ya da Java anlambilimin adlandırdığı noktalarda açık serbest bırakmayla bir slab'e tutamaçlar (`WeakReference` temizlenmesi, `Cleaner`; `finalize` desteklenmiyor).
2. Yığın kökleri: muhafazakâr yığın taraması, Rust yerel değişkenlerinde tutulan her referans bir tutamaç tipine sahipse kalkar; üretilen kod bugün bunu yapmıyor.
3. Kaçış analizi (`--escape`), kaçmayan nesneleri çerçeve yuvalarında (`rt::stack_obj`), kaçmayan dizileri metodun arenasında tutar (`rt::new_array_arena`, `rt::new_obj_array_arena`; metot dönünce `rt::ArenaGuard` ile serbest bırakılır). İkisi de toplayıcıya ulaşmaz. Bir döngü tahsisi, bir tur boyunca yaşayıp metot boyunca yaşamıyorsa henüz arenaya konmuyor: arena metot dönene kadar yaşar, yani uzun bir döngü her turun nesnesini canlı tutardı. Bunlar önce tur başına ya da kapsam başına bir sıfırlama gerektirir.

## Ölçüm: `--escape --inline` ile `Stack1` için üretilen kod

Üretilen Rust'ta sayıldı (statik çağrı noktaları, çalışma zamanı sayıları değil):

| tahsis | nokta | yol |
|---|---|---|
| `rt::stack_obj` | 78 | çerçeve yuvası, toplayıcıya asla ulaşmaz |
| `rt::new_array_arena` | 14 | metot arenası, dönüşte serbest |
| `rt::new_obj_array_arena` | 38 | metot arenası, dönüşte serbest |
| `rt::alloc` | 4972 | toplanan heap |
| `rt::new_array` | 977 | toplanan heap |
| `rt::new_obj_array` | 930 | toplanan heap |
| `rt::alloc_recyclable` | 18 | toplanan heap, sınıf başına geri dönüştürülen |

Sayılar, üretilen `Stack1` çıktısından (`--escape --inline`, `--opt 1`, tek koşu). Daha önceki "arena üretilmiyor" notu dizilerde yanlıştı: 52 arena noktası zaten üretiliyor. Nesnelerin arena yolu yok: `new` yolu `stack_obj` ya da heap üretir, asla `alloc_arena` üretmez.

`--escape` ile bile, üretilen tahsis noktalarının yaklaşık %98'i (7027'den 6897'si) hâlâ toplanan heap'te. `--escape` altındaki üreteç mesajı 223 çerçeve ya da arena noktası bildiriyor (analiz edilen tüm metotları sayar, çıktıya ulaşmayanları da). `--inline` olmadan kapanıştaki döngü bölgesi adayları 1513 sınıflandırılmış noktadan 4 idi; `--inline` ile henüz sayılmadı. Kaçan noktalar çoğunluk; bu yüzden asıl iş, kaçan nesneler için sahiplik modelidir.

## Sonraki adımlar (başlanmadı)

- Tipik bir Minecraft büyüklüğündeki programın kaç kaçan tahsis yaptığını ölçün (`--escape` istatistikleri). `--escape` mesajı artık çerçeve ve arena noktası sayısını yazar (önceden çerçeve dışı döngü noktaları dahil her şeyi sayıyordu).
- `--inline` ile `Stack1` ve Minecraft büyüklüğündeki bir programda döngü bölgesi adaylarını (bir tur yaşayan, metot yaşamayan) sayın; tur başına sıfırlamanın değip değmeyeceğine karar vermeden önce.
- Kaçan nesneler için sahiplik modelini seçin ve `gc.rs` değişmeden önce yazılı hâle getirin.
- `heap.rs` arkasındaki toplayıcıyı bir seferde bir tahsis sınıfı için değiştirin; her biri `tests/run_tests.py` yeşilken.
