// DataFixerUpper ısınması. `DataFixers.optimize(Set)`, açılışta her düzeltici için yeniden yazma kurallarını önceden hesaplar:
// kural sınıfı başına yaklaşık 1,4 milyon kural nesnesi (tüm oturum boyunca canlı kalan yaklaşık 0,6 GiB) ve onları kuran
// tahsisler. Kurallar yalnızca eski bir oyun sürümüyle yazılmış verileri yükseltmek için gerekir ve
// DataFixerUpper, optimize edilmemişse bunları ilk kullanımda hesaplar. Güncel bir dünya onlara hiç ihtiyaç duymaz.
//
// MONO_DATAFIX_OPTIMIZE=1 olduğunda orijinal çalışır (örn. eski dünyaların ilk yükleme maliyetini ölçmek için).

fn keep_original() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MONO_DATAFIX_OPTIMIZE").map_or(false, |v| !v.is_empty() && v != "0"))
}

pub fn optimize(orig: impl Fn(rt::Ref) -> rt::Res<rt::Ref>, types: rt::Ref) -> rt::Res<rt::Ref> {
    if keep_original() {
        return orig(types);
    }
    c_completed(rt::Ref::NULL)
}
