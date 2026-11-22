//! VarHandle access-mode intrinsics.
//!
//! The transpiler lowers `invokevirtual java/lang/invoke/VarHandle.<mode>(<coords>, <values>)`
//! (signature-polymorphic) to calls of these functions. T is the storage type of the variable
//! taken from the call-site descriptor; `nc` is the number of coordinates:
//!   0 = static field, 1 = instance field (obj), 2 = array element (obj, idx).
const std = @import("std");
const rt = @import("rt.zig");
const P = @import("classes.zig");

const Ref = rt.Ref;
const E = rt.E;

pub fn St(comptime T: type) type {
    return switch (T) {
        i8, i16, u16 => i32,
        else => T,
    };
}

inline fn toSt(comptime T: type, v: T) St(T) {
    return v;
}

inline fn fromSt(comptime T: type, v: St(T)) T {
    return switch (T) {
        i8 => @truncate(v),
        i16 => @truncate(v),
        u16 => @truncate(@as(u32, @bitCast(v))),
        else => v,
    };
}

/// Integer type with the same size as T, for CAS on floating point values (raw bits).
fn Bits(comptime T: type) type {
    return switch (T) {
        f32 => u32,
        f64 => u64,
        else => T,
    };
}

const VH = P.C_java_lang_invoke_VarHandle;

fn address(comptime T: type, vh_ref: Ref, obj: Ref, idx: i32, comptime nc: u8) E!*T {
    const vh = VH.cast(try rt.nn(vh_ref));
    const kind = vh.f_j2zKind;
    switch (nc) {
        0 => {
            if (kind != 1) return rt.throwKind(.UnsupportedOperation, "VarHandle coordinates mismatch (static)");
            return @ptrFromInt(@as(usize, @bitCast(vh.f_j2zOffset)));
        },
        1 => {
            if (kind != 0) return rt.throwKind(.UnsupportedOperation, "VarHandle coordinates mismatch (field)");
            const o = try rt.nn(obj);
            return @ptrFromInt(@intFromPtr(o) +% @as(usize, @bitCast(vh.f_j2zOffset)));
        },
        else => {
            if (kind != 2) return rt.throwKind(.UnsupportedOperation, "VarHandle coordinates mismatch (array)");
            const o = try rt.nn(obj);
            const a = rt.asArray(o);
            if (@as(u32, @bitCast(idx)) >= @as(u32, @bitCast(a.length))) {
                var buf: [96]u8 = undefined;
                return rt.throwKind(.ArrayIndex, std.fmt.bufPrint(&buf, "Index {d} out of bounds for length {d}", .{ idx, a.length }) catch null);
            }
            return &rt.arrayData(T, a)[@intCast(idx)];
        },
    }
}

fn storeCheck(comptime T: type, vh_ref: Ref, obj: Ref, v: T, comptime nc: u8) E!void {
    if (T != Ref or nc != 2) return;
    _ = vh_ref;
    const val = v orelse return;
    const a = (try rt.nn(obj)).class;
    if (!rt.isSubclass(val.class, a.elem.?)) return rt.throwKind(.ArrayStore, val.class.name);
}

/// byteArrayViewVarHandle access: (byte[] array, int byteIndex), unaligned, explicit byte order.
fn viewPtr(comptime T: type, vh_ref: Ref, obj: Ref, idx: i32) E!?*[@sizeOf(T)]u8 {
    const vh = VH.cast(try rt.nn(vh_ref));
    if (vh.f_j2zKind != 3) return null;
    const a = rt.asArray(try rt.nn(obj));
    if (idx < 0 or @as(i64, idx) + @sizeOf(T) > a.length) {
        var buf: [96]u8 = undefined;
        return rt.throwKind(.ArrayIndex, std.fmt.bufPrint(&buf, "Index {d} out of bounds for length {d}", .{ idx, a.length }) catch null);
    }
    return @ptrCast(rt.arrayData(u8, a) + @as(usize, @intCast(idx)));
}

fn viewBig(vh_ref: Ref) bool {
    return VH.cast(vh_ref.?).f_j2zBigEndian != 0;
}

pub fn get(comptime T: type, comptime volatile_: bool, vh: Ref, obj: Ref, idx: i32, comptime nc: u8) E!St(T) {
    if (nc == 2 and T != Ref) {
        if (try viewPtr(T, vh, obj, idx)) |bp| {
            const B = std.meta.Int(.unsigned, @bitSizeOf(T));
            const bits = std.mem.readInt(B, bp, if (viewBig(vh)) .big else .little);
            return toSt(T, @bitCast(bits));
        }
    }
    const p = try address(T, vh, obj, idx, nc);
    if (volatile_) return toSt(T, @atomicLoad(T, p, .seq_cst));
    return toSt(T, p.*);
}

pub fn set(comptime T: type, comptime volatile_: bool, vh: Ref, obj: Ref, idx: i32, comptime nc: u8, v: St(T)) E!void {
    if (nc == 2 and T != Ref) {
        if (try viewPtr(T, vh, obj, idx)) |bp| {
            const B = std.meta.Int(.unsigned, @bitSizeOf(T));
            std.mem.writeInt(B, bp, @bitCast(fromSt(T, v)), if (viewBig(vh)) .big else .little);
            return;
        }
    }
    const p = try address(T, vh, obj, idx, nc);
    const x = fromSt(T, v);
    try storeCheck(T, vh, obj, x, nc);
    if (volatile_) @atomicStore(T, p, x, .seq_cst) else p.* = x;
}

fn cmpxchg(comptime T: type, p: *T, e: T, n: T) T {
    const B = Bits(T);
    if (B == T) return @cmpxchgStrong(T, p, e, n, .seq_cst, .seq_cst) orelse e;
    const bp: *B = @ptrCast(p);
    const r = @cmpxchgStrong(B, bp, @bitCast(e), @bitCast(n), .seq_cst, .seq_cst) orelse @as(B, @bitCast(e));
    return @bitCast(r);
}

fn bitsEq(comptime T: type, a: T, b: T) bool {
    const B = Bits(T);
    if (B == T) return a == b;
    return @as(B, @bitCast(a)) == @as(B, @bitCast(b));
}

pub fn compareAndSet(comptime T: type, vh: Ref, obj: Ref, idx: i32, comptime nc: u8, e: St(T), n: St(T)) E!i32 {
    const p = try address(T, vh, obj, idx, nc);
    const ee = fromSt(T, e);
    const nn_ = fromSt(T, n);
    try storeCheck(T, vh, obj, nn_, nc);
    return @intFromBool(bitsEq(T, cmpxchg(T, p, ee, nn_), ee));
}

pub fn compareAndExchange(comptime T: type, vh: Ref, obj: Ref, idx: i32, comptime nc: u8, e: St(T), n: St(T)) E!St(T) {
    const p = try address(T, vh, obj, idx, nc);
    const nn_ = fromSt(T, n);
    try storeCheck(T, vh, obj, nn_, nc);
    return toSt(T, cmpxchg(T, p, fromSt(T, e), nn_));
}

pub fn getAndSet(comptime T: type, vh: Ref, obj: Ref, idx: i32, comptime nc: u8, n: St(T)) E!St(T) {
    const p = try address(T, vh, obj, idx, nc);
    const nn_ = fromSt(T, n);
    try storeCheck(T, vh, obj, nn_, nc);
    while (true) {
        const old = @atomicLoad(T, p, .seq_cst);
        if (bitsEq(T, cmpxchg(T, p, old, nn_), old)) return toSt(T, old);
    }
}

pub const RmwOp = enum { add, bor, band, bxor };

pub fn getAndOp(comptime T: type, comptime op: RmwOp, vh: Ref, obj: Ref, idx: i32, comptime nc: u8, d: St(T)) E!St(T) {
    const p = try address(T, vh, obj, idx, nc);
    const dv = fromSt(T, d);
    while (true) {
        const old = @atomicLoad(T, p, .seq_cst);
        const new: T = switch (op) {
            .add => if (@typeInfo(T) == .float) old + dv else old +% dv,
            .bor => if (@typeInfo(T) == .int) old | dv else unreachable,
            .band => if (@typeInfo(T) == .int) old & dv else unreachable,
            .bxor => if (@typeInfo(T) == .int) old ^ dv else unreachable,
        };
        if (bitsEq(T, cmpxchg(T, p, old, new), old)) return toSt(T, old);
    }
}

pub fn J_java_lang_invoke_VarHandle__fence0() E!void {
    var x = std.atomic.Value(u32).init(0);
    _ = x.fetchAdd(0, .seq_cst);
}

pub fn J_java_lang_invoke_MethodHandles__fieldOffset0(c: Ref, name: Ref, is_static: i32) E!i64 {
    const f = findField(c, name, is_static != 0) catch |e| return e;
    return if (f) |x| x.off else -1;
}

pub fn J_java_lang_invoke_MethodHandles__fieldIsFinal0(c: Ref, name: Ref, is_static: i32) E!i32 {
    const f = try findField(c, name, is_static != 0);
    return if (f) |x| @intFromBool(x.flags & 0x10 != 0) else 0;
}

const Found = struct { off: i64, flags: u32 };

fn findField(c: Ref, name: Ref, is_static: bool) E!?Found {
    var k: ?*rt.Class = rt.hooks.mirror_get_class(try rt.nn(c));
    const n = rt.hooks.string_chars(try rt.nn(name));
    while (k) |kk| : (k = kk.super) {
        if (is_static) {
            for (kk.static_fields) |f| if (eqlName(f.name, n)) {
                try rt.ensureInit(kk);
                return .{ .off = @bitCast(@as(u64, @intFromPtr(f.ptr))), .flags = f.flags };
            };
        } else {
            for (kk.fields) |f| if (eqlName(f.name, n)) return .{ .off = f.offset, .flags = f.flags };
        }
    }
    return null;
}

fn eqlName(a: []const u8, b: []const u16) bool {
    if (a.len != b.len) return false;
    for (a, 0..) |ch, i| if (b[i] != ch) return false;
    return true;
}

// ------------------------------------------------------------------ MemorySegment VarHandle access
fn segPtr(comptime T: type, addr: i64) E!*T {
    if (addr == 0) return rt.throwKind(.NPE, "null MemorySegment address");
    return @ptrFromInt(@as(usize, @bitCast(addr)));
}

pub fn getSeg(comptime T: type, comptime volatile_: bool, addr: i64) E!St(T) {
    if (T == Ref) return rt.throwKind(.UnsupportedOperation, "MemorySegment VarHandle object access");
    if (addr == 0) return rt.throwKind(.NPE, "null MemorySegment address");
    const B = std.meta.Int(.unsigned, @bitSizeOf(T));
    const bits: B = if (volatile_) blk: {
        const p: *B = @ptrFromInt(@as(usize, @bitCast(addr)));
        break :blk @atomicLoad(B, p, .seq_cst);
    } else blk: {
        const p: *align(1) const B = @ptrFromInt(@as(usize, @bitCast(addr)));
        break :blk p.*;
    };
    return toSt(T, @bitCast(bits));
}

pub fn setSeg(comptime T: type, comptime volatile_: bool, addr: i64, v: St(T)) E!void {
    if (T == Ref) return rt.throwKind(.UnsupportedOperation, "MemorySegment VarHandle object access");
    if (addr == 0) return rt.throwKind(.NPE, "null MemorySegment address");
    const B = std.meta.Int(.unsigned, @bitSizeOf(T));
    const bits: B = @bitCast(fromSt(T, v));
    if (volatile_) {
        const p: *B = @ptrFromInt(@as(usize, @bitCast(addr)));
        @atomicStore(B, p, bits, .seq_cst);
    } else {
        const p: *align(1) B = @ptrFromInt(@as(usize, @bitCast(addr)));
        p.* = bits;
    }
}

pub fn compareAndSetSeg(comptime T: type, addr: i64, e: St(T), n: St(T)) E!i32 {
    const p = try segPtr(T, addr);
    const ee = fromSt(T, e);
    const nn_ = fromSt(T, n);
    return @intFromBool(bitsEq(T, cmpxchg(T, p, ee, nn_), ee));
}

pub fn compareAndExchangeSeg(comptime T: type, addr: i64, e: St(T), n: St(T)) E!St(T) {
    const p = try segPtr(T, addr);
    return toSt(T, cmpxchg(T, p, fromSt(T, e), fromSt(T, n)));
}

pub fn getAndSetSeg(comptime T: type, addr: i64, n: St(T)) E!St(T) {
    const p = try segPtr(T, addr);
    const nn_ = fromSt(T, n);
    while (true) {
        const old = @atomicLoad(T, p, .seq_cst);
        if (bitsEq(T, cmpxchg(T, p, old, nn_), old)) return toSt(T, old);
    }
}

pub fn getAndOpSeg(comptime T: type, comptime op: RmwOp, addr: i64, d: St(T)) E!St(T) {
    const p = try segPtr(T, addr);
    const dv = fromSt(T, d);
    while (true) {
        const old = @atomicLoad(T, p, .seq_cst);
        const new: T = switch (op) {
            .add => if (@typeInfo(T) == .float) old + dv else old +% dv,
            .bor => if (@typeInfo(T) == .int) old | dv else unreachable,
            .band => if (@typeInfo(T) == .int) old & dv else unreachable,
            .bxor => if (@typeInfo(T) == .int) old ^ dv else unreachable,
        };
        if (bitsEq(T, cmpxchg(T, p, old, new), old)) return toSt(T, old);
    }
}
