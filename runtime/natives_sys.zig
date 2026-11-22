//! Process / platform natives: ProcessHandleImpl, ProcessEnvironment, management beans, cgroup
//! metrics, performance counters, modules, misc small JDK natives.
//!
//! Module natives are no-ops by design: the image has no module system (see Class.getModule), so
//! there are no export/read checks to update.
const std = @import("std");
const rt = @import("rt.zig");
const P = @import("classes.zig");

const Ref = rt.Ref;
const E = rt.E;

/// errno location through std.c: `__errno` on Android (bionic has no `__errno_location`), the glibc name elsewhere.
inline fn __errno_location() *c_int {
    return std.c._errno();
}
extern "c" fn fstat(fd: c_int, st: *std.c.Stat) c_int;
extern "c" fn adler32(adler: c_ulong, buf: [*]const u8, len: c_uint) c_ulong;
extern "c" fn getpid() c_int;
extern "c" fn kill(pid: c_int, sig: c_int) c_int;
extern "c" fn waitpid(pid: c_int, status: *c_int, options: c_int) c_int;
extern "c" var environ: [*:null]?[*:0]u8;
extern "c" fn sysinfo(info: *SysInfo) c_int;
extern "c" fn read(fd: c_int, buf: [*]u8, n: usize) isize;
extern "c" fn write(fd: c_int, buf: [*]const u8, n: usize) isize;
extern "c" fn usleep(us: c_uint) c_int;
extern "c" fn clock_gettime(clk: c_int, ts: *std.c.timespec) c_int;

const SysInfo = extern struct {
    uptime: c_long,
    loads: [3]c_ulong,
    totalram: c_ulong,
    freeram: c_ulong,
    sharedram: c_ulong,
    bufferram: c_ulong,
    totalswap: c_ulong,
    freeswap: c_ulong,
    procs: u16,
    pad: u16,
    totalhigh: c_ulong,
    freehigh: c_ulong,
    mem_unit: c_uint,
    _f: [256]u8,
};

const meta = rt.MethodMeta{ .class = "<runtime>", .name = "<sys>", .desc = "" };

fn nowMs() i64 {
    var ts: std.c.timespec = undefined;
    _ = clock_gettime(0, &ts); // CLOCK_REALTIME
    return @as(i64, ts.sec) * 1000 + @divTrunc(@as(i64, ts.nsec), 1_000_000);
}
pub fn markStart() void {
    if (rt.st.start_ms == 0) rt.st.start_ms = nowMs();
}

// ------------------------------------------------------------------ java.io / java.util.zip / serialization
pub fn J_java_io_FileInputStream__isRegularFile0(this: Ref, fdo: Ref) E!i32 {
    _ = this;
    const fd = P.C_java_io_FileDescriptor.cast(try rt.nn(fdo)).f_fd;
    var st: std.c.Stat = undefined;
    if (fstat(fd, &st) < 0) return rt.throwKind(.IO, "fstat failed");
    return @intFromBool(@as(u32, @intCast(st.mode)) & 0o170000 == 0o100000);
}
pub fn J_java_util_zip_Adler32__updateByteBuffer(adler: i32, addr: i64, off: i32, len: i32) E!i32 {
    const p: [*]const u8 = @ptrFromInt(@as(usize, @bitCast(addr)) + @as(usize, @intCast(off)));
    return @bitCast(@as(u32, @truncate(adler32(@as(u32, @bitCast(adler)), p, @intCast(len)))));
}
pub fn J_java_io_ObjectStreamClass__initNative() E!void {}
pub fn J_java_io_ObjectStreamClass__hasStaticInitializer(c: Ref) E!i32 {
    const k = rt.hooks.mirror_get_class(try rt.nn(c));
    return @intFromBool(k.clinit != null);
}

// ------------------------------------------------------------------ java.lang.Module (no module system: nothing to record)
pub fn J_java_lang_Module__defineModule0(m: Ref, open: i32, ver: Ref, loc: Ref, pkgs: Ref) E!void {
    _ = .{ m, open, ver, loc, pkgs };
}
pub fn J_java_lang_Module__addReads0(a: Ref, b: Ref) E!void {
    _ = .{ a, b };
}
pub fn J_java_lang_Module__addExports0(a: Ref, pkg: Ref, to: Ref) E!void {
    _ = .{ a, pkg, to };
}
pub fn J_java_lang_Module__addExportsToAll0(a: Ref, pkg: Ref) E!void {
    _ = .{ a, pkg };
}
pub fn J_java_lang_Module__addExportsToAllUnnamed0(a: Ref, pkg: Ref) E!void {
    _ = .{ a, pkg };
}

// ------------------------------------------------------------------ processes
pub fn J_java_lang_ProcessHandleImpl__initNative() E!void {}
pub fn J_java_lang_ProcessHandleImpl__getCurrentPid0() E!i64 {
    return getpid();
}
/// start time in ms since the epoch (0 when unknown), -1 if the process is gone
pub fn J_java_lang_ProcessHandleImpl__isAlive0(pid: i64) E!i64 {
    if (kill(@intCast(pid), 0) != 0 and __errno_location().* == 3) return -1; // ESRCH
    return 0;
}
pub fn J_java_lang_ProcessHandleImpl__destroy0(pid: i64, start: i64, force: i32) E!i32 {
    _ = start;
    return @intFromBool(kill(@intCast(pid), if (force != 0) 9 else 15) == 0);
}
pub fn J_java_lang_ProcessHandleImpl__waitForProcessExit0(pid: i64, reap: i32) E!i32 {
    rt.enterSafe();
    defer rt.leaveSafe();
    var status: c_int = 0;
    if (reap != 0) {
        while (true) {
            const r = waitpid(@intCast(pid), &status, 0);
            if (r >= 0) break;
            if (__errno_location().* == 4) continue;
            return -1; // not our child
        }
        if (status & 0x7f == 0) return (status >> 8) & 0xff;
        return 0x80 + (status & 0x7f);
    }
    while (kill(@intCast(pid), 0) == 0) _ = usleep(10_000);
    return 0;
}
pub fn J_java_lang_ProcessEnvironment__environ() E!Ref {
    var n: usize = 0;
    while (environ[n] != null) n += 1;
    var R: [3]Ref = @splat(null);
    var F = rt.Frame{ .refs = &R, .n = 3, .meth = &meta };
    try rt.pushFrame(&F);
    defer rt.popFrame(&F);
    const bytes = rt.arrayOf(rt.prim_B);
    R[0] = try rt.newArray(rt.arrayOf(bytes), @intCast(n * 2));
    var k: i32 = 0;
    for (0..n) |i| {
        const kv = std.mem.span(environ[i].?);
        const eq = std.mem.indexOfScalar(u8, kv, '=') orelse continue;
        if (eq == 0) continue;
        R[1] = try newBytes(kv[0..eq]);
        try rt.aastore(R[0], k, R[1]);
        R[2] = try newBytes(kv[eq + 1 ..]);
        try rt.aastore(R[0], k + 1, R[2]);
        k += 2;
    }
    if (k == @as(i32, @intCast(n * 2))) return R[0];
    // some entries had no '=': shrink
    R[1] = try rt.newArray(rt.arrayOf(bytes), k);
    for (0..@as(usize, @intCast(k))) |i| try rt.aastore(R[1], @intCast(i), rt.arrayData(Ref, rt.asArray(R[0].?))[i]);
    return R[1];
}
fn newBytes(b: []const u8) E!Ref {
    const a = try rt.newArray(rt.arrayOf(rt.prim_B), @intCast(b.len));
    @memcpy(rt.arrayData(u8, rt.asArray(a.?))[0..b.len], b);
    return a;
}

// ------------------------------------------------------------------ container / VM support queries
pub fn J_jdk_internal_platform_CgroupMetrics__isUseContainerSupport() E!i32 {
    return 0; // cgroup controllers are not interpreted; limits come from the host values below
}
pub fn J_jdk_internal_platform_CgroupMetrics__getTotalMemorySize0() E!i64 {
    var si: SysInfo = undefined;
    if (sysinfo(&si) != 0) return 0;
    return @intCast(@as(u64, si.totalram) * si.mem_unit);
}
pub fn J_jdk_internal_platform_CgroupMetrics__getTotalSwapSize0() E!i64 {
    var si: SysInfo = undefined;
    if (sysinfo(&si) != 0) return 0;
    return @intCast(@as(u64, si.totalswap) * si.mem_unit);
}
pub fn J_jdk_internal_vm_ContinuationSupport__isSupported0() E!i32 {
    return 0; // threads are platform threads
}
pub fn J_jdk_internal_vm_ForeignLinkerSupport__isSupported0() E!i32 {
    return 0; // native calls go through the generated JNI/FFI trampolines
}

// ------------------------------------------------------------------ jdk.internal.perf.Perf
pub fn J_jdk_internal_perf_Perf__registerNatives() E!void {}
/// A counter is an 8-byte native cell exposed as a direct ByteBuffer.
pub fn J_jdk_internal_perf_Perf__createLong(this: Ref, name: Ref, variability: i32, units: i32, value: i64) E!Ref {
    _ = .{ this, name, variability, units };
    const mem = std.heap.c_allocator.alloc(u8, 8) catch return rt.throwKind(.OutOfMemory, "perf counter");
    @memcpy(mem[0..8], std.mem.asBytes(&value));
    return P.newDirectByteBuffer(@bitCast(@intFromPtr(mem.ptr)), 8);
}

// ------------------------------------------------------------------ sun.management
pub fn J_sun_management_VMManagementImpl__getVersion0() E!Ref {
    return rt.newStringUtf8("4.0");
}
pub fn J_sun_management_VMManagementImpl__initOptionalSupportFields() E!void {}
pub fn J_sun_management_VMManagementImpl__isThreadContentionMonitoringEnabled(this: Ref) E!i32 {
    _ = this;
    return 0;
}
pub fn J_sun_management_VMManagementImpl__isThreadCpuTimeEnabled(this: Ref) E!i32 {
    _ = this;
    return 0;
}
pub fn J_sun_management_VMManagementImpl__isThreadAllocatedMemoryEnabled(this: Ref) E!i32 {
    _ = this;
    return 0;
}
pub fn J_sun_management_VMManagementImpl__getTotalClassCount(this: Ref) E!i64 {
    _ = this;
    return @intCast(rt.hooks.all_classes.len);
}
pub fn J_sun_management_VMManagementImpl__getProcessId(this: Ref) E!i32 {
    _ = this;
    return getpid();
}
pub fn J_sun_management_VMManagementImpl__getVmArguments0(this: Ref) E!Ref {
    _ = this;
    return rt.newArray(rt.arrayOf(P.C_java_lang_String.cls()), 0);
}
pub fn J_sun_management_VMManagementImpl__getStartupTime(this: Ref) E!i64 {
    _ = this;
    markStart();
    return rt.st.start_ms;
}
pub fn J_sun_management_VMManagementImpl__getUptime0(this: Ref) E!i64 {
    _ = this;
    markStart();
    return nowMs() - rt.st.start_ms;
}
pub fn J_sun_management_VMManagementImpl__getTotalCompileTime(this: Ref) E!i64 {
    _ = this;
    return 0; // ahead-of-time compiled
}
pub fn J_sun_management_VMManagementImpl__getTotalThreadCount(this: Ref) E!i64 {
    _ = this;
    return 1;
}
pub fn J_sun_management_VMManagementImpl__getTotalSafepointTime(this: Ref) E!i64 {
    _ = this;
    return 0;
}
pub fn J_sun_management_VMManagementImpl__getTotalApplicationNonStoppedTime(this: Ref) E!i64 {
    _ = this;
    return 0;
}
pub fn J_sun_management_MemoryImpl__getMemoryPools0() E!Ref {
    return rt.newArray(rt.arrayOf(P.C_java_lang_Object.cls()), 0); // no memory pools: one native heap
}
pub fn J_sun_management_MemoryImpl__getMemoryManagers0() E!Ref {
    return rt.newArray(rt.arrayOf(P.C_java_lang_Object.cls()), 0);
}
pub fn J_sun_management_MemoryImpl__getMemoryUsage0(this: Ref, heap: i32) E!Ref {
    _ = this;
    const MU = P.C_java_lang_management_MemoryUsage;
    const o = try rt.newObject(MU.cls());
    const m = MU.cast(o.?);
    if (heap != 0) {
        m.f_init = 0;
        m.f_used = @intCast(rt.usedBytes());
        m.f_committed = @intCast(@max(rt.heapBytes(), rt.usedBytes()));
        m.f_max = if (rt.st.gc_max_heap != 0) @intCast(rt.st.gc_max_heap) else -1;
    } else {
        m.f_init = 0;
        m.f_used = 0;
        m.f_committed = 0;
        m.f_max = -1;
    }
    return o;
}
pub fn J_sun_management_MemoryImpl__getTotalThreadAllocatedMemory() E!i64 {
    return 0;
}
pub fn J_sun_management_ThreadImpl__dumpThreads0(ids: Ref, locks: i32, syncs: i32, depth: i32) E!Ref {
    _ = .{ ids, locks, syncs, depth };
    return rt.throwKind(.UnsupportedOperation, "thread dumps (ThreadMXBean.dumpAllThreads) are not supported by the native runtime");
}

// ------------------------------------------------------------------ misc
pub fn J_sun_net_spi_DefaultProxySelector__init() E!i32 {
    return 0; // no desktop proxy configuration (gconf/libproxy): direct connections unless -D properties are set
}
pub fn J_sun_net_spi_DefaultProxySelector__getSystemProxies(this: Ref, scheme: Ref, host: Ref) E!Ref {
    _ = .{ this, scheme, host };
    return null;
}
pub fn J_sun_net_sdp_SdpSupport__convert0(fd: i32) E!void {
    _ = fd;
    return rt.throwKind(.IO, "Sockets Direct Protocol is not supported");
}
pub fn J_java_lang_reflect_Executable__getParameters0(this: Ref) E!Ref {
    _ = this;
    return null; // MethodParameters attributes are not kept: callers fall back to arg0, arg1, ...
}
pub fn J_jdk_internal_misc_Unsafe__writeback0(this: Ref, addr: i64) E!void {
    _ = .{ this, addr };
}
pub fn J_jdk_internal_misc_Unsafe__writebackPreSync0(this: Ref) E!void {
    _ = this;
}
pub fn J_jdk_internal_misc_Unsafe__writebackPostSync0(this: Ref) E!void {
    _ = this;
}
pub fn J_sun_nio_fs_UnixFileSystem__bufferedCopy0(dst: i32, src: i32, buf: i64, size: i32, cancel: i64) E!void {
    const p: [*]u8 = @ptrFromInt(@as(usize, @bitCast(buf)));
    const flag: ?*const volatile i32 = if (cancel != 0) @ptrFromInt(@as(usize, @bitCast(cancel))) else null;
    rt.enterSafe();
    defer rt.leaveSafe();
    while (true) {
        if (flag) |f| if (f.* != 0) return;
        const n = read(src, p, @intCast(size));
        if (n < 0) {
            if (__errno_location().* == 4) continue;
            break;
        }
        if (n == 0) return;
        var off: usize = 0;
        while (off < n) {
            const w = write(dst, p + off, @as(usize, @intCast(n)) - off);
            if (w < 0) {
                if (__errno_location().* == 4) continue;
                return;
            }
            off += @intCast(w);
        }
    }
}

// ------------------------------------------------------------------ java.lang.ProcessImpl (Runtime.exec / ProcessBuilder)
// posix_spawn* exist in bionic only from API 28 (addchdir_np from API 34): weak imports keep dlopen of the engine
// working on older Android, and a missing function surfaces as an IOException (error=38), never as a silent skip.
const SpawnFa = *anyopaque;
fn weakC(comptime T: type, comptime name: []const u8) ?*const T {
    return @extern(?*const T, .{ .name = name, .linkage = .weak });
}
const ENOSYS_CODE: c_int = 38;
fn posix_spawnp(pid: *c_int, file: [*:0]const u8, fa: *const anyopaque, attr: ?*const anyopaque, argv: [*:null]const ?[*:0]const u8, envp: [*:null]const ?[*:0]const u8) c_int {
    const f = weakC(fn (*c_int, [*:0]const u8, *const anyopaque, ?*const anyopaque, [*:null]const ?[*:0]const u8, [*:null]const ?[*:0]const u8) callconv(.c) c_int, "posix_spawnp") orelse return ENOSYS_CODE;
    return f(pid, file, fa, attr, argv, envp);
}
fn posix_spawn_file_actions_init(fa: SpawnFa) c_int {
    const f = weakC(fn (SpawnFa) callconv(.c) c_int, "posix_spawn_file_actions_init") orelse return ENOSYS_CODE;
    return f(fa);
}
fn posix_spawn_file_actions_destroy(fa: SpawnFa) c_int {
    const f = weakC(fn (SpawnFa) callconv(.c) c_int, "posix_spawn_file_actions_destroy") orelse return ENOSYS_CODE;
    return f(fa);
}
fn posix_spawn_file_actions_adddup2(fa: SpawnFa, fd: c_int, newfd: c_int) c_int {
    const f = weakC(fn (SpawnFa, c_int, c_int) callconv(.c) c_int, "posix_spawn_file_actions_adddup2") orelse return ENOSYS_CODE;
    return f(fa, fd, newfd);
}
fn posix_spawn_file_actions_addclose(fa: SpawnFa, fd: c_int) c_int {
    const f = weakC(fn (SpawnFa, c_int) callconv(.c) c_int, "posix_spawn_file_actions_addclose") orelse return ENOSYS_CODE;
    return f(fa, fd);
}
fn posix_spawn_file_actions_addchdir_np(fa: SpawnFa, path: [*:0]const u8) c_int {
    const f = weakC(fn (SpawnFa, [*:0]const u8) callconv(.c) c_int, "posix_spawn_file_actions_addchdir_np") orelse return ENOSYS_CODE;
    return f(fa, path);
}
extern "c" fn pipe2(fds: *[2]c_int, flags: c_int) c_int;
extern "c" fn close(fd: c_int) c_int;
extern "c" fn strerror(e: c_int) [*:0]const u8;

pub fn J_java_lang_ProcessImpl__init() E!void {}

fn splitNul(alloc: std.mem.Allocator, block: []const u8, count: usize) ![][*:0]const u8 {
    const out = try alloc.alloc([*:0]const u8, count);
    var pos: usize = 0;
    for (0..count) |i| {
        const end = std.mem.indexOfScalarPos(u8, block, pos, 0) orelse block.len;
        const copy = try alloc.allocSentinel(u8, end - pos, 0);
        @memcpy(copy, block[pos..end]);
        out[i] = copy.ptr;
        pos = end + 1;
    }
    return out;
}

pub fn J_java_lang_ProcessImpl__forkAndExec(this: Ref, mode: i32, helper: Ref, prog: Ref, args: Ref, argc: i32, envb: Ref, envc: i32, dir: Ref, fds: Ref, redirect: i32) E!i32 {
    _ = .{ this, mode, helper };
    var arena_state = std.heap.ArenaAllocator.init(std.heap.c_allocator);
    defer arena_state.deinit();
    const a = arena_state.allocator();
    

    const pb = rt.arrayData(u8, rt.asArray(try rt.nn(prog)));
    const prog_s = a.dupeZ(u8, std.mem.sliceTo(pb[0..@intCast(rt.asArray(prog.?).length)], 0)) catch return rt.throwKind(.OutOfMemory, "forkAndExec");
    const arg_bytes: []const u8 = if (args) |x| rt.arrayData(u8, rt.asArray(x))[0..@intCast(rt.asArray(x).length)] else &.{};
    const rest = splitNul(a, arg_bytes, @intCast(argc)) catch return rt.throwKind(.OutOfMemory, "forkAndExec");
    const argv = a.allocSentinel(?[*:0]const u8, rest.len + 1, null) catch return rt.throwKind(.OutOfMemory, "forkAndExec");
    argv[0] = prog_s;
    for (rest, 0..) |r, i| argv[i + 1] = r;

    var envp_buf: [*:null]const ?[*:0]const u8 = @ptrCast(@extern(*[*:null]?[*:0]u8, .{ .name = "environ" }).*);
    if (envb) |e| {
        const eb = rt.arrayData(u8, rt.asArray(e))[0..@intCast(rt.asArray(e).length)];
        const env_list = splitNul(a, eb, @intCast(envc)) catch return rt.throwKind(.OutOfMemory, "forkAndExec");
        const ep = a.allocSentinel(?[*:0]const u8, env_list.len, null) catch return rt.throwKind(.OutOfMemory, "forkAndExec");
        for (env_list, 0..) |r, i| ep[i] = r;
        envp_buf = ep.ptr;
    }

    const fd_arr = rt.arrayData(i32, rt.asArray(try rt.nn(fds)));
    var child_fd: [3]c_int = undefined; // fds the child uses
    var parent_fd: [3]c_int = .{ -1, -1, -1 };
    var to_close: [6]c_int = undefined;
    var nclose: usize = 0;
    for (0..3) |i| {
        if (i == 2 and redirect != 0) {
            child_fd[2] = -2; // dup of child's stdout
            continue;
        }
        if (fd_arr[i] == -1) {
            var p: [2]c_int = undefined;
            if (pipe2(&p, 0o2000000) != 0) return rt.throwKind(.IO, std.mem.span(strerror(__errno_location().*)));
            if (i == 0) {
                child_fd[0] = p[0];
                parent_fd[0] = p[1];
                to_close[nclose] = p[0];
            } else {
                child_fd[i] = p[1];
                parent_fd[i] = p[0];
                to_close[nclose] = p[1];
            }
            nclose += 1;
        } else {
            child_fd[i] = fd_arr[i];
        }
    }
    defer for (to_close[0..nclose]) |c| {
        _ = close(c);
    };

    var fa: [128]u8 align(16) = undefined;
    _ = posix_spawn_file_actions_init(&fa);
    defer _ = posix_spawn_file_actions_destroy(&fa);
    for (0..3) |i| {
        if (child_fd[i] == -2) continue;
        _ = posix_spawn_file_actions_adddup2(&fa, child_fd[i], @intCast(i));
    }
    if (child_fd[2] == -2) _ = posix_spawn_file_actions_adddup2(&fa, 1, 2);
    for (0..3) |i| if (parent_fd[i] >= 0) {
        _ = posix_spawn_file_actions_addclose(&fa, parent_fd[i]);
    };
    if (dir) |d| {
        const db = rt.arrayData(u8, rt.asArray(d))[0..@intCast(rt.asArray(d).length)];
        const dz = a.dupeZ(u8, std.mem.sliceTo(db, 0)) catch return rt.throwKind(.OutOfMemory, "forkAndExec");
        const crc = posix_spawn_file_actions_addchdir_np(&fa, dz);
        if (crc != 0) {
            for (parent_fd) |p| if (p >= 0) {
                _ = close(p);
            };
            var cmsg: [200]u8 = undefined;
            return rt.throwKind(.IO, std.fmt.bufPrint(&cmsg, "error={d}, cannot set the working directory of the child process", .{crc}) catch "chdir unsupported");
        }
    }

    var pid: c_int = 0;
    rt.enterSafe();
    const rc = posix_spawnp(&pid, prog_s, &fa, null, argv.ptr, envp_buf);
    rt.leaveSafe();
    if (rc != 0) {
        for (parent_fd) |p| if (p >= 0) {
            _ = close(p);
        };
        var msg: [200]u8 = undefined;
        return rt.throwKind(.IO, std.fmt.bufPrint(&msg, "error={d}, {s}", .{ rc, std.mem.span(strerror(rc)) }) catch "spawn failed");
    }
    for (0..3) |i| fd_arr[i] = parent_fd[i];
    return pid;
}
