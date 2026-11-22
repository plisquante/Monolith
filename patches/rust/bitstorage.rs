// Hand-written fast paths for net.minecraft.util.SimpleBitStorage, the packed block-state / biome / heightmap
// storage that every chunk operation reads. Same observable behaviour as the Java methods: any argument the
// Java code would reject (null, index out of range, value out of range) goes to the translated original, which
// throws exactly the same exception.

#[inline(always)]
fn cell_of(this: rt::Ref, i: i32) -> i32 {
    unsafe {
        let mul = get::<i32>(this, F_divide_mul) as u32 as i64;
        let add = get::<i32>(this, F_divide_add) as u32 as i64;
        let v = ((i as i64).wrapping_mul(mul).wrapping_add(add)) >> 32;
        (v >> (get::<i32>(this, F_divide_shift) & 63)) as i32
    }
}

pub fn cell_index(orig: impl Fn(rt::Ref, i32) -> rt::Res<i32>, this: rt::Ref, i: i32) -> rt::Res<i32> {
    if this.is_null() {
        return orig(this, i);
    }
    Ok(cell_of(this, i))
}

pub fn get_value(orig: impl Fn(rt::Ref, i32) -> rt::Res<i32>, this: rt::Ref, i: i32) -> rt::Res<i32> {
    if this.is_null() {
        return orig(this, i);
    }
    unsafe {
        if i < 0 || i >= get::<i32>(this, F_size) {
            return orig(this, i);
        }
        let cell = cell_of(this, i);
        let data: rt::Ref = get(this, F_data);
        if data.is_null() || (cell as u32) >= (array_len(data) as u32) {
            return orig(this, i);
        }
        let word = array::<i64>(data)[cell as usize];
        let sh = i.wrapping_sub(cell.wrapping_mul(get::<i32>(this, F_values_per_long))).wrapping_mul(get::<i32>(this, F_bits));
        Ok(((word >> (sh & 63)) & get::<i64>(this, F_mask)) as i32)
    }
}

pub fn set_value(orig: impl Fn(rt::Ref, i32, i32) -> rt::Res<()>, this: rt::Ref, i: i32, v: i32) -> rt::Res<()> {
    if this.is_null() {
        return orig(this, i, v);
    }
    unsafe {
        let mask: i64 = get(this, F_mask);
        if i < 0 || i >= get::<i32>(this, F_size) || v < 0 || (v as i64) > mask {
            return orig(this, i, v);
        }
        let cell = cell_of(this, i);
        let data: rt::Ref = get(this, F_data);
        if data.is_null() || (cell as u32) >= (array_len(data) as u32) {
            return orig(this, i, v);
        }
        let p = &mut array::<i64>(data)[cell as usize];
        let sh = i.wrapping_sub(cell.wrapping_mul(get::<i32>(this, F_values_per_long))).wrapping_mul(get::<i32>(this, F_bits));
        let n = sh & 63;
        *p = (*p & !(mask << n)) | (((v as i64) & mask) << n);
        Ok(())
    }
}
