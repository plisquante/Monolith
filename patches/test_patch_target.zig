// Hand-written replacements used by tests/java/PatchTarget (class does not exist in Minecraft builds).
const rt = @import("rt.zig");

pub fn mix(a: i32, b: i32) rt.E!i32 {
    return ((a *% 31) +% b) ^ @as(i32, @bitCast(@as(u32, @bitCast(a)) >> 3));
}

pub fn scale(x: f64, f: f32) rt.E!f64 {
    return x * @as(f64, f) + 0.5;
}

pub fn sum(this: rt.Ref, k: i64) rt.E!i64 {
    _ = this;
    return k + 5;
}
