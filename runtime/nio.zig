//! sun.nio.fs.UnixNativeDispatcher natives (java.nio.file on Linux), implemented directly on
//! libc. Errors are reported like the JDK: `throw new UnixException(errno)`.
const std = @import("std");
const rt = @import("rt.zig");
const P = @import("classes.zig");
const c = std.c;

const Ref = rt.Ref;
const E = rt.E;

/// errno location through std.c: `__errno` on Android (bionic has no `__errno_location`), the glibc name elsewhere.
inline fn __errno_location() *c_int {
    return std.c._errno();
}
extern "c" fn open(path: [*:0]const u8, flags: c_int, ...) c_int;
extern "c" fn openat(fd: c_int, path: [*:0]const u8, flags: c_int, ...) c_int;
extern "c" fn close(fd: c_int) c_int;
extern "c" fn dup(fd: c_int) c_int;
extern "c" fn link(a: [*:0]const u8, b: [*:0]const u8) c_int;
extern "c" fn unlink(a: [*:0]const u8) c_int;
extern "c" fn unlinkat(fd: c_int, a: [*:0]const u8, flags: c_int) c_int;
extern "c" fn mknod(a: [*:0]const u8, mode: c_uint, dev: u64) c_int;
extern "c" fn rename(a: [*:0]const u8, b: [*:0]const u8) c_int;
extern "c" fn renameat(fa: c_int, a: [*:0]const u8, fb: c_int, b: [*:0]const u8) c_int;
extern "c" fn mkdir(a: [*:0]const u8, mode: c_uint) c_int;
extern "c" fn rmdir(a: [*:0]const u8) c_int;
extern "c" fn readlink(a: [*:0]const u8, buf: [*]u8, n: usize) isize;
extern "c" fn realpath(a: [*:0]const u8, buf: ?[*]u8) ?[*:0]u8;
extern "c" fn symlink(a: [*:0]const u8, b: [*:0]const u8) c_int;
extern "c" fn chown(a: [*:0]const u8, u: c_uint, g: c_uint) c_int;
extern "c" fn lchown(a: [*:0]const u8, u: c_uint, g: c_uint) c_int;
extern "c" fn fchown(fd: c_int, u: c_uint, g: c_uint) c_int;
extern "c" fn chmod(a: [*:0]const u8, m: c_uint) c_int;
extern "c" fn fchmod(fd: c_int, m: c_uint) c_int;
extern "c" fn fchmodat(fd: c_int, a: [*:0]const u8, m: c_uint, flags: c_int) c_int;
extern "c" fn futimens(fd: c_int, times: *const [2]std.c.timespec) c_int;
extern "c" fn utimensat(fd: c_int, a: [*:0]const u8, times: *const [2]std.c.timespec, flags: c_int) c_int;
extern "c" fn opendir(a: [*:0]const u8) ?*anyopaque;
extern "c" fn fdopendir(fd: c_int) ?*anyopaque;
extern "c" fn closedir(d: *anyopaque) c_int;
extern "c" fn rewinddir(d: *anyopaque) void;
extern "c" fn rewind(f: *anyopaque) void;
extern "c" fn getline(line: *?[*]u8, n: *usize, f: *anyopaque) isize;
extern "c" fn free(p: ?*anyopaque) void;
extern "c" fn readdir(d: *anyopaque) ?*extern struct { ino: u64, off: i64, reclen: u16, type: u8, name: [256]u8 };
extern "c" fn read(fd: c_int, buf: [*]u8, n: usize) isize;
extern "c" fn write(fd: c_int, buf: [*]const u8, n: usize) isize;
extern "c" fn access(a: [*:0]const u8, m: c_int) c_int;
extern "c" fn getcwd(buf: [*]u8, n: usize) ?[*:0]u8;
extern "c" fn strerror(e: c_int) [*:0]const u8;
extern "c" fn stat(a: [*:0]const u8, st: *std.c.Stat) c_int;
extern "c" fn lstat(a: [*:0]const u8, st: *std.c.Stat) c_int;
extern "c" fn fstat(fd: c_int, st: *std.c.Stat) c_int;
extern "c" fn fstatat(fd: c_int, a: [*:0]const u8, st: *std.c.Stat, flags: c_int) c_int;
const StatVfs = extern struct {
    f_bsize: c_ulong,
    f_frsize: c_ulong,
    f_blocks: u64,
    f_bfree: u64,
    f_bavail: u64,
    f_files: u64,
    f_ffree: u64,
    f_favail: u64,
    f_fsid: c_ulong,
    f_flag: c_ulong,
    f_namemax: c_ulong,
    spare: [6]c_int,
};
extern "c" fn statvfs(a: [*:0]const u8, st: *StatVfs) c_int;

inline fn path(a: i64) [*:0]const u8 {
    return @ptrFromInt(@as(usize, @bitCast(a)));
}
inline fn errno() i32 {
    return __errno_location().*;
}

/// throw new UnixException(errno)
fn fail() E {
    const e = errno();
    const ex = P.newUnixException(e) catch return error.Throw;
    rt.cur.exc = ex;
    return error.Throw;
}

fn check(r: c_int) E!void {
    if (r == -1) return fail();
}

fn bytes(b: []const u8) E!Ref {
    const a = try rt.newArray(rt.arrayOf(rt.prim_B), @intCast(b.len));
    @memcpy(rt.arrayData(u8, rt.asArray(a.?))[0..b.len], b);
    return a;
}

/// Blocking syscalls run in the GC-safe state.
inline fn safe() void {
    rt.enterSafe();
}
inline fn unsafe() void {
    rt.leaveSafe();
}

const S = "J_sun_nio_fs_UnixNativeDispatcher__";
const nmeta = rt.MethodMeta{ .class = "<runtime>", .name = "<nio>", .desc = "" };
pub fn J_sun_nio_fs_UnixNativeDispatcher__init() E!i32 {
    return 2; // SUPPORTS_OPENAT (no xattr, no birthtime)
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__getcwd() E!Ref {
    var buf: [std.fs.max_path_bytes]u8 = undefined;
    const r = getcwd(&buf, buf.len) orelse return fail();
    return bytes(std.mem.span(r));
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__dup(fd: i32) E!i32 {
    const r = dup(fd);
    if (r == -1) return fail();
    return r;
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__open0(p: i64, flags: i32, mode: i32) E!i32 {
    safe();
    const r = open(path(p), flags, @as(c_uint, @bitCast(mode)));
    unsafe();
    if (r == -1) return fail();
    return r;
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__openat0(dfd: i32, p: i64, flags: i32, mode: i32) E!i32 {
    safe();
    const r = openat(dfd, path(p), flags, @as(c_uint, @bitCast(mode)));
    unsafe();
    if (r == -1) return fail();
    return r;
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__close0(fd: i32) E!void {
    if (close(fd) == -1 and errno() != 4) return fail(); // EINTR is ignored like the JDK
}
/// rewind(FILE *) — the stream is the FILE* of setmntent/fopen (not a DIR*)
pub fn J_sun_nio_fs_UnixNativeDispatcher__rewind(f: i64) E!void {
    rewind(@ptrFromInt(@as(usize, @bitCast(f))));
}
/// Length of the next line of the stream (getline), -1 at EOF; callers take the maximum to size
/// the getmntent buffer.
pub fn J_sun_nio_fs_UnixNativeDispatcher__getlinelen(f: i64) E!i32 {
    const fp: *anyopaque = @ptrFromInt(@as(usize, @bitCast(f)));
    var buf: ?[*]u8 = null;
    var cap: usize = 0;
    const n = getline(&buf, &cap, fp);
    free(buf);
    return if (n < 0) -1 else @intCast(n);
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__link0(a: i64, b: i64) E!void {
    return check(link(path(a), path(b)));
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__unlink0(a: i64) E!void {
    return check(unlink(path(a)));
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__unlinkat0(fd: i32, a: i64, flags: i32) E!void {
    return check(unlinkat(fd, path(a), flags));
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__mknod0(a: i64, mode: i32, dev: i64) E!void {
    return check(mknod(path(a), @bitCast(mode), @bitCast(dev)));
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__rename0(a: i64, b: i64) E!void {
    return check(rename(path(a), path(b)));
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__renameat0(fa: i32, a: i64, fb: i32, b: i64) E!void {
    return check(renameat(fa, path(a), fb, path(b)));
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__mkdir0(a: i64, mode: i32) E!void {
    return check(mkdir(path(a), @bitCast(mode)));
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__rmdir0(a: i64) E!void {
    return check(rmdir(path(a)));
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__readlink0(a: i64) E!Ref {
    var buf: [std.fs.max_path_bytes]u8 = undefined;
    const n = readlink(path(a), &buf, buf.len);
    if (n < 0) return fail();
    return bytes(buf[0..@intCast(n)]);
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__realpath0(a: i64) E!Ref {
    var buf: [std.fs.max_path_bytes + 1]u8 = undefined;
    const r = realpath(path(a), &buf) orelse return fail();
    return bytes(std.mem.span(r));
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__symlink0(a: i64, b: i64) E!void {
    return check(symlink(path(a), path(b)));
}

fn fillAttrs(st: *const std.c.Stat, attrs: Ref) void {
    const A = P.C_sun_nio_fs_UnixFileAttributes;
    const o = A.cast(attrs.?);
    o.f_st_1mode = @bitCast(st.mode);
    o.f_st_1ino = @bitCast(st.ino);
    o.f_st_1dev = @bitCast(st.dev);
    o.f_st_1rdev = @bitCast(st.rdev);
    o.f_st_1nlink = @intCast(st.nlink);
    o.f_st_1uid = @bitCast(st.uid);
    o.f_st_1gid = @bitCast(st.gid);
    o.f_st_1size = st.size;
    o.f_st_1atime_1sec = st.atim.sec;
    o.f_st_1atime_1nsec = st.atim.nsec;
    o.f_st_1mtime_1sec = st.mtim.sec;
    o.f_st_1mtime_1nsec = st.mtim.nsec;
    o.f_st_1ctime_1sec = st.ctim.sec;
    o.f_st_1ctime_1nsec = st.ctim.nsec;
}

pub fn J_sun_nio_fs_UnixNativeDispatcher__stat0(a: i64, attrs: Ref) E!i32 {
    var st: std.c.Stat = undefined;
    safe();
    const r = stat(path(a), &st);
    unsafe();
    if (r == -1) return errno();
    fillAttrs(&st, attrs);
    return 0;
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__lstat0(a: i64, attrs: Ref) E!void {
    var st: std.c.Stat = undefined;
    if (lstat(path(a), &st) == -1) return fail();
    fillAttrs(&st, attrs);
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__fstat0(fd: i32, attrs: Ref) E!void {
    var st: std.c.Stat = undefined;
    if (fstat(fd, &st) == -1) return fail();
    fillAttrs(&st, attrs);
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__fstatat0(dfd: i32, a: i64, flags: i32, attrs: Ref) E!void {
    var st: std.c.Stat = undefined;
    if (fstatat(dfd, path(a), &st, flags) == -1) return fail();
    fillAttrs(&st, attrs);
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__chown0(a: i64, u: i32, g: i32) E!void {
    return check(chown(path(a), @bitCast(u), @bitCast(g)));
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__lchown0(a: i64, u: i32, g: i32) E!void {
    return check(lchown(path(a), @bitCast(u), @bitCast(g)));
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__fchown0(fd: i32, u: i32, g: i32) E!void {
    return check(fchown(fd, @bitCast(u), @bitCast(g)));
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__chmod0(a: i64, m: i32) E!void {
    return check(chmod(path(a), @bitCast(m)));
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__fchmod0(fd: i32, m: i32) E!void {
    return check(fchmod(fd, @bitCast(m)));
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__fchmodat0(fd: i32, a: i64, m: i32, flags: i32) E!void {
    return check(fchmodat(fd, path(a), @bitCast(m), flags));
}
fn ts(ns: i64) std.c.timespec {
    return .{ .sec = @divFloor(ns, 1_000_000_000), .nsec = @mod(ns, 1_000_000_000) };
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__futimens0(fd: i32, at: i64, mt: i64) E!void {
    const t = [2]std.c.timespec{ ts(at), ts(mt) };
    return check(futimens(fd, &t));
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__utimensat0(fd: i32, a: i64, at: i64, mt: i64, flags: i32) E!void {
    const t = [2]std.c.timespec{ ts(at), ts(mt) };
    return check(utimensat(fd, path(a), &t, flags));
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__opendir0(a: i64) E!i64 {
    const d = opendir(path(a)) orelse return fail();
    return @bitCast(@as(u64, @intFromPtr(d)));
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__fdopendir(fd: i32) E!i64 {
    const d = fdopendir(fd) orelse return fail();
    return @bitCast(@as(u64, @intFromPtr(d)));
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__closedir(d: i64) E!void {
    if (closedir(@ptrFromInt(@as(usize, @bitCast(d)))) == -1 and errno() != 4) return fail();
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__readdir0(d: i64) E!Ref {
    __errno_location().* = 0;
    const e = readdir(@ptrFromInt(@as(usize, @bitCast(d)))) orelse {
        if (errno() != 0) return fail();
        return null;
    };
    return bytes(std.mem.sliceTo(&e.name, 0));
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__read0(fd: i32, buf: i64, n: i32) E!i32 {
    safe();
    const r = read(fd, @ptrFromInt(@as(usize, @bitCast(buf))), @intCast(n));
    unsafe();
    if (r < 0) return fail();
    return @intCast(r);
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__write0(fd: i32, buf: i64, n: i32) E!i32 {
    safe();
    const r = write(fd, @ptrFromInt(@as(usize, @bitCast(buf))), @intCast(n));
    unsafe();
    if (r < 0) return fail();
    return @intCast(r);
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__access0(a: i64, m: i32) E!i32 {
    return if (access(path(a), m) == -1) errno() else 0;
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__getpwuid(uid: i32) E!Ref {
    var buf: [32]u8 = undefined;
    return bytes(std.fmt.bufPrint(&buf, "{d}", .{uid}) catch "0");
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__getgrgid(gid: i32) E!Ref {
    var buf: [32]u8 = undefined;
    return bytes(std.fmt.bufPrint(&buf, "{d}", .{gid}) catch "0");
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__getpwnam0(n: i64) E!i32 {
    _ = n;
    return -1;
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__getgrnam0(n: i64) E!i32 {
    _ = n;
    return -1;
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__statvfs0(a: i64, attrs: Ref) E!void {
    var st: StatVfs = undefined;
    if (statvfs(path(a), &st) == -1) return fail();
    const o = P.C_sun_nio_fs_UnixFileStoreAttributes.cast(attrs.?);
    o.f_f_1frsize = @intCast(st.f_frsize);
    o.f_f_1blocks = @intCast(st.f_blocks);
    o.f_f_1bfree = @intCast(st.f_bfree);
    o.f_f_1bavail = @intCast(st.f_bavail);
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__strerror(e: i32) E!Ref {
    return bytes(std.mem.span(strerror(e)));
}
fn noXattr() E {
    __errno_location().* = 95; // ENOTSUP
    return fail();
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__fgetxattr0(fd: i32, n: i64, v: i64, l: i32) E!i32 {
    _ = fd;
    _ = n;
    _ = v;
    _ = l;
    return noXattr();
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__fsetxattr0(fd: i32, n: i64, v: i64, l: i32) E!void {
    _ = fd;
    _ = n;
    _ = v;
    _ = l;
    return noXattr();
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__fremovexattr0(fd: i32, n: i64) E!void {
    _ = fd;
    _ = n;
    return noXattr();
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__flistxattr(fd: i32, l: i64, n: i32) E!i32 {
    _ = fd;
    _ = l;
    _ = n;
    return noXattr();
}
pub fn J_sun_nio_fs_UnixNativeDispatcher__fchmodatNoFollowSupported0() E!i32 {
    return 0;
}

comptime {
    _ = S;
}

// ------------------------------------------------------------------ sun.nio.fs.LinuxNativeDispatcher
const MntEnt = extern struct { fsname: [*:0]u8, dir: [*:0]u8, type: [*:0]u8, opts: [*:0]u8, freq: c_int, passno: c_int };
extern "c" fn setmntent(p: [*:0]const u8, m: [*:0]const u8) ?*anyopaque;
extern "c" fn getmntent_r(f: *anyopaque, e: *MntEnt, buf: [*]u8, n: c_int) ?*MntEnt;
extern "c" fn endmntent(f: *anyopaque) c_int;
extern "c" fn posix_fadvise(fd: c_int, off: i64, len: i64, advice: c_int) c_int;
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

pub fn J_sun_nio_fs_LinuxNativeDispatcher__init() E!void {}
pub fn J_sun_nio_fs_LinuxNativeDispatcher__setmntent0(p: i64, m: i64) E!i64 {
    const f = setmntent(path(p), path(m)) orelse return fail();
    return @bitCast(@as(u64, @intFromPtr(f)));
}
pub fn J_sun_nio_fs_LinuxNativeDispatcher__getmntent0(f: i64, entry: Ref, buf: i64, len: i32) E!i32 {
    var e: MntEnt = undefined;
    _ = getmntent_r(@ptrFromInt(@as(usize, @bitCast(f))), &e, @ptrFromInt(@as(usize, @bitCast(buf))), len) orelse return -1;
    var R: [2]Ref = .{ entry, null };
    var F = rt.Frame{ .refs = &R, .n = 2, .meth = &nmeta };
    try rt.pushFrame(&F);
    defer rt.popFrame(&F);
    const C = P.C_sun_nio_fs_UnixMountEntry;
    R[1] = try bytes(std.mem.span(e.fsname));
    C.cast(R[0].?).f_name = R[1];
    R[1] = try bytes(std.mem.span(e.dir));
    C.cast(R[0].?).f_dir = R[1];
    R[1] = try bytes(std.mem.span(e.type));
    C.cast(R[0].?).f_fstype = R[1];
    R[1] = try bytes(std.mem.span(e.opts));
    C.cast(R[0].?).f_opts = R[1];
    return 0;
}
pub fn J_sun_nio_fs_LinuxNativeDispatcher__endmntent(f: i64) E!void {
    _ = endmntent(@ptrFromInt(@as(usize, @bitCast(f))));
}
pub fn J_sun_nio_fs_LinuxNativeDispatcher__posix_1fadvise(fd: i32, off: i64, len: i64, advice: i32) E!i32 {
    return posix_fadvise(fd, off, len, advice);
}
/// Kernel-side file copy; IOS_UNSUPPORTED_CASE (-6) asks the caller to fall back to a
/// user-space copy, as the JDK does.
pub fn J_sun_nio_fs_LinuxNativeDispatcher__directCopy0(dst: i32, src: i32, cancel: i64) E!i32 {
    const flag: ?*const volatile i32 = if (cancel != 0) @ptrFromInt(@as(usize, @bitCast(cancel))) else null;
    var copied: usize = 0;
    while (true) {
        if (flag) |cf| if (cf.* != 0) {
            __errno_location().* = 125; // ECANCELED
            return fail();
        };
        safe();
        const n = copy_file_range(src, null, dst, null, 1 << 30, 0);
        unsafe();
        if (n == 0) return 0;
        if (n < 0) {
            const e = errno();
            if (copied == 0 and (e == 18 or e == 38 or e == 22 or e == 95 or e == 1 or e == 9)) return -6;
            if (e == 4) continue;
            return fail();
        }
        copied += @intCast(n);
    }
}
