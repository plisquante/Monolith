// E6: dünya üretiminin iç döngüsündeki küçük sayısal metotlar. Gövdeleri birkaç işlem; maliyet çağrının kendisi (çerçeve
// kaydı). Yamalı metot çerçevesiz çalışır. Aritmetik Java ile bit düzeyinde aynı: long çarpma/toplama taşar (wrapping),
// float işlemleri IEEE tek duyarlık ve aynı sırada (Rust kendiliğinden FMA'ya birleştirmez), int -> float `as f32`.
// null / aralık dışı / sınıfı başlatılmamış durumlar orijinale gider.

/// LinearCongruentialGenerator.next(long, long) (statik)
pub fn lcg_next(_orig: impl Fn(i64, i64) -> rt::Res<i64>, a: i64, b: i64) -> rt::Res<i64> {
    let a = a.wrapping_mul(a.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407));
    Ok(a.wrapping_add(b))
}

/// GradientNoise.permute(int): perms[i & 255] & 255
pub fn permute(orig: impl Fn(rt::Ref, i32) -> rt::Res<i32>, this: rt::Ref, i: i32) -> rt::Res<i32> {
    if this.is_null() {
        return orig(this, i);
    }
    unsafe {
        let p: rt::Ref = get(this, F_perms);
        let k = (i & 255) as usize;
        if p.is_null() || k >= array_len(p) as usize {
            return orig(this, i);
        }
        Ok((array::<i8>(p)[k] as i32) & 255)
    }
}

/// GradientNoise.gradDot(int, float, float, float) (statik): GRADIENT[h & 15].dot(x, y, z)
pub fn grad_dot(orig: impl Fn(i32, f32, f32, f32) -> rt::Res<f32>, h: i32, x: f32, y: f32, z: f32) -> rt::Res<f32> {
    unsafe {
        let g: rt::Ref = *s_gradient();
        let k = (h & 15) as usize;
        if g.is_null() || k >= array_len(g) as usize {
            return orig(h, x, y, z);
        }
        let v = array::<rt::Ref>(g)[k];
        if !is_Gradient(v) {
            return orig(h, x, y, z);
        }
        let (gx, gy, gz) = (get::<i32>(v, F_g_x) as f32, get::<i32>(v, F_g_y) as f32, get::<i32>(v, F_g_z) as f32);
        Ok(gx * x + gy * y + gz * z)
    }
}

/// Climate$Parameter.distance(long): max'ın üstündeyse fark, değilse max(min - v, 0)
pub fn param_distance(orig: impl Fn(rt::Ref, i64) -> rt::Res<i64>, this: rt::Ref, v: i64) -> rt::Res<i64> {
    if !is_Param(this) {
        return orig(this, v);
    }
    unsafe {
        let above = v.wrapping_sub(get::<i64>(this, F_max));
        if above > 0 {
            return Ok(above);
        }
        Ok(get::<i64>(this, F_min).wrapping_sub(v).max(0))
    }
}
