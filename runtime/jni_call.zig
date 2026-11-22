//! JNI method and field access: GetMethodID/GetFieldID (+Static), Call<Type>Method{,V,A}
//! (virtual, nonvirtual, static), NewObject*, AllocObject, Get/Set<Type>Field (+Static),
//! FromReflected*. Built on the class metadata (rt.Class.methods/fields/static_fields) and the
//! generated reflective invokers (classes.zig reflectInvoke + box/unbox helpers).
//!
//! A method ID points to {declaring class, method slot}. Only methods the build kept (reachable
//! in the closed world, or listed by --reflect / reflection folding) can be called: a missing one
//! raises NoSuchMethodError at GetMethodID time, never a silent no-op.
const std = @import("std");
const builtin = @import("builtin");
const rt = @import("rt.zig");
const P = @import("classes.zig");
const J = @import("jni.zig");
const N = @import("natives.zig");

const Ref = rt.Ref;
const jobject = J.jobject;
const EnvPtr = J.EnvPtr;

pub const jvalue = extern union { z: u8, b: i8, c: u16, s: i16, i: i32, j: i64, f: f32, d: f64, l: ?*anyopaque };

const MID = struct { decl: *rt.Class, slot: u32 };
const FID = struct { is_static: bool, offset: usize, ptr: ?*anyopaque, kind: u8 };

fn alloc(comptime T: type, v: T) ?*T {
    const p = std.heap.c_allocator.create(T) catch return null;
    p.* = v;
    return p;
}

fn klass(c: jobject) *rt.Class {
    return rt.hooks.mirror_get_class(J.obj(c).?);
}

fn throwNoSuch(comptime what: []const u8, name: []const u8) void {
    var buf: [512]u8 = undefined;
    const msg = std.fmt.bufPrint(&buf, "{s}", .{name}) catch what;
    J.running();
    defer J.native();
    rt.throwKind(if (std.mem.eql(u8, what, "method")) .NoSuchMethod else .NoSuchField, msg) catch {};
}

// ------------------------------------------------------------------ IDs
fn findMethodIn(c: *rt.Class, name: []const u8, desc: []const u8) ?MID {
    for (c.methods, 0..) |m, i| {
        if (std.mem.eql(u8, m.name, name) and std.mem.eql(u8, m.desc, desc)) return .{ .decl = c, .slot = @intCast(i) };
    }
    return null;
}

fn findMethod(c: *rt.Class, name: []const u8, desc: []const u8, want_static: bool) ?MID {
    var k: ?*rt.Class = c;
    while (k) |kk| : (k = kk.super) {
        if (findMethodIn(kk, name, desc)) |m| {
            const is_static = kk.methods[m.slot].flags & 0x8 != 0;
            if (is_static == want_static) return m;
        }
    }
    var k2: ?*rt.Class = c;
    while (k2) |kk| : (k2 = kk.super) {
        for (kk.interfaces) |i| if (findMethodIn(i, name, desc)) |m| {
            const is_static = i.methods[m.slot].flags & 0x8 != 0;
            if (is_static == want_static) return m;
        };
    }
    return null;
}

fn getMethodID(comptime is_static: bool) fn (EnvPtr, jobject, [*:0]const u8, [*:0]const u8) callconv(.c) ?*anyopaque {
    return struct {
        fn f(e: EnvPtr, c: jobject, name: [*:0]const u8, sig: [*:0]const u8) callconv(.c) ?*anyopaque {
            _ = e;
            const k = klass(c);
            if (!is_static) {
                J.running();
                defer J.native();
                rt.ensureInit(k) catch return null;
            } else {
                J.running();
                defer J.native();
                rt.ensureInit(k) catch return null;
            }
            const m = findMethod(k, std.mem.span(name), std.mem.span(sig), is_static) orelse {
                throwNoSuch("method", std.mem.span(name));
                return null;
            };
            if (m.decl.methods[m.slot].fp == null and m.decl.methods[m.slot].flags & 0x400 == 0) {
                throwNoSuch("method", std.mem.span(name)); // pruned: not reachable in the closed world
                return null;
            }
            return @ptrCast(alloc(MID, m));
        }
    }.f;
}

fn kindOfDesc(d: []const u8) u8 {
    return switch (d[0]) {
        '[' => 'L',
        else => d[0],
    };
}

fn getFieldID(comptime is_static: bool) fn (EnvPtr, jobject, [*:0]const u8, [*:0]const u8) callconv(.c) ?*anyopaque {
    return struct {
        fn f(e: EnvPtr, c: jobject, name: [*:0]const u8, sig: [*:0]const u8) callconv(.c) ?*anyopaque {
            _ = e;
            const n = std.mem.span(name);
            const s = std.mem.span(sig);
            const k = klass(c);
            var kk: ?*rt.Class = k;
            if (is_static) {
                J.running();
                defer J.native();
                rt.ensureInit(k) catch return null;
            }
            while (kk) |x| : (kk = x.super) {
                if (is_static) {
                    for (x.static_fields) |sf| {
                        if (std.mem.eql(u8, sf.name, n) and std.mem.eql(u8, sf.desc, s))
                            return @ptrCast(alloc(FID, .{ .is_static = true, .offset = 0, .ptr = sf.ptr, .kind = kindOfDesc(s) }));
                    }
                } else {
                    for (x.fields) |ff| {
                        if (std.mem.eql(u8, ff.name, n) and std.mem.eql(u8, ff.desc, s))
                            return @ptrCast(alloc(FID, .{ .is_static = false, .offset = ff.offset, .ptr = null, .kind = kindOfDesc(s) }));
                    }
                }
            }
            if (is_static) { // static fields of interfaces
                var k2: ?*rt.Class = k;
                while (k2) |x| : (k2 = x.super) for (x.interfaces) |i| for (i.static_fields) |sf| {
                    if (std.mem.eql(u8, sf.name, n) and std.mem.eql(u8, sf.desc, s))
                        return @ptrCast(alloc(FID, .{ .is_static = true, .offset = 0, .ptr = sf.ptr, .kind = kindOfDesc(s) }));
                };
            }
            throwNoSuch("field", n);
            return null;
        }
    }.f;
}

// ------------------------------------------------------------------ field access
fn CT(comptime k: u8) type {
    return switch (k) {
        'L' => ?*anyopaque,
        'Z' => u8,
        'B' => i8,
        'C' => u16,
        'S' => i16,
        'I' => i32,
        'J' => i64,
        'F' => f32,
        'D' => f64,
        'V' => void,
        else => @compileError("bad kind"),
    };
}
fn ST(comptime k: u8) type { // storage type in object layouts / statics
    return switch (k) {
        'L' => Ref,
        'Z' => i8,
        else => CT(k),
    };
}

fn fieldPtr(comptime k: u8, f: *FID, o: jobject) *ST(k) {
    if (f.is_static) return @ptrCast(@alignCast(f.ptr.?));
    const base: [*]u8 = @ptrCast(J.obj(o).?);
    return @ptrCast(@alignCast(base + f.offset));
}

fn getField(comptime k: u8, comptime is_static: bool) type {
    return struct {
        fn f(e: EnvPtr, o: jobject, id: ?*anyopaque) callconv(.c) CT(k) {
            _ = e;
            const p = fieldPtr(k, @ptrCast(@alignCast(id.?)), if (is_static) null else o);
            if (k == 'L') {
                J.running();
                defer J.native();
                return J.local(p.*);
            }
            if (k == 'Z') return @bitCast(p.*);
            return p.*;
        }
    };
}

fn setField(comptime k: u8, comptime is_static: bool) type {
    return struct {
        fn f(e: EnvPtr, o: jobject, id: ?*anyopaque, v: CT(k)) callconv(.c) void {
            _ = e;
            const p = fieldPtr(k, @ptrCast(@alignCast(id.?)), if (is_static) null else o);
            if (k == 'L') {
                p.* = J.obj(v);
            } else if (k == 'Z') {
                p.* = @bitCast(v);
            } else {
                p.* = v;
            }
        }
    };
}

// ------------------------------------------------------------------ calls
const use_c_va = (builtin.cpu.arch == .aarch64 or builtin.cpu.arch == .aarch64_be) and builtin.os.tag == .linux and builtin.zig_backend == .stage2_llvm;

extern fn mono_va_next(vp: *anyopaque, d: u8, out: *jvalue) callconv(.c) void;
extern fn mono_jni_set_va_cbs(dc: *const anyopaque, no: *const anyopaque) callconv(.c) void;

/// Reads one argument of descriptor type `d` from a va_list (C promotions: small ints are int,
/// float is double). callconv(.c) because Zig only allows @cVaArg in C-ABI functions.
fn nextVa(ap: *std.builtin.VaList, d: u8) callconv(.c) jvalue {
    var r: jvalue = .{ .j = 0 };
    switch (d) {
        'Z', 'B', 'C', 'S', 'I' => r.i = @cVaArg(ap, c_int),
        'J' => r.j = @cVaArg(ap, i64),
        'F', 'D' => {
            const x = @cVaArg(ap, f64);
            if (d == 'F') r.f = @floatCast(x) else r.d = x;
        },
        else => r.l = @cVaArg(ap, ?*anyopaque),
    }
    return r;
}

const Reader = union(enum) {
    jv: [*]const jvalue,
    va: if (use_c_va) *anyopaque else *std.builtin.VaList,

    fn next(self: *Reader, d: u8) jvalue {
        switch (self.*) {
            .jv => |*p| {
                const r = p.*[0];
                p.* += 1;
                return r;
            },
            .va => |ap| {
                if (comptime use_c_va) {
                    var r: jvalue = undefined;
                    mono_va_next(ap, d, &r);
                    return r;
                } else {
                    return nextVa(ap, d);
                }
            },
        }
    }
};

fn zeroOf(comptime k: u8) CT(k) {
    return switch (k) {
        'V' => {},
        'L' => null,
        else => 0,
    };
}

/// Boxes the arguments described by `desc` into an Object[] (in R[0]).
fn buildArgs(desc: []const u8, rd: *Reader, R: []Ref) rt.E!void {
    var n: usize = 0;
    var i: usize = 1;
    while (desc[i] != ')') : (n += 1) i = skipType(desc, i);
    R[0] = try rt.newArray(rt.arrayOf(rt.hooks.object_class), @intCast(n));
    i = 1;
    var k: i32 = 0;
    while (desc[i] != ')') : (k += 1) {
        const d = desc[i];
        const v = rd.next(d);
        switch (d) {
            'Z' => R[1] = try P.boxZ(@intFromBool(if (rd.* == .jv) v.z != 0 else v.i != 0)),
            'B' => R[1] = try P.boxB(if (rd.* == .jv) v.b else @truncate(v.i)),
            'C' => R[1] = try P.boxC(if (rd.* == .jv) v.c else @as(u16, @truncate(@as(u32, @bitCast(v.i))))),
            'S' => R[1] = try P.boxS(if (rd.* == .jv) v.s else @truncate(v.i)),
            'I' => R[1] = try P.boxI(v.i),
            'J' => R[1] = try P.boxJ(v.j),
            'F' => R[1] = try P.boxF(v.f),
            'D' => R[1] = try P.boxD(v.d),
            else => R[1] = J.obj(v.l),
        }
        try rt.aastore(R[0], k, R[1]);
        i = skipType(desc, i);
    }
}

fn skipType(desc: []const u8, start: usize) usize {
    var i = start;
    while (desc[i] == '[') i += 1;
    if (desc[i] == 'L') {
        while (desc[i] != ';') i += 1;
    }
    return i + 1;
}

fn doCall(comptime k: u8, recv: Ref, m: *MID, rd: *Reader, virtual: bool) CT(k) {
    J.running();
    defer J.native();
    var R: [3]Ref = .{ null, null, recv };
    var F = rt.Frame{ .refs = &R, .n = 3, .meth = &jmeta };
    rt.pushFrame(&F) catch return zeroOf(k);
    defer rt.popFrame(&F);
    const meth = &m.decl.methods[m.slot];
    if (meth.flags & 0x8 != 0) rt.ensureInit(m.decl) catch return zeroOf(k);
    buildArgs(meth.desc, rd, &R) catch return zeroOf(k);
    const t = N.findTarget(m.decl, m.slot, recv, virtual) catch return zeroOf(k);
    R[1] = P.reflectInvoke(t.shape, t.fp.?, recv, R[0]) catch return zeroOf(k);
    switch (k) {
        'V' => return,
        'L' => return J.local(R[1]),
        'Z' => return @intCast(P.unboxZ(R[1]) catch return 0),
        'B' => return @truncate(P.unboxB(R[1]) catch return 0),
        'C' => return @truncate(@as(u32, @bitCast(P.unboxC(R[1]) catch return 0))),
        'S' => return @truncate(P.unboxS(R[1]) catch return 0),
        'I' => return P.unboxI(R[1]) catch return 0,
        'J' => return P.unboxJ(R[1]) catch return 0,
        'F' => return P.unboxF(R[1]) catch return 0,
        else => return P.unboxD(R[1]) catch return 0,
    }
}

const jmeta = rt.MethodMeta{ .class = "<runtime>", .name = "<jni-call>", .desc = "" };

const Mode = enum { virtual, nonvirtual, static };
const Variant = enum { plain, v, a };

fn doCallC(k: u8, recv_obj: ?*anyopaque, id: ?*anyopaque, vp: *anyopaque, virt: u8) callconv(.c) jvalue {
    var rd: Reader = .{ .va = vp };
    inline for ("VLZBCSIJFD") |ck| {
        if (k == ck) {
            const val = doCall(ck, J.obj(recv_obj), @ptrCast(@alignCast(id.?)), &rd, virt != 0);
            var out: jvalue = .{ .j = 0 };
            switch (ck) {
                'V' => {},
                'L' => out.l = val,
                'Z' => out.z = val,
                'B' => out.b = val,
                'C' => out.c = val,
                'S' => out.s = val,
                'I' => out.i = val,
                'J' => out.j = val,
                'F' => out.f = val,
                'D' => out.d = val,
                else => unreachable,
            }
            return out;
        }
    }
    return .{ .j = 0 };
}

fn newObjC(c: ?*anyopaque, id: ?*anyopaque, vp: *anyopaque) callconv(.c) ?*anyopaque {
    var rd: Reader = .{ .va = vp };
    return newObj(c, id, &rd);
}

var va_cbs_ready = std.atomic.Value(bool).init(false);

pub fn initVaCallbacks() void {
    if (!use_c_va) return;
    if (va_cbs_ready.load(.acquire)) return;
    mono_jni_set_va_cbs(@ptrCast(&doCallC), @ptrCast(&newObjC));
    va_cbs_ready.store(true, .release);
}

fn callFn(comptime k: u8, comptime mode: Mode, comptime variant: Variant) *const anyopaque {
    if (use_c_va and variant != .a) {
        const prefix = switch (mode) {
            .virtual => "mono_jni_virt_",
            .nonvirtual => "mono_jni_nv_",
            .static => "mono_jni_st_",
        };
        const suf = if (variant == .plain) "_plain" else "_v";
        return @extern(*const anyopaque, .{ .name = prefix ++ &[_]u8{k} ++ suf });
    }
    const R_ = CT(k);
    const S = struct {
        // virtual: (env, obj, mid, ...)
        fn virt_plain(e: EnvPtr, o: jobject, id: ?*anyopaque, ...) callconv(.c) R_ {
            _ = e;
            var ap = @cVaStart();
            defer @cVaEnd(&ap);
            var rd: Reader = .{ .va = &ap };
            return doCall(k, J.obj(o), @ptrCast(@alignCast(id.?)), &rd, true);
        }
        fn virt_v(e: EnvPtr, o: jobject, id: ?*anyopaque, ap0: std.builtin.VaList) callconv(.c) R_ {
            _ = e;
            var ap = @cVaCopy(@constCast(&ap0));
            defer @cVaEnd(&ap);
            var rd: Reader = .{ .va = &ap };
            return doCall(k, J.obj(o), @ptrCast(@alignCast(id.?)), &rd, true);
        }
        fn virt_a(e: EnvPtr, o: jobject, id: ?*anyopaque, a: [*]const jvalue) callconv(.c) R_ {
            _ = e;
            var rd: Reader = .{ .jv = a };
            return doCall(k, J.obj(o), @ptrCast(@alignCast(id.?)), &rd, true);
        }
        // nonvirtual: (env, obj, cls, mid, ...)
        fn nv_plain(e: EnvPtr, o: jobject, c: jobject, id: ?*anyopaque, ...) callconv(.c) R_ {
            _ = e;
            _ = c;
            var ap = @cVaStart();
            defer @cVaEnd(&ap);
            var rd: Reader = .{ .va = &ap };
            return doCall(k, J.obj(o), @ptrCast(@alignCast(id.?)), &rd, false);
        }
        fn nv_v(e: EnvPtr, o: jobject, c: jobject, id: ?*anyopaque, ap0: std.builtin.VaList) callconv(.c) R_ {
            _ = e;
            _ = c;
            var ap = @cVaCopy(@constCast(&ap0));
            defer @cVaEnd(&ap);
            var rd: Reader = .{ .va = &ap };
            return doCall(k, J.obj(o), @ptrCast(@alignCast(id.?)), &rd, false);
        }
        fn nv_a(e: EnvPtr, o: jobject, c: jobject, id: ?*anyopaque, a: [*]const jvalue) callconv(.c) R_ {
            _ = e;
            _ = c;
            var rd: Reader = .{ .jv = a };
            return doCall(k, J.obj(o), @ptrCast(@alignCast(id.?)), &rd, false);
        }
        // static: (env, cls, mid, ...)
        fn st_plain(e: EnvPtr, c: jobject, id: ?*anyopaque, ...) callconv(.c) R_ {
            _ = e;
            _ = c;
            var ap = @cVaStart();
            defer @cVaEnd(&ap);
            var rd: Reader = .{ .va = &ap };
            return doCall(k, null, @ptrCast(@alignCast(id.?)), &rd, false);
        }
        fn st_v(e: EnvPtr, c: jobject, id: ?*anyopaque, ap0: std.builtin.VaList) callconv(.c) R_ {
            _ = e;
            _ = c;
            var ap = @cVaCopy(@constCast(&ap0));
            defer @cVaEnd(&ap);
            var rd: Reader = .{ .va = &ap };
            return doCall(k, null, @ptrCast(@alignCast(id.?)), &rd, false);
        }
        fn st_a(e: EnvPtr, c: jobject, id: ?*anyopaque, a: [*]const jvalue) callconv(.c) R_ {
            _ = e;
            _ = c;
            var rd: Reader = .{ .jv = a };
            return doCall(k, null, @ptrCast(@alignCast(id.?)), &rd, false);
        }
    };
    return switch (mode) {
        .virtual => switch (variant) {
            .plain => @ptrCast(&S.virt_plain),
            .v => @ptrCast(&S.virt_v),
            .a => @ptrCast(&S.virt_a),
        },
        .nonvirtual => switch (variant) {
            .plain => @ptrCast(&S.nv_plain),
            .v => @ptrCast(&S.nv_v),
            .a => @ptrCast(&S.nv_a),
        },
        .static => switch (variant) {
            .plain => @ptrCast(&S.st_plain),
            .v => @ptrCast(&S.st_v),
            .a => @ptrCast(&S.st_a),
        },
    };
}

// ------------------------------------------------------------------ object creation
fn newObj(c: jobject, id: ?*anyopaque, rd: *Reader) jobject {
    J.running();
    defer J.native();
    const m: *MID = @ptrCast(@alignCast(id.?));
    const k = klass(c);
    var R: [3]Ref = .{ null, null, null };
    var F = rt.Frame{ .refs = &R, .n = 3, .meth = &jmeta };
    rt.pushFrame(&F) catch return null;
    defer rt.popFrame(&F);
    rt.ensureInit(k) catch return null;
    R[2] = rt.newObject(k) catch return null;
    buildArgs(m.decl.methods[m.slot].desc, rd, &R) catch return null;
    const t = N.findTarget(m.decl, m.slot, null, false) catch return null;
    _ = P.reflectInvoke(t.shape, t.fp.?, R[2], R[0]) catch return null;
    return J.local(R[2]);
}

const NewObjectFns = struct {
    fn plain(e: EnvPtr, c: jobject, id: ?*anyopaque, ...) callconv(.c) jobject {
        _ = e;
        var ap = @cVaStart();
        defer @cVaEnd(&ap);
        var rd: Reader = .{ .va = &ap };
        return newObj(c, id, &rd);
    }
    fn v(e: EnvPtr, c: jobject, id: ?*anyopaque, ap0: std.builtin.VaList) callconv(.c) jobject {
        _ = e;
        var ap = @cVaCopy(@constCast(&ap0));
        defer @cVaEnd(&ap);
        var rd: Reader = .{ .va = &ap };
        return newObj(c, id, &rd);
    }
    fn a(e: EnvPtr, c: jobject, id: ?*anyopaque, args: [*]const jvalue) callconv(.c) jobject {
        _ = e;
        var rd: Reader = .{ .jv = args };
        return newObj(c, id, &rd);
    }
    fn alloc_(e: EnvPtr, c: jobject) callconv(.c) jobject {
        _ = e;
        J.running();
        defer J.native();
        const k = klass(c);
        rt.ensureInit(k) catch return null;
        return J.local(rt.newObject(k) catch return null);
    }
};

fn fromReflected(e: EnvPtr, m: jobject) callconv(.c) ?*anyopaque {
    _ = e;
    const o = J.obj(m) orelse return null;
    if (comptime @hasDecl(P, "C_java_lang_reflect_Constructor")) {
        if (o.class == P.C_java_lang_reflect_Constructor.cls()) {
            const c = P.C_java_lang_reflect_Constructor.cast(o);
            return @ptrCast(alloc(MID, .{ .decl = rt.hooks.mirror_get_class(c.f_clazz.?), .slot = @intCast(c.f_slot) }));
        }
    }
    if (comptime @hasDecl(P, "C_java_lang_reflect_Method")) {
        const c = P.C_java_lang_reflect_Method.cast(o);
        return @ptrCast(alloc(MID, .{ .decl = rt.hooks.mirror_get_class(c.f_clazz.?), .slot = @intCast(c.f_slot) }));
    }
    return null;
}

fn fromReflectedField(e: EnvPtr, f: jobject) callconv(.c) ?*anyopaque {
    _ = e;
    const o = J.obj(f) orelse return null;
    if (comptime @hasDecl(P, "C_java_lang_reflect_Field")) {
        const fo = P.C_java_lang_reflect_Field.cast(o);
        const k = rt.hooks.mirror_get_class(fo.f_clazz.?);
        const slot: usize = @intCast(fo.f_slot);
        const is_static = (fo.f_modifiers & 0x8) != 0;
        if (is_static) {
            if (slot >= k.static_fields.len) return null;
            const sf = k.static_fields[slot];
            return @ptrCast(alloc(FID, .{ .is_static = true, .offset = 0, .ptr = sf.ptr, .kind = kindOfDesc(sf.desc) }));
        } else {
            if (slot >= k.fields.len) return null;
            const ff = k.fields[slot];
            return @ptrCast(alloc(FID, .{ .is_static = false, .offset = ff.offset, .ptr = undefined, .kind = kindOfDesc(ff.desc) }));
        }
    }
    return null;
}

fn toReflectedMethod(e: EnvPtr, cls: jobject, methodID: ?*anyopaque, isStatic: u8) callconv(.c) jobject {
    _ = .{ e, cls, isStatic };
    const id: *const MID = @ptrCast(@alignCast(methodID orelse return null));
    J.running();
    defer J.native();
    const m = id.decl.methods[id.slot];
    if (std.mem.eql(u8, m.name, "<init>")) {
        if (comptime @hasDecl(P, "C_java_lang_reflect_Constructor")) {
            const o = rt.newObject(P.C_java_lang_reflect_Constructor.cls()) catch return null;
            const co = P.C_java_lang_reflect_Constructor.cast(o.?);
            co.f_clazz = rt.classMirror(id.decl) catch return null;
            co.f_slot = @intCast(id.slot);
            co.f_modifiers = @intCast(m.flags & 0xffff);
            return J.local(o);
        }
    } else if (comptime @hasDecl(P, "C_java_lang_reflect_Method")) {
        const o = rt.newObject(P.C_java_lang_reflect_Method.cls()) catch return null;
        const mo = P.C_java_lang_reflect_Method.cast(o.?);
        mo.f_clazz = rt.classMirror(id.decl) catch return null;
        mo.f_slot = @intCast(id.slot);
        mo.f_name = rt.newStringUtf8(m.name) catch return null;
        mo.f_modifiers = @intCast(m.flags & 0xffff);
        return J.local(o);
    }
    return null;
}

fn toReflectedField(e: EnvPtr, cls: jobject, fieldID: ?*anyopaque, isStatic: u8) callconv(.c) jobject {
    _ = e;
    const id: *const FID = @ptrCast(@alignCast(fieldID orelse return null));
    const co = J.obj(cls) orelse return null;
    const k = rt.hooks.mirror_get_class(co);
    J.running();
    defer J.native();
    if (comptime @hasDecl(P, "C_java_lang_reflect_Field")) {
        const o = rt.newObject(P.C_java_lang_reflect_Field.cls()) catch return null;
        const fo = P.C_java_lang_reflect_Field.cast(o.?);
        fo.f_clazz = co;
        if (isStatic != 0 or id.is_static) {
            for (k.static_fields, 0..) |sf, idx| {
                if (sf.ptr == id.ptr) {
                    fo.f_slot = @intCast(idx);
                    fo.f_name = rt.newStringUtf8(sf.name) catch return null;
                    fo.f_modifiers = @intCast(sf.flags & 0xffff);
                    break;
                }
            }
        } else {
            for (k.fields, 0..) |ff, idx| {
                if (ff.offset == id.offset) {
                    fo.f_slot = @intCast(idx);
                    fo.f_name = rt.newStringUtf8(ff.name) catch return null;
                    fo.f_modifiers = @intCast(ff.flags & 0xffff);
                    break;
                }
            }
        }
        return J.local(o);
    }
    return null;
}

fn getModule(e: EnvPtr, clazz: jobject) callconv(.c) jobject {
    _ = e;
    const co = J.obj(clazz) orelse return null;
    J.running();
    defer J.native();
    if (comptime @hasDecl(P, "C_java_lang_Class")) {
        inline for (@typeInfo(P.C_java_lang_Class).@"struct".decls) |d| {
            if (comptime std.mem.startsWith(u8, d.name, "m_getModule_") and !std.mem.endsWith(u8, d.name, "_r")) {
                const raw = @field(P.C_java_lang_Class, d.name)(co);
                const res = if (@TypeOf(raw) == rt.Res(rt.Ref))
                    (rt.unR(rt.Ref, raw) catch return null)
                else
                    (raw catch return null);
                return J.local(res);
            }
        }
    }
    return null;
}

// ------------------------------------------------------------------ lookup by JNI function name
const kinds = [_]struct { name: []const u8, k: u8 }{
    .{ .name = "Object", .k = 'L' }, .{ .name = "Boolean", .k = 'Z' }, .{ .name = "Byte", .k = 'B' },
    .{ .name = "Char", .k = 'C' },   .{ .name = "Short", .k = 'S' },   .{ .name = "Int", .k = 'I' },
    .{ .name = "Long", .k = 'J' },   .{ .name = "Float", .k = 'F' },   .{ .name = "Double", .k = 'D' },
    .{ .name = "Void", .k = 'V' },
};

pub fn lookup(comptime name: []const u8) ?*const anyopaque {
    const eq = std.mem.eql;
    if (eq(u8, name, "GetMethodID")) return @ptrCast(&getMethodID(false));
    if (eq(u8, name, "GetStaticMethodID")) return @ptrCast(&getMethodID(true));
    if (eq(u8, name, "GetFieldID")) return @ptrCast(&getFieldID(false));
    if (eq(u8, name, "GetStaticFieldID")) return @ptrCast(&getFieldID(true));
    if (eq(u8, name, "NewObject")) return if (use_c_va) @extern(*const anyopaque, .{ .name = "mono_jni_newobj_plain" }) else @ptrCast(&NewObjectFns.plain);
    if (eq(u8, name, "NewObjectV")) return if (use_c_va) @extern(*const anyopaque, .{ .name = "mono_jni_newobj_v" }) else @ptrCast(&NewObjectFns.v);
    if (eq(u8, name, "NewObjectA")) return @ptrCast(&NewObjectFns.a);
    if (eq(u8, name, "AllocObject")) return @ptrCast(&NewObjectFns.alloc_);
    if (eq(u8, name, "FromReflectedMethod")) return @ptrCast(&fromReflected);
    if (eq(u8, name, "FromReflectedField")) return @ptrCast(&fromReflectedField);
    if (eq(u8, name, "ToReflectedMethod")) return @ptrCast(&toReflectedMethod);
    if (eq(u8, name, "ToReflectedField")) return @ptrCast(&toReflectedField);
    if (eq(u8, name, "GetModule")) return @ptrCast(&getModule);
    inline for (kinds) |kd| {
        inline for ([_]struct { p: []const u8, m: Mode }{
            .{ .p = "Call", .m = .virtual }, .{ .p = "CallNonvirtual", .m = .nonvirtual }, .{ .p = "CallStatic", .m = .static },
        }) |md| {
            inline for ([_]struct { s: []const u8, v: Variant }{ .{ .s = "", .v = .plain }, .{ .s = "V", .v = .v }, .{ .s = "A", .v = .a } }) |vr| {
                if (comptime eq(u8, name, md.p ++ kd.name ++ "Method" ++ vr.s)) return callFn(kd.k, md.m, vr.v);
            }
        }
        if (comptime kd.k != 'V') {
            if (comptime eq(u8, name, "Get" ++ kd.name ++ "Field")) return @ptrCast(&getField(kd.k, false).f);
            if (comptime eq(u8, name, "Set" ++ kd.name ++ "Field")) return @ptrCast(&setField(kd.k, false).f);
            if (comptime eq(u8, name, "GetStatic" ++ kd.name ++ "Field")) return @ptrCast(&getField(kd.k, true).f);
            if (comptime eq(u8, name, "SetStatic" ++ kd.name ++ "Field")) return @ptrCast(&setField(kd.k, true).f);
        }
    }
    return null;
}
