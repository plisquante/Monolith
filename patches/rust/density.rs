// E6 (ilk adım): net.minecraft.world.level.levelgen.densityfunction.DensityBuffer.get/set. Dünya üretiminin iç döngüsünde
// en sık çağrılan iki metot (S26 profilinde işçi iş parçacıklarının ilk sıralarında). Gövdeleri tek bir dizi erişimi; maliyet
// çağrının kendisi (çerçeve kaydı, satır işaretleri). Yamalı metot çerçevesiz çalışır. null dizi ya da aralık dışı indeks
// orijinale gider (aynı NullPointerException / ArrayIndexOutOfBoundsException).

pub fn get_value(orig: impl Fn(rt::Ref, i32) -> rt::Res<f32>, this: rt::Ref, i: i32) -> rt::Res<f32> {
    if this.is_null() {
        return orig(this, i);
    }
    unsafe {
        let a: rt::Ref = get(this, F_values);
        if a.is_null() || (i as u32) >= (array_len(a) as u32) {
            return orig(this, i);
        }
        Ok(array::<f32>(a)[i as usize])
    }
}

pub fn set_value(orig: impl Fn(rt::Ref, i32, f32) -> rt::Res<()>, this: rt::Ref, i: i32, v: f32) -> rt::Res<()> {
    if this.is_null() {
        return orig(this, i, v);
    }
    unsafe {
        let a: rt::Ref = get(this, F_values);
        if a.is_null() || (i as u32) >= (array_len(a) as u32) {
            return orig(this, i, v);
        }
        array::<f32>(a)[i as usize] = v;
        Ok(())
    }
}
