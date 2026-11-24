//! NIO dosya sistemi ve kanal yerel metotları (UnixNativeDispatcher, LinuxNativeDispatcher,
//! LinuxWatchService, UnixFileDispatcherImpl, FileDispatcherImpl, EPoll, EventFD,
//! IOUtil, NativeThread, FileKey, UnixDispatcher, MappedMemoryUtils).
use crate::heap::ScopedBlock;
use crate::rt::{self, Arr, Ref, Res};
use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_long, c_uint, c_ulong, c_void};
use std::sync::atomic::{AtomicI32, AtomicPtr, Ordering};

#[cfg(target_arch = "x86_64")]
#[repr(C)]
struct Stat {
    st_dev: u64,
    st_ino: u64,
    st_nlink: u64,
    st_mode: u32,
    st_uid: u32,
    st_gid: u32,
    __pad0: i32,
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
    __unused: [i64; 3],
}

#[cfg(not(target_arch = "x86_64"))]
#[repr(C)]
struct Stat {
    st_dev: u64,
    st_ino: u64,
    st_mode: u32,
    st_nlink: u32,
    st_uid: u32,
    st_gid: u32,
    st_rdev: u64,
    __pad1: u64,
    st_size: i64,
    st_blksize: i32,
    __pad2: i32,
    st_blocks: i64,
    st_atim_sec: i64,
    st_atim_nsec: i64,
    st_mtim_sec: i64,
    st_mtim_nsec: i64,
    st_ctim_sec: i64,
    st_ctim_nsec: i64,
    __unused: [i32; 2],
}

#[repr(C)]
struct StatVfs {
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
    __f_spare: [c_int; 6],
}

#[repr(C)]
struct Timespec {
    tv_sec: i64,
    tv_nsec: i64,
}

#[repr(C)]
struct Flock {
    l_type: i16,
    l_whence: i16,
    l_start: i64,
    l_len: i64,
    l_pid: i32,
}

#[repr(C)]
struct MntEnt {
    mnt_fsname: *mut c_char,
    mnt_dir: *mut c_char,
    mnt_type: *mut c_char,
    mnt_opts: *mut c_char,
    mnt_freq: c_int,
    mnt_passno: c_int,
}

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
struct PollFd {
    fd: c_int,
    events: i16,
    revents: i16,
}

#[cfg_attr(unix, link(name = "c"))]
extern "C" {
    fn __errno_location() -> *mut c_int;
    fn strerror(errnum: c_int) -> *const c_char;
    fn free(ptr: *mut c_void);
    fn getcwd(buf: *mut c_char, size: usize) -> *mut c_char;
    fn dup(oldfd: c_int) -> c_int;
    fn dup2(oldfd: c_int, newfd: c_int) -> c_int;
    fn open(pathname: *const c_char, flags: c_int, ...) -> c_int;
    fn openat(dirfd: c_int, pathname: *const c_char, flags: c_int, ...) -> c_int;
    fn close(fd: c_int) -> c_int;
    fn rewind(stream: *mut c_void);
    fn getline(lineptr: *mut *mut u8, n: *mut usize, stream: *mut c_void) -> isize;
    fn link(oldpath: *const c_char, newpath: *const c_char) -> c_int;
    fn unlink(pathname: *const c_char) -> c_int;
    fn unlinkat(dirfd: c_int, pathname: *const c_char, flags: c_int) -> c_int;
    fn mknod(pathname: *const c_char, mode: u32, dev: u64) -> c_int;
    fn rename(oldpath: *const c_char, newpath: *const c_char) -> c_int;
    fn renameat(olddirfd: c_int, oldpath: *const c_char, newdirfd: c_int, newpath: *const c_char) -> c_int;
    fn mkdir(pathname: *const c_char, mode: u32) -> c_int;
    fn rmdir(pathname: *const c_char) -> c_int;
    fn readlink(pathname: *const c_char, buf: *mut c_char, bufsiz: usize) -> isize;
    fn realpath(path: *const c_char, resolved_path: *mut c_char) -> *mut c_char;
    fn symlink(target: *const c_char, linkpath: *const c_char) -> c_int;
    fn stat(pathname: *const c_char, statbuf: *mut Stat) -> c_int;
    fn lstat(pathname: *const c_char, statbuf: *mut Stat) -> c_int;
    fn fstat(fd: c_int, statbuf: *mut Stat) -> c_int;
    fn fstatat(dirfd: c_int, pathname: *const c_char, statbuf: *mut Stat, flags: c_int) -> c_int;
    fn chown(pathname: *const c_char, owner: u32, group: u32) -> c_int;
    fn lchown(pathname: *const c_char, owner: u32, group: u32) -> c_int;
    fn fchown(fd: c_int, owner: u32, group: u32) -> c_int;
    fn chmod(pathname: *const c_char, mode: u32) -> c_int;
    fn fchmod(fd: c_int, mode: u32) -> c_int;
    fn fchmodat(dirfd: c_int, pathname: *const c_char, mode: u32, flags: c_int) -> c_int;
    fn futimens(fd: c_int, times: *const Timespec) -> c_int;
    fn utimensat(dirfd: c_int, pathname: *const c_char, times: *const Timespec, flags: c_int) -> c_int;
    fn opendir(name: *const c_char) -> *mut c_void;
    fn fdopendir(fd: c_int) -> *mut c_void;
    fn closedir(dirp: *mut c_void) -> c_int;
    fn readdir(dirp: *mut c_void) -> *const u8;
    fn read(fd: c_int, buf: *mut c_void, count: usize) -> isize;
    fn write(fd: c_int, buf: *const c_void, count: usize) -> isize;
    fn pread(fd: c_int, buf: *mut c_void, count: usize, offset: i64) -> isize;
    fn pwrite(fd: c_int, buf: *const c_void, count: usize, offset: i64) -> isize;
    fn readv(fd: c_int, iov: *const c_void, iovcnt: c_int) -> isize;
    fn writev(fd: c_int, iov: *const c_void, iovcnt: c_int) -> isize;
    fn access(pathname: *const c_char, mode: c_int) -> c_int;
    fn statvfs(path: *const c_char, buf: *mut StatVfs) -> c_int;
    fn fstatvfs(fd: c_int, buf: *mut StatVfs) -> c_int;
    fn setmntent(filename: *const c_char, ty: *const c_char) -> *mut c_void;
    fn getmntent_r(stream: *mut c_void, mntbuf: *mut MntEnt, buf: *mut c_char, buflen: c_int) -> *mut MntEnt;
    fn endmntent(streamp: *mut c_void) -> c_int;
    fn posix_fadvise(fd: c_int, offset: i64, len: i64, advice: c_int) -> c_int;
    fn fsync(fd: c_int) -> c_int;
    fn fdatasync(fd: c_int) -> c_int;
    fn lseek(fd: c_int, offset: i64, whence: c_int) -> i64;
    fn ftruncate(fd: c_int, length: i64) -> c_int;
    fn fcntl(fd: c_int, cmd: c_int, ...) -> c_int;
    fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
    fn pipe2(pipefd: *mut c_int, flags: c_int) -> c_int;
    fn socketpair(domain: c_int, ty: c_int, protocol: c_int, sv: *mut c_int) -> c_int;
    fn poll(fds: *mut PollFd, nfds: c_ulong, timeout: c_int) -> c_int;
    fn mmap(addr: *mut c_void, length: usize, prot: c_int, flags: c_int, fd: c_int, offset: i64) -> isize;
    fn munmap(addr: *mut c_void, length: usize) -> c_int;
    fn sysconf(name: c_int) -> c_long;
    fn getrlimit(resource: c_int, rlim: *mut [u64; 2]) -> c_int;
    fn sendfile(out_fd: c_int, in_fd: c_int, offset: *mut i64, count: usize) -> isize;
    fn syscall(number: c_long, ...) -> c_long;
    fn epoll_create(size: c_int) -> c_int;
    fn epoll_ctl(epfd: c_int, op: c_int, fd: c_int, event: *const EpollEvent) -> c_int;
    fn epoll_wait(epfd: c_int, events: *mut EpollEvent, maxevents: c_int, timeout: c_int) -> c_int;
    fn eventfd(initval: c_uint, flags: c_int) -> c_int;
    fn inotify_init() -> c_int;
    fn inotify_add_watch(fd: c_int, pathname: *const c_char, mask: u32) -> c_int;
    fn inotify_rm_watch(fd: c_int, wd: c_int) -> c_int;
    fn mincore(addr: *mut c_void, length: usize, vec: *mut u8) -> c_int;
    fn madvise(addr: *mut c_void, length: usize, advice: c_int) -> c_int;
    fn msync(addr: *mut c_void, length: usize, flags: c_int) -> c_int;
    fn pthread_self() -> usize;
    fn pthread_kill(thread: usize, sig: c_int) -> c_int;
    fn __libc_current_sigrtmax() -> c_int;
    fn sigaction(signum: c_int, act: *const u8, oldact: *mut u8) -> c_int;
}

const IOS_EOF: i32 = -1;
const IOS_UNAVAILABLE: i32 = -2;
const IOS_INTERRUPTED: i32 = -3;
const IOS_UNSUPPORTED_CASE: i32 = -6;

const EINTR: i32 = 4;
const EBADF: i32 = 9;
const EAGAIN: i32 = 11;
const ENOMEM: i32 = 12;
const EACCES: i32 = 13;
const EXDEV: i32 = 18;
const EINVAL: i32 = 22;
const ENOSYS: i32 = 38;
const EOPNOTSUPP: i32 = 95;

#[cfg(target_arch = "x86_64")]
const SYS_COPY_FILE_RANGE: c_long = 326;
#[cfg(target_arch = "aarch64")]
const SYS_COPY_FILE_RANGE: c_long = 285;
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
const SYS_COPY_FILE_RANGE: c_long = -1;

static PRE_CLOSE_FD: AtomicI32 = AtomicI32::new(-1);

#[inline(always)]
fn errno() -> i32 {
    unsafe { *__errno_location() }
}

#[inline(always)]
fn set_errno(e: i32) {
    unsafe { *__errno_location() = e; }
}

fn c_err_str(e: i32) -> String {
    unsafe { CStr::from_ptr(strerror(e)).to_string_lossy().into_owned() }
}

fn unix_fail<T>() -> Res<T> {
    rt::throw_unix_errno(errno())
}

fn unix_check(rc: c_int) -> Res<()> {
    if rc == -1 { unix_fail() } else { Ok(()) }
}

fn io_fail<T>() -> Res<T> {
    rt::throw("java/io/IOException", c_err_str(errno()))
}

#[inline(always)]
fn cpath(p: i64) -> *const c_char {
    p as usize as *const c_char
}

fn new_bytes(b: &[u8]) -> Res<Ref> {
    let a = rt::new_array(b'B', b.len() as i32)?;
    unsafe { rt::byte_slice_mut(a)?.copy_from_slice(b) };
    Ok(a)
}

fn convert_ret(n: isize, reading: bool) -> Res<i32> {
    if n > 0 { return Ok(n as i32); }
    if n == 0 { return Ok(if reading { IOS_EOF } else { 0 }); }
    let e = errno();
    if e == EAGAIN { return Ok(IOS_UNAVAILABLE); }
    if e == EINTR { return Ok(IOS_INTERRUPTED); }
    io_fail()
}

fn convert_long(n: isize, reading: bool) -> Res<i64> {
    if n > 0 { return Ok(n as i64); }
    if n == 0 { return Ok(if reading { IOS_EOF as i64 } else { 0 }); }
    let e = errno();
    if e == EAGAIN { return Ok(IOS_UNAVAILABLE as i64); }
    if e == EINTR { return Ok(IOS_INTERRUPTED as i64); }
    io_fail()
}

// ------------------------------------------------------------------ UnixNativeDispatcher
pub fn n_sun_nio_fs_UnixNativeDispatcher__init() -> Res<i32> {
    Ok(2) // SUPPORTS_OPENAT
}

pub fn n_sun_nio_fs_UnixNativeDispatcher__getcwd() -> Res<Ref> {
    let mut buf = [0u8; 4096];
    let p = unsafe { getcwd(buf.as_mut_ptr() as *mut c_char, buf.len()) };
    if p.is_null() {
        return unix_fail();
    }
    let s = unsafe { CStr::from_ptr(p).to_bytes() };
    new_bytes(s)
}

pub fn n_sun_nio_fs_UnixNativeDispatcher__dup(fd: i32) -> Res<i32> {
    let r = unsafe { dup(fd) };
    if r == -1 { unix_fail() } else { Ok(r) }
}

pub fn n_sun_nio_fs_UnixNativeDispatcher__open0(p: i64, flags: i32, mode: i32) -> Res<i32> {
    let _blk = ScopedBlock::enter();
    let r = unsafe { open(cpath(p), flags, mode as c_uint) };
    if r == -1 { unix_fail() } else { Ok(r) }
}

pub fn n_sun_nio_fs_UnixNativeDispatcher__openat0(dfd: i32, p: i64, flags: i32, mode: i32) -> Res<i32> {
    let _blk = ScopedBlock::enter();
    let r = unsafe { openat(dfd, cpath(p), flags, mode as c_uint) };
    if r == -1 { unix_fail() } else { Ok(r) }
}

pub fn n_sun_nio_fs_UnixNativeDispatcher__close0(fd: i32) -> Res<()> {
    if unsafe { close(fd) } == -1 && errno() != EINTR {
        return unix_fail();
    }
    Ok(())
}

pub fn n_sun_nio_fs_UnixNativeDispatcher__rewind(f: i64) -> Res<()> {
    unsafe { rewind(f as usize as *mut c_void) };
    Ok(())
}

pub fn n_sun_nio_fs_UnixNativeDispatcher__getlinelen(f: i64) -> Res<i32> {
    let mut buf: *mut u8 = std::ptr::null_mut();
    let mut cap: usize = 0;
    let n = unsafe { getline(&mut buf, &mut cap, f as usize as *mut c_void) };
    unsafe { free(buf as *mut c_void) };
    Ok(if n < 0 { -1 } else { n as i32 })
}

pub fn n_sun_nio_fs_UnixNativeDispatcher__link0(a: i64, b: i64) -> Res<()> {
    unix_check(unsafe { link(cpath(a), cpath(b)) })
}
pub fn n_sun_nio_fs_UnixNativeDispatcher__unlink0(a: i64) -> Res<()> {
    unix_check(unsafe { unlink(cpath(a)) })
}
pub fn n_sun_nio_fs_UnixNativeDispatcher__unlinkat0(dfd: i32, a: i64, flags: i32) -> Res<()> {
    unix_check(unsafe { unlinkat(dfd, cpath(a), flags) })
}
pub fn n_sun_nio_fs_UnixNativeDispatcher__mknod0(a: i64, mode: i32, dev: i64) -> Res<()> {
    unix_check(unsafe { mknod(cpath(a), mode as u32, dev as u64) })
}
pub fn n_sun_nio_fs_UnixNativeDispatcher__rename0(a: i64, b: i64) -> Res<()> {
    unix_check(unsafe { rename(cpath(a), cpath(b)) })
}
pub fn n_sun_nio_fs_UnixNativeDispatcher__renameat0(fa: i32, a: i64, fb: i32, b: i64) -> Res<()> {
    unix_check(unsafe { renameat(fa, cpath(a), fb, cpath(b)) })
}
pub fn n_sun_nio_fs_UnixNativeDispatcher__mkdir0(a: i64, mode: i32) -> Res<()> {
    unix_check(unsafe { mkdir(cpath(a), mode as u32) })
}
pub fn n_sun_nio_fs_UnixNativeDispatcher__rmdir0(a: i64) -> Res<()> {
    unix_check(unsafe { rmdir(cpath(a)) })
}

pub fn n_sun_nio_fs_UnixNativeDispatcher__readlink0(a: i64) -> Res<Ref> {
    let mut buf = [0u8; 4096];
    let n = unsafe { readlink(cpath(a), buf.as_mut_ptr() as *mut c_char, buf.len()) };
    if n < 0 { unix_fail() } else { new_bytes(&buf[..n as usize]) }
}

pub fn n_sun_nio_fs_UnixNativeDispatcher__realpath0(a: i64) -> Res<Ref> {
    let mut buf = [0u8; 4097];
    let r = unsafe { realpath(cpath(a), buf.as_mut_ptr() as *mut c_char) };
    if r.is_null() { unix_fail() } else { new_bytes(unsafe { CStr::from_ptr(r).to_bytes() }) }
}

pub fn n_sun_nio_fs_UnixNativeDispatcher__symlink0(a: i64, b: i64) -> Res<()> {
    unix_check(unsafe { symlink(cpath(a), cpath(b)) })
}

fn fill_attrs(st: &Stat, attrs: Ref) -> Res<()> {
    let o = rt::nn_ref(attrs)?;
    rt::set_i32(o, "st_mode", st.st_mode as i32);
    rt::set_i64(o, "st_ino", st.st_ino as i64);
    rt::set_i64(o, "st_dev", st.st_dev as i64);
    rt::set_i64(o, "st_rdev", st.st_rdev as i64);
    rt::set_i32(o, "st_nlink", st.st_nlink as i32);
    rt::set_i32(o, "st_uid", st.st_uid as i32);
    rt::set_i32(o, "st_gid", st.st_gid as i32);
    rt::set_i64(o, "st_size", st.st_size);
    rt::set_i64(o, "st_atime_sec", st.st_atim_sec);
    rt::set_i64(o, "st_atime_nsec", st.st_atim_nsec);
    rt::set_i64(o, "st_mtime_sec", st.st_mtim_sec);
    rt::set_i64(o, "st_mtime_nsec", st.st_mtim_nsec);
    rt::set_i64(o, "st_ctime_sec", st.st_ctim_sec);
    rt::set_i64(o, "st_ctime_nsec", st.st_ctim_nsec);
    Ok(())
}

pub fn n_sun_nio_fs_UnixNativeDispatcher__stat0(a: i64, attrs: Ref) -> Res<i32> {
    let mut st: Stat = unsafe { std::mem::zeroed() };
    let r = {
        let _blk = ScopedBlock::enter();
        unsafe { stat(cpath(a), &mut st) }
    };
    if r == -1 {
        return Ok(errno());
    }
    fill_attrs(&st, attrs)?;
    Ok(0)
}

pub fn n_sun_nio_fs_UnixNativeDispatcher__lstat0(a: i64, attrs: Ref) -> Res<()> {
    let mut st: Stat = unsafe { std::mem::zeroed() };
    if unsafe { lstat(cpath(a), &mut st) } == -1 {
        return unix_fail();
    }
    fill_attrs(&st, attrs)
}

pub fn n_sun_nio_fs_UnixNativeDispatcher__fstat0(fd: i32, attrs: Ref) -> Res<()> {
    let mut st: Stat = unsafe { std::mem::zeroed() };
    if unsafe { fstat(fd, &mut st) } == -1 {
        return unix_fail();
    }
    fill_attrs(&st, attrs)
}

pub fn n_sun_nio_fs_UnixNativeDispatcher__fstatat0(dfd: i32, a: i64, flags: i32, attrs: Ref) -> Res<()> {
    let mut st: Stat = unsafe { std::mem::zeroed() };
    if unsafe { fstatat(dfd, cpath(a), &mut st, flags) } == -1 {
        return unix_fail();
    }
    fill_attrs(&st, attrs)
}

pub fn n_sun_nio_fs_UnixNativeDispatcher__chown0(a: i64, u: i32, g: i32) -> Res<()> {
    unix_check(unsafe { chown(cpath(a), u as u32, g as u32) })
}
pub fn n_sun_nio_fs_UnixNativeDispatcher__lchown0(a: i64, u: i32, g: i32) -> Res<()> {
    unix_check(unsafe { lchown(cpath(a), u as u32, g as u32) })
}
pub fn n_sun_nio_fs_UnixNativeDispatcher__fchown0(fd: i32, u: i32, g: i32) -> Res<()> {
    unix_check(unsafe { fchown(fd, u as u32, g as u32) })
}
pub fn n_sun_nio_fs_UnixNativeDispatcher__chmod0(a: i64, m: i32) -> Res<()> {
    unix_check(unsafe { chmod(cpath(a), m as u32) })
}
pub fn n_sun_nio_fs_UnixNativeDispatcher__fchmod0(fd: i32, m: i32) -> Res<()> {
    unix_check(unsafe { fchmod(fd, m as u32) })
}
pub fn n_sun_nio_fs_UnixNativeDispatcher__fchmodat0(fd: i32, a: i64, m: i32, flags: i32) -> Res<()> {
    unix_check(unsafe { fchmodat(fd, cpath(a), m as u32, flags) })
}

fn make_ts(ns: i64) -> Timespec {
    Timespec {
        tv_sec: ns.div_euclid(1_000_000_000),
        tv_nsec: ns.rem_euclid(1_000_000_000),
    }
}

pub fn n_sun_nio_fs_UnixNativeDispatcher__futimens0(fd: i32, at: i64, mt: i64) -> Res<()> {
    let t = [make_ts(at), make_ts(mt)];
    unix_check(unsafe { futimens(fd, t.as_ptr()) })
}

pub fn n_sun_nio_fs_UnixNativeDispatcher__utimensat0(fd: i32, a: i64, at: i64, mt: i64, flags: i32) -> Res<()> {
    let t = [make_ts(at), make_ts(mt)];
    unix_check(unsafe { utimensat(fd, cpath(a), t.as_ptr(), flags) })
}

pub fn n_sun_nio_fs_UnixNativeDispatcher__opendir0(a: i64) -> Res<i64> {
    let d = unsafe { opendir(cpath(a)) };
    if d.is_null() { unix_fail() } else { Ok(d as usize as i64) }
}

pub fn n_sun_nio_fs_UnixNativeDispatcher__fdopendir(fd: i32) -> Res<i64> {
    let d = unsafe { fdopendir(fd) };
    if d.is_null() { unix_fail() } else { Ok(d as usize as i64) }
}

pub fn n_sun_nio_fs_UnixNativeDispatcher__closedir(d: i64) -> Res<()> {
    if unsafe { closedir(d as usize as *mut c_void) } == -1 && errno() != EINTR {
        return unix_fail();
    }
    Ok(())
}

pub fn n_sun_nio_fs_UnixNativeDispatcher__readdir0(d: i64) -> Res<Ref> {
    set_errno(0);
    let e = unsafe { readdir(d as usize as *mut c_void) };
    if e.is_null() {
        if errno() != 0 {
            return unix_fail();
        }
        return Ok(Ref::NULL);
    }
    let name_bytes = unsafe { CStr::from_ptr(e.add(19) as *const c_char).to_bytes() };
    new_bytes(name_bytes)
}

pub fn n_sun_nio_fs_UnixNativeDispatcher__read0(fd: i32, buf: i64, n: i32) -> Res<i32> {
    let r = {
        let _blk = ScopedBlock::enter();
        unsafe { read(fd, buf as usize as *mut c_void, n.max(0) as usize) }
    };
    if r < 0 { unix_fail() } else { Ok(r as i32) }
}

pub fn n_sun_nio_fs_UnixNativeDispatcher__write0(fd: i32, buf: i64, n: i32) -> Res<i32> {
    let r = {
        let _blk = ScopedBlock::enter();
        unsafe { write(fd, buf as usize as *const c_void, n.max(0) as usize) }
    };
    if r < 0 { unix_fail() } else { Ok(r as i32) }
}

pub fn n_sun_nio_fs_UnixNativeDispatcher__access0(a: i64, m: i32) -> Res<i32> {
    Ok(if unsafe { access(cpath(a), m) } == -1 { errno() } else { 0 })
}

pub fn n_sun_nio_fs_UnixNativeDispatcher__getpwuid(uid: i32) -> Res<Ref> {
    new_bytes(uid.to_string().as_bytes())
}
pub fn n_sun_nio_fs_UnixNativeDispatcher__getgrgid(gid: i32) -> Res<Ref> {
    new_bytes(gid.to_string().as_bytes())
}
pub fn n_sun_nio_fs_UnixNativeDispatcher__getpwnam0(_n: i64) -> Res<i32> { Ok(-1) }
pub fn n_sun_nio_fs_UnixNativeDispatcher__getgrnam0(_n: i64) -> Res<i32> { Ok(-1) }

pub fn n_sun_nio_fs_UnixNativeDispatcher__statvfs0(a: i64, attrs: Ref) -> Res<()> {
    let mut st: StatVfs = unsafe { std::mem::zeroed() };
    if unsafe { statvfs(cpath(a), &mut st) } == -1 {
        return unix_fail();
    }
    let o = rt::nn_ref(attrs)?;
    rt::set_i64(o, "f_frsize", st.f_frsize as i64);
    rt::set_i64(o, "f_blocks", st.f_blocks as i64);
    rt::set_i64(o, "f_bfree", st.f_bfree as i64);
    rt::set_i64(o, "f_bavail", st.f_bavail as i64);
    Ok(())
}

pub fn n_sun_nio_fs_UnixNativeDispatcher__strerror(e: i32) -> Res<Ref> {
    let s = unsafe { CStr::from_ptr(strerror(e)).to_bytes() };
    new_bytes(s)
}

fn no_xattr<T>() -> Res<T> {
    set_errno(EOPNOTSUPP);
    unix_fail()
}
pub fn n_sun_nio_fs_UnixNativeDispatcher__fgetxattr0(_fd: i32, _n: i64, _v: i64, _l: i32) -> Res<i32> { no_xattr() }
pub fn n_sun_nio_fs_UnixNativeDispatcher__fsetxattr0(_fd: i32, _n: i64, _v: i64, _l: i32) -> Res<()> { no_xattr() }
pub fn n_sun_nio_fs_UnixNativeDispatcher__fremovexattr0(_fd: i32, _n: i64) -> Res<()> { no_xattr() }
pub fn n_sun_nio_fs_UnixNativeDispatcher__flistxattr(_fd: i32, _l: i64, _n: i32) -> Res<i32> { no_xattr() }
pub fn n_sun_nio_fs_UnixNativeDispatcher__fchmodatNoFollowSupported0() -> Res<i32> { Ok(0) }

// ------------------------------------------------------------------ LinuxNativeDispatcher ve bufferedCopy0
pub fn n_sun_nio_fs_LinuxNativeDispatcher__init() -> Res<()> { Ok(()) }

pub fn n_sun_nio_fs_LinuxNativeDispatcher__setmntent0(p: i64, m: i64) -> Res<i64> {
    let f = unsafe { setmntent(cpath(p), cpath(m)) };
    if f.is_null() { unix_fail() } else { Ok(f as usize as i64) }
}

pub fn n_sun_nio_fs_LinuxNativeDispatcher__getmntent0(f: i64, entry: Ref, buf: i64, len: i32) -> Res<i32> {
    let mut e: MntEnt = unsafe { std::mem::zeroed() };
    let res = unsafe { getmntent_r(f as usize as *mut c_void, &mut e, buf as usize as *mut c_char, len) };
    if res.is_null() {
        return Ok(-1);
    }
    let o = rt::nn_ref(entry)?;
    unsafe {
        if !e.mnt_fsname.is_null() { rt::set_ref(o, "name", new_bytes(CStr::from_ptr(e.mnt_fsname).to_bytes())?); }
        if !e.mnt_dir.is_null() { rt::set_ref(o, "dir", new_bytes(CStr::from_ptr(e.mnt_dir).to_bytes())?); }
        if !e.mnt_type.is_null() { rt::set_ref(o, "fstype", new_bytes(CStr::from_ptr(e.mnt_type).to_bytes())?); }
        if !e.mnt_opts.is_null() { rt::set_ref(o, "opts", new_bytes(CStr::from_ptr(e.mnt_opts).to_bytes())?); }
    }
    Ok(0)
}

pub fn n_sun_nio_fs_LinuxNativeDispatcher__endmntent(f: i64) -> Res<()> {
    unsafe { endmntent(f as usize as *mut c_void) };
    Ok(())
}

pub fn n_sun_nio_fs_LinuxNativeDispatcher__posix_fadvise(fd: i32, off: i64, len: i64, advice: i32) -> Res<i32> {
    Ok(unsafe { posix_fadvise(fd, off, len, advice) })
}

fn cfr_call(in_fd: i32, off_in: *mut i64, out_fd: i32, off_out: *mut i64, len: usize) -> isize {
    if SYS_COPY_FILE_RANGE < 0 {
        set_errno(ENOSYS);
        return -1;
    }
    unsafe { syscall(SYS_COPY_FILE_RANGE, in_fd as c_long, off_in, out_fd as c_long, off_out, len, 0 as c_uint) as isize }
}

pub fn n_sun_nio_fs_LinuxNativeDispatcher__directCopy0(dst: i32, src: i32, cancel: i64) -> Res<i32> {
    let flag = cancel as usize as *const AtomicI32;
    let mut copied: usize = 0;
    loop {
        if !flag.is_null() && unsafe { (*flag).load(Ordering::Relaxed) != 0 } {
            set_errno(125); // ECANCELED
            return unix_fail();
        }
        let n = {
            let _blk = ScopedBlock::enter();
            cfr_call(src, std::ptr::null_mut(), dst, std::ptr::null_mut(), 1 << 30)
        };
        if n == 0 {
            return Ok(0);
        }
        if n < 0 {
            let e = errno();
            if copied == 0 && matches!(e, EXDEV | ENOSYS | EINVAL | EOPNOTSUPP | 1 | EBADF) {
                return Ok(IOS_UNSUPPORTED_CASE);
            }
            if e == EINTR {
                continue;
            }
            return unix_fail();
        }
        copied += n as usize;
    }
}

pub fn n_sun_nio_fs_UnixFileSystem__bufferedCopy0(dst: i32, src: i32, buf: i64, size: i32, cancel: i64) -> Res<()> {
    let p = buf as usize as *mut u8;
    let flag = cancel as usize as *const AtomicI32;
    let _blk = ScopedBlock::enter();
    loop {
        if !flag.is_null() && unsafe { (*flag).load(Ordering::Relaxed) != 0 } {
            return Ok(());
        }
        let n = unsafe { read(src, p as *mut c_void, size.max(1) as usize) };
        if n < 0 {
            if errno() == EINTR { continue; }
            break;
        }
        if n == 0 {
            return Ok(());
        }
        let mut off = 0usize;
        while off < n as usize {
            let w = unsafe { write(dst, p.add(off) as *const c_void, n as usize - off) };
            if w < 0 {
                if errno() == EINTR { continue; }
                return Ok(());
            }
            off += w as usize;
        }
    }
    Ok(())
}

// ------------------------------------------------------------------ LinuxWatchService
pub fn n_sun_nio_fs_LinuxWatchService__eventSize() -> Res<i32> { Ok(16) }

pub fn n_sun_nio_fs_LinuxWatchService__eventOffsets() -> Res<Ref> {
    let a = rt::new_array(b'I', 5)?;
    let ptr = unsafe { (a.0 as *mut u8).add(24) as *mut i32 };
    unsafe {
        *ptr.add(0) = 0;  // wd
        *ptr.add(1) = 4;  // mask
        *ptr.add(2) = 8;  // cookie
        *ptr.add(3) = 12; // len
        *ptr.add(4) = 16; // name
    }
    Ok(a)
}

pub fn n_sun_nio_fs_LinuxWatchService__inotifyInit() -> Res<i32> {
    let fd = unsafe { inotify_init() };
    if fd < 0 { unix_fail() } else { Ok(fd) }
}

pub fn n_sun_nio_fs_LinuxWatchService__inotifyAddWatch(fd: i32, path: i64, mask: i32) -> Res<i32> {
    let wd = unsafe { inotify_add_watch(fd, cpath(path), mask as u32) };
    if wd < 0 { unix_fail() } else { Ok(wd) }
}

pub fn n_sun_nio_fs_LinuxWatchService__inotifyRmWatch(fd: i32, wd: i32) -> Res<()> {
    unsafe { inotify_rm_watch(fd, wd) };
    Ok(())
}

pub fn n_sun_nio_fs_LinuxWatchService__configureBlocking(fd: i32, blocking: i32) -> Res<()> {
    let fl = unsafe { fcntl(fd, 3) };
    let nfl = if blocking != 0 { fl & !0o4000 } else { fl | 0o4000 };
    if fl < 0 || unsafe { fcntl(fd, 4, nfl) } < 0 {
        return io_fail();
    }
    Ok(())
}

pub fn n_sun_nio_fs_LinuxWatchService__socketpair(arr: Ref) -> Res<()> {
    let mut sv = [0 as c_int; 2];
    if unsafe { socketpair(1, 1, 0, sv.as_mut_ptr()) } < 0 {
        return io_fail();
    }
    let _ = rt::nn(arr)?;
    let d = unsafe { (arr.0 as *mut u8).add(24) as *mut i32 };
    unsafe {
        *d.add(0) = sv[0];
        *d.add(1) = sv[1];
    }
    Ok(())
}

pub fn n_sun_nio_fs_LinuxWatchService__poll(fd1: i32, fd2: i32) -> Res<i32> {
    let mut p = [
        PollFd { fd: fd1, events: 1, revents: 0 },
        PollFd { fd: fd2, events: 1, revents: 0 },
    ];
    let r = {
        let _blk = ScopedBlock::enter();
        unsafe { poll(p.as_mut_ptr(), 2, -1) }
    };
    if r < 0 {
        if errno() == EINTR { return Ok(0); }
        return io_fail();
    }
    Ok(r)
}

// ------------------------------------------------------------------ UnixFileDispatcherImpl ve FileDispatcherImpl
pub fn n_sun_nio_ch_UnixFileDispatcherImpl__read0(fdo: Ref, a: i64, len: i32) -> Res<i32> {
    let fd = rt::fd_of(fdo)?;
    let n = {
        let _blk = ScopedBlock::enter();
        unsafe { read(fd, a as usize as *mut c_void, len.max(0) as usize) }
    };
    convert_ret(n, true)
}

pub fn n_sun_nio_ch_UnixFileDispatcherImpl__pread0(fdo: Ref, a: i64, len: i32, pos: i64) -> Res<i32> {
    let fd = rt::fd_of(fdo)?;
    let n = {
        let _blk = ScopedBlock::enter();
        unsafe { pread(fd, a as usize as *mut c_void, len.max(0) as usize, pos) }
    };
    convert_ret(n, true)
}

pub fn n_sun_nio_ch_UnixFileDispatcherImpl__readv0(fdo: Ref, a: i64, len: i32) -> Res<i64> {
    let fd = rt::fd_of(fdo)?;
    let n = {
        let _blk = ScopedBlock::enter();
        unsafe { readv(fd, a as usize as *const c_void, len) }
    };
    convert_long(n, true)
}

pub fn n_sun_nio_ch_UnixFileDispatcherImpl__write0(fdo: Ref, a: i64, len: i32) -> Res<i32> {
    let fd = rt::fd_of(fdo)?;
    let n = {
        let _blk = ScopedBlock::enter();
        unsafe { write(fd, a as usize as *const c_void, len.max(0) as usize) }
    };
    convert_ret(n, false)
}

pub fn n_sun_nio_ch_UnixFileDispatcherImpl__pwrite0(fdo: Ref, a: i64, len: i32, pos: i64) -> Res<i32> {
    let fd = rt::fd_of(fdo)?;
    let n = {
        let _blk = ScopedBlock::enter();
        unsafe { pwrite(fd, a as usize as *const c_void, len.max(0) as usize, pos) }
    };
    convert_ret(n, false)
}

pub fn n_sun_nio_ch_UnixFileDispatcherImpl__writev0(fdo: Ref, a: i64, len: i32) -> Res<i64> {
    let fd = rt::fd_of(fdo)?;
    let n = {
        let _blk = ScopedBlock::enter();
        unsafe { writev(fd, a as usize as *const c_void, len) }
    };
    convert_long(n, false)
}

pub fn n_sun_nio_ch_UnixFileDispatcherImpl__force0(fdo: Ref, md: i32) -> Res<i32> {
    let fd = rt::fd_of(fdo)?;
    let r = {
        let _blk = ScopedBlock::enter();
        unsafe { if md != 0 { fsync(fd) } else { fdatasync(fd) } }
    };
    if r < 0 { io_fail() } else { Ok(0) }
}

pub fn n_sun_nio_ch_UnixFileDispatcherImpl__seek0(fdo: Ref, off: i64) -> Res<i64> {
    let fd = rt::fd_of(fdo)?;
    let r = unsafe { if off < 0 { lseek(fd, 0, 1) } else { lseek(fd, off, 0) } };
    if r < 0 { io_fail() } else { Ok(r) }
}

pub fn n_sun_nio_ch_UnixFileDispatcherImpl__truncate0(fdo: Ref, size: i64) -> Res<i32> {
    let fd = rt::fd_of(fdo)?;
    if unsafe { ftruncate(fd, size) } < 0 {
        if errno() == EINTR { return Ok(IOS_INTERRUPTED); }
        return io_fail();
    }
    Ok(0)
}

pub fn n_sun_nio_ch_UnixFileDispatcherImpl__size0(fdo: Ref) -> Res<i64> {
    let fd = rt::fd_of(fdo)?;
    let mut st: Stat = unsafe { std::mem::zeroed() };
    if unsafe { fstat(fd, &mut st) } < 0 { io_fail() } else { Ok(st.st_size) }
}

pub fn n_sun_nio_ch_UnixFileDispatcherImpl__available0(fdo: Ref) -> Res<i32> {
    let fd = rt::fd_of(fdo)?;
    let mut st: Stat = unsafe { std::mem::zeroed() };
    if unsafe { fstat(fd, &mut st) } < 0 { return io_fail(); }
    let t = st.st_mode & 0o170000;
    if matches!(t, 0o020000 | 0o010000 | 0o140000) {
        let mut n: c_int = 0;
        if unsafe { ioctl(fd, 0x541b, &mut n) } >= 0 {
            return Ok(n);
        }
    }
    let cur = unsafe { lseek(fd, 0, 1) };
    if cur < 0 { return io_fail(); }
    Ok((st.st_size - cur).clamp(0, i32::MAX as i64) as i32)
}

pub fn n_sun_nio_ch_UnixFileDispatcherImpl__isOther0(fdo: Ref) -> Res<i32> {
    let fd = rt::fd_of(fdo)?;
    let mut st: Stat = unsafe { std::mem::zeroed() };
    if unsafe { fstat(fd, &mut st) } < 0 { return io_fail(); }
    let t = st.st_mode & 0o170000;
    Ok(if matches!(t, 0o100000 | 0o040000 | 0o120000) { 0 } else { 1 })
}

pub fn n_sun_nio_ch_UnixFileDispatcherImpl__lock0(fdo: Ref, blocking: i32, pos: i64, size: i64, shared: i32) -> Res<i32> {
    let fd = rt::fd_of(fdo)?;
    let mut fl = Flock {
        l_type: if shared != 0 { 0 } else { 1 },
        l_whence: 0,
        l_start: pos,
        l_len: if size == i64::MAX { 0 } else { size },
        l_pid: 0,
    };
    let cmd = if blocking != 0 { 7 } else { 6 };
    let r = if blocking != 0 {
        let _blk = ScopedBlock::enter();
        unsafe { fcntl(fd, cmd, &mut fl) }
    } else {
        unsafe { fcntl(fd, cmd, &mut fl) }
    };
    if r < 0 {
        let e = errno();
        if blocking == 0 && (e == EAGAIN || e == EACCES) { return Ok(-1); }
        if e == EINTR { return Ok(2); }
        return io_fail();
    }
    Ok(0)
}

pub fn n_sun_nio_ch_UnixFileDispatcherImpl__release0(fdo: Ref, pos: i64, size: i64) -> Res<()> {
    let fd = rt::fd_of(fdo)?;
    let mut fl = Flock {
        l_type: 2,
        l_whence: 0,
        l_start: pos,
        l_len: if size == i64::MAX { 0 } else { size },
        l_pid: 0,
    };
    if unsafe { fcntl(fd, 6, &mut fl) } < 0 { io_fail() } else { Ok(()) }
}

pub fn n_sun_nio_ch_UnixFileDispatcherImpl__closeIntFD(fd: i32) -> Res<()> {
    if fd != -1 && unsafe { close(fd) } < 0 && errno() != EINTR {
        return io_fail();
    }
    Ok(())
}

pub fn n_sun_nio_ch_UnixFileDispatcherImpl__allocationGranularity0() -> Res<i64> {
    Ok(unsafe { sysconf(30) } as i64) // _SC_PAGESIZE
}

pub fn n_sun_nio_ch_UnixFileDispatcherImpl__map0(fdo: Ref, prot: i32, off: i64, len: i64, _sync: i32) -> Res<i64> {
    let fd = rt::fd_of(fdo)?;
    let p = if prot == 0 { 1 } else { 3 };
    let flags = if prot == 2 { 2 } else { 1 };
    let r = unsafe { mmap(std::ptr::null_mut(), len as usize, p, flags, fd, off) };
    if r == -1 {
        if errno() == ENOMEM { return rt::throw("java/lang/OutOfMemoryError", "Map failed".to_string()); }
        return io_fail();
    }
    Ok(r as i64)
}

pub fn n_sun_nio_ch_UnixFileDispatcherImpl__unmap0(a: i64, len: i64) -> Res<i32> {
    if unsafe { munmap(a as usize as *mut c_void, len as usize) } < 0 { io_fail() } else { Ok(0) }
}

pub fn n_sun_nio_ch_UnixFileDispatcherImpl__setDirect0(fdo: Ref) -> Res<i32> {
    let fd = rt::fd_of(fdo)?;
    let fl = unsafe { fcntl(fd, 3) };
    if fl < 0 || unsafe { fcntl(fd, 4, fl | 0o40000) } < 0 {
        return io_fail();
    }
    let mut vfs: StatVfs = unsafe { std::mem::zeroed() };
    if unsafe { fstatvfs(fd, &mut vfs) } < 0 {
        return io_fail();
    }
    Ok(vfs.f_bsize as i32)
}

pub fn n_sun_nio_ch_FileDispatcherImpl__init0() -> Res<()> { Ok(()) }

pub fn n_sun_nio_ch_FileDispatcherImpl__transferTo0(src: Ref, pos: i64, count: i64, dst: Ref, _append: i32) -> Res<i64> {
    let sfd = rt::fd_of(src)?;
    let dfd = rt::fd_of(dst)?;
    let mut off = pos;
    let n = {
        let _blk = ScopedBlock::enter();
        unsafe { sendfile(dfd, sfd, &mut off, count.max(0) as usize) }
    };
    if n < 0 {
        let e = errno();
        if e == EAGAIN { return Ok(IOS_UNAVAILABLE as i64); }
        if e == EINVAL && count >= 0 { return Ok(IOS_UNSUPPORTED_CASE as i64); }
        if e == EINTR { return Ok(IOS_INTERRUPTED as i64); }
        return io_fail();
    }
    Ok(n as i64)
}

pub fn n_sun_nio_ch_FileDispatcherImpl__transferFrom0(src: Ref, dst: Ref, pos: i64, count: i64, append: i32) -> Res<i64> {
    if append != 0 {
        return Ok(IOS_UNSUPPORTED_CASE as i64);
    }
    let sfd = rt::fd_of(src)?;
    let dfd = rt::fd_of(dst)?;
    let mut off = pos;
    let n = {
        let _blk = ScopedBlock::enter();
        cfr_call(sfd, std::ptr::null_mut(), dfd, &mut off, count.max(0) as usize)
    };
    if n < 0 {
        let e = errno();
        if matches!(e, EINVAL | EXDEV | ENOSYS | EOPNOTSUPP | EBADF | 1) {
            return Ok(IOS_UNSUPPORTED_CASE as i64);
        }
        if e == EINTR { return Ok(IOS_INTERRUPTED as i64); }
        return io_fail();
    }
    Ok(n as i64)
}

// ------------------------------------------------------------------ UnixDispatcher / IOUtil / NativeThread / FileKey
pub fn n_sun_nio_ch_UnixDispatcher__init() -> Res<()> {
    let mut sv = [0 as c_int; 2];
    if unsafe { socketpair(1, 1, 0, sv.as_mut_ptr()) } < 0 {
        return io_fail();
    }
    PRE_CLOSE_FD.store(sv[0], Ordering::Release);
    unsafe { close(sv[1]) };
    Ok(())
}

pub fn n_sun_nio_ch_UnixDispatcher__close0(fdo: Ref) -> Res<()> {
    let fd = rt::fd_of(fdo)?;
    if fd != -1 && unsafe { close(fd) } < 0 && errno() != EINTR { io_fail() } else { Ok(()) }
}

pub fn n_sun_nio_ch_UnixDispatcher__preClose0(fdo: Ref) -> Res<()> {
    let fd = rt::fd_of(fdo)?;
    let pc = PRE_CLOSE_FD.load(Ordering::Acquire);
    if pc >= 0 && unsafe { dup2(pc, fd) } < 0 { io_fail() } else { Ok(()) }
}

pub fn n_sun_nio_ch_IOUtil__initIDs() -> Res<()> { Ok(()) }
pub fn n_sun_nio_ch_IOUtil__randomBytes(_b: Ref) -> Res<i32> { Ok(0) }
pub fn n_sun_nio_ch_IOUtil__fdVal(fdo: Ref) -> Res<i32> { rt::fd_of(fdo) }
pub fn n_sun_nio_ch_IOUtil__setfdVal(fdo: Ref, v: i32) -> Res<()> { rt::set_fd(fdo, v) }

pub fn n_sun_nio_ch_IOUtil__configureBlocking(fdo: Ref, blocking: i32) -> Res<()> {
    let fd = rt::fd_of(fdo)?;
    let fl = unsafe { fcntl(fd, 3) };
    let nfl = if blocking != 0 { fl & !0o4000 } else { fl | 0o4000 };
    if fl < 0 || (fl != nfl && unsafe { fcntl(fd, 4, nfl) } < 0) {
        return io_fail();
    }
    Ok(())
}

pub fn n_sun_nio_ch_IOUtil__fdLimit() -> Res<i32> {
    let mut rl = [0u64; 2];
    if unsafe { getrlimit(7, &mut rl) } < 0 {
        return io_fail();
    }
    Ok(rl[0].min(i32::MAX as u64) as i32)
}

pub fn n_sun_nio_ch_IOUtil__iovMax() -> Res<i32> {
    let v = unsafe { sysconf(60) };
    Ok(if v < 0 { 16 } else { v as i32 })
}

pub fn n_sun_nio_ch_IOUtil__writevMax() -> Res<i64> {
    Ok(isize::MAX as i64)
}

pub fn n_sun_nio_ch_IOUtil__makePipe(blocking: i32) -> Res<i64> {
    let mut fds = [0 as c_int; 2];
    if unsafe { pipe2(fds.as_mut_ptr(), if blocking != 0 { 0 } else { 0o4000 }) } < 0 {
        return io_fail();
    }
    Ok(((fds[0] as i64) << 32) | ((fds[1] as u32) as i64))
}

pub fn n_sun_nio_ch_IOUtil__write1(fd: i32, b: i32) -> Res<i32> {
    let c = [b as u8];
    convert_ret(unsafe { write(fd, c.as_ptr() as *const c_void, 1) }, false)
}

pub fn n_sun_nio_ch_IOUtil__drain(fd: i32) -> Res<i32> {
    let mut buf = [0u8; 64];
    let mut any = false;
    loop {
        let n = unsafe { read(fd, buf.as_mut_ptr() as *mut c_void, buf.len()) };
        if n < 0 {
            let e = errno();
            if e == EAGAIN { break; }
            if e == EINTR { continue; }
            return io_fail();
        }
        if n > 0 { any = true; }
        if (n as usize) < buf.len() { break; }
    }
    Ok(any as i32)
}

pub fn n_sun_nio_ch_IOUtil__drain1(fd: i32) -> Res<i32> {
    let mut b = [0u8; 1];
    let n = unsafe { read(fd, b.as_mut_ptr() as *mut c_void, 1) };
    if n < 0 {
        let e = errno();
        if e == EAGAIN { return Ok(0); }
        if e == EINTR { return Ok(IOS_INTERRUPTED); }
        return io_fail();
    }
    Ok(n as i32)
}

extern "C" fn on_io_signal(_sig: c_int) {}

pub fn n_sun_nio_ch_NativeThread__init() -> Res<()> {
    let mut act = [0u8; 152];
    let h = on_io_signal as *const () as usize;
    act[0..8].copy_from_slice(&h.to_ne_bytes());
    let sig = unsafe { __libc_current_sigrtmax() } - 2;
    if unsafe { sigaction(sig, act.as_ptr(), std::ptr::null_mut()) } < 0 {
        return io_fail();
    }
    Ok(())
}

pub fn n_sun_nio_ch_NativeThread__supportPendingSignals0() -> Res<i32> { Ok(0) }
pub fn n_sun_nio_ch_NativeThread__current0() -> Res<i64> { Ok(unsafe { pthread_self() } as i64) }

pub fn n_sun_nio_ch_NativeThread__signal0(t: i64) -> Res<()> {
    let sig = unsafe { __libc_current_sigrtmax() } - 2;
    let r = unsafe { pthread_kill(t as usize, sig) };
    if r != 0 {
        return rt::throw("java/io/IOException", "Thread signal failed".to_string());
    }
    Ok(())
}

pub fn n_sun_nio_ch_FileKey__init(fdo: Ref, info: Ref) -> Res<()> {
    let fd = rt::fd_of(fdo)?;
    let mut st: Stat = unsafe { std::mem::zeroed() };
    if unsafe { fstat(fd, &mut st) } < 0 {
        return io_fail();
    }
    let _ = rt::nn(info)?;
    let ptr = unsafe { (info.0 as *mut u8).add(24) as *mut i64 };
    unsafe {
        *ptr.add(0) = st.st_dev as i64;
        *ptr.add(1) = st.st_ino as i64;
    }
    Ok(())
}

// ------------------------------------------------------------------ EPoll / EventFD
pub fn n_sun_nio_ch_EPoll__eventSize() -> Res<i32> { Ok(std::mem::size_of::<EpollEvent>() as i32) }
pub fn n_sun_nio_ch_EPoll__eventsOffset() -> Res<i32> { Ok(0) }
pub fn n_sun_nio_ch_EPoll__dataOffset() -> Res<i32> { Ok(std::mem::offset_of!(EpollEvent, data) as i32) }

pub fn n_sun_nio_ch_EPoll__create() -> Res<i32> {
    let fd = unsafe { epoll_create(256) };
    if fd < 0 { io_fail() } else { Ok(fd) }
}

pub fn n_sun_nio_ch_EPoll__ctl(epfd: i32, op: i32, fd: i32, events: i32) -> Res<i32> {
    let ev = EpollEvent { events: events as u32, data: fd as u64 };
    Ok(if unsafe { epoll_ctl(epfd, op, fd, &ev) } == 0 { 0 } else { errno() })
}

pub fn n_sun_nio_ch_EPoll__wait(epfd: i32, address: i64, numfds: i32, timeout: i32) -> Res<i32> {
    let n = {
        let _blk = ScopedBlock::enter();
        unsafe { epoll_wait(epfd, address as usize as *mut EpollEvent, numfds, timeout) }
    };
    if n < 0 {
        if errno() == EINTR { return Ok(IOS_INTERRUPTED); }
        return io_fail();
    }
    Ok(n)
}

pub fn n_sun_nio_ch_EventFD__eventfd0() -> Res<i32> {
    let fd = unsafe { eventfd(0, 0) };
    if fd < 0 { io_fail() } else { Ok(fd) }
}

pub fn n_sun_nio_ch_EventFD__set0(fd: i32) -> Res<i32> {
    let one: u64 = 1;
    if unsafe { write(fd, &one as *const u64 as *const c_void, 8) } < 0 {
        return io_fail();
    }
    Ok(0)
}

// ------------------------------------------------------------------ MappedMemoryUtils
pub fn n_java_nio_MappedMemoryUtils__registerNatives() -> Res<()> { Ok(()) }

pub fn n_java_nio_MappedMemoryUtils__isLoaded0(a: i64, len: i64, pages: i64) -> Res<i32> {
    let n = pages.max(0) as usize;
    let mut vec = vec![0u8; n];
    if unsafe { mincore(a as usize as *mut c_void, len.max(0) as usize, vec.as_mut_ptr()) } != 0 {
        return io_fail();
    }
    Ok(if vec.iter().all(|&v| (v & 1) != 0) { 1 } else { 0 })
}

pub fn n_java_nio_MappedMemoryUtils__load0(a: i64, len: i64) -> Res<()> {
    if unsafe { madvise(a as usize as *mut c_void, len.max(0) as usize, 3) } != 0 { io_fail() } else { Ok(()) }
}

pub fn n_java_nio_MappedMemoryUtils__unload0(a: i64, len: i64) -> Res<()> {
    if unsafe { madvise(a as usize as *mut c_void, len.max(0) as usize, 4) } != 0 { io_fail() } else { Ok(()) }
}

pub fn n_java_nio_MappedMemoryUtils__force0(_fdo: Ref, a: i64, len: i64) -> Res<()> {
    if unsafe { msync(a as usize as *mut c_void, len.max(0) as usize, 4) } != 0 { io_fail() } else { Ok(()) }
}
