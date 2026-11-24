//! Çalışma zamanının bildirdiği POSIX C fonksiyonları; Win32 API üzerinde Windows için gerçekleştirilmiştir. Yerel kodlar
//! bunları POSIX adlarıyla çağırır; bu modül tanımları sağlar ki Windows programı bağlansın. Her fonksiyon
//! POSIX hata kuralını döndürür ve Windows'un sunmadığı işler için errno'yu ayarlar (ENOSYS, 38). Böylece eksik bir özellik
//! sessizce başarılı olmak yerine çağrıda açıkça başarısız olur.

#![cfg(windows)]
#![allow(non_camel_case_types, clippy::missing_safety_doc)]

use std::ffi::{c_char, c_int, c_long, c_void, CStr};

const ENOSYS: c_int = 38;
const EINVAL: c_int = 22;
const ENOMEM: c_int = 12;

extern "C" {
    fn _set_errno(value: c_int) -> c_int;
}

fn fail_errno(code: c_int) {
    unsafe { _set_errno(code) };
}

// ----- dinamik yükleme (dlfcn.h): LoadLibrary / GetProcAddress üzerinden

#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryA(name: *const c_char) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
    fn FreeLibrary(module: *mut c_void) -> i32;
    fn GetLastError() -> u32;
    fn FormatMessageA(flags: u32, source: *const c_void, id: u32, lang: u32, buf: *mut c_char, size: u32, args: *const c_void) -> u32;
    fn VirtualAlloc(addr: *mut c_void, size: usize, alloc_type: u32, protect: u32) -> *mut c_void;
    fn VirtualFree(addr: *mut c_void, size: usize, free_type: u32) -> i32;
}

const RTLD_NOW: c_int = 2;
static mut DL_ERROR: [c_char; 256] = [0; 256];
static mut DL_ERROR_SET: bool = false;

fn remember_dl_error() {
    let code = unsafe { GetLastError() };
    let mut buf = [0 as c_char; 256];
    unsafe {
        let n = FormatMessageA(0x0000_1000 | 0x0000_0200, std::ptr::null(), code, 0, buf.as_mut_ptr(), buf.len() as u32, std::ptr::null());
        if n == 0 {
            let msg = b"dynamic loading failed\0";
            buf[..msg.len()].copy_from_slice(&msg.iter().map(|&b| b as c_char).collect::<Vec<_>>());
        }
        DL_ERROR = buf;
        DL_ERROR_SET = true;
    }
}

#[no_mangle]
pub unsafe extern "C" fn dlopen(filename: *const c_char, _flags: c_int) -> *mut c_void {
    let h = if filename.is_null() { std::ptr::null_mut() } else { LoadLibraryA(filename) };
    if h.is_null() {
        remember_dl_error();
    }
    h
}

#[no_mangle]
pub unsafe extern "C" fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void {
    let p = GetProcAddress(handle, symbol);
    if p.is_null() {
        remember_dl_error();
    }
    p
}

#[no_mangle]
pub unsafe extern "C" fn dlclose(handle: *mut c_void) -> c_int {
    if FreeLibrary(handle) != 0 { 0 } else { remember_dl_error(); -1 }
}

#[no_mangle]
pub unsafe extern "C" fn dlerror() -> *const c_char {
    if !DL_ERROR_SET {
        return std::ptr::null();
    }
    DL_ERROR_SET = false;
    CStr::from_ptr(DL_ERROR.as_ptr()).as_ptr()
}

// ----- sistem yapılandırması (unistd.h, sys/utsname.h)

const SC_PAGESIZE: c_int = 30; // glibc value; the runtime asks for it by this number

#[no_mangle]
pub unsafe extern "C" fn sysconf(name: c_int) -> c_long {
    match name {
        SC_PAGESIZE => 4096,
        _ => {
            fail_errno(EINVAL);
            -1
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn pathconf(_path: *const c_char, name: c_int) -> c_long {
    const PC_NAME_MAX: c_int = 3;
    match name {
        PC_NAME_MAX => 255, // NTFS file name limit
        _ => {
            fail_errno(EINVAL);
            -1
        }
    }
}

#[repr(C)]
pub struct utsname {
    pub sysname: [c_char; 65],
    pub nodename: [c_char; 65],
    pub release: [c_char; 65],
    pub version: [c_char; 65],
    pub machine: [c_char; 65],
    pub domainname: [c_char; 65],
}

fn fill(dst: &mut [c_char; 65], text: &str) {
    dst.fill(0);
    for (d, &b) in dst.iter_mut().zip(text.as_bytes().iter().take(64)) {
        *d = b as c_char;
    }
}

#[no_mangle]
pub unsafe extern "C" fn uname(buf: *mut utsname) -> c_int {
    if buf.is_null() {
        fail_errno(EINVAL);
        return -1;
    }
    let host = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "localhost".to_string());
    let u = &mut *buf;
    fill(&mut u.sysname, "Windows");
    fill(&mut u.nodename, &host);
    fill(&mut u.release, "");
    fill(&mut u.version, "");
    fill(&mut u.machine, if cfg!(target_arch = "x86_64") { "x86_64" } else { "aarch64" });
    fill(&mut u.domainname, "");
    0
}

// ----- sinyaller: Windows'ta sigaction yoktur; çalışma zamanının boş-işaretçi tuzağı bu platformda kurulmaz

#[no_mangle]
pub unsafe extern "C" fn sigaction(_sig: c_int, _act: *const u8, _oldact: *mut u8) -> c_int {
    fail_errno(ENOSYS);
    -1
}

// ----- bellek eşleme: anonim eşlemeler VirtualAlloc ile; dosya eşlemeleri burada desteklenmez

const PROT_READ: c_int = 1;
const PROT_WRITE: c_int = 2;
const MAP_PRIVATE: c_int = 0x02;
const MAP_ANONYMOUS: c_int = 0x20;
const MEM_COMMIT: u32 = 0x1000;
const MEM_RESERVE: u32 = 0x2000;
const MEM_RELEASE: u32 = 0x8000;
const PAGE_READONLY: u32 = 0x02;
const PAGE_READWRITE: u32 = 0x04;
const MAP_FAILED: isize = -1;

#[no_mangle]
pub unsafe extern "C" fn mmap(_addr: *mut c_void, length: usize, prot: c_int, flags: c_int, fd: c_int, _offset: i64) -> isize {
    if flags & MAP_ANONYMOUS == 0 || fd != -1 || flags & MAP_PRIVATE == 0 {
        fail_errno(ENOSYS);
        return MAP_FAILED;
    }
    let protect = if prot & PROT_WRITE != 0 {
        PAGE_READWRITE
    } else if prot & PROT_READ != 0 {
        PAGE_READONLY
    } else {
        PAGE_READONLY
    };
    let p = VirtualAlloc(std::ptr::null_mut(), length, MEM_RESERVE | MEM_COMMIT, protect);
    if p.is_null() {
        fail_errno(ENOMEM);
        return MAP_FAILED;
    }
    p as isize
}

#[no_mangle]
pub unsafe extern "C" fn munmap(addr: *mut c_void, _length: usize) -> c_int {
    if VirtualFree(addr, 0, MEM_RELEASE) != 0 {
        0
    } else {
        fail_errno(EINVAL);
        -1
    }
}

// ----- Winsock: soketler (sys/socket.h). POSIX fd, SOCKET değeridir. Windows tutamaçları 2^31'in altında 4'ün katlarıdır,
// bu yüzden değer, yerel kodların kullandığı C int'e sığar.

#[link(name = "ws2_32")]
extern "system" {
    fn WSAStartup(version: u16, data: *mut u8) -> c_int;
    fn WSAGetLastError() -> c_int;
    #[link_name = "socket"]
    fn ws_socket(af: c_int, ty: c_int, proto: c_int) -> usize;
    #[link_name = "bind"]
    fn ws_bind(s: usize, addr: *const c_void, len: c_int) -> c_int;
    #[link_name = "connect"]
    fn ws_connect(s: usize, addr: *const c_void, len: c_int) -> c_int;
    fn WSAPoll(fds: *mut WsaPollFd, count: u32, timeout: c_int) -> c_int;
    fn closesocket(s: usize) -> c_int;
}

#[repr(C)]
struct WsaPollFd {
    fd: usize,
    events: i16,
    revents: i16,
}

/// POSIX pollfd (fd bir int olarak), yerel kodların bildirdiği biçimde.
#[repr(C)]
pub struct PollFd {
    pub fd: c_int,
    pub events: i16,
    pub revents: i16,
}

const INVALID_SOCKET: usize = usize::MAX;

/// Winsock 2.2'yi süreç başına bir kez başlatır; başarısızlık her soket çağrısında bildirilir.
fn winsock_ready() -> Result<(), c_int> {
    static START: std::sync::OnceLock<c_int> = std::sync::OnceLock::new();
    let r = *START.get_or_init(|| {
        let mut data = [0u8; 512];
        unsafe { WSAStartup(0x0202, data.as_mut_ptr()) }
    });
    if r == 0 { Ok(()) } else { Err(r) }
}

/// Bir Winsock hata kodunu (WSA*) en yakın POSIX errno değerine çevirir ve errno'yu ayarlar.
fn set_winsock_errno() -> c_int {
    let wsa = unsafe { WSAGetLastError() };
    let e = match wsa {
        10004 => 4,    // WSAEINTR
        10009 => 9,    // WSAEBADF
        10013 => 13,   // WSAEACCES
        10014 => 14,   // WSAEFAULT
        10022 => 22,   // WSAEINVAL
        10024 => 24,   // WSAEMFILE
        10035 => 11,   // WSAEWOULDBLOCK
        10036 => 115,  // WSAEINPROGRESS
        10048 => 98,   // WSAEADDRINUSE
        10049 => 99,   // WSAEADDRNOTAVAIL
        10050 => 100,  // WSAENETDOWN
        10051 => 101,  // WSAENETUNREACH
        10054 => 104,  // WSAECONNRESET
        10060 => 110,  // WSAETIMEDOUT
        10061 => 111,  // WSAECONNREFUSED
        10038 => 88,   // WSAENOTSOCK
        _ => 5,        // EIO for anything without a direct equivalent
    };
    fail_errno(e);
    -1
}

#[no_mangle]
pub unsafe extern "C" fn socket(domain: c_int, ty: c_int, protocol: c_int) -> c_int {
    if winsock_ready().is_err() {
        return set_winsock_errno();
    }
    let s = ws_socket(domain, ty, protocol);
    if s == INVALID_SOCKET {
        return set_winsock_errno();
    }
    s as c_int
}

#[no_mangle]
pub unsafe extern "C" fn bind(sockfd: c_int, addr: *const c_void, addrlen: u32) -> c_int {
    if ws_bind(sockfd as usize, addr, addrlen as c_int) != 0 {
        return set_winsock_errno();
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn connect(sockfd: c_int, addr: *const c_void, addrlen: u32) -> c_int {
    if ws_connect(sockfd as usize, addr, addrlen as c_int) != 0 {
        return set_winsock_errno();
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn poll(fds: *mut PollFd, nfds: u64, timeout: c_int) -> c_int {
    if fds.is_null() && nfds != 0 {
        fail_errno(EINVAL);
        return -1;
    }
    let n = nfds as usize;
    let mut w: Vec<WsaPollFd> = (0..n)
        .map(|i| {
            let p = &*fds.add(i);
            WsaPollFd { fd: p.fd as usize, events: p.events, revents: 0 }
        })
        .collect();
    let r = WSAPoll(w.as_mut_ptr(), n as u32, timeout);
    if r < 0 {
        return set_winsock_errno();
    }
    for (i, p) in w.iter().enumerate() {
        (*fds.add(i)).revents = p.revents;
    }
    r
}

#[no_mangle]
pub unsafe extern "C" fn closesocket_posix(sockfd: c_int) -> c_int {
    if closesocket(sockfd as usize) != 0 {
        return set_winsock_errno();
    }
    0
}

// ----- süreçler: posix_spawnp CreateProcessW'yi çalıştırır; waitpid süreç tutamacını bekler

#[repr(C)]
#[derive(Clone, Copy)]
struct StartupInfoW {
    cb: u32,
    _reserved: usize,
    _desktop: usize,
    _title: usize,
    _x: u32,
    _y: u32,
    _xs: u32,
    _ys: u32,
    _xc: u32,
    _yc: u32,
    _fill: u32,
    _flags: u32,
    _show: u16,
    _cb2: u16,
    _cb3: usize,
    _stdin: usize,
    _stdout: usize,
    _stderr: usize,
}

#[repr(C)]
struct ProcessInformation {
    process: usize,
    thread: usize,
    pid: u32,
    tid: u32,
}

#[link(name = "kernel32")]
extern "system" {
    fn CreateProcessW(app: *const u16, cmd: *mut u16, pa: *const c_void, ta: *const c_void, inherit: i32, flags: u32,
                      env: *const u16, cwd: *const u16, si: *const StartupInfoW, pi: *mut ProcessInformation) -> i32;
    fn WaitForSingleObject(h: usize, ms: u32) -> u32;
    fn GetExitCodeProcess(h: usize, code: *mut u32) -> i32;
    fn CloseHandle(h: usize) -> i32;
}

const CREATE_UNICODE_ENVIRONMENT: u32 = 0x0000_0400;
const WNOHANG: c_int = 1;
const ECHILD: c_int = 10;

/// posix_spawnp ile başlatılan ve henüz toplanmamış her alt süreç için pid -> süreç tutamacı tablosu.
static CHILDREN: std::sync::Mutex<Vec<(c_int, usize)>> = std::sync::Mutex::new(Vec::new());

/// Bir argümanı Windows komut satırı için tırnaklar (CommandLineToArgvW kuralları).
fn quote_arg(a: &str, out: &mut String) {
    if !a.is_empty() && !a.contains([' ', '\t', '"']) {
        out.push_str(a);
        return;
    }
    out.push('"');
    let mut backslashes = 0;
    for c in a.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                out.extend(std::iter::repeat('\\').take(backslashes * 2 + 1));
                out.push('"');
                backslashes = 0;
            }
            _ => {
                out.extend(std::iter::repeat('\\').take(backslashes));
                out.push(c);
                backslashes = 0;
            }
        }
    }
    out.extend(std::iter::repeat('\\').take(backslashes * 2));
    out.push('"');
}

unsafe fn c_strings(list: *const *const c_char) -> Vec<String> {
    let mut v = Vec::new();
    let mut p = list;
    while !p.is_null() && !(*p).is_null() {
        v.push(CStr::from_ptr(*p).to_string_lossy().into_owned());
        p = p.add(1);
    }
    v
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Dosya eylemlerini 8 baytlık şeffaf bir kayıt olarak geçirir: ilk sözcük, Windows'un yapamadığı bir eylem
/// (dup2, close, chdir) kaydedildiğinde ayarlanır; böylece posix_spawnp eylemi yok saymak yerine başlatmayı reddeder.
const FA_UNSUPPORTED: u64 = 1;

#[no_mangle]
pub unsafe extern "C" fn posix_spawn_file_actions_init(fa: *mut c_void) -> c_int {
    if fa.is_null() {
        return EINVAL;
    }
    *(fa as *mut u64) = 0;
    0
}

#[no_mangle]
pub unsafe extern "C" fn posix_spawn_file_actions_destroy(fa: *mut c_void) -> c_int {
    if !fa.is_null() {
        *(fa as *mut u64) = 0;
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn posix_spawn_file_actions_adddup2(fa: *mut c_void, _fd: c_int, _newfd: c_int) -> c_int {
    if !fa.is_null() {
        *(fa as *mut u64) |= FA_UNSUPPORTED;
    }
    ENOSYS
}

#[no_mangle]
pub unsafe extern "C" fn posix_spawn_file_actions_addclose(fa: *mut c_void, _fd: c_int) -> c_int {
    if !fa.is_null() {
        *(fa as *mut u64) |= FA_UNSUPPORTED;
    }
    ENOSYS
}

#[no_mangle]
pub unsafe extern "C" fn posix_spawn_file_actions_addchdir_np(fa: *mut c_void, _path: *const c_char) -> c_int {
    if !fa.is_null() {
        *(fa as *mut u64) |= FA_UNSUPPORTED;
    }
    ENOSYS
}

#[no_mangle]
pub unsafe extern "C" fn posix_spawnp(pid: *mut c_int, file: *const c_char, fa: *const c_void, _attr: *const c_void,
                                      argv: *const *const c_char, envp: *const *const c_char) -> c_int {
    if !fa.is_null() && *(fa as *const u64) & FA_UNSUPPORTED != 0 {
        return ENOSYS;
    }
    if file.is_null() {
        return EINVAL;
    }
    let program = CStr::from_ptr(file).to_string_lossy().into_owned();
    let mut args = c_strings(argv);
    if args.is_empty() {
        args.push(program.clone());
    }
    let mut cmd = String::new();
    for (i, a) in args.iter().enumerate() {
        if i > 0 {
            cmd.push(' ');
        }
        quote_arg(a, &mut cmd);
    }
    let mut cmd_w = wide(&cmd);
    let prog_w = wide(&program);
    // envp: NUL ile biten "AD=değer" girdilerinden oluşan bir blok; ek bir NUL ile biter. NULL ise miras alınır
    let env_block: Option<Vec<u16>> = if envp.is_null() {
        None
    } else {
        let mut block = Vec::new();
        for e in c_strings(envp) {
            block.extend(e.encode_utf16());
            block.push(0);
        }
        block.push(0);
        Some(block)
    };
    let si = StartupInfoW {
        cb: std::mem::size_of::<StartupInfoW>() as u32,
        _reserved: 0, _desktop: 0, _title: 0, _x: 0, _y: 0, _xs: 0, _ys: 0, _xc: 0, _yc: 0, _fill: 0,
        _flags: 0, _show: 0, _cb2: 0, _cb3: 0, _stdin: 0, _stdout: 0, _stderr: 0,
    };
    let mut pi = ProcessInformation { process: 0, thread: 0, pid: 0, tid: 0 };
    let flags = if env_block.is_some() { CREATE_UNICODE_ENVIRONMENT } else { 0 };
    let env_ptr = env_block.as_ref().map_or(std::ptr::null(), |b| b.as_ptr());
    let ok = CreateProcessW(
        prog_w.as_ptr(), cmd_w.as_mut_ptr(), std::ptr::null(), std::ptr::null(), 0, flags,
        env_ptr as *const u16, std::ptr::null(), &si, &mut pi,
    );
    if ok == 0 {
        let code = GetLastError();
        // ERROR_FILE_NOT_FOUND (2) ve ERROR_PATH_NOT_FOUND (3), ENOENT anlamına gelir
        fail_errno(if code == 2 || code == 3 { 2 } else { 13 });
        return if code == 2 || code == 3 { 2 } else { 13 };
    }
    CloseHandle(pi.thread);
    if let Ok(mut ch) = CHILDREN.lock() {
        ch.push((pi.pid as c_int, pi.process));
    }
    if !pid.is_null() {
        *pid = pi.pid as c_int;
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn waitpid(pid: c_int, status: *mut c_int, options: c_int) -> c_int {
    let entry = {
        let ch = match CHILDREN.lock() {
            Ok(c) => c,
            Err(_) => {
                fail_errno(ECHILD);
                return -1;
            }
        };
        ch.iter().position(|&(p, _)| p == pid || pid <= 0).map(|i| ch[i])
    };
    let Some((p, handle)) = entry else {
        fail_errno(ECHILD);
        return -1;
    };
    const WAIT_OBJECT_0: u32 = 0;
    const INFINITE: u32 = 0xFFFF_FFFF;
    let wait_ms = if options & WNOHANG != 0 { 0 } else { INFINITE };
    match WaitForSingleObject(handle, wait_ms) {
        WAIT_OBJECT_0 => {
            let mut code = 0u32;
            GetExitCodeProcess(handle, &mut code);
            CloseHandle(handle);
            if let Ok(mut ch) = CHILDREN.lock() {
                ch.retain(|&(q, _)| q != p);
            }
            if !status.is_null() {
                // WIFEXITED / WEXITSTATUS kodlaması: çıkış kodu ikinci bayttadır
                *status = ((code & 0xFF) << 8) as c_int;
            }
            p
        }
        0x102 => 0, // WAIT_TIMEOUT: WNOHANG and the child is still running
        _ => {
            fail_errno(ECHILD);
            -1
        }
    }
}

// ----- çalışma zamanının kullanımında Windows karşılığı olmayan fonksiyonlar: ENOSYS ile başarısız olurlar

macro_rules! unsupported {
    ($( fn $name:ident ( $($arg:ident : $ty:ty),* ) -> $ret:ty; )*) => {
        $(
            #[no_mangle]
            pub unsafe extern "C" fn $name($($arg: $ty),*) -> $ret {
                $( let _ = $arg; )*
                fail_errno(ENOSYS);
                <$ret>::from(-1i8)
            }
        )*
    };
}

unsupported! {
    fn pipe2(fds: *mut c_int, flags: c_int) -> c_int;
    fn socketpair(domain: c_int, ty: c_int, protocol: c_int, sv: *mut c_int) -> c_int;
    fn getaddrinfo(node: *const c_char, service: *const c_char, hints: *const c_void, res: *mut *mut c_void) -> c_int;
    fn readv(fd: c_int, iov: *const c_void, iovcnt: c_int) -> isize;
    fn writev(fd: c_int, iov: *const c_void, iovcnt: c_int) -> isize;
    fn sendfile(out_fd: c_int, in_fd: c_int, offset: *mut i64, count: usize) -> isize;
    fn splice(fd_in: c_int, off_in: *mut i64, fd_out: c_int, off_out: *mut i64, len: usize, flags: u32) -> isize;
    fn timerfd_create(clockid: c_int, flags: c_int) -> c_int;
    fn timerfd_settime(fd: c_int, flags: c_int, new_val: *const c_void, old_val: *mut c_void) -> c_int;
    fn openat(dirfd: c_int, pathname: *const c_char, flags: c_int) -> c_int;
    fn unlinkat(dirfd: c_int, pathname: *const c_char, flags: c_int) -> c_int;
    fn renameat(olddirfd: c_int, oldpath: *const c_char, newdirfd: c_int, newpath: *const c_char) -> c_int;
    fn fstatat(dirfd: c_int, pathname: *const c_char, statbuf: *mut c_void, flags: c_int) -> c_int;
    fn utimensat(dirfd: c_int, pathname: *const c_char, times: *const c_void, flags: c_int) -> c_int;
    fn utimes(filename: *const c_char, times: *const c_void) -> c_int;
    fn symlink(target: *const c_char, linkpath: *const c_char) -> c_int;
    fn readlink(pathname: *const c_char, buf: *mut c_char, bufsiz: usize) -> isize;
    fn statvfs(path: *const c_char, buf: *mut c_void) -> c_int;
    fn fstatvfs(fd: c_int, buf: *mut c_void) -> c_int;
    fn posix_fadvise(fd: c_int, offset: i64, len: i64, advice: c_int) -> c_int;
    fn pread(fd: c_int, buf: *mut c_void, count: usize, offset: i64) -> isize;
    fn pwrite(fd: c_int, buf: *const c_void, count: usize, offset: i64) -> isize;
    fn msync(addr: *mut c_void, length: usize, flags: c_int) -> c_int;
    fn sysinfo(info: *mut c_void) -> c_int;
}

/// realpath: NULL sonuç ve errno ENOSYS (Windows'ta çalışma zamanı yolları kendisi çözer).
#[no_mangle]
pub unsafe extern "C" fn realpath(_path: *const c_char, _resolved: *mut c_char) -> *mut c_char {
    fail_errno(ENOSYS);
    std::ptr::null_mut()
}

/// setmntent: Windows'ta bağlama tablosu yoktur; NULL ve errno ENOSYS.
#[no_mangle]
pub unsafe extern "C" fn setmntent(_filename: *const c_char, _ty: *const c_char) -> *mut c_void {
    fail_errno(ENOSYS);
    std::ptr::null_mut()
}

// ----- stat ailesi: GetFileInformationByHandle ile doldurulur; yerel kodların kullandığı glibc x86_64 struct stat düzeninde.

#[repr(C)]
struct ByHandleFileInformation {
    file_attributes: u32,
    creation_time: u64,
    last_access_time: u64,
    last_write_time: u64,
    volume_serial: u32,
    size_high: u32,
    size_low: u32,
    number_of_links: u32,
    index_high: u32,
    index_low: u32,
}

#[repr(C)]
struct GlibcStat {
    st_dev: u64,
    st_ino: u64,
    st_nlink: u64,
    st_mode: u32,
    st_uid: u32,
    st_gid: u32,
    pad0: i32,
    st_rdev: u64,
    st_size: i64,
    st_blksize: i64,
    st_blocks: i64,
    st_atim_sec: i64,
    st_atim_nsec: i64,
    st_mtim_sec: i64,
    st_mtim_nsec: i64,
    st_ctim_sec: i64,
    st_ctim_nsec: i64,
    unused: [i64; 3],
}

#[link(name = "kernel32")]
extern "system" {
    fn CreateFileW(name: *const u16, access: u32, share: u32, sa: *const c_void, disp: u32, flags: u32, tmpl: usize) -> usize;
    fn GetFileInformationByHandle(h: usize, info: *mut ByHandleFileInformation) -> i32;
}

extern "C" {
    fn _get_osfhandle(fd: c_int) -> isize;
    fn _errno() -> *mut c_int;
}

const FILE_READ_ATTRIBUTES: u32 = 0x0080;
const FILE_SHARE_ALL: u32 = 0x0000_0007;
const OPEN_EXISTING: u32 = 3;
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000; // directories can be opened
const FILE_ATTRIBUTE_READONLY: u32 = 0x1;
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
const S_IFDIR: u32 = 0o040000;
const S_IFREG: u32 = 0o100000;
const INVALID_HANDLE_VALUE: usize = usize::MAX;

/// FILETIME (1601'den beri 100 ns) değerini (saniye, 1970'ten beri nanosaniye) çiftine çevirir.
fn filetime_split(ft: u64) -> (i64, i64) {
    let t = ft as i64 - 116_444_736_000_000_000;
    (t.div_euclid(10_000_000), t.rem_euclid(10_000_000) * 100)
}

fn fill_stat(info: &ByHandleFileInformation, out: *mut GlibcStat) {
    let dir = info.file_attributes & FILE_ATTRIBUTE_DIRECTORY != 0;
    let mut perm: u32 = if dir { 0o755 } else { 0o644 };
    if info.file_attributes & FILE_ATTRIBUTE_READONLY != 0 {
        perm &= !0o222;
    }
    let size = ((info.size_high as u64) << 32) | info.size_low as u64;
    let ino = ((info.index_high as u64) << 32) | info.index_low as u64;
    let (at_s, at_ns) = filetime_split(info.last_access_time);
    let (mt_s, mt_ns) = filetime_split(info.last_write_time);
    let (ct_s, ct_ns) = filetime_split(info.creation_time);
    unsafe {
        *out = GlibcStat {
            st_dev: info.volume_serial as u64,
            st_ino: ino,
            st_nlink: info.number_of_links as u64,
            st_mode: (if dir { S_IFDIR } else { S_IFREG }) | perm,
            st_uid: 0,
            st_gid: 0,
            pad0: 0,
            st_rdev: 0,
            st_size: size as i64,
            st_blksize: 4096,
            st_blocks: ((size + 511) / 512) as i64,
            st_atim_sec: at_s,
            st_atim_nsec: at_ns,
            st_mtim_sec: mt_s,
            st_mtim_nsec: mt_ns,
            st_ctim_sec: ct_s,
            st_ctim_nsec: ct_ns,
            unused: [0; 3],
        };
    }
}

fn set_last_error_errno(code: u32) -> c_int {
    // ERROR_FILE_NOT_FOUND 2, ERROR_PATH_NOT_FOUND 3 -> ENOENT; ERROR_ACCESS_DENIED 5 -> EACCES; diğerleri EIO
    let e = match code {
        2 | 3 => 2,
        5 => 13,
        _ => 5,
    };
    fail_errno(e);
    -1
}

#[no_mangle]
pub unsafe extern "C" fn stat(path: *const c_char, buf: *mut GlibcStat) -> c_int {
    if path.is_null() || buf.is_null() {
        fail_errno(EINVAL);
        return -1;
    }
    let p = CStr::from_ptr(path).to_string_lossy().into_owned();
    let w = wide(&p);
    let h = CreateFileW(w.as_ptr(), FILE_READ_ATTRIBUTES, FILE_SHARE_ALL, std::ptr::null(), OPEN_EXISTING,
                        FILE_FLAG_BACKUP_SEMANTICS, 0);
    if h == INVALID_HANDLE_VALUE {
        return set_last_error_errno(GetLastError());
    }
    let mut info: ByHandleFileInformation = std::mem::zeroed();
    let ok = GetFileInformationByHandle(h, &mut info);
    let err = GetLastError();
    CloseHandle(h);
    if ok == 0 {
        return set_last_error_errno(err);
    }
    fill_stat(&info, buf);
    0
}

#[no_mangle]
pub unsafe extern "C" fn lstat(path: *const c_char, buf: *mut GlibcStat) -> c_int {
    // Windows sembolik bağlantıları, işaret ettikleri dosya olarak bildirilir
    stat(path, buf)
}

#[no_mangle]
pub unsafe extern "C" fn fstat(fd: c_int, buf: *mut GlibcStat) -> c_int {
    if buf.is_null() {
        fail_errno(EINVAL);
        return -1;
    }
    let h = _get_osfhandle(fd);
    if h == -1 {
        fail_errno(9); // EBADF
        return -1;
    }
    let mut info: ByHandleFileInformation = std::mem::zeroed();
    if GetFileInformationByHandle(h as usize, &mut info) == 0 {
        return set_last_error_errno(GetLastError());
    }
    fill_stat(&info, buf);
    0
}

/// glibc'nin errno erişimcisi; C çalışma zamanının errno'su üzerinden gerçekleştirilmiştir.
#[no_mangle]
pub unsafe extern "C" fn __errno_location() -> *mut c_int {
    _errno()
}

// ----- yalnızca Linux'a özgü ve bu platformda henüz kullanımı olmayan ağ çağrıları: ENOSYS ile başarısız olurlar

unsupported! {
    fn epoll_create(size: c_int) -> c_int;
    fn epoll_create1(flags: c_int) -> c_int;
    fn epoll_ctl(epfd: c_int, op: c_int, fd: c_int, event: *mut c_void) -> c_int;
    fn epoll_wait(epfd: c_int, events: *mut c_void, maxevents: c_int, timeout: c_int) -> c_int;
    fn eventfd(init: u32, flags: c_int) -> c_int;
    fn inotify_init() -> c_int;
    fn inotify_add_watch(fd: c_int, path: *const c_char, mask: u32) -> c_int;
    fn inotify_rm_watch(fd: c_int, wd: c_int) -> c_int;
    fn accept4(fd: c_int, addr: *mut c_void, len: *mut u32, flags: c_int) -> c_int;
    fn getsockname(fd: c_int, addr: *mut c_void, len: *mut u32) -> c_int;
    fn getpeername(fd: c_int, addr: *mut c_void, len: *mut u32) -> c_int;
    fn getsockopt(fd: c_int, level: c_int, name: c_int, val: *mut c_void, len: *mut u32) -> c_int;
    fn setsockopt(fd: c_int, level: c_int, name: c_int, val: *const c_void, len: u32) -> c_int;
    fn listen(fd: c_int, backlog: c_int) -> c_int;
    fn shutdown(fd: c_int, how: c_int) -> c_int;
    fn send(fd: c_int, buf: *const c_void, len: usize, flags: c_int) -> isize;
    fn recv(fd: c_int, buf: *mut c_void, len: usize, flags: c_int) -> isize;
    fn sendto(fd: c_int, buf: *const c_void, len: usize, flags: c_int, addr: *const c_void, alen: u32) -> isize;
    fn recvfrom(fd: c_int, buf: *mut c_void, len: usize, flags: c_int, addr: *mut c_void, alen: *mut u32) -> isize;
    fn getnameinfo(addr: *const c_void, alen: u32, host: *mut c_char, hlen: u32, serv: *mut c_char, slen: u32, flags: c_int) -> c_int;
    fn getifaddrs(ifap: *mut *mut c_void) -> c_int;
    fn gethostname(name: *mut c_char, len: usize) -> c_int;
    fn tcgetattr(fd: c_int, t: *mut c_void) -> c_int;
    fn tcsetattr(fd: c_int, act: c_int, t: *const c_void) -> c_int;
    fn ioctl(fd: c_int, req: u64, arg: usize) -> c_int;
    fn fcntl(fd: c_int, cmd: c_int, arg: usize) -> c_int;
    fn getrlimit(res: c_int, rlim: *mut c_void) -> c_int;
    fn getloadavg(loadavg: *mut f64, n: c_int) -> c_int;
    fn sigaltstack(ss: *const c_void, old: *mut c_void) -> c_int;
    fn kill(pid: c_int, sig: c_int) -> c_int;
    fn madvise(addr: *mut c_void, len: usize, advice: c_int) -> c_int;
    fn mincore(addr: *mut c_void, len: usize, vec: *mut u8) -> c_int;
    fn fchmodat(dirfd: c_int, path: *const c_char, mode: u32, flags: c_int) -> c_int;
    fn fchown(fd: c_int, uid: u32, gid: u32) -> c_int;
    fn lchown(path: *const c_char, uid: u32, gid: u32) -> c_int;
    fn chown(path: *const c_char, uid: u32, gid: u32) -> c_int;
    fn mknod(path: *const c_char, mode: u32, dev: u64) -> c_int;
    fn link(old: *const c_char, new: *const c_char) -> c_int;
    fn fdatasync(fd: c_int) -> c_int;
    fn futimens(fd: c_int, times: *const c_void) -> c_int;
    fn endmntent(f: *mut c_void) -> c_int;
    fn backtrace(buf: *mut *mut c_void, size: c_int) -> c_int;
    fn syscall(number: i64, a: i64, b: i64, c: i64) -> i64;
}

// GNU hedefi pthread_kill'i libmingw32 (thread.obj) içinden alır; kendi kopyamızı orada dışa aktarmak yinelenen sembol olur.
#[cfg(not(target_env = "gnu"))]
unsupported! {
    fn pthread_kill(t: usize, sig: c_int) -> c_int;
}

/// Windows karşılığı olmayan, işaretçi döndüren fonksiyonlar: errno ENOSYS ile NULL.
#[no_mangle]
pub unsafe extern "C" fn gai_strerror(_code: c_int) -> *const c_char {
    b"address resolution is not available on this platform\0".as_ptr() as *const c_char
}

#[no_mangle]
pub unsafe extern "C" fn fdopendir(_fd: c_int) -> *mut c_void {
    fail_errno(ENOSYS);
    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn getmntent_r(_f: *mut c_void, _ent: *mut c_void, _buf: *mut c_char, _len: c_int) -> *mut c_void {
    fail_errno(ENOSYS);
    std::ptr::null_mut()
}

/// Windows karşılığı olmayan, void fonksiyonlar: yalnızca errno'yu ayarlarlar.
#[no_mangle]
pub unsafe extern "C" fn freeaddrinfo(_res: *mut c_void) {
    fail_errno(ENOSYS);
}

#[no_mangle]
pub unsafe extern "C" fn freeifaddrs(_ifa: *mut c_void) {
    fail_errno(ENOSYS);
}

#[no_mangle]
pub unsafe extern "C" fn backtrace_symbols_fd(_buf: *const *mut c_void, _size: c_int, _fd: c_int) {
    fail_errno(ENOSYS);
}

/// if_nametoindex: 0 (böyle bir arayüz yok) ve errno ENOSYS.
#[no_mangle]
pub unsafe extern "C" fn if_nametoindex(_name: *const c_char) -> u32 {
    fail_errno(ENOSYS);
    0
}

// ----- çalışma zamanının çağırdığı POSIX adları; UCRT bunları o adlarla dışa aktarmaz

extern "C" {
    fn _commit(fd: c_int) -> c_int;
    fn fgetc(stream: *mut c_void) -> c_int;
    fn malloc(size: usize) -> *mut c_void;
    fn realloc(p: *mut c_void, size: usize) -> *mut c_void;
    fn _localtime64_s(out: *mut c_void, t: *const i64) -> c_int;
    fn _get_timezone(seconds: *mut c_long) -> c_int;
    fn _get_dstbias(seconds: *mut c_long) -> c_int;
}

/// fsync: UCRT _commit, dosyanın tamponlarını diske yazar.
#[no_mangle]
pub unsafe extern "C" fn fsync(fd: c_int) -> c_int {
    _commit(fd)
}

/// Çalışma zamanının `Tm` yapısıyla (natives_io.rs) aynı alan sırası: dokuz tamsayı, tm_gmtoff, tm_zone.
#[repr(C)]
pub struct TmOut {
    fields: [c_int; 9],
    gmtoff: c_long,
    zone: *const c_char,
}

/// localtime_r: yerel parçalanmış zaman. tm_gmtoff, UTC'den saniye cinsinden farktır (yaz saati dahil);
/// tm_zone NULL'dır, çünkü UCRT bölge adı vermez.
#[no_mangle]
pub unsafe extern "C" fn localtime_r(t: *const i64, result: *mut TmOut) -> *mut TmOut {
    if _localtime64_s(result as *mut c_void, t) != 0 {
        fail_errno(EINVAL);
        return std::ptr::null_mut();
    }
    let mut timezone: c_long = 0;
    let mut dstbias: c_long = 0;
    if _get_timezone(&mut timezone) != 0 || _get_dstbias(&mut dstbias) != 0 {
        fail_errno(EINVAL);
        return std::ptr::null_mut();
    }
    let dst = (*result).fields[8] > 0;
    (*result).gmtoff = -timezone - if dst { dstbias } else { 0 };
    (*result).zone = std::ptr::null();
    result
}

/// getline: bir satırı ('\n' dahil) realloc ile büyütülen bir tampona okur. Dosya sonunda -1.
#[no_mangle]
pub unsafe extern "C" fn getline(lineptr: *mut *mut u8, n: *mut usize, stream: *mut c_void) -> isize {
    if lineptr.is_null() || n.is_null() {
        fail_errno(EINVAL);
        return -1;
    }
    if (*lineptr).is_null() || *n == 0 {
        let p = malloc(128) as *mut u8;
        if p.is_null() {
            fail_errno(ENOMEM);
            return -1;
        }
        *lineptr = p;
        *n = 128;
    }
    let mut len = 0usize;
    loop {
        let c = fgetc(stream);
        if c == -1 {
            if len == 0 {
                return -1;
            }
            break;
        }
        if len + 2 > *n {
            let cap = (*n * 2).max(len + 2);
            let p = realloc(*lineptr as *mut c_void, cap) as *mut u8;
            if p.is_null() {
                fail_errno(ENOMEM);
                return -1;
            }
            *lineptr = p;
            *n = cap;
        }
        *(*lineptr).add(len) = c as u8;
        len += 1;
        if c == b'\n' as c_int {
            break;
        }
    }
    *(*lineptr).add(len) = 0;
    len as isize
}

/// Windows'ta gerçek zamanlı sinyal yoktur: aralık boştur.
#[no_mangle]
pub extern "C" fn __libc_current_sigrtmax() -> c_int {
    0
}

/// fchmod: Windows dosyalarında POSIX kip bitleri yoktur; taklit etmek yerine ENOSYS ile başarısız olur.
unsupported! {
    fn fchmod(fd: c_int, mode: u32) -> c_int;
}
