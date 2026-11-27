// net.minecraft.world.level.chunk.PalettedContainer.get(int) için elle yazılmış hızlı yol. Her blok durumu okuması
// (LevelChunk.getBlockState -> LevelChunkSection.getBlockState -> PalettedContainer.get(x,y,z) -> get(i)) buradan geçer;
// çevrilmiş kodda bu iki arayüz çağrısıdır (BitStorage.get, Palette.valueFor). S26 profilinde sunucu iş parçacığının en
// sık işlevlerinden biri.
//
// Yalnızca yaygın biçimler ele alınır: depo SimpleBitStorage ya da ZeroBitStorage, palet LinearPalette, HashMapPalette
// (CrudeIncrementalIntIdentityHashBiMap) ya da SingleValuePalette. Geri kalan her şey (GlobalPalette, başka depolar, null,
// aralık dışı indeks, paletinde olmayan kimlik: Java'nın istisna fırlattığı durumlar) çevrilmiş orijinale gider; orijinal
// aynı sonucu ya da aynı istisnayı üretir. Yan etki yok (yalnızca okuma).

#[inline(always)]
fn bits_get(st: rt::Ref, i: i32) -> Option<i32> {
    unsafe {
        if is_SimpleBitStorage(st) {
            if i < 0 || i >= get::<i32>(st, F_sbs_size) {
                return None;
            }
            let mul = get::<i32>(st, F_sbs_divide_mul) as u32 as i64;
            let add = get::<i32>(st, F_sbs_divide_add) as u32 as i64;
            let v = ((i as i64).wrapping_mul(mul).wrapping_add(add)) >> 32;
            let cell = (v >> (get::<i32>(st, F_sbs_divide_shift) & 63)) as i32;
            let data: rt::Ref = get(st, F_sbs_data);
            if data.is_null() || (cell as u32) >= (array_len(data) as u32) {
                return None;
            }
            let word = array::<i64>(data)[cell as usize];
            let sh = i.wrapping_sub(cell.wrapping_mul(get::<i32>(st, F_sbs_values_per_long))).wrapping_mul(get::<i32>(st, F_sbs_bits));
            return Some(((word >> (sh & 63)) & get::<i64>(st, F_sbs_mask)) as i32);
        }
        if is_ZeroBitStorage(st) {
            if i < 0 || i >= get::<i32>(st, F_zbs_size) {
                return None;
            }
            return Some(0);
        }
        None
    }
}

#[inline(always)]
fn palette_value(p: rt::Ref, id: i32) -> Option<rt::Ref> {
    unsafe {
        if is_LinearPalette(p) {
            if id < 0 || id >= get::<i32>(p, F_lp_size) {
                return None;
            }
            let vals: rt::Ref = get(p, F_lp_values);
            if vals.is_null() || (id as u32) >= (array_len(vals) as u32) {
                return None;
            }
            return Some(array::<rt::Ref>(vals)[id as usize]);
        }
        if is_HashMapPalette(p) {
            let m: rt::Ref = get(p, F_hp_values);
            if !is_BiMap(m) {
                return None;
            }
            let arr: rt::Ref = get(m, F_bimap_by_id);
            if arr.is_null() || id < 0 || (id as u32) >= (array_len(arr) as u32) {
                return None;
            }
            let v = array::<rt::Ref>(arr)[id as usize];
            return if v.is_null() { None } else { Some(v) };
        }
        if is_SingleValuePalette(p) {
            let v: rt::Ref = get(p, F_sv_value);
            return if v.is_null() || id != 0 { None } else { Some(v) };
        }
        None
    }
}

pub fn pc_get(orig: impl Fn(rt::Ref, i32) -> rt::Res<rt::Ref>, this: rt::Ref, i: i32) -> rt::Res<rt::Ref> {
    if this.is_null() {
        return orig(this, i);
    }
    let data: rt::Ref = unsafe { get(this, F_pc_data) };
    if data.is_null() {
        return orig(this, i);
    }
    let (st, pal): (rt::Ref, rt::Ref) = unsafe { (get(data, F_data_storage), get(data, F_data_palette)) };
    if let Some(id) = bits_get(st, i) {
        if let Some(v) = palette_value(pal, id) {
            return Ok(v);
        }
    }
    orig(this, i)
}

/// PalettedContainer.get(x, y, z): `strategy.getIndex` (Strategy'de tanımlı, iki anonim alt sınıf onu geçersiz kılmaz) +
/// `get(i)`. Yalnızca nesne tam olarak PalettedContainer ve strateji bu iki sınıftan biriyse; alt sınıf olsaydı `get(i)`
/// geçersiz kılınmış olabilirdi, o durumda orijinal yol.
pub fn pc_get3(orig: impl Fn(rt::Ref, i32, i32, i32) -> rt::Res<rt::Ref>, this: rt::Ref, x: i32, y: i32, z: i32) -> rt::Res<rt::Ref> {
    if !is_PalettedContainer(this) {
        return orig(this, x, y, z);
    }
    let s: rt::Ref = unsafe { get(this, F_pc_strategy) };
    if !is_Strategy1(s) && !is_Strategy2(s) {
        return orig(this, x, y, z);
    }
    let b = unsafe { get::<i32>(s, F_st_bits_per_axis) } & 31;
    let i = ((y << b) | z) << b | x;
    let data: rt::Ref = unsafe { get(this, F_pc_data) };
    if data.is_null() {
        return orig(this, x, y, z);
    }
    let (st, pal): (rt::Ref, rt::Ref) = unsafe { (get(data, F_data_storage), get(data, F_data_palette)) };
    if let Some(id) = bits_get(st, i) {
        if let Some(v) = palette_value(pal, id) {
            return Ok(v);
        }
    }
    orig(this, x, y, z)
}
