//! java.io / java.util.zip natives (POSIX file descriptors, zlib).
const std = @import("std");
const rt = @import("rt.zig");
const P = @import("classes.zig");
const posix = std.posix;

const Ref = rt.Ref;
const E = rt.E;

const z = @cImport(@cInclude("zlib.h"));

fn bytes(r: Ref) E![]u8 {
    const a = rt.asArray(try rt.nn(r));
    return rt.arrayData(u8, a)[0..@intCast(a.length)];
}

fn range(r: Ref, off: i32, len: i32) E![]u8 {
    const b = try bytes(r);
    if (off < 0 or len < 0 or @as(i64, off) + len > b.len) return rt.throwKind(.ArrayIndex, null);
    return b[@intCast(off)..@intCast(off + len)];
}

fn pathZ(buf: []u8, s: Ref) E![:0]u8 {
    const p = rt.stringToUtf8(rt.gpa, s) catch return rt.throwKind(.OutOfMemory, null);
    defer rt.gpa.free(p);
    if (p.len >= buf.len) return rt.throwKind(.FileNotFound, "path too long");
    @memcpy(buf[0..p.len], p);
    buf[p.len] = 0;
    return buf[0..p.len :0];
}

fn ioError(prefix: []const u8, err: anyerror) E {
    var buf: [512]u8 = undefined;
    const msg = std.fmt.bufPrint(&buf, "{s}{s}", .{ prefix, errnoText(err) }) catch "I/O error";
    return rt.throwKind(.IO, msg);
}

fn errnoText(err: anyerror) []const u8 {
    return switch (err) {
        error.FileNotFound => "No such file or directory",
        error.AccessDenied, error.PermissionDenied => "Permission denied",
        error.IsDir => "Is a directory",
        error.NotDir => "Not a directory",
        error.PathAlreadyExists => "File exists",
        error.NoSpaceLeft => "No space left on device",
        error.BrokenPipe => "Broken pipe",
        else => @errorName(err),
    };
}

// FileDescriptor objects: field `fd` (int)
fn fdOf(stream: Ref, comptime C: type) E!i32 {
    const fdo = C.cast(try rt.nn(stream)).f_fd orelse return rt.throwKind(.IO, "Stream Closed");
    const fd = P.C_java_io_FileDescriptor.cast(fdo).f_fd;
    if (fd < 0) return rt.throwKind(.IO, "Stream Closed");
    return fd;
}

fn setFd(stream: Ref, comptime C: type, fd: i32) E!void {
    const fdo = C.cast(try rt.nn(stream)).f_fd orelse return rt.throwKind(.IO, "no FileDescriptor");
    P.C_java_io_FileDescriptor.cast(fdo).f_fd = fd;
}

fn openFile(path: Ref, flags: posix.O, mode: posix.mode_t) E!i32 {
    var buf: [4096]u8 = undefined;
    const p = try pathZ(&buf, path);
    rt.enterSafe();
    const r = posix.openZ(p, flags, mode);
    rt.leaveSafe();
    const fd = r catch |err| {
        var mb: [4200]u8 = undefined;
        const msg = std.fmt.bufPrint(&mb, "{s} ({s})", .{ p, errnoText(err) }) catch "open failed";
        return rt.throwKind(.FileNotFound, msg);
    };
    // the JDK refuses to open directories as streams
    const st = posix.fstat(fd) catch null;
    if (st) |s| if (posix.S.ISDIR(s.mode)) {
        posix.close(fd);
        var mb: [4200]u8 = undefined;
        return rt.throwKind(.FileNotFound, std.fmt.bufPrint(&mb, "{s} (Is a directory)", .{p}) catch "Is a directory");
    };
    return fd;
}

pub fn J_java_io_FileDescriptor__initIDs() E!void {}
pub fn J_java_io_FileInputStream__initIDs() E!void {}
pub fn J_java_io_FileOutputStream__initIDs() E!void {}
pub fn J_java_io_RandomAccessFile__initIDs() E!void {}
pub fn J_java_io_UnixFileSystem__initIDs() E!void {}

pub fn J_java_io_FileDescriptor__getHandle(fd: i32) E!i64 {
    _ = fd;
    return -1;
}
pub fn J_java_io_FileDescriptor__getAppend(fd: i32) E!i32 {
    const fl = posix.fcntl(fd, posix.F.GETFL, 0) catch return 0;
    return @intFromBool(fl & @as(usize, @bitCast(@as(isize, @as(u32, @bitCast(posix.O{ .APPEND = true }))))) != 0);
}
pub fn J_java_io_FileDescriptor__close0(this: Ref) E!void {
    const o = P.C_java_io_FileDescriptor.cast(try rt.nn(this));
    const fd = o.f_fd;
    if (fd < 0) return;
    o.f_fd = -1;
    if (fd > 2) posix.close(fd);
}
pub fn J_java_io_FileDescriptor__sync0(this: Ref) E!void {
    const fd = P.C_java_io_FileDescriptor.cast(try rt.nn(this)).f_fd;
    posix.fsync(fd) catch {};
}
pub fn J_java_io_FileCleanable__cleanupClose0(fd: i32, handle: i64) E!void {
    _ = handle;
    if (fd > 2) posix.close(fd);
}

// ---------------------------------------------------------------- FileInputStream
const FIS = P.C_java_io_FileInputStream;
pub fn J_java_io_FileInputStream__open0(this: Ref, path: Ref) E!void {
    const fd = try openFile(path, .{ .ACCMODE = .RDONLY, .CLOEXEC = true }, 0);
    try setFd(this, FIS, fd);
}
pub fn J_java_io_FileInputStream__read0(this: Ref) E!i32 {
    const fd = try fdOf(this, FIS);
    var b: [1]u8 = undefined;
    rt.enterSafe();
    const n = posix.read(fd, &b);
    rt.leaveSafe();
    const k = n catch |err| return ioError("Read error: ", err);
    return if (k == 0) -1 else b[0];
}
pub fn J_java_io_FileInputStream__readBytes(this: Ref, arr: Ref, off: i32, len: i32) E!i32 {
    const dst = try range(arr, off, len);
    if (len == 0) return 0;
    const fd = try fdOf(this, FIS);
    var tmp: [65536]u8 = undefined;
    const want = @min(dst.len, tmp.len);
    rt.enterSafe();
    const n = posix.read(fd, tmp[0..want]);
    rt.leaveSafe();
    const k = n catch |err| return ioError("Read error: ", err);
    if (k == 0) return -1;
    @memcpy(dst[0..k], tmp[0..k]);
    return @intCast(k);
}
pub fn J_java_io_FileInputStream__length0(this: Ref) E!i64 {
    const st = posix.fstat(try fdOf(this, FIS)) catch |err| return ioError("", err);
    return st.size;
}
pub fn J_java_io_FileInputStream__position0(this: Ref) E!i64 {
    const p = posix.lseek_CUR_get(try fdOf(this, FIS)) catch |err| return ioError("", err);
    return @intCast(p);
}
pub fn J_java_io_FileInputStream__skip0(this: Ref, n: i64) E!i64 {
    const fd = try fdOf(this, FIS);
    const cur = posix.lseek_CUR_get(fd) catch |err| return ioError("", err);
    posix.lseek_CUR(fd, n) catch |err| return ioError("", err);
    const now = posix.lseek_CUR_get(fd) catch |err| return ioError("", err);
    return @as(i64, @intCast(now)) - @as(i64, @intCast(cur));
}
pub fn J_java_io_FileInputStream__available0(this: Ref) E!i32 {
    const fd = try fdOf(this, FIS);
    const st = posix.fstat(fd) catch return 0;
    if (!posix.S.ISREG(st.mode)) return 0;
    const cur = posix.lseek_CUR_get(fd) catch return 0;
    const left = st.size - @as(i64, @intCast(cur));
    return @intCast(@max(0, @min(left, std.math.maxInt(i32))));
}

// ---------------------------------------------------------------- FileOutputStream
const FOS = P.C_java_io_FileOutputStream;
pub fn J_java_io_FileOutputStream__open0(this: Ref, path: Ref, append: i32) E!void {
    const fd = try openFile(path, .{ .ACCMODE = .WRONLY, .CREAT = true, .TRUNC = append == 0, .APPEND = append != 0, .CLOEXEC = true }, 0o666);
    try setFd(this, FOS, fd);
}
pub fn J_java_io_FileOutputStream__write(this: Ref, b: i32, append: i32) E!void {
    _ = append;
    const fd = try fdOf(this, FOS);
    const one = [1]u8{@truncate(@as(u32, @bitCast(b)))};
    rt.writeFd(fd, &one);
}
pub fn J_java_io_FileOutputStream__writeBytes(this: Ref, arr: Ref, off: i32, len: i32, append: i32) E!void {
    _ = append;
    const src = try range(arr, off, len);
    const fd = try fdOf(this, FOS);
    var done: usize = 0;
    while (done < src.len) {
        const n = posix.write(fd, src[done..]) catch |err| return ioError("Write error: ", err);
        done += n;
    }
}

// ---------------------------------------------------------------- RandomAccessFile
const RAF = P.C_java_io_RandomAccessFile;
pub fn J_java_io_RandomAccessFile__open0(this: Ref, path: Ref, mode: i32) E!void {
    // java.io.RandomAccessFile: O_RDONLY = 1, O_RDWR = 2, O_SYNC = 4, O_DSYNC = 8
    const rw = mode & 2 != 0;
    const fd = try openFile(path, .{ .ACCMODE = if (rw) .RDWR else .RDONLY, .CREAT = rw, .CLOEXEC = true }, 0o666);
    try setFd(this, RAF, fd);
}
pub fn J_java_io_RandomAccessFile__read0(this: Ref) E!i32 {
    const fd = try fdOf(this, RAF);
    var b: [1]u8 = undefined;
    const k = posix.read(fd, &b) catch |err| return ioError("Read error: ", err);
    return if (k == 0) -1 else b[0];
}
pub fn J_java_io_RandomAccessFile__readBytes0(this: Ref, arr: Ref, off: i32, len: i32) E!i32 {
    const dst = try range(arr, off, len);
    if (len == 0) return 0;
    const fd = try fdOf(this, RAF);
    const k = posix.read(fd, dst) catch |err| return ioError("Read error: ", err);
    return if (k == 0) -1 else @intCast(k);
}
pub fn J_java_io_RandomAccessFile__write0(this: Ref, b: i32) E!void {
    const one = [1]u8{@truncate(@as(u32, @bitCast(b)))};
    rt.writeFd(try fdOf(this, RAF), &one);
}
pub fn J_java_io_RandomAccessFile__writeBytes0(this: Ref, arr: Ref, off: i32, len: i32) E!void {
    const src = try range(arr, off, len);
    const fd = try fdOf(this, RAF);
    var done: usize = 0;
    while (done < src.len) done += posix.write(fd, src[done..]) catch |err| return ioError("Write error: ", err);
}
pub fn J_java_io_RandomAccessFile__getFilePointer(this: Ref) E!i64 {
    return @intCast(posix.lseek_CUR_get(try fdOf(this, RAF)) catch |err| return ioError("", err));
}
pub fn J_java_io_RandomAccessFile__seek0(this: Ref, pos: i64) E!void {
    if (pos < 0) return rt.throwKind(.IO, "Negative seek offset");
    posix.lseek_SET(try fdOf(this, RAF), @intCast(pos)) catch |err| return ioError("", err);
}
pub fn J_java_io_RandomAccessFile__length0(this: Ref) E!i64 {
    const st = posix.fstat(try fdOf(this, RAF)) catch |err| return ioError("", err);
    return st.size;
}
pub fn J_java_io_RandomAccessFile__setLength0(this: Ref, n: i64) E!void {
    posix.ftruncate(try fdOf(this, RAF), @intCast(n)) catch |err| return ioError("", err);
}

// ---------------------------------------------------------------- UnixFileSystem
fn filePath(buf: []u8, f: Ref) E![:0]u8 {
    return pathZ(buf, P.C_java_io_File.cast(try rt.nn(f)).f_path);
}
pub fn J_java_io_UnixFileSystem__getBooleanAttributes0(this: Ref, f: Ref) E!i32 {
    _ = this;
    var buf: [4096]u8 = undefined;
    const p = try filePath(&buf, f);
    const st = posix.fstatatZ(posix.AT.FDCWD, p, 0) catch return 0;
    // BA_EXISTS = 1, BA_REGULAR = 2, BA_DIRECTORY = 4, BA_HIDDEN = 8 (computed in Java)
    var r: i32 = 1;
    if (posix.S.ISREG(st.mode)) r |= 2;
    if (posix.S.ISDIR(st.mode)) r |= 4;
    return r;
}
pub fn J_java_io_UnixFileSystem__checkAccess0(this: Ref, f: Ref, access: i32) E!i32 {
    _ = this;
    var buf: [4096]u8 = undefined;
    const p = try filePath(&buf, f);
    // ACCESS_READ = 4, ACCESS_WRITE = 2, ACCESS_EXECUTE = 1 (same as POSIX R_OK/W_OK/X_OK)
    posix.accessZ(p, @intCast(access)) catch return 0;
    return 1;
}
pub fn J_java_io_UnixFileSystem__getLastModifiedTime0(this: Ref, f: Ref) E!i64 {
    _ = this;
    var buf: [4096]u8 = undefined;
    const st = posix.fstatatZ(posix.AT.FDCWD, try filePath(&buf, f), 0) catch return 0;
    return @as(i64, st.mtim.sec) * 1000 + @divTrunc(st.mtim.nsec, 1000000);
}
pub fn J_java_io_UnixFileSystem__getLength0(this: Ref, f: Ref) E!i64 {
    _ = this;
    var buf: [4096]u8 = undefined;
    const st = posix.fstatatZ(posix.AT.FDCWD, try filePath(&buf, f), 0) catch return 0;
    return st.size;
}
pub fn J_java_io_UnixFileSystem__createFileExclusively0(this: Ref, path: Ref) E!i32 {
    _ = this;
    var buf: [4096]u8 = undefined;
    const p = try pathZ(&buf, path);
    const fd = posix.openZ(p, .{ .ACCMODE = .RDWR, .CREAT = true, .EXCL = true, .CLOEXEC = true }, 0o666) catch |err| {
        if (err == error.PathAlreadyExists) return 0;
        return ioError("", err);
    };
    posix.close(fd);
    return 1;
}
pub fn J_java_io_UnixFileSystem__delete0(this: Ref, f: Ref) E!i32 {
    _ = this;
    var buf: [4096]u8 = undefined;
    const p = try filePath(&buf, f);
    posix.unlinkZ(p) catch {
        posix.rmdirZ(p) catch return 0;
    };
    return 1;
}
pub fn J_java_io_UnixFileSystem__createDirectory0(this: Ref, f: Ref) E!i32 {
    _ = this;
    var buf: [4096]u8 = undefined;
    posix.mkdirZ(try filePath(&buf, f), 0o777) catch return 0;
    return 1;
}
pub fn J_java_io_UnixFileSystem__rename0(this: Ref, a: Ref, b: Ref) E!i32 {
    _ = this;
    var b1: [4096]u8 = undefined;
    var b2: [4096]u8 = undefined;
    posix.renameZ(try filePath(&b1, a), try filePath(&b2, b)) catch return 0;
    return 1;
}
pub fn J_java_io_UnixFileSystem__canonicalize0(this: Ref, path: Ref) E!Ref {
    _ = this;
    var buf: [4096]u8 = undefined;
    const p = try pathZ(&buf, path);
    var out: [std.fs.max_path_bytes]u8 = undefined;
    const r = std.fs.realpathZ(p, &out) catch return path; // the JDK falls back to the
    // (already normalized) input for paths that do not exist
    return rt.newStringUtf8(r);
}
pub fn J_java_io_UnixFileSystem__list0(this: Ref, f: Ref) E!Ref {
    _ = this;
    var buf: [4096]u8 = undefined;
    const p = try filePath(&buf, f);
    var dir = std.fs.cwd().openDirZ(p, .{ .iterate = true }) catch return null;
    defer dir.close();
    var names: std.ArrayList([]u8) = .empty;
    defer {
        for (names.items) |n| rt.gpa.free(n);
        names.deinit(rt.gpa);
    }
    var it = dir.iterate();
    while (it.next() catch null) |e| {
        names.append(rt.gpa, rt.gpa.dupe(u8, e.name) catch return rt.throwKind(.OutOfMemory, null)) catch return rt.throwKind(.OutOfMemory, null);
    }
    var R: [2]Ref = .{ null, null };
    var F = rt.Frame{ .refs = &R, .n = 2, .meth = &meta };
    try rt.pushFrame(&F);
    defer rt.popFrame(&F);
    R[0] = try rt.newArray(rt.arrayOf(P.C_java_lang_String.cls()), @intCast(names.items.len));
    for (names.items, 0..) |n, i| {
        R[1] = try rt.newStringUtf8(n);
        try rt.aastore(R[0], @intCast(i), R[1]);
    }
    return R[0];
}
pub fn J_java_io_UnixFileSystem__setLastModifiedTime0(this: Ref, f: Ref, t: i64) E!i32 {
    _ = this;
    _ = f;
    _ = t;
    return 0;
}
pub fn J_java_io_UnixFileSystem__setReadOnly0(this: Ref, f: Ref) E!i32 {
    _ = this;
    var buf: [4096]u8 = undefined;
    posix.fchmodatZ(posix.AT.FDCWD, try filePath(&buf, f), 0o444, 0) catch return 0;
    return 1;
}
pub fn J_java_io_UnixFileSystem__getNameMax0(this: Ref, p: Ref) E!i64 {
    _ = this;
    _ = p;
    return 255;
}
const meta = rt.MethodMeta{ .class = "<runtime>", .name = "<native>", .desc = "" };

// ---------------------------------------------------------------- java.util.zip checksums
pub fn J_java_util_zip_CRC32__update(crc: i32, b: i32) E!i32 {
    const one = [1]u8{@truncate(@as(u32, @bitCast(b)))};
    return @bitCast(@as(u32, @truncate(z.crc32(@as(c_ulong, @as(u32, @bitCast(crc))), &one, 1))));
}
pub fn J_java_util_zip_CRC32__updateBytes0(crc: i32, arr: Ref, off: i32, len: i32) E!i32 {
    const src = try range(arr, off, len);
    return @bitCast(@as(u32, @truncate(z.crc32(@as(c_ulong, @as(u32, @bitCast(crc))), src.ptr, @intCast(src.len)))));
}
pub fn J_java_util_zip_CRC32__updateByteBuffer0(crc: i32, addr: i64, off: i32, len: i32) E!i32 {
    const p: [*]const u8 = @ptrFromInt(@as(usize, @bitCast(addr)) +% @as(usize, @intCast(off)));
    return @bitCast(@as(u32, @truncate(z.crc32(@as(c_ulong, @as(u32, @bitCast(crc))), p, @intCast(len)))));
}
pub fn J_java_util_zip_Adler32__update(adler: i32, b: i32) E!i32 {
    const one = [1]u8{@truncate(@as(u32, @bitCast(b)))};
    return @bitCast(@as(u32, @truncate(z.adler32(@as(c_ulong, @as(u32, @bitCast(adler))), &one, 1))));
}
pub fn J_java_util_zip_Adler32__updateBytes(adler: i32, arr: Ref, off: i32, len: i32) E!i32 {
    const src = try range(arr, off, len);
    return @bitCast(@as(u32, @truncate(z.adler32(@as(c_ulong, @as(u32, @bitCast(adler))), src.ptr, @intCast(src.len)))));
}

// ---------------------------------------------------------------- Inflater / Deflater (zlib)
fn zs(addr: i64) *z.z_stream {
    return @ptrFromInt(@as(usize, @bitCast(addr)));
}
fn zerr(strm: *z.z_stream, code: c_int) E {
    const msg: []const u8 = if (strm.msg != null) std.mem.span(strm.msg) else switch (code) {
        z.Z_DATA_ERROR => "invalid data",
        z.Z_MEM_ERROR => "out of memory",
        else => "zlib error",
    };
    if (code == z.Z_DATA_ERROR) return rt.throwKind(.InternalError, msg);
    return rt.throwKind(.InternalError, msg);
}

pub fn J_java_util_zip_Inflater__initIDs() E!void {}
pub fn J_java_util_zip_Inflater__init(nowrap: i32) E!i64 {
    const strm = std.c.malloc(@sizeOf(z.z_stream)) orelse return rt.throwKind(.OutOfMemory, null);
    const s: *z.z_stream = @ptrCast(@alignCast(strm));
    s.* = std.mem.zeroes(z.z_stream);
    const r = z.inflateInit2_(s, if (nowrap != 0) -15 else 15, z.zlibVersion(), @sizeOf(z.z_stream));
    if (r != z.Z_OK) {
        std.c.free(strm);
        return rt.throwKind(.OutOfMemory, "zlib inflateInit failed");
    }
    return @bitCast(@as(u64, @intFromPtr(strm)));
}
pub fn J_java_util_zip_Inflater__end(addr: i64) E!void {
    _ = z.inflateEnd(zs(addr));
    std.c.free(@ptrFromInt(@as(usize, @bitCast(addr))));
}
pub fn J_java_util_zip_Inflater__reset(addr: i64) E!void {
    _ = z.inflateReset(zs(addr));
}

/// Shared inflate step; returns the JDK's packed result:
/// (inputConsumed & 0x7fff_ffff) | (outputProduced & 0x7fff_ffff) << 31 | finished << 62 | needDict << 63
fn inflateStep(this: Ref, addr: i64, in: [*]u8, in_len: usize, out: [*]u8, out_len: usize) E!i64 {
    _ = this;
    const s = zs(addr);
    s.next_in = in;
    s.avail_in = @intCast(in_len);
    s.next_out = out;
    s.avail_out = @intCast(out_len);
    const ret = z.inflate(s, z.Z_PARTIAL_FLUSH);
    const consumed: i64 = @intCast(in_len - s.avail_in);
    const produced: i64 = @intCast(out_len - s.avail_out);
    var finished: i64 = 0;
    var need_dict: i64 = 0;
    switch (ret) {
        z.Z_STREAM_END => finished = 1,
        z.Z_OK => {},
        z.Z_NEED_DICT => need_dict = 1,
        z.Z_BUF_ERROR => {},
        z.Z_DATA_ERROR => {
            const msg: []const u8 = if (s.msg != null) std.mem.span(s.msg) else "invalid data";
            return rt.throwKind(.InternalError, msg); // the JDK throws DataFormatException (via Java)
        },
        else => return zerr(s, ret),
    }
    return consumed | (produced << 31) | (finished << 62) | (need_dict << 63);
}

pub fn J_java_util_zip_Inflater__inflateBytesBytes(this: Ref, addr: i64, in: Ref, ioff: i32, ilen: i32, out: Ref, ooff: i32, olen: i32) E!i64 {
    const i = try range(in, ioff, ilen);
    const o = try range(out, ooff, olen);
    return inflateStep(this, addr, i.ptr, i.len, o.ptr, o.len);
}
pub fn J_java_util_zip_Inflater__inflateBytesBuffer(this: Ref, addr: i64, in: Ref, ioff: i32, ilen: i32, oaddr: i64, olen: i32) E!i64 {
    const i = try range(in, ioff, ilen);
    return inflateStep(this, addr, i.ptr, i.len, @ptrFromInt(@as(usize, @bitCast(oaddr))), @intCast(olen));
}
pub fn J_java_util_zip_Inflater__inflateBufferBytes(this: Ref, addr: i64, iaddr: i64, ilen: i32, out: Ref, ooff: i32, olen: i32) E!i64 {
    const o = try range(out, ooff, olen);
    return inflateStep(this, addr, @ptrFromInt(@as(usize, @bitCast(iaddr))), @intCast(ilen), o.ptr, o.len);
}
pub fn J_java_util_zip_Inflater__inflateBufferBuffer(this: Ref, addr: i64, iaddr: i64, ilen: i32, oaddr: i64, olen: i32) E!i64 {
    return inflateStep(this, addr, @ptrFromInt(@as(usize, @bitCast(iaddr))), @intCast(ilen), @ptrFromInt(@as(usize, @bitCast(oaddr))), @intCast(olen));
}
pub fn J_java_util_zip_Inflater__getAdler(addr: i64) E!i32 {
    return @bitCast(@as(u32, @truncate(zs(addr).adler)));
}

pub fn J_java_util_zip_Deflater__init(level: i32, strategy: i32, nowrap: i32) E!i64 {
    const strm = std.c.malloc(@sizeOf(z.z_stream)) orelse return rt.throwKind(.OutOfMemory, null);
    const s: *z.z_stream = @ptrCast(@alignCast(strm));
    s.* = std.mem.zeroes(z.z_stream);
    const r = z.deflateInit2_(s, level, z.Z_DEFLATED, if (nowrap != 0) -15 else 15, 8, strategy, z.zlibVersion(), @sizeOf(z.z_stream));
    if (r != z.Z_OK) {
        std.c.free(strm);
        return rt.throwKind(.OutOfMemory, "zlib deflateInit failed");
    }
    return @bitCast(@as(u64, @intFromPtr(strm)));
}
pub fn J_java_util_zip_Deflater__end(addr: i64) E!void {
    _ = z.deflateEnd(zs(addr));
    std.c.free(@ptrFromInt(@as(usize, @bitCast(addr))));
}
pub fn J_java_util_zip_Deflater__reset(addr: i64) E!void {
    _ = z.deflateReset(zs(addr));
}
pub fn J_java_util_zip_Deflater__getAdler(addr: i64) E!i32 {
    return @bitCast(@as(u32, @truncate(zs(addr).adler)));
}

/// Mirrors the JDK's Deflater.c doDeflate: params change, then deflate with the given flush.
/// Result: consumed | produced << 31 | finished << 62 | paramsChanged << 63 (Deflater.java).
fn deflateStep(addr: i64, in: [*]u8, in_len: usize, out: [*]u8, out_len: usize, flush: i32, params: i32) E!i64 {
    const s = zs(addr);
    s.next_in = in;
    s.avail_in = @intCast(in_len);
    s.next_out = out;
    s.avail_out = @intCast(out_len);
    var finished: i64 = 0;
    var set_params: i64 = 0;
    if (params & 1 != 0) {
        const level = params >> 1 & 0x1f;
        const strategy = params >> 6 & 0x3;
        const lv: c_int = if (level == 0xf) -1 else level;
        const r = z.deflateParams(s, lv, strategy);
        switch (r) {
            z.Z_OK => set_params = 1,
            z.Z_BUF_ERROR => {},
            else => return zerr(s, r),
        }
    } else {
        const r = z.deflate(s, flush);
        switch (r) {
            z.Z_STREAM_END => finished = 1,
            z.Z_OK, z.Z_BUF_ERROR => {},
            else => return zerr(s, r),
        }
    }
    const consumed: i64 = @intCast(in_len - s.avail_in);
    const produced: i64 = @intCast(out_len - s.avail_out);
    return consumed | (produced << 31) | (finished << 62) | (set_params << 63);
}
pub fn J_java_util_zip_Deflater__deflateBytesBytes(this: Ref, addr: i64, in: Ref, ioff: i32, ilen: i32, out: Ref, ooff: i32, olen: i32, flush: i32, params: i32) E!i64 {
    _ = this;
    const i = try range(in, ioff, ilen);
    const o = try range(out, ooff, olen);
    return deflateStep(addr, i.ptr, i.len, o.ptr, o.len, flush, params);
}
pub fn J_java_util_zip_Deflater__deflateBytesBuffer(this: Ref, addr: i64, in: Ref, ioff: i32, ilen: i32, oaddr: i64, olen: i32, flush: i32, params: i32) E!i64 {
    _ = this;
    const i = try range(in, ioff, ilen);
    return deflateStep(addr, i.ptr, i.len, @ptrFromInt(@as(usize, @bitCast(oaddr))), @intCast(olen), flush, params);
}
pub fn J_java_util_zip_Deflater__deflateBufferBytes(this: Ref, addr: i64, iaddr: i64, ilen: i32, out: Ref, ooff: i32, olen: i32, flush: i32, params: i32) E!i64 {
    _ = this;
    const o = try range(out, ooff, olen);
    return deflateStep(addr, @ptrFromInt(@as(usize, @bitCast(iaddr))), @intCast(ilen), o.ptr, o.len, flush, params);
}
pub fn J_java_util_zip_Deflater__deflateBufferBuffer(this: Ref, addr: i64, iaddr: i64, ilen: i32, oaddr: i64, olen: i32, flush: i32, params: i32) E!i64 {
    _ = this;
    return deflateStep(addr, @ptrFromInt(@as(usize, @bitCast(iaddr))), @intCast(ilen), @ptrFromInt(@as(usize, @bitCast(oaddr))), @intCast(olen), flush, params);
}
