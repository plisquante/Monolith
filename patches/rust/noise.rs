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

#[inline(always)]
fn lcg(a: i64, b: i64) -> i64 {
    a.wrapping_mul(a.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407)).wrapping_add(b)
}

#[inline(always)]
fn fiddle(s: i64) -> f64 {
    // Math.floorMod((s >> 24), 1024) / 1024.0, sonra (d - 0.5) * 0.9
    let d = ((s >> 24).rem_euclid(1024)) as f64 / 1024.0;
    (d - 0.5) * 0.9
}

/// BiomeManager.getFiddledDistance(long, int, int, int, double, double, double) (statik). getBiome(int,int,int) her çağrıda
/// sekiz kez çağırır; çevrilmiş halde her biri dokuz LCG.next, üç getFiddle ve üç Mth.square çağrısı. Bit düzeyinde aynı:
/// long taşmaları wrapping, double işlemleri Java'daki sırayla (toplama sırası korunur).
pub fn fiddled_distance(_orig: impl Fn(i64, i32, i32, i32, f64, f64, f64) -> rt::Res<f64>, seed: i64, x: i32, y: i32, z: i32, dx: f64, dy: f64, dz: f64) -> rt::Res<f64> {
    let mut s = lcg(seed, x as i64);
    s = lcg(s, y as i64);
    s = lcg(s, z as i64);
    s = lcg(s, x as i64);
    s = lcg(s, y as i64);
    s = lcg(s, z as i64);
    let f1 = fiddle(s);
    s = lcg(s, seed);
    let f2 = fiddle(s);
    s = lcg(s, seed);
    let f3 = fiddle(s);
    let a = dz + f3;
    let b = dy + f2;
    let c = dx + f1;
    Ok(a * a + b * b + c * c)
}

/// Climate$RTree$Node.distance(long[]): 7 boyutta Σ square(parameterSpace[i].distance(q[i])). Biyom ağacı aramasının iç
/// döngüsü (Leaf ve SubTree bunu geçersiz kılmaz). Parameter final record: alanlar doğrudan okunur. long taşmaları wrapping.
/// null / kısa dizi / null parametre orijinale gider (aynı istisna).
pub fn node_distance(orig: impl Fn(rt::Ref, rt::Ref) -> rt::Res<i64>, this: rt::Ref, q: rt::Ref) -> rt::Res<i64> {
    if this.is_null() || q.is_null() {
        return orig(this, q);
    }
    unsafe {
        let ps: rt::Ref = get(this, F_node_params);
        if ps.is_null() || array_len(ps) < 7 || array_len(q) < 7 {
            return orig(this, q);
        }
        let pa = array::<rt::Ref>(ps);
        let qa = array::<i64>(q);
        let mut sum = 0i64;
        for i in 0..7 {
            let p = pa[i];
            if !is_Param(p) {
                return orig(this, q);
            }
            let v = qa[i];
            let above = v.wrapping_sub(get::<i64>(p, F_max));
            let d = if above > 0 { above } else { get::<i64>(p, F_min).wrapping_sub(v).max(0) };
            sum = sum.wrapping_add(d.wrapping_mul(d));
        }
        Ok(sum)
    }
}
