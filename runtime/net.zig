//! Networking natives on Linux, directly on libc: sun.nio.ch.Net (sockets), socket / datagram /
//! unix-domain dispatchers, epoll + eventfd, inotify (WatchService), java.net.InetAddress
//! resolution (getaddrinfo) and NetworkInterface (getifaddrs).
//!
//! Return conventions follow the JDK's IOStatus: >0 count, 0, EOF -1, UNAVAILABLE -2,
//! INTERRUPTED -3, UNSUPPORTED_CASE -6. Failures throw the same exception classes the JDK's
//! native code throws (ConnectException, BindException, NoRouteToHostException, SocketException,
//! PortUnreachableException, UnknownHostException, IOException).
const std = @import("std");
const builtin = @import("builtin");
const rt = @import("rt.zig");
const P = @import("classes.zig");

const Ref = rt.Ref;
const E = rt.E;

const IOS_EOF: i32 = -1;
const IOS_UNAVAILABLE: i32 = -2;
const IOS_INTERRUPTED: i32 = -3;

const EINTR = 4;
const EAGAIN = 11;
const EINPROGRESS = 115;
const ECONNREFUSED = 111;
const ECONNRESET = 104;
const ETIMEDOUT = 110;
const EHOSTUNREACH = 113;
const ENETUNREACH = 101;
const EADDRINUSE = 98;
const EADDRNOTAVAIL = 99;
const EPIPE = 32;
const ENOTCONN = 107;
const EPROTO = 71;
const ECONNABORTED = 103;
const EOPNOTSUPP = 95;
const EACCES = 13;

const AF_UNIX: c_int = 1;
const AF_INET: c_int = 2;
const AF_INET6: c_int = 10;
const AF_UNSPEC: c_int = 0;
const SOCK_STREAM: c_int = 1;
const SOCK_DGRAM: c_int = 2;
const SOCK_NONBLOCK: c_int = 0o4000;
const SOCK_CLOEXEC: c_int = 0o2000000;
const SOL_SOCKET: c_int = 1;
const SO_REUSEADDR: c_int = 2;
const SO_ERROR: c_int = 4;
const SO_LINGER: c_int = 13;
const IPPROTO_IP: c_int = 0;
const IPPROTO_IPV6: c_int = 41;
const IPV6_V6ONLY: c_int = 26;
const IP_MULTICAST_TTL: c_int = 33;
const IP_MULTICAST_LOOP: c_int = 34;
const IP_MULTICAST_IF: c_int = 32;
const IP_ADD_MEMBERSHIP: c_int = 35;
const IP_DROP_MEMBERSHIP: c_int = 36;
const IP_ADD_SOURCE_MEMBERSHIP: c_int = 39;
const IP_DROP_SOURCE_MEMBERSHIP: c_int = 40;
const IP_BLOCK_SOURCE: c_int = 38;
const IP_UNBLOCK_SOURCE: c_int = 37;
const IPV6_ADD_MEMBERSHIP: c_int = 20;
const IPV6_DROP_MEMBERSHIP: c_int = 21;
const IPV6_MULTICAST_IF: c_int = 17;
const MCAST_JOIN_SOURCE_GROUP: c_int = 46;
const MCAST_LEAVE_SOURCE_GROUP: c_int = 47;
const MCAST_BLOCK_SOURCE: c_int = 43;
const MCAST_UNBLOCK_SOURCE: c_int = 44;
const MSG_OOB: c_int = 1;
const MSG_NOSIGNAL: c_int = 0x4000;
const FIONREAD = 0x541B;
const POLLIN = 1;
const POLLOUT = 4;
const POLLERR = 8;
const POLLHUP = 0x10;
const POLLNVAL = 0x20;

/// errno location through std.c: `__errno` on Android (bionic has no `__errno_location`), the glibc name elsewhere.
inline fn __errno_location() *c_int {
    return std.c._errno();
}
extern "c" fn socket(d: c_int, t: c_int, p: c_int) c_int;
extern "c" fn bind(fd: c_int, a: *const anyopaque, len: u32) c_int;
extern "c" fn listen(fd: c_int, backlog: c_int) c_int;
extern "c" fn connect(fd: c_int, a: *const anyopaque, len: u32) c_int;
extern "c" fn accept4(fd: c_int, a: ?*anyopaque, len: ?*u32, flags: c_int) c_int;
extern "c" fn shutdown(fd: c_int, how: c_int) c_int;
extern "c" fn getsockname(fd: c_int, a: *anyopaque, len: *u32) c_int;
extern "c" fn getpeername(fd: c_int, a: *anyopaque, len: *u32) c_int;
extern "c" fn getsockopt(fd: c_int, level: c_int, opt: c_int, v: *anyopaque, len: *u32) c_int;
extern "c" fn setsockopt(fd: c_int, level: c_int, opt: c_int, v: *const anyopaque, len: u32) c_int;
extern "c" fn poll(fds: *anyopaque, n: c_ulong, timeout: c_int) c_int;
extern "c" fn ioctl(fd: c_int, req: c_ulong, ...) c_int;
extern "c" fn send(fd: c_int, buf: *const anyopaque, n: usize, flags: c_int) isize;
extern "c" fn sendto(fd: c_int, buf: *const anyopaque, n: usize, flags: c_int, a: ?*const anyopaque, len: u32) isize;
extern "c" fn recvfrom(fd: c_int, buf: *anyopaque, n: usize, flags: c_int, a: ?*anyopaque, len: ?*u32) isize;
extern "c" fn read(fd: c_int, buf: [*]u8, n: usize) isize;
extern "c" fn write(fd: c_int, buf: [*]const u8, n: usize) isize;
extern "c" fn readv(fd: c_int, iov: *const anyopaque, n: c_int) isize;
extern "c" fn writev(fd: c_int, iov: *const anyopaque, n: c_int) isize;
extern "c" fn dup2(a: c_int, b: c_int) c_int;
extern "c" fn close(fd: c_int) c_int;
extern "c" fn fcntl(fd: c_int, cmd: c_int, ...) c_int;
extern "c" fn socketpair(d: c_int, t: c_int, p: c_int, sv: *[2]c_int) c_int;
extern "c" fn epoll_create(size: c_int) c_int;
extern "c" fn epoll_ctl(epfd: c_int, op: c_int, fd: c_int, ev: *const anyopaque) c_int;
extern "c" fn epoll_wait(epfd: c_int, evs: *anyopaque, max: c_int, timeout: c_int) c_int;
extern "c" fn eventfd(init: c_uint, flags: c_int) c_int;
extern "c" fn inotify_init() c_int;
extern "c" fn inotify_add_watch(fd: c_int, path: [*:0]const u8, mask: u32) c_int;
extern "c" fn inotify_rm_watch(fd: c_int, wd: c_int) c_int;
extern "c" fn gethostname(buf: [*]u8, n: usize) c_int;
extern "c" fn getaddrinfo(node: ?[*:0]const u8, service: ?[*:0]const u8, hints: ?*const AddrInfo, res: *?*AddrInfo) c_int;
extern "c" fn freeaddrinfo(res: *AddrInfo) void;
extern "c" fn gai_strerror(e: c_int) [*:0]const u8;
extern "c" fn getnameinfo(sa: *const anyopaque, salen: u32, host: [*]u8, hostlen: u32, serv: ?[*]u8, servlen: u32, flags: c_int) c_int;
extern "c" fn getifaddrs(out: *?*IfAddrs) c_int;
extern "c" fn freeifaddrs(p: *IfAddrs) void;
extern "c" fn if_nametoindex(name: [*:0]const u8) c_uint;
extern "c" fn if_indextoname(idx: c_uint, buf: [*]u8) ?[*:0]u8;
extern "c" fn strerror(e: c_int) [*:0]const u8;

const AddrInfo = extern struct {
    flags: c_int,
    family: c_int,
    socktype: c_int,
    protocol: c_int,
    addrlen: u32,
    addr: ?*anyopaque,
    canonname: ?[*:0]u8,
    next: ?*AddrInfo,
};
const IfAddrs = extern struct {
    next: ?*IfAddrs,
    name: [*:0]u8,
    flags: c_uint,
    addr: ?*anyopaque,
    netmask: ?*anyopaque,
    ifu: ?*anyopaque, // broadaddr / dstaddr
    data: ?*anyopaque,
};

const meta = rt.MethodMeta{ .class = "<runtime>", .name = "<net>", .desc = "" };

/// Copy a sockaddr of unknown alignment (getifaddrs/getaddrinfo results) into aligned storage.
fn loadSa(p: ?*anyopaque) ?Storage {
    const src: [*]const u8 = @ptrCast(p orelse return null);
    var st: Storage = .{};
    @memcpy(st.data[0..28], src[0..28]);
    return st;
}

inline fn errno() c_int {
    return __errno_location().*;
}
inline fn fdOf(o: Ref) E!c_int {
    return P.C_java_io_FileDescriptor.cast(try rt.nn(o)).f_fd;
}
inline fn ptr(a: i64) usize {
    return @bitCast(a);
}

// ------------------------------------------------------------------ exceptions
fn classByName(n: []const u8) ?*rt.Class {
    return rt.findClass(n);
}

fn throwNamed(name: []const u8, msg: []const u8) E {
    const k = classByName(name) orelse return rt.throwKind(.IO, msg);
    const ex = rt.hooks.new_throwable(k, msg) catch |e| return e;
    rt.cur.exc = ex;
    return error.Throw;
}

fn errMsg(e: c_int) []const u8 {
    return std.mem.span(strerror(e));
}

/// JDK handleSocketError: map errno to the exception class the JDK would throw.
fn socketError(e: c_int) E {
    const cls: []const u8 = switch (e) {
        EINPROGRESS => return throwNamed("java/net/ConnectException", "Connection refused"),
        EPROTO => "java/net/ProtocolException",
        ECONNREFUSED, ETIMEDOUT, ENOTCONN => "java/net/ConnectException",
        EHOSTUNREACH => "java/net/NoRouteToHostException",
        EADDRINUSE, EADDRNOTAVAIL, EACCES => "java/net/BindException",
        else => "java/net/SocketException",
    };
    return throwNamed(cls, errMsg(e));
}

fn ioFail() E {
    return rt.throwKind(.IO, errMsg(errno()));
}

fn convert(n: isize, reading: bool) E!i32 {
    if (n > 0) return @intCast(n);
    if (n == 0) return if (reading) IOS_EOF else 0;
    const e = errno();
    if (e == EAGAIN) return IOS_UNAVAILABLE;
    if (e == EINTR) return IOS_INTERRUPTED;
    if (e == ECONNRESET) return throwNamed("java/net/SocketException", "Connection reset");
    return ioFail();
}
fn convertLong(n: isize, reading: bool) E!i64 {
    if (n > 0) return @intCast(n);
    if (n == 0) return if (reading) IOS_EOF else 0;
    const e = errno();
    if (e == EAGAIN) return IOS_UNAVAILABLE;
    if (e == EINTR) return IOS_INTERRUPTED;
    if (e == ECONNRESET) return throwNamed("java/net/SocketException", "Connection reset");
    return ioFail();
}

// ------------------------------------------------------------------ calling Java helpers
fn findMethod(cname: []const u8, mname: []const u8, desc: []const u8) E!struct { k: *rt.Class, m: *const rt.MethodInfoMeta, slot: usize } {
    const k = classByName(cname) orelse return rt.throwKind(.InternalError, "net: class not in image");
    for (k.methods, 0..) |*m, i| {
        if (std.mem.eql(u8, m.name, mname) and std.mem.eql(u8, m.desc, desc)) {
            if (m.fp == null) return rt.throwKind(.InternalError, "net: Java helper was not compiled");
            return .{ .k = k, .m = m, .slot = i };
        }
    }
    return rt.throwKind(.InternalError, "net: Java helper not found");
}

/// args: pointer to an Object[] of boxed arguments (kept rooted by the caller)
fn callStatic(cname: []const u8, mname: []const u8, desc: []const u8, args: Ref) E!Ref {
    const f = try findMethod(cname, mname, desc);
    try rt.ensureInit(f.k);
    return P.reflectInvoke(f.m.shape, f.m.fp.?, null, args);
}

fn newBytes(b: []const u8) E!Ref {
    const a = try rt.newArray(rt.arrayOf(rt.prim_B), @intCast(b.len));
    @memcpy(rt.arrayData(u8, rt.asArray(a.?))[0..b.len], b);
    return a;
}

/// InetAddress for raw address bytes (4 or 16), with optional host name and IPv6 scope.
fn inetFromBytes(bytes: []const u8, host: ?[]const u8, scope: u32) E!Ref {
    var R: [5]Ref = @splat(null);
    var F = rt.Frame{ .refs = &R, .n = 5, .meth = &meta };
    try rt.pushFrame(&F);
    defer rt.popFrame(&F);
    R[0] = if (host) |h| try rt.newStringUtf8(h) else null;
    R[1] = try newBytes(bytes);
    if (bytes.len == 16 and scope != 0) {
        R[2] = try rt.newArray(rt.arrayOf(rt.hooks.object_class), 3);
        try rt.aastore(R[2], 0, R[0]);
        try rt.aastore(R[2], 1, R[1]);
        R[3] = try P.boxI(@bitCast(scope));
        try rt.aastore(R[2], 2, R[3]);
        return callStatic("java/net/Inet6Address", "getByAddress", "(Ljava/lang/String;[BI)Ljava/net/Inet6Address;", R[2]);
    }
    R[2] = try rt.newArray(rt.arrayOf(rt.hooks.object_class), 2);
    try rt.aastore(R[2], 0, R[0]);
    try rt.aastore(R[2], 1, R[1]);
    return callStatic("java/net/InetAddress", "getByAddress", "(Ljava/lang/String;[B)Ljava/net/InetAddress;", R[2]);
}

fn newInetSocketAddress(addr: Ref, port: i32) E!Ref {
    var R: [4]Ref = @splat(null);
    R[0] = addr;
    var F = rt.Frame{ .refs = &R, .n = 4, .meth = &meta };
    try rt.pushFrame(&F);
    defer rt.popFrame(&F);
    const k = classByName("java/net/InetSocketAddress") orelse return rt.throwKind(.InternalError, "net: InetSocketAddress missing");
    const f = try findMethod("java/net/InetSocketAddress", "<init>", "(Ljava/net/InetAddress;I)V");
    R[1] = try rt.newObject(k);
    R[2] = try rt.newArray(rt.arrayOf(rt.hooks.object_class), 2);
    try rt.aastore(R[2], 0, R[0]);
    R[3] = try P.boxI(port);
    try rt.aastore(R[2], 1, R[3]);
    _ = try P.reflectInvoke(f.m.shape, f.m.fp.?, R[1], R[2]);
    return R[1];
}

// ------------------------------------------------------------------ socket addresses
const Storage = extern struct { data: [128]u8 align(8) = @splat(0) };

fn isInet6(o: *rt.Object) bool {
    return o.class == P.C_java_net_Inet6Address.cls();
}

/// Fill `st` from a java.net.InetAddress + port. ipv6_socket: IPv4 addresses become IPv4-mapped.
fn toSockaddr(addr: Ref, port: i32, ipv6_socket: bool, st: *Storage) E!u32 {
    const o = try rt.nn(addr);
    const p: u16 = @intCast(@as(u32, @bitCast(port)) & 0xffff);
    const be = std.mem.nativeToBig(u16, p);
    if (isInet6(o)) {
        const h6 = P.C_java_net_Inet6Address.cast(o).f_holder6 orelse return rt.throwKind(.NPE, null);
        const hh = P.C_java_net_Inet6Address_2Inet6AddressHolder.cast(h6);
        const ip = rt.arrayData(u8, rt.asArray((hh.f_ipaddress orelse return rt.throwKind(.NPE, null))));
        std.mem.writeInt(u16, st.data[0..2], AF_INET6, builtin.cpu.arch.endian());
        std.mem.writeInt(u16, st.data[2..4], be, builtin.cpu.arch.endian());
        @memcpy(st.data[8..24], ip[0..16]);
        std.mem.writeInt(u32, st.data[24..28], @bitCast(hh.f_scope_1id), builtin.cpu.arch.endian());
        return 28;
    }
    const holder = P.C_java_net_InetAddress.cast(o).f_holder orelse return rt.throwKind(.NPE, null);
    const a: u32 = @bitCast(P.C_java_net_InetAddress_2InetAddressHolder.cast(holder).f_address);
    if (ipv6_socket) {
        std.mem.writeInt(u16, st.data[0..2], AF_INET6, builtin.cpu.arch.endian());
        std.mem.writeInt(u16, st.data[2..4], be, builtin.cpu.arch.endian());
        st.data[18] = 0xff;
        st.data[19] = 0xff;
        std.mem.writeInt(u32, st.data[20..24], a, .big);
        return 28;
    }
    std.mem.writeInt(u16, st.data[0..2], AF_INET, builtin.cpu.arch.endian());
    std.mem.writeInt(u16, st.data[2..4], be, builtin.cpu.arch.endian());
    std.mem.writeInt(u32, st.data[4..8], a, .big);
    return 16;
}

/// java.net.InetAddress for a sockaddr; IPv4-mapped IPv6 addresses become Inet4Address.
fn fromSockaddr(st: *const Storage, port: *i32) E!Ref {
    const fam = std.mem.readInt(u16, st.data[0..2], builtin.cpu.arch.endian());
    const pbe = std.mem.readInt(u16, st.data[2..4], builtin.cpu.arch.endian());
    port.* = std.mem.bigToNative(u16, pbe);
    if (fam == AF_INET) return inetFromBytes(st.data[4..8], null, 0);
    if (fam == AF_INET6) {
        const ip = st.data[8..24];
        const mapped = std.mem.allEqual(u8, ip[0..10], 0) and ip[10] == 0xff and ip[11] == 0xff;
        if (mapped) return inetFromBytes(ip[12..16], null, 0);
        const scope = std.mem.readInt(u32, st.data[24..28], builtin.cpu.arch.endian());
        return inetFromBytes(ip, null, scope);
    }
    return rt.throwKind(.IO, "unsupported address family");
}

// ------------------------------------------------------------------ sun.nio.ch.Net
pub fn J_sun_nio_ch_Net__initIDs() E!void {}

fn ipv6Available() bool {
    if (rt.st.ipv6_cached < 0) {
        const fd = socket(AF_INET6, SOCK_STREAM, 0);
        if (fd >= 0) {
            _ = close(fd);
            rt.st.ipv6_cached = 1;
        } else rt.st.ipv6_cached = 0;
    }
    return rt.st.ipv6_cached == 1;
}

pub fn J_sun_nio_ch_Net__isIPv6Available0() E!i32 {
    return @intFromBool(ipv6Available());
}
pub fn J_sun_nio_ch_Net__isReusePortAvailable0() E!i32 {
    return 1;
}
pub fn J_sun_nio_ch_Net__isExclusiveBindAvailable() E!i32 {
    return -1;
}
pub fn J_sun_nio_ch_Net__shouldShutdownWriteBeforeClose0() E!i32 {
    return 0;
}
pub fn J_sun_nio_ch_Net__shouldSetBothIPv4AndIPv6Options0() E!i32 {
    return 1;
}
pub fn J_sun_nio_ch_Net__canIPv6SocketJoinIPv4Group0() E!i32 {
    return 1;
}
pub fn J_sun_nio_ch_Net__canJoin6WithIPv4Group0() E!i32 {
    return 0;
}
pub fn J_sun_nio_ch_Net__canUseIPv6OptionsWithIPv4LocalAddress0() E!i32 {
    return 1;
}
pub fn J_sun_nio_ch_Net__pollinValue() E!i32 {
    return POLLIN;
}
pub fn J_sun_nio_ch_Net__polloutValue() E!i32 {
    return POLLOUT;
}
pub fn J_sun_nio_ch_Net__pollerrValue() E!i32 {
    return POLLERR;
}
pub fn J_sun_nio_ch_Net__pollhupValue() E!i32 {
    return POLLHUP;
}
pub fn J_sun_nio_ch_Net__pollnvalValue() E!i32 {
    return POLLNVAL;
}
pub fn J_sun_nio_ch_Net__pollconnValue() E!i32 {
    return POLLOUT;
}

pub fn J_sun_nio_ch_Net__socket0(prefer_ipv6: i32, stream: i32, reuse: i32, fast_loopback: i32) E!i32 {
    _ = fast_loopback;
    const v6 = prefer_ipv6 != 0 and ipv6Available();
    const fd = socket(if (v6) AF_INET6 else AF_INET, (if (stream != 0) SOCK_STREAM else SOCK_DGRAM) | SOCK_CLOEXEC, 0);
    if (fd < 0) return socketError(errno());
    if (v6) {
        var off: c_int = 0;
        _ = setsockopt(fd, IPPROTO_IPV6, IPV6_V6ONLY, &off, 4);
    }
    if (reuse != 0) {
        var one: c_int = 1;
        _ = setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &one, 4);
    }
    return fd;
}

fn isV6Socket(fd: c_int) bool {
    var st: Storage = .{};
    var len: u32 = @sizeOf(Storage);
    if (getsockname(fd, &st, &len) != 0) return false;
    return std.mem.readInt(u16, st.data[0..2], builtin.cpu.arch.endian()) == AF_INET6;
}

pub fn J_sun_nio_ch_Net__bind0(fdo: Ref, prefer_ipv6: i32, excl: i32, addr: Ref, port: i32) E!void {
    _ = excl;
    const fd = try fdOf(fdo);
    var st: Storage = .{};
    const len = try toSockaddr(addr, port, prefer_ipv6 != 0 and ipv6Available(), &st);
    if (bind(fd, &st, len) < 0) return socketError(errno());
}
pub fn J_sun_nio_ch_Net__listen(fdo: Ref, backlog: i32) E!void {
    if (listen(try fdOf(fdo), backlog) < 0) return socketError(errno());
}
pub fn J_sun_nio_ch_Net__connect0(prefer_ipv6: i32, fdo: Ref, addr: Ref, port: i32) E!i32 {
    const fd = try fdOf(fdo);
    var st: Storage = .{};
    const len = try toSockaddr(addr, port, prefer_ipv6 != 0 and ipv6Available(), &st);
    rt.enterSafe();
    const r = connect(fd, &st, len);
    const e = errno();
    rt.leaveSafe();
    if (r < 0) {
        if (e == EINPROGRESS) return IOS_UNAVAILABLE;
        if (e == EINTR) return IOS_INTERRUPTED;
        return socketError(e);
    }
    return 1;
}
pub fn J_sun_nio_ch_Net__accept(fdo: Ref, newfdo: Ref, isaa: Ref) E!i32 {
    const fd = try fdOf(fdo);
    var st: Storage = .{};
    var len: u32 = @sizeOf(Storage);
    rt.enterSafe();
    const nfd = accept4(fd, &st, &len, SOCK_CLOEXEC);
    const e = errno();
    rt.leaveSafe();
    if (nfd < 0) {
        if (e == EAGAIN) return IOS_UNAVAILABLE;
        if (e == EINTR) return IOS_INTERRUPTED;
        if (e == ECONNABORTED) return IOS_UNAVAILABLE;
        return socketError(e);
    }
    P.C_java_io_FileDescriptor.cast(try rt.nn(newfdo)).f_fd = nfd;
    var port: i32 = 0;
    var R: [2]Ref = @splat(null);
    var F = rt.Frame{ .refs = &R, .n = 2, .meth = &meta };
    try rt.pushFrame(&F);
    defer rt.popFrame(&F);
    R[0] = try fromSockaddr(&st, &port);
    R[1] = try newInetSocketAddress(R[0], port);
    try rt.aastore(isaa, 0, R[1]);
    return 1;
}
pub fn J_sun_nio_ch_Net__shutdown(fdo: Ref, how: i32) E!void {
    if (shutdown(try fdOf(fdo), how) < 0 and errno() != ENOTCONN) return socketError(errno());
}
pub fn J_sun_nio_ch_Net__localPort(fdo: Ref) E!i32 {
    var st: Storage = .{};
    var len: u32 = @sizeOf(Storage);
    if (getsockname(try fdOf(fdo), &st, &len) < 0) return socketError(errno());
    var port: i32 = 0;
    _ = std.mem.readInt(u16, st.data[0..2], builtin.cpu.arch.endian());
    port = std.mem.bigToNative(u16, std.mem.readInt(u16, st.data[2..4], builtin.cpu.arch.endian()));
    return port;
}
pub fn J_sun_nio_ch_Net__localInetAddress(fdo: Ref) E!Ref {
    var st: Storage = .{};
    var len: u32 = @sizeOf(Storage);
    if (getsockname(try fdOf(fdo), &st, &len) < 0) return socketError(errno());
    var port: i32 = 0;
    return fromSockaddr(&st, &port);
}
pub fn J_sun_nio_ch_Net__remotePort(fdo: Ref) E!i32 {
    var st: Storage = .{};
    var len: u32 = @sizeOf(Storage);
    if (getpeername(try fdOf(fdo), &st, &len) < 0) return socketError(errno());
    return std.mem.bigToNative(u16, std.mem.readInt(u16, st.data[2..4], builtin.cpu.arch.endian()));
}
pub fn J_sun_nio_ch_Net__remoteInetAddress(fdo: Ref) E!Ref {
    var st: Storage = .{};
    var len: u32 = @sizeOf(Storage);
    if (getpeername(try fdOf(fdo), &st, &len) < 0) return socketError(errno());
    var port: i32 = 0;
    return fromSockaddr(&st, &port);
}

const Linger = extern struct { onoff: c_int, linger: c_int };

pub fn J_sun_nio_ch_Net__getIntOption0(fdo: Ref, may_need_conversion: i32, level: i32, opt: i32) E!i32 {
    const fd = try fdOf(fdo);
    _ = may_need_conversion;
    if (level == SOL_SOCKET and opt == SO_LINGER) {
        var l: Linger = undefined;
        var len: u32 = @sizeOf(Linger);
        if (getsockopt(fd, level, opt, &l, &len) < 0) return socketError(errno());
        return if (l.onoff != 0) l.linger else -1;
    }
    if (level == IPPROTO_IP and (opt == IP_MULTICAST_TTL or opt == IP_MULTICAST_LOOP)) {
        var b: u8 = 0;
        var len: u32 = 1;
        if (getsockopt(fd, level, opt, &b, &len) < 0) return socketError(errno());
        return b;
    }
    var v: c_int = 0;
    var len: u32 = 4;
    if (getsockopt(fd, level, opt, &v, &len) < 0) return socketError(errno());
    return v;
}
pub fn J_sun_nio_ch_Net__setIntOption0(fdo: Ref, may_need_conversion: i32, level: i32, opt: i32, arg: i32, ipv6: i32) E!void {
    _ = may_need_conversion;
    _ = ipv6;
    const fd = try fdOf(fdo);
    var r: c_int = undefined;
    if (level == SOL_SOCKET and opt == SO_LINGER) {
        const l: Linger = .{ .onoff = @intFromBool(arg >= 0), .linger = if (arg >= 0) arg else 0 };
        r = setsockopt(fd, level, opt, &l, @sizeOf(Linger));
    } else if (level == IPPROTO_IP and (opt == IP_MULTICAST_TTL or opt == IP_MULTICAST_LOOP)) {
        const b: u8 = @truncate(@as(u32, @bitCast(arg)));
        r = setsockopt(fd, level, opt, &b, 1);
    } else {
        const v: c_int = arg;
        r = setsockopt(fd, level, opt, &v, 4);
    }
    if (r < 0) return socketError(errno());
}

const PollFd = extern struct { fd: c_int, events: i16, revents: i16 };

pub fn J_sun_nio_ch_Net__poll(fdo: Ref, events: i32, timeout: i64) E!i32 {
    var p = PollFd{ .fd = try fdOf(fdo), .events = @intCast(events), .revents = 0 };
    rt.enterSafe();
    const r = poll(&p, 1, @intCast(timeout));
    const e = errno();
    rt.leaveSafe();
    if (r < 0) {
        if (e == EINTR) return IOS_INTERRUPTED;
        return ioFail();
    }
    return if (r > 0) p.revents else 0;
}
pub fn J_sun_nio_ch_Net__pollConnect(fdo: Ref, timeout: i64) E!i32 {
    const fd = try fdOf(fdo);
    var p = PollFd{ .fd = fd, .events = POLLOUT, .revents = 0 };
    rt.enterSafe();
    const r = poll(&p, 1, @intCast(timeout));
    const e = errno();
    rt.leaveSafe();
    if (r < 0) {
        if (e == EINTR) return 0;
        return ioFail();
    }
    if (r == 0) return 0;
    var err: c_int = 0;
    var len: u32 = 4;
    if (getsockopt(fd, SOL_SOCKET, SO_ERROR, &err, &len) < 0) return socketError(errno());
    if (err != 0) return socketError(err);
    return 1;
}
pub fn J_sun_nio_ch_Net__available(fdo: Ref) E!i32 {
    var n: c_int = 0;
    if (ioctl(try fdOf(fdo), FIONREAD, &n) < 0) return ioFail();
    return n;
}
pub fn J_sun_nio_ch_Net__sendOOB(fdo: Ref, b: i32) E!i32 {
    const c: [1]u8 = .{@truncate(@as(u32, @bitCast(b)))};
    const n = send(try fdOf(fdo), &c, 1, MSG_OOB | MSG_NOSIGNAL);
    return convert(n, false);
}
pub fn J_sun_nio_ch_Net__discardOOB(fdo: Ref) E!i32 {
    _ = fdo;
    return 0;
}

// multicast
const IpMreq = extern struct { multiaddr: u32, interface: u32 };
const IpMreqSource = extern struct { multiaddr: u32, interface: u32, sourceaddr: u32 };
const Ipv6Mreq = extern struct { multiaddr: [16]u8, ifindex: c_uint };

fn be32(x: i32) u32 {
    return std.mem.nativeToBig(u32, @bitCast(x));
}
pub fn J_sun_nio_ch_Net__joinOrDrop4(join: i32, fdo: Ref, group: i32, interface: i32, source: i32) E!i32 {
    const fd = try fdOf(fdo);
    var r: c_int = undefined;
    if (source == 0) {
        const m = IpMreq{ .multiaddr = be32(group), .interface = be32(interface) };
        r = setsockopt(fd, IPPROTO_IP, if (join != 0) IP_ADD_MEMBERSHIP else IP_DROP_MEMBERSHIP, &m, @sizeOf(IpMreq));
    } else {
        const m = IpMreqSource{ .multiaddr = be32(group), .interface = be32(interface), .sourceaddr = be32(source) };
        r = setsockopt(fd, IPPROTO_IP, if (join != 0) IP_ADD_SOURCE_MEMBERSHIP else IP_DROP_SOURCE_MEMBERSHIP, &m, @sizeOf(IpMreqSource));
    }
    if (r < 0) {
        if (errno() == EOPNOTSUPP) return -6;
        return socketError(errno());
    }
    return 0;
}
pub fn J_sun_nio_ch_Net__blockOrUnblock4(block: i32, fdo: Ref, group: i32, interface: i32, source: i32) E!i32 {
    const m = IpMreqSource{ .multiaddr = be32(group), .interface = be32(interface), .sourceaddr = be32(source) };
    if (setsockopt(try fdOf(fdo), IPPROTO_IP, if (block != 0) IP_BLOCK_SOURCE else IP_UNBLOCK_SOURCE, &m, @sizeOf(IpMreqSource)) < 0) {
        if (errno() == EOPNOTSUPP) return -6;
        return socketError(errno());
    }
    return 0;
}
pub fn J_sun_nio_ch_Net__joinOrDrop6(join: i32, fdo: Ref, group: Ref, index: i32, source: Ref) E!i32 {
    if (source != null) return -6;
    var m = Ipv6Mreq{ .multiaddr = undefined, .ifindex = @intCast(index) };
    @memcpy(&m.multiaddr, rt.arrayData(u8, rt.asArray(try rt.nn(group)))[0..16]);
    if (setsockopt(try fdOf(fdo), IPPROTO_IPV6, if (join != 0) IPV6_ADD_MEMBERSHIP else IPV6_DROP_MEMBERSHIP, &m, @sizeOf(Ipv6Mreq)) < 0)
        return socketError(errno());
    return 0;
}
pub fn J_sun_nio_ch_Net__blockOrUnblock6(block: i32, fdo: Ref, group: Ref, index: i32, source: Ref) E!i32 {
    _ = .{ block, fdo, group, index, source };
    return -6;
}
pub fn J_sun_nio_ch_Net__setInterface4(fdo: Ref, interface: i32) E!void {
    const a = be32(interface);
    if (setsockopt(try fdOf(fdo), IPPROTO_IP, IP_MULTICAST_IF, &a, 4) < 0) return socketError(errno());
}
pub fn J_sun_nio_ch_Net__getInterface4(fdo: Ref) E!i32 {
    var a: u32 = 0;
    var len: u32 = 4;
    if (getsockopt(try fdOf(fdo), IPPROTO_IP, IP_MULTICAST_IF, &a, &len) < 0) return socketError(errno());
    return @bitCast(std.mem.bigToNative(u32, a));
}
pub fn J_sun_nio_ch_Net__setInterface6(fdo: Ref, index: i32) E!void {
    const v: c_int = index;
    if (setsockopt(try fdOf(fdo), IPPROTO_IPV6, IPV6_MULTICAST_IF, &v, 4) < 0) return socketError(errno());
}
pub fn J_sun_nio_ch_Net__getInterface6(fdo: Ref) E!i32 {
    var v: c_int = 0;
    var len: u32 = 4;
    if (getsockopt(try fdOf(fdo), IPPROTO_IPV6, IPV6_MULTICAST_IF, &v, &len) < 0) return socketError(errno());
    return v;
}

// ------------------------------------------------------------------ NativeSocketAddress layout (Linux)
pub fn J_sun_nio_ch_NativeSocketAddress__AFINET() E!i32 {
    return AF_INET;
}
pub fn J_sun_nio_ch_NativeSocketAddress__AFINET6() E!i32 {
    return AF_INET6;
}
pub fn J_sun_nio_ch_NativeSocketAddress__sizeofSockAddr4() E!i32 {
    return 16;
}
pub fn J_sun_nio_ch_NativeSocketAddress__sizeofSockAddr6() E!i32 {
    return 28;
}
pub fn J_sun_nio_ch_NativeSocketAddress__sizeofFamily() E!i32 {
    return 2;
}
pub fn J_sun_nio_ch_NativeSocketAddress__offsetFamily() E!i32 {
    return 0;
}
pub fn J_sun_nio_ch_NativeSocketAddress__offsetSin4Port() E!i32 {
    return 2;
}
pub fn J_sun_nio_ch_NativeSocketAddress__offsetSin4Addr() E!i32 {
    return 4;
}
pub fn J_sun_nio_ch_NativeSocketAddress__offsetSin6Port() E!i32 {
    return 2;
}
pub fn J_sun_nio_ch_NativeSocketAddress__offsetSin6FlowInfo() E!i32 {
    return 4;
}
pub fn J_sun_nio_ch_NativeSocketAddress__offsetSin6Addr() E!i32 {
    return 8;
}
pub fn J_sun_nio_ch_NativeSocketAddress__offsetSin6ScopeId() E!i32 {
    return 24;
}

// ------------------------------------------------------------------ epoll / eventfd
const EpollEvent = if (builtin.cpu.arch == .x86_64)
    extern struct { events: u32 align(1), data: u64 align(1) }
else
    extern struct { events: u32, data: u64 };

pub fn J_sun_nio_ch_EPoll__eventSize() E!i32 {
    return @sizeOf(EpollEvent);
}
pub fn J_sun_nio_ch_EPoll__eventsOffset() E!i32 {
    return @offsetOf(EpollEvent, "events");
}
pub fn J_sun_nio_ch_EPoll__dataOffset() E!i32 {
    return @offsetOf(EpollEvent, "data");
}
pub fn J_sun_nio_ch_EPoll__create() E!i32 {
    const fd = epoll_create(256);
    if (fd < 0) return rt.throwKind(.IO, "epoll_create failed");
    return fd;
}
pub fn J_sun_nio_ch_EPoll__ctl(epfd: i32, op: i32, fd: i32, events: i32) E!i32 {
    const ev = EpollEvent{ .events = @bitCast(events), .data = @as(u64, @intCast(fd)) };
    return if (epoll_ctl(epfd, op, fd, &ev) == 0) 0 else errno();
}
pub fn J_sun_nio_ch_EPoll__wait(epfd: i32, address: i64, numfds: i32, timeout: i32) E!i32 {
    rt.enterSafe();
    const n = epoll_wait(epfd, @ptrFromInt(ptr(address)), numfds, timeout);
    const e = errno();
    rt.leaveSafe();
    if (n < 0) {
        if (e == EINTR) return IOS_INTERRUPTED;
        return rt.throwKind(.IO, "epoll_wait failed");
    }
    return n;
}
pub fn J_sun_nio_ch_EventFD__eventfd0() E!i32 {
    const fd = eventfd(0, 0);
    if (fd < 0) return ioFail();
    return fd;
}
pub fn J_sun_nio_ch_EventFD__set0(fd: i32) E!i32 {
    const one: u64 = 1;
    if (write(fd, std.mem.asBytes(&one).ptr, 8) < 0) return ioFail();
    return 0;
}

// ------------------------------------------------------------------ SocketDispatcher / DatagramDispatcher
pub fn J_sun_nio_ch_SocketDispatcher__read0(fdo: Ref, a: i64, len: i32) E!i32 {
    const fd = try fdOf(fdo);
    rt.enterSafe();
    const n = read(fd, @ptrFromInt(ptr(a)), @intCast(len));
    rt.leaveSafe();
    return convert(n, true);
}
pub fn J_sun_nio_ch_SocketDispatcher__readv0(fdo: Ref, a: i64, len: i32) E!i64 {
    const fd = try fdOf(fdo);
    rt.enterSafe();
    const n = readv(fd, @ptrFromInt(ptr(a)), len);
    rt.leaveSafe();
    return convertLong(n, true);
}
pub fn J_sun_nio_ch_SocketDispatcher__write0(fdo: Ref, a: i64, len: i32) E!i32 {
    const fd = try fdOf(fdo);
    rt.enterSafe();
    const n = send(fd, @ptrFromInt(ptr(a)), @intCast(len), MSG_NOSIGNAL);
    rt.leaveSafe();
    return convert(n, false);
}
pub fn J_sun_nio_ch_SocketDispatcher__writev0(fdo: Ref, a: i64, len: i32) E!i64 {
    const fd = try fdOf(fdo);
    rt.enterSafe();
    const n = writev(fd, @ptrFromInt(ptr(a)), len);
    rt.leaveSafe();
    return convertLong(n, false);
}
pub fn J_sun_nio_ch_DatagramDispatcher__read0(fdo: Ref, a: i64, len: i32) E!i32 {
    return J_sun_nio_ch_SocketDispatcher__read0(fdo, a, len);
}
pub fn J_sun_nio_ch_DatagramDispatcher__readv0(fdo: Ref, a: i64, len: i32) E!i64 {
    return J_sun_nio_ch_SocketDispatcher__readv0(fdo, a, len);
}
pub fn J_sun_nio_ch_DatagramDispatcher__write0(fdo: Ref, a: i64, len: i32) E!i32 {
    return J_sun_nio_ch_SocketDispatcher__write0(fdo, a, len);
}
pub fn J_sun_nio_ch_DatagramDispatcher__writev0(fdo: Ref, a: i64, len: i32) E!i64 {
    return J_sun_nio_ch_SocketDispatcher__writev0(fdo, a, len);
}
pub fn J_sun_nio_ch_DatagramDispatcher__dup0(a: Ref, b: Ref) E!void {
    if (dup2(try fdOf(a), try fdOf(b)) < 0) return ioFail();
}
pub fn J_sun_nio_ch_DatagramChannelImpl__disconnect0(fdo: Ref, ipv6: i32) E!void {
    _ = ipv6;
    var st: Storage = .{};
    if (connect(try fdOf(fdo), &st, 16) < 0 and errno() != 97) return socketError(errno()); // EAFNOSUPPORT is the success case
}
pub fn J_sun_nio_ch_DatagramChannelImpl__receive0(fdo: Ref, buf: i64, len: i32, sender: i64, connected: i32) E!i32 {
    const fd = try fdOf(fdo);
    var alen: u32 = 28;
    rt.enterSafe();
    const n = recvfrom(fd, @ptrFromInt(ptr(buf)), @intCast(len), 0, @ptrFromInt(ptr(sender)), &alen);
    const e = errno();
    rt.leaveSafe();
    if (n < 0) {
        if (e == EAGAIN) return IOS_UNAVAILABLE;
        if (e == EINTR) return IOS_INTERRUPTED;
        if (e == ECONNREFUSED and connected != 0) return throwNamed("java/net/PortUnreachableException", "ICMP Port Unreachable");
        return socketError(e);
    }
    return @intCast(n);
}
pub fn J_sun_nio_ch_DatagramChannelImpl__send0(fdo: Ref, buf: i64, len: i32, target: i64, target_len: i32) E!i32 {
    const fd = try fdOf(fdo);
    rt.enterSafe();
    const n = sendto(fd, @ptrFromInt(ptr(buf)), @intCast(len), 0, if (target_len > 0) @ptrFromInt(ptr(target)) else null, @intCast(target_len));
    const e = errno();
    rt.leaveSafe();
    if (n < 0) {
        if (e == EAGAIN) return IOS_UNAVAILABLE;
        if (e == EINTR) return IOS_INTERRUPTED;
        if (e == ECONNREFUSED) return throwNamed("java/net/PortUnreachableException", "ICMP Port Unreachable");
        return socketError(e);
    }
    return @intCast(n);
}

// ------------------------------------------------------------------ unix domain sockets
const SockUn = extern struct { family: u16, path: [108]u8 };

fn unAddr(path: Ref, un: *SockUn) E!u32 {
    const p = rt.arrayData(u8, rt.asArray(try rt.nn(path)));
    const n: usize = @intCast(rt.asArray(path.?).length);
    if (n >= un.path.len) return throwNamed("java/net/SocketException", "Unix domain path too long");
    un.* = .{ .family = AF_UNIX, .path = @splat(0) };
    @memcpy(un.path[0..n], p[0..n]);
    return @intCast(2 + n + (if (n > 0 and un.path[0] != 0) @as(usize, 1) else 0));
}

pub fn J_sun_nio_ch_UnixDomainSockets__init() E!i32 {
    return 1;
}
pub fn J_sun_nio_ch_UnixDomainSockets__socket0() E!i32 {
    const fd = socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0);
    if (fd < 0) return socketError(errno());
    return fd;
}
pub fn J_sun_nio_ch_UnixDomainSockets__bind0(fdo: Ref, path: Ref) E!void {
    var un: SockUn = undefined;
    const len = try unAddr(path, &un);
    if (bind(try fdOf(fdo), &un, len) < 0) return socketError(errno());
}
pub fn J_sun_nio_ch_UnixDomainSockets__connect0(fdo: Ref, path: Ref) E!i32 {
    var un: SockUn = undefined;
    const len = try unAddr(path, &un);
    const fd = try fdOf(fdo);
    rt.enterSafe();
    const r = connect(fd, &un, len);
    const e = errno();
    rt.leaveSafe();
    if (r < 0) {
        if (e == EINPROGRESS) return IOS_UNAVAILABLE;
        if (e == EINTR) return IOS_INTERRUPTED;
        return socketError(e);
    }
    return 1;
}
fn unPath(un: *const SockUn, len: u32) E!Ref {
    var n: usize = if (len > 2) len - 2 else 0;
    if (n > 0 and un.path[0] != 0) n = std.mem.indexOfScalar(u8, un.path[0..n], 0) orelse n;
    return newBytes(un.path[0..n]);
}
pub fn J_sun_nio_ch_UnixDomainSockets__accept0(fdo: Ref, newfdo: Ref, array: Ref) E!i32 {
    const fd = try fdOf(fdo);
    var un: SockUn = undefined;
    var len: u32 = @sizeOf(SockUn);
    rt.enterSafe();
    const nfd = accept4(fd, &un, &len, SOCK_CLOEXEC);
    const e = errno();
    rt.leaveSafe();
    if (nfd < 0) {
        if (e == EAGAIN) return IOS_UNAVAILABLE;
        if (e == EINTR) return IOS_INTERRUPTED;
        return socketError(e);
    }
    P.C_java_io_FileDescriptor.cast(try rt.nn(newfdo)).f_fd = nfd;
    var R: [1]Ref = .{null};
    var F = rt.Frame{ .refs = &R, .n = 1, .meth = &meta };
    try rt.pushFrame(&F);
    defer rt.popFrame(&F);
    R[0] = try unPath(&un, len);
    try rt.aastore(array, 0, R[0]);
    return 1;
}
pub fn J_sun_nio_ch_UnixDomainSockets__localAddress0(fdo: Ref) E!Ref {
    var un: SockUn = undefined;
    var len: u32 = @sizeOf(SockUn);
    if (getsockname(try fdOf(fdo), &un, &len) < 0) return socketError(errno());
    return unPath(&un, len);
}

// ------------------------------------------------------------------ inotify (LinuxWatchService)
pub fn J_sun_nio_fs_LinuxWatchService__eventSize() E!i32 {
    return 16;
}
pub fn J_sun_nio_fs_LinuxWatchService__eventOffsets() E!Ref {
    const a = try rt.newArray(rt.arrayOf(rt.prim_I), 5);
    const d = rt.arrayData(i32, rt.asArray(a.?));
    d[0] = 0; // wd
    d[1] = 4; // mask
    d[2] = 8; // cookie
    d[3] = 12; // len
    d[4] = 16; // name
    return a;
}
pub fn J_sun_nio_fs_LinuxWatchService__inotifyInit() E!i32 {
    const fd = inotify_init();
    if (fd < 0) return rt.throwKind(.IO, errMsg(errno()));
    return fd;
}
pub fn J_sun_nio_fs_LinuxWatchService__inotifyAddWatch(fd: i32, path: i64, mask: i32) E!i32 {
    const wd = inotify_add_watch(fd, @ptrFromInt(ptr(path)), @bitCast(mask));
    if (wd < 0) return rt.throwKind(.IO, errMsg(errno()));
    return wd;
}
pub fn J_sun_nio_fs_LinuxWatchService__inotifyRmWatch(fd: i32, wd: i32) E!void {
    _ = inotify_rm_watch(fd, wd);
}
pub fn J_sun_nio_fs_LinuxWatchService__configureBlocking(fd: i32, blocking: i32) E!void {
    const fl = fcntl(fd, 3);
    const nfl = if (blocking != 0) fl & ~@as(c_int, 0o4000) else fl | 0o4000;
    if (fl < 0 or fcntl(fd, 4, nfl) < 0) return ioFail();
}
pub fn J_sun_nio_fs_LinuxWatchService__socketpair(arr: Ref) E!void {
    var sv: [2]c_int = undefined;
    if (socketpair(AF_UNIX, SOCK_STREAM, 0, &sv) < 0) return ioFail();
    const d = rt.arrayData(i32, rt.asArray(try rt.nn(arr)));
    d[0] = sv[0];
    d[1] = sv[1];
}
pub fn J_sun_nio_fs_LinuxWatchService__poll(fd1: i32, fd2: i32) E!i32 {
    var p = [2]PollFd{ .{ .fd = fd1, .events = POLLIN, .revents = 0 }, .{ .fd = fd2, .events = POLLIN, .revents = 0 } };
    rt.enterSafe();
    const r = poll(&p, 2, -1);
    const e = errno();
    rt.leaveSafe();
    if (r < 0) {
        if (e == EINTR) return 0;
        return ioFail();
    }
    return r;
}

// ------------------------------------------------------------------ java.net.InetAddress
pub fn J_java_net_InetAddress__init() E!void {}
pub fn J_java_net_Inet4Address__init() E!void {}
pub fn J_java_net_Inet6Address__init() E!void {}
pub fn J_java_net_InetAddress__isIPv4Available() E!i32 {
    return 1;
}
pub fn J_java_net_InetAddress__isIPv6Supported() E!i32 {
    return @intFromBool(ipv6Available());
}

fn hostName(buf: *[256]u8) []const u8 {
    if (gethostname(buf, buf.len - 1) != 0) return "localhost";
    return std.mem.sliceTo(buf, 0);
}
pub fn J_java_net_Inet4AddressImpl__getLocalHostName(this: Ref) E!Ref {
    _ = this;
    var buf: [256]u8 = undefined;
    return rt.newStringUtf8(hostName(&buf));
}
pub fn J_java_net_Inet6AddressImpl__getLocalHostName(this: Ref) E!Ref {
    return J_java_net_Inet4AddressImpl__getLocalHostName(this);
}

fn utf8Of(s: Ref, buf: []u8) E![:0]const u8 {
    const chars = rt.hooks.string_chars(try rt.nn(s));
    var n: usize = 0;
    for (chars) |c| {
        if (c >= 0x80 or n + 1 >= buf.len) return throwNamed("java/net/UnknownHostException", "invalid host name");
        buf[n] = @intCast(c);
        n += 1;
    }
    buf[n] = 0;
    return buf[0..n :0];
}

/// flags: 1 IPv4, 2 IPv6, 4 IPv4 first, 8 IPv6 first (InetAddress.PREFER_*)
fn lookup(host: Ref, want_v4: bool, want_v6: bool, v6_first: bool) E!Ref {
    var hbuf: [256]u8 = undefined;
    const h = try utf8Of(host, &hbuf);
    var hints = std.mem.zeroes(AddrInfo);
    hints.family = if (want_v4 and want_v6) AF_UNSPEC else if (want_v4) AF_INET else AF_INET6;
    hints.socktype = SOCK_STREAM;
    var res: ?*AddrInfo = null;
    rt.enterSafe();
    const rc = getaddrinfo(h, null, &hints, &res);
    rt.leaveSafe();
    if (rc != 0) {
        var m: [320]u8 = undefined;
        const msg = std.fmt.bufPrint(&m, "{s}: {s}", .{ h, std.mem.span(gai_strerror(rc)) }) catch "unknown host";
        return throwNamed("java/net/UnknownHostException", msg);
    }
    defer if (res) |r| freeaddrinfo(r);
    var v4: std.ArrayList([16]u8) = .empty;
    var v6: std.ArrayList(struct { a: [16]u8, scope: u32 }) = .empty;
    defer v4.deinit(std.heap.c_allocator);
    defer v6.deinit(std.heap.c_allocator);
    var it = res;
    while (it) |ai| : (it = ai.next) {
        const sa_v = loadSa(ai.addr) orelse continue;
        const sa = &sa_v;
        if (ai.family == AF_INET and want_v4) {
            var e: [16]u8 = @splat(0);
            @memcpy(e[0..4], sa.data[4..8]);
            var dup = false;
            for (v4.items) |x| if (std.mem.eql(u8, x[0..4], e[0..4])) {
                dup = true;
            };
            if (!dup) v4.append(std.heap.c_allocator, e) catch return rt.throwKind(.OutOfMemory, "lookup");
        } else if (ai.family == AF_INET6 and want_v6) {
            var e: [16]u8 = undefined;
            @memcpy(&e, sa.data[8..24]);
            var dup = false;
            for (v6.items) |x| if (std.mem.eql(u8, &x.a, &e)) {
                dup = true;
            };
            if (!dup) v6.append(std.heap.c_allocator, .{ .a = e, .scope = std.mem.readInt(u32, sa.data[24..28], builtin.cpu.arch.endian()) }) catch return rt.throwKind(.OutOfMemory, "lookup");
        }
    }
    const total = v4.items.len + v6.items.len;
    if (total == 0) return throwNamed("java/net/UnknownHostException", h);
    var R: [3]Ref = @splat(null);
    var F = rt.Frame{ .refs = &R, .n = 3, .meth = &meta };
    try rt.pushFrame(&F);
    defer rt.popFrame(&F);
    R[0] = try rt.newArray(rt.arrayOf(P.C_java_net_InetAddress.cls()), @intCast(total));
    var k: i32 = 0;
    const order = [2]bool{ v6_first, !v6_first };
    for (order) |is6| {
        if (is6) {
            for (v6.items) |x| {
                R[1] = try inetFromBytes(&x.a, h, x.scope);
                try rt.aastore(R[0], k, R[1]);
                k += 1;
            }
        } else {
            for (v4.items) |x| {
                R[1] = try inetFromBytes(x[0..4], h, 0);
                try rt.aastore(R[0], k, R[1]);
                k += 1;
            }
        }
    }
    return R[0];
}
pub fn J_java_net_Inet4AddressImpl__lookupAllHostAddr(this: Ref, host: Ref) E!Ref {
    _ = this;
    return lookup(host, true, false, false);
}
pub fn J_java_net_Inet6AddressImpl__lookupAllHostAddr(this: Ref, host: Ref, characteristics: i32) E!Ref {
    _ = this;
    const c: u32 = @bitCast(characteristics);
    const v4 = c & 1 != 0;
    const v6 = c & 2 != 0;
    return lookup(host, v4 or !v6, v6 or !v4, c & 8 != 0);
}
fn reverse(addr: Ref) E!Ref {
    const b = rt.arrayData(u8, rt.asArray(try rt.nn(addr)));
    const n = rt.asArray(addr.?).length;
    var st: Storage = .{};
    var len: u32 = undefined;
    if (n == 4) {
        std.mem.writeInt(u16, st.data[0..2], AF_INET, builtin.cpu.arch.endian());
        @memcpy(st.data[4..8], b[0..4]);
        len = 16;
    } else {
        std.mem.writeInt(u16, st.data[0..2], AF_INET6, builtin.cpu.arch.endian());
        @memcpy(st.data[8..24], b[0..16]);
        len = 28;
    }
    var host: [1025]u8 = undefined;
    rt.enterSafe();
    const rc = getnameinfo(&st, len, &host, host.len, null, 0, 8); // NI_NAMEREQD
    rt.leaveSafe();
    if (rc != 0) return throwNamed("java/net/UnknownHostException", "");
    return rt.newStringUtf8(std.mem.sliceTo(&host, 0));
}
pub fn J_java_net_Inet4AddressImpl__getHostByAddr(this: Ref, addr: Ref) E!Ref {
    _ = this;
    return reverse(addr);
}
pub fn J_java_net_Inet6AddressImpl__getHostByAddr(this: Ref, addr: Ref) E!Ref {
    _ = this;
    return reverse(addr);
}

/// Reachability without raw ICMP sockets: a TCP connect to the echo port that gets either an
/// accept or a refusal proves the host answers (the same fallback the JDK uses without privileges).
fn reachable(st: *Storage, len: u32, timeout_ms: i32) bool {
    const fam: c_int = std.mem.readInt(u16, st.data[0..2], builtin.cpu.arch.endian());
    const fd = socket(fam, SOCK_STREAM | SOCK_NONBLOCK | SOCK_CLOEXEC, 0);
    if (fd < 0) return false;
    defer _ = close(fd);
    std.mem.writeInt(u16, st.data[2..4], std.mem.nativeToBig(u16, 7), builtin.cpu.arch.endian());
    if (connect(fd, st, len) == 0) return true;
    if (errno() == ECONNREFUSED) return true;
    if (errno() != EINPROGRESS) return false;
    var p = PollFd{ .fd = fd, .events = POLLOUT, .revents = 0 };
    if (poll(&p, 1, timeout_ms) <= 0) return false;
    var err: c_int = 0;
    var l: u32 = 4;
    _ = getsockopt(fd, SOL_SOCKET, SO_ERROR, &err, &l);
    return err == 0 or err == ECONNREFUSED;
}
pub fn J_java_net_Inet4AddressImpl__isReachable0(this: Ref, addr: Ref, timeout: i32, ifaddr: Ref, ttl: i32) E!i32 {
    _ = .{ this, ifaddr, ttl };
    var st: Storage = .{};
    std.mem.writeInt(u16, st.data[0..2], AF_INET, builtin.cpu.arch.endian());
    @memcpy(st.data[4..8], rt.arrayData(u8, rt.asArray(try rt.nn(addr)))[0..4]);
    rt.enterSafe();
    defer rt.leaveSafe();
    return @intFromBool(reachable(&st, 16, timeout));
}
pub fn J_java_net_Inet6AddressImpl__isReachable0(this: Ref, addr: Ref, scope: i32, timeout: i32, ifaddr: Ref, ttl: i32, ifscope: i32) E!i32 {
    _ = .{ this, ifaddr, ttl, ifscope };
    var st: Storage = .{};
    std.mem.writeInt(u16, st.data[0..2], AF_INET6, builtin.cpu.arch.endian());
    @memcpy(st.data[8..24], rt.arrayData(u8, rt.asArray(try rt.nn(addr)))[0..16]);
    std.mem.writeInt(u32, st.data[24..28], @bitCast(scope), builtin.cpu.arch.endian());
    rt.enterSafe();
    defer rt.leaveSafe();
    return @intFromBool(reachable(&st, 28, timeout));
}

// ------------------------------------------------------------------ java.net.NetworkInterface
const IFF_UP = 0x1;
const IFF_BROADCAST = 0x2;
const IFF_LOOPBACK = 0x8;
const IFF_POINTOPOINT = 0x10;
const IFF_MULTICAST = 0x1000;

fn ifFlags(name: []const u8) ?c_uint {
    var head: ?*IfAddrs = null;
    if (getifaddrs(&head) != 0) return null;
    defer if (head) |h| freeifaddrs(h);
    var it = head;
    while (it) |a| : (it = a.next) if (std.mem.eql(u8, std.mem.span(a.name), name)) return a.flags;
    return null;
}

pub fn J_java_net_NetworkInterface__init() E!void {}

fn prefixLen(mask: *const Storage) i16 {
    const fam = std.mem.readInt(u16, mask.data[0..2], builtin.cpu.arch.endian());
    const bytes: []const u8 = if (fam == AF_INET) mask.data[4..8] else mask.data[8..24];
    var n: i16 = 0;
    for (bytes) |b| n += @popCount(b);
    return n;
}

/// Builds NetworkInterface objects (R[..] holds the roots) for all interfaces, or those matching
/// the predicate.
const Filter = union(enum) { all, name: []const u8, index: u32, addr: Ref };

fn buildInterfaces(filter: Filter, want_array: bool) E!Ref {
    var head: ?*IfAddrs = null;
    if (getifaddrs(&head) != 0) return throwNamed("java/net/SocketException", errMsg(errno()));
    defer if (head) |h| freeifaddrs(h);

    // distinct interface names in order
    var names: std.ArrayList([]const u8) = .empty;
    defer names.deinit(std.heap.c_allocator);
    var it = head;
    while (it) |a| : (it = a.next) {
        const n = std.mem.span(a.name);
        var seen = false;
        for (names.items) |x| if (std.mem.eql(u8, x, n)) {
            seen = true;
        };
        if (!seen) names.append(std.heap.c_allocator, n) catch return rt.throwKind(.OutOfMemory, "interfaces");
    }

    var R: [8]Ref = @splat(null);
    var F = rt.Frame{ .refs = &R, .n = 8, .meth = &meta };
    try rt.pushFrame(&F);
    defer rt.popFrame(&F);
    const NI = P.C_java_net_NetworkInterface;
    const IA = P.C_java_net_InterfaceAddress;
    // R[0] result array / single, R[1] ni, R[2] addrs, R[3] bindings, R[4] tmp addr, R[5] ia, R[6] name
    var list: std.ArrayList(Ref) = .empty; // interface objects: rooted through R[0] array below
    defer list.deinit(std.heap.c_allocator);
    R[0] = try rt.newArray(rt.arrayOf(NI.cls()), @intCast(names.items.len));
    var count: i32 = 0;
    for (names.items) |nm| {
        var nbuf: [64:0]u8 = undefined;
        const nl = @min(nm.len, 63);
        @memcpy(nbuf[0..nl], nm[0..nl]);
        nbuf[nl] = 0;
        const idx = if_nametoindex(&nbuf);
        switch (filter) {
            .name => |want| if (!std.mem.eql(u8, want, nm)) continue,
            .index => |want| if (want != idx) continue,
            else => {},
        }
        // collect addresses of this interface
        var n_addr: usize = 0;
        var cur = head;
        while (cur) |a| : (cur = a.next) {
            if (!std.mem.eql(u8, std.mem.span(a.name), nm)) continue;
            const sa_v = loadSa(a.addr) orelse continue;
            const sa = &sa_v;
            const fam = std.mem.readInt(u16, sa.data[0..2], builtin.cpu.arch.endian());
            if (fam == AF_INET or fam == AF_INET6) n_addr += 1;
        }
        R[2] = try rt.newArray(rt.arrayOf(P.C_java_net_InetAddress.cls()), @intCast(n_addr));
        R[3] = try rt.newArray(rt.arrayOf(IA.cls()), @intCast(n_addr));
        var k: i32 = 0;
        cur = head;
        while (cur) |a| : (cur = a.next) {
            if (!std.mem.eql(u8, std.mem.span(a.name), nm)) continue;
            const sa_v = loadSa(a.addr) orelse continue;
            const sa = &sa_v;
            const fam = std.mem.readInt(u16, sa.data[0..2], builtin.cpu.arch.endian());
            if (fam != AF_INET and fam != AF_INET6) continue;
            var port: i32 = 0;
            R[4] = try fromSockaddr(sa, &port);
            // a scoped IPv6 address carries the interface scope
            try rt.aastore(R[2], k, R[4]);
            R[5] = try rt.newObject(IA.cls());
            const ia = IA.cast(R[5].?);
            ia.f_address = R[4];
            if (loadSa(a.netmask)) |m| ia.f_maskLength = prefixLen(&m);
            if (fam == AF_INET and a.flags & IFF_BROADCAST != 0 and a.flags & IFF_POINTOPOINT == 0) {
                if (loadSa(a.ifu)) |b| {
                    R[6] = try fromSockaddr(&b, &port);
                    ia.f_broadcast = R[6];
                }
            }
            try rt.aastore(R[3], k, R[5]);
            k += 1;
        }
        R[1] = try rt.newObject(NI.cls());
        const ni = NI.cast(R[1].?);
        R[6] = try rt.newStringUtf8(nm);
        ni.f_name = R[6];
        ni.f_displayName = R[6];
        ni.f_index = @intCast(idx);
        ni.f_addrs = R[2];
        ni.f_bindings = R[3];
        ni.f_childs = try rt.newArray(rt.arrayOf(NI.cls()), 0);
        switch (filter) {
            .addr => |want| {
                var match = false;
                var q: i32 = 0;
                while (q < k) : (q += 1) {
                    const e = rt.arrayData(Ref, rt.asArray(R[2].?))[@intCast(q)];
                    if (addrEquals(e, want)) match = true;
                }
                if (!match) continue;
            },
            else => {},
        }
        try rt.aastore(R[0], count, R[1]);
        count += 1;
    }
    if (!want_array) {
        if (count == 0) return null;
        return rt.arrayData(Ref, rt.asArray(R[0].?))[0];
    }
    if (count == @as(i32, @intCast(names.items.len))) return R[0];
    R[7] = try rt.newArray(rt.arrayOf(NI.cls()), count);
    var i: i32 = 0;
    while (i < count) : (i += 1) try rt.aastore(R[7], i, rt.arrayData(Ref, rt.asArray(R[0].?))[@intCast(i)]);
    return R[7];
}

fn addrEquals(a: Ref, b: Ref) bool {
    const x = a orelse return false;
    const y = b orelse return false;
    if (x.class != y.class) return false;
    if (isInet6(x)) {
        const hx = P.C_java_net_Inet6Address_2Inet6AddressHolder.cast(P.C_java_net_Inet6Address.cast(x).f_holder6.?);
        const hy = P.C_java_net_Inet6Address_2Inet6AddressHolder.cast(P.C_java_net_Inet6Address.cast(y).f_holder6.?);
        return std.mem.eql(u8, rt.arrayData(u8, rt.asArray(hx.f_ipaddress.?))[0..16], rt.arrayData(u8, rt.asArray(hy.f_ipaddress.?))[0..16]);
    }
    const hx = P.C_java_net_InetAddress_2InetAddressHolder.cast(P.C_java_net_InetAddress.cast(x).f_holder.?);
    const hy = P.C_java_net_InetAddress_2InetAddressHolder.cast(P.C_java_net_InetAddress.cast(y).f_holder.?);
    return hx.f_address == hy.f_address;
}

pub fn J_java_net_NetworkInterface__getAll() E!Ref {
    return buildInterfaces(.all, true);
}
pub fn J_java_net_NetworkInterface__getByName0(name: Ref) E!Ref {
    var b: [128]u8 = undefined;
    const n = try utf8Of(name, &b);
    return buildInterfaces(.{ .name = n }, false);
}
pub fn J_java_net_NetworkInterface__getByIndex0(idx: i32) E!Ref {
    return buildInterfaces(.{ .index = @intCast(idx) }, false);
}
pub fn J_java_net_NetworkInterface__getByInetAddress0(addr: Ref) E!Ref {
    return buildInterfaces(.{ .addr = addr }, false);
}
pub fn J_java_net_NetworkInterface__boundInetAddress0(addr: Ref) E!i32 {
    return @intFromBool((try buildInterfaces(.{ .addr = addr }, false)) != null);
}
fn flagOf(name: Ref, bit: c_uint) E!i32 {
    var b: [128]u8 = undefined;
    const n = try utf8Of(name, &b);
    const f = ifFlags(n) orelse return 0;
    return @intFromBool(f & bit != 0);
}
pub fn J_java_net_NetworkInterface__isUp0(name: Ref, index: i32) E!i32 {
    _ = index;
    return flagOf(name, IFF_UP);
}
pub fn J_java_net_NetworkInterface__isLoopback0(name: Ref, index: i32) E!i32 {
    _ = index;
    return flagOf(name, IFF_LOOPBACK);
}
pub fn J_java_net_NetworkInterface__supportsMulticast0(name: Ref, index: i32) E!i32 {
    _ = index;
    return flagOf(name, IFF_MULTICAST);
}
pub fn J_java_net_NetworkInterface__isP2P0(name: Ref, index: i32) E!i32 {
    _ = index;
    return flagOf(name, IFF_POINTOPOINT);
}
pub fn J_java_net_NetworkInterface__getMTU0(name: Ref, index: i32) E!i32 {
    _ = index;
    var b: [128]u8 = undefined;
    const n = try utf8Of(name, &b);
    var path: [160]u8 = undefined;
    const p = std.fmt.bufPrint(&path, "/sys/class/net/{s}/mtu", .{n}) catch return -1;
    var f = std.fs.openFileAbsolute(p, .{}) catch return -1;
    defer f.close();
    var buf: [32]u8 = undefined;
    const len = f.read(&buf) catch return -1;
    return std.fmt.parseInt(i32, std.mem.trim(u8, buf[0..len], " \n"), 10) catch -1;
}
pub fn J_java_net_NetworkInterface__getMacAddr0(addr: Ref, name: Ref, index: i32) E!Ref {
    _ = .{ addr, index };
    var b: [128]u8 = undefined;
    const n = try utf8Of(name, &b);
    var path: [160]u8 = undefined;
    const p = std.fmt.bufPrint(&path, "/sys/class/net/{s}/address", .{n}) catch return null;
    var f = std.fs.openFileAbsolute(p, .{}) catch return null;
    defer f.close();
    var buf: [64]u8 = undefined;
    const len = f.read(&buf) catch return null;
    var mac: [6]u8 = undefined;
    var it = std.mem.splitScalar(u8, std.mem.trim(u8, buf[0..len], " \n"), ':');
    var i: usize = 0;
    while (it.next()) |part| : (i += 1) {
        if (i >= 6) return null;
        mac[i] = std.fmt.parseInt(u8, part, 16) catch return null;
    }
    if (i != 6 or std.mem.allEqual(u8, &mac, 0)) return null;
    return newBytes(&mac);
}
