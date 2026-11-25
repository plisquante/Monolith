// PalettedContainer.reencodeContents: tahsisi tekrar kullanarak.
//
// Java metodu her çağrıda `new int[storage.getSize()]` açar (blok depolaması için 4096 eleman, biyom için 64), `storage.unpack`
// ile doldurur, her değeri eski paletten yeni palete çevirir ve diziyi döndürür. Çağıranlar (`PalettedContainer.pack` ve
// `PalettedContainer.unpack`) diziyi hemen `new SimpleBitStorage(bits, size, ints)` kurucusuna verir; kurucu değerleri kendi `long[]`
// dizisine kopyalar ve `ints`'i saklamaz. Yonga kaydı ve gönderimi her bölüm için bu metodu çağırdığından, 240 sn'lik profilde tüm
// tahsisin yaklaşık %8'i (yaklaşık 946 MB) bu `int[]`'tir.
//
// Bu sürüm iş parçacığı başına, boyuta göre bir dizi tutar ve her seferinde onu yeniden doldurur. Metot kendi içinde yeniden girişli
// değildir ve sonucu tüketen kod bir sonraki çağrıdan önce biter, o yüzden aynı dizinin iki çağrı arasında paylaşılması güvenlidir.
// Dizi her çağrıda tamamen yeniden yazılır (`unpack` tüm `size` elemanı doldurur); önceki çağrının değerleri sızmaz.
// Bilinmeyen bir boyut (4096 ve 64 dışında) ya da null depolama, çevrilmiş orijinale gider.

thread_local! {
    // (boyut, sabitlenmiş dizi); iki küçük yuva yeter: 4096 (blok) ve 64 (biyom)
    static SCRATCH: std::cell::Cell<[(i32, rt::Ref); 2]> = const { std::cell::Cell::new([(0, rt::Ref::NULL); 2]) };
}

fn scratch(size: i32) -> rt::Res<rt::Ref> {
    let slot = if size == 4096 { 0 } else { 1 };
    let mut s = SCRATCH.with(|c| c.get());
    if s[slot].0 == size && !s[slot].1.is_null() {
        return Ok(s[slot].1);
    }
    let arr = rt::new_array(b'I', size)?;
    mono_rt::heap::pin_ref(arr); // iş parçacığına özel yerel depolama GC köklerinde değil: dizi sabitlenmezse süpürülür
    s[slot] = (size, arr);
    SCRATCH.with(|c| c.set(s));
    Ok(arr)
}

pub fn reencode_contents(orig: impl Fn(rt::Ref, rt::Ref, rt::Ref) -> rt::Res<rt::Ref>, storage: rt::Ref, from: rt::Ref, to: rt::Ref) -> rt::Res<rt::Ref> {
    if storage.is_null() {
        return orig(storage, from, to);
    }
    let size = v_bs_size(storage)?;
    if size != 4096 && size != 64 {
        return orig(storage, from, to);
    }
    let arr = scratch(size)?;
    v_bs_unpack(storage, arr)?;
    let cb = c_no_resize()?;
    let data = unsafe { array::<i32>(arr) };
    let mut last_from = -1i32;
    let mut last_to = -1i32;
    let mut i = 0usize;
    while i < data.len() {
        let v = data[i];
        if v != last_from {
            last_from = v;
            let val = v_pal_value_for(from, v)?;
            last_to = v_pal_id_for(to, val, cb)?;
        }
        data[i] = last_to;
        i += 1;
    }
    Ok(arr)
}
