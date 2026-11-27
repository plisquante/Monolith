// E7: net.minecraft.client.gui.navigation.ScreenRectangle (final record) için alan okumalı hızlı yollar. GUI çiziminde
// GuiRenderState.hasIntersection her yeni öğeyi öncekilerle karşılaştırır (öğe sayısının karesi kadar intersects çağrısı;
// F3 ekranı açıkken çizim iş parçacığının %10'undan fazlası). Çevrilmiş kodda her intersects 8 sanal çağrı (left/right/
// top/bottom, her biri ScreenPosition.x()/y()) demektir. ScreenRectangle ve ScreenPosition final: alanlar doğrudan okunur.
// null alıcı ya da null konum orijinale gider (aynı NullPointerException). Toplamalar Java gibi taşar (wrapping).

#[inline(always)]
fn rect(r: rt::Ref) -> Option<(i32, i32, i32, i32)> {
    if !is_Rect(r) {
        return None;
    }
    unsafe {
        let p: rt::Ref = get(r, F_r_position);
        if !is_Pos(p) {
            return None;
        }
        let (x, y) = (get::<i32>(p, F_p_x), get::<i32>(p, F_p_y));
        Some((x, x.wrapping_add(get::<i32>(r, F_r_width)), y, y.wrapping_add(get::<i32>(r, F_r_height))))
    }
}

pub fn left(orig: impl Fn(rt::Ref) -> rt::Res<i32>, this: rt::Ref) -> rt::Res<i32> {
    match rect(this) { Some(r) => Ok(r.0), None => orig(this) }
}
pub fn right(orig: impl Fn(rt::Ref) -> rt::Res<i32>, this: rt::Ref) -> rt::Res<i32> {
    match rect(this) { Some(r) => Ok(r.1), None => orig(this) }
}
pub fn top(orig: impl Fn(rt::Ref) -> rt::Res<i32>, this: rt::Ref) -> rt::Res<i32> {
    match rect(this) { Some(r) => Ok(r.2), None => orig(this) }
}
pub fn bottom(orig: impl Fn(rt::Ref) -> rt::Res<i32>, this: rt::Ref) -> rt::Res<i32> {
    match rect(this) { Some(r) => Ok(r.3), None => orig(this) }
}

pub fn intersects(orig: impl Fn(rt::Ref, rt::Ref) -> rt::Res<i32>, this: rt::Ref, o: rt::Ref) -> rt::Res<i32> {
    match (rect(this), rect(o)) {
        // Java: left < o.right && right > o.left && top < o.bottom && bottom > o.top
        (Some(a), Some(b)) => Ok((a.0 < b.1 && a.1 > b.0 && a.2 < b.3 && a.3 > b.2) as i32),
        _ => orig(this, o),
    }
}
