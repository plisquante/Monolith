// Hand-written fast paths for net.minecraft.util.SimpleBitStorage (the packed block-state / biome / heightmap
// storage read by every chunk operation). Same observable behaviour as the Java methods: any argument the
// Java code would reject (null, index out of range, value out of range) goes to the translated original,
// which throws exactly the same exception.
const rt = @import("rt.zig");
const P = @import("classes.zig");
const S = P.C_net_minecraft_util_SimpleBitStorage;

inline fn cellOf(s: anytype, i: i32) i32 {
    const mul: i64 = @as(u32, @bitCast(s.f_divideMul));
    const add: i64 = @as(u32, @bitCast(s.f_divideAdd));
    const v = (@as(i64, i) *% mul +% add) >> 32;
    return @truncate(v >> @as(u6, @intCast(s.f_divideShift & 63)));
}

pub fn cellIndex(orig: anytype, this: rt.Ref, i: i32) rt.E!i32 {
    const o = this orelse return orig(this, i);
    return cellOf(S.cast(o), i);
}

pub fn get(orig: anytype, this: rt.Ref, i: i32) rt.E!i32 {
    const o = this orelse return orig(this, i);
    const s = S.cast(o);
    if (i < 0 or i >= s.f_size) return orig(this, i);
    const cell = cellOf(s, i);
    const d = s.f_data orelse return orig(this, i);
    const arr = rt.asArray(d);
    if (@as(u32, @bitCast(cell)) >= @as(u32, @bitCast(arr.length))) return orig(this, i);
    const word = rt.arrayData(i64, arr)[@intCast(cell)];
    const sh: i32 = (i -% cell *% s.f_valuesPerLong) *% s.f_bits;
    return @truncate((word >> @as(u6, @intCast(sh & 63))) & s.f_mask);
}

pub fn set(orig: anytype, this: rt.Ref, i: i32, v: i32) rt.E!void {
    const o = this orelse return orig(this, i, v);
    const s = S.cast(o);
    if (i < 0 or i >= s.f_size or v < 0 or @as(i64, v) > s.f_mask) return orig(this, i, v);
    const cell = cellOf(s, i);
    const d = s.f_data orelse return orig(this, i, v);
    const arr = rt.asArray(d);
    if (@as(u32, @bitCast(cell)) >= @as(u32, @bitCast(arr.length))) return orig(this, i, v);
    const p = &rt.arrayData(i64, arr)[@intCast(cell)];
    const sh: i32 = (i -% cell *% s.f_valuesPerLong) *% s.f_bits;
    const n: u6 = @intCast(sh & 63);
    p.* = (p.* & ~(s.f_mask << n)) | ((@as(i64, v) & s.f_mask) << n);
}
