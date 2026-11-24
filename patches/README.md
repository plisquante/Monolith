# Yamalar: sıcak metotlar için el yazımı Rust (Rust arka ucu)

Rust arka ucu `patches/rust/manifest.json` dosyasını okur (uygulama: `monolith/patches.py`). Mojang'ın kodundan burada hiçbir şey saklanmaz: bir yama, bir Rust fonksiyonu ve değiştirdiği metodu adlandıran (`class`, `method`, `desc`) bir manifest girdisidir. Çevirici orijinali yine çevirir (`r_<ad>_orig` olarak kalır) ve yerine bir sarmalayıcı üretir. Oyun mantığı böyle, metot metot Rust'a taşınır: orijinali artık çağrılmayan değiştirilmiş bir metot derlemeden çıkar.

## manifest.json

    {
      "files": {
        "bitstorage.rs": {
          "fields":  {"size": "net/minecraft/util/SimpleBitStorage.size"},   // F_size: usize, bayt ofseti
          "statics": {"count": "pkg/Cls.count"},                              // s_count() -> *mut T
          "classes": {"Level": "net/minecraft/world/level/Level"},            // class_Level(), is_Level(o), init_Level()
          "calls":   {"helper": "pkg/Cls.helper(I)I"}                         // c_helper(args) -> rt::Res<R>
        }
      },
      "patches": [
        {"class": "pkg/Cls", "method": "get", "desc": "(I)I", "file": "bitstorage.rs", "fn": "get_value", "orig": true}
      ]
    }

Yama dosyası, yamalanan sınıfın yonga (shard) içinde, üretilen bağlamalardan sonra bir modüle yapıştırılır; bu yüzden `rt::` (çalışma zamanı), `mono_rt::patch::*` (`get`, `set`, `array`, `array_len`, `class_ptr`) ve yukarıdaki bağlamaları kullanabilir.
Bir üst sınıfta bildirilmiş bir alan, bağlamada adı geçen alt sınıf üzerinden bulunur. `calls` doğrudan (sanal olmayan) çağrılardır; çağrılan metot, başka hiçbir şey ona ulaşmasa bile derlenir ve statik bir çağrılanın sınıfı önce başlatılır.

## Yama fonksiyonları için kurallar

* Üretilen metodun imzası: `pub fn f(this: rt::Ref, a0: i32, ...) -> rt::Res<R>` (statik metotlarda `this` yok). `Err` döndürmek için `rt::throw(...)` kullanın ya da çağrıyı orijinale devredin.
* `"orig": true`: ilk parametre çevrilmiş orijinaldir (`orig: impl Fn(rt::Ref, i32) -> rt::Res<i32>`). Yalnızca yaygın durumu ele alın; Java kodunun reddedeceği her girdiyi (null, aralık dışı indeks, ...) `orig`'e devredin; o da Java kodunun tam olarak fırlattığını fırlatır. Bkz. `patches/rust/bitstorage.rs`.
* Java metoduyla aynı gözlemlenebilir davranış, istisnalar dahil.
* Yerel değişkenlerde tutulan referansları toplayıcı bulur (yığınları muhafazakâr biçimde tarar): kaydedilecek bir çerçeve yoktur.
* `MONO_PATCH_VERIFY=1`, ilkel değer döndüren bir metodun her çağrısında orijinali ve yamayı çalıştırır ve ilk farkta (ham bitler olarak karşılaştırılır) durur. Yan etkisi olan metotlar `"verify": false` yazmalıdır (ikisi de çalışırdı).
  Referans ya da `void` döndüren metotlar fark testleriyle kapsanır: `python3 monolith/patch_tests.py` ve `monolith/tests/patch/`.
* Sınıfı sınıf yolunda olmayan bir yama atlanır (üretici kaç yamanın uygulandığını, kaçının kullanılmadığını ya da atlandığını yazar). Sınıf var ama metot yoksa derleme durur; böylece bir Minecraft güncellemesi yamayı sessizce devre dışı bırakamaz. `MONO_PATCH_MANIFEST=<dosya>` başka bir manifest seçer (testler).
* `"fingerprint"`: yamanın yazıldığı metodun bytecode'unu tanımlayan 16 hanelik onaltılık sayı (çözülmüş komutlar, istisna tablosu, çerçeve boyutları; sabit havuzu numaralarından ve satır numaralarından bağımsız). Minecraft o gövdeyi değiştirdiğinde derleme, iki değeri de adlandıran bir mesajla durur. Yamayı yeni bytecode'a göre gözden geçirdikten sonra yeni değeri şöyle kaydedin: `python3 tools/patch_fingerprints.py --cp <client.jar:libs...> --write`. `--write` olmadan araç yalnızca raporlar (`ok`, `MISMATCH`, `no value`, `skipped`). Manifestteki tüm yamalarda bir parmak izi var. Testler: `python3 -m monolith.patch_fp_tests`.
