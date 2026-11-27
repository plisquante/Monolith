// fastutil Long2ObjectOpenHashMap / Long2ObjectLinkedOpenHashMap get(long): Minecraft'ın chunk, bölüm ve konum haritaları
// (ChunkMap, ServerChunkCache, ışık motoru, yol bulma) bunu çok sık çağırır (S26 profilinde sunucu iş parçacığında).
// Gövde açık adresli doğrusal yoklama; Java ile aynı: anahtar 0 özel yuvada (containsNullKey, value[n]), karma
// HashCommon.mix (çarp, >>>32 xor, >>>16 xor), mask ile indeks. Dizi null ya da sınırlar tutarsızsa orijinale gider.
// Yalnızca okur: MONO_PATCH_VERIFY=1 ile orijinal de çalıştırılıp sonuç karşılaştırılır.

#[inline(always)]
fn mix(x: i64) -> i64 {
    let h = x.wrapping_mul(-7046029254386353131i64);
    let h = h ^ ((h as u64) >> 32) as i64;
    h ^ ((h as u64) >> 16) as i64
}

#[inline(always)]
unsafe fn probe(this: rt::Ref, k: i64, f_key: usize, f_value: usize, f_mask: usize, f_null: usize, f_n: usize) -> Option<rt::Ref> {
    let keys: rt::Ref = get(this, f_key);
    let vals: rt::Ref = get(this, f_value);
    if keys.is_null() || vals.is_null() {
        return None;
    }
    let (kl, vl) = (array_len(keys) as u32, array_len(vals) as u32);
    if k == 0 {
        if get::<i8>(this, f_null) != 0 {
            let n = get::<i32>(this, f_n);
            if (n as u32) >= vl {
                return None;
            }
            return Some(array::<rt::Ref>(vals)[n as usize]);
        }
        return Some(get(this, F_def_ret));
    }
    let mask = get::<i32>(this, f_mask);
    let ka = array::<i64>(keys);
    let va = array::<rt::Ref>(vals);
    let mut pos = (mix(k) as i32) & mask;
    // en çok tablo boyu kadar yokla (Java sonsuz döngüye girmez çünkü tabloda hep boş yuva vardır; burada güvence)
    for _ in 0..=kl {
        if (pos as u32) >= kl || (pos as u32) >= vl {
            return None;
        }
        let cur = ka[pos as usize];
        if cur == 0 {
            return Some(get(this, F_def_ret));
        }
        if cur == k {
            return Some(va[pos as usize]);
        }
        pos = pos.wrapping_add(1) & mask;
    }
    None
}

#[inline(always)]
fn checked(fast: rt::Ref, what: &str, orig: impl FnOnce() -> rt::Res<rt::Ref>) -> rt::Res<rt::Ref> {
    if verify() {
        match orig() {
            Ok(o) if o.0 == fast.0 => {}
            _ => mismatch(what),
        }
    }
    Ok(fast)
}

pub fn l2o_get(orig: impl Fn(rt::Ref, i64) -> rt::Res<rt::Ref>, this: rt::Ref, k: i64) -> rt::Res<rt::Ref> {
    if this.is_null() {
        return orig(this, k);
    }
    match unsafe { probe(this, k, F_h_key, F_h_value, F_h_mask, F_h_null, F_h_n) } {
        Some(v) => checked(v, "Long2ObjectOpenHashMap.get", || orig(this, k)),
        None => orig(this, k),
    }
}

pub fn l2o_linked_get(orig: impl Fn(rt::Ref, i64) -> rt::Res<rt::Ref>, this: rt::Ref, k: i64) -> rt::Res<rt::Ref> {
    if this.is_null() {
        return orig(this, k);
    }
    match unsafe { probe(this, k, F_l_key, F_l_value, F_l_mask, F_l_null, F_l_n) } {
        Some(v) => checked(v, "Long2ObjectLinkedOpenHashMap.get", || orig(this, k)),
        None => orig(this, k),
    }
}
