//! Networking natives: sun.nio.ch.Net, SocketDispatcher, DatagramDispatcher,
//! DatagramChannelImpl, NativeSocketAddress, UnixDomainSockets, InetAddress,
//! Inet4AddressImpl, Inet6AddressImpl, NetworkInterface, DefaultProxySelector, SdpSupport.
use crate::gc::ScopedBlock;
use crate::rt::{self, Ref, Res};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_ulong, c_void};
use std::sync::atomic::{AtomicI32, Ordering};

const IOS_EOF: i32 = -1;
const IOS_UNAVAILABLE: i32 = -2;
const IOS_INTERRUPTED: i32 = -3;
const IOS_UNSUPPORTED_CASE: i32 = -6;

const EINTR: i32 = 4;
const EAGAIN: i32 = 11;
const EACCES: i32 = 13;
const EPIPE: i32 = 32;
const EPROTO: i32 = 71;
const EOPNOTSUPP: i32 = 95;
const EAFNOSUPPORT: i32 = 97;
const EADDRINUSE: i32 = 98;
const EADDRNOTAVAIL: i32 = 99;
const ENETUNREACH: i32 = 101;
const ECONNABORTED: i32 = 103;
const ECONNRESET: i32 = 104;
const ENOTCONN: i32 = 107;
const ETIMEDOUT: i32 = 110;
const ECONNREFUSED: i32 = 111;
const EHOSTUNREACH: i32 = 113;
const EINPROGRESS: i32 = 115;

const AF_UNSPEC: c_int = 0;
const AF_UNIX: c_int = 1;
const AF_INET: c_int = 2;
const AF_INET6: c_int = 10;
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
const IP_MULTICAST_IF: c_int = 32;
const IP_MULTICAST_TTL: c_int = 33;
const IP_MULTICAST_LOOP: c_int = 34;
const IP_ADD_MEMBERSHIP: c_int = 35;
const IP_DROP_MEMBERSHIP: c_int = 36;
const IP_UNBLOCK_SOURCE: c_int = 37;
const IP_BLOCK_SOURCE: c_int = 38;
const IP_ADD_SOURCE_MEMBERSHIP: c_int = 39;
const IP_DROP_SOURCE_MEMBERSHIP: c_int = 40;
const IPV6_MULTICAST_IF: c_int = 17;
const IPV6_ADD_MEMBERSHIP: c_int = 20;
const IPV6_DROP_MEMBERSHIP: c_int = 21;
const MSG_OOB: c_int = 1;
const MSG_NOSIGNAL: c_int = 0x4000;
const FIONREAD: c_ulong = 0x541b;
const POLLIN: i16 = 1;
const POLLOUT: i16 = 4;
const POLLERR: i16 = 8;
const POLLHUP: i16 = 0x10;
const POLLNVAL: i16 = 0x20;
const NI_NAMEREQD: c_int = 8;

const IFF_UP: u32 = 0x1;
const IFF_BROADCAST: u32 = 0x2;
const IFF_LOOPBACK: u32 = 0x8;
const IFF_POINTOPOINT: u32 = 0x10;
const IFF_MULTICAST: u32 = 0x1000;

#[repr(C)]
struct PollFd {
    fd: c_int,
    events: i16,
    revents: i16,
}

#[repr(C)]
struct Linger {
    l_onoff: c_int,
    l_linger: c_int,
}

#[repr(C)]
struct IpMreq {
    multiaddr: u32,
    interface: u32,
}

#[repr(C)]
struct IpMreqSource {
    multiaddr: u32,
    interface: u32,
    sourceaddr: u32,
}

#[repr(C)]
struct Ipv6Mreq {
    multiaddr: [u8; 16],
    ifindex: u32,
}

#[repr(C)]
struct AddrInfo {
    ai_flags: c_int,
    ai_family: c_int,
    ai_socktype: c_int,
    ai_protocol: c_int,
    ai_addrlen: u32,
    ai_addr: *mut c_void,
    ai_canonname: *mut c_char,
    ai_next: *mut AddrInfo,
}

#[repr(C)]
struct IfAddrs {
    ifa_next: *mut IfAddrs,
    ifa_name: *mut c_char,
    ifa_flags: u32,
    ifa_addr: *mut c_void,
    ifa_netmask: *mut c_void,
    ifa_ifu: *mut c_void,
    ifa_data: *mut c_void,
}

#[repr(C, align(8))]
#[derive(Clone, Copy)]
struct Storage {
    data: [u8; 128],
}
impl Storage {
    fn zeroed() -> Self {
        Self { data: [0; 128] }
    }
}

#[repr(C)]
struct SockUn {
    family: u16,
    path: [u8; 108],
}

#[link(name = "c")]
extern "C" {
    fn __errno_location() -> *mut c_int;
    fn strerror(errnum: c_int) -> *const c_char;
    fn socket(domain: c_int, ty: c_int, protocol: c_int) -> c_int;
    fn bind(sockfd: c_int, addr: *const c_void, addrlen: u32) -> c_int;
    fn listen(sockfd: c_int, backlog: c_int) -> c_int;
    fn connect(sockfd: c_int, addr: *const c_void, addrlen: u32) -> c_int;
    fn accept4(sockfd: c_int, addr: *mut c_void, addrlen: *mut u32, flags: c_int) -> c_int;
    fn shutdown(sockfd: c_int, how: c_int) -> c_int;
    fn getsockname(sockfd: c_int, addr: *mut c_void, addrlen: *mut u32) -> c_int;
    fn getpeername(sockfd: c_int, addr: *mut c_void, addrlen: *mut u32) -> c_int;
    fn getsockopt(sockfd: c_int, level: c_int, optname: c_int, optval: *mut c_void, optlen: *mut u32) -> c_int;
    fn setsockopt(sockfd: c_int, level: c_int, optname: c_int, optval: *const c_void, optlen: u32) -> c_int;
    fn poll(fds: *mut PollFd, nfds: c_ulong, timeout: c_int) -> c_int;
    fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
    fn send(sockfd: c_int, buf: *const c_void, len: usize, flags: c_int) -> isize;
    fn sendto(sockfd: c_int, buf: *const c_void, len: usize, flags: c_int, dest_addr: *const c_void, addrlen: u32) -> isize;
    fn recvfrom(sockfd: c_int, buf: *mut c_void, len: usize, flags: c_int, src_addr: *mut c_void, addrlen: *mut u32) -> isize;
    fn read(fd: c_int, buf: *mut c_void, count: usize) -> isize;
    fn readv(fd: c_int, iov: *const c_void, iovcnt: c_int) -> isize;
    fn writev(fd: c_int, iov: *const c_void, iovcnt: c_int) -> isize;
    fn dup2(oldfd: c_int, newfd: c_int) -> c_int;
    fn close(fd: c_int) -> c_int;
    fn gethostname(name: *mut c_char, len: usize) -> c_int;
    fn getaddrinfo(node: *const c_char, service: *const c_char, hints: *const AddrInfo, res: *mut *mut AddrInfo) -> c_int;
    fn freeaddrinfo(res: *mut AddrInfo);
    fn gai_strerror(errcode: c_int) -> *const c_char;
    fn getnameinfo(addr: *const c_void, addrlen: u32, host: *mut c_char, hostlen: u32, serv: *mut c_char, servlen: u32, flags: c_int) -> c_int;
    fn getifaddrs(ifap: *mut *mut IfAddrs) -> c_int;
    fn freeifaddrs(ifa: *mut IfAddrs);
    fn if_nametoindex(ifname: *const c_char) -> u32;
}

static IPV6_CACHED: AtomicI32 = AtomicI32::new(-1);

#[inline(always)]
fn errno() -> i32 {
    unsafe { *__errno_location() }
}

fn err_msg(e: i32) -> String {
    unsafe { CStr::from_ptr(strerror(e)).to_string_lossy().into_owned() }
}

fn throw_net<T>(cls: &'static str, msg: String) -> Res<T> {
    if let Some(_) = rt::find_class(cls) {
        return rt::throw(cls, msg);
    }
    rt::throw("java/io/IOException", msg)
}

fn socket_error<T>(e: i32) -> Res<T> {
    match e {
        EINPROGRESS => throw_net("java/net/ConnectException", "Connection refused".to_string()),
        EPROTO => throw_net("java/net/ProtocolException", err_msg(e)),
        ECONNREFUSED | ETIMEDOUT | ENOTCONN => throw_net("java/net/ConnectException", err_msg(e)),
        EHOSTUNREACH | ENETUNREACH => throw_net("java/net/NoRouteToHostException", err_msg(e)),
        EADDRINUSE | EADDRNOTAVAIL | EACCES => throw_net("java/net/BindException", err_msg(e)),
        _ => throw_net("java/net/SocketException", err_msg(e)),
    }
}

fn io_fail<T>() -> Res<T> {
    rt::throw("java/io/IOException", err_msg(errno()))
}

fn convert_net(n: isize, reading: bool) -> Res<i32> {
    if n > 0 { return Ok(n as i32); }
    if n == 0 { return Ok(if reading { IOS_EOF } else { 0 }); }
    let e = errno();
    if e == EAGAIN { return Ok(IOS_UNAVAILABLE); }
    if e == EINTR { return Ok(IOS_INTERRUPTED); }
    if e == ECONNRESET { return throw_net("java/net/SocketException", "Connection reset".to_string()); }
    if e == EPIPE { return throw_net("java/net/SocketException", "Broken pipe".to_string()); }
    io_fail()
}

fn convert_net_long(n: isize, reading: bool) -> Res<i64> {
    if n > 0 { return Ok(n as i64); }
    if n == 0 { return Ok(if reading { IOS_EOF as i64 } else { 0 }); }
    let e = errno();
    if e == EAGAIN { return Ok(IOS_UNAVAILABLE as i64); }
    if e == EINTR { return Ok(IOS_INTERRUPTED as i64); }
    if e == ECONNRESET { return throw_net("java/net/SocketException", "Connection reset".to_string()); }
    if e == EPIPE { return throw_net("java/net/SocketException", "Broken pipe".to_string()); }
    io_fail()
}

fn ipv6_available() -> bool {
    let v = IPV6_CACHED.load(Ordering::Relaxed);
    if v >= 0 {
        return v == 1;
    }
    let fd = unsafe { socket(AF_INET6, SOCK_STREAM | SOCK_CLOEXEC, 0) };
    let ok = if fd >= 0 {
        unsafe { close(fd) };
        true
    } else {
        false
    };
    IPV6_CACHED.store(if ok { 1 } else { 0 }, Ordering::Relaxed);
    ok
}

fn new_bytes(b: &[u8]) -> Res<Ref> {
    let a = rt::new_array(b'B', b.len() as i32)?;
    unsafe { rt::byte_slice_mut(a)?.copy_from_slice(b) };
    Ok(a)
}

fn is_inet6(o: Ref) -> bool {
    if o.is_null() { return false; }
    unsafe { (*o.0).class.name == "java/net/Inet6Address" }
}

fn inet_from_bytes(bytes: &[u8], host: Option<&str>, scope: u32) -> Res<Ref> {
    let h = match host {
        Some(s) => rt::new_str(s.to_string()),
        None => Ref::NULL,
    };
    let ba = new_bytes(bytes)?;
    if bytes.len() == 16 && scope != 0 {
        let sc = rt::box_i32(scope as i32)?;
        return rt::call_reflective(
            "java/net/Inet6Address",
            "getByAddress",
            "(Ljava/lang/String;[BI)Ljava/net/Inet6Address;",
            Ref::NULL,
            &[h, ba, sc],
        );
    }
    rt::call_reflective(
        "java/net/InetAddress",
        "getByAddress",
        "(Ljava/lang/String;[B)Ljava/net/InetAddress;",
        Ref::NULL,
        &[h, ba],
    )
}

fn new_inet_socket_address(addr: Ref, port: i32) -> Res<Ref> {
    let c = rt::find_class("java/net/InetSocketAddress")
        .ok_or_else(|| rt::Thrown { obj: (rt::hooks().throw_new)("java/lang/InternalError", rt::new_str("InetSocketAddress missing".to_string())) })?;
    let isa = rt::alloc(c);
    let p_box = rt::box_i32(port)?;
    rt::call_reflective("java/net/InetSocketAddress", "<init>", "(Ljava/net/InetAddress;I)V", isa, &[addr, p_box])?;
    Ok(isa)
}

fn to_sockaddr(addr: Ref, port: i32, ipv6_socket: bool, st: &mut Storage) -> Res<u32> {
    let o = rt::nn_ref(addr)?;
    let be_port = (port as u16).to_be_bytes();
    if is_inet6(o) {
        let h6 = rt::nn_ref(rt::get_ref(o, "holder6"))?;
        let ip_arr = rt::nn_ref(rt::get_ref(h6, "ipaddress"))?;
        let scope = rt::get_i32(h6, "scope_id") as u32;
        st.data[0..2].copy_from_slice(&(AF_INET6 as u16).to_ne_bytes());
        st.data[2..4].copy_from_slice(&be_port);
        unsafe {
            st.data[8..24].copy_from_slice(&rt::byte_slice(ip_arr)?[..16]);
        }
        st.data[24..28].copy_from_slice(&scope.to_ne_bytes());
        return Ok(28);
    }
    let holder = rt::nn_ref(rt::get_ref(o, "holder"))?;
    let a = (rt::get_i32(holder, "address") as u32).to_be_bytes();
    if ipv6_socket {
        st.data[0..2].copy_from_slice(&(AF_INET6 as u16).to_ne_bytes());
        st.data[2..4].copy_from_slice(&be_port);
        st.data[18] = 0xff;
        st.data[19] = 0xff;
        st.data[20..24].copy_from_slice(&a);
        return Ok(28);
    }
    st.data[0..2].copy_from_slice(&(AF_INET as u16).to_ne_bytes());
    st.data[2..4].copy_from_slice(&be_port);
    st.data[4..8].copy_from_slice(&a);
    Ok(16)
}

fn from_sockaddr(st: &Storage, port_out: &mut i32) -> Res<Ref> {
    let fam = u16::from_ne_bytes([st.data[0], st.data[1]]) as c_int;
    *port_out = u16::from_be_bytes([st.data[2], st.data[3]]) as i32;
    if fam == AF_INET {
        return inet_from_bytes(&st.data[4..8], None, 0);
    }
    if fam == AF_INET6 {
        let ip = &st.data[8..24];
        if ip[0..10].iter().all(|&b| b == 0) && ip[10] == 0xff && ip[11] == 0xff {
            return inet_from_bytes(&ip[12..16], None, 0);
        }
        let scope = u32::from_ne_bytes([st.data[24], st.data[25], st.data[26], st.data[27]]);
        return inet_from_bytes(ip, None, scope);
    }
    rt::throw("java/io/IOException", "unsupported address family".to_string())
}

// ------------------------------------------------------------------ sun.nio.ch.Net
pub fn n_sun_nio_ch_Net__initIDs() -> Res<()> { Ok(()) }
pub fn n_sun_nio_ch_Net__isIPv6Available0() -> Res<i32> { Ok(ipv6_available() as i32) }
pub fn n_sun_nio_ch_Net__isReusePortAvailable0() -> Res<i32> { Ok(1) }
pub fn n_sun_nio_ch_Net__isExclusiveBindAvailable() -> Res<i32> { Ok(-1) }
pub fn n_sun_nio_ch_Net__shouldShutdownWriteBeforeClose0() -> Res<i32> { Ok(0) }
pub fn n_sun_nio_ch_Net__shouldSetBothIPv4AndIPv6Options0() -> Res<i32> { Ok(1) }
pub fn n_sun_nio_ch_Net__canIPv6SocketJoinIPv4Group0() -> Res<i32> { Ok(1) }
pub fn n_sun_nio_ch_Net__canJoin6WithIPv4Group0() -> Res<i32> { Ok(0) }
pub fn n_sun_nio_ch_Net__canUseIPv6OptionsWithIPv4LocalAddress0() -> Res<i32> { Ok(1) }
pub fn n_sun_nio_ch_Net__pollinValue() -> Res<i32> { Ok(POLLIN as i32) }
pub fn n_sun_nio_ch_Net__polloutValue() -> Res<i32> { Ok(POLLOUT as i32) }
pub fn n_sun_nio_ch_Net__pollerrValue() -> Res<i32> { Ok(POLLERR as i32) }
pub fn n_sun_nio_ch_Net__pollhupValue() -> Res<i32> { Ok(POLLHUP as i32) }
pub fn n_sun_nio_ch_Net__pollnvalValue() -> Res<i32> { Ok(POLLNVAL as i32) }
pub fn n_sun_nio_ch_Net__pollconnValue() -> Res<i32> { Ok(POLLOUT as i32) }

pub fn n_sun_nio_ch_Net__socket0(prefer_ipv6: i32, stream: i32, reuse: i32, _fast_loopback: i32) -> Res<i32> {
    let v6 = prefer_ipv6 != 0 && ipv6_available();
    let ty = (if stream != 0 { SOCK_STREAM } else { SOCK_DGRAM }) | SOCK_CLOEXEC;
    let fd = unsafe { socket(if v6 { AF_INET6 } else { AF_INET }, ty, 0) };
    if fd < 0 {
        return socket_error(errno());
    }
    if v6 {
        let off: c_int = 0;
        unsafe { setsockopt(fd, IPPROTO_IPV6, IPV6_V6ONLY, &off as *const c_int as *const c_void, 4) };
    }
    if reuse != 0 {
        let one: c_int = 1;
        unsafe { setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &one as *const c_int as *const c_void, 4) };
    }
    Ok(fd)
}

pub fn n_sun_nio_ch_Net__bind0(fdo: Ref, prefer_ipv6: i32, _excl: i32, addr: Ref, port: i32) -> Res<()> {
    let fd = rt::fd_of(fdo)?;
    let mut st = Storage::zeroed();
    let len = to_sockaddr(addr, port, prefer_ipv6 != 0 && ipv6_available(), &mut st)?;
    if unsafe { bind(fd, st.data.as_ptr() as *const c_void, len) } < 0 {
        return socket_error(errno());
    }
    Ok(())
}

pub fn n_sun_nio_ch_Net__listen(fdo: Ref, backlog: i32) -> Res<()> {
    let fd = rt::fd_of(fdo)?;
    if unsafe { listen(fd, backlog) } < 0 { socket_error(errno()) } else { Ok(()) }
}

pub fn n_sun_nio_ch_Net__connect0(prefer_ipv6: i32, fdo: Ref, addr: Ref, port: i32) -> Res<i32> {
    let fd = rt::fd_of(fdo)?;
    let mut st = Storage::zeroed();
    let len = to_sockaddr(addr, port, prefer_ipv6 != 0 && ipv6_available(), &mut st)?;
    let (r, e) = {
        let _blk = ScopedBlock::enter();
        let rc = unsafe { connect(fd, st.data.as_ptr() as *const c_void, len) };
        (rc, errno())
    };
    if r < 0 {
        if e == EINPROGRESS { return Ok(IOS_UNAVAILABLE); }
        if e == EINTR { return Ok(IOS_INTERRUPTED); }
        return socket_error(e);
    }
    Ok(1)
}

pub fn n_sun_nio_ch_Net__accept(fdo: Ref, newfdo: Ref, isaa: Ref) -> Res<i32> {
    let fd = rt::fd_of(fdo)?;
    let mut st = Storage::zeroed();
    let mut len = std::mem::size_of::<Storage>() as u32;
    let (nfd, e) = {
        let _blk = ScopedBlock::enter();
        let rc = unsafe { accept4(fd, st.data.as_mut_ptr() as *mut c_void, &mut len, SOCK_CLOEXEC) };
        (rc, errno())
    };
    if nfd < 0 {
        if e == EAGAIN || e == ECONNABORTED { return Ok(IOS_UNAVAILABLE); }
        if e == EINTR { return Ok(IOS_INTERRUPTED); }
        return socket_error(e);
    }
    rt::set_fd(newfdo, nfd)?;
    let mut port = 0i32;
    let addr = from_sockaddr(&st, &mut port)?;
    let isa = new_inet_socket_address(addr, port)?;
    rt::aastore_ref(isaa, 0, isa)?;
    Ok(1)
}

pub fn n_sun_nio_ch_Net__shutdown(fdo: Ref, how: i32) -> Res<()> {
    let fd = rt::fd_of(fdo)?;
    if unsafe { shutdown(fd, how) } < 0 && errno() != ENOTCONN {
        return socket_error(errno());
    }
    Ok(())
}

pub fn n_sun_nio_ch_Net__localPort(fdo: Ref) -> Res<i32> {
    let fd = rt::fd_of(fdo)?;
    let mut st = Storage::zeroed();
    let mut len = std::mem::size_of::<Storage>() as u32;
    if unsafe { getsockname(fd, st.data.as_mut_ptr() as *mut c_void, &mut len) } < 0 {
        return socket_error(errno());
    }
    Ok(u16::from_be_bytes([st.data[2], st.data[3]]) as i32)
}

pub fn n_sun_nio_ch_Net__localInetAddress(fdo: Ref) -> Res<Ref> {
    let fd = rt::fd_of(fdo)?;
    let mut st = Storage::zeroed();
    let mut len = std::mem::size_of::<Storage>() as u32;
    if unsafe { getsockname(fd, st.data.as_mut_ptr() as *mut c_void, &mut len) } < 0 {
        return socket_error(errno());
    }
    let mut port = 0i32;
    from_sockaddr(&st, &mut port)
}

pub fn n_sun_nio_ch_Net__remotePort(fdo: Ref) -> Res<i32> {
    let fd = rt::fd_of(fdo)?;
    let mut st = Storage::zeroed();
    let mut len = std::mem::size_of::<Storage>() as u32;
    if unsafe { getpeername(fd, st.data.as_mut_ptr() as *mut c_void, &mut len) } < 0 {
        return socket_error(errno());
    }
    Ok(u16::from_be_bytes([st.data[2], st.data[3]]) as i32)
}

pub fn n_sun_nio_ch_Net__remoteInetAddress(fdo: Ref) -> Res<Ref> {
    let fd = rt::fd_of(fdo)?;
    let mut st = Storage::zeroed();
    let mut len = std::mem::size_of::<Storage>() as u32;
    if unsafe { getpeername(fd, st.data.as_mut_ptr() as *mut c_void, &mut len) } < 0 {
        return socket_error(errno());
    }
    let mut port = 0i32;
    from_sockaddr(&st, &mut port)
}

pub fn n_sun_nio_ch_Net__getIntOption0(fdo: Ref, _may_conv: i32, level: i32, opt: i32) -> Res<i32> {
    let fd = rt::fd_of(fdo)?;
    if level == SOL_SOCKET && opt == SO_LINGER {
        let mut l = Linger { l_onoff: 0, l_linger: 0 };
        let mut len = std::mem::size_of::<Linger>() as u32;
        if unsafe { getsockopt(fd, level, opt, &mut l as *mut Linger as *mut c_void, &mut len) } < 0 {
            return socket_error(errno());
        }
        return Ok(if l.l_onoff != 0 { l.l_linger } else { -1 });
    }
    if level == IPPROTO_IP && (opt == IP_MULTICAST_TTL || opt == IP_MULTICAST_LOOP) {
        let mut b: u8 = 0;
        let mut len = 1u32;
        if unsafe { getsockopt(fd, level, opt, &mut b as *mut u8 as *mut c_void, &mut len) } < 0 {
            return socket_error(errno());
        }
        return Ok(b as i32);
    }
    let mut v: c_int = 0;
    let mut len = 4u32;
    if unsafe { getsockopt(fd, level, opt, &mut v as *mut c_int as *mut c_void, &mut len) } < 0 {
        return socket_error(errno());
    }
    Ok(v)
}

pub fn n_sun_nio_ch_Net__setIntOption0(fdo: Ref, _may_conv: i32, level: i32, opt: i32, arg: i32, _ipv6: i32) -> Res<()> {
    let fd = rt::fd_of(fdo)?;
    let r = if level == SOL_SOCKET && opt == SO_LINGER {
        let l = Linger { l_onoff: if arg >= 0 { 1 } else { 0 }, l_linger: arg.max(0) };
        unsafe { setsockopt(fd, level, opt, &l as *const Linger as *const c_void, std::mem::size_of::<Linger>() as u32) }
    } else if level == IPPROTO_IP && (opt == IP_MULTICAST_TTL || opt == IP_MULTICAST_LOOP) {
        let b = arg as u8;
        unsafe { setsockopt(fd, level, opt, &b as *const u8 as *const c_void, 1) }
    } else {
        unsafe { setsockopt(fd, level, opt, &arg as *const c_int as *const c_void, 4) }
    };
    if r < 0 { socket_error(errno()) } else { Ok(()) }
}

pub fn n_sun_nio_ch_Net__poll(fdo: Ref, events: i32, timeout: i64) -> Res<i32> {
    let mut p = PollFd { fd: rt::fd_of(fdo)?, events: events as i16, revents: 0 };
    let (r, e) = {
        let _blk = ScopedBlock::enter();
        let rc = unsafe { poll(&mut p, 1, timeout as c_int) };
        (rc, errno())
    };
    if r < 0 {
        if e == EINTR { return Ok(IOS_INTERRUPTED); }
        return io_fail();
    }
    Ok(if r > 0 { p.revents as i32 } else { 0 })
}

pub fn n_sun_nio_ch_Net__pollConnect(fdo: Ref, timeout: i64) -> Res<i32> {
    let fd = rt::fd_of(fdo)?;
    let mut p = PollFd { fd, events: POLLOUT, revents: 0 };
    let (r, e) = {
        let _blk = ScopedBlock::enter();
        let rc = unsafe { poll(&mut p, 1, timeout as c_int) };
        (rc, errno())
    };
    if r < 0 {
        if e == EINTR { return Ok(0); }
        return io_fail();
    }
    if r == 0 {
        return Ok(0);
    }
    let mut err: c_int = 0;
    let mut len = 4u32;
    if unsafe { getsockopt(fd, SOL_SOCKET, SO_ERROR, &mut err as *mut c_int as *mut c_void, &mut len) } < 0 {
        return socket_error(errno());
    }
    if err != 0 {
        return socket_error(err);
    }
    Ok(1)
}

pub fn n_sun_nio_ch_Net__available(fdo: Ref) -> Res<i32> {
    let fd = rt::fd_of(fdo)?;
    let mut n: c_int = 0;
    if unsafe { ioctl(fd, FIONREAD, &mut n) } < 0 { io_fail() } else { Ok(n) }
}

pub fn n_sun_nio_ch_Net__sendOOB(fdo: Ref, b: i32) -> Res<i32> {
    let fd = rt::fd_of(fdo)?;
    let c = [b as u8];
    let n = unsafe { send(fd, c.as_ptr() as *const c_void, 1, MSG_OOB | MSG_NOSIGNAL) };
    convert_net(n, false)
}

pub fn n_sun_nio_ch_Net__discardOOB(_fdo: Ref) -> Res<i32> { Ok(0) }

pub fn n_sun_nio_ch_Net__joinOrDrop4(join: i32, fdo: Ref, group: i32, interface: i32, source: i32) -> Res<i32> {
    let fd = rt::fd_of(fdo)?;
    let r = if source == 0 {
        let m = IpMreq { multiaddr: (group as u32).to_be(), interface: (interface as u32).to_be() };
        let opt = if join != 0 { IP_ADD_MEMBERSHIP } else { IP_DROP_MEMBERSHIP };
        unsafe { setsockopt(fd, IPPROTO_IP, opt, &m as *const IpMreq as *const c_void, std::mem::size_of::<IpMreq>() as u32) }
    } else {
        let m = IpMreqSource {
            multiaddr: (group as u32).to_be(),
            interface: (interface as u32).to_be(),
            sourceaddr: (source as u32).to_be(),
        };
        let opt = if join != 0 { IP_ADD_SOURCE_MEMBERSHIP } else { IP_DROP_SOURCE_MEMBERSHIP };
        unsafe { setsockopt(fd, IPPROTO_IP, opt, &m as *const IpMreqSource as *const c_void, std::mem::size_of::<IpMreqSource>() as u32) }
    };
    if r < 0 {
        if errno() == EOPNOTSUPP { return Ok(IOS_UNSUPPORTED_CASE); }
        return socket_error(errno());
    }
    Ok(0)
}

pub fn n_sun_nio_ch_Net__blockOrUnblock4(block: i32, fdo: Ref, group: i32, interface: i32, source: i32) -> Res<i32> {
    let fd = rt::fd_of(fdo)?;
    let m = IpMreqSource {
        multiaddr: (group as u32).to_be(),
        interface: (interface as u32).to_be(),
        sourceaddr: (source as u32).to_be(),
    };
    let opt = if block != 0 { IP_BLOCK_SOURCE } else { IP_UNBLOCK_SOURCE };
    if unsafe { setsockopt(fd, IPPROTO_IP, opt, &m as *const IpMreqSource as *const c_void, std::mem::size_of::<IpMreqSource>() as u32) } < 0 {
        if errno() == EOPNOTSUPP { return Ok(IOS_UNSUPPORTED_CASE); }
        return socket_error(errno());
    }
    Ok(0)
}

pub fn n_sun_nio_ch_Net__joinOrDrop6(join: i32, fdo: Ref, group: Ref, index: i32, source: Ref) -> Res<i32> {
    if !source.is_null() {
        return Ok(IOS_UNSUPPORTED_CASE);
    }
    let fd = rt::fd_of(fdo)?;
    let mut m = Ipv6Mreq { multiaddr: [0; 16], ifindex: index as u32 };
    unsafe { m.multiaddr.copy_from_slice(&rt::byte_slice(group)?[..16]) };
    let opt = if join != 0 { IPV6_ADD_MEMBERSHIP } else { IPV6_DROP_MEMBERSHIP };
    if unsafe { setsockopt(fd, IPPROTO_IPV6, opt, &m as *const Ipv6Mreq as *const c_void, std::mem::size_of::<Ipv6Mreq>() as u32) } < 0 {
        return socket_error(errno());
    }
    Ok(0)
}

pub fn n_sun_nio_ch_Net__blockOrUnblock6(_block: i32, _fdo: Ref, _group: Ref, _index: i32, _source: Ref) -> Res<i32> {
    Ok(IOS_UNSUPPORTED_CASE)
}

pub fn n_sun_nio_ch_Net__setInterface4(fdo: Ref, interface: i32) -> Res<()> {
    let fd = rt::fd_of(fdo)?;
    let a = (interface as u32).to_be();
    if unsafe { setsockopt(fd, IPPROTO_IP, IP_MULTICAST_IF, &a as *const u32 as *const c_void, 4) } < 0 {
        return socket_error(errno());
    }
    Ok(())
}

pub fn n_sun_nio_ch_Net__getInterface4(fdo: Ref) -> Res<i32> {
    let fd = rt::fd_of(fdo)?;
    let mut a = 0u32;
    let mut len = 4u32;
    if unsafe { getsockopt(fd, IPPROTO_IP, IP_MULTICAST_IF, &mut a as *mut u32 as *mut c_void, &mut len) } < 0 {
        return socket_error(errno());
    }
    Ok(u32::from_be(a) as i32)
}

pub fn n_sun_nio_ch_Net__setInterface6(fdo: Ref, index: i32) -> Res<()> {
    let fd = rt::fd_of(fdo)?;
    if unsafe { setsockopt(fd, IPPROTO_IPV6, IPV6_MULTICAST_IF, &index as *const c_int as *const c_void, 4) } < 0 {
        return socket_error(errno());
    }
    Ok(())
}

pub fn n_sun_nio_ch_Net__getInterface6(fdo: Ref) -> Res<i32> {
    let fd = rt::fd_of(fdo)?;
    let mut v: c_int = 0;
    let mut len = 4u32;
    if unsafe { getsockopt(fd, IPPROTO_IPV6, IPV6_MULTICAST_IF, &mut v as *mut c_int as *mut c_void, &mut len) } < 0 {
        return socket_error(errno());
    }
    Ok(v)
}

// ------------------------------------------------------------------ NativeSocketAddress
pub fn n_sun_nio_ch_NativeSocketAddress__AFINET() -> Res<i32> { Ok(AF_INET) }
pub fn n_sun_nio_ch_NativeSocketAddress__AFINET6() -> Res<i32> { Ok(AF_INET6) }
pub fn n_sun_nio_ch_NativeSocketAddress__sizeofSockAddr4() -> Res<i32> { Ok(16) }
pub fn n_sun_nio_ch_NativeSocketAddress__sizeofSockAddr6() -> Res<i32> { Ok(28) }
pub fn n_sun_nio_ch_NativeSocketAddress__sizeofFamily() -> Res<i32> { Ok(2) }
pub fn n_sun_nio_ch_NativeSocketAddress__offsetFamily() -> Res<i32> { Ok(0) }
pub fn n_sun_nio_ch_NativeSocketAddress__offsetSin4Port() -> Res<i32> { Ok(2) }
pub fn n_sun_nio_ch_NativeSocketAddress__offsetSin4Addr() -> Res<i32> { Ok(4) }
pub fn n_sun_nio_ch_NativeSocketAddress__offsetSin6Port() -> Res<i32> { Ok(2) }
pub fn n_sun_nio_ch_NativeSocketAddress__offsetSin6FlowInfo() -> Res<i32> { Ok(4) }
pub fn n_sun_nio_ch_NativeSocketAddress__offsetSin6Addr() -> Res<i32> { Ok(8) }
pub fn n_sun_nio_ch_NativeSocketAddress__offsetSin6ScopeId() -> Res<i32> { Ok(24) }

// ------------------------------------------------------------------ SocketDispatcher / DatagramDispatcher / DatagramChannelImpl
pub fn n_sun_nio_ch_SocketDispatcher__read0(fdo: Ref, a: i64, len: i32) -> Res<i32> {
    let fd = rt::fd_of(fdo)?;
    let n = {
        let _blk = ScopedBlock::enter();
        unsafe { read(fd, a as usize as *mut c_void, len.max(0) as usize) }
    };
    convert_net(n, true)
}

pub fn n_sun_nio_ch_SocketDispatcher__readv0(fdo: Ref, a: i64, len: i32) -> Res<i64> {
    let fd = rt::fd_of(fdo)?;
    let n = {
        let _blk = ScopedBlock::enter();
        unsafe { readv(fd, a as usize as *const c_void, len) }
    };
    convert_net_long(n, true)
}

pub fn n_sun_nio_ch_SocketDispatcher__write0(fdo: Ref, a: i64, len: i32) -> Res<i32> {
    let fd = rt::fd_of(fdo)?;
    let n = {
        let _blk = ScopedBlock::enter();
        unsafe { send(fd, a as usize as *const c_void, len.max(0) as usize, MSG_NOSIGNAL) }
    };
    convert_net(n, false)
}

pub fn n_sun_nio_ch_SocketDispatcher__writev0(fdo: Ref, a: i64, len: i32) -> Res<i64> {
    let fd = rt::fd_of(fdo)?;
    let n = {
        let _blk = ScopedBlock::enter();
        unsafe { writev(fd, a as usize as *const c_void, len) }
    };
    convert_net_long(n, false)
}

pub fn n_sun_nio_ch_DatagramDispatcher__read0(fdo: Ref, a: i64, len: i32) -> Res<i32> {
    n_sun_nio_ch_SocketDispatcher__read0(fdo, a, len)
}
pub fn n_sun_nio_ch_DatagramDispatcher__readv0(fdo: Ref, a: i64, len: i32) -> Res<i64> {
    n_sun_nio_ch_SocketDispatcher__readv0(fdo, a, len)
}
pub fn n_sun_nio_ch_DatagramDispatcher__write0(fdo: Ref, a: i64, len: i32) -> Res<i32> {
    n_sun_nio_ch_SocketDispatcher__write0(fdo, a, len)
}
pub fn n_sun_nio_ch_DatagramDispatcher__writev0(fdo: Ref, a: i64, len: i32) -> Res<i64> {
    n_sun_nio_ch_SocketDispatcher__writev0(fdo, a, len)
}
pub fn n_sun_nio_ch_DatagramDispatcher__dup0(a: Ref, b: Ref) -> Res<()> {
    if unsafe { dup2(rt::fd_of(a)?, rt::fd_of(b)?) } < 0 { io_fail() } else { Ok(()) }
}

pub fn n_sun_nio_ch_DatagramChannelImpl__disconnect0(fdo: Ref, _ipv6: i32) -> Res<()> {
    let st = Storage::zeroed();
    if unsafe { connect(rt::fd_of(fdo)?, st.data.as_ptr() as *const c_void, 16) } < 0 && errno() != EAFNOSUPPORT {
        return socket_error(errno());
    }
    Ok(())
}

pub fn n_sun_nio_ch_DatagramChannelImpl__receive0(fdo: Ref, buf: i64, len: i32, sender: i64, connected: i32) -> Res<i32> {
    let fd = rt::fd_of(fdo)?;
    let mut alen = 28u32;
    let (n, e) = {
        let _blk = ScopedBlock::enter();
        let rc = unsafe { recvfrom(fd, buf as usize as *mut c_void, len.max(0) as usize, 0, sender as usize as *mut c_void, &mut alen) };
        (rc, errno())
    };
    if n < 0 {
        if e == EAGAIN { return Ok(IOS_UNAVAILABLE); }
        if e == EINTR { return Ok(IOS_INTERRUPTED); }
        if e == ECONNREFUSED && connected != 0 {
            return throw_net("java/net/PortUnreachableException", "ICMP Port Unreachable".to_string());
        }
        return socket_error(e);
    }
    Ok(n as i32)
}

pub fn n_sun_nio_ch_DatagramChannelImpl__send0(fdo: Ref, buf: i64, len: i32, target: i64, target_len: i32) -> Res<i32> {
    let fd = rt::fd_of(fdo)?;
    let dst = if target_len > 0 { target as usize as *const c_void } else { std::ptr::null() };
    let (n, e) = {
        let _blk = ScopedBlock::enter();
        let rc = unsafe { sendto(fd, buf as usize as *const c_void, len.max(0) as usize, 0, dst, target_len.max(0) as u32) };
        (rc, errno())
    };
    if n < 0 {
        if e == EAGAIN { return Ok(IOS_UNAVAILABLE); }
        if e == EINTR { return Ok(IOS_INTERRUPTED); }
        if e == ECONNREFUSED {
            return throw_net("java/net/PortUnreachableException", "ICMP Port Unreachable".to_string());
        }
        return socket_error(e);
    }
    Ok(n as i32)
}

// ------------------------------------------------------------------ UnixDomainSockets
fn un_addr(path_arr: Ref, un: &mut SockUn) -> Res<u32> {
    let bytes = unsafe { rt::byte_slice(path_arr)? };
    if bytes.len() >= un.path.len() {
        return throw_net("java/net/SocketException", "Unix domain path too long".to_string());
    }
    un.family = AF_UNIX as u16;
    un.path = [0u8; 108];
    un.path[..bytes.len()].copy_from_slice(bytes);
    let nul = if !bytes.is_empty() && un.path[0] != 0 { 1 } else { 0 };
    Ok((2 + bytes.len() + nul) as u32)
}

fn un_path(un: &SockUn, len: u32) -> Res<Ref> {
    let mut n = if len > 2 { (len - 2) as usize } else { 0 };
    n = n.min(un.path.len());
    if n > 0 && un.path[0] != 0 {
        if let Some(pos) = un.path[..n].iter().position(|&b| b == 0) {
            n = pos;
        }
    }
    new_bytes(&un.path[..n])
}

pub fn n_sun_nio_ch_UnixDomainSockets__init() -> Res<i32> { Ok(1) }

pub fn n_sun_nio_ch_UnixDomainSockets__socket0() -> Res<i32> {
    let fd = unsafe { socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0) };
    if fd < 0 { socket_error(errno()) } else { Ok(fd) }
}

pub fn n_sun_nio_ch_UnixDomainSockets__bind0(fdo: Ref, path: Ref) -> Res<()> {
    let fd = rt::fd_of(fdo)?;
    let mut un = SockUn { family: 0, path: [0; 108] };
    let len = un_addr(path, &mut un)?;
    if unsafe { bind(fd, &un as *const SockUn as *const c_void, len) } < 0 {
        return socket_error(errno());
    }
    Ok(())
}

pub fn n_sun_nio_ch_UnixDomainSockets__connect0(fdo: Ref, path: Ref) -> Res<i32> {
    let fd = rt::fd_of(fdo)?;
    let mut un = SockUn { family: 0, path: [0; 108] };
    let len = un_addr(path, &mut un)?;
    let (r, e) = {
        let _blk = ScopedBlock::enter();
        let rc = unsafe { connect(fd, &un as *const SockUn as *const c_void, len) };
        (rc, errno())
    };
    if r < 0 {
        if e == EINPROGRESS { return Ok(IOS_UNAVAILABLE); }
        if e == EINTR { return Ok(IOS_INTERRUPTED); }
        return socket_error(e);
    }
    Ok(1)
}

pub fn n_sun_nio_ch_UnixDomainSockets__accept0(fdo: Ref, newfdo: Ref, arr: Ref) -> Res<i32> {
    let fd = rt::fd_of(fdo)?;
    let mut un = SockUn { family: 0, path: [0; 108] };
    let mut len = std::mem::size_of::<SockUn>() as u32;
    let (nfd, e) = {
        let _blk = ScopedBlock::enter();
        let rc = unsafe { accept4(fd, &mut un as *mut SockUn as *mut c_void, &mut len, SOCK_CLOEXEC) };
        (rc, errno())
    };
    if nfd < 0 {
        if e == EAGAIN { return Ok(IOS_UNAVAILABLE); }
        if e == EINTR { return Ok(IOS_INTERRUPTED); }
        return socket_error(e);
    }
    rt::set_fd(newfdo, nfd)?;
    let p = un_path(&un, len)?;
    rt::aastore_ref(arr, 0, p)?;
    Ok(1)
}

pub fn n_sun_nio_ch_UnixDomainSockets__localAddress0(fdo: Ref) -> Res<Ref> {
    let fd = rt::fd_of(fdo)?;
    let mut un = SockUn { family: 0, path: [0; 108] };
    let mut len = std::mem::size_of::<SockUn>() as u32;
    if unsafe { getsockname(fd, &mut un as *mut SockUn as *mut c_void, &mut len) } < 0 {
        return socket_error(errno());
    }
    un_path(&un, len)
}

// ------------------------------------------------------------------ java.net.InetAddress / Impl
pub fn n_java_net_InetAddress__init() -> Res<()> { Ok(()) }
pub fn n_java_net_Inet4Address__init() -> Res<()> { Ok(()) }
pub fn n_java_net_Inet6Address__init() -> Res<()> { Ok(()) }
pub fn n_java_net_InetAddress__isIPv4Available() -> Res<i32> { Ok(1) }
pub fn n_java_net_InetAddress__isIPv6Supported() -> Res<i32> { Ok(ipv6_available() as i32) }

pub fn n_java_net_Inet4AddressImpl__getLocalHostName(_this: Ref) -> Res<Ref> {
    let mut buf = [0u8; 256];
    if unsafe { gethostname(buf.as_mut_ptr() as *mut c_char, buf.len() - 1) } != 0 {
        return Ok(rt::new_str("localhost".to_string()));
    }
    let s = unsafe { CStr::from_ptr(buf.as_ptr() as *const c_char).to_string_lossy().into_owned() };
    Ok(rt::new_str(s))
}

pub fn n_java_net_Inet6AddressImpl__getLocalHostName(this: Ref) -> Res<Ref> {
    n_java_net_Inet4AddressImpl__getLocalHostName(this)
}

unsafe fn load_sa(p: *const c_void) -> Option<Storage> {
    if p.is_null() {
        return None;
    }
    let mut st = Storage::zeroed();
    std::ptr::copy_nonoverlapping(p as *const u8, st.data.as_mut_ptr(), 28);
    Some(st)
}

fn lookup_host(host: Ref, want_v4: bool, want_v6: bool, v6_first: bool) -> Res<Ref> {
    let h_str = rt::str_to_rust(rt::nn_ref(host)?);
    let ch = CString::new(h_str.clone())
        .map_err(|_| rt::Thrown { obj: (rt::hooks().throw_new)("java/net/UnknownHostException", rt::new_str("invalid host".to_string())) })?;
    let hints = AddrInfo {
        ai_flags: 0,
        ai_family: if want_v4 && want_v6 { AF_UNSPEC } else if want_v4 { AF_INET } else { AF_INET6 },
        ai_socktype: SOCK_STREAM,
        ai_protocol: 0,
        ai_addrlen: 0,
        ai_addr: std::ptr::null_mut(),
        ai_canonname: std::ptr::null_mut(),
        ai_next: std::ptr::null_mut(),
    };
    let mut res: *mut AddrInfo = std::ptr::null_mut();
    let rc = {
        let _blk = ScopedBlock::enter();
        unsafe { getaddrinfo(ch.as_ptr(), std::ptr::null(), &hints, &mut res) }
    };
    if rc != 0 {
        let gerr = unsafe { CStr::from_ptr(gai_strerror(rc)).to_string_lossy() };
        return throw_net("java/net/UnknownHostException", format!("{}: {}", h_str, gerr));
    }
    let mut v4: Vec<[u8; 4]> = Vec::new();
    let mut v6: Vec<([u8; 16], u32)> = Vec::new();
    let mut it = res;
    unsafe {
        while !it.is_null() {
            let ai = &*it;
            if let Some(sa) = load_sa(ai.ai_addr) {
                if ai.ai_family == AF_INET && want_v4 {
                    let mut ip = [0u8; 4];
                    ip.copy_from_slice(&sa.data[4..8]);
                    if !v4.contains(&ip) {
                        v4.push(ip);
                    }
                } else if ai.ai_family == AF_INET6 && want_v6 {
                    let mut ip = [0u8; 16];
                    ip.copy_from_slice(&sa.data[8..24]);
                    let scope = u32::from_ne_bytes([sa.data[24], sa.data[25], sa.data[26], sa.data[27]]);
                    if !v6.iter().any(|x| x.0 == ip) {
                        v6.push((ip, scope));
                    }
                }
            }
            it = ai.ai_next;
        }
        freeaddrinfo(res);
    }
    let total = v4.len() + v6.len();
    if total == 0 {
        return throw_net("java/net/UnknownHostException", h_str);
    }
    let ia_cls = rt::find_class("java/net/InetAddress")
        .ok_or_else(|| rt::Thrown { obj: (rt::hooks().throw_new)("java/lang/InternalError", rt::new_str("InetAddress missing".to_string())) })?;
    let out = rt::new_obj_array(ia_cls, total as i32)?;
    let mut k = 0i32;
    let order = [v6_first, !v6_first];
    for is6 in order {
        if is6 {
            for (ip, scope) in &v6 {
                let obj = inet_from_bytes(ip, Some(&h_str), *scope)?;
                rt::aastore_ref(out, k, obj)?;
                k += 1;
            }
        } else {
            for ip in &v4 {
                let obj = inet_from_bytes(ip, Some(&h_str), 0)?;
                rt::aastore_ref(out, k, obj)?;
                k += 1;
            }
        }
    }
    Ok(out)
}

pub fn n_java_net_Inet4AddressImpl__lookupAllHostAddr(_this: Ref, host: Ref) -> Res<Ref> {
    lookup_host(host, true, false, false)
}

pub fn n_java_net_Inet6AddressImpl__lookupAllHostAddr(_this: Ref, host: Ref, characteristics: i32) -> Res<Ref> {
    let c = characteristics as u32;
    let v4 = (c & 1) != 0;
    let v6 = (c & 2) != 0;
    lookup_host(host, v4 || !v6, v6 || !v4, (c & 8) != 0)
}

fn reverse_lookup(addr: Ref) -> Res<Ref> {
    let b = unsafe { rt::byte_slice(addr)? };
    let mut st = Storage::zeroed();
    let len = if b.len() == 4 {
        st.data[0..2].copy_from_slice(&(AF_INET as u16).to_ne_bytes());
        st.data[4..8].copy_from_slice(&b[0..4]);
        16u32
    } else {
        st.data[0..2].copy_from_slice(&(AF_INET6 as u16).to_ne_bytes());
        st.data[8..24].copy_from_slice(&b[0..16]);
        28u32
    };
    let mut host = [0u8; 1025];
    let rc = {
        let _blk = ScopedBlock::enter();
        unsafe { getnameinfo(st.data.as_ptr() as *const c_void, len, host.as_mut_ptr() as *mut c_char, host.len() as u32, std::ptr::null_mut(), 0, NI_NAMEREQD) }
    };
    if rc != 0 {
        return throw_net("java/net/UnknownHostException", String::new());
    }
    let s = unsafe { CStr::from_ptr(host.as_ptr() as *const c_char).to_string_lossy().into_owned() };
    Ok(rt::new_str(s))
}

pub fn n_java_net_Inet4AddressImpl__getHostByAddr(_this: Ref, addr: Ref) -> Res<Ref> { reverse_lookup(addr) }
pub fn n_java_net_Inet6AddressImpl__getHostByAddr(_this: Ref, addr: Ref) -> Res<Ref> { reverse_lookup(addr) }

fn reachable(st: &mut Storage, len: u32, timeout_ms: i32) -> bool {
    let fam = u16::from_ne_bytes([st.data[0], st.data[1]]) as c_int;
    let _blk = ScopedBlock::enter();
    let fd = unsafe { socket(fam, SOCK_STREAM | SOCK_NONBLOCK | SOCK_CLOEXEC, 0) };
    if fd < 0 {
        return false;
    }
    st.data[2..4].copy_from_slice(&7u16.to_be_bytes());
    let ok = if unsafe { connect(fd, st.data.as_ptr() as *const c_void, len) } == 0 || errno() == ECONNREFUSED {
        true
    } else if errno() != EINPROGRESS {
        false
    } else {
        let mut p = PollFd { fd, events: POLLOUT, revents: 0 };
        if unsafe { poll(&mut p, 1, timeout_ms) } <= 0 {
            false
        } else {
            let mut err: c_int = 0;
            let mut l = 4u32;
            unsafe { getsockopt(fd, SOL_SOCKET, SO_ERROR, &mut err as *mut c_int as *mut c_void, &mut l) };
            err == 0 || err == ECONNREFUSED
        }
    };
    unsafe { close(fd) };
    ok
}

pub fn n_java_net_Inet4AddressImpl__isReachable0(_this: Ref, addr: Ref, timeout: i32, _ifaddr: Ref, _ttl: i32) -> Res<i32> {
    let b = unsafe { rt::byte_slice(addr)? };
    let mut st = Storage::zeroed();
    st.data[0..2].copy_from_slice(&(AF_INET as u16).to_ne_bytes());
    st.data[4..8].copy_from_slice(&b[..4]);
    Ok(reachable(&mut st, 16, timeout) as i32)
}

pub fn n_java_net_Inet6AddressImpl__isReachable0(_this: Ref, addr: Ref, scope: i32, timeout: i32, _ifaddr: Ref, _ttl: i32, _ifscope: i32) -> Res<i32> {
    let b = unsafe { rt::byte_slice(addr)? };
    let mut st = Storage::zeroed();
    st.data[0..2].copy_from_slice(&(AF_INET6 as u16).to_ne_bytes());
    st.data[8..24].copy_from_slice(&b[..16]);
    st.data[24..28].copy_from_slice(&(scope as u32).to_ne_bytes());
    Ok(reachable(&mut st, 28, timeout) as i32)
}

// ------------------------------------------------------------------ NetworkInterface & DefaultProxySelector & SdpSupport
pub fn n_java_net_NetworkInterface__init() -> Res<()> { Ok(()) }

enum IfFilter<'a> {
    All,
    Name(&'a str),
    Index(u32),
    Addr(Ref),
}

fn addr_equals(a: Ref, b: Ref) -> bool {
    if a.is_null() || b.is_null() { return false; }
    let ca = unsafe { (*a.0).class };
    let cb = unsafe { (*b.0).class };
    if !std::ptr::eq(ca, cb) { return false; }
    if is_inet6(a) {
        let ha = rt::get_ref(a, "holder6");
        let hb = rt::get_ref(b, "holder6");
        let pa = rt::get_ref(ha, "ipaddress");
        let pb = rt::get_ref(hb, "ipaddress");
        if pa.is_null() || pb.is_null() { return false; }
        unsafe { rt::byte_slice(pa).ok().map(|s| &s[..16]) == rt::byte_slice(pb).ok().map(|s| &s[..16]) }
    } else {
        let ha = rt::get_ref(a, "holder");
        let hb = rt::get_ref(b, "holder");
        rt::get_i32(ha, "address") == rt::get_i32(hb, "address")
    }
}

fn build_interfaces(filter: IfFilter, want_array: bool) -> Res<Ref> {
    let mut head: *mut IfAddrs = std::ptr::null_mut();
    if unsafe { getifaddrs(&mut head) } != 0 {
        return throw_net("java/net/SocketException", err_msg(errno()));
    }
    let mut names: Vec<String> = Vec::new();
    unsafe {
        let mut it = head;
        while !it.is_null() {
            let a = &*it;
            if !a.ifa_name.is_null() {
                let nm = CStr::from_ptr(a.ifa_name).to_string_lossy().into_owned();
                if !names.contains(&nm) {
                    names.push(nm);
                }
            }
            it = a.ifa_next;
        }
    }
    let ni_cls = rt::find_class("java/net/NetworkInterface")
        .ok_or_else(|| rt::Thrown { obj: (rt::hooks().throw_new)("java/lang/InternalError", rt::new_str("NetworkInterface missing".to_string())) })?;
    let ia_cls = rt::find_class("java/net/InterfaceAddress");
    let inet_cls = rt::find_class("java/net/InetAddress").unwrap_or(rt::hooks().object_class);

    let mut nis: Vec<Ref> = Vec::new();
    for nm in &names {
        let Ok(cn) = CString::new(nm.as_str()) else { continue; };
        let idx = unsafe { if_nametoindex(cn.as_ptr()) };
        match filter {
            IfFilter::Name(want) if want != nm => continue,
            IfFilter::Index(want) if want != idx => continue,
            _ => {}
        }
        let mut addrs_vec: Vec<Ref> = Vec::new();
        let mut bind_vec: Vec<Ref> = Vec::new();
        let mut cur = head;
        unsafe {
            while !cur.is_null() {
                let a = &*cur;
                if !a.ifa_name.is_null() && CStr::from_ptr(a.ifa_name).to_string_lossy() == *nm {
                    if let Some(sa) = load_sa(a.ifa_addr) {
                        let fam = u16::from_ne_bytes([sa.data[0], sa.data[1]]) as c_int;
                        if fam == AF_INET || fam == AF_INET6 {
                            let mut port = 0i32;
                            let inet = from_sockaddr(&sa, &mut port)?;
                            addrs_vec.push(inet);
                            if let Some(iac) = ia_cls {
                                let ia = rt::alloc(iac);
                                rt::set_ref(ia, "address", inet);
                                if let Some(mask) = load_sa(a.ifa_netmask) {
                                    let mf = u16::from_ne_bytes([mask.data[0], mask.data[1]]) as c_int;
                                    let bytes = if mf == AF_INET { &mask.data[4..8] } else { &mask.data[8..24] };
                                    let plen: i16 = bytes.iter().map(|b| b.count_ones() as i16).sum();
                                    rt::set_i16(ia, "maskLength", plen);
                                }
                                if fam == AF_INET && (a.ifa_flags & IFF_BROADCAST) != 0 && (a.ifa_flags & IFF_POINTOPOINT) == 0 {
                                    if let Some(b) = load_sa(a.ifa_ifu) {
                                        let bcast = from_sockaddr(&b, &mut port)?;
                                        rt::set_ref(ia, "broadcast", bcast);
                                    }
                                }
                                bind_vec.push(ia);
                            }
                        }
                    }
                }
                cur = a.ifa_next;
            }
        }
        if let IfFilter::Addr(want) = filter {
            if !addrs_vec.iter().any(|&a| addr_equals(a, want)) {
                continue;
            }
        }
        let addrs_arr = rt::new_obj_array(inet_cls, addrs_vec.len() as i32)?;
        for (i, &v) in addrs_vec.iter().enumerate() {
            rt::aastore_ref(addrs_arr, i as i32, v)?;
        }
        let bind_arr = if let Some(iac) = ia_cls {
            let arr = rt::new_obj_array(iac, bind_vec.len() as i32)?;
            for (i, &v) in bind_vec.iter().enumerate() {
                rt::aastore_ref(arr, i as i32, v)?;
            }
            arr
        } else {
            Ref::NULL
        };
        let childs = rt::new_obj_array(ni_cls, 0)?;
        let ni = rt::alloc(ni_cls);
        let jname = rt::new_str(nm.clone());
        rt::set_ref(ni, "name", jname);
        rt::set_ref(ni, "displayName", jname);
        rt::set_i32(ni, "index", idx as i32);
        rt::set_ref(ni, "addrs", addrs_arr);
        rt::set_ref(ni, "bindings", bind_arr);
        rt::set_ref(ni, "childs", childs);
        nis.push(ni);
        if !want_array {
            break;
        }
    }
    unsafe { freeifaddrs(head) };
    if !want_array {
        return Ok(nis.first().copied().unwrap_or(Ref::NULL));
    }
    let out = rt::new_obj_array(ni_cls, nis.len() as i32)?;
    for (i, &v) in nis.iter().enumerate() {
        rt::aastore_ref(out, i as i32, v)?;
    }
    Ok(out)
}

pub fn n_java_net_NetworkInterface__getAll() -> Res<Ref> {
    build_interfaces(IfFilter::All, true)
}
pub fn n_java_net_NetworkInterface__getByName0(name: Ref) -> Res<Ref> {
    let n = rt::str_to_rust(rt::nn_ref(name)?);
    build_interfaces(IfFilter::Name(&n), false)
}
pub fn n_java_net_NetworkInterface__getByIndex0(idx: i32) -> Res<Ref> {
    build_interfaces(IfFilter::Index(idx as u32), false)
}
pub fn n_java_net_NetworkInterface__getByInetAddress0(addr: Ref) -> Res<Ref> {
    build_interfaces(IfFilter::Addr(addr), false)
}
pub fn n_java_net_NetworkInterface__boundInetAddress0(addr: Ref) -> Res<i32> {
    Ok(!build_interfaces(IfFilter::Addr(addr), false)?.is_null() as i32)
}

fn flag_of(name: Ref, bit: u32) -> Res<i32> {
    let n = rt::str_to_rust(rt::nn_ref(name)?);
    let mut head: *mut IfAddrs = std::ptr::null_mut();
    if unsafe { getifaddrs(&mut head) } != 0 {
        return Ok(0);
    }
    let mut res = 0i32;
    unsafe {
        let mut it = head;
        while !it.is_null() {
            let a = &*it;
            if !a.ifa_name.is_null() && CStr::from_ptr(a.ifa_name).to_string_lossy() == n {
                res = ((a.ifa_flags & bit) != 0) as i32;
                break;
            }
            it = a.ifa_next;
        }
        freeifaddrs(head);
    }
    Ok(res)
}

pub fn n_java_net_NetworkInterface__isUp0(name: Ref, _index: i32) -> Res<i32> { flag_of(name, IFF_UP) }
pub fn n_java_net_NetworkInterface__isLoopback0(name: Ref, _index: i32) -> Res<i32> { flag_of(name, IFF_LOOPBACK) }
pub fn n_java_net_NetworkInterface__supportsMulticast0(name: Ref, _index: i32) -> Res<i32> { flag_of(name, IFF_MULTICAST) }
pub fn n_java_net_NetworkInterface__isP2P0(name: Ref, _index: i32) -> Res<i32> { flag_of(name, IFF_POINTOPOINT) }

pub fn n_java_net_NetworkInterface__getMTU0(name: Ref, _index: i32) -> Res<i32> {
    let n = rt::str_to_rust(rt::nn_ref(name)?);
    let p = format!("/sys/class/net/{}/mtu", n);
    Ok(std::fs::read_to_string(p).ok().and_then(|s| s.trim().parse::<i32>().ok()).unwrap_or(-1))
}

pub fn n_java_net_NetworkInterface__getMacAddr0(_addr: Ref, name: Ref, _index: i32) -> Res<Ref> {
    let n = rt::str_to_rust(rt::nn_ref(name)?);
    let p = format!("/sys/class/net/{}/address", n);
    let Ok(s) = std::fs::read_to_string(p) else { return Ok(Ref::NULL); };
    let mut mac = [0u8; 6];
    let mut i = 0usize;
    for part in s.trim().split(':') {
        if i >= 6 { return Ok(Ref::NULL); }
        let Ok(b) = u8::from_str_radix(part, 16) else { return Ok(Ref::NULL); };
        mac[i] = b;
        i += 1;
    }
    if i != 6 || mac.iter().all(|&b| b == 0) {
        return Ok(Ref::NULL);
    }
    new_bytes(&mac)
}

pub fn n_sun_net_spi_DefaultProxySelector__init() -> Res<i32> { Ok(0) }
pub fn n_sun_net_spi_DefaultProxySelector__getSystemProxies(_this: Ref, _s: Ref, _h: Ref) -> Res<Ref> { Ok(Ref::NULL) }
pub fn n_sun_net_sdp_SdpSupport__convert0(_fd: i32) -> Res<()> {
    rt::throw("java/io/IOException", "SDP unsupported".to_string())
}
