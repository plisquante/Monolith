//! Netty native transport (epoll + unix domain/TCP/UDP sockets + BSD kqueue guard),
//! JNA (`com.sun.jna.Native` over dlopen/dlsym/libffi), Tracy profiler bindings,
//! and NarratorLinux (`libflite.so.1` via dlopen).
use crate::gc::ScopedBlock;
use crate::rt::{self, Arr, Ref, Res};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_long, c_uint, c_ulong, c_void};
use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

#[cfg(target_arch = "x86_64")]
#[repr(C, packed)]
struct EpollEvent {
    events: u32,
    data: u64,
}

#[cfg(not(target_arch = "x86_64"))]
#[repr(C)]
struct EpollEvent {
    events: u32,
    data: u64,
}

#[repr(C)]
struct SockAddrIn {
    sin_family: u16,
    sin_port: u16,
    sin_addr: u32,
    sin_zero: [u8; 8],
}

#[repr(C)]
struct SockAddrIn6 {
    sin6_family: u16,
    sin6_port: u16,
    sin6_flowinfo: u32,
    sin6_addr: [u8; 16],
    sin6_scope_id: u32,
}

#[repr(C)]
struct SockAddrUn {
    sun_family: u16,
    sun_path: [u8; 108],
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
struct UtsName {
    sysname: [c_char; 65],
    nodename: [c_char; 65],
    release: [c_char; 65],
    version: [c_char; 65],
    machine: [c_char; 65],
    domainname: [c_char; 65],
}

#[repr(C)]
struct ITimerSpec {
    it_interval_sec: i64,
    it_interval_nsec: i64,
    it_value_sec: i64,
    it_value_nsec: i64,
}

#[link(name = "c")]
extern "C" {
    fn __errno_location() -> *mut c_int;
    fn strerror(errnum: c_int) -> *const c_char;
    fn malloc(size: usize) -> *mut c_void;
    fn free(ptr: *mut c_void);
    fn open(pathname: *const c_char, flags: c_int, ...) -> c_int;
    fn close(fd: c_int) -> c_int;
    fn write(fd: c_int, buf: *const c_void, count: usize) -> isize;
    fn writev(fd: c_int, iov: *const c_void, iovcnt: c_int) -> isize;
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
    fn send(sockfd: c_int, buf: *const c_void, len: usize, flags: c_int) -> isize;
    fn recv(sockfd: c_int, buf: *mut c_void, len: usize, flags: c_int) -> isize;
    fn sendto(sockfd: c_int, buf: *const c_void, len: usize, flags: c_int, dest_addr: *const c_void, addrlen: u32) -> isize;
    fn sendfile(out_fd: c_int, in_fd: c_int, offset: *mut i64, count: usize) -> isize;
    fn epoll_create1(flags: c_int) -> c_int;
    fn epoll_ctl(epfd: c_int, op: c_int, fd: c_int, event: *const EpollEvent) -> c_int;
    fn epoll_wait(epfd: c_int, events: *mut EpollEvent, maxevents: c_int, timeout: c_int) -> c_int;
    fn eventfd(initval: c_uint, flags: c_int) -> c_int;
    fn timerfd_create(clockid: c_int, flags: c_int) -> c_int;
    fn timerfd_settime(fd: c_int, flags: c_int, new_val: *const ITimerSpec, old_val: *mut ITimerSpec) -> c_int;
    fn splice(fd_in: c_int, off_in: *mut i64, fd_out: c_int, off_out: *mut i64, len: usize, flags: c_uint) -> isize;
    fn uname(buf: *mut UtsName) -> c_int;
}

#[link(name = "dl")]
extern "C" {
    fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
    fn dlerror() -> *const c_char;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlclose(handle: *mut c_void) -> c_int;
}

#[link(name = "ffi")]
extern "C" {
    static mut ffi_type_void: u8;
    static mut ffi_type_sint8: u8;
    static mut ffi_type_uint16: u8;
    static mut ffi_type_sint16: u8;
    static mut ffi_type_sint32: u8;
    static mut ffi_type_sint64: u8;
    static mut ffi_type_float: u8;
    static mut ffi_type_double: u8;
    static mut ffi_type_pointer: u8;
    fn ffi_prep_cif(cif: *mut c_void, abi: c_int, nargs: c_uint, rtype: *mut c_void, atypes: *mut c_void) -> c_int;
    fn ffi_call(cif: *mut c_void, fn_ptr: *const c_void, rvalue: *mut c_void, avalue: *mut *mut c_void);
}

const AF_UNIX: c_int = 1;
const AF_INET: c_int = 2;
const AF_INET6: c_int = 10;
const SOCK_STREAM: c_int = 1;
const SOCK_DGRAM: c_int = 2;
const SOCK_NONBLOCK: c_int = 0o4000;
const SOCK_CLOEXEC: c_int = 0o2000000;
const SOL_SOCKET: c_int = 1;
const SOL_TCP: c_int = 6;
const IPPROTO_IP: c_int = 0;
const IPPROTO_IPV6: c_int = 41;

#[inline(always)]
fn errno() -> i32 {
    unsafe { *__errno_location() }
}

fn new_bytes(b: &[u8]) -> Res<Ref> {
    let a = rt::new_array(b'B', b.len() as i32)?;
    unsafe { rt::byte_slice_mut(a)?.copy_from_slice(b) };
    Ok(a)
}

fn netty_sockaddr(ipv6: bool, addr_arr: Ref, scope_id: i32, port: i32, buf: &mut [u8; 128]) -> Res<u32> {
    let b = unsafe { rt::byte_slice(addr_arr)? };
    if ipv6 || b.len() == 16 {
        let mut ip = [0u8; 16];
        if b.len() == 4 {
            ip[10] = 0xff;
            ip[11] = 0xff;
            ip[12..16].copy_from_slice(b);
        } else {
            ip.copy_from_slice(&b[..16]);
        }
        let sa = SockAddrIn6 {
            sin6_family: AF_INET6 as u16,
            sin6_port: (port as u16).to_be(),
            sin6_flowinfo: 0,
            sin6_addr: ip,
            sin6_scope_id: scope_id as u32,
        };
        let sz = std::mem::size_of::<SockAddrIn6>();
        unsafe { std::ptr::copy_nonoverlapping(&sa as *const SockAddrIn6 as *const u8, buf.as_mut_ptr(), sz) };
        Ok(sz as u32)
    } else {
        let ip = u32::from_ne_bytes([b[0], b[1], b[2], b[3]]);
        let sa = SockAddrIn {
            sin_family: AF_INET as u16,
            sin_port: (port as u16).to_be(),
            sin_addr: ip,
            sin_zero: [0; 8],
        };
        let sz = std::mem::size_of::<SockAddrIn>();
        unsafe { std::ptr::copy_nonoverlapping(&sa as *const SockAddrIn as *const u8, buf.as_mut_ptr(), sz) };
        Ok(sz as u32)
    }
}

fn netty_unpack_addr(buf: &[u8; 128]) -> Res<Ref> {
    let fam = u16::from_ne_bytes([buf[0], buf[1]]) as c_int;
    if fam == AF_INET {
        let mut out = [0u8; 8];
        out[0] = 4;
        out[1..5].copy_from_slice(&buf[4..8]);
        out[5] = buf[2];
        out[6] = buf[3];
        new_bytes(&out)
    } else if fam == AF_INET6 {
        let mut out = [0u8; 24];
        out[0] = 16;
        out[1..17].copy_from_slice(&buf[8..24]);
        out[17..21].copy_from_slice(&buf[24..28]);
        out[21] = buf[2];
        out[22] = buf[3];
        new_bytes(&out)
    } else {
        Ok(Ref::NULL)
    }
}

// ------------------------------------------------------------------ io.netty.channel.unix.*
pub fn n_io_netty_channel_unix_Buffer__addressSize0() -> Res<i32> {
    Ok(std::mem::size_of::<usize>() as i32)
}
pub fn n_io_netty_channel_unix_Buffer__memoryAddress0(buf: Ref) -> Res<i64> {
    Ok(rt::get_i64(rt::nn_ref(buf)?, "address"))
}

pub fn n_io_netty_channel_unix_LimitsStaticallyReferencedJniMethods__iovMax() -> Res<i32> { Ok(1024) }
pub fn n_io_netty_channel_unix_LimitsStaticallyReferencedJniMethods__uioMaxIov() -> Res<i32> { Ok(1024) }
pub fn n_io_netty_channel_unix_LimitsStaticallyReferencedJniMethods__sizeOfjlong() -> Res<i32> { Ok(8) }
pub fn n_io_netty_channel_unix_LimitsStaticallyReferencedJniMethods__ssizeMax() -> Res<i64> { Ok(isize::MAX as i64) }

pub fn n_io_netty_channel_unix_ErrorsStaticallyReferencedJniMethods__errnoEAGAIN() -> Res<i32> { Ok(11) }
pub fn n_io_netty_channel_unix_ErrorsStaticallyReferencedJniMethods__errnoEBADF() -> Res<i32> { Ok(9) }
pub fn n_io_netty_channel_unix_ErrorsStaticallyReferencedJniMethods__errnoECONNRESET() -> Res<i32> { Ok(104) }
pub fn n_io_netty_channel_unix_ErrorsStaticallyReferencedJniMethods__errnoEINPROGRESS() -> Res<i32> { Ok(115) }
pub fn n_io_netty_channel_unix_ErrorsStaticallyReferencedJniMethods__errnoENOENT() -> Res<i32> { Ok(2) }
pub fn n_io_netty_channel_unix_ErrorsStaticallyReferencedJniMethods__errnoENOTCONN() -> Res<i32> { Ok(107) }
pub fn n_io_netty_channel_unix_ErrorsStaticallyReferencedJniMethods__errnoEPIPE() -> Res<i32> { Ok(32) }
pub fn n_io_netty_channel_unix_ErrorsStaticallyReferencedJniMethods__errnoEWOULDBLOCK() -> Res<i32> { Ok(11) }
pub fn n_io_netty_channel_unix_ErrorsStaticallyReferencedJniMethods__errorEALREADY() -> Res<i32> { Ok(114) }
pub fn n_io_netty_channel_unix_ErrorsStaticallyReferencedJniMethods__errorECONNREFUSED() -> Res<i32> { Ok(111) }
pub fn n_io_netty_channel_unix_ErrorsStaticallyReferencedJniMethods__errorEHOSTUNREACH() -> Res<i32> { Ok(113) }
pub fn n_io_netty_channel_unix_ErrorsStaticallyReferencedJniMethods__errorEISCONN() -> Res<i32> { Ok(106) }
pub fn n_io_netty_channel_unix_ErrorsStaticallyReferencedJniMethods__errorENETUNREACH() -> Res<i32> { Ok(101) }
pub fn n_io_netty_channel_unix_ErrorsStaticallyReferencedJniMethods__strError(err: i32) -> Res<Ref> {
    let msg = unsafe { CStr::from_ptr(strerror(err)).to_string_lossy().into_owned() };
    Ok(rt::new_str(msg))
}

pub fn n_io_netty_channel_unix_FileDescriptor__open(path: Ref) -> Res<i32> {
    let cp = rt::c_str_arg(path)?;
    let r = unsafe { open(cp.as_ptr(), 0o2000000) };
    Ok(if r < 0 { -errno() } else { r })
}
pub fn n_io_netty_channel_unix_FileDescriptor__close(fd: i32) -> Res<i32> {
    Ok(if unsafe { close(fd) } < 0 { -errno() } else { 0 })
}
pub fn n_io_netty_channel_unix_FileDescriptor__write(fd: i32, buf: Ref, pos: i32, lim: i32) -> Res<i32> {
    let base = rt::get_i64(rt::nn_ref(buf)?, "address") as usize;
    let n = unsafe { write(fd, (base + pos as usize) as *const c_void, (lim - pos).max(0) as usize) };
    Ok(if n < 0 { -errno() } else { n as i32 })
}
pub fn n_io_netty_channel_unix_FileDescriptor__writevAddresses(fd: i32, iov: i64, cnt: i32) -> Res<i64> {
    let n = unsafe { writev(fd, iov as usize as *const c_void, cnt) };
    Ok(if n < 0 { -(errno() as i64) } else { n as i64 })
}
pub fn n_io_netty_channel_unix_FileDescriptor__writev(fd: i32, bufs: Ref, off: i32, len: i32, _max: i64) -> Res<i64> {
    let mut iovs = Vec::with_capacity(len.max(0) as usize);
    for i in 0..len {
        let b = rt::aload::<Ref>(bufs, off + i)?;
        let p = rt::get_i32(b, "position");
        let l = rt::get_i32(b, "limit");
        let addr = rt::get_i64(b, "address") as usize + p as usize;
        iovs.push([addr, (l - p).max(0) as usize]);
    }
    let n = unsafe { writev(fd, iovs.as_ptr() as *const c_void, iovs.len() as c_int) };
    Ok(if n < 0 { -(errno() as i64) } else { n as i64 })
}

fn getsock_i32(fd: i32, level: c_int, opt: c_int) -> Res<i32> {
    let mut v: c_int = 0;
    let mut len = 4u32;
    if unsafe { getsockopt(fd, level, opt, &mut v as *mut c_int as *mut c_void, &mut len) } < 0 {
        return rt::throw("java/io/IOException", unsafe { CStr::from_ptr(strerror(errno())).to_string_lossy().into_owned() });
    }
    Ok(v)
}

fn setsock_i32(fd: i32, level: c_int, opt: c_int, v: i32) -> Res<()> {
    if unsafe { setsockopt(fd, level, opt, &v as *const c_int as *const c_void, 4) } < 0 {
        return rt::throw("java/io/IOException", unsafe { CStr::from_ptr(strerror(errno())).to_string_lossy().into_owned() });
    }
    Ok(())
}

pub fn n_io_netty_channel_unix_Socket__newSocketStreamFd(ipv6: i32) -> Res<i32> {
    let fd = unsafe { socket(if ipv6 != 0 { AF_INET6 } else { AF_INET }, SOCK_STREAM | SOCK_NONBLOCK | SOCK_CLOEXEC, 0) };
    Ok(if fd < 0 { -errno() } else { fd })
}
pub fn n_io_netty_channel_unix_Socket__newSocketDgramFd(ipv6: i32) -> Res<i32> {
    let fd = unsafe { socket(if ipv6 != 0 { AF_INET6 } else { AF_INET }, SOCK_DGRAM | SOCK_NONBLOCK | SOCK_CLOEXEC, 0) };
    Ok(if fd < 0 { -errno() } else { fd })
}
pub fn n_io_netty_channel_unix_Socket__bind(fd: i32, ipv6: i32, addr: Ref, scope: i32, port: i32) -> Res<i32> {
    let mut buf = [0u8; 128];
    let len = netty_sockaddr(ipv6 != 0, addr, scope, port, &mut buf)?;
    Ok(if unsafe { bind(fd, buf.as_ptr() as *const c_void, len) } < 0 { -errno() } else { 0 })
}
pub fn n_io_netty_channel_unix_Socket__bindDomainSocket(fd: i32, path: Ref) -> Res<i32> {
    let p = unsafe { rt::byte_slice(path)? };
    let mut sa = SockAddrUn { sun_family: AF_UNIX as u16, sun_path: [0; 108] };
    let n = p.len().min(107);
    sa.sun_path[..n].copy_from_slice(&p[..n]);
    Ok(if unsafe { bind(fd, &sa as *const SockAddrUn as *const c_void, (2 + n + 1) as u32) } < 0 { -errno() } else { 0 })
}
pub fn n_io_netty_channel_unix_Socket__listen(fd: i32, backlog: i32) -> Res<i32> {
    Ok(if unsafe { listen(fd, backlog) } < 0 { -errno() } else { 0 })
}
pub fn n_io_netty_channel_unix_Socket__connect(fd: i32, ipv6: i32, addr: Ref, scope: i32, port: i32) -> Res<i32> {
    let mut buf = [0u8; 128];
    let len = netty_sockaddr(ipv6 != 0, addr, scope, port, &mut buf)?;
    Ok(if unsafe { connect(fd, buf.as_ptr() as *const c_void, len) } < 0 { -errno() } else { 0 })
}
pub fn n_io_netty_channel_unix_Socket__connectDomainSocket(fd: i32, path: Ref) -> Res<i32> {
    let p = unsafe { rt::byte_slice(path)? };
    let mut sa = SockAddrUn { sun_family: AF_UNIX as u16, sun_path: [0; 108] };
    let n = p.len().min(107);
    sa.sun_path[..n].copy_from_slice(&p[..n]);
    Ok(if unsafe { connect(fd, &sa as *const SockAddrUn as *const c_void, (2 + n + 1) as u32) } < 0 { -errno() } else { 0 })
}
pub fn n_io_netty_channel_unix_Socket__finishConnect(fd: i32) -> Res<i32> {
    let mut err: c_int = 0;
    let mut len = 4u32;
    if unsafe { getsockopt(fd, SOL_SOCKET, 4, &mut err as *mut c_int as *mut c_void, &mut len) } < 0 {
        return Ok(-errno());
    }
    Ok(if err != 0 { -err } else { 0 })
}
pub fn n_io_netty_channel_unix_Socket__disconnect(fd: i32, _ipv6: i32) -> Res<i32> {
    let sa = [0u8; 16];
    let r = unsafe { connect(fd, sa.as_ptr() as *const c_void, 16) };
    Ok(if r < 0 && errno() != 97 { -errno() } else { 0 })
}
pub fn n_io_netty_channel_unix_Socket__accept(fd: i32, accepted_addr: Ref) -> Res<i32> {
    let mut buf = [0u8; 128];
    let mut len = 128u32;
    let nfd = unsafe { accept4(fd, buf.as_mut_ptr() as *mut c_void, &mut len, SOCK_NONBLOCK | SOCK_CLOEXEC) };
    if nfd < 0 {
        return Ok(-errno());
    }
    if !accepted_addr.is_null() {
        if let Ok(dst) = unsafe { rt::byte_slice_mut(accepted_addr) } {
            if let Ok(packed) = netty_unpack_addr(&buf) {
                if !packed.is_null() {
                    if let Ok(src) = unsafe { rt::byte_slice(packed) } {
                        let k = dst.len().min(src.len());
                        dst[..k].copy_from_slice(&src[..k]);
                    }
                }
            }
        }
    }
    Ok(nfd)
}
pub fn n_io_netty_channel_unix_Socket__shutdown(fd: i32, rd: i32, wr: i32) -> Res<i32> {
    let how = if rd != 0 && wr != 0 { 2 } else if wr != 0 { 1 } else { 0 };
    Ok(if unsafe { shutdown(fd, how) } < 0 { -errno() } else { 0 })
}
pub fn n_io_netty_channel_unix_Socket__localAddress(fd: i32) -> Res<Ref> {
    let mut buf = [0u8; 128];
    let mut len = 128u32;
    if unsafe { getsockname(fd, buf.as_mut_ptr() as *mut c_void, &mut len) } < 0 {
        return Ok(Ref::NULL);
    }
    netty_unpack_addr(&buf)
}
pub fn n_io_netty_channel_unix_Socket__remoteAddress(fd: i32) -> Res<Ref> {
    let mut buf = [0u8; 128];
    let mut len = 128u32;
    if unsafe { getpeername(fd, buf.as_mut_ptr() as *mut c_void, &mut len) } < 0 {
        return Ok(Ref::NULL);
    }
    netty_unpack_addr(&buf)
}
pub fn n_io_netty_channel_unix_Socket__send(fd: i32, buf: Ref, pos: i32, lim: i32) -> Res<i32> {
    let base = rt::get_i64(rt::nn_ref(buf)?, "address") as usize;
    let n = unsafe { send(fd, (base + pos as usize) as *const c_void, (lim - pos).max(0) as usize, 0x4000) };
    Ok(if n < 0 { -errno() } else { n as i32 })
}
pub fn n_io_netty_channel_unix_Socket__sendAddress(fd: i32, addr: i64, pos: i32, lim: i32) -> Res<i32> {
    let n = unsafe { send(fd, (addr as usize + pos as usize) as *const c_void, (lim - pos).max(0) as usize, 0x4000) };
    Ok(if n < 0 { -errno() } else { n as i32 })
}
pub fn n_io_netty_channel_unix_Socket__recv(fd: i32, buf: Ref, pos: i32, lim: i32) -> Res<i32> {
    let base = rt::get_i64(rt::nn_ref(buf)?, "address") as usize;
    let n = unsafe { recv(fd, (base + pos as usize) as *mut c_void, (lim - pos).max(0) as usize, 0) };
    Ok(if n < 0 { -errno() } else { n as i32 })
}
pub fn n_io_netty_channel_unix_Socket__recvAddress(fd: i32, addr: i64, pos: i32, lim: i32) -> Res<i32> {
    let n = unsafe { recv(fd, (addr as usize + pos as usize) as *mut c_void, (lim - pos).max(0) as usize, 0) };
    Ok(if n < 0 { -errno() } else { n as i32 })
}
pub fn n_io_netty_channel_unix_Socket__sendTo(fd: i32, ipv6: i32, buf: Ref, pos: i32, lim: i32, addr: Ref, scope: i32, port: i32, _flags: i32) -> Res<i32> {
    let base = rt::get_i64(rt::nn_ref(buf)?, "address") as usize;
    n_io_netty_channel_unix_Socket__sendToAddress(fd, ipv6, base as i64, pos, lim, addr, scope, port, 0)
}
pub fn n_io_netty_channel_unix_Socket__sendToAddress(fd: i32, ipv6: i32, mem: i64, pos: i32, lim: i32, addr: Ref, scope: i32, port: i32, _flags: i32) -> Res<i32> {
    let mut sa = [0u8; 128];
    let len = netty_sockaddr(ipv6 != 0, addr, scope, port, &mut sa)?;
    let n = unsafe { sendto(fd, (mem as usize + pos as usize) as *const c_void, (lim - pos).max(0) as usize, 0x4000, sa.as_ptr() as *const c_void, len) };
    Ok(if n < 0 { -errno() } else { n as i32 })
}
pub fn n_io_netty_channel_unix_Socket__sendToAddresses(fd: i32, ipv6: i32, mem: i64, len_cnt: i32, addr: Ref, scope: i32, port: i32, _flags: i32) -> Res<i32> {
    if len_cnt <= 0 { return Ok(0); }
    let iov0 = unsafe { std::slice::from_raw_parts(mem as usize as *const usize, 2) };
    let mut sa = [0u8; 128];
    let slen = netty_sockaddr(ipv6 != 0, addr, scope, port, &mut sa)?;
    let n = unsafe { sendto(fd, iov0[0] as *const c_void, iov0[1], 0x4000, sa.as_ptr() as *const c_void, slen) };
    Ok(if n < 0 { -errno() } else { n as i32 })
}
pub fn n_io_netty_channel_unix_Socket__getIntOpt(fd: i32, level: i32, opt: i32) -> Res<i32> { getsock_i32(fd, level, opt) }
pub fn n_io_netty_channel_unix_Socket__setIntOpt(fd: i32, level: i32, opt: i32, v: i32) -> Res<()> { setsock_i32(fd, level, opt, v) }
pub fn n_io_netty_channel_unix_Socket__getRawOptArray(fd: i32, level: i32, opt: i32, out: Ref, off: i32, len: i32) -> Res<()> {
    let s = unsafe { rt::byte_slice_mut(out)? };
    let mut l = len as u32;
    unsafe { getsockopt(fd, level, opt, s.as_mut_ptr().add(off as usize) as *mut c_void, &mut l) };
    Ok(())
}
pub fn n_io_netty_channel_unix_Socket__setRawOptArray(fd: i32, level: i32, opt: i32, arr: Ref, off: i32, len: i32) -> Res<()> {
    let s = unsafe { rt::byte_slice(arr)? };
    unsafe { setsockopt(fd, level, opt, s.as_ptr().add(off as usize) as *const c_void, len as u32) };
    Ok(())
}
pub fn n_io_netty_channel_unix_Socket__getRawOptAddress(fd: i32, level: i32, opt: i32, addr: i64, len: i32) -> Res<()> {
    let mut l = len as u32;
    unsafe { getsockopt(fd, level, opt, addr as usize as *mut c_void, &mut l) };
    Ok(())
}
pub fn n_io_netty_channel_unix_Socket__setRawOptAddress(fd: i32, level: i32, opt: i32, addr: i64, len: i32) -> Res<()> {
    unsafe { setsockopt(fd, level, opt, addr as usize as *const c_void, len as u32) };
    Ok(())
}
pub fn n_io_netty_channel_unix_Socket__getReceiveBufferSize(fd: i32) -> Res<i32> { getsock_i32(fd, SOL_SOCKET, 8) }
pub fn n_io_netty_channel_unix_Socket__setReceiveBufferSize(fd: i32, v: i32) -> Res<()> { setsock_i32(fd, SOL_SOCKET, 8, v) }
pub fn n_io_netty_channel_unix_Socket__getSendBufferSize(fd: i32) -> Res<i32> { getsock_i32(fd, SOL_SOCKET, 7) }
pub fn n_io_netty_channel_unix_Socket__setSendBufferSize(fd: i32, v: i32) -> Res<()> { setsock_i32(fd, SOL_SOCKET, 7, v) }
pub fn n_io_netty_channel_unix_Socket__getSoError(fd: i32) -> Res<i32> { getsock_i32(fd, SOL_SOCKET, 4) }
pub fn n_io_netty_channel_unix_Socket__getSoLinger(fd: i32) -> Res<i32> {
    let mut l = Linger { l_onoff: 0, l_linger: 0 };
    let mut len = 8u32;
    if unsafe { getsockopt(fd, SOL_SOCKET, 13, &mut l as *mut Linger as *mut c_void, &mut len) } < 0 { return Ok(-1); }
    Ok(if l.l_onoff != 0 { l.l_linger } else { -1 })
}
pub fn n_io_netty_channel_unix_Socket__setSoLinger(fd: i32, v: i32) -> Res<()> {
    let l = Linger { l_onoff: (v >= 0) as c_int, l_linger: v.max(0) };
    unsafe { setsockopt(fd, SOL_SOCKET, 13, &l as *const Linger as *const c_void, 8) };
    Ok(())
}
pub fn n_io_netty_channel_unix_Socket__getTrafficClass(fd: i32, ipv6: i32) -> Res<i32> {
    getsock_i32(fd, if ipv6 != 0 { IPPROTO_IPV6 } else { IPPROTO_IP }, if ipv6 != 0 { 67 } else { 1 })
}
pub fn n_io_netty_channel_unix_Socket__setTrafficClass(fd: i32, ipv6: i32, v: i32) -> Res<()> {
    setsock_i32(fd, if ipv6 != 0 { IPPROTO_IPV6 } else { IPPROTO_IP }, if ipv6 != 0 { 67 } else { 1 }, v)
}
pub fn n_io_netty_channel_unix_Socket__isBroadcast(fd: i32) -> Res<i32> { getsock_i32(fd, SOL_SOCKET, 6) }
pub fn n_io_netty_channel_unix_Socket__setBroadcast(fd: i32, v: i32) -> Res<()> { setsock_i32(fd, SOL_SOCKET, 6, v) }
pub fn n_io_netty_channel_unix_Socket__isKeepAlive(fd: i32) -> Res<i32> { getsock_i32(fd, SOL_SOCKET, 9) }
pub fn n_io_netty_channel_unix_Socket__setKeepAlive(fd: i32, v: i32) -> Res<()> { setsock_i32(fd, SOL_SOCKET, 9, v) }
pub fn n_io_netty_channel_unix_Socket__isReuseAddress(fd: i32) -> Res<i32> { getsock_i32(fd, SOL_SOCKET, 2) }
pub fn n_io_netty_channel_unix_Socket__setReuseAddress(fd: i32, v: i32) -> Res<()> { setsock_i32(fd, SOL_SOCKET, 2, v) }
pub fn n_io_netty_channel_unix_Socket__isReusePort(fd: i32) -> Res<i32> { getsock_i32(fd, SOL_SOCKET, 15) }
pub fn n_io_netty_channel_unix_Socket__setReusePort(fd: i32, v: i32) -> Res<()> { setsock_i32(fd, SOL_SOCKET, 15, v) }
pub fn n_io_netty_channel_unix_Socket__isTcpNoDelay(fd: i32) -> Res<i32> { getsock_i32(fd, SOL_TCP, 1) }
pub fn n_io_netty_channel_unix_Socket__setTcpNoDelay(fd: i32, v: i32) -> Res<()> { setsock_i32(fd, SOL_TCP, 1, v) }
pub fn n_io_netty_channel_unix_Socket__isIPv6(fd: i32) -> Res<i32> {
    let mut sa = [0u8; 32];
    let mut len = 32u32;
    if unsafe { getsockname(fd, sa.as_mut_ptr() as *mut c_void, &mut len) } < 0 { return Ok(0); }
    Ok((u16::from_ne_bytes([sa[0], sa[1]]) == AF_INET6 as u16) as i32)
}
pub fn n_io_netty_channel_unix_Socket__isIPv6Preferred0(_force_v4: i32) -> Res<i32> {
    Ok(if _force_v4 != 0 { 0 } else { 1 })
}
pub fn n_io_netty_channel_unix_Socket__msgFastopen() -> Res<i32> { Ok(0x2000_0000) }

// ------------------------------------------------------------------ io.netty.channel.epoll.*
pub fn n_io_netty_channel_epoll_NativeStaticallyReferencedJniMethods__epollin() -> Res<i32> { Ok(0x001) }
pub fn n_io_netty_channel_epoll_NativeStaticallyReferencedJniMethods__epollout() -> Res<i32> { Ok(0x004) }
pub fn n_io_netty_channel_epoll_NativeStaticallyReferencedJniMethods__epollrdhup() -> Res<i32> { Ok(0x2000) }
pub fn n_io_netty_channel_epoll_NativeStaticallyReferencedJniMethods__epollet() -> Res<i32> { Ok(1 << 31) }
pub fn n_io_netty_channel_epoll_NativeStaticallyReferencedJniMethods__epollerr() -> Res<i32> { Ok(0x008) }
pub fn n_io_netty_channel_epoll_NativeStaticallyReferencedJniMethods__tcpMd5SigMaxKeyLen() -> Res<i32> { Ok(80) }
pub fn n_io_netty_channel_epoll_NativeStaticallyReferencedJniMethods__isSupportingSendmmsg() -> Res<i32> { Ok(1) }
pub fn n_io_netty_channel_epoll_NativeStaticallyReferencedJniMethods__isSupportingRecvmmsg() -> Res<i32> { Ok(1) }
pub fn n_io_netty_channel_epoll_NativeStaticallyReferencedJniMethods__tcpFastopenMode() -> Res<i32> { Ok(3) }
pub fn n_io_netty_channel_epoll_NativeStaticallyReferencedJniMethods__kernelVersion() -> Res<Ref> {
    let mut un: UtsName = unsafe { std::mem::zeroed() };
    unsafe { uname(&mut un) };
    let rel = unsafe { CStr::from_ptr(un.release.as_ptr()).to_string_lossy().into_owned() };
    Ok(rt::new_str(rel))
}

pub fn n_io_netty_channel_epoll_Native__offsetofEpollData() -> Res<i32> { Ok(std::mem::offset_of!(EpollEvent, data) as i32) }
pub fn n_io_netty_channel_epoll_Native__sizeofEpollEvent() -> Res<i32> { Ok(std::mem::size_of::<EpollEvent>() as i32) }
pub fn n_io_netty_channel_epoll_Native__registerUnix() -> Res<i32> { Ok(0) }
pub fn n_io_netty_channel_epoll_Native__isSupportingUdpSegment() -> Res<i32> { Ok(0) }
pub fn n_io_netty_channel_epoll_Native__epollCreate() -> Res<i32> {
    let fd = unsafe { epoll_create1(0o2000000) };
    Ok(if fd < 0 { -errno() } else { fd })
}
pub fn n_io_netty_channel_epoll_Native__eventFd() -> Res<i32> {
    let fd = unsafe { eventfd(0, 0o4000 | 0o2000000) };
    Ok(if fd < 0 { -errno() } else { fd })
}
pub fn n_io_netty_channel_epoll_Native__timerFd() -> Res<i32> {
    let fd = unsafe { timerfd_create(1, 0o4000 | 0o2000000) };
    Ok(if fd < 0 { -errno() } else { fd })
}
pub fn n_io_netty_channel_epoll_Native__eventFdWrite(fd: i32, val: i64) -> Res<()> {
    unsafe { write(fd, &val as *const i64 as *const c_void, 8) };
    Ok(())
}
pub fn n_io_netty_channel_epoll_Native__epollCtlAdd0(efd: i32, fd: i32, flags: i32) -> Res<i32> {
    let ev = EpollEvent { events: flags as u32, data: fd as u64 };
    Ok(if unsafe { epoll_ctl(efd, 1, fd, &ev) } < 0 { -errno() } else { 0 })
}
pub fn n_io_netty_channel_epoll_Native__epollCtlMod0(efd: i32, fd: i32, flags: i32) -> Res<i32> {
    let ev = EpollEvent { events: flags as u32, data: fd as u64 };
    Ok(if unsafe { epoll_ctl(efd, 3, fd, &ev) } < 0 { -errno() } else { 0 })
}
pub fn n_io_netty_channel_epoll_Native__epollCtlDel0(efd: i32, fd: i32) -> Res<i32> {
    let ev = EpollEvent { events: 0, data: 0 };
    Ok(if unsafe { epoll_ctl(efd, 2, fd, &ev) } < 0 { -errno() } else { 0 })
}
pub fn n_io_netty_channel_epoll_Native__epollWait(efd: i32, addr: i64, len: i32, timeout_ms: i32) -> Res<i32> {
    let n = {
        let _blk = ScopedBlock::enter();
        unsafe { epoll_wait(efd, addr as usize as *mut EpollEvent, len, timeout_ms) }
    };
    Ok(if n < 0 { -errno() } else { n })
}
pub fn n_io_netty_channel_epoll_Native__epollBusyWait0(efd: i32, addr: i64, len: i32) -> Res<i32> {
    n_io_netty_channel_epoll_Native__epollWait(efd, addr, len, 0)
}
pub fn n_io_netty_channel_epoll_Native__epollWait0(
    efd: i32, addr: i64, len: i32, timer_fd: i32, tv_sec: i32, tv_nsec: i32, _millis_threshold: i64,
) -> Res<i64> {
    if timer_fd >= 0 && (tv_sec > 0 || tv_nsec > 0) {
        let its = ITimerSpec { it_interval_sec: 0, it_interval_nsec: 0, it_value_sec: tv_sec as i64, it_value_nsec: tv_nsec as i64 };
        unsafe { timerfd_settime(timer_fd, 0, &its, std::ptr::null_mut()) };
    }
    let timeout = if tv_sec < 0 { -1 } else if tv_sec == 0 && tv_nsec == 0 { 0 } else { (tv_sec * 1000 + tv_nsec / 1_000_000).max(1) };
    let n = n_io_netty_channel_epoll_Native__epollWait(efd, addr, len, timeout)?;
    Ok(if n < 0 { n as i64 } else { (n as i64) << 32 })
}
pub fn n_io_netty_channel_epoll_Native__splice0(fd: i32, off_in: i64, fd_out: i32, off_out: i64, len: i64) -> Res<i32> {
    let mut oi = off_in;
    let mut oo = off_out;
    let r = unsafe {
        splice(
            fd, if off_in < 0 { std::ptr::null_mut() } else { &mut oi },
            fd_out, if off_out < 0 { std::ptr::null_mut() } else { &mut oo },
            len.max(0) as usize, 0,
        )
    };
    Ok(if r < 0 { -errno() } else { r as i32 })
}
pub fn n_io_netty_channel_epoll_Native__sendmmsg0(_fd: i32, _ipv6: i32, _pkts: Ref, _off: i32, _len: i32) -> Res<i32> { Ok(-95) }
pub fn n_io_netty_channel_epoll_Native__recvmmsg0(_fd: i32, _ipv6: i32, _pkts: Ref, _off: i32, _len: i32) -> Res<i32> { Ok(-95) }
pub fn n_io_netty_channel_epoll_Native__recvmsg0(_fd: i32, _ipv6: i32, _pkt: Ref) -> Res<i32> { Ok(-95) }

pub fn n_io_netty_channel_epoll_LinuxSocket__setTimeToLive(fd: i32, v: i32) -> Res<()> { setsock_i32(fd, IPPROTO_IP, 2, v) }
pub fn n_io_netty_channel_epoll_LinuxSocket__getTimeToLive(fd: i32) -> Res<i32> { getsock_i32(fd, IPPROTO_IP, 2) }
pub fn n_io_netty_channel_epoll_LinuxSocket__setTcpCork(fd: i32, v: i32) -> Res<()> { setsock_i32(fd, SOL_TCP, 3, v) }
pub fn n_io_netty_channel_epoll_LinuxSocket__isTcpCork(fd: i32) -> Res<i32> { getsock_i32(fd, SOL_TCP, 3) }
pub fn n_io_netty_channel_epoll_LinuxSocket__setTcpQuickAck(fd: i32, v: i32) -> Res<()> { setsock_i32(fd, SOL_TCP, 12, v) }
pub fn n_io_netty_channel_epoll_LinuxSocket__isTcpQuickAck(fd: i32) -> Res<i32> { getsock_i32(fd, SOL_TCP, 12) }
pub fn n_io_netty_channel_epoll_LinuxSocket__setTcpDeferAccept(fd: i32, v: i32) -> Res<()> { setsock_i32(fd, SOL_TCP, 9, v) }
pub fn n_io_netty_channel_epoll_LinuxSocket__getTcpDeferAccept(fd: i32) -> Res<i32> { getsock_i32(fd, SOL_TCP, 9) }
pub fn n_io_netty_channel_epoll_LinuxSocket__setTcpFastOpen(fd: i32, v: i32) -> Res<()> { setsock_i32(fd, SOL_TCP, 23, v) }
pub fn n_io_netty_channel_epoll_LinuxSocket__setTcpKeepIdle(fd: i32, v: i32) -> Res<()> { setsock_i32(fd, SOL_TCP, 4, v) }
pub fn n_io_netty_channel_epoll_LinuxSocket__getTcpKeepIdle(fd: i32) -> Res<i32> { getsock_i32(fd, SOL_TCP, 4) }
pub fn n_io_netty_channel_epoll_LinuxSocket__setTcpKeepIntvl(fd: i32, v: i32) -> Res<()> { setsock_i32(fd, SOL_TCP, 5, v) }
pub fn n_io_netty_channel_epoll_LinuxSocket__getTcpKeepIntvl(fd: i32) -> Res<i32> { getsock_i32(fd, SOL_TCP, 5) }
pub fn n_io_netty_channel_epoll_LinuxSocket__setTcpKeepCnt(fd: i32, v: i32) -> Res<()> { setsock_i32(fd, SOL_TCP, 6, v) }
pub fn n_io_netty_channel_epoll_LinuxSocket__getTcpKeepCnt(fd: i32) -> Res<i32> { getsock_i32(fd, SOL_TCP, 6) }
pub fn n_io_netty_channel_epoll_LinuxSocket__setTcpUserTimeout(fd: i32, v: i32) -> Res<()> { setsock_i32(fd, SOL_TCP, 18, v) }
pub fn n_io_netty_channel_epoll_LinuxSocket__getTcpUserTimeout(fd: i32) -> Res<i32> { getsock_i32(fd, SOL_TCP, 18) }
pub fn n_io_netty_channel_epoll_LinuxSocket__setTcpNotSentLowAt(fd: i32, v: i32) -> Res<()> { setsock_i32(fd, SOL_TCP, 25, v) }
pub fn n_io_netty_channel_epoll_LinuxSocket__getTcpNotSentLowAt(fd: i32) -> Res<i32> { getsock_i32(fd, SOL_TCP, 25) }
pub fn n_io_netty_channel_epoll_LinuxSocket__setIpTransparent(fd: i32, v: i32) -> Res<()> { setsock_i32(fd, IPPROTO_IP, 19, v) }
pub fn n_io_netty_channel_epoll_LinuxSocket__isIpTransparent(fd: i32) -> Res<i32> { getsock_i32(fd, IPPROTO_IP, 19) }
pub fn n_io_netty_channel_epoll_LinuxSocket__setIpFreeBind(fd: i32, v: i32) -> Res<()> { setsock_i32(fd, IPPROTO_IP, 15, v) }
pub fn n_io_netty_channel_epoll_LinuxSocket__isIpFreeBind(fd: i32) -> Res<i32> { getsock_i32(fd, IPPROTO_IP, 15) }
pub fn n_io_netty_channel_epoll_LinuxSocket__setIpBindAddressNoPort(fd: i32, v: i32) -> Res<()> { setsock_i32(fd, IPPROTO_IP, 24, v) }
pub fn n_io_netty_channel_epoll_LinuxSocket__isIpBindAddressNoPort(fd: i32) -> Res<i32> { getsock_i32(fd, IPPROTO_IP, 24) }
pub fn n_io_netty_channel_epoll_LinuxSocket__setIpRecvOrigDestAddr(fd: i32, v: i32) -> Res<()> { setsock_i32(fd, IPPROTO_IP, 20, v) }
pub fn n_io_netty_channel_epoll_LinuxSocket__isIpRecvOrigDestAddr(fd: i32) -> Res<i32> { getsock_i32(fd, IPPROTO_IP, 20) }
pub fn n_io_netty_channel_epoll_LinuxSocket__setSoBusyPoll(fd: i32, v: i32) -> Res<()> { setsock_i32(fd, SOL_SOCKET, 46, v) }
pub fn n_io_netty_channel_epoll_LinuxSocket__getSoBusyPoll(fd: i32) -> Res<i32> { getsock_i32(fd, SOL_SOCKET, 46) }
pub fn n_io_netty_channel_epoll_LinuxSocket__setUdpGro(_fd: i32, _v: i32) -> Res<()> { Ok(()) }
pub fn n_io_netty_channel_epoll_LinuxSocket__setIpMulticastAll(fd: i32, ipv6: i32, v: i32) -> Res<()> {
    setsock_i32(fd, if ipv6 != 0 { IPPROTO_IPV6 } else { IPPROTO_IP }, 49, v)
}
pub fn n_io_netty_channel_epoll_LinuxSocket__isIpMulticastAll(fd: i32, ipv6: i32) -> Res<i32> {
    getsock_i32(fd, if ipv6 != 0 { IPPROTO_IPV6 } else { IPPROTO_IP }, 49)
}
pub fn n_io_netty_channel_epoll_LinuxSocket__setIpMulticastLoop(fd: i32, ipv6: i32, v: i32) -> Res<()> {
    setsock_i32(fd, if ipv6 != 0 { IPPROTO_IPV6 } else { IPPROTO_IP }, if ipv6 != 0 { 19 } else { 34 }, v)
}
pub fn n_io_netty_channel_epoll_LinuxSocket__getIpMulticastLoop(fd: i32, ipv6: i32) -> Res<i32> {
    getsock_i32(fd, if ipv6 != 0 { IPPROTO_IPV6 } else { IPPROTO_IP }, if ipv6 != 0 { 19 } else { 34 })
}
pub fn n_io_netty_channel_epoll_LinuxSocket__setInterface(fd: i32, ipv6: i32, addr: Ref, _scope: i32, idx: i32) -> Res<()> {
    if ipv6 != 0 {
        return setsock_i32(fd, IPPROTO_IPV6, 17, idx);
    }
    let b = unsafe { rt::byte_slice(addr)? };
    let ip = u32::from_ne_bytes([b[0], b[1], b[2], b[3]]);
    unsafe { setsockopt(fd, IPPROTO_IP, 32, &ip as *const u32 as *const c_void, 4) };
    Ok(())
}
pub fn n_io_netty_channel_epoll_LinuxSocket__getInterface(fd: i32, ipv6: i32) -> Res<i32> {
    getsock_i32(fd, if ipv6 != 0 { IPPROTO_IPV6 } else { IPPROTO_IP }, if ipv6 != 0 { 17 } else { 32 })
}
pub fn n_io_netty_channel_epoll_LinuxSocket__joinGroup(fd: i32, ipv6: i32, grp: Ref, if_addr: Ref, _sc: i32, idx: i32) -> Res<()> {
    let gb = unsafe { rt::byte_slice(grp)? };
    if ipv6 != 0 && gb.len() == 16 {
        let mut m = Ipv6Mreq { multiaddr: [0; 16], ifindex: idx as u32 };
        m.multiaddr.copy_from_slice(&gb[..16]);
        unsafe { setsockopt(fd, IPPROTO_IPV6, 20, &m as *const Ipv6Mreq as *const c_void, 20) };
    } else {
        let ib = unsafe { rt::byte_slice(if_addr)? };
        let m = IpMreq {
            multiaddr: u32::from_ne_bytes([gb[0], gb[1], gb[2], gb[3]]),
            interface: u32::from_ne_bytes([ib[0], ib[1], ib[2], ib[3]]),
        };
        unsafe { setsockopt(fd, IPPROTO_IP, 35, &m as *const IpMreq as *const c_void, 8) };
    }
    Ok(())
}
pub fn n_io_netty_channel_epoll_LinuxSocket__leaveGroup(fd: i32, ipv6: i32, grp: Ref, if_addr: Ref, _sc: i32, idx: i32) -> Res<()> {
    let gb = unsafe { rt::byte_slice(grp)? };
    if ipv6 != 0 && gb.len() == 16 {
        let mut m = Ipv6Mreq { multiaddr: [0; 16], ifindex: idx as u32 };
        m.multiaddr.copy_from_slice(&gb[..16]);
        unsafe { setsockopt(fd, IPPROTO_IPV6, 21, &m as *const Ipv6Mreq as *const c_void, 20) };
    } else {
        let ib = unsafe { rt::byte_slice(if_addr)? };
        let m = IpMreq {
            multiaddr: u32::from_ne_bytes([gb[0], gb[1], gb[2], gb[3]]),
            interface: u32::from_ne_bytes([ib[0], ib[1], ib[2], ib[3]]),
        };
        unsafe { setsockopt(fd, IPPROTO_IP, 36, &m as *const IpMreq as *const c_void, 8) };
    }
    Ok(())
}
pub fn n_io_netty_channel_epoll_LinuxSocket__joinSsmGroup(fd: i32, _v6: i32, grp: Ref, if_addr: Ref, _sc: i32, _idx: i32, src: Ref) -> Res<()> {
    let (gb, ib, sb) = unsafe { (rt::byte_slice(grp)?, rt::byte_slice(if_addr)?, rt::byte_slice(src)?) };
    if gb.len() >= 4 && ib.len() >= 4 && sb.len() >= 4 {
        let m = IpMreqSource {
            multiaddr: u32::from_ne_bytes([gb[0], gb[1], gb[2], gb[3]]),
            interface: u32::from_ne_bytes([ib[0], ib[1], ib[2], ib[3]]),
            sourceaddr: u32::from_ne_bytes([sb[0], sb[1], sb[2], sb[3]]),
        };
        unsafe { setsockopt(fd, IPPROTO_IP, 39, &m as *const IpMreqSource as *const c_void, 12) };
    }
    Ok(())
}
pub fn n_io_netty_channel_epoll_LinuxSocket__leaveSsmGroup(fd: i32, _v6: i32, grp: Ref, if_addr: Ref, _sc: i32, _idx: i32, src: Ref) -> Res<()> {
    let (gb, ib, sb) = unsafe { (rt::byte_slice(grp)?, rt::byte_slice(if_addr)?, rt::byte_slice(src)?) };
    if gb.len() >= 4 && ib.len() >= 4 && sb.len() >= 4 {
        let m = IpMreqSource {
            multiaddr: u32::from_ne_bytes([gb[0], gb[1], gb[2], gb[3]]),
            interface: u32::from_ne_bytes([ib[0], ib[1], ib[2], ib[3]]),
            sourceaddr: u32::from_ne_bytes([sb[0], sb[1], sb[2], sb[3]]),
        };
        unsafe { setsockopt(fd, IPPROTO_IP, 40, &m as *const IpMreqSource as *const c_void, 12) };
    }
    Ok(())
}
pub fn n_io_netty_channel_epoll_LinuxSocket__setTcpMd5Sig(_fd: i32, _v6: i32, _addr: Ref, _sc: i32, _key: Ref) -> Res<()> { Ok(()) }
pub fn n_io_netty_channel_epoll_LinuxSocket__getTcpInfo(fd: i32, arr: Ref) -> Res<()> {
    let a = rt::nn(arr)? as *const Arr;
    let len = unsafe { (*a).len as usize };
    let mut info = [0u32; 64];
    let mut ilen = 256u32;
    if unsafe { getsockopt(fd, SOL_TCP, 11, info.as_mut_ptr() as *mut c_void, &mut ilen) } >= 0 {
        let dst = unsafe { std::slice::from_raw_parts_mut((a as *mut u8).add(24) as *mut i64, len) };
        for (i, d) in dst.iter_mut().enumerate() {
            if i < info.len() { *d = info[i] as i64; }
        }
    }
    Ok(())
}
pub fn n_io_netty_channel_epoll_LinuxSocket__sendFile(fd: i32, region: Ref, base: i64, off: i64, len: i64) -> Res<i64> {
    let fobj = rt::get_ref(rt::nn_ref(region)?, "file");
    let src_fd = rt::get_i32(fobj, "fd");
    let mut offset = base + off;
    let n = {
        let _blk = ScopedBlock::enter();
        unsafe { sendfile(fd, src_fd, &mut offset, len.max(0) as usize) }
    };
    Ok(if n < 0 { -(errno() as i64) } else { n as i64 })
}

// ------------------------------------------------------------------ io.netty.channel.kqueue (BSD-only -> UnsatisfiedLinkError on Linux)
fn kqueue_err<T>() -> Res<T> {
    rt::throw("java/lang/UnsatisfiedLinkError", "kqueue unavailable on Linux".to_string())
}
pub fn n_io_netty_channel_kqueue_Native__registerUnix() -> Res<i32> { kqueue_err() }
pub fn n_io_netty_channel_kqueue_Native__kqueueCreate() -> Res<i32> { kqueue_err() }
pub fn n_io_netty_channel_kqueue_Native__keventAddUserEvent(_kfd: i32, _id: i32) -> Res<i32> { kqueue_err() }
pub fn n_io_netty_channel_kqueue_Native__keventTriggerUserEvent(_kfd: i32, _id: i32) -> Res<i32> { kqueue_err() }
pub fn n_io_netty_channel_kqueue_Native__keventWait(_kfd: i32, _ca: i64, _cs: i32, _ea: i64, _es: i32, _s: i32, _ns: i32) -> Res<i32> { kqueue_err() }
pub fn n_io_netty_channel_kqueue_Native__sizeofKEvent() -> Res<i32> { Ok(32) }
pub fn n_io_netty_channel_kqueue_Native__offsetofKEventIdent() -> Res<i32> { Ok(0) }
pub fn n_io_netty_channel_kqueue_Native__offsetofKEventFilter() -> Res<i32> { Ok(8) }
pub fn n_io_netty_channel_kqueue_Native__offsetofKEventFFlags() -> Res<i32> { Ok(12) }
pub fn n_io_netty_channel_kqueue_Native__offsetofKEventFlags() -> Res<i32> { Ok(10) }
pub fn n_io_netty_channel_kqueue_Native__offsetofKeventData() -> Res<i32> { Ok(16) }
pub fn n_io_netty_channel_kqueue_Native__offsetofKeventUdata() -> Res<i32> { Ok(24) }
pub fn n_io_netty_channel_kqueue_KQueueStaticallyReferencedJniMethods__evAdd() -> Res<i32> { Ok(1) }
pub fn n_io_netty_channel_kqueue_KQueueStaticallyReferencedJniMethods__evEnable() -> Res<i32> { Ok(4) }
pub fn n_io_netty_channel_kqueue_KQueueStaticallyReferencedJniMethods__evDisable() -> Res<i32> { Ok(8) }
pub fn n_io_netty_channel_kqueue_KQueueStaticallyReferencedJniMethods__evDelete() -> Res<i32> { Ok(2) }
pub fn n_io_netty_channel_kqueue_KQueueStaticallyReferencedJniMethods__evClear() -> Res<i32> { Ok(0x20) }
pub fn n_io_netty_channel_kqueue_KQueueStaticallyReferencedJniMethods__evError() -> Res<i32> { Ok(0x4000) }
pub fn n_io_netty_channel_kqueue_KQueueStaticallyReferencedJniMethods__evEOF() -> Res<i32> { Ok(0x8000u16 as i16 as i32) }
pub fn n_io_netty_channel_kqueue_KQueueStaticallyReferencedJniMethods__evfiltRead() -> Res<i32> { Ok(-1) }
pub fn n_io_netty_channel_kqueue_KQueueStaticallyReferencedJniMethods__evfiltWrite() -> Res<i32> { Ok(-2) }
pub fn n_io_netty_channel_kqueue_KQueueStaticallyReferencedJniMethods__evfiltUser() -> Res<i32> { Ok(-10) }
pub fn n_io_netty_channel_kqueue_KQueueStaticallyReferencedJniMethods__evfiltSock() -> Res<i32> { Ok(-13) }
pub fn n_io_netty_channel_kqueue_KQueueStaticallyReferencedJniMethods__noteReadClosed() -> Res<i32> { Ok(2) }
pub fn n_io_netty_channel_kqueue_KQueueStaticallyReferencedJniMethods__noteConnReset() -> Res<i32> { Ok(1) }
pub fn n_io_netty_channel_kqueue_KQueueStaticallyReferencedJniMethods__noteDisconnected() -> Res<i32> { Ok(0x1000) }
pub fn n_io_netty_channel_kqueue_KQueueStaticallyReferencedJniMethods__fastOpenClient() -> Res<i32> { Ok(0) }
pub fn n_io_netty_channel_kqueue_KQueueStaticallyReferencedJniMethods__fastOpenServer() -> Res<i32> { Ok(0) }
pub fn n_io_netty_channel_kqueue_KQueueStaticallyReferencedJniMethods__connectDataIdempotent() -> Res<i32> { Ok(0) }
pub fn n_io_netty_channel_kqueue_KQueueStaticallyReferencedJniMethods__connectResumeOnReadWrite() -> Res<i32> { Ok(0) }
pub fn n_io_netty_channel_kqueue_KQueueEventArray__evSet(_a: i64, _i: i32, _f: i32, _fl: i32, _ff: i32, _d: i64, _u: i64) -> Res<()> { kqueue_err() }

// ------------------------------------------------------------------ com.sun.jna.Native
static JNA_LAST_ERR: AtomicI32 = AtomicI32::new(0);

#[inline(always)]
fn jna_addr(ptr: Ref, base: i64, off: i64) -> usize {
    let b = if base != 0 {
        base as usize
    } else if !ptr.is_null() {
        rt::get_i64(ptr, "peer") as usize
    } else {
        0
    };
    b.wrapping_add(off as usize)
}

pub fn n_com_sun_jna_Native__initIDs() -> Res<()> { Ok(()) }
pub fn n_com_sun_jna_Native__setProtected(_p: i32) -> Res<()> { Ok(()) }
pub fn n_com_sun_jna_Native__getNativeVersion() -> Res<Ref> { Ok(rt::new_str("7.0.0".to_string())) }
pub fn n_com_sun_jna_Native__getLastError() -> Res<i32> { Ok(JNA_LAST_ERR.load(Ordering::Relaxed)) }
pub fn n_com_sun_jna_Native__malloc(sz: i64) -> Res<i64> { Ok(unsafe { malloc(sz.max(1) as usize) } as usize as i64) }
pub fn n_com_sun_jna_Native__free(a: i64) -> Res<()> { if a != 0 { unsafe { free(a as usize as *mut c_void) }; } Ok(()) }
pub fn n_com_sun_jna_Native___getPointer(a: i64) -> Res<i64> { Ok(unsafe { *(a as usize as *const i64) }) }
pub fn n_com_sun_jna_Native___getDirectBufferPointer(buf: Ref) -> Res<i64> {
    if buf.is_null() { return Ok(0); }
    Ok(rt::get_i64(buf, "address"))
}
pub fn n_com_sun_jna_Native__open(name: Ref, flags: i32) -> Res<i64> {
    let fl = if flags <= 0 { 2 | 0x100 } else { flags };
    let h = if name.is_null() {
        unsafe { dlopen(std::ptr::null(), if flags <= 0 { 2 } else { flags }) }
    } else {
        let s = rt::str_to_rust(name);
        let cp = CString::new(s.as_bytes()).map_err(|_| rt::Thrown { obj: (rt::hooks().throw_new)("java/lang/IllegalArgumentException", rt::new_str("nul byte".to_string())) })?;
        let mut ptr = unsafe { dlopen(cp.as_ptr(), fl) };
        if ptr.is_null() {
            let base = s.rsplit('/').next().unwrap_or(&s);
            let alt: Option<&str> = match base {
                "c" | "libc.so" => Some("libc.so.6"),
                "m" | "libm.so" => Some("libm.so.6"),
                "pthread" | "libpthread.so" | "dl" | "libdl.so" | "rt" | "librt.so" => Some("libc.so.6"),
                _ => None,
            };
            if let Some(a) = alt {
                if let Ok(ca) = CString::new(a) {
                    ptr = unsafe { dlopen(ca.as_ptr(), fl) };
                }
            }
        }
        ptr
    };
    if h.is_null() {
        let err = unsafe { dlerror() };
        let msg = if err.is_null() { "dlopen failed".to_string() } else { unsafe { CStr::from_ptr(err) }.to_string_lossy().into_owned() };
        return rt::throw("java/lang/UnsatisfiedLinkError", msg);
    }
    Ok(h as usize as i64)
}
pub fn n_com_sun_jna_Native__close(h: i64) -> Res<()> {
    if h != 0 { unsafe { dlclose(h as usize as *mut c_void) }; }
    Ok(())
}
pub fn n_com_sun_jna_Native__findSymbol(h: i64, name: Ref) -> Res<i64> {
    let cn = rt::c_str_arg(name)?;
    let s = unsafe { dlsym(h as usize as *mut c_void, cn.as_ptr()) };
    if s.is_null() {
        return rt::throw("java/lang/UnsatisfiedLinkError", cn.to_string_lossy().into_owned());
    }
    Ok(s as usize as i64)
}
pub fn n_com_sun_jna_Native__sizeof(ty: i32) -> Res<i32> {
    Ok(match ty { 0 => 8, 1 | 2 => std::mem::size_of::<c_long>() as i32, _ => 8 })
}
pub fn n_com_sun_jna_Native__getByte(p: Ref, b: i64, o: i64) -> Res<i32> { Ok(unsafe { *(jna_addr(p, b, o) as *const i8) } as i32) }
pub fn n_com_sun_jna_Native__getChar(p: Ref, b: i64, o: i64) -> Res<i32> { Ok(unsafe { *(jna_addr(p, b, o) as *const u16) } as i32) }
pub fn n_com_sun_jna_Native__getShort(p: Ref, b: i64, o: i64) -> Res<i32> { Ok(unsafe { *(jna_addr(p, b, o) as *const i16) } as i32) }
pub fn n_com_sun_jna_Native__getInt(p: Ref, b: i64, o: i64) -> Res<i32> { Ok(unsafe { *(jna_addr(p, b, o) as *const i32) }) }
pub fn n_com_sun_jna_Native__getLong(p: Ref, b: i64, o: i64) -> Res<i64> { Ok(unsafe { *(jna_addr(p, b, o) as *const i64) }) }
pub fn n_com_sun_jna_Native__getFloat(p: Ref, b: i64, o: i64) -> Res<f32> { Ok(unsafe { *(jna_addr(p, b, o) as *const f32) }) }
pub fn n_com_sun_jna_Native__getDouble(p: Ref, b: i64, o: i64) -> Res<f64> { Ok(unsafe { *(jna_addr(p, b, o) as *const f64) }) }
pub fn n_com_sun_jna_Native__setByte(p: Ref, b: i64, o: i64, v: i32) -> Res<()> { unsafe { *(jna_addr(p, b, o) as *mut i8) = v as i8 }; Ok(()) }
pub fn n_com_sun_jna_Native__setChar(p: Ref, b: i64, o: i64, v: i32) -> Res<()> { unsafe { *(jna_addr(p, b, o) as *mut u16) = v as u16 }; Ok(()) }
pub fn n_com_sun_jna_Native__setShort(p: Ref, b: i64, o: i64, v: i32) -> Res<()> { unsafe { *(jna_addr(p, b, o) as *mut i16) = v as i16 }; Ok(()) }
pub fn n_com_sun_jna_Native__setInt(p: Ref, b: i64, o: i64, v: i32) -> Res<()> { unsafe { *(jna_addr(p, b, o) as *mut i32) = v }; Ok(()) }
pub fn n_com_sun_jna_Native__setLong(p: Ref, b: i64, o: i64, v: i64) -> Res<()> { unsafe { *(jna_addr(p, b, o) as *mut i64) = v }; Ok(()) }
pub fn n_com_sun_jna_Native__setFloat(p: Ref, b: i64, o: i64, v: f32) -> Res<()> { unsafe { *(jna_addr(p, b, o) as *mut f32) = v }; Ok(()) }
pub fn n_com_sun_jna_Native__setDouble(p: Ref, b: i64, o: i64, v: f64) -> Res<()> { unsafe { *(jna_addr(p, b, o) as *mut f64) = v }; Ok(()) }
pub fn n_com_sun_jna_Native__setPointer(p: Ref, b: i64, o: i64, v: i64) -> Res<()> { unsafe { *(jna_addr(p, b, o) as *mut i64) = v }; Ok(()) }
pub fn n_com_sun_jna_Native__setMemory(p: Ref, b: i64, o: i64, count: i64, val: i32) -> Res<()> {
    unsafe { std::ptr::write_bytes(jna_addr(p, b, o) as *mut u8, val as u8, count.max(0) as usize) };
    Ok(())
}
pub fn n_com_sun_jna_Native__getStringBytes(p: Ref, b: i64, o: i64) -> Res<Ref> {
    let ptr = jna_addr(p, b, o) as *const c_char;
    new_bytes(unsafe { CStr::from_ptr(ptr).to_bytes() })
}
pub fn n_com_sun_jna_Native__getWideString(p: Ref, b: i64, o: i64) -> Res<Ref> {
    let mut ptr = jna_addr(p, b, o) as *const u32;
    let mut s = String::new();
    unsafe {
        while *ptr != 0 {
            if let Some(ch) = char::from_u32(*ptr) { s.push(ch); }
            ptr = ptr.add(1);
        }
    }
    Ok(rt::new_str(s))
}
pub fn n_com_sun_jna_Native__setWideString(p: Ref, b: i64, o: i64, val: Ref) -> Res<()> {
    let ptr = jna_addr(p, b, o) as *mut u32;
    let rs = if val.is_null() { String::new() } else { rt::str_to_rust(val) };
    unsafe {
        for (i, ch) in rs.chars().enumerate() {
            *ptr.add(i) = ch as u32;
        }
        *ptr.add(rs.chars().count()) = 0;
    }
    Ok(())
}

unsafe fn jna_rw<T: Copy>(p: Ref, b: i64, o: i64, arr: Ref, idx: i32, n: i32, write: bool) -> Res<()> {
    let _ = rt::nn(arr)?;
    let native = jna_addr(p, b, o) as *mut T;
    let jptr = (arr.0 as *mut u8).add(24) as *mut T;
    if write {
        std::ptr::copy_nonoverlapping(jptr.add(idx as usize), native, n.max(0) as usize);
    } else {
        std::ptr::copy_nonoverlapping(native, jptr.add(idx as usize), n.max(0) as usize);
    }
    Ok(())
}
pub fn n_com_sun_jna_Native__read__56ade1(p: Ref, b: i64, o: i64, a: Ref, i: i32, n: i32) -> Res<()> { unsafe { jna_rw::<u8>(p, b, o, a, i, n, false) } }
pub fn n_com_sun_jna_Native__read__224c37(p: Ref, b: i64, o: i64, a: Ref, i: i32, n: i32) -> Res<()> { unsafe { jna_rw::<i16>(p, b, o, a, i, n, false) } }
pub fn n_com_sun_jna_Native__read__a71511(p: Ref, b: i64, o: i64, a: Ref, i: i32, n: i32) -> Res<()> { unsafe { jna_rw::<u16>(p, b, o, a, i, n, false) } }
pub fn n_com_sun_jna_Native__read__63500f(p: Ref, b: i64, o: i64, a: Ref, i: i32, n: i32) -> Res<()> { unsafe { jna_rw::<i32>(p, b, o, a, i, n, false) } }
pub fn n_com_sun_jna_Native__read__31753f(p: Ref, b: i64, o: i64, a: Ref, i: i32, n: i32) -> Res<()> { unsafe { jna_rw::<i64>(p, b, o, a, i, n, false) } }
pub fn n_com_sun_jna_Native__read__ce26bc(p: Ref, b: i64, o: i64, a: Ref, i: i32, n: i32) -> Res<()> { unsafe { jna_rw::<f32>(p, b, o, a, i, n, false) } }
pub fn n_com_sun_jna_Native__read__cdd726(p: Ref, b: i64, o: i64, a: Ref, i: i32, n: i32) -> Res<()> { unsafe { jna_rw::<f64>(p, b, o, a, i, n, false) } }
pub fn n_com_sun_jna_Native__write__56ade1(p: Ref, b: i64, o: i64, a: Ref, i: i32, n: i32) -> Res<()> { unsafe { jna_rw::<u8>(p, b, o, a, i, n, true) } }
pub fn n_com_sun_jna_Native__write__224c37(p: Ref, b: i64, o: i64, a: Ref, i: i32, n: i32) -> Res<()> { unsafe { jna_rw::<i16>(p, b, o, a, i, n, true) } }
pub fn n_com_sun_jna_Native__write__a71511(p: Ref, b: i64, o: i64, a: Ref, i: i32, n: i32) -> Res<()> { unsafe { jna_rw::<u16>(p, b, o, a, i, n, true) } }
pub fn n_com_sun_jna_Native__write__63500f(p: Ref, b: i64, o: i64, a: Ref, i: i32, n: i32) -> Res<()> { unsafe { jna_rw::<i32>(p, b, o, a, i, n, true) } }
pub fn n_com_sun_jna_Native__write__31753f(p: Ref, b: i64, o: i64, a: Ref, i: i32, n: i32) -> Res<()> { unsafe { jna_rw::<i64>(p, b, o, a, i, n, true) } }
pub fn n_com_sun_jna_Native__write__ce26bc(p: Ref, b: i64, o: i64, a: Ref, i: i32, n: i32) -> Res<()> { unsafe { jna_rw::<f32>(p, b, o, a, i, n, true) } }
pub fn n_com_sun_jna_Native__write__cdd726(p: Ref, b: i64, o: i64, a: Ref, i: i32, n: i32) -> Res<()> { unsafe { jna_rw::<f64>(p, b, o, a, i, n, true) } }

unsafe fn jna_ffi_call(fp: i64, args_arr: Ref, rtype: *mut c_void, ret_buf: *mut c_void) -> Res<()> {
    if fp == 0 {
        return rt::throw("java/lang/UnsatisfiedLinkError", "null function pointer".to_string());
    }
    let argc = if args_arr.is_null() { 0 } else { (*(args_arr.0 as *const Arr)).len as usize };
    let mut arg_types: Vec<*mut c_void> = Vec::with_capacity(argc);
    let mut arg_vals: Vec<u64> = vec![0; argc];
    let mut cstrings: Vec<CString> = Vec::new();
    for i in 0..argc {
        let a = rt::aload::<Ref>(args_arr, i as i32)?;
        if a.is_null() {
            arg_types.push(&raw mut ffi_type_pointer as *mut c_void);
            arg_vals[i] = 0;
            continue;
        }
        let cname = (*a.0).class.name;
        match cname {
            "java/lang/Integer" => {
                arg_types.push(&raw mut ffi_type_sint32 as *mut c_void);
                arg_vals[i] = rt::get_i32(a, "value") as u32 as u64;
            }
            "java/lang/Long" => {
                arg_types.push(&raw mut ffi_type_sint64 as *mut c_void);
                arg_vals[i] = rt::get_i64(a, "value") as u64;
            }
            "java/lang/Byte" => {
                arg_types.push(&raw mut ffi_type_sint8 as *mut c_void);
                arg_vals[i] = rt::get_i32(a, "value") as u64;
            }
            "java/lang/Short" => {
                arg_types.push(&raw mut ffi_type_sint16 as *mut c_void);
                arg_vals[i] = rt::get_i32(a, "value") as u64;
            }
            "java/lang/Boolean" => {
                arg_types.push(&raw mut ffi_type_sint32 as *mut c_void);
                arg_vals[i] = rt::get_bool(a, "value") as u64;
            }
            "java/lang/Float" => {
                arg_types.push(&raw mut ffi_type_float as *mut c_void);
                arg_vals[i] = rt::get_i32(a, "value") as u32 as u64;
            }
            "java/lang/Double" => {
                arg_types.push(&raw mut ffi_type_double as *mut c_void);
                arg_vals[i] = rt::get_i64(a, "value") as u64;
            }
            "java/lang/String" => {
                arg_types.push(&raw mut ffi_type_pointer as *mut c_void);
                let cs = CString::new(rt::str_to_rust(a)).unwrap_or_else(|_| CString::new("").unwrap());
                arg_vals[i] = cs.as_ptr() as usize as u64;
                cstrings.push(cs);
            }
            "com/sun/jna/Pointer" => {
                arg_types.push(&raw mut ffi_type_pointer as *mut c_void);
                arg_vals[i] = rt::get_i64(a, "peer") as u64;
            }
            _ => {
                arg_types.push(&raw mut ffi_type_pointer as *mut c_void);
                if (*a.0).class.elem != 0 {
                    arg_vals[i] = (a.0 as *mut u8).add(24) as usize as u64;
                } else {
                    arg_vals[i] = rt::get_i64(a, "peer") as u64;
                }
            }
        }
    }
    let mut arg_ptrs: Vec<*mut c_void> = arg_vals.iter_mut().map(|v| v as *mut u64 as *mut c_void).collect();
    let mut cif = [0u64; 8];
    let abi = if cfg!(target_arch = "x86_64") { 2 } else { 1 };
    ffi_prep_cif(
        cif.as_mut_ptr() as *mut c_void, abi, argc as c_uint, rtype, arg_types.as_mut_ptr() as *mut c_void,
    );
    let _blk = ScopedBlock::enter();
    ffi_call(cif.as_mut_ptr() as *mut c_void, fp as usize as *const c_void, ret_buf, arg_ptrs.as_mut_ptr());
    JNA_LAST_ERR.store(errno(), Ordering::Relaxed);
    Ok(())
}

pub fn n_com_sun_jna_Native__invokeVoid(_f: Ref, fp: i64, _flags: i32, args: Ref) -> Res<()> {
    let mut dummy = 0u64;
    unsafe { jna_ffi_call(fp, args, &raw mut ffi_type_void as *mut c_void, &mut dummy as *mut u64 as *mut c_void) }
}
pub fn n_com_sun_jna_Native__invokeInt(_f: Ref, fp: i64, _flags: i32, args: Ref) -> Res<i32> {
    let mut ret: i64 = 0;
    unsafe { jna_ffi_call(fp, args, &raw mut ffi_type_sint32 as *mut c_void, &mut ret as *mut i64 as *mut c_void)? };
    Ok(ret as i32)
}
pub fn n_com_sun_jna_Native__invokeLong(_f: Ref, fp: i64, _flags: i32, args: Ref) -> Res<i64> {
    let mut ret: i64 = 0;
    unsafe { jna_ffi_call(fp, args, &raw mut ffi_type_sint64 as *mut c_void, &mut ret as *mut i64 as *mut c_void)? };
    Ok(ret)
}
pub fn n_com_sun_jna_Native__invokeFloat(_f: Ref, fp: i64, _flags: i32, args: Ref) -> Res<f32> {
    let mut ret: f32 = 0.0;
    unsafe { jna_ffi_call(fp, args, &raw mut ffi_type_float as *mut c_void, &mut ret as *mut f32 as *mut c_void)? };
    Ok(ret)
}
pub fn n_com_sun_jna_Native__invokeDouble(_f: Ref, fp: i64, _flags: i32, args: Ref) -> Res<f64> {
    let mut ret: f64 = 0.0;
    unsafe { jna_ffi_call(fp, args, &raw mut ffi_type_double as *mut c_void, &mut ret as *mut f64 as *mut c_void)? };
    Ok(ret)
}
pub fn n_com_sun_jna_Native__invokePointer(_f: Ref, fp: i64, _flags: i32, args: Ref) -> Res<i64> {
    let mut ret: usize = 0;
    unsafe { jna_ffi_call(fp, args, &raw mut ffi_type_pointer as *mut c_void, &mut ret as *mut usize as *mut c_void)? };
    Ok(ret as i64)
}
pub fn n_com_sun_jna_Native__invokeObject(_f: Ref, _fp: i64, _flags: i32, _args: Ref) -> Res<Ref> { Ok(Ref::NULL) }
pub fn n_com_sun_jna_Native__invokeStructure(_f: Ref, fp: i64, _flags: i32, args: Ref, mem: i64, _sz: i64) -> Res<()> {
    unsafe { jna_ffi_call(fp, args, &raw mut ffi_type_pointer as *mut c_void, mem as usize as *mut c_void) }
}
pub fn n_com_sun_jna_Native__createNativeCallback(_cb: Ref, _m: Ref, _p: Ref, _r: Ref, _cc: i32, _f: i32, _enc: Ref) -> Res<i64> { Ok(0) }
pub fn n_com_sun_jna_Native__freeNativeCallback(_a: i64) -> Res<()> { Ok(()) }
pub fn n_com_sun_jna_Native__registerMethod(
    _c: Ref, _nm: Ref, _sig: Ref, _at: Ref, _uc: Ref, _vc: Ref, _sz: i32, _h: i64, _fp: i64,
    _m: Ref, _fp2: i64, _cv: i32, _th: i32, _tn: Ref, _fn: Ref, _enc: Ref,
) -> Res<i64> {
    Ok(0)
}

// ------------------------------------------------------------------ TracyBindings & NarratorLinux (Flite)
static TRACY_IDS: AtomicU32 = AtomicU32::new(1);
static TRACY_START: OnceLock<Instant> = OnceLock::new();
fn tracy_plots() -> &'static Mutex<HashMap<i64, f64>> {
    static M: OnceLock<Mutex<HashMap<i64, f64>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn n_com_mojang_jtracy_TracyBindings__appInfo(_s: Ref) -> Res<()> { Ok(()) }
pub fn n_com_mojang_jtracy_TracyBindings__setThreadName(_s: Ref, _w: i32) -> Res<()> { Ok(()) }
pub fn n_com_mojang_jtracy_TracyBindings__markFrame(_n: i64) -> Res<()> { let _ = TRACY_START.get_or_init(Instant::now); Ok(()) }
pub fn n_com_mojang_jtracy_TracyBindings__markFrameStart(_n: i64) -> Res<()> { let _ = TRACY_START.get_or_init(Instant::now); Ok(()) }
pub fn n_com_mojang_jtracy_TracyBindings__markFrameEnd(_n: i64) -> Res<()> { Ok(()) }
pub fn n_com_mojang_jtracy_TracyBindings__beginZone(_n: Ref, _f: Ref, _fn: Ref, _ln: i32) -> Res<i32> {
    Ok(TRACY_IDS.fetch_add(1, Ordering::Relaxed) as i32)
}
pub fn n_com_mojang_jtracy_TracyBindings__endZone(_id: i32) -> Res<()> { Ok(()) }
pub fn n_com_mojang_jtracy_TracyBindings__addZoneText(_id: i32, _t: Ref) -> Res<()> { Ok(()) }
pub fn n_com_mojang_jtracy_TracyBindings__messageColored(_t: Ref, _c: i32) -> Res<()> { Ok(()) }
pub fn n_com_mojang_jtracy_TracyBindings__plotValue(k: i64, v: f64) -> Res<()> {
    tracy_plots().lock().unwrap().insert(k, v);
    Ok(())
}
pub fn n_com_mojang_jtracy_TracyBindings__leakName(s: Ref) -> Res<i64> {
    if s.is_null() { return Ok(0); }
    let rs = rt::str_to_rust(s);
    let cs = CString::new(rs).unwrap_or_else(|_| CString::new("").unwrap());
    let len = cs.as_bytes_with_nul().len();
    let p = unsafe { malloc(len) as *mut u8 };
    if !p.is_null() {
        unsafe { std::ptr::copy_nonoverlapping(cs.as_bytes_with_nul().as_ptr(), p, len) };
    }
    Ok(p as usize as i64)
}
pub fn n_com_mojang_jtracy_TracyBindings__mallocNamed(a: i64, _sz: i64, _d: i32) -> Res<i64> { Ok(a) }
pub fn n_com_mojang_jtracy_TracyBindings__freeNamed(a: i64, _n: i64) -> Res<i64> { Ok(a) }
pub fn n_com_mojang_jtracy_TracyBindings__newGpuContext(_i: i32, _t: i64, _p: f32, _f: i32, _l: i32) -> Res<()> { Ok(()) }
pub fn n_com_mojang_jtracy_TracyBindings__beginGpuZone(_c: i32, _q: i32, _n: Ref, _f: Ref, _fn: Ref, _ln: i32) -> Res<i32> { Ok(0) }
pub fn n_com_mojang_jtracy_TracyBindings__endGpuZone(_c: i32, _q: i32) -> Res<i32> { Ok(0) }
pub fn n_com_mojang_jtracy_TracyBindings__submitQueryTimestamp(_c: i32, _q: i32, _ts: i64) -> Res<i32> { Ok(0) }
pub fn n_com_mojang_jtracy_TracyBindings__frameImage(_b: Ref, _w: i32, _h: i32, _off: i32, _flip: i32) -> Res<i32> { Ok(0) }
pub fn n_com_mojang_jtracy_TracyBindings__sectionSetup(_id: i32, _n: Ref) -> Res<()> { Ok(()) }
pub fn n_com_mojang_jtracy_TracyBindings__sectionEnter(_id: i32, _n: Ref) -> Res<i32> { Ok(0) }
pub fn n_com_mojang_jtracy_TracyBindings__sectionLeave(_id: i32) -> Res<()> { Ok(()) }

fn flite_lib() -> *mut c_void {
    static H: OnceLock<usize> = OnceLock::new();
    *H.get_or_init(|| {
        let h = unsafe { dlopen(b"libflite.so.1\0".as_ptr() as *const c_char, 2 | 0x100) };
        h as usize
    }) as *mut c_void
}
fn kal16_lib() -> *mut c_void {
    static H: OnceLock<usize> = OnceLock::new();
    *H.get_or_init(|| {
        let h = unsafe { dlopen(b"libflite_cmu_us_kal16.so.1\0".as_ptr() as *const c_char, 2 | 0x100) };
        h as usize
    }) as *mut c_void
}
fn jna_ptr(peer: usize) -> Res<Ref> {
    if peer == 0 { return Ok(Ref::NULL); }
    if let Some(c) = rt::find_class("com/sun/jna/Pointer") {
        let o = rt::alloc(c);
        rt::set_i64(o, "peer", peer as i64);
        return Ok(o);
    }
    Ok(Ref::NULL)
}

pub fn n_com_mojang_text2speech_NarratorLinux_FliteLibrary__flite_init() -> Res<i32> {
    let h = flite_lib();
    if !h.is_null() {
        let f = unsafe { dlsym(h, b"flite_init\0".as_ptr() as *const c_char) };
        if !f.is_null() {
            let func: extern "C" fn() -> c_int = unsafe { std::mem::transmute(f) };
            return Ok(func());
        }
    }
    Ok(0)
}
pub fn n_com_mojang_text2speech_NarratorLinux_FliteLibrary_CmuUsKal16__register_cmu_us_kal16(vox: Ref) -> Res<Ref> {
    let h = kal16_lib();
    if !h.is_null() {
        let f = unsafe { dlsym(h, b"register_cmu_us_kal16\0".as_ptr() as *const c_char) };
        if !f.is_null() {
            let cs = if vox.is_null() { CString::new("kal16").unwrap() } else { rt::c_str_arg(vox)? };
            let func: extern "C" fn(*const c_char) -> *mut c_void = unsafe { std::mem::transmute(f) };
            return jna_ptr(func(cs.as_ptr()) as usize);
        }
    }
    Ok(Ref::NULL)
}
pub fn n_com_mojang_text2speech_NarratorLinux_FliteLibrary__flite_synth_text(text: Ref, voice: Ref) -> Res<Ref> {
    let h = flite_lib();
    if !h.is_null() && !voice.is_null() {
        let f = unsafe { dlsym(h, b"flite_synth_text\0".as_ptr() as *const c_char) };
        if !f.is_null() {
            let cs = rt::c_str_arg(text)?;
            let vp = rt::get_i64(voice, "peer") as usize as *mut c_void;
            let func: extern "C" fn(*const c_char, *mut c_void) -> *mut c_void = unsafe { std::mem::transmute(f) };
            return jna_ptr(func(cs.as_ptr(), vp) as usize);
        }
    }
    Ok(Ref::NULL)
}
pub fn n_com_mojang_text2speech_NarratorLinux_FliteLibrary__utt_wave(utt: Ref) -> Res<Ref> {
    let h = flite_lib();
    if !h.is_null() && !utt.is_null() {
        let f = unsafe { dlsym(h, b"utt_wave\0".as_ptr() as *const c_char) };
        if !f.is_null() {
            let up = rt::get_i64(utt, "peer") as usize as *mut c_void;
            let func: extern "C" fn(*mut c_void) -> *mut c_void = unsafe { std::mem::transmute(f) };
            return jna_ptr(func(up) as usize);
        }
    }
    Ok(Ref::NULL)
}
pub fn n_com_mojang_text2speech_NarratorLinux_FliteLibrary__cst_wave_rescale(wave: Ref, fac: i32) -> Res<()> {
    let h = flite_lib();
    if !h.is_null() && !wave.is_null() {
        let f = unsafe { dlsym(h, b"cst_wave_rescale\0".as_ptr() as *const c_char) };
        if !f.is_null() {
            let wp = rt::get_i64(wave, "peer") as usize as *mut c_void;
            let func: extern "C" fn(*mut c_void, c_int) = unsafe { std::mem::transmute(f) };
            func(wp, fac);
        }
    }
    Ok(())
}
pub fn n_com_mojang_text2speech_NarratorLinux_FliteLibrary__play_wave(wave: Ref) -> Res<()> {
    let h = flite_lib();
    if !h.is_null() && !wave.is_null() {
        let f = unsafe { dlsym(h, b"play_wave\0".as_ptr() as *const c_char) };
        if !f.is_null() {
            let wp = rt::get_i64(wave, "peer") as usize as *mut c_void;
            let func: extern "C" fn(*mut c_void) = unsafe { std::mem::transmute(f) };
            let _blk = ScopedBlock::enter();
            func(wp);
        }
    }
    Ok(())
}
pub fn n_com_mojang_text2speech_NarratorLinux_FliteLibrary__delete_utterance(utt: Ref) -> Res<()> {
    let h = flite_lib();
    if !h.is_null() && !utt.is_null() {
        let f = unsafe { dlsym(h, b"delete_utterance\0".as_ptr() as *const c_char) };
        if !f.is_null() {
            let up = rt::get_i64(utt, "peer") as usize as *mut c_void;
            let func: extern "C" fn(*mut c_void) = unsafe { std::mem::transmute(f) };
            func(up);
        }
    }
    Ok(())
}
