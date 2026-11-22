//! Native method implementations for the j2zig class library.
//!
//! Naming: J_<mangled class>__<mangled method>[__<mangled arg descriptor>] (see
//! j2zig/zigen.py:native_name). Signatures follow the generated calling convention:
//! instance methods take `this: rt.Ref` first, Java int-like types are i32.
const std = @import("std");
const rt = @import("rt.zig");
const P = @import("classes.zig");

const Ref = rt.Ref;
const E = rt.E;

fn cls(m: Ref) *rt.Class {
    return rt.hooks.mirror_get_class(m.?);
}

fn str(s: []const u8) E!Ref {
    return rt.newStringUtf8(s);
}

fn dotted(alloc: std.mem.Allocator, name: []const u8) ![]u8 {
    const out = try alloc.dupe(u8, name);
    for (out) |*c| if (c.* == '/') {
        c.* = '.';
    };
    return out;
}

// ------------------------------------------------------------------ java.lang.Object
pub fn J_java_lang_Object__getClass(this: Ref) E!Ref {
    return rt.classMirror((try rt.nn(this)).class);
}
pub fn J_java_lang_Object__hashCode(this: Ref) E!i32 {
    return rt.identityHash(try rt.nn(this));
}
pub fn J_java_lang_Object__clone(this: Ref) E!Ref {
    return rt.cloneObject(this);
}
pub fn J_java_lang_Object__notify(this: Ref) E!void {
    return rt.monitorNotify(this, false);
}
pub fn J_java_lang_Object__notifyAll(this: Ref) E!void {
    return rt.monitorNotify(this, true);
}
pub fn J_java_lang_Object__wait0(this: Ref, ms: i64, ns: i32) E!void {
    return rt.monitorWait(this, ms, ns);
}

// ------------------------------------------------------------------ java.lang.String
pub fn J_java_lang_String__intern(this: Ref) E!Ref {
    return rt.intern(rt.hooks.string_chars(try rt.nn(this)));
}

// ------------------------------------------------------------------ java.lang.Class
pub fn J_java_lang_Class__getName0(this: Ref) E!Ref {
    const c = cls(this);
    var buf: [1024]u8 = undefined;
    var fba = std.heap.FixedBufferAllocator.init(&buf);
    const name = c.name;
    if (c.kind == .array) {
        const d = dotted(fba.allocator(), name) catch return rt.throwKind(.OutOfMemory, null);
        return str(d);
    }
    const d = dotted(fba.allocator(), name) catch return rt.throwKind(.OutOfMemory, null);
    return str(d);
}
pub fn J_java_lang_Class__modifiers0(this: Ref) E!i32 {
    return @bitCast(cls(this).flags);
}
pub fn J_java_lang_Class__super0(this: Ref) E!Ref {
    const c = cls(this);
    if (c.kind == .interface or c.kind == .primitive) return null;
    const s = c.super orelse return null;
    return rt.classMirror(s);
}
pub fn J_java_lang_Class__component0(this: Ref) E!Ref {
    const c = cls(this);
    if (c.kind != .array) return null;
    return rt.classMirror(c.elem.?);
}
pub fn J_java_lang_Class__interfaces0(this: Ref) E!Ref {
    const c = cls(this);
    // direct interfaces are not tracked separately; report all (transitive) interfaces
    var R: [2]Ref = .{ this, null };
    var F = rt.Frame{ .refs = &R, .n = 2, .meth = &meta };
    try rt.pushFrame(&F);
    defer rt.popFrame(&F);
    const n: i32 = @intCast(c.interfaces.len);
    R[1] = try rt.newArray(rt.arrayOf(P.C_java_lang_Class.cls()), n);
    for (c.interfaces, 0..) |i, k| {
        const m = try rt.classMirror(i);
        try rt.aastore(R[1], @intCast(k), m);
    }
    return R[1];
}
pub fn J_java_lang_Class__isInstance0(this: Ref, o: Ref) E!i32 {
    return @intFromBool(rt.instanceOf(o, cls(this)));
}
pub fn J_java_lang_Class__isAssignableFrom0(this: Ref, other: Ref) E!i32 {
    const o = try rt.nn(other);
    const c = cls(this);
    const oc = rt.hooks.mirror_get_class(o);
    if (c.kind == .primitive or oc.kind == .primitive) return @intFromBool(c == oc);
    return @intFromBool(rt.isSubclass(oc, c));
}
pub fn J_java_lang_Class__kind0(this: Ref) E!i32 {
    return @intFromEnum(cls(this).kind);
}
pub fn J_java_lang_Class__enumValues0(this: Ref) E!Ref {
    const c = cls(this);
    try rt.ensureInit(c);
    const f = c.enum_values orelse return null;
    return rt.unR(Ref, f());
}
pub fn J_java_lang_Class__ensureInit(this: Ref) E!void {
    return rt.ensureInit(cls(this));
}
pub fn J_java_lang_Class__forName0(name: Ref) E!Ref {
    const chars = rt.hooks.string_chars(try rt.nn(name));
    var buf: [1024]u8 = undefined;
    if (chars.len > buf.len) return null;
    for (chars, 0..) |ch, i| buf[i] = if (ch == '.') '/' else if (ch < 128) @intCast(ch) else '?';
    const n = buf[0..chars.len];
    if (rt.findClass(n)) |c| return rt.classMirror(c);
    return null;
}
pub fn J_java_lang_Class__getPrimitiveClass(name: Ref) E!Ref {
    const chars = rt.hooks.string_chars(try rt.nn(name));
    const names = [_][]const u8{ "boolean", "byte", "char", "short", "int", "long", "float", "double", "void" };
    const codes = "ZBCSIJFDV";
    for (names, 0..) |n, i| {
        if (n.len == chars.len) {
            var eq = true;
            for (n, 0..) |ch, k| if (chars[k] != ch) {
                eq = false;
            };
            if (eq) return rt.classMirror(rt.primByChar(codes[i]));
        }
    }
    return null;
}

// ------------------------------------------------------------------ java.lang.System / Runtime
pub fn J_java_lang_System__write0(fd: i32, b: Ref, off: i32, len: i32) E!void {
    const a = rt.asArray(try rt.nn(b));
    if (off < 0 or len < 0 or @as(i64, off) + len > a.length) return rt.throwKind(.ArrayIndex, null);
    const data = rt.arrayData(u8, a)[@intCast(off)..@intCast(off + len)];
    rt.enterSafe();
    rt.writeFd(fd, data);
    rt.leaveSafe();
}
pub fn J_java_lang_System__read0(fd: i32, b: Ref, off: i32, len: i32) E!i32 {
    const a = rt.asArray(try rt.nn(b));
    if (off < 0 or len < 0 or @as(i64, off) + len > a.length) return rt.throwKind(.ArrayIndex, null);
    // read into a native buffer while in the safe state (the array may not be touched by GC,
    // it is not moved, but stay conservative and copy afterwards)
    var tmp: [8192]u8 = undefined;
    const want: usize = @min(@as(usize, @intCast(len)), tmp.len);
    rt.enterSafe();
    const n = std.posix.read(fd, tmp[0..want]) catch 0;
    rt.leaveSafe();
    @memcpy(rt.arrayData(u8, a)[@intCast(off)..][0..n], tmp[0..n]);
    return @intCast(n);
}
pub fn J_java_lang_System__currentTimeMillis() E!i64 {
    return std.time.milliTimestamp();
}
pub fn J_java_lang_System__nanoTime() E!i64 {
    const ts = std.posix.clock_gettime(.MONOTONIC) catch return 0;
    return @as(i64, ts.sec) * std.time.ns_per_s + ts.nsec;
}
pub fn J_java_lang_System__arraycopy(src: Ref, sp: i32, dst: Ref, dp: i32, len: i32) E!void {
    return rt.arraycopy(src, sp, dst, dp, len);
}
pub fn J_java_lang_System__identityHashCode(o: Ref) E!i32 {
    const obj = o orelse return 0;
    return rt.identityHash(obj);
}
pub fn J_java_lang_System__getenv0(name: Ref) E!Ref {
    const n = rt.stringToUtf8(rt.gpa, name) catch return rt.throwKind(.OutOfMemory, null);
    defer rt.gpa.free(n);
    const nz = rt.gpa.dupeZ(u8, n) catch return rt.throwKind(.OutOfMemory, null);
    defer rt.gpa.free(nz);
    const v = rt.getenv(nz) orelse return null;
    return str(v);
}
pub fn J_java_lang_System__env0() E!Ref {
    var count: usize = 0;
    count = rt.environList().len;
    var R: [2]Ref = .{ null, null };
    var F = rt.Frame{ .refs = &R, .n = 2, .meth = &meta };
    try rt.pushFrame(&F);
    defer rt.popFrame(&F);
    R[0] = try rt.newArray(rt.arrayOf(P.C_java_lang_String.cls()), @intCast(count * 2));
    for (rt.environList(), 0..) |e, i| {
        const s = std.mem.span(e);
        const eq = std.mem.indexOfScalar(u8, s, '=') orelse s.len;
        R[1] = try str(s[0..eq]);
        try rt.aastore(R[0], @intCast(2 * i), R[1]);
        R[1] = try str(if (eq < s.len) s[eq + 1 ..] else "");
        try rt.aastore(R[0], @intCast(2 * i + 1), R[1]);
    }
    return R[0];
}
pub fn J_java_lang_System__cwd0() E!Ref {
    var buf: [std.fs.max_path_bytes]u8 = undefined;
    const p = std.posix.getcwd(&buf) catch return str("/");
    return str(p);
}
pub fn J_java_lang_System__arch0() E!Ref {
    const builtin = @import("builtin");
    return str(switch (builtin.cpu.arch) {
        .x86_64 => "amd64",
        .aarch64 => "aarch64",
        .x86 => "i386",
        .arm => "arm",
        .riscv64 => "riscv64",
        else => @tagName(builtin.cpu.arch),
    });
}
pub fn J_java_lang_Runtime__halt0(status: i32) E!void {
    rt.exitProcess(status);
}
pub fn J_java_lang_Runtime__availableProcessors(this: Ref) E!i32 {
    _ = this;
    return @intCast(std.Thread.getCpuCount() catch 1);
}
pub fn J_java_lang_Runtime__freeMemory(this: Ref) E!i64 {
    _ = this;
    const total = @max(rt.heapBytes(), rt.usedBytes());
    return @intCast(total - rt.usedBytes());
}
pub fn J_java_lang_Runtime__totalMemory(this: Ref) E!i64 {
    _ = this;
    return @intCast(@max(rt.heapBytes(), rt.usedBytes()));
}
/// J2Z_HEAP_MAX_MB when set; otherwise the heap is bounded only by memory: report the physical memory.
pub fn J_java_lang_Runtime__maxMemory(this: Ref) E!i64 {
    _ = this;
    if (rt.st.gc_max_heap != 0) return @intCast(rt.st.gc_max_heap);
    const sys = @import("natives_sys.zig");
    const v = sys.J_jdk_internal_platform_CgroupMetrics__getTotalMemorySize0() catch 0;
    return if (v > 0) v else std.math.maxInt(i64);
}
pub fn J_java_lang_Runtime__gc(this: Ref) E!void {
    _ = this;
    rt.gc();
}

// ------------------------------------------------------------------ java.lang.Thread
fn threadState(t: *rt.Object) ?*rt.ThreadState {
    const v = P.C_java_lang_Thread.cast(t).f_eetop;
    if (v == 0) return null;
    return @ptrFromInt(@as(usize, @bitCast(v)));
}
pub fn J_java_lang_Thread__currentThread0() E!Ref {
    return rt.cur.java_thread;
}
pub fn J_java_lang_Thread__yield() E!void {
    std.Thread.yield() catch {};
    rt.safepoint();
}
pub fn J_java_lang_Thread__sleep0(nanos: i64) E!void {
    return rt.sleepNs(@intCast(@max(nanos, 0)));
}
pub fn J_java_lang_Thread__start0(this: Ref, stack: i64) E!void {
    return rt.startThread(this, stack);
}
pub fn J_java_lang_Thread__interrupt0(this: Ref) E!void {
    const ts = threadState(try rt.nn(this)) orelse return;
    ts.interrupted.store(true, .seq_cst);
    rt.unpark(ts); // like HotSpot: an interrupt makes LockSupport.park return
    // Thread.sleep waits on park_cond without touching park_state, so unpark() above never wakes it.
    // The flag is set before taking park_mutex and sleepNs re-checks it under that mutex: no lost wakeup.
    ts.park_mutex.lock();
    ts.park_cond.broadcast();
    ts.park_mutex.unlock();
    if (ts.sleeper) |m| {
        m.mutex.lock();
        m.cond.broadcast();
        m.mutex.unlock();
    }
}
pub fn J_java_lang_Thread__clearInterrupt0(this: Ref) E!i32 {
    const ts = threadState(try rt.nn(this)) orelse return 0;
    return @intFromBool(ts.interrupted.swap(false, .seq_cst));
}
pub fn J_java_lang_Thread__isInterrupted0(this: Ref) E!i32 {
    const ts = threadState(try rt.nn(this)) orelse return 0;
    return @intFromBool(ts.interrupted.load(.seq_cst));
}
pub fn J_java_lang_Thread__holdsLock(o: Ref) E!i32 {
    const obj = try rt.nn(o);
    const m = rt.monitorIfAny(obj) orelse return 0;
    return @intFromBool(m.owner == rt.cur);
}

// ------------------------------------------------------------------ java.lang.Throwable
pub fn J_java_lang_Throwable__backtrace0(this: Ref) E!Ref {
    const self = try rt.nn(this);
    // collect frames, skipping the native frame chain entries for <init>/fillInStackTrace of
    // this exception's class hierarchy (like HotSpot does)
    var frames: [1024]*const rt.MethodMeta = undefined;
    var lines: [1024]u32 = undefined;
    var n: usize = 0;
    var f = rt.cur.top;
    var skipping = true;
    while (f) |fr| : (f = fr.prev) {
        const m = fr.meth;
        if (std.mem.eql(u8, m.class, "<runtime>")) continue;
        if (std.mem.eql(u8, m.file, "<synthetic>")) continue; // lambda/indy proxies are hidden
        if (skipping) {
            if (std.mem.eql(u8, m.name, "fillInStackTrace") or std.mem.eql(u8, m.name, "backtrace0")) continue;
            if (std.mem.eql(u8, m.name, "<init>") and isThrowableClass(self.class, m.class)) continue;
            skipping = false;
        }
        if (n < frames.len) {
            frames[n] = m;
            lines[n] = fr.line;
            n += 1;
        }
    }
    var R: [2]Ref = .{ this, null };
    var F = rt.Frame{ .refs = &R, .n = 2, .meth = &meta };
    try rt.pushFrame(&F);
    defer rt.popFrame(&F);
    const arr = try rt.newArray(rt.arrayOf(P.C_java_lang_String.cls()), @intCast(n * 4));
    R[1] = arr;
    var buf: [1024]u8 = undefined;
    for (frames[0..n], 0..) |m, i| {
        var fba = std.heap.FixedBufferAllocator.init(&buf);
        const cn = dotted(fba.allocator(), m.class) catch m.class;
        try rt.aastore(R[1], @intCast(4 * i), try str(cn));
        try rt.aastore(R[1], @intCast(4 * i + 1), try str(m.name));
        try rt.aastore(R[1], @intCast(4 * i + 2), try str(m.file));
        var lb: [16]u8 = undefined;
        const ls = std.fmt.bufPrint(&lb, "{d}", .{@as(i32, if (lines[i] == 0) -1 else @intCast(lines[i]))}) catch "-1";
        try rt.aastore(R[1], @intCast(4 * i + 3), try str(ls));
    }
    return R[1];
}

fn isThrowableClass(c: *rt.Class, name: []const u8) bool {
    var s: ?*rt.Class = c;
    while (s) |x| : (s = x.super) if (std.mem.eql(u8, x.name, name)) return true;
    return false;
}

const meta = rt.MethodMeta{ .class = "<runtime>", .name = "<native>", .desc = "" };

// ------------------------------------------------------------------ java.lang.Float / Double
pub fn J_java_lang_Float__floatToRawIntBits(v: f32) E!i32 {
    return @bitCast(v);
}
pub fn J_java_lang_Float__intBitsToFloat(v: i32) E!f32 {
    return @bitCast(v);
}
pub fn J_java_lang_Double__doubleToRawLongBits(v: f64) E!i64 {
    return @bitCast(v);
}
pub fn J_java_lang_Double__longBitsToDouble(v: i64) E!f64 {
    return @bitCast(v);
}

// ------------------------------------------------------------------ java.lang.reflect.Array
pub fn J_java_lang_reflect_Array__newArray(component: Ref, len: i32) E!Ref {
    const c = cls(try rt.nn(component));
    if (c == rt.prim_V) return rt.throwKind(.IllegalArgument, null);
    return rt.newArray(rt.arrayOf(c), len);
}
pub fn J_java_lang_reflect_Array__getLength(a: Ref) E!i32 {
    const o = try rt.nn(a);
    if (o.class.kind != .array) return rt.throwKind(.IllegalArgument, "Argument is not an array");
    return rt.asArray(o).length;
}
pub fn J_java_lang_reflect_Array__multiNewArray(component: Ref, dims: Ref) E!Ref {
    const c = cls(try rt.nn(component));
    const d = rt.asArray(try rt.nn(dims));
    const n: usize = @intCast(d.length);
    if (n == 0 or n > 255) return rt.throwKind(.IllegalArgument, null);
    var ac = c;
    for (0..n) |_| ac = rt.arrayOf(ac);
    return rt.multiNewArray(ac, rt.arrayData(i32, d)[0..n]);
}

// ------------------------------------------------------------------ atomics
pub fn J_java_util_concurrent_atomic_AtomicInteger__cas(this: Ref, e: i32, n: i32) E!i32 {
    const p = &P.C_java_util_concurrent_atomic_AtomicInteger.cast(try rt.nn(this)).f_value;
    return @intFromBool(@cmpxchgStrong(i32, p, e, n, .seq_cst, .seq_cst) == null);
}
pub fn J_java_util_concurrent_atomic_AtomicLong__cas(this: Ref, e: i64, n: i64) E!i32 {
    const p = &P.C_java_util_concurrent_atomic_AtomicLong.cast(try rt.nn(this)).f_value;
    return @intFromBool(@cmpxchgStrong(i64, p, e, n, .seq_cst, .seq_cst) == null);
}
pub fn J_java_util_concurrent_atomic_AtomicBoolean__cas(this: Ref, e: i32, n: i32) E!i32 {
    const p = &P.C_java_util_concurrent_atomic_AtomicBoolean.cast(try rt.nn(this)).f_value;
    return @intFromBool(@cmpxchgStrong(i32, p, e, n, .seq_cst, .seq_cst) == null);
}
pub fn J_java_util_concurrent_atomic_AtomicReference__cas(this: Ref, e: Ref, n: Ref) E!i32 {
    const p = &P.C_java_util_concurrent_atomic_AtomicReference.cast(try rt.nn(this)).f_value;
    return @intFromBool(@cmpxchgStrong(Ref, p, e, n, .seq_cst, .seq_cst) == null);
}

// ------------------------------------------------------------------ java.lang.NullPointerException
/// HotSpot computes "helpful" NPE messages from bytecode; the native image reports none.
pub fn J_java_lang_NullPointerException__getExtendedNPEMessage(this: Ref) E!Ref {
    _ = this;
    return null;
}

// ------------------------------------------------------------------ jdk.internal.misc.Unsafe
// Memory model: (o, offset) addresses o + offset when o != null (instance fields use the
// generated Layout offsets, array elements use @sizeOf(rt.Array) + index * scale), and an
// absolute native address when o == null.
inline fn addrU(comptime T: type, o: Ref, off: i64) *align(1) T {
    const base: usize = if (o) |obj| @intFromPtr(obj) else 0;
    return @ptrFromInt(base +% @as(usize, @bitCast(off)));
}
inline fn addr(comptime T: type, o: Ref, off: i64) *T {
    const base: usize = if (o) |obj| @intFromPtr(obj) else 0;
    return @ptrFromInt(base +% @as(usize, @bitCast(off)));
}
pub fn J_jdk_internal_misc_Unsafe__registerNatives() E!void {}
pub fn J_jdk_internal_misc_Unsafe__arrayBaseOffset0(this: Ref, c: Ref) E!i32 {
    _ = this;
    _ = c;
    return @sizeOf(rt.Array);
}
pub fn J_jdk_internal_misc_Unsafe__arrayIndexScale0(this: Ref, c: Ref) E!i32 {
    _ = this;
    const k = cls(try rt.nn(c));
    if (k.kind != .array) return rt.throwKind(.IllegalArgument, null);
    return @intCast(k.elem_size);
}
pub fn J_jdk_internal_misc_Unsafe__objectFieldOffset1(this: Ref, c: Ref, name: Ref) E!i64 {
    _ = this;
    var k: ?*rt.Class = cls(try rt.nn(c));
    const n = rt.hooks.string_chars(try rt.nn(name));
    while (k) |kk| : (k = kk.super) {
        for (kk.fields) |f| {
            if (f.name.len != n.len) continue;
            var eq = true;
            for (f.name, 0..) |ch, i| if (n[i] != ch) {
                eq = false;
                break;
            };
            if (eq) return f.offset;
        }
    }
    return rt.throwKind(.InternalError, "Unsafe.objectFieldOffset: no such field");
}
pub fn J_jdk_internal_misc_Unsafe__shouldBeInitialized0(this: Ref, c: Ref) E!i32 {
    _ = this;
    return @intFromBool(cls(try rt.nn(c)).init_state != rt.INIT_DONE);
}
pub fn J_jdk_internal_misc_Unsafe__ensureClassInitialized0(this: Ref, c: Ref) E!void {
    _ = this;
    return rt.ensureInit(cls(try rt.nn(c)));
}
pub fn J_jdk_internal_misc_Unsafe__allocateInstance(this: Ref, c: Ref) E!Ref {
    _ = this;
    const k = cls(try rt.nn(c));
    if (k.kind != .instance or k.flags & 0x0400 != 0) return rt.throwKind(.Instantiation, k.name);
    try rt.ensureInit(k);
    return rt.allocObject(k, k.size);
}
pub fn J_jdk_internal_misc_Unsafe__allocateMemory0(this: Ref, n: i64) E!i64 {
    _ = this;
    const sz: usize = @intCast(@max(n, 1));
    const mem = std.c.malloc(sz) orelse return rt.throwKind(.OutOfMemory, "Unable to allocate native memory");
    return @bitCast(@as(u64, @intFromPtr(mem)));
}
pub fn J_jdk_internal_misc_Unsafe__reallocateMemory0(this: Ref, a: i64, n: i64) E!i64 {
    _ = this;
    const p: ?*anyopaque = if (a == 0) null else @ptrFromInt(@as(usize, @bitCast(a)));
    const mem = std.c.realloc(p, @intCast(@max(n, 1))) orelse return rt.throwKind(.OutOfMemory, "Unable to allocate native memory");
    return @bitCast(@as(u64, @intFromPtr(mem)));
}
pub fn J_jdk_internal_misc_Unsafe__freeMemory0(this: Ref, a: i64) E!void {
    _ = this;
    if (a != 0) std.c.free(@ptrFromInt(@as(usize, @bitCast(a))));
}
pub fn J_jdk_internal_misc_Unsafe__setMemory0(this: Ref, o: Ref, off: i64, n: i64, v: i32) E!void {
    _ = this;
    const p: [*]u8 = @ptrCast(addr(u8, o, off));
    @memset(p[0..@intCast(n)], @as(u8, @truncate(@as(u32, @bitCast(v)))));
}
pub fn J_jdk_internal_misc_Unsafe__copyMemory0(this: Ref, so: Ref, soff: i64, d: Ref, doff: i64, n: i64) E!void {
    _ = this;
    const sp: [*]u8 = @ptrCast(addr(u8, so, soff));
    const dp: [*]u8 = @ptrCast(addr(u8, d, doff));
    const len: usize = @intCast(n);
    if (@intFromPtr(dp) > @intFromPtr(sp)) std.mem.copyBackwards(u8, dp[0..len], sp[0..len]) else std.mem.copyForwards(u8, dp[0..len], sp[0..len]);
}
pub fn J_jdk_internal_misc_Unsafe__copySwapMemory0(this: Ref, so: Ref, soff: i64, d: Ref, doff: i64, n: i64, esz: i64) E!void {
    _ = this;
    const sp: [*]u8 = @ptrCast(addr(u8, so, soff));
    const dp: [*]u8 = @ptrCast(addr(u8, d, doff));
    const len: usize = @intCast(n);
    const es: usize = @intCast(esz);
    var i: usize = 0;
    while (i < len) : (i += es) {
        var k: usize = 0;
        while (k < es) : (k += 1) dp[i + k] = sp[i + es - 1 - k];
    }
}
pub fn J_jdk_internal_misc_Unsafe__fullFence(this: Ref) E!void {
    _ = this;
    var x = std.atomic.Value(u32).init(0);
    _ = x.fetchAdd(0, .seq_cst);
}
pub fn J_jdk_internal_misc_Unsafe__throwException(this: Ref, t: Ref) E!void {
    _ = this;
    try rt.athrow(t);
}
pub fn J_jdk_internal_misc_Unsafe__park(this: Ref, absolute: i32, time: i64) E!void {
    _ = this;
    rt.park(absolute != 0, time);
}
pub fn J_jdk_internal_misc_Unsafe__unpark(this: Ref, t: Ref) E!void {
    _ = this;
    const obj = t orelse return;
    const ts = threadState(obj) orelse return;
    rt.unpark(ts);
}
pub fn J_jdk_internal_misc_Unsafe__getLoadAverage0(this: Ref, a: Ref, n: i32) E!i32 {
    _ = this;
    _ = a;
    _ = n;
    return -1;
}

fn UnsafeAccessors(comptime T: type, comptime Ret: type) type {
    return struct {
        fn conv(v: T) Ret {
            return if (Ret == T) v else @as(Ret, v);
        }
        fn back(v: Ret) T {
            if (Ret == T) return v;
            return switch (@typeInfo(T)) {
                .int => |ii| if (ii.signedness == .unsigned) @truncate(@as(u32, @bitCast(v))) else @truncate(v),
                else => v,
            };
        }
        pub fn get(this: Ref, o: Ref, off: i64) E!Ret {
            _ = this;
            return conv(addrU(T, o, off).*);
        }
        pub fn getVolatile(this: Ref, o: Ref, off: i64) E!Ret {
            _ = this;
            return conv(@atomicLoad(T, addr(T, o, off), .seq_cst));
        }
        pub fn put(this: Ref, o: Ref, off: i64, v: Ret) E!void {
            _ = this;
            addrU(T, o, off).* = back(v);
        }
        pub fn putVolatile(this: Ref, o: Ref, off: i64, v: Ret) E!void {
            _ = this;
            @atomicStore(T, addr(T, o, off), back(v), .seq_cst);
        }
    };
}
const UI = UnsafeAccessors(i32, i32);
const UJ = UnsafeAccessors(i64, i64);
const UB = UnsafeAccessors(i8, i32);
const US = UnsafeAccessors(i16, i32);
const UC = UnsafeAccessors(u16, i32);
const UF = UnsafeAccessors(f32, f32);
const UD = UnsafeAccessors(f64, f64);
const UR = UnsafeAccessors(Ref, Ref);
pub const J_jdk_internal_misc_Unsafe__getInt = UI.get;
pub const J_jdk_internal_misc_Unsafe__getIntVolatile = UI.getVolatile;
pub const J_jdk_internal_misc_Unsafe__putInt = UI.put;
pub const J_jdk_internal_misc_Unsafe__putIntVolatile = UI.putVolatile;
pub const J_jdk_internal_misc_Unsafe__getLong = UJ.get;
pub const J_jdk_internal_misc_Unsafe__getLongVolatile = UJ.getVolatile;
pub const J_jdk_internal_misc_Unsafe__putLong = UJ.put;
pub const J_jdk_internal_misc_Unsafe__putLongVolatile = UJ.putVolatile;
pub const J_jdk_internal_misc_Unsafe__getByte = UB.get;
pub const J_jdk_internal_misc_Unsafe__getByteVolatile = UB.getVolatile;
pub const J_jdk_internal_misc_Unsafe__putByte = UB.put;
pub const J_jdk_internal_misc_Unsafe__putByteVolatile = UB.putVolatile;
pub const J_jdk_internal_misc_Unsafe__getBoolean = UB.get;
pub const J_jdk_internal_misc_Unsafe__getBooleanVolatile = UB.getVolatile;
pub const J_jdk_internal_misc_Unsafe__putBoolean = UB.put;
pub const J_jdk_internal_misc_Unsafe__putBooleanVolatile = UB.putVolatile;
pub const J_jdk_internal_misc_Unsafe__getShort = US.get;
pub const J_jdk_internal_misc_Unsafe__getShortVolatile = US.getVolatile;
pub const J_jdk_internal_misc_Unsafe__putShort = US.put;
pub const J_jdk_internal_misc_Unsafe__putShortVolatile = US.putVolatile;
pub const J_jdk_internal_misc_Unsafe__getChar = UC.get;
pub const J_jdk_internal_misc_Unsafe__getCharVolatile = UC.getVolatile;
pub const J_jdk_internal_misc_Unsafe__putChar = UC.put;
pub const J_jdk_internal_misc_Unsafe__putCharVolatile = UC.putVolatile;
pub const J_jdk_internal_misc_Unsafe__getFloat = UF.get;
pub const J_jdk_internal_misc_Unsafe__getFloatVolatile = UF.getVolatile;
pub const J_jdk_internal_misc_Unsafe__putFloat = UF.put;
pub const J_jdk_internal_misc_Unsafe__putFloatVolatile = UF.putVolatile;
pub const J_jdk_internal_misc_Unsafe__getDouble = UD.get;
pub const J_jdk_internal_misc_Unsafe__getDoubleVolatile = UD.getVolatile;
pub const J_jdk_internal_misc_Unsafe__putDouble = UD.put;
pub const J_jdk_internal_misc_Unsafe__putDoubleVolatile = UD.putVolatile;
pub const J_jdk_internal_misc_Unsafe__getReference = UR.get;
pub const J_jdk_internal_misc_Unsafe__getReferenceVolatile = UR.getVolatile;
pub const J_jdk_internal_misc_Unsafe__putReference = UR.put;
pub const J_jdk_internal_misc_Unsafe__putReferenceVolatile = UR.putVolatile;

fn CasOps(comptime T: type) type {
    return struct {
        pub fn cas(this: Ref, o: Ref, off: i64, e: T, n: T) E!i32 {
            _ = this;
            return @intFromBool(@cmpxchgStrong(T, addr(T, o, off), e, n, .seq_cst, .seq_cst) == null);
        }
        pub fn cax(this: Ref, o: Ref, off: i64, e: T, n: T) E!T {
            _ = this;
            return @cmpxchgStrong(T, addr(T, o, off), e, n, .seq_cst, .seq_cst) orelse e;
        }
    };
}
pub const J_jdk_internal_misc_Unsafe__compareAndSetInt = CasOps(i32).cas;
pub const J_jdk_internal_misc_Unsafe__compareAndSetLong = CasOps(i64).cas;
pub const J_jdk_internal_misc_Unsafe__compareAndSetReference = CasOps(Ref).cas;
pub const J_jdk_internal_misc_Unsafe__compareAndExchangeInt = CasOps(i32).cax;
pub const J_jdk_internal_misc_Unsafe__compareAndExchangeLong = CasOps(i64).cax;
pub const J_jdk_internal_misc_Unsafe__compareAndExchangeReference = CasOps(Ref).cax;

// ------------------------------------------------------------------ java.security.SecureRandom
pub fn J_java_security_SecureRandom__fill0(b: Ref) E!void {
    const a = rt.asArray(try rt.nn(b));
    const data = rt.arrayData(u8, a)[0..@intCast(a.length)];
    std.crypto.random.bytes(data);
}

// ------------------------------------------------------------------ jdk.internal.reflect.Reflection
/// Class of the method that called the method calling getCallerClass (frame 0 is that method,
/// since native wrappers do not push frames; runtime and synthetic proxy frames are skipped).
pub fn J_jdk_internal_reflect_Reflection__getCallerClass() E!Ref {
    var f = rt.cur.top;
    var depth: u32 = 0;
    while (f) |fr| : (f = fr.prev) {
        const c = fr.meth.cls orelse continue;
        if (std.mem.eql(u8, fr.meth.file, "<synthetic>")) continue;
        if (depth == 1) return rt.classMirror(c);
        depth += 1;
    }
    return null;
}
pub fn J_jdk_internal_reflect_Reflection__getClassAccessFlags(c: Ref) E!i32 {
    return @bitCast(cls(try rt.nn(c)).flags);
}
pub fn J_jdk_internal_reflect_Reflection__areNestMates(a: Ref, b: Ref) E!i32 {
    _ = a;
    _ = b;
    return 1;
}

// ------------------------------------------------------------------ minimal java.lang.reflect.Field
pub fn classForDesc(desc: []const u8) ?*rt.Class {
    if (desc.len == 0) return null;
    switch (desc[0]) {
        '[' => return if (classForDesc(desc[1..])) |e| rt.arrayOf(e) else null,
        'L' => {
            const n = desc[1 .. desc.len - 1];
            return rt.findClass(n) orelse rt.hooks.object_class;
        },
        else => return rt.primByChar(desc[0]),
    }
}

fn eqlName(a: []const u8, b: []const u16) bool {
    if (a.len != b.len) return false;
    for (a, 0..) |ch, i| if (b[i] != ch) return false;
    return true;
}

pub fn J_java_lang_Class__newField0(this: Ref, name: Ref) E!Ref {
    const c = cls(try rt.nn(this));
    const n = rt.hooks.string_chars(try rt.nn(name));
    var desc: ?[]const u8 = null;
    var flags: u32 = 0;
    var sig: ?[]const u8 = null;
    var ann: ?[]const u8 = null;
    for (c.fields) |f| if (eqlName(f.name, n)) {
        desc = f.desc;
        flags = f.flags;
        sig = f.signature;
        ann = f.annotations;
    };
    if (desc == null) for (c.static_fields) |f| if (eqlName(f.name, n)) {
        desc = f.desc;
        flags = f.flags;
        sig = f.signature;
        ann = f.annotations;
    };
    const d = desc orelse return null;
    var R: [5]Ref = .{ this, name, null, null, null };
    var F = rt.Frame{ .refs = &R, .n = 5, .meth = &meta };
    try rt.pushFrame(&F);
    defer rt.popFrame(&F);
    const t = classForDesc(d) orelse rt.hooks.object_class;
    R[2] = try rt.classMirror(t);
    R[3] = try optString(sig);
    R[4] = try optBytes(ann);
    const o = try rt.newObject(P.C_java_lang_reflect_Field.cls());
    const fo = P.C_java_lang_reflect_Field.cast(o.?);
    fo.f_clazz = R[0];
    fo.f_name = R[1];
    fo.f_type = R[2];
    fo.f_modifiers = @intCast(flags & 0xffff);
    fo.f_signature = R[3];
    fo.f_annotations = R[4];
    return o;
}

fn fieldTarget(f: Ref) E!struct { c: *rt.Class, name: []const u16, static: bool } {
    const fo = P.C_java_lang_reflect_Field.cast(try rt.nn(f));
    return .{ .c = cls(fo.f_clazz), .name = rt.hooks.string_chars(fo.f_name.?), .static = fo.f_modifiers & 0x8 != 0 };
}

pub fn J_jdk_internal_misc_Unsafe__objectFieldOffset0(this: Ref, f: Ref) E!i64 {
    _ = this;
    const t = try fieldTarget(f);
    var k: ?*rt.Class = t.c;
    while (k) |kk| : (k = kk.super) for (kk.fields) |fm| if (eqlName(fm.name, t.name)) return fm.offset;
    return rt.throwKind(.InternalError, "Unsafe.objectFieldOffset: no such field");
}
pub fn J_jdk_internal_misc_Unsafe__staticFieldOffset0(this: Ref, f: Ref) E!i64 {
    _ = this;
    const t = try fieldTarget(f);
    for (t.c.static_fields) |fm| if (eqlName(fm.name, t.name)) return @bitCast(@as(u64, @intFromPtr(fm.ptr)));
    return rt.throwKind(.InternalError, "Unsafe.staticFieldOffset: no such field");
}
/// Static fields are addressed absolutely (base == null, offset == address).
pub fn J_jdk_internal_misc_Unsafe__staticFieldBase0(this: Ref, f: Ref) E!Ref {
    _ = this;
    _ = f;
    return null;
}

// ------------------------------------------------------------------ jdk.internal.misc.ScopedMemoryAccess
pub fn J_jdk_internal_misc_ScopedMemoryAccess__registerNatives() E!void {}
/// No thread can be inside a scoped access of a closing shared session in this runtime's
/// model (no asynchronous handshakes): closing always succeeds.
/// JDK 25: HotSpot handshakes all threads and throws `error` if one is inside a scoped access of
/// the closing shared session. The native image does not track scoped accesses per thread, so a
/// concurrent close is not detected here (the access then sees the closed session's state).
pub fn J_jdk_internal_misc_ScopedMemoryAccess__closeScope0(this: Ref, session: Ref, err: Ref) E!void {
    _ = this;
    _ = session;
    _ = err;
}

// ------------------------------------------------------------------ java.util.TimeZone (same lookup order as the JDK on Linux)
pub fn J_java_util_TimeZone__getSystemTimeZoneID(java_home: Ref) E!Ref {
    _ = java_home;
    if (rt.getenv("TZ")) |tz0| {
        var tz = tz0;
        if (tz.len > 0 and tz[0] == ':') tz = tz[1..];
        if (std.mem.startsWith(u8, tz, "/usr/share/zoneinfo/")) tz = tz["/usr/share/zoneinfo/".len..];
        if (tz.len > 0) return str(tz);
    }
    var buf: [256]u8 = undefined;
    if (std.fs.cwd().readFile("/etc/timezone", &buf)) |content| {
        const t = std.mem.trim(u8, content, " \t\r\n");
        if (t.len > 0) return str(t);
    } else |_| {}
    var lb: [std.fs.max_path_bytes]u8 = undefined;
    if (std.posix.readlink("/etc/localtime", &lb)) |target| {
        if (std.mem.indexOf(u8, target, "zoneinfo/")) |i| return str(target[i + "zoneinfo/".len ..]);
    } else |_| {}
    return null;
}
pub fn J_java_util_TimeZone__getSystemGMTOffsetID() E!Ref {
    return str("GMT");
}
/// Directory of the running executable (used as java.home; holds lib/tzdb.dat).
pub fn J_java_lang_System__exeDir0() E!Ref {
    var buf: [std.fs.max_path_bytes]u8 = undefined;
    const p = std.fs.selfExePath(&buf) catch return str("/");
    return str(std.fs.path.dirname(p) orelse "/");
}

// ------------------------------------------------------------------ java.util.ServiceLoader (build-time provider table)
pub fn J_java_util_ServiceLoader__providers0(name: Ref) E!Ref {
    const n = rt.hooks.string_chars(try rt.nn(name));
    var count: usize = 0;
    for (P.services) |e| if (eqlName(e.service, n)) {
        count += 1;
    };
    var R: [2]Ref = .{ name, null };
    var F = rt.Frame{ .refs = &R, .n = 2, .meth = &meta };
    try rt.pushFrame(&F);
    defer rt.popFrame(&F);
    R[1] = try rt.newArray(rt.arrayOf(rt.hooks.object_class), @intCast(count));
    var i: i32 = 0;
    for (P.services) |e| if (eqlName(e.service, n)) {
        const o = try e.create();
        try rt.aastore(R[1], i, o);
        i += 1;
    };
    return R[1];
}

// ------------------------------------------------------------------ LWJGL upcalls (libffi closures)
/// libffi closure handler: user_data is the JNI global reference of the CallbackI object.
fn lwjglHandler(cif: ?*anyopaque, ret: ?*anyopaque, args: ?*anyopaque, user: ?*anyopaque) callconv(.c) void {
    _ = cif;
    const jni = @import("jni.zig");
    _ = jni.env(); // attaches a foreign (native-created) thread on first use
    rt.leaveSafe();
    P.lwjglCallback(@ptrCast(@alignCast(user)), @bitCast(@as(u64, @intFromPtr(ret))), @bitCast(@as(u64, @intFromPtr(args)))) catch {
        const ex = rt.takePending();
        rt.hooks.thread_uncaught(rt.cur.java_thread, ex) catch {
            _ = rt.takePending();
        };
    };
    rt.enterSafeFromCallback();
}
pub fn J_org_lwjgl_system_Upcalls__getCallbackHandler(m: Ref) E!i64 {
    _ = m;
    return @bitCast(@as(u64, @intFromPtr(&lwjglHandler)));
}
pub fn J_org_lwjgl_system_MemoryUtil__memGlobalRefToObject(ref: i64) E!Ref {
    return @ptrFromInt(@as(usize, @bitCast(ref)));
}

// ------------------------------------------------------------------ reflection: declared fields / methods
/// Parameter descriptors of a method descriptor, e.g. "(I[JLjava/lang/String;)V".
fn paramDescs(desc: []const u8, out: *[64][]const u8) usize {
    var n: usize = 0;
    var i: usize = 1;
    while (i < desc.len and desc[i] != ')') {
        const start = i;
        while (desc[i] == '[') i += 1;
        if (desc[i] == 'L') {
            while (desc[i] != ';') i += 1;
        }
        i += 1;
        if (n < out.len) {
            out[n] = desc[start..i];
            n += 1;
        }
    }
    return n;
}

fn mirrorForDesc(d: []const u8) E!Ref {
    return rt.classMirror(classForDesc(d) orelse rt.hooks.object_class);
}

pub fn J_java_lang_Class__fields0(this: Ref) E!Ref {
    const c = cls(try rt.nn(this));
    const n = c.fields.len + c.static_fields.len;
    var R: [4]Ref = .{ this, null, null, null };
    var F = rt.Frame{ .refs = &R, .n = 4, .meth = &meta };
    try rt.pushFrame(&F);
    defer rt.popFrame(&F);
    R[1] = try rt.newArray(rt.arrayOf(rt.hooks.object_class), @intCast(n));
    var k: i32 = 0;
    for (c.fields) |f| {
        R[2] = try rt.newStringUtf8(f.name);
        R[3] = try J_java_lang_Class__newField0(R[0], R[2]);
        try rt.aastore(R[1], k, R[3]);
        k += 1;
    }
    for (c.static_fields) |f| {
        R[2] = try rt.newStringUtf8(f.name);
        R[3] = try J_java_lang_Class__newField0(R[0], R[2]);
        try rt.aastore(R[1], k, R[3]);
        k += 1;
    }
    return R[1];
}

pub fn J_java_lang_Class__methods0(this: Ref, ctors: i32) E!Ref {
    const c = cls(try rt.nn(this));
    var count: usize = 0;
    for (c.methods) |m| {
        if ((ctors != 0) == std.mem.eql(u8, m.name, "<init>")) count += 1;
    }
    var R: [10]Ref = @splat(null);
    R[0] = this;
    var F = rt.Frame{ .refs = &R, .n = 10, .meth = &meta };
    try rt.pushFrame(&F);
    defer rt.popFrame(&F);
    R[1] = try rt.newArray(rt.arrayOf(rt.hooks.object_class), @intCast(count));
    const class_arr = rt.arrayOf(P.C_java_lang_Class.cls());
    var k: i32 = 0;
    for (c.methods, 0..) |m, slot| {
        const is_ctor = std.mem.eql(u8, m.name, "<init>");
        if ((ctors != 0) != is_ctor) continue;
        var ps: [64][]const u8 = undefined;
        const np = paramDescs(m.desc, &ps);
        R[2] = try rt.newArray(class_arr, @intCast(np));
        for (ps[0..np], 0..) |pd, i| try rt.aastore(R[2], @intCast(i), try mirrorForDesc(pd));
        R[3] = try rt.newArray(class_arr, @intCast(m.exceptions.len));
        for (m.exceptions, 0..) |en, i| {
            const ec = classByName(en) orelse continue;
            try rt.aastore(R[3], @intCast(i), try rt.classMirror(ec));
        }
        R[6] = try optString(m.signature);
        R[7] = try optBytes(m.annotations);
        R[8] = try optBytes(m.param_annotations);
        R[9] = try optBytes(m.annotation_default);
        if (is_ctor) {
            R[4] = try rt.newObject(P.C_java_lang_reflect_Constructor.cls());
            const o = P.C_java_lang_reflect_Constructor.cast(R[4].?);
            o.f_clazz = R[0];
            o.f_slot = @intCast(slot);
            o.f_parameterTypes = R[2];
            o.f_exceptionTypes = R[3];
            o.f_modifiers = @intCast(m.flags & 0xffff);
            o.f_signature = R[6];
            o.f_annotations = R[7];
            o.f_parameterAnnotations = R[8];
        } else {
            R[5] = try rt.newStringUtf8(m.name);
            R[4] = try rt.newObject(P.C_java_lang_reflect_Method.cls());
            const o = P.C_java_lang_reflect_Method.cast(R[4].?);
            o.f_clazz = R[0];
            o.f_slot = @intCast(slot);
            o.f_name = try rt.intern(rt.hooks.string_chars(R[5].?));
            o.f_parameterTypes = R[2];
            o.f_exceptionTypes = R[3];
            o.f_modifiers = @intCast(m.flags & 0xffff);
            const rd = m.desc[(std.mem.indexOfScalar(u8, m.desc, ')') orelse 0) + 1 ..];
            o.f_returnType = try mirrorForDesc(rd);
            o.f_signature = R[6];
            o.f_annotations = R[7];
            o.f_parameterAnnotations = R[8];
            o.f_annotationDefault = R[9];
        }
        try rt.aastore(R[1], k, R[4]);
        k += 1;
    }
    return R[1];
}

// ------------------------------------------------------------------ reflective invocation
pub fn findTarget(decl: *rt.Class, slot: usize, recv: Ref, dispatch: bool) E!*const rt.MethodInfoMeta {
    const m = &decl.methods[slot];
    if (dispatch and m.flags & 0x000A == 0) { // not static, not private: virtual dispatch
        if (recv) |o| {
            var k: ?*rt.Class = o.class;
            while (k) |kk| : (k = kk.super) {
                for (kk.methods) |*km| {
                    if (km.fp != null and (km.flags & 0x000A == 0) and std.mem.eql(u8, km.name, m.name) and std.mem.eql(u8, km.desc, m.desc)) return km;
                }
            }
            var best_iface: ?*rt.Class = null;
            var best_m: ?*const rt.MethodInfoMeta = null;
            for (o.class.interfaces) |ic| {
                for (ic.methods) |*km| {
                    if (km.fp != null and (km.flags & 0x040A == 0) and std.mem.eql(u8, km.name, m.name) and std.mem.eql(u8, km.desc, m.desc)) {
                        if (best_iface == null or rt.isSubclass(ic, best_iface.?)) {
                            best_iface = ic;
                            best_m = km;
                        }
                    }
                }
            }
            if (best_m) |bm| return bm;
        }
    }
    if (m.fp == null) {
        var buf: [512]u8 = undefined;
        return rt.throwKind(.UnsupportedOperation, std.fmt.bufPrint(&buf, "reflective target was not compiled (not reachable at build time): {s}.{s}{s}", .{ decl.name, m.name, m.desc }) catch "reflective target not compiled");
    }
    return m;
}

pub fn J_jdk_internal_reflect_MethodHandleAccessorFactory__invoke0(m: Ref, recv: Ref, args: Ref) E!Ref {
    const mo = P.C_java_lang_reflect_Method.cast(try rt.nn(m));
    const decl = cls(mo.f_clazz);
    const t = try findTarget(decl, @intCast(mo.f_slot), recv, true);
    return P.reflectInvoke(t.shape, t.fp.?, recv, args);
}

pub fn J_jdk_internal_reflect_MethodHandleAccessorFactory__newInstance0(c: Ref, args: Ref) E!Ref {
    const co = P.C_java_lang_reflect_Constructor.cast(try rt.nn(c));
    const decl = cls(co.f_clazz);
    const t = try findTarget(decl, @intCast(co.f_slot), null, false);
    var R: [2]Ref = .{ args, null };
    var F = rt.Frame{ .refs = &R, .n = 2, .meth = &meta };
    try rt.pushFrame(&F);
    defer rt.popFrame(&F);
    R[1] = try rt.newObject(decl);
    _ = try P.reflectInvoke(t.shape, t.fp.?, R[1], R[0]);
    return R[1];
}

// ------------------------------------------------------------------ class metadata (nesting, generics, annotations)
fn classByName(n: []const u8) ?*rt.Class {
    return rt.findClass(n);
}

fn optString(s: ?[]const u8) E!Ref {
    return if (s) |x| try rt.newStringUtf8(x) else null;
}

fn optBytes(b: ?[]const u8) E!Ref {
    const x = b orelse return null;
    const a = try rt.newArray(rt.arrayOf(rt.primByChar('B')), @intCast(x.len));
    @memcpy(rt.arrayData(u8, rt.asArray(a.?))[0..x.len], x);
    return a;
}

fn stringArray(items: []const ?[]const u8) E!Ref {
    var R: [2]Ref = .{ null, null };
    var F = rt.Frame{ .refs = &R, .n = 2, .meth = &meta };
    try rt.pushFrame(&F);
    defer rt.popFrame(&F);
    R[0] = try rt.newArray(rt.arrayOf(P.C_java_lang_String.cls()), @intCast(items.len));
    for (items, 0..) |it, i| {
        R[1] = try optString(it);
        try rt.aastore(R[0], @intCast(i), R[1]);
    }
    return R[0];
}

fn reflOf(this: Ref) E!?*const rt.ClassReflect {
    return cls(try rt.nn(this)).refl;
}

pub fn J_java_lang_Class__innerFlags0(this: Ref) E!i32 {
    const r = (try reflOf(this)) orelse return -1;
    return if (r.is_inner) @intCast(r.inner_flags) else -1;
}
pub fn J_java_lang_Class__outerName0(this: Ref) E!Ref {
    const r = (try reflOf(this)) orelse return null;
    return optString(r.outer);
}
pub fn J_java_lang_Class__innerName0(this: Ref) E!Ref {
    const r = (try reflOf(this)) orelse return null;
    return optString(r.inner_name);
}
pub fn J_java_lang_Class__enclosingMethod0(this: Ref) E!Ref {
    const r = (try reflOf(this)) orelse return null;
    const ec = r.enclosing_class orelse return null;
    return stringArray(&.{ ec, r.enclosing_name, r.enclosing_desc });
}
pub fn J_java_lang_Class__declaredClasses0(this: Ref) E!Ref {
    const r = try reflOf(this);
    const names: []const []const u8 = if (r) |x| x.declared_classes else &.{};
    var buf: [256]?[]const u8 = undefined;
    const n = @min(names.len, buf.len);
    for (names[0..n], 0..) |x, i| buf[i] = x;
    return stringArray(buf[0..n]);
}
pub fn J_java_lang_Class__nestHost0(this: Ref) E!Ref {
    const r = (try reflOf(this)) orelse return null;
    return optString(r.nest_host);
}
pub fn J_java_lang_Class__genericSignature0(this: Ref) E!Ref {
    const r = (try reflOf(this)) orelse return null;
    return optString(r.signature);
}
pub fn J_java_lang_Class__getRawAnnotations(this: Ref) E!Ref {
    const r = (try reflOf(this)) orelse return null;
    return optBytes(r.annotations);
}
pub fn J_java_lang_Class__getConstantPool(this: Ref) E!Ref {
    var R: [1]Ref = .{this};
    var F = rt.Frame{ .refs = &R, .n = 1, .meth = &meta };
    try rt.pushFrame(&F);
    defer rt.popFrame(&F);
    const o = try rt.newObject(P.C_jdk_internal_reflect_ConstantPool.cls());
    P.C_jdk_internal_reflect_ConstantPool.cast(o.?).f_constantPoolOop = R[0];
    return o;
}

// jdk.internal.reflect.ConstantPool over the compact per-class pool (annotation constants)
fn cpEntry(oop: Ref, idx: i32) E!rt.CpEntry {
    const r = cls(try rt.nn(oop)).refl orelse return rt.throwKind(.IllegalArgument, "Constant pool index out of bounds");
    if (idx <= 0 or idx >= r.cpool.len) return rt.throwKind(.IllegalArgument, "Constant pool index out of bounds");
    return r.cpool[@intCast(idx)];
}
fn cpWrong() E {
    return rt.throwKind(.IllegalArgument, "Wrong type at constant pool index");
}
pub fn J_jdk_internal_reflect_ConstantPool__getSize0(this: Ref, oop: Ref) E!i32 {
    _ = this;
    const r = cls(try rt.nn(oop)).refl orelse return 0;
    return @intCast(r.cpool.len);
}
pub fn J_jdk_internal_reflect_ConstantPool__getUTF8At0(this: Ref, oop: Ref, idx: i32) E!Ref {
    _ = this;
    return switch (try cpEntry(oop, idx)) {
        .utf8 => |v| rt.newStringUtf8(v),
        else => cpWrong(),
    };
}
pub fn J_jdk_internal_reflect_ConstantPool__getIntAt0(this: Ref, oop: Ref, idx: i32) E!i32 {
    _ = this;
    return switch (try cpEntry(oop, idx)) {
        .int => |v| v,
        else => cpWrong(),
    };
}
pub fn J_jdk_internal_reflect_ConstantPool__getLongAt0(this: Ref, oop: Ref, idx: i32) E!i64 {
    _ = this;
    return switch (try cpEntry(oop, idx)) {
        .long => |v| v,
        else => cpWrong(),
    };
}
pub fn J_jdk_internal_reflect_ConstantPool__getFloatAt0(this: Ref, oop: Ref, idx: i32) E!f32 {
    _ = this;
    return switch (try cpEntry(oop, idx)) {
        .float => |v| @bitCast(v),
        else => cpWrong(),
    };
}
pub fn J_jdk_internal_reflect_ConstantPool__getDoubleAt0(this: Ref, oop: Ref, idx: i32) E!f64 {
    _ = this;
    return switch (try cpEntry(oop, idx)) {
        .double => |v| @bitCast(v),
        else => cpWrong(),
    };
}
pub fn J_jdk_internal_reflect_ConstantPool__getTagAt0(this: Ref, oop: Ref, idx: i32) E!i32 {
    _ = this;
    return switch (try cpEntry(oop, idx)) {
        .none => 0,
        .utf8 => 1,
        .int => 3,
        .float => 4,
        .long => 5,
        .double => 6,
    };
}

/// InvocationHandler.invokeDefault: call the interface's default method non-virtually.
pub fn J_java_lang_reflect_Proxy__invokeDefault0(m: Ref, proxy: Ref, args: Ref) E!Ref {
    const mo = P.C_java_lang_reflect_Method.cast(try rt.nn(m));
    const t = try findTarget(cls(mo.f_clazz), @intCast(mo.f_slot), null, false);
    return P.reflectInvoke(t.shape, t.fp.?, proxy, args);
}

// ------------------------------------------------------------------ java.lang.invoke.J2zMH (method handles)
pub fn J_java_lang_invoke_J2zMH__invokeMethod0(m: Ref, recv: Ref, args: Ref, dispatch: i32) E!Ref {
    const mo = P.C_java_lang_reflect_Method.cast(try rt.nn(m));
    const t = try findTarget(cls(mo.f_clazz), @intCast(mo.f_slot), recv, dispatch != 0);
    return P.reflectInvoke(t.shape, t.fp.?, recv, args);
}

pub fn J_java_lang_invoke_J2zMH__newInstance0(c: Ref, args: Ref) E!Ref {
    return J_jdk_internal_reflect_MethodHandleAccessorFactory__newInstance0(c, args);
}

// ------------------------------------------------------------------ java.lang.StackWalker
pub fn J_java_lang_StackWalker__frames0() E!Ref {
    var n: usize = 0;
    var f = rt.cur.top;
    while (f) |fr| : (f = fr.prev) {
        if (fr.meth.cls != null) n += 1;
    }
    var R: [7]Ref = @splat(null);
    var F = rt.Frame{ .refs = &R, .n = 7, .meth = &meta };
    try rt.pushFrame(&F);
    defer rt.popFrame(&F);
    const len: i32 = @intCast(n);
    R[0] = try rt.newArray(rt.arrayOf(rt.hooks.object_class), 5);
    R[1] = try rt.newArray(rt.arrayOf(P.C_java_lang_Class.cls()), len);
    R[2] = try rt.newArray(rt.arrayOf(P.C_java_lang_String.cls()), len);
    R[3] = try rt.newArray(rt.arrayOf(P.C_java_lang_String.cls()), len);
    R[4] = try rt.newArray(rt.arrayOf(P.C_java_lang_String.cls()), len);
    R[5] = try rt.newArray(rt.arrayOf(rt.primByChar('I')), len);
    var i: i32 = 0;
    f = F.prev; // skip this native's own frame
    while (f) |fr| : (f = fr.prev) {
        const c = fr.meth.cls orelse continue;
        try rt.aastore(R[1], i, try rt.classMirror(c));
        R[6] = try rt.newStringUtf8(fr.meth.name);
        try rt.aastore(R[2], i, R[6]);
        R[6] = try rt.newStringUtf8(fr.meth.desc);
        try rt.aastore(R[3], i, R[6]);
        R[6] = if (fr.meth.file.len > 0 and !std.mem.eql(u8, fr.meth.file, "<synthetic>")) try rt.newStringUtf8(fr.meth.file) else null;
        try rt.aastore(R[4], i, R[6]);
        rt.arrayData(i32, rt.asArray(R[5].?))[@intCast(i)] = @intCast(fr.line);
        i += 1;
    }
    for (1..6) |k| try rt.aastore(R[0], @intCast(k - 1), R[k]);
    return R[0];
}

pub fn J_java_lang_System__userProps0() E!Ref {
    var R: [2]Ref = @splat(null);
    var F = rt.Frame{ .refs = &R, .n = 2, .meth = &meta };
    try rt.pushFrame(&F);
    defer rt.popFrame(&F);
    R[0] = try rt.newArray(rt.arrayOf(P.C_java_lang_String.cls()), @intCast(rt.st.user_props.len));
    for (rt.st.user_props, 0..) |a, i| {
        R[1] = try rt.newStringUtf8(std.mem.span(a)[2..]);
        try rt.aastore(R[0], @intCast(i), R[1]);
    }
    return R[0];
}

pub fn J_java_lang_Class__isBoot0(this: Ref) E!i32 {
    return @intFromBool(cls(try rt.nn(this)).boot);
}

// ------------------------------------------------------------------ JDK 25 MethodHandleNatives & NativeImageBuffer
pub fn J_java_lang_invoke_MethodHandleNatives__registerNatives() E!void {}

pub fn J_java_lang_invoke_MethodHandleNatives__getNamedCon(which: i32, name: Ref) E!i32 {
    _ = which;
    _ = name;
    return 0;
}

pub fn J_java_lang_invoke_MethodHandleNatives__init(self: Ref, ref: Ref) E!void {
    _ = self;
    _ = ref;
}

pub fn J_java_lang_invoke_MethodHandleNatives__expand(self: Ref) E!void {
    _ = self;
}

pub fn J_jdk_internal_jimage_NativeImageBuffer__getNativeMap(name: Ref) E!Ref {
    _ = name;
    return null;
}
