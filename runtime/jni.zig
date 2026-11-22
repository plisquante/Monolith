//! JNI bridge: lets the native image call the real native libraries shipped with an
//! application (LWJGL, OpenAL, stb, jtracy, ...) through their `Java_*` entry points.
//!
//! * A real JNIEnv (pointer to a function table, laid out as in the JDK's jni.h) per thread.
//!   Libraries may replace a thread's table with their own copy (LWJGL does).
//! * jobject handles are object addresses (the GC is non-moving). Objects created by native
//!   code are kept alive as thread-local "local references" for the duration of the native
//!   call; NewGlobalRef pins objects in a global root list.
//! * Native code runs in the GC-safe state; JNI functions that touch the heap switch back to
//!   the running state first.
//! * Libraries are dlopen()ed from the natives directory (MONO_NATIVES, default
//!   <exe dir>/natives) and their JNI_OnLoad is called with our JavaVM.
//! * JNI functions that are not implemented abort with their name (never a silent result).
const std = @import("std");
const rt = @import("rt.zig");
const P = @import("classes.zig");
const T = @import("jni_table.zig");
const Calls = @import("jni_call.zig");

const Ref = rt.Ref;
const E = rt.E;
pub const jobject = ?*anyopaque;
pub const Env = ?*const anyopaque; // JNIEnv == pointer to the function table
pub const EnvPtr = *Env;

// ------------------------------------------------------------------ helpers
pub inline fn obj(o: jobject) Ref {
    return @ptrCast(@alignCast(o));
}
pub inline fn hdl(r: Ref) jobject {
    return @ptrCast(r);
}

/// Called with the thread in the safe state (inside a native call): run `body` in the
/// running state so it may allocate / touch the heap safely.
pub inline fn running() void {
    rt.leaveSafe();
}
pub inline fn native() void {
    rt.enterSafeFromCallback();
}

pub fn local(r: Ref) jobject {
    if (r == null) return null;
    rt.cur.jni_locals.append(rt.gpa, r) catch @panic("JNI: out of memory");
    return hdl(r);
}

fn chars(s: Ref) []u16 {
    return rt.hooks.string_chars(s.?);
}

fn arr(a: jobject) *rt.Array {
    return rt.asArray(obj(a).?);
}

fn fatal(comptime fmt: []const u8, args: anytype) noreturn {
    var buf: [512]u8 = undefined;
    rt.writeFd(2, std.fmt.bufPrint(&buf, "monolith: JNI: " ++ fmt ++ "\n", args) catch "monolith: JNI: fatal\n");
    std.process.abort();
}

// ------------------------------------------------------------------ the JNI functions

/// Process-wide JNI state, shared by all program shards (see rt.Shared).
const JniState = struct {
    registry: std.StringHashMapUnmanaged(*const anyopaque) = .{},
    registry_lock: std.Thread.Mutex = .{},
    libs_lock: std.Thread.Mutex = .{},
    libs_loaded: bool = false,
    libs: std.ArrayList(*anyopaque) = .empty,
};
const js = rt.Shared(JniState, "jni", 1 << 16).ptr;
const registry = &js.registry;
const registry_lock = &js.registry_lock;
const libs_lock = &js.libs_lock;
const libs = &js.libs;
// ------------------------------------------------------------------ RegisterNatives
const NativeMethod = extern struct { name: [*:0]const u8, signature: [*:0]const u8, fn_ptr: ?*const anyopaque };

/// JNI name mangling (JNI spec, "Resolving Native Method Names") of a UTF-8 string.
fn mangleInto(out: *std.ArrayListUnmanaged(u8), str: []const u8) !void {
    for (str) |ch| switch (ch) {
        '/' => try out.append(rt.gpa, '_'),
        '_' => try out.appendSlice(rt.gpa, "_1"),
        ';' => try out.appendSlice(rt.gpa, "_2"),
        '[' => try out.appendSlice(rt.gpa, "_3"),
        'a'...'z', 'A'...'Z', '0'...'9' => try out.append(rt.gpa, ch),
        else => try out.writer(rt.gpa).print("_0{x:0>4}", .{ch}),
    };
}

fn registerOne(cname: []const u8, m: NativeMethod) !void {
    const sig = std.mem.span(m.signature);
    const args_end = std.mem.indexOfScalar(u8, sig, ')') orelse sig.len;
    var key: std.ArrayListUnmanaged(u8) = .{};
    try key.appendSlice(rt.gpa, "Java_");
    try mangleInto(&key, cname);
    try key.append(rt.gpa, '_');
    try mangleInto(&key, std.mem.span(m.name));
    try key.appendSlice(rt.gpa, "__");
    try mangleInto(&key, sig[1..args_end]);
    registry_lock.lock();
    defer registry_lock.unlock();
    try registry.put(rt.gpa, try key.toOwnedSlice(rt.gpa), m.fn_ptr.?);
}

/// Native method registered at run time with RegisterNatives (JNA, JNI_OnLoad tables), by long name.
pub fn registered(long: [*:0]const u8) ?*const anyopaque {
    registry_lock.lock();
    defer registry_lock.unlock();
    return registry.get(std.mem.span(long));
}

const Impl = struct {
    fn RegisterNatives(e: EnvPtr, c: jobject, methods: [*]const NativeMethod, n: i32) callconv(.c) i32 {
        _ = e;
        running();
        defer native();
        const k = rt.hooks.mirror_get_class(obj(c).?);
        var i: usize = 0;
        while (i < @as(usize, @intCast(n))) : (i += 1) {
            registerOne(k.name, methods[i]) catch return -1;
        }
        return 0;
    }
    fn UnregisterNatives(e: EnvPtr, c: jobject) callconv(.c) i32 {
        _ = e;
        _ = c;
        return 0;
    }
    fn GetVersion(e: EnvPtr) callconv(.c) i32 {
        _ = e;
        return 0x00150000; // JNI_VERSION_21
    }
    fn DefineClass(e: EnvPtr, name: ?[*:0]const u8, loader: jobject, buf: [*]const u8, len: i32) callconv(.c) jobject {
        _ = .{ e, loader, buf, len };
        running();
        defer native();
        rt.throwKind(.NoClassDefFound, if (name) |n| std.mem.span(n) else "DefineClass not supported in closed-world image") catch {};
        return null;
    }
    fn FindClass(e: EnvPtr, name: [*:0]const u8) callconv(.c) jobject {
        _ = e;
        const n = std.mem.span(name);
        running();
        defer native();
        if (rt.findClass(n)) |c| {
            const m = rt.classMirror(c) catch return null;
            return local(m);
        }
        rt.throwKind(.NoClassDefFound, n) catch {};
        return null;
    }
    fn GetSuperclass(e: EnvPtr, c: jobject) callconv(.c) jobject {
        _ = e;
        const k = rt.hooks.mirror_get_class(obj(c).?);
        if (k.kind != .instance) return null;
        const s = k.super orelse return null;
        running();
        defer native();
        return local(rt.classMirror(s) catch null);
    }
    fn IsAssignableFrom(e: EnvPtr, a: jobject, b: jobject) callconv(.c) u8 {
        _ = e;
        return @intFromBool(rt.isSubclass(rt.hooks.mirror_get_class(obj(a).?), rt.hooks.mirror_get_class(obj(b).?)));
    }
    fn Throw(e: EnvPtr, t: jobject) callconv(.c) i32 {
        _ = e;
        rt.cur.exc = obj(t);
        return 0;
    }
    fn ThrowNew(e: EnvPtr, c: jobject, msg: ?[*:0]const u8) callconv(.c) i32 {
        _ = e;
        running();
        defer native();
        const k = rt.hooks.mirror_get_class(obj(c).?);
        const ex = rt.hooks.new_throwable(k, if (msg) |m| std.mem.span(m) else null) catch {
            return -1;
        };
        rt.cur.exc = ex;
        return 0;
    }
    fn ExceptionOccurred(e: EnvPtr) callconv(.c) jobject {
        _ = e;
        return hdl(rt.cur.exc);
    }
    fn ExceptionDescribe(e: EnvPtr) callconv(.c) void {
        _ = e;
        rt.writeFd(2, "monolith: JNI: ExceptionDescribe: exception pending in native code\n");
    }
    fn ExceptionClear(e: EnvPtr) callconv(.c) void {
        _ = e;
        rt.cur.exc = null;
    }
    fn ExceptionCheck(e: EnvPtr) callconv(.c) u8 {
        _ = e;
        return @intFromBool(rt.cur.exc != null);
    }
    fn FatalError(e: EnvPtr, msg: [*:0]const u8) callconv(.c) noreturn {
        _ = e;
        fatal("FatalError: {s}", .{std.mem.span(msg)});
    }
    fn PushLocalFrame(e: EnvPtr, cap: i32) callconv(.c) i32 {
        _ = e;
        _ = cap;
        return 0;
    }
    fn PopLocalFrame(e: EnvPtr, result: jobject) callconv(.c) jobject {
        _ = e;
        return result;
    }
    fn NewGlobalRef(e: EnvPtr, o: jobject) callconv(.c) jobject {
        _ = e;
        if (o == null) return null;
        rt.jni_globals_lock.lock();
        defer rt.jni_globals_lock.unlock();
        rt.jni_globals.append(rt.gpa, obj(o)) catch @panic("JNI: out of memory");
        return o;
    }
    fn DeleteGlobalRef(e: EnvPtr, o: jobject) callconv(.c) void {
        _ = e;
        if (o == null) return;
        rt.jni_globals_lock.lock();
        defer rt.jni_globals_lock.unlock();
        var i = rt.jni_globals.items.len;
        while (i > 0) {
            i -= 1;
            if (rt.jni_globals.items[i] == obj(o)) {
                _ = rt.jni_globals.swapRemove(i);
                return;
            }
        }
    }
    fn DeleteLocalRef(e: EnvPtr, o: jobject) callconv(.c) void {
        _ = e;
        _ = o; // released when the native call returns
    }
    fn IsSameObject(e: EnvPtr, a: jobject, b: jobject) callconv(.c) u8 {
        _ = e;
        return @intFromBool(a == b);
    }
    fn NewLocalRef(e: EnvPtr, o: jobject) callconv(.c) jobject {
        _ = e;
        return local(obj(o));
    }
    fn EnsureLocalCapacity(e: EnvPtr, cap: i32) callconv(.c) i32 {
        _ = e;
        _ = cap;
        return 0;
    }
    fn GetObjectClass(e: EnvPtr, o: jobject) callconv(.c) jobject {
        _ = e;
        running();
        defer native();
        return local(rt.classMirror(obj(o).?.class) catch null);
    }
    fn IsInstanceOf(e: EnvPtr, o: jobject, c: jobject) callconv(.c) u8 {
        _ = e;
        if (o == null) return 1;
        return @intFromBool(rt.isSubclass(obj(o).?.class, rt.hooks.mirror_get_class(obj(c).?)));
    }
    fn NewString(e: EnvPtr, u: [*]const u16, len: i32) callconv(.c) jobject {
        _ = e;
        running();
        defer native();
        return local(rt.hooks.make_string(u[0..@intCast(len)]) catch null);
    }
    fn GetStringLength(e: EnvPtr, s: jobject) callconv(.c) i32 {
        _ = e;
        return @intCast(chars(obj(s)).len);
    }
    fn GetStringChars(e: EnvPtr, s: jobject, is_copy: ?*u8) callconv(.c) [*]const u16 {
        _ = e;
        if (is_copy) |c| c.* = 0;
        return chars(obj(s)).ptr;
    }
    fn ReleaseStringChars(e: EnvPtr, s: jobject, p: [*]const u16) callconv(.c) void {
        _ = e;
        _ = s;
        _ = p;
    }
    fn NewStringUTF(e: EnvPtr, u: ?[*:0]const u8) callconv(.c) jobject {
        _ = e;
        const p = u orelse return null;
        running();
        defer native();
        return local(rt.newStringUtf8(std.mem.span(p)) catch null);
    }
    fn GetStringUTFLength(e: EnvPtr, s: jobject) callconv(.c) i32 {
        _ = e;
        const u = rt.stringToUtf8(rt.gpa, obj(s)) catch return 0;
        defer rt.gpa.free(u);
        return @intCast(u.len);
    }
    fn GetStringUTFLengthAsLong(e: EnvPtr, s: jobject) callconv(.c) i64 {
        return GetStringUTFLength(e, s);
    }
    fn GetStringUTFChars(e: EnvPtr, s: jobject, is_copy: ?*u8) callconv(.c) ?[*:0]u8 {
        _ = e;
        if (is_copy) |c| c.* = 1;
        const u = rt.stringToUtf8(rt.gpa, obj(s)) catch return null;
        defer rt.gpa.free(u);
        const m: [*]u8 = @ptrCast(std.c.malloc(u.len + 1) orelse return null);
        @memcpy(m[0..u.len], u);
        m[u.len] = 0;
        return @ptrCast(m);
    }
    fn ReleaseStringUTFChars(e: EnvPtr, s: jobject, p: ?*anyopaque) callconv(.c) void {
        _ = e;
        _ = s;
        std.c.free(p);
    }
    fn GetStringRegion(e: EnvPtr, s: jobject, start: i32, len: i32, buf: [*]u16) callconv(.c) void {
        _ = e;
        const c = chars(obj(s));
        @memcpy(buf[0..@intCast(len)], c[@intCast(start)..][0..@intCast(len)]);
    }
    fn GetStringUTFRegion(e: EnvPtr, s: jobject, start: i32, len: i32, buf: [*]u8) callconv(.c) void {
        _ = e;
        const c = chars(obj(s))[@intCast(start)..][0..@intCast(len)];
        var o: usize = 0;
        for (c) |ch| {
            var tmp: [4]u8 = undefined;
            const n = std.unicode.utf8Encode(if (ch >= 0xD800 and ch < 0xE000) '?' else ch, &tmp) catch 1;
            @memcpy(buf[o..][0..n], tmp[0..n]);
            o += n;
        }
        buf[o] = 0;
    }
    fn GetStringCritical(e: EnvPtr, s: jobject, is_copy: ?*u8) callconv(.c) [*]const u16 {
        return GetStringChars(e, s, is_copy);
    }
    fn ReleaseStringCritical(e: EnvPtr, s: jobject, p: [*]const u16) callconv(.c) void {
        ReleaseStringChars(e, s, p);
    }
    fn GetArrayLength(e: EnvPtr, a: jobject) callconv(.c) i32 {
        _ = e;
        return arr(a).length;
    }
    fn NewObjectArray(e: EnvPtr, len: i32, c: jobject, init: jobject) callconv(.c) jobject {
        _ = e;
        running();
        defer native();
        const k = rt.hooks.mirror_get_class(obj(c).?);
        const a = rt.newArray(rt.arrayOf(k), len) catch return null;
        const d = rt.arrayData(Ref, rt.asArray(a.?));
        for (0..@intCast(len)) |i| d[i] = obj(init);
        return local(a);
    }
    fn GetObjectArrayElement(e: EnvPtr, a: jobject, i: i32) callconv(.c) jobject {
        _ = e;
        running();
        defer native();
        return local(rt.aload(Ref, obj(a), i) catch null);
    }
    fn SetObjectArrayElement(e: EnvPtr, a: jobject, i: i32, v: jobject) callconv(.c) void {
        _ = e;
        running();
        defer native();
        rt.aastore(obj(a), i, obj(v)) catch {};
    }
    fn GetPrimitiveArrayCritical(e: EnvPtr, a: jobject, is_copy: ?*u8) callconv(.c) ?*anyopaque {
        _ = e;
        if (is_copy) |c| c.* = 0;
        return @ptrCast(rt.arrayData(u8, arr(a)));
    }
    fn ReleasePrimitiveArrayCritical(e: EnvPtr, a: jobject, p: ?*anyopaque, mode: i32) callconv(.c) void {
        _ = e;
        _ = a;
        _ = p;
        _ = mode;
    }
    fn NewDirectByteBuffer(e: EnvPtr, addr: ?*anyopaque, cap: i64) callconv(.c) jobject {
        _ = e;
        running();
        defer native();
        const b = P.newDirectByteBuffer(@bitCast(@intFromPtr(addr)), cap) catch return null;
        return local(b);
    }
    fn GetDirectBufferAddress(e: EnvPtr, b: jobject) callconv(.c) ?*anyopaque {
        _ = e;
        if (comptime !@hasDecl(P, "C_java_nio_Buffer")) return null;
        const a = P.C_java_nio_Buffer.cast(obj(b) orelse return null).f_address;
        return @ptrFromInt(@as(usize, @bitCast(a)));
    }
    fn GetDirectBufferCapacity(e: EnvPtr, b: jobject) callconv(.c) i64 {
        _ = e;
        if (comptime !@hasDecl(P, "C_java_nio_Buffer")) return -1;
        return P.C_java_nio_Buffer.cast(obj(b) orelse return -1).f_capacity;
    }
    fn MonitorEnter(e: EnvPtr, o: jobject) callconv(.c) i32 {
        _ = e;
        running();
        defer native();
        rt.monitorEnter(obj(o)) catch return -1;
        return 0;
    }
    fn MonitorExit(e: EnvPtr, o: jobject) callconv(.c) i32 {
        _ = e;
        running();
        defer native();
        rt.monitorExit(obj(o)) catch return -1;
        return 0;
    }
    fn GetJavaVM(e: EnvPtr, vm_out: *?*const anyopaque) callconv(.c) i32 {
        _ = e;
        vm_out.* = @ptrCast(&java_vm);
        return 0;
    }
    fn GetObjectRefType(e: EnvPtr, o: jobject) callconv(.c) i32 {
        _ = e;
        return if (o == null) 0 else 1; // JNILocalRefType
    }
    fn NewWeakGlobalRef(e: EnvPtr, o: jobject) callconv(.c) jobject {
        return NewGlobalRef(e, o);
    }
    fn DeleteWeakGlobalRef(e: EnvPtr, o: jobject) callconv(.c) void {
        DeleteGlobalRef(e, o);
    }
    fn IsVirtualThread(e: EnvPtr, o: jobject) callconv(.c) u8 {
        _ = e;
        _ = o;
        return 0;
    }
};

// Typed primitive array accessors (Get/Release<T>ArrayElements, Get/Set<T>ArrayRegion, New<T>Array)
fn PrimOps(comptime Elem: type, comptime prim: u8) type {
    return struct {
        fn getElements(e: EnvPtr, a: jobject, is_copy: ?*u8) callconv(.c) ?[*]Elem {
            _ = e;
            if (is_copy) |c| c.* = 0;
            return rt.arrayData(Elem, arr(a));
        }
        fn releaseElements(e: EnvPtr, a: jobject, p: ?[*]Elem, mode: i32) callconv(.c) void {
            _ = e;
            _ = a;
            _ = p;
            _ = mode;
        }
        fn getRegion(e: EnvPtr, a: jobject, start: i32, len: i32, buf: [*]Elem) callconv(.c) void {
            _ = e;
            const d = rt.arrayData(Elem, arr(a));
            @memcpy(buf[0..@intCast(len)], d[@intCast(start)..][0..@intCast(len)]);
        }
        fn setRegion(e: EnvPtr, a: jobject, start: i32, len: i32, buf: [*]const Elem) callconv(.c) void {
            _ = e;
            const d = rt.arrayData(Elem, arr(a));
            @memcpy(d[@intCast(start)..][0..@intCast(len)], buf[0..@intCast(len)]);
        }
        fn new(e: EnvPtr, len: i32) callconv(.c) jobject {
            _ = e;
            running();
            defer native();
            return local(rt.newArray(rt.arrayOf(rt.primByChar(prim)), len) catch null);
        }
    };
}

const prim_names = .{
    .{ "Boolean", u8, 'Z' }, .{ "Byte", i8, 'B' }, .{ "Char", u16, 'C' }, .{ "Short", i16, 'S' },
    .{ "Int", i32, 'I' },    .{ "Long", i64, 'J' }, .{ "Float", f32, 'F' }, .{ "Double", f64, 'D' },
};

fn lookupImpl(comptime name: []const u8) ?*const anyopaque {
    if (@hasDecl(Impl, name)) return @ptrCast(&@field(Impl, name));
    if (Calls.lookup(name)) |p| return p;
    inline for (prim_names) |pn| {
        const Ops = PrimOps(pn[1], pn[2]);
        if (comptime std.mem.eql(u8, name, "Get" ++ pn[0] ++ "ArrayElements")) return @ptrCast(&Ops.getElements);
        if (comptime std.mem.eql(u8, name, "Release" ++ pn[0] ++ "ArrayElements")) return @ptrCast(&Ops.releaseElements);
        if (comptime std.mem.eql(u8, name, "Get" ++ pn[0] ++ "ArrayRegion")) return @ptrCast(&Ops.getRegion);
        if (comptime std.mem.eql(u8, name, "Set" ++ pn[0] ++ "ArrayRegion")) return @ptrCast(&Ops.setRegion);
        if (comptime std.mem.eql(u8, name, "New" ++ pn[0] ++ "Array")) return @ptrCast(&Ops.new);
    }
    return null;
}

/// A distinct function per unimplemented slot, so the abort message names the JNI function.
fn Missing(comptime idx: usize) type {
    return struct {
        fn f() callconv(.c) noreturn {
            fatal("JNI function {s} is not implemented by the monolith runtime", .{T.names[idx]});
        }
    };
}

/// Mutable: LWJGL stores its capabilities pointer in a reserved slot of the table.
var function_table: [T.count]?*const anyopaque = blk: {
    @setEvalBranchQuota(200000);
    var t: [T.count]?*const anyopaque = undefined;
    for (0..T.count) |i| {
        const n = T.names[i];
        if (std.mem.startsWith(u8, n, "reserved")) {
            t[i] = null;
        } else if (lookupImpl(n)) |p| {
            t[i] = p;
        } else {
            t[i] = @ptrCast(&Missing(i).f);
        }
    }
    break :blk t;
};

// ------------------------------------------------------------------ JavaVM
const VmImpl = struct {
    fn DestroyJavaVM(vm: *anyopaque) callconv(.c) i32 {
        _ = vm;
        return 0;
    }
    fn AttachCurrentThread(vm: *anyopaque, penv: *?EnvPtr, args: ?*anyopaque) callconv(.c) i32 {
        _ = vm;
        _ = args;
        penv.* = attachNativeThread();
        return 0;
    }
    fn DetachCurrentThread(vm: *anyopaque) callconv(.c) i32 {
        _ = vm;
        return 0;
    }
    fn GetEnv(vm: *anyopaque, penv: *?EnvPtr, version: i32) callconv(.c) i32 {
        _ = vm;
        _ = version;
        if (!attached and !rt.thread_attached) {
            penv.* = null;
            return -2; // JNI_EDETACHED
        }
        penv.* = env();
        return 0;
    }
};
const vm_table = [_]?*const anyopaque{
    null,                                     null, null,
    @ptrCast(&VmImpl.DestroyJavaVM),          @ptrCast(&VmImpl.AttachCurrentThread),
    @ptrCast(&VmImpl.DetachCurrentThread),    @ptrCast(&VmImpl.GetEnv),
    @ptrCast(&VmImpl.AttachCurrentThread), // AttachCurrentThreadAsDaemon
};
var java_vm: *const [vm_table.len]?*const anyopaque = &vm_table;

threadlocal var attached: bool = false;

/// JNIEnv* of the calling (attached) thread.
pub fn env() EnvPtr {
    Calls.initVaCallbacks();
    const t = rt.cur;
    if (t.jni_env == null) t.jni_env = @ptrCast(&function_table);
    return &t.jni_env;
}

/// A thread created by native code calls into Java (e.g. an audio callback thread).
fn attachNativeThread() EnvPtr {
    if (!attached and !rt.thread_attached) {
        rt.attachForeignThread();
        attached = true;
    }
    return env();
}

pub fn markAttached() void {
    attached = true;
}

// ------------------------------------------------------------------ library loading

fn nativesDir(buf: []u8) []const u8 {
    return rt.getenv("MONO_NATIVES") orelse blk: {
        var exe_buf: [std.fs.max_path_bytes]u8 = undefined;
        const exe = std.fs.selfExePath(&exe_buf) catch break :blk "natives";
        const d = std.fs.path.dirname(exe) orelse ".";
        break :blk std.fmt.bufPrint(buf, "{s}/natives", .{d}) catch "natives";
    };
}

fn openLib(p: [*:0]const u8) bool {
    const h = std.c.dlopen(p, .{ .NOW = true, .GLOBAL = true }) orelse return false;
    for (libs.items) |x| if (x == h) return true;
    libs.append(rt.gpa, h) catch {};
    if (std.c.dlsym(h, "JNI_OnLoad")) |onload| {
        const f: *const fn (*const anyopaque, ?*anyopaque) callconv(.c) i32 = @ptrCast(@alignCast(onload));
        rt.enterNative();
        _ = f(@ptrCast(&java_vm), null);
        rt.leaveNative();
    }
    return true;
}

/// Runtime.load(path): dlopen a library and make its JNI entry points resolvable.
pub fn J_java_lang_Runtime__load1(this: Ref, path: Ref) E!i32 {
    _ = this;
    loadLibraries();
    const jchars = rt.hooks.string_chars(try rt.nn(path));
    var buf: [std.fs.max_path_bytes]u8 = undefined;
    var n: usize = 0;
    var it = std.unicode.Utf16LeIterator.init(jchars);
    while (it.nextCodepoint() catch null) |cp| {
        const l = std.unicode.utf8Encode(cp, buf[n..]) catch return 0;
        n += l;
        if (n + 8 >= buf.len) return 0;
    }
    buf[n] = 0;
    libs_lock.lock();
    defer libs_lock.unlock();
    return @intFromBool(openLib(@ptrCast(&buf)));
}
/// True if lib<name>.so was found in the natives directory at startup.
pub fn J_java_lang_Runtime__isPreloaded(this: Ref, file: Ref) E!i32 {
    _ = this;
    loadLibraries();
    var dbuf: [std.fs.max_path_bytes]u8 = undefined;
    const dir = nativesDir(&dbuf);
    var fb: [256]u8 = undefined;
    const jchars = rt.hooks.string_chars(try rt.nn(file));
    if (jchars.len >= fb.len) return 0;
    for (jchars, 0..) |c, i| fb[i] = @truncate(c);
    var pb: [std.fs.max_path_bytes]u8 = undefined;
    const p = std.fmt.bufPrint(&pb, "{s}/{s}", .{ dir, fb[0..jchars.len] }) catch return 0;
    std.fs.cwd().access(p, .{}) catch return 0;
    return 1;
}

fn loadLibraries() void {
    libs_lock.lock();
    defer libs_lock.unlock();
    if (js.libs_loaded) return;
    js.libs_loaded = true;
    var dir_buf: [std.fs.max_path_bytes]u8 = undefined;
    const dir_path = nativesDir(&dir_buf);
    var dir = std.fs.cwd().openDir(dir_path, .{ .iterate = true }) catch return;
    defer dir.close();
    var it = dir.iterate();
    while (it.next() catch null) |e| {
        if (!std.mem.endsWith(u8, e.name, ".so") and std.mem.indexOf(u8, e.name, ".so.") == null) continue;
        var pb: [std.fs.max_path_bytes]u8 = undefined;
        const p = std.fmt.bufPrintZ(&pb, "{s}/{s}", .{ dir_path, e.name }) catch continue;
        _ = openLib(p);
    }
}

/// Resolve a JNI entry point (short name, then the overload-qualified long name).
pub fn resolve(slot: *?*const anyopaque, short: [*:0]const u8, long: [*:0]const u8) E!*const anyopaque {
    if (@atomicLoad(?*const anyopaque, slot, .acquire)) |p| return p;
    if (registered(long)) |p| {
        @atomicStore(?*const anyopaque, slot, p, .release);
        return p;
    }
    loadLibraries();
    for (libs.items) |h| {
        const sym = std.c.dlsym(h, short) orelse std.c.dlsym(h, long) orelse continue;
        @atomicStore(?*const anyopaque, slot, sym, .release);
        return sym;
    }
    return rt.throwKind(.UnsatisfiedLink, std.mem.span(short));
}

/// Bracket for a call into native code: returns the local-reference mark.
pub inline fn begin() usize {
    _ = env();
    attached = true;
    const m = rt.cur.jni_locals.items.len;
    rt.enterNative();
    return m;
}

/// After the native call: back to the running state, drop local refs, report a pending exception.
pub inline fn end(mark: usize) E!void {
    rt.leaveNative();
    rt.cur.jni_locals.shrinkRetainingCapacity(mark);
    if (rt.cur.exc != null) return error.Throw;
}
