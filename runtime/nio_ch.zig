//! sun.nio.ch natives for file channels on Linux (FileChannelImpl, file locks, mapped buffers,
//! pipes), implemented directly on libc with the JDK's IOStatus return protocol.
const std = @import("std");
const rt = @import("rt.zig");
const P = @import("classes.zig");

const Ref = rt.Ref;
const E = rt.E;

/// errno location through std.c: `__errno` on Android (bionic has no `__errno_location`), the glibc name elsewhere.
inline fn __errno_location() *c_int {
    return std.c._errno();
}
extern "c" fn strerror(e: c_int) [*:0]const u8;
extern "c" fn read(fd: c_int, buf: [*]u8, n: usize) isize;
extern "c" fn write(fd: c_int, buf: [*]const u8, n: usize) isize;
extern "c" fn pread(fd: c_int, buf: [*]u8, n: usize, off: i64) isize;
extern "c" fn pwrite(fd: c_int, buf: [*]const u8, n: usize, off: i64) isize;
extern "c" fn readv(fd: c_int, iov: *const anyopaque, n: c_int) isize;
extern "c" fn writev(fd: c_int, iov: *const anyopaque, n: c_int) isize;
extern "c" fn fsync(fd: c_int) c_int;
extern "c" fn fdatasync(fd: c_int) c_int;
extern "c" fn lseek(fd: c_int, off: i64, whence: c_int) i64;
extern "c" fn ftruncate(fd: c_int, len: i64) c_int;
extern "c" fn fstat(fd: c_int, st: *std.c.Stat) c_int;
extern "c" fn fcntl(fd: c_int, cmd: c_int, ...) c_int;
extern "c" fn ioctl(fd: c_int, req: c_ulong, ...) c_int;
extern "c" fn close(fd: c_int) c_int;
extern "c" fn dup2(a: c_int, b: c_int) c_int;
extern "c" fn pipe2(fds: *[2]c_int, flags: c_int) c_int;
extern "c" fn socketpair(d: c_int, t: c_int, p: c_int, sv: *[2]c_int) c_int;
extern "c" fn mmap(addr: ?*anyopaque, len: usize, prot: c_int, flags: c_int, fd: c_int, off: i64) isize;
extern "c" fn munmap(addr: *anyopaque, len: usize) c_int;
extern "c" fn sysconf(n: c_int) c_long;
extern "c" fn getrlimit(res: c_int, rl: *[2]u64) c_int;
extern "c" fn sendfile(out: c_int, in: c_int, off: ?*i64, n: usize) isize;
/// copy_file_range(2) as a raw syscall: libc exports it only from Android API 34 on, and an unresolved import
/// would make dlopen of the whole engine fail on older devices. ENOSYS reaches the callers' user-space fallback.
fn copy_file_range(in: c_int, off_in: ?*i64, out: c_int, off_out: ?*i64, n: usize, flags: c_uint) isize {
    const rc = std.os.linux.copy_file_range(in, off_in, out, off_out, n, flags);
    if (rc > @as(usize, @bitCast(@as(isize, -4096)))) { // raw syscall error: -errno
        __errno_location().* = @intCast(0 -% rc);
        return -1;
    }
    return @bitCast(rc);
}
extern "c" fn pthread_self() usize;
extern "c" fn pthread_kill(t: usize, sig: c_int) c_int;
extern "c" fn __libc_current_sigrtmax() c_int;
extern "c" fn sigaction(sig: c_int, act: *const anyopaque, old: ?*anyopaque) c_int;
extern "c" fn fstatvfs(fd: c_int, st: *[16]u64) c_int;

// IOStatus
const IOS_EOF: i32 = -1;
const IOS_UNAVAILABLE: i32 = -2;
const IOS_INTERRUPTED: i32 = -3;
const IOS_UNSUPPORTED_CASE: i32 = -6;

const EINTR = 4;
const EAGAIN = 11;
const EINVAL = 22;
const ENOSYS = 38;
const EXDEV = 18;
const ENOMEM = 12;
const EOPNOTSUPP = 95;
const ENOTSUP = 95;
const EBADF = 9;

inline fn errno() c_int {
    return __errno_location().*;
}

fn ioFail(what: []const u8) E {
    _ = what;
    return rt.throwKind(.IO, std.mem.span(strerror(errno())));
}

fn fdOf(fdo: Ref) E!c_int {
    return P.C_java_io_FileDescriptor.cast(try rt.nn(fdo)).f_fd;
}

inline fn addr(a: i64) usize {
    return @bitCast(a);
}

/// Map a read/write style result to the IOStatus protocol.
fn convertReturn(n: isize, reading: bool, what: []const u8) E!i32 {
    if (n > 0) return @intCast(n);
    if (n == 0) return if (reading) IOS_EOF else 0;
    const e = errno();
    if (e == EAGAIN) return IOS_UNAVAILABLE;
    if (e == EINTR) return IOS_INTERRUPTED;
    return ioFail(what);
}

fn convertLong(n: isize, reading: bool, what: []const u8) E!i64 {
    if (n > 0) return @intCast(n);
    if (n == 0) return if (reading) IOS_EOF else 0;
    const e = errno();
    if (e == EAGAIN) return IOS_UNAVAILABLE;
    if (e == EINTR) return IOS_INTERRUPTED;
    return ioFail(what);
}

// ------------------------------------------------------------------ UnixFileDispatcherImpl
const D = "J_sun_nio_ch_UnixFileDispatcherImpl__";

pub fn J_sun_nio_ch_UnixFileDispatcherImpl__read0(fdo: Ref, a: i64, len: i32) E!i32 {
    const fd = try fdOf(fdo);
    rt.enterSafe();
    const n = read(fd, @ptrFromInt(addr(a)), @intCast(len));
    rt.leaveSafe();
    return convertReturn(n, true, "Read failed");
}
pub fn J_sun_nio_ch_UnixFileDispatcherImpl__pread0(fdo: Ref, a: i64, len: i32, pos: i64) E!i32 {
    const fd = try fdOf(fdo);
    rt.enterSafe();
    const n = pread(fd, @ptrFromInt(addr(a)), @intCast(len), pos);
    rt.leaveSafe();
    return convertReturn(n, true, "Read failed");
}
pub fn J_sun_nio_ch_UnixFileDispatcherImpl__readv0(fdo: Ref, a: i64, len: i32) E!i64 {
    const fd = try fdOf(fdo);
    rt.enterSafe();
    const n = readv(fd, @ptrFromInt(addr(a)), len);
    rt.leaveSafe();
    return convertLong(n, true, "Read failed");
}
pub fn J_sun_nio_ch_UnixFileDispatcherImpl__write0(fdo: Ref, a: i64, len: i32) E!i32 {
    const fd = try fdOf(fdo);
    rt.enterSafe();
    const n = write(fd, @ptrFromInt(addr(a)), @intCast(len));
    rt.leaveSafe();
    return convertReturn(n, false, "Write failed");
}
pub fn J_sun_nio_ch_UnixFileDispatcherImpl__pwrite0(fdo: Ref, a: i64, len: i32, pos: i64) E!i32 {
    const fd = try fdOf(fdo);
    rt.enterSafe();
    const n = pwrite(fd, @ptrFromInt(addr(a)), @intCast(len), pos);
    rt.leaveSafe();
    return convertReturn(n, false, "Write failed");
}
pub fn J_sun_nio_ch_UnixFileDispatcherImpl__writev0(fdo: Ref, a: i64, len: i32) E!i64 {
    const fd = try fdOf(fdo);
    rt.enterSafe();
    const n = writev(fd, @ptrFromInt(addr(a)), len);
    rt.leaveSafe();
    return convertLong(n, false, "Write failed");
}
pub fn J_sun_nio_ch_UnixFileDispatcherImpl__force0(fdo: Ref, md: i32) E!i32 {
    const fd = try fdOf(fdo);
    rt.enterSafe();
    const r = if (md != 0) fsync(fd) else fdatasync(fd);
    rt.leaveSafe();
    if (r < 0) return ioFail("Force failed");
    return 0;
}
pub fn J_sun_nio_ch_UnixFileDispatcherImpl__seek0(fdo: Ref, off: i64) E!i64 {
    const fd = try fdOf(fdo);
    const r = if (off < 0) lseek(fd, 0, 1) else lseek(fd, off, 0);
    if (r < 0) return ioFail("lseek64 failed");
    return r;
}
pub fn J_sun_nio_ch_UnixFileDispatcherImpl__truncate0(fdo: Ref, size: i64) E!i32 {
    const fd = try fdOf(fdo);
    if (ftruncate(fd, size) < 0) {
        if (errno() == EINTR) return IOS_INTERRUPTED;
        return ioFail("Truncation failed");
    }
    return 0;
}
pub fn J_sun_nio_ch_UnixFileDispatcherImpl__size0(fdo: Ref) E!i64 {
    const fd = try fdOf(fdo);
    var st: std.c.Stat = undefined;
    if (fstat(fd, &st) < 0) return ioFail("Size failed");
    return st.size;
}
fn modeType(m: u32) u32 {
    return m & 0o170000;
}
pub fn J_sun_nio_ch_UnixFileDispatcherImpl__available0(fdo: Ref) E!i32 {
    const fd = try fdOf(fdo);
    var st: std.c.Stat = undefined;
    if (fstat(fd, &st) < 0) return ioFail("Available failed");
    const t = modeType(@intCast(st.mode));
    if (t == 0o020000 or t == 0o010000 or t == 0o140000) { // chr, fifo, sock
        var n: c_int = 0;
        if (ioctl(fd, 0x541B, &n) >= 0) return n; // FIONREAD
    }
    const cur = lseek(fd, 0, 1);
    if (cur < 0) return ioFail("Available failed");
    const rem = st.size - cur;
    return @intCast(@max(0, @min(rem, std.math.maxInt(i32))));
}
pub fn J_sun_nio_ch_UnixFileDispatcherImpl__isOther0(fdo: Ref) E!i32 {
    const fd = try fdOf(fdo);
    var st: std.c.Stat = undefined;
    if (fstat(fd, &st) < 0) return ioFail("isOther failed");
    const t = modeType(@intCast(st.mode));
    return if (t == 0o100000 or t == 0o040000 or t == 0o120000) 0 else 1;
}

const Flock = extern struct { l_type: i16, l_whence: i16, l_start: i64, l_len: i64, l_pid: i32 };
const NO_LOCK: i32 = -1;
const LOCKED: i32 = 0;
const INTERRUPTED: i32 = 2;

pub fn J_sun_nio_ch_UnixFileDispatcherImpl__lock0(fdo: Ref, blocking: i32, pos: i64, size: i64, shared: i32) E!i32 {
    const fd = try fdOf(fdo);
    var fl = Flock{ .l_type = if (shared != 0) 0 else 1, .l_whence = 0, .l_start = pos, .l_len = if (size == std.math.maxInt(i64)) 0 else size, .l_pid = 0 };
    const cmd: c_int = if (blocking != 0) 7 else 6; // F_SETLKW / F_SETLK
    if (blocking != 0) rt.enterSafe();
    const r = fcntl(fd, cmd, &fl);
    if (blocking != 0) rt.leaveSafe();
    if (r < 0) {
        const e = errno();
        if (blocking == 0 and (e == EAGAIN or e == 13)) return NO_LOCK; // EACCES
        if (e == EINTR) return INTERRUPTED;
        return ioFail("Lock failed");
    }
    return LOCKED;
}
pub fn J_sun_nio_ch_UnixFileDispatcherImpl__release0(fdo: Ref, pos: i64, size: i64) E!void {
    const fd = try fdOf(fdo);
    var fl = Flock{ .l_type = 2, .l_whence = 0, .l_start = pos, .l_len = if (size == std.math.maxInt(i64)) 0 else size, .l_pid = 0 };
    if (fcntl(fd, 6, &fl) < 0) return ioFail("Release failed");
}
pub fn J_sun_nio_ch_UnixFileDispatcherImpl__closeIntFD(fd: i32) E!void {
    if (fd != -1 and close(fd) < 0 and errno() != EINTR) return ioFail("Close failed");
}
pub fn J_sun_nio_ch_UnixFileDispatcherImpl__allocationGranularity0() E!i64 {
    return sysconf(30); // _SC_PAGESIZE
}
pub fn J_sun_nio_ch_UnixFileDispatcherImpl__map0(fdo: Ref, prot: i32, off: i64, len: i64, sync: i32) E!i64 {
    _ = sync;
    const fd = try fdOf(fdo);
    // FileChannelImpl.MAP_RO = 0, MAP_RW = 1, MAP_PV = 2
    const p: c_int = if (prot == 0) 1 else 3; // PROT_READ | PROT_WRITE
    const flags: c_int = if (prot == 2) 2 else 1; // MAP_PRIVATE / MAP_SHARED
    const r = mmap(null, @intCast(len), p, flags, fd, off);
    if (r == -1) {
        if (errno() == ENOMEM) return rt.throwKind(.OutOfMemory, "Map failed");
        return ioFail("Map failed");
    }
    return @intCast(r);
}
pub fn J_sun_nio_ch_UnixFileDispatcherImpl__unmap0(a: i64, len: i64) E!i32 {
    if (munmap(@ptrFromInt(addr(a)), @intCast(len)) < 0) return ioFail("Unmap failed");
    return 0;
}
pub fn J_sun_nio_ch_UnixFileDispatcherImpl__setDirect0(fdo: Ref) E!i32 {
    const fd = try fdOf(fdo);
    const fl = fcntl(fd, 3); // F_GETFL
    if (fl < 0 or fcntl(fd, 4, fl | 0o40000) < 0) return ioFail("DirectIO setup failed"); // O_DIRECT
    var vfs: [16]u64 = undefined;
    if (fstatvfs(fd, &vfs) < 0) return ioFail("DirectIO setup failed");
    return @intCast(vfs[0]); // f_bsize
}

// ------------------------------------------------------------------ FileDispatcherImpl (Linux)
pub fn J_sun_nio_ch_FileDispatcherImpl__init0() E!void {}
pub fn J_sun_nio_ch_FileDispatcherImpl__transferTo0(src: Ref, pos: i64, count: i64, dst: Ref, append: i32) E!i64 {
    _ = append;
    const sfd = try fdOf(src);
    const dfd = try fdOf(dst);
    var off: i64 = pos;
    rt.enterSafe();
    const n = sendfile(dfd, sfd, &off, @intCast(count));
    rt.leaveSafe();
    if (n < 0) {
        const e = errno();
        if (e == EAGAIN) return IOS_UNAVAILABLE;
        if (e == EINVAL and count >= 0) return IOS_UNSUPPORTED_CASE;
        if (e == EINTR) return IOS_INTERRUPTED;
        return ioFail("Transfer failed");
    }
    return n;
}
pub fn J_sun_nio_ch_FileDispatcherImpl__transferFrom0(src: Ref, dst: Ref, pos: i64, count: i64, append: i32) E!i64 {
    if (append != 0) return IOS_UNSUPPORTED_CASE;
    const sfd = try fdOf(src);
    const dfd = try fdOf(dst);
    var off: i64 = pos;
    rt.enterSafe();
    const n = copy_file_range(sfd, null, dfd, &off, @intCast(count), 0);
    rt.leaveSafe();
    if (n < 0) {
        const e = errno();
        if (e == EINVAL or e == EXDEV or e == ENOSYS or e == EOPNOTSUPP or e == EBADF or e == 1) return IOS_UNSUPPORTED_CASE;
        if (e == EINTR) return IOS_INTERRUPTED;
        return ioFail("Transfer failed");
    }
    return n;
}

// ------------------------------------------------------------------ UnixDispatcher
pub fn J_sun_nio_ch_UnixDispatcher__init() E!void {
    var sv: [2]c_int = undefined;
    if (socketpair(1, 1, 0, &sv) < 0) return ioFail("socketpair failed"); // AF_UNIX, SOCK_STREAM
    rt.st.pre_close_fd = sv[0];
    _ = close(sv[1]);
}
pub fn J_sun_nio_ch_UnixDispatcher__close0(fdo: Ref) E!void {
    const fd = try fdOf(fdo);
    if (fd != -1 and close(fd) < 0 and errno() != EINTR) return ioFail("Close failed");
}
pub fn J_sun_nio_ch_UnixDispatcher__preClose0(fdo: Ref) E!void {
    const fd = try fdOf(fdo);
    if (rt.st.pre_close_fd >= 0 and dup2(rt.st.pre_close_fd, fd) < 0) return ioFail("dup2 failed");
}

// ------------------------------------------------------------------ IOUtil
pub fn J_sun_nio_ch_IOUtil__initIDs() E!void {}
pub fn J_sun_nio_ch_IOUtil__randomBytes(b: Ref) E!i32 {
    _ = b;
    return 0; // as on Linux HotSpot: not supported, callers fall back to SecureRandom
}
pub fn J_sun_nio_ch_IOUtil__fdVal(fdo: Ref) E!i32 {
    return fdOf(fdo);
}
pub fn J_sun_nio_ch_IOUtil__setfdVal(fdo: Ref, v: i32) E!void {
    P.C_java_io_FileDescriptor.cast(try rt.nn(fdo)).f_fd = v;
}
pub fn J_sun_nio_ch_IOUtil__configureBlocking(fdo: Ref, blocking: i32) E!void {
    const fd = try fdOf(fdo);
    const fl = fcntl(fd, 3);
    const nfl = if (blocking != 0) fl & ~@as(c_int, 0o4000) else fl | 0o4000; // O_NONBLOCK
    if (fl < 0 or (fl != nfl and fcntl(fd, 4, nfl) < 0)) return ioFail("Configure blocking failed");
}
pub fn J_sun_nio_ch_IOUtil__fdLimit() E!i32 {
    var rl: [2]u64 = undefined;
    if (getrlimit(7, &rl) < 0) return ioFail("getrlimit failed"); // RLIMIT_NOFILE
    return @intCast(@min(rl[0], std.math.maxInt(i32)));
}
pub fn J_sun_nio_ch_IOUtil__iovMax() E!i32 {
    const v = sysconf(60); // _SC_IOV_MAX
    return if (v < 0) 16 else @intCast(v);
}
pub fn J_sun_nio_ch_IOUtil__writevMax() E!i64 {
    return std.math.maxInt(isize);
}
pub fn J_sun_nio_ch_IOUtil__makePipe(blocking: i32) E!i64 {
    var fds: [2]c_int = undefined;
    if (pipe2(&fds, if (blocking != 0) 0 else 0o4000) < 0) return ioFail("Pipe failed");
    return (@as(i64, fds[0]) << 32) | @as(i64, @as(u32, @bitCast(fds[1])));
}
pub fn J_sun_nio_ch_IOUtil__write1(fd: i32, b: i32) E!i32 {
    const c: [1]u8 = .{@truncate(@as(u32, @bitCast(b)))};
    return convertReturn(write(fd, &c, 1), false, "write1 failed");
}
pub fn J_sun_nio_ch_IOUtil__drain(fd: i32) E!i32 {
    var buf: [16]u8 = undefined;
    var any = false;
    while (true) {
        const n = read(fd, &buf, buf.len);
        if (n < 0) {
            if (errno() == EAGAIN) break;
            if (errno() == EINTR) continue;
            return ioFail("Drain");
        }
        if (n > 0) any = true;
        if (n < buf.len) break;
    }
    return @intFromBool(any);
}
pub fn J_sun_nio_ch_IOUtil__drain1(fd: i32) E!i32 {
    var buf: [1]u8 = undefined;
    const n = read(fd, &buf, 1);
    if (n < 0) {
        if (errno() == EAGAIN) return 0;
        if (errno() == EINTR) return IOS_INTERRUPTED;
        return ioFail("Drain");
    }
    return @intCast(n);
}

// ------------------------------------------------------------------ NativeThread (Linux)
fn onSignal(_: c_int) callconv(.c) void {}
pub fn J_sun_nio_ch_NativeThread__init() E!void {
    // a no-op handler for the signal used to interrupt threads blocked in I/O
    var act: [152]u8 = @splat(0); // struct sigaction (x86_64/aarch64 glibc: 152 bytes)
    const h: usize = @intFromPtr(&onSignal);
    @memcpy(act[0..8], std.mem.asBytes(&h));
    if (sigaction(__libc_current_sigrtmax() - 2, &act, null) < 0) return ioFail("sigaction failed");
}
pub fn J_sun_nio_ch_NativeThread__supportPendingSignals0() E!i32 {
    return 0;
}
pub fn J_sun_nio_ch_NativeThread__current0() E!i64 {
    return @bitCast(@as(u64, pthread_self()));
}
pub fn J_sun_nio_ch_NativeThread__signal0(t: i64) E!void {
    const r = pthread_kill(@bitCast(t), __libc_current_sigrtmax() - 2);
    if (r != 0) return rt.throwKind(.IO, "Thread signal failed");
}

// ------------------------------------------------------------------ FileKey
pub fn J_sun_nio_ch_FileKey__init(fdo: Ref, info: Ref) E!void {
    const fd = try fdOf(fdo);
    var st: std.c.Stat = undefined;
    if (fstat(fd, &st) < 0) return ioFail("fstat failed");
    const a = rt.arrayData(i64, rt.asArray(try rt.nn(info)));
    a[0] = @bitCast(@as(u64, st.dev));
    a[1] = @bitCast(@as(u64, st.ino));
}

// ------------------------------------------------------------------ java.nio.MappedMemoryUtils
extern "c" fn mincore(addr: *anyopaque, len: usize, vec: [*]u8) c_int;
extern "c" fn madvise(addr: *anyopaque, len: usize, advice: c_int) c_int;
extern "c" fn msync(addr: *anyopaque, len: usize, flags: c_int) c_int;

pub fn J_java_nio_MappedMemoryUtils__registerNatives() E!void {}

pub fn J_java_nio_MappedMemoryUtils__isLoaded0(a: i64, len: i64, pages: i64) E!i32 {
    const n: usize = @intCast(pages);
    const vec = std.heap.c_allocator.alloc(u8, n) catch return rt.throwKind(.OutOfMemory, "isLoaded");
    defer std.heap.c_allocator.free(vec);
    if (mincore(@ptrFromInt(addr(a)), @intCast(len), vec.ptr) != 0) return ioFail("mincore failed");
    for (vec) |v| if (v & 1 == 0) return 0;
    return 1;
}
pub fn J_java_nio_MappedMemoryUtils__load0(a: i64, len: i64) E!void {
    if (madvise(@ptrFromInt(addr(a)), @intCast(len), 3) != 0) return ioFail("madvise failed"); // MADV_WILLNEED
}
pub fn J_java_nio_MappedMemoryUtils__unload0(a: i64, len: i64) E!void {
    if (madvise(@ptrFromInt(addr(a)), @intCast(len), 4) != 0) return ioFail("madvise failed"); // MADV_DONTNEED
}
pub fn J_java_nio_MappedMemoryUtils__force0(fdo: Ref, a: i64, len: i64) E!void {
    _ = fdo;
    if (msync(@ptrFromInt(addr(a)), @intCast(len), 4) != 0) return ioFail("msync failed"); // MS_SYNC
}
