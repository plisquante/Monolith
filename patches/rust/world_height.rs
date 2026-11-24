// Her blok erişiminde çalışan "dünya yüksekliği" kalıbı için elle yazılmış hızlı yollar:
//   ChunkAccess.getMinY -> LevelHeightAccessor.getMinY -> LevelReader.getMinY -> Level.dimensionType
//   -> Holder.value -> DimensionType.minY
// bu, çevrilmiş kodda beş çağrıdır (üçü sanal). Her yama yalnızca yaygın
// alıcı sınıfları (bir istemci ya da sunucu seviyesi, çözülmüş bir tutucu) ele alır ve geri kalan her şeyi (WorldGenRegion gibi diğer LevelReader
// uygulamaları, bağlanmamış tutucular, yanlış tipler) çevrilmiş orijinale geçirir.

/// `o` tam olarak bağlı bir tutucuya sahip bir istemci ya da sunucu seviyesiyse, `o`'nun boyut tipinin minY değeri.
#[inline(always)]
fn level_min_y(o: rt::Ref) -> Option<i32> {
    if !is_ClientLevel(o) && !is_ServerLevel(o) {
        return None;
    }
    unsafe {
        let h: rt::Ref = get(o, F_level_dim);
        if !is_Holder(h) {
            return None;
        }
        let v: rt::Ref = get(h, F_holder_value);
        if !is_DimType(v) {
            return None;
        }
        Some(get::<i32>(v, F_dim_min_y))
    }
}

pub fn holder_value(orig: impl Fn(rt::Ref) -> rt::Res<rt::Ref>, this: rt::Ref) -> rt::Res<rt::Ref> {
    if this.is_null() {
        return orig(this);
    }
    let v: rt::Ref = unsafe { get(this, F_holder_value) };
    if v.is_null() { orig(this) } else { Ok(v) }
}

pub fn level_dimension_type(orig: impl Fn(rt::Ref) -> rt::Res<rt::Ref>, this: rt::Ref) -> rt::Res<rt::Ref> {
    if this.is_null() {
        return orig(this);
    }
    unsafe {
        let h: rt::Ref = get(this, F_level_dim);
        if is_Holder(h) {
            let v: rt::Ref = get(h, F_holder_value);
            if is_DimType(v) {
                return Ok(v);
            }
        }
    }
    orig(this)
}

pub fn reader_min_y(orig: impl Fn(rt::Ref) -> rt::Res<i32>, this: rt::Ref) -> rt::Res<i32> {
    if this.is_null() {
        return orig(this);
    }
    match level_min_y(this) { Some(y) => Ok(y), None => orig(this) }
}

pub fn chunk_min_y(orig: impl Fn(rt::Ref) -> rt::Res<i32>, this: rt::Ref) -> rt::Res<i32> {
    if this.is_null() {
        return orig(this);
    }
    let a: rt::Ref = unsafe { get(this, F_chunk_accessor) };
    if a.is_null() {
        return orig(this);
    }
    match level_min_y(a) { Some(y) => Ok(y), None => orig(this) }
}
