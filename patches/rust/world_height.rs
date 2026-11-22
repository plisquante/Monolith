// Hand-written fast paths for the "world height" idiom that runs on every block access:
//   ChunkAccess.getMinY -> LevelHeightAccessor.getMinY -> LevelReader.getMinY -> Level.dimensionType
//   -> Holder.value -> DimensionType.minY
// which is five calls (three of them virtual) in the translated code. Each patch handles only the common
// receiver classes (a client or server level, a resolved holder) and passes everything else (other LevelReader
// implementations such as WorldGenRegion, unbound holders, wrong types) to the translated original.

/// minY of the dimension type of `o` when `o` is exactly a client or server level with a bound holder.
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
