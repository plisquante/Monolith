// Rust patches for Patch1.java. Each function must behave exactly like the Java method it replaces.

pub fn mix(a: i32, b: i32) -> rt::Res<i32> {
    Ok(a.wrapping_mul(31) ^ (((b as u32) >> 3) as i32))
}

pub fn scale(d: f64, f: f32) -> rt::Res<f64> {
    Ok(d * (f as f64) + 0.5)
}

pub fn sum(n: i64) -> rt::Res<i64> {
    let (mut s, mut i) = (0i64, 0i64);
    while i < n {
        s = s.wrapping_add(i.wrapping_mul(i));
        i += 1;
    }
    Ok(s)
}

pub fn nan() -> rt::Res<f64> {
    Ok(f64::NAN)
}

pub fn bits_get(orig: impl Fn(rt::Ref, i32) -> rt::Res<i32>, this: rt::Ref, i: i32) -> rt::Res<i32> {
    if !is_Bits(this) {
        return orig(this, i);
    }
    unsafe {
        let size: i32 = get(this, F_size);
        let data: rt::Ref = get(this, F_data);
        if i < 0 || i >= size || data.is_null() {
            return orig(this, i);
        }
        let words: &mut [i64] = array(data);
        Ok(((words[(i >> 6) as usize] >> (i & 63)) & 1) as i32)
    }
}

pub fn bits_set(orig: impl Fn(rt::Ref, i32, i32) -> rt::Res<()>, this: rt::Ref, i: i32, v: i32) -> rt::Res<()> {
    if !is_Bits(this) {
        return orig(this, i, v);
    }
    unsafe {
        let size: i32 = get(this, F_size);
        let data: rt::Ref = get(this, F_data);
        if i < 0 || i >= size || (v & !1) != 0 || data.is_null() {
            return orig(this, i, v);
        }
        let words: &mut [i64] = array(data);
        let m = 1i64 << (i & 63);
        let w = &mut words[(i >> 6) as usize];
        if v == 0 { *w &= !m } else { *w |= m }
        Ok(())
    }
}

/// `a` is declared in Base, `b` in Derived: both offsets are resolved against Derived's layout.
pub fn derived_total(this: rt::Ref) -> rt::Res<i32> {
    rt::nn(this)?;
    unsafe { Ok(get::<i32>(this, F_a).wrapping_add(get::<i32>(this, F_b))) }
}

pub fn reg_bump(x: i32) -> rt::Res<i32> {
    unsafe { *s_count() += 1; }
    let _ = c_helper(x)?; // Reg.helper is reached only through this binding
    Ok(x)
}

pub fn acc_add(this: rt::Ref, a: i32) -> rt::Res<i32> {
    rt::nn(this)?;
    unsafe {
        let t = get::<i32>(this, F_total).wrapping_add(a);
        set::<i32>(this, F_total, t);
        Ok(t)
    }
}

pub fn shapes_area(orig: impl Fn(rt::Ref) -> rt::Res<i32>, s: rt::Ref) -> rt::Res<i32> {
    unsafe {
        if is_Sq(s) { let v: i32 = get(s, F_sq_s); return Ok(v.wrapping_mul(v)); }
        if is_Rect(s) { return Ok(get::<i32>(s, F_rect_w).wrapping_mul(get::<i32>(s, F_rect_h))); }
    }
    orig(s)
}
