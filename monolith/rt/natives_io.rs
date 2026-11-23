//! File IO, UnixFileSystem, zlib compression/checksums, ServiceLoader, TimeZone,
//! Atomics, Reflection Array/Executable helpers, System/Runtime IO, and remaining Unsafe ops.
use crate::gc::ScopedBlock;
use crate::rt::{self, Arr, Ref, Res};
use std::ffi::{CStr, CString};
use std::io::ErrorKind;
use std::os::raw::{c_char, c_int, c_long, c_uint, c_ulong, c_void};
use std::sync::atomic::{AtomicI16, AtomicI32, AtomicI64, AtomicI8, AtomicU16, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

#[cfg_attr(unix, link(name = "c"))]
extern "C" {
    fn open(pathname: *const c_char, flags: c_int, ...) -> c_int;
    fn close(fd: c_int) -> c_int;
    fn read(fd: c_int, buf: *mut c_void, count: usize) -> isize;
    fn write(fd: c_int, buf: *const c_void, count: usize) -> isize;
    fn lseek(fd: c_int, offset: i64, whence: c_int) -> i64;
    fn ftruncate(fd: c_int, length: i64) -> c_int;
    fn fsync(fd: c_int) -> c_int;
    fn fcntl(fd: c_int, cmd: c_int, ...) -> c_int;
    fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
    fn access(pathname: *const c_char, mode: c_int) -> c_int;
    fn unlink(pathname: *const c_char) -> c_int;
    fn pathconf(path: *const c_char, name: c_int) -> c_long;
    fn utimes(filename: *const c_char, times: *const TimeVal) -> c_int;
    fn getloadavg(loadavg: *mut f64, nelem: c_int) -> c_int;
    fn localtime_r(timep: *const i64, result: *mut Tm) -> *mut Tm;
    #[cfg(not(windows))]
    static environ: *const *const c_char;
    // UCRT keeps the environment block in _environ
    #[cfg(windows)]
    #[link_name = "_environ"]
    static environ: *const *const c_char;
}

#[cfg_attr(unix, link(name = "dl"))]
extern "C" {
    fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

#[link(name = "z")]
extern "C" {
    fn adler32(adler: c_ulong, buf: *const u8, len: c_uint) -> c_ulong;
    fn inflateInit2_(strm: *mut ZStream, window_bits: c_int, version: *const c_char, stream_size: c_int) -> c_int;
    fn inflate(strm: *mut ZStream, flush: c_int) -> c_int;
    fn inflateReset(strm: *mut ZStream) -> c_int;
    fn inflateEnd(strm: *mut ZStream) -> c_int;
    fn inflateSetDictionary(strm: *mut ZStream, dict: *const u8, dict_len: c_uint) -> c_int;
    fn deflateInit2_(strm: *mut ZStream, level: c_int, method: c_int, window_bits: c_int, mem_level: c_int, strategy: c_int, version: *const c_char, stream_size: c_int) -> c_int;
    fn deflate(strm: *mut ZStream, flush: c_int) -> c_int;
    fn deflateParams(strm: *mut ZStream, level: c_int, strategy: c_int) -> c_int;
    fn deflateReset(strm: *mut ZStream) -> c_int;
    fn deflateEnd(strm: *mut ZStream) -> c_int;
    fn deflateSetDictionary(strm: *mut ZStream, dict: *const u8, dict_len: c_uint) -> c_int;
}

#[repr(C)]
#[derive(Clone, Copy)]
struct TimeVal {
    tv_sec: i64,
    tv_usec: i64,
}

#[repr(C)]
struct Tm {
    tm_sec: c_int,
    tm_min: c_int,
    tm_hour: c_int,
    tm_mday: c_int,
    tm_mon: c_int,
    tm_year: c_int,
    tm_wday: c_int,
    tm_yday: c_int,
    tm_isdst: c_int,
    tm_gmtoff: c_long,
    tm_zone: *const c_char,
}

#[repr(C)]
struct ZStream {
    next_in: *const u8,
    avail_in: c_uint,
    total_in: c_ulong,
    next_out: *mut u8,
    avail_out: c_uint,
    total_out: c_ulong,
    msg: *const c_char,
    state: *mut c_void,
    zalloc: *const c_void,
    zfree: *const c_void,
    opaque: *mut c_void,
    data_type: c_int,
    adler: c_ulong,
    reserved: c_ulong,
}

const O_RDONLY: c_int = 0;
const O_WRONLY: c_int = 1;
const O_RDWR: c_int = 2;
const O_CREAT: c_int = 0o100;
const O_TRUNC: c_int = 0o1000;
const O_APPEND: c_int = 0o2000;
const O_DSYNC: c_int = 0o10000;
const O_CLOEXEC: c_int = 0o2000000;
const O_SYNC: c_int = 0o4010000;
const F_GETFL: c_int = 3;
const FIONREAD: c_ulong = 0x541b;
const SEEK_SET: c_int = 0;
const SEEK_CUR: c_int = 1;
const SEEK_END: c_int = 2;
const EINTR: i32 = 4;
const _PC_NAME_MAX: c_int = 3;
const RTLD_NOW: c_int = 2;
const RTLD_GLOBAL: c_int = 0x100;

const Z_OK: c_int = 0;
const Z_STREAM_END: c_int = 1;
const Z_NEED_DICT: c_int = 2;
const Z_DATA_ERROR: c_int = -3;
const Z_MEM_ERROR: c_int = -4;
const Z_BUF_ERROR: c_int = -5;
const Z_NO_FLUSH: c_int = 0;
const Z_DEFLATED: c_int = 8;
const ZLIB_VERSION: &[u8] = b"1.3\0";

static LOADED_LIBS: OnceLock<Mutex<Vec<usize>>> = OnceLock::new();

pub fn loaded_lib_handles() -> Vec<usize> {
    LOADED_LIBS.get_or_init(|| Mutex::new(Vec::new())).lock().unwrap().clone()
}

pub fn register_lib_handle(handle: *mut c_void) {
    if !handle.is_null() {
        LOADED_LIBS.get_or_init(|| Mutex::new(Vec::new())).lock().unwrap().push(handle as usize);
    }
}

fn last_os_err() -> (i32, String) {
    let e = std::io::Error::last_os_error();
    let code = e.raw_os_error().unwrap_or(0);
    (code, e.to_string())
}

fn stream_fd(this: Ref) -> Res<i32> {
    let o = rt::nn_ref(this)?;
    let fd_obj = rt::get_ref(o, "fd");
    let fd = rt::fd_of(fd_obj)?;
    if fd < 0 {
        return rt::throw("java/io/IOException", "Stream Closed".to_string());
    }
    Ok(fd)
}

fn file_path(f: Ref) -> Res<String> {
    let fo = rt::nn_ref(f)?;
    let p = rt::get_ref(fo, "path");
    Ok(rt::str_to_rust(rt::nn_ref(p)?))
}

fn check_bounds(arr: Ref, off: i32, len: i32) -> Res<usize> {
    let a = rt::nn(arr)? as *const Arr;
    let alen = unsafe { (*a).len as i64 };
    if off < 0 || len < 0 || (off as i64) + (len as i64) > alen {
        return rt::throw("java/lang/IndexOutOfBoundsException", format!("off={} len={} alen={}", off, len, alen));
    }
    Ok(alen as usize)
}

// ------------------------------------------------------------------ java.io.FileDescriptor & FileCleanable
pub fn n_java_io_FileDescriptor__initIDs() -> Res<()> {
    Ok(())
}

pub fn n_java_io_FileDescriptor__getHandle(_fd: i32) -> Res<i64> {
    Ok(-1)
}

pub fn n_java_io_FileDescriptor__getAppend(fd: i32) -> Res<i32> {
    if fd < 0 {
        return Ok(0);
    }
    let flags = unsafe { fcntl(fd, F_GETFL, 0) };
    Ok(if flags >= 0 && (flags & O_APPEND) != 0 { 1 } else { 0 })
}

pub fn n_java_io_FileDescriptor__close0(this: Ref) -> Res<()> {
    let o = rt::nn_ref(this)?;
    let fd = rt::get_i32(o, "fd");
    if fd < 0 {
        return Ok(());
    }
    rt::set_i32(o, "fd", -1);
    rt::set_bool(o, "closed", true);
    let rc = unsafe { close(fd) };
    if rc != 0 {
        let (_, msg) = last_os_err();
        return rt::throw("java/io/IOException", msg);
    }
    Ok(())
}

pub fn n_java_io_FileDescriptor__sync0(this: Ref) -> Res<()> {
    let fd = rt::fd_of(this)?;
    if fd < 0 {
        return rt::throw("java/io/SyncFailedException", "sync failed".to_string());
    }
    let rc = {
        let _blk = ScopedBlock::enter();
        unsafe { fsync(fd) }
    };
    if rc != 0 {
        let (_, msg) = last_os_err();
        return rt::throw("java/io/SyncFailedException", msg);
    }
    Ok(())
}

pub fn n_java_io_FileCleanable__cleanupClose0(fd: i32, _handle: i64) -> Res<()> {
    if fd >= 0 {
        unsafe { close(fd) };
    }
    Ok(())
}

// ------------------------------------------------------------------ java.io.FileInputStream
pub fn n_java_io_FileInputStream__initIDs() -> Res<()> {
    Ok(())
}

pub fn n_java_io_FileInputStream__open0(this: Ref, name: Ref) -> Res<()> {
    let o = rt::nn_ref(this)?;
    let cpath = rt::c_str_arg(name)?;
    let fd = {
        let _blk = ScopedBlock::enter();
        loop {
            let r = unsafe { open(cpath.as_ptr(), O_RDONLY | O_CLOEXEC) };
            if r < 0 && last_os_err().0 == EINTR {
                continue;
            }
            break r;
        }
    };
    if fd < 0 {
        let (_, msg) = last_os_err();
        return rt::throw("java/io/FileNotFoundException", format!("{} ({})", cpath.to_string_lossy(), msg));
    }
    let fd_obj = rt::get_ref(o, "fd");
    rt::set_fd(fd_obj, fd)?;
    Ok(())
}

pub fn n_java_io_FileInputStream__read0(this: Ref) -> Res<i32> {
    let fd = stream_fd(this)?;
    let mut b: u8 = 0;
    let n = {
        let _blk = ScopedBlock::enter();
        loop {
            let r = unsafe { read(fd, &mut b as *mut u8 as *mut c_void, 1) };
            if r < 0 && last_os_err().0 == EINTR {
                continue;
            }
            break r;
        }
    };
    if n < 0 {
        let (_, msg) = last_os_err();
        return rt::throw("java/io/IOException", msg);
    }
    if n == 0 { Ok(-1) } else { Ok(b as i32) }
}

fn read_bytes_fd(fd: i32, arr: Ref, off: i32, len: i32) -> Res<i32> {
    check_bounds(arr, off, len)?;
    if len == 0 {
        return Ok(0);
    }
    let ulen = len as usize;
    let mut stack_buf = [0u8; 8192];
    let mut heap_buf = Vec::new();
    let buf: &mut [u8] = if ulen <= stack_buf.len() {
        &mut stack_buf[..ulen]
    } else {
        heap_buf.resize(ulen, 0);
        &mut heap_buf[..ulen]
    };
    let n = {
        let _blk = ScopedBlock::enter();
        loop {
            let r = unsafe { read(fd, buf.as_mut_ptr() as *mut c_void, ulen) };
            if r < 0 && last_os_err().0 == EINTR {
                continue;
            }
            break r;
        }
    };
    if n < 0 {
        let (_, msg) = last_os_err();
        return rt::throw("java/io/IOException", msg);
    }
    if n == 0 {
        return Ok(-1);
    }
    let nu = n as usize;
    unsafe {
        let dst = rt::byte_slice_mut(arr)?;
        dst[off as usize..off as usize + nu].copy_from_slice(&buf[..nu]);
    }
    Ok(n as i32)
}

pub fn n_java_io_FileInputStream__readBytes(this: Ref, b: Ref, off: i32, len: i32) -> Res<i32> {
    let fd = stream_fd(this)?;
    read_bytes_fd(fd, b, off, len)
}

pub fn n_java_io_FileInputStream__skip0(this: Ref, n: i64) -> Res<i64> {
    let fd = stream_fd(this)?;
    let cur = unsafe { lseek(fd, 0, SEEK_CUR) };
    if cur < 0 {
        return rt::throw("java/io/IOException", last_os_err().1);
    }
    let end = unsafe { lseek(fd, n, SEEK_CUR) };
    if end < 0 {
        return rt::throw("java/io/IOException", last_os_err().1);
    }
    Ok(end - cur)
}

pub fn n_java_io_FileInputStream__available0(this: Ref) -> Res<i32> {
    let fd = stream_fd(this)?;
    let mut avail: c_int = 0;
    if unsafe { ioctl(fd, FIONREAD, &mut avail) } >= 0 {
        return Ok(avail);
    }
    let cur = unsafe { lseek(fd, 0, SEEK_CUR) };
    if cur >= 0 {
        let end = unsafe { lseek(fd, 0, SEEK_END) };
        unsafe { lseek(fd, cur, SEEK_SET) };
        if end >= cur {
            return Ok((end - cur).min(i32::MAX as i64) as i32);
        }
    }
    Ok(0)
}

pub fn n_java_io_FileInputStream__position0(this: Ref) -> Res<i64> {
    let fd = stream_fd(this)?;
    let pos = unsafe { lseek(fd, 0, SEEK_CUR) };
    if pos < 0 {
        return rt::throw("java/io/IOException", last_os_err().1);
    }
    Ok(pos)
}

pub fn n_java_io_FileInputStream__length0(this: Ref) -> Res<i64> {
    let fd = stream_fd(this)?;
    let cur = unsafe { lseek(fd, 0, SEEK_CUR) };
    if cur >= 0 {
        let end = unsafe { lseek(fd, 0, SEEK_END) };
        unsafe { lseek(fd, cur, SEEK_SET) };
        if end >= 0 {
            return Ok(end);
        }
    }
    rt::throw("java/io/IOException", last_os_err().1)
}

pub fn n_java_io_FileInputStream__isRegularFile0(_this: Ref, fd_obj: Ref) -> Res<i32> {
    let fd = rt::fd_of(fd_obj)?;
    if fd < 0 {
        return Ok(0);
    }
    #[cfg(unix)]
    let f = {
        use std::os::unix::io::FromRawFd;
        std::mem::ManuallyDrop::new(unsafe { std::fs::File::from_raw_fd(fd) })
    };
    #[cfg(windows)]
    let f = {
        use std::os::windows::io::{FromRawHandle, RawHandle};
        extern "C" { fn _get_osfhandle(fd: i32) -> isize; }
        let h = unsafe { _get_osfhandle(fd) };
        if h == -1 || h == 0 {
            return Ok(0);
        }
        std::mem::ManuallyDrop::new(unsafe { std::fs::File::from_raw_handle(h as RawHandle) })
    };
    Ok(match f.metadata() {
        Ok(m) if m.is_file() => 1,
        _ => 0,
    })
}

// ------------------------------------------------------------------ java.io.FileOutputStream
pub fn n_java_io_FileOutputStream__initIDs() -> Res<()> {
    Ok(())
}

pub fn n_java_io_FileOutputStream__open0(this: Ref, name: Ref, append: i32) -> Res<()> {
    let o = rt::nn_ref(this)?;
    let cpath = rt::c_str_arg(name)?;
    let is_append = append != 0;
    let flags = O_WRONLY | O_CREAT | O_CLOEXEC | if is_append { O_APPEND } else { O_TRUNC };
    let fd = {
        let _blk = ScopedBlock::enter();
        loop {
            let r = unsafe { open(cpath.as_ptr(), flags, 0o666 as c_uint) };
            if r < 0 && last_os_err().0 == EINTR {
                continue;
            }
            break r;
        }
    };
    if fd < 0 {
        let (_, msg) = last_os_err();
        return rt::throw("java/io/FileNotFoundException", format!("{} ({})", cpath.to_string_lossy(), msg));
    }
    let fd_obj = rt::get_ref(o, "fd");
    rt::set_fd(fd_obj, fd)?;
    rt::set_bool(fd_obj, "append", is_append);
    Ok(())
}

fn write_all_fd(fd: i32, data: &[u8]) -> Res<()> {
    let mut off = 0usize;
    let _blk = ScopedBlock::enter();
    while off < data.len() {
        let r = unsafe { write(fd, data[off..].as_ptr() as *const c_void, data.len() - off) };
        if r < 0 {
            let (err, msg) = last_os_err();
            if err == EINTR {
                continue;
            }
            return rt::throw("java/io/IOException", msg);
        }
        off += r as usize;
    }
    Ok(())
}

pub fn n_java_io_FileOutputStream__write(this: Ref, b: i32, _append: i32) -> Res<()> {
    let fd = stream_fd(this)?;
    let byte = [b as u8];
    write_all_fd(fd, &byte)
}

pub fn n_java_io_FileOutputStream__writeBytes(this: Ref, b: Ref, off: i32, len: i32, _append: i32) -> Res<()> {
    let fd = stream_fd(this)?;
    check_bounds(b, off, len)?;
    if len == 0 {
        return Ok(());
    }
    let slice = unsafe { &rt::byte_slice(b)?[off as usize..off as usize + len as usize] };
    let copy = slice.to_vec();
    write_all_fd(fd, &copy)
}

// ------------------------------------------------------------------ java.io.RandomAccessFile
pub fn n_java_io_RandomAccessFile__initIDs() -> Res<()> {
    Ok(())
}

pub fn n_java_io_RandomAccessFile__open0(this: Ref, name: Ref, mode: i32) -> Res<()> {
    let o = rt::nn_ref(this)?;
    let cpath = rt::c_str_arg(name)?;
    let mut flags = if (mode & 2) != 0 { O_RDWR | O_CREAT } else { O_RDONLY };
    if (mode & 4) != 0 {
        flags |= O_SYNC;
    }
    if (mode & 8) != 0 {
        flags |= O_DSYNC;
    }
    flags |= O_CLOEXEC;
    let fd = {
        let _blk = ScopedBlock::enter();
        loop {
            let r = unsafe { open(cpath.as_ptr(), flags, 0o666 as c_uint) };
            if r < 0 && last_os_err().0 == EINTR {
                continue;
            }
            break r;
        }
    };
    if fd < 0 {
        let (_, msg) = last_os_err();
        return rt::throw("java/io/FileNotFoundException", format!("{} ({})", cpath.to_string_lossy(), msg));
    }
    if (mode & 16) != 0 {
        unsafe { unlink(cpath.as_ptr()) };
    }
    let fd_obj = rt::get_ref(o, "fd");
    rt::set_fd(fd_obj, fd)?;
    Ok(())
}

pub fn n_java_io_RandomAccessFile__read0(this: Ref) -> Res<i32> {
    n_java_io_FileInputStream__read0(this)
}

pub fn n_java_io_RandomAccessFile__readBytes0(this: Ref, b: Ref, off: i32, len: i32) -> Res<i32> {
    let fd = stream_fd(this)?;
    read_bytes_fd(fd, b, off, len)
}

pub fn n_java_io_RandomAccessFile__write0(this: Ref, b: i32) -> Res<()> {
    let fd = stream_fd(this)?;
    let byte = [b as u8];
    write_all_fd(fd, &byte)
}

pub fn n_java_io_RandomAccessFile__writeBytes0(this: Ref, b: Ref, off: i32, len: i32) -> Res<()> {
    n_java_io_FileOutputStream__writeBytes(this, b, off, len, 0)
}

pub fn n_java_io_RandomAccessFile__getFilePointer(this: Ref) -> Res<i64> {
    n_java_io_FileInputStream__position0(this)
}

pub fn n_java_io_RandomAccessFile__seek0(this: Ref, pos: i64) -> Res<()> {
    if pos < 0 {
        return rt::throw("java/io/IOException", "Negative seek offset".to_string());
    }
    let fd = stream_fd(this)?;
    let r = unsafe { lseek(fd, pos, SEEK_SET) };
    if r < 0 {
        return rt::throw("java/io/IOException", last_os_err().1);
    }
    Ok(())
}

pub fn n_java_io_RandomAccessFile__length0(this: Ref) -> Res<i64> {
    n_java_io_FileInputStream__length0(this)
}

pub fn n_java_io_RandomAccessFile__setLength0(this: Ref, new_len: i64) -> Res<()> {
    let fd = stream_fd(this)?;
    let rc = {
        let _blk = ScopedBlock::enter();
        loop {
            let r = unsafe { ftruncate(fd, new_len) };
            if r < 0 && last_os_err().0 == EINTR {
                continue;
            }
            break r;
        }
    };
    if rc < 0 {
        return rt::throw("java/io/IOException", last_os_err().1);
    }
    Ok(())
}

// ------------------------------------------------------------------ java.io.UnixFileSystem
pub fn n_java_io_UnixFileSystem__initIDs() -> Res<()> {
    Ok(())
}

pub fn n_java_io_UnixFileSystem__canonicalize0(_this: Ref, path: Ref) -> Res<Ref> {
    let p = rt::str_to_rust(rt::nn_ref(path)?);
    if let Ok(c) = std::fs::canonicalize(&p) {
        return Ok(rt::new_str(c.to_string_lossy().into_owned()));
    }
    let path_obj = std::path::Path::new(&p);
    let mut cur = path_obj.to_path_buf();
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    while !cur.exists() {
        if let Some(name) = cur.file_name() {
            tail.push(name.to_os_string());
            if !cur.pop() {
                break;
            }
        } else {
            break;
        }
    }
    let mut base = if cur.as_os_str().is_empty() {
        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"))
    } else {
        std::fs::canonicalize(&cur).unwrap_or(cur)
    };
    for comp in tail.into_iter().rev() {
        base.push(comp);
    }
    Ok(rt::new_str(base.to_string_lossy().into_owned()))
}

pub fn n_java_io_UnixFileSystem__getBooleanAttributes0(_this: Ref, f: Ref) -> Res<i32> {
    let p = file_path(f)?;
    let _blk = ScopedBlock::enter();
    Ok(match std::fs::metadata(&p) {
        Ok(m) => 1 | if m.is_file() { 2 } else { 0 } | if m.is_dir() { 4 } else { 0 },
        Err(_) => 0,
    })
}

pub fn n_java_io_UnixFileSystem__checkAccess0(_this: Ref, f: Ref, access_mode: i32) -> Res<i32> {
    let p = file_path(f)?;
    let Ok(cp) = CString::new(p) else { return Ok(0); };
    let rc = unsafe { access(cp.as_ptr(), access_mode) };
    Ok(if rc == 0 { 1 } else { 0 })
}

pub fn n_java_io_UnixFileSystem__getLastModifiedTime0(_this: Ref, f: Ref) -> Res<i64> {
    let p = file_path(f)?;
    let _blk = ScopedBlock::enter();
    Ok(match std::fs::metadata(&p).and_then(|m| m.modified()) {
        Ok(t) => t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0),
        Err(_) => 0,
    })
}

pub fn n_java_io_UnixFileSystem__getLength0(_this: Ref, f: Ref) -> Res<i64> {
    let p = file_path(f)?;
    let _blk = ScopedBlock::enter();
    Ok(match std::fs::metadata(&p) {
        Ok(m) => m.len() as i64,
        Err(_) => 0,
    })
}

pub fn n_java_io_UnixFileSystem__createFileExclusively0(_this: Ref, path: Ref) -> Res<i32> {
    let p = rt::str_to_rust(rt::nn_ref(path)?);
    let res = {
        let _blk = ScopedBlock::enter();
        std::fs::OpenOptions::new().write(true).create_new(true).open(&p)
    };
    match res {
        Ok(_) => Ok(1),
        Err(e) if e.kind() == ErrorKind::AlreadyExists => Ok(0),
        Err(e) => rt::throw("java/io/IOException", e.to_string()),
    }
}

pub fn n_java_io_UnixFileSystem__delete0(_this: Ref, f: Ref) -> Res<i32> {
    let p = file_path(f)?;
    let _blk = ScopedBlock::enter();
    let ok = std::fs::remove_file(&p).is_ok() || std::fs::remove_dir(&p).is_ok();
    Ok(if ok { 1 } else { 0 })
}

pub fn n_java_io_UnixFileSystem__list0(_this: Ref, f: Ref) -> Res<Ref> {
    let p = file_path(f)?;
    let entries: Vec<String> = {
        let _blk = ScopedBlock::enter();
        match std::fs::read_dir(&p) {
            Ok(rd) => rd.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect(),
            Err(_) => return Ok(Ref::NULL),
        }
    };
    rt::string_array(&entries)
}

pub fn n_java_io_UnixFileSystem__createDirectory0(_this: Ref, f: Ref) -> Res<i32> {
    let p = file_path(f)?;
    let _blk = ScopedBlock::enter();
    Ok(if std::fs::create_dir(&p).is_ok() { 1 } else { 0 })
}

pub fn n_java_io_UnixFileSystem__rename0(_this: Ref, f1: Ref, f2: Ref) -> Res<i32> {
    let p1 = file_path(f1)?;
    let p2 = file_path(f2)?;
    let _blk = ScopedBlock::enter();
    Ok(if std::fs::rename(&p1, &p2).is_ok() { 1 } else { 0 })
}

pub fn n_java_io_UnixFileSystem__setLastModifiedTime0(_this: Ref, f: Ref, time_ms: i64) -> Res<i32> {
    if time_ms < 0 {
        return rt::throw("java/lang/IllegalArgumentException", "Negative time".to_string());
    }
    let p = file_path(f)?;
    let Ok(cp) = CString::new(p) else { return Ok(0); };
    let sec = time_ms / 1000;
    let usec = (time_ms % 1000) * 1000;
    let tv = [
        TimeVal { tv_sec: sec, tv_usec: usec },
        TimeVal { tv_sec: sec, tv_usec: usec },
    ];
    let rc = unsafe { utimes(cp.as_ptr(), tv.as_ptr()) };
    Ok(if rc == 0 { 1 } else { 0 })
}

pub fn n_java_io_UnixFileSystem__getNameMax0(_this: Ref, path: Ref) -> Res<i64> {
    let cp = rt::c_str_arg(path)?;
    let r = unsafe { pathconf(cp.as_ptr(), _PC_NAME_MAX) };
    Ok(if r > 0 { r as i64 } else { 255 })
}

// ------------------------------------------------------------------ java.util.zip CRC32 / Adler32
/// CRC-32 (IEEE 802.3, reflected polynomial 0xEDB88320), table driven. Continues from `crc` (the value Java's CRC32
/// holds): same result as zlib's `crc32(crc, buf, len)`.
fn crc32_continue(crc: u32, data: &[u8]) -> u32 {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let t = TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for (i, e) in t.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            *e = c;
        }
        t
    });
    let mut c = !crc;
    for &b in data {
        c = t[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    !c
}

pub fn n_java_util_zip_CRC32__update(crc_val: i32, b: i32) -> Res<i32> {
    Ok(crc32_continue(crc_val as u32, &[b as u8]) as i32)
}

pub fn n_java_util_zip_CRC32__updateBytes0(crc_val: i32, b: Ref, off: i32, len: i32) -> Res<i32> {
    check_bounds(b, off, len)?;
    if len == 0 {
        return Ok(crc_val);
    }
    let s = unsafe { &rt::byte_slice(b)?[off as usize..off as usize + len as usize] };
    Ok(crc32_continue(crc_val as u32, s) as i32)
}

pub fn n_java_util_zip_CRC32__updateByteBuffer0(crc_val: i32, addr: i64, off: i32, len: i32) -> Res<i32> {
    if len <= 0 || addr == 0 {
        return Ok(crc_val);
    }
    let ptr = unsafe { (addr as usize as *const u8).add(off as usize) };
    let s = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    Ok(crc32_continue(crc_val as u32, s) as i32)
}

pub fn n_java_util_zip_Adler32__update(adler_val: i32, b: i32) -> Res<i32> {
    let byte = b as u8;
    Ok(unsafe { adler32(adler_val as u32 as c_ulong, &byte, 1) as u32 as i32 })
}

pub fn n_java_util_zip_Adler32__updateBytes(adler_val: i32, b: Ref, off: i32, len: i32) -> Res<i32> {
    check_bounds(b, off, len)?;
    if len == 0 {
        return Ok(adler_val);
    }
    let s = unsafe { &rt::byte_slice(b)?[off as usize..off as usize + len as usize] };
    Ok(unsafe { adler32(adler_val as u32 as c_ulong, s.as_ptr(), s.len() as c_uint) as u32 as i32 })
}

pub fn n_java_util_zip_Adler32__updateByteBuffer(adler_val: i32, addr: i64, off: i32, len: i32) -> Res<i32> {
    if len <= 0 || addr == 0 {
        return Ok(adler_val);
    }
    let ptr = unsafe { (addr as usize as *const u8).add(off as usize) };
    Ok(unsafe { adler32(adler_val as u32 as c_ulong, ptr, len as c_uint) as u32 as i32 })
}

// ------------------------------------------------------------------ java.util.zip Inflater / Deflater
pub fn n_java_util_zip_Inflater__initIDs() -> Res<()> {
    Ok(())
}

pub fn n_java_util_zip_Inflater__init(nowrap: i32) -> Res<i64> {
    let strm = Box::into_raw(Box::new(unsafe { std::mem::zeroed::<ZStream>() }));
    let wbits = if nowrap != 0 { -15 } else { 15 };
    let rc = unsafe {
        inflateInit2_(strm, wbits, ZLIB_VERSION.as_ptr() as *const c_char, std::mem::size_of::<ZStream>() as c_int)
    };
    if rc != Z_OK {
        unsafe { drop(Box::from_raw(strm)) };
        return rt::throw(
            if rc == Z_MEM_ERROR { "java/lang/OutOfMemoryError" } else { "java/lang/InternalError" },
            format!("inflateInit2 failed: {}", rc),
        );
    }
    Ok(strm as usize as i64)
}

static EMPTY_ZIN: [u8; 4] = [0; 4];

unsafe fn run_inflate(
    this: Ref,
    addr: i64,
    in_ptr: *const u8,
    in_len: i32,
    out_ptr: *mut u8,
    out_len: i32,
) -> Res<i64> {
    let strm = addr as usize as *mut ZStream;
    if strm.is_null() {
        return rt::throw("java/lang/NullPointerException", "Inflater closed".to_string());
    }
    let mut empty_out = [0u8; 4];
    (*strm).next_in = if in_ptr.is_null() || in_len <= 0 { EMPTY_ZIN.as_ptr() } else { in_ptr };
    (*strm).avail_in = in_len.max(0) as c_uint;
    (*strm).next_out = if out_ptr.is_null() || out_len <= 0 { empty_out.as_mut_ptr() } else { out_ptr };
    (*strm).avail_out = out_len.max(0) as c_uint;

    let rc = inflate(strm, Z_NO_FLUSH);
    let consumed = (in_len.max(0) as c_uint - (*strm).avail_in) as i64;
    let produced = (out_len.max(0) as c_uint - (*strm).avail_out) as i64;

    let (finished, need_dict) = match rc {
        Z_STREAM_END => (1i64, 0i64),
        Z_NEED_DICT => (0i64, 1i64),
        Z_OK | Z_BUF_ERROR => (0i64, 0i64),
        Z_DATA_ERROR => {
            if !this.is_null() {
                rt::set_i32(this, "inputConsumed", consumed as i32);
                rt::set_i32(this, "outputConsumed", produced as i32);
            }
            let msg = if (*strm).msg.is_null() {
                "invalid stored/compressed data".to_string()
            } else {
                CStr::from_ptr((*strm).msg).to_string_lossy().into_owned()
            };
            return rt::throw("java/util/zip/DataFormatException", msg);
        }
        Z_MEM_ERROR => return rt::throw("java/lang/OutOfMemoryError", "zlib OOM".to_string()),
        _ => return rt::throw("java/lang/InternalError", format!("inflate failed: {}", rc)),
    };

    if !this.is_null() {
        rt::set_i32(this, "inputConsumed", consumed as i32);
        rt::set_i32(this, "outputConsumed", produced as i32);
    }
    Ok((consumed & 0x7fff_ffff) | ((produced & 0x7fff_ffff) << 31) | (finished << 62) | (need_dict << 63))
}

pub fn n_java_util_zip_Inflater__inflateBytesBytes(
    this: Ref, addr: i64, in_arr: Ref, in_off: i32, in_len: i32, out_arr: Ref, out_off: i32, out_len: i32,
) -> Res<i64> {
    check_bounds(in_arr, in_off, in_len)?;
    check_bounds(out_arr, out_off, out_len)?;
    unsafe {
        let ip = rt::byte_slice(in_arr)?.as_ptr().add(in_off as usize);
        let op = rt::byte_slice_mut(out_arr)?.as_mut_ptr().add(out_off as usize);
        run_inflate(this, addr, ip, in_len, op, out_len)
    }
}

pub fn n_java_util_zip_Inflater__inflateBytesBuffer(
    this: Ref, addr: i64, in_arr: Ref, in_off: i32, in_len: i32, out_addr: i64, out_len: i32,
) -> Res<i64> {
    check_bounds(in_arr, in_off, in_len)?;
    unsafe {
        let ip = rt::byte_slice(in_arr)?.as_ptr().add(in_off as usize);
        run_inflate(this, addr, ip, in_len, out_addr as usize as *mut u8, out_len)
    }
}

pub fn n_java_util_zip_Inflater__inflateBufferBytes(
    this: Ref, addr: i64, in_addr: i64, in_len: i32, out_arr: Ref, out_off: i32, out_len: i32,
) -> Res<i64> {
    check_bounds(out_arr, out_off, out_len)?;
    unsafe {
        let op = rt::byte_slice_mut(out_arr)?.as_mut_ptr().add(out_off as usize);
        run_inflate(this, addr, in_addr as usize as *const u8, in_len, op, out_len)
    }
}

pub fn n_java_util_zip_Inflater__inflateBufferBuffer(
    this: Ref, addr: i64, in_addr: i64, in_len: i32, out_addr: i64, out_len: i32,
) -> Res<i64> {
    unsafe {
        run_inflate(this, addr, in_addr as usize as *const u8, in_len, out_addr as usize as *mut u8, out_len)
    }
}

pub fn n_java_util_zip_Inflater__setDictionary(addr: i64, b: Ref, off: i32, len: i32) -> Res<()> {
    check_bounds(b, off, len)?;
    let strm = addr as usize as *mut ZStream;
    let ptr = unsafe { rt::byte_slice(b)?.as_ptr().add(off as usize) };
    let rc = unsafe { inflateSetDictionary(strm, ptr, len as c_uint) };
    if rc != Z_OK {
        return rt::throw("java/lang/IllegalArgumentException", format!("inflateSetDictionary failed: {}", rc));
    }
    Ok(())
}

pub fn n_java_util_zip_Inflater__setDictionaryBuffer(addr: i64, buf_addr: i64, len: i32) -> Res<()> {
    let strm = addr as usize as *mut ZStream;
    let rc = unsafe { inflateSetDictionary(strm, buf_addr as usize as *const u8, len as c_uint) };
    if rc != Z_OK {
        return rt::throw("java/lang/IllegalArgumentException", format!("inflateSetDictionaryBuffer failed: {}", rc));
    }
    Ok(())
}

pub fn n_java_util_zip_Inflater__getAdler(addr: i64) -> Res<i32> {
    let strm = addr as usize as *mut ZStream;
    Ok(if strm.is_null() { 0 } else { unsafe { (*strm).adler as u32 as i32 } })
}

pub fn n_java_util_zip_Inflater__reset(addr: i64) -> Res<()> {
    let strm = addr as usize as *mut ZStream;
    if !strm.is_null() {
        unsafe { inflateReset(strm) };
    }
    Ok(())
}

pub fn n_java_util_zip_Inflater__end(addr: i64) -> Res<()> {
    let strm = addr as usize as *mut ZStream;
    if !strm.is_null() {
        unsafe {
            inflateEnd(strm);
            drop(Box::from_raw(strm));
        }
    }
    Ok(())
}

pub fn n_java_util_zip_Deflater__init(level: i32, strategy: i32, nowrap: i32) -> Res<i64> {
    let strm = Box::into_raw(Box::new(unsafe { std::mem::zeroed::<ZStream>() }));
    let wbits = if nowrap != 0 { -15 } else { 15 };
    let rc = unsafe {
        deflateInit2_(strm, level, Z_DEFLATED, wbits, 8, strategy, ZLIB_VERSION.as_ptr() as *const c_char, std::mem::size_of::<ZStream>() as c_int)
    };
    if rc != Z_OK {
        unsafe { drop(Box::from_raw(strm)) };
        return rt::throw(
            if rc == Z_MEM_ERROR { "java/lang/OutOfMemoryError" } else { "java/lang/InternalError" },
            format!("deflateInit2 failed: {}", rc),
        );
    }
    Ok(strm as usize as i64)
}

unsafe fn run_deflate(
    addr: i64,
    in_ptr: *const u8,
    in_len: i32,
    out_ptr: *mut u8,
    out_len: i32,
    flush: i32,
    params: i32,
) -> Res<i64> {
    let strm = addr as usize as *mut ZStream;
    if strm.is_null() {
        return rt::throw("java/lang/NullPointerException", "Deflater closed".to_string());
    }
    let mut empty_out = [0u8; 4];
    (*strm).next_in = if in_ptr.is_null() || in_len <= 0 { EMPTY_ZIN.as_ptr() } else { in_ptr };
    (*strm).avail_in = in_len.max(0) as c_uint;
    (*strm).next_out = if out_ptr.is_null() || out_len <= 0 { empty_out.as_mut_ptr() } else { out_ptr };
    (*strm).avail_out = out_len.max(0) as c_uint;

    if (params & 1) != 0 {
        let strategy = (params >> 1) & 0x3;
        let level = params >> 3;
        let prc = deflateParams(strm, level, strategy);
        let set_params_done = match prc {
            Z_OK => 1i64,
            Z_BUF_ERROR => 0i64,
            _ => return rt::throw("java/lang/InternalError", format!("deflateParams failed: {}", prc)),
        };
        let consumed = (in_len.max(0) as c_uint - (*strm).avail_in) as i64;
        let produced = (out_len.max(0) as c_uint - (*strm).avail_out) as i64;
        return Ok((consumed & 0x7fff_ffff) | ((produced & 0x7fff_ffff) << 31) | (set_params_done << 63));
    }

    let rc = deflate(strm, flush);
    let consumed = (in_len.max(0) as c_uint - (*strm).avail_in) as i64;
    let produced = (out_len.max(0) as c_uint - (*strm).avail_out) as i64;

    let finished = match rc {
        Z_STREAM_END => 1i64,
        Z_OK | Z_BUF_ERROR => 0i64,
        Z_MEM_ERROR => return rt::throw("java/lang/OutOfMemoryError", "zlib OOM".to_string()),
        _ => return rt::throw("java/lang/InternalError", format!("deflate failed: {}", rc)),
    };

    Ok((consumed & 0x7fff_ffff) | ((produced & 0x7fff_ffff) << 31) | (finished << 62))
}

pub fn n_java_util_zip_Deflater__deflateBytesBytes(
    _this: Ref, addr: i64, in_arr: Ref, in_off: i32, in_len: i32, out_arr: Ref, out_off: i32, out_len: i32, flush: i32, params: i32,
) -> Res<i64> {
    check_bounds(in_arr, in_off, in_len)?;
    check_bounds(out_arr, out_off, out_len)?;
    unsafe {
        let ip = rt::byte_slice(in_arr)?.as_ptr().add(in_off as usize);
        let op = rt::byte_slice_mut(out_arr)?.as_mut_ptr().add(out_off as usize);
        run_deflate(addr, ip, in_len, op, out_len, flush, params)
    }
}

pub fn n_java_util_zip_Deflater__deflateBufferBytes(
    _this: Ref, addr: i64, in_addr: i64, in_len: i32, out_arr: Ref, out_off: i32, out_len: i32, flush: i32, params: i32,
) -> Res<i64> {
    check_bounds(out_arr, out_off, out_len)?;
    unsafe {
        let op = rt::byte_slice_mut(out_arr)?.as_mut_ptr().add(out_off as usize);
        run_deflate(addr, in_addr as usize as *const u8, in_len, op, out_len, flush, params)
    }
}

pub fn n_java_util_zip_Deflater__deflateBytesBuffer(
    _this: Ref, addr: i64, in_arr: Ref, in_off: i32, in_len: i32, out_addr: i64, out_len: i32, flush: i32, params: i32,
) -> Res<i64> {
    check_bounds(in_arr, in_off, in_len)?;
    unsafe {
        let ip = rt::byte_slice(in_arr)?.as_ptr().add(in_off as usize);
        run_deflate(addr, ip, in_len, out_addr as usize as *mut u8, out_len, flush, params)
    }
}

pub fn n_java_util_zip_Deflater__deflateBufferBuffer(
    _this: Ref, addr: i64, in_addr: i64, in_len: i32, out_addr: i64, out_len: i32, flush: i32, params: i32,
) -> Res<i64> {
    unsafe {
        run_deflate(addr, in_addr as usize as *const u8, in_len, out_addr as usize as *mut u8, out_len, flush, params)
    }
}

pub fn n_java_util_zip_Deflater__setDictionary(addr: i64, b: Ref, off: i32, len: i32) -> Res<()> {
    check_bounds(b, off, len)?;
    let strm = addr as usize as *mut ZStream;
    let ptr = unsafe { rt::byte_slice(b)?.as_ptr().add(off as usize) };
    let rc = unsafe { deflateSetDictionary(strm, ptr, len as c_uint) };
    if rc != Z_OK {
        return rt::throw("java/lang/IllegalArgumentException", format!("deflateSetDictionary failed: {}", rc));
    }
    Ok(())
}

pub fn n_java_util_zip_Deflater__setDictionaryBuffer(addr: i64, buf_addr: i64, len: i32) -> Res<()> {
    let strm = addr as usize as *mut ZStream;
    let rc = unsafe { deflateSetDictionary(strm, buf_addr as usize as *const u8, len as c_uint) };
    if rc != Z_OK {
        return rt::throw("java/lang/IllegalArgumentException", format!("deflateSetDictionaryBuffer failed: {}", rc));
    }
    Ok(())
}

pub fn n_java_util_zip_Deflater__getAdler(addr: i64) -> Res<i32> {
    let strm = addr as usize as *mut ZStream;
    Ok(if strm.is_null() { 0 } else { unsafe { (*strm).adler as u32 as i32 } })
}

pub fn n_java_util_zip_Deflater__reset(addr: i64) -> Res<()> {
    let strm = addr as usize as *mut ZStream;
    if !strm.is_null() {
        unsafe { deflateReset(strm) };
    }
    Ok(())
}

pub fn n_java_util_zip_Deflater__end(addr: i64) -> Res<()> {
    let strm = addr as usize as *mut ZStream;
    if !strm.is_null() {
        unsafe {
            deflateEnd(strm);
            drop(Box::from_raw(strm));
        }
    }
    Ok(())
}

// ------------------------------------------------------------------ ServiceLoader / TimeZone
pub fn n_java_util_ServiceLoader__providers0(name: Ref) -> Res<Ref> {
    let svc = rt::str_to_rust(rt::nn_ref(name)?);
    let services = rt::hooks().services;
    let mut created = Vec::new();
    for entry in services {
        if entry.service == svc {
            created.push((entry.create)()?);
        }
    }
    let arr = rt::new_obj_array(rt::hooks().object_class, created.len() as i32)?;
    for (i, item) in created.into_iter().enumerate() {
        rt::aastore_ref(arr, i as i32, item)?;
    }
    Ok(arr)
}

pub fn n_java_util_TimeZone__getSystemTimeZoneID(_java_home: Ref) -> Res<Ref> {
    if let Ok(tz) = std::env::var("TZ") {
        let s = tz.strip_prefix(':').unwrap_or(&tz);
        if !s.is_empty() && !s.starts_with('/') {
            return Ok(rt::new_str(s.to_string()));
        }
        if let Some((_, zone)) = s.rsplit_once("zoneinfo/") {
            if !zone.is_empty() {
                return Ok(rt::new_str(zone.to_string()));
            }
        }
    }
    if let Ok(link) = std::fs::read_link("/etc/localtime") {
        let s = link.to_string_lossy();
        if let Some((_, zone)) = s.rsplit_once("zoneinfo/") {
            if !zone.is_empty() {
                return Ok(rt::new_str(zone.to_string()));
            }
        }
    }
    if let Ok(tz_file) = std::fs::read_to_string("/etc/timezone") {
        let z = tz_file.trim();
        if !z.is_empty() {
            return Ok(rt::new_str(z.to_string()));
        }
    }
    n_java_util_TimeZone__getSystemGMTOffsetID()
}

pub fn n_java_util_TimeZone__getSystemGMTOffsetID() -> Res<Ref> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let mut tm: Tm = unsafe { std::mem::zeroed() };
    let ptr = unsafe { localtime_r(&now, &mut tm) };
    let off = if ptr.is_null() { 0 } else { tm.tm_gmtoff as i64 };
    if off == 0 {
        return Ok(rt::new_str("GMT".to_string()));
    }
    let sign = if off < 0 { '-' } else { '+' };
    let abs = off.abs();
    let h = abs / 3600;
    let m = (abs % 3600) / 60;
    Ok(rt::new_str(format!("GMT{}{:02}:{:02}", sign, h, m)))
}

// ------------------------------------------------------------------ Atomics
pub fn n_java_util_concurrent_atomic_AtomicBoolean__cas(this: Ref, expected: i32, new_val: i32) -> Res<i32> {
    let o = rt::nn_ref(this)?;
    let c = unsafe { (*o.0).class };
    if let Some((_, f)) = c.find_field("value", false) {
        if let rt::FLoc::Off(off) = f.loc {
            let atom = unsafe { &*((o.0 as *const u8).add(off as usize) as *const AtomicI32) };
            return Ok(atom.compare_exchange(expected, new_val, Ordering::SeqCst, Ordering::SeqCst).is_ok() as i32);
        }
    }
    Ok(0)
}

pub fn n_java_util_concurrent_atomic_AtomicReference__cas(this: Ref, expected: Ref, new_val: Ref) -> Res<i32> {
    let o = rt::nn_ref(this)?;
    let c = unsafe { (*o.0).class };
    if let Some((_, f)) = c.find_field("value", false) {
        if let rt::FLoc::Off(off) = f.loc {
            let atom = unsafe { &*((o.0 as *const u8).add(off as usize) as *const AtomicUsize) };
            return Ok(atom.compare_exchange(expected.0 as usize, new_val.0 as usize, Ordering::SeqCst, Ordering::SeqCst).is_ok() as i32);
        }
    }
    Ok(0)
}

// ------------------------------------------------------------------ Reflection Array / Executable / ObjectStreamClass
pub fn n_java_lang_reflect_Array__getLength(arr: Ref) -> Res<i32> {
    let a = rt::nn(arr)? as *const Arr;
    let c = unsafe { (*a).hdr.class };
    if c.kind != 2 || c.elem == 0 {
        return rt::throw("java/lang/IllegalArgumentException", "Argument is not an array".to_string());
    }
    Ok(unsafe { (*a).len })
}

fn multi_alloc(cls_chain: &[&'static rt::Class], dims: &[i32], depth: usize) -> Res<Ref> {
    let len = dims[depth];
    let ac = cls_chain[depth];
    if depth == dims.len() - 1 {
        return if ac.elem != b'A' {
            rt::new_array(ac.elem, len)
        } else {
            rt::new_obj_array(ac.elem_class.unwrap(), len)
        };
    }
    let arr = rt::new_obj_array(ac.elem_class.unwrap(), len)?;
    for i in 0..len {
        let sub = multi_alloc(cls_chain, dims, depth + 1)?;
        rt::aastore_ref(arr, i, sub)?;
    }
    Ok(arr)
}

pub fn n_java_lang_reflect_Array__multiNewArray(component: Ref, dims_arr: Ref) -> Res<Ref> {
    let c = rt::class_of_mirror(rt::nn_ref(component)?);
    if std::ptr::eq(c, &rt::CLS_P_VOID) {
        return rt::throw("java/lang/IllegalArgumentException", "void array component".to_string());
    }
    let d_hdr = rt::nn(dims_arr)? as *const Arr;
    let n = unsafe { (*d_hdr).len } as usize;
    if n == 0 || n > 255 {
        return rt::throw("java/lang/IllegalArgumentException", "invalid dimensions".to_string());
    }
    let d_slice = unsafe { std::slice::from_raw_parts((d_hdr as *const u8).add(24) as *const i32, n) }.to_vec();
    for &d in &d_slice {
        if d < 0 {
            return rt::throw("java/lang/NegativeArraySizeException", d.to_string());
        }
    }
    let mut chain = Vec::with_capacity(n);
    let mut cur = c;
    for _ in 0..n {
        cur = rt::array_class_of(cur);
        chain.push(cur);
    }
    chain.reverse();
    multi_alloc(&chain, &d_slice, 0)
}

pub fn n_java_lang_reflect_Executable__getParameters0(_this: Ref) -> Res<Ref> {
    Ok(Ref::NULL)
}

pub fn n_java_io_ObjectStreamClass__hasStaticInitializer(c_mirror: Ref) -> Res<i32> {
    let c = rt::class_of_mirror(rt::nn_ref(c_mirror)?);
    Ok(c.init.is_some() as i32)
}

// ------------------------------------------------------------------ System / Runtime
pub fn n_java_lang_System__env0() -> Res<Ref> {
    let mut pairs = Vec::new();
    unsafe {
        let mut p = environ;
        while !p.is_null() && !(*p).is_null() {
            let s = CStr::from_ptr(*p).to_string_lossy();
            if let Some((k, v)) = s.split_once('=') {
                pairs.push(k.to_string());
                pairs.push(v.to_string());
            } else {
                pairs.push(s.into_owned());
                pairs.push(String::new());
            }
            p = p.add(1);
        }
    }
    rt::string_array(&pairs)
}

pub fn n_java_lang_System__read0(fd: i32, b: Ref, off: i32, len: i32) -> Res<i32> {
    check_bounds(b, off, len)?;
    if len == 0 {
        return Ok(0);
    }
    let want = (len as usize).min(8192);
    let mut buf = [0u8; 8192];
    let n = {
        let _blk = ScopedBlock::enter();
        loop {
            let r = unsafe { read(fd, buf.as_mut_ptr() as *mut c_void, want) };
            if r < 0 && last_os_err().0 == EINTR {
                continue;
            }
            break r;
        }
    };
    if n <= 0 {
        return Ok(0);
    }
    let nu = n as usize;
    unsafe {
        rt::byte_slice_mut(b)?[off as usize..off as usize + nu].copy_from_slice(&buf[..nu]);
    }
    Ok(n as i32)
}

pub fn n_java_lang_Runtime__load1(_this: Ref, path: Ref) -> Res<i32> {
    let cp = rt::c_str_arg(path)?;
    let handle = unsafe { dlopen(cp.as_ptr(), RTLD_NOW | RTLD_GLOBAL) };
    if handle.is_null() {
        return Ok(0);
    }
    register_lib_handle(handle);
    let onload = unsafe { dlsym(handle, b"JNI_OnLoad\0".as_ptr() as *const c_char) };
    if !onload.is_null() {
        // Defer full JavaVM InvokeInterface initialization if invoked by JNI libraries loaded later
        let _ = onload;
    }
    Ok(1)
}

pub fn n_java_lang_Runtime__isPreloaded(_this: Ref, file: Ref) -> Res<i32> {
    let fname = rt::str_to_rust(rt::nn_ref(file)?);
    let base = fname.rsplit('/').next().unwrap_or(&fname);
    if base.contains("jnidispatch")
        || base.contains("netty_transport_native_epoll")
        || base.contains("netty_transport_native_kqueue")
        || base.contains("jtracy")
        || base.contains("monoshader")
        || base.contains("lwjgl")
        || base.contains("shaderc")
        || base.contains("spirv-cross")
    {
        return Ok(1);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let cand = dir.join("natives").join(&fname);
            if cand.is_file() {
                if let Ok(cp) = CString::new(cand.to_string_lossy().as_bytes()) {
                    let h = unsafe { dlopen(cp.as_ptr(), RTLD_NOW | RTLD_GLOBAL) };
                    if !h.is_null() {
                        register_lib_handle(h);
                        return Ok(1);
                    }
                }
            }
        }
    }
    Ok(0)
}

// ------------------------------------------------------------------ jdk.internal.misc.Unsafe remaining ops
#[inline(always)]
fn uptr(o: Ref, off: i64) -> *mut u8 {
    let base = if o.is_null() { 0usize } else { o.0 as usize };
    base.wrapping_add(off as usize) as *mut u8
}

pub fn n_jdk_internal_misc_Unsafe__allocateInstance(_this: Ref, c_mirror: Ref) -> Res<Ref> {
    let c = rt::class_of_mirror(rt::nn_ref(c_mirror)?);
    if c.kind != 0 || (c.access & 0x0600) != 0 {
        return rt::throw("java/lang/InstantiationException", c.name.replace('/', "."));
    }
    if let Some(i) = c.init {
        i().res()?;
    }
    Ok(rt::alloc(c))
}

pub fn n_jdk_internal_misc_Unsafe__shouldBeInitialized0(_this: Ref, c_mirror: Ref) -> Res<i32> {
    let c = rt::class_of_mirror(rt::nn_ref(c_mirror)?);
    Ok(c.init.is_some() as i32)
}

pub fn n_jdk_internal_misc_Unsafe__throwException(_this: Ref, t: Ref) -> Res<()> {
    rt::throw_ref(t)
}

pub fn n_jdk_internal_misc_Unsafe__allocateMemory0(_this: Ref, bytes: i64) -> Res<i64> {
    let sz = bytes.max(1) as usize;
    let layout = std::alloc::Layout::from_size_align(sz, 16)
        .map_err(|_| rt::Thrown { obj: (rt::hooks().throw_new)("java/lang/OutOfMemoryError", rt::new_str("bad alloc size".to_string())) })?;
    let p = unsafe { std::alloc::alloc(layout) };
    if p.is_null() {
        return rt::throw("java/lang/OutOfMemoryError", "Unable to allocate native memory".to_string());
    }
    // Store size right in malloc via C libc so realloc/free work uniformly across C FFI and Unsafe
    unsafe { std::alloc::dealloc(p, layout) };
    extern "C" {
        fn malloc(size: usize) -> *mut c_void;
    }
    let mp = unsafe { malloc(sz) };
    if mp.is_null() {
        return rt::throw("java/lang/OutOfMemoryError", "Unable to allocate native memory".to_string());
    }
    Ok(mp as usize as i64)
}

pub fn n_jdk_internal_misc_Unsafe__reallocateMemory0(_this: Ref, addr: i64, bytes: i64) -> Res<i64> {
    extern "C" {
        fn realloc(ptr: *mut c_void, size: usize) -> *mut c_void;
    }
    let p = addr as usize as *mut c_void;
    let np = unsafe { realloc(p, bytes.max(1) as usize) };
    if np.is_null() {
        return rt::throw("java/lang/OutOfMemoryError", "Unable to reallocate native memory".to_string());
    }
    Ok(np as usize as i64)
}

pub fn n_jdk_internal_misc_Unsafe__freeMemory0(_this: Ref, addr: i64) -> Res<()> {
    extern "C" {
        fn free(ptr: *mut c_void);
    }
    if addr != 0 {
        unsafe { free(addr as usize as *mut c_void) };
    }
    Ok(())
}

pub fn n_jdk_internal_misc_Unsafe__setMemory0(_this: Ref, o: Ref, off: i64, bytes: i64, val: i32) -> Res<()> {
    if bytes <= 0 {
        return Ok(());
    }
    unsafe { std::ptr::write_bytes(uptr(o, off), val as u8, bytes as usize) };
    Ok(())
}

pub fn n_jdk_internal_misc_Unsafe__copySwapMemory0(_this: Ref, so: Ref, soff: i64, d: Ref, doff: i64, bytes: i64, esz: i64) -> Res<()> {
    let len = bytes.max(0) as usize;
    let es = esz.max(1) as usize;
    let sp = uptr(so, soff) as *const u8;
    let dp = uptr(d, doff);
    unsafe {
        let mut i = 0;
        while i + es <= len {
            for k in 0..es {
                *dp.add(i + k) = *sp.add(i + es - 1 - k);
            }
            i += es;
        }
    }
    Ok(())
}

pub fn n_jdk_internal_misc_Unsafe__getBoolean(_this: Ref, o: Ref, off: i64) -> Res<i32> {
    Ok(unsafe { std::ptr::read_unaligned(uptr(o, off) as *const i8) != 0 } as i32)
}
pub fn n_jdk_internal_misc_Unsafe__getBooleanVolatile(_this: Ref, o: Ref, off: i64) -> Res<i32> {
    Ok(unsafe { (*(uptr(o, off) as *const AtomicI8)).load(Ordering::SeqCst) != 0 } as i32)
}
pub fn n_jdk_internal_misc_Unsafe__putBoolean(_this: Ref, o: Ref, off: i64, v: i32) -> Res<()> {
    unsafe { std::ptr::write_unaligned(uptr(o, off) as *mut i8, if v != 0 { 1 } else { 0 }) };
    Ok(())
}
pub fn n_jdk_internal_misc_Unsafe__putBooleanVolatile(_this: Ref, o: Ref, off: i64, v: i32) -> Res<()> {
    unsafe { (*(uptr(o, off) as *const AtomicI8)).store(if v != 0 { 1 } else { 0 }, Ordering::SeqCst) };
    Ok(())
}

pub fn n_jdk_internal_misc_Unsafe__getByteVolatile(_this: Ref, o: Ref, off: i64) -> Res<i32> {
    Ok(unsafe { (*(uptr(o, off) as *const AtomicI8)).load(Ordering::SeqCst) as i32 })
}
pub fn n_jdk_internal_misc_Unsafe__putByte(_this: Ref, o: Ref, off: i64, v: i32) -> Res<()> {
    unsafe { std::ptr::write_unaligned(uptr(o, off) as *mut i8, v as i8) };
    Ok(())
}
pub fn n_jdk_internal_misc_Unsafe__putByteVolatile(_this: Ref, o: Ref, off: i64, v: i32) -> Res<()> {
    unsafe { (*(uptr(o, off) as *const AtomicI8)).store(v as i8, Ordering::SeqCst) };
    Ok(())
}

pub fn n_jdk_internal_misc_Unsafe__getShortVolatile(_this: Ref, o: Ref, off: i64) -> Res<i32> {
    Ok(unsafe { (*(uptr(o, off) as *const AtomicI16)).load(Ordering::SeqCst) as i32 })
}
pub fn n_jdk_internal_misc_Unsafe__putShort(_this: Ref, o: Ref, off: i64, v: i32) -> Res<()> {
    unsafe { std::ptr::write_unaligned(uptr(o, off) as *mut i16, v as i16) };
    Ok(())
}
pub fn n_jdk_internal_misc_Unsafe__putShortVolatile(_this: Ref, o: Ref, off: i64, v: i32) -> Res<()> {
    unsafe { (*(uptr(o, off) as *const AtomicI16)).store(v as i16, Ordering::SeqCst) };
    Ok(())
}

pub fn n_jdk_internal_misc_Unsafe__getChar(_this: Ref, o: Ref, off: i64) -> Res<i32> {
    Ok(unsafe { std::ptr::read_unaligned(uptr(o, off) as *const u16) as i32 })
}
pub fn n_jdk_internal_misc_Unsafe__getCharVolatile(_this: Ref, o: Ref, off: i64) -> Res<i32> {
    Ok(unsafe { (*(uptr(o, off) as *const AtomicU16)).load(Ordering::SeqCst) as i32 })
}
pub fn n_jdk_internal_misc_Unsafe__putChar(_this: Ref, o: Ref, off: i64, v: i32) -> Res<()> {
    unsafe { std::ptr::write_unaligned(uptr(o, off) as *mut u16, v as u16) };
    Ok(())
}
pub fn n_jdk_internal_misc_Unsafe__putCharVolatile(_this: Ref, o: Ref, off: i64, v: i32) -> Res<()> {
    unsafe { (*(uptr(o, off) as *const AtomicU16)).store(v as u16, Ordering::SeqCst) };
    Ok(())
}

pub fn n_jdk_internal_misc_Unsafe__putInt(_this: Ref, o: Ref, off: i64, v: i32) -> Res<()> {
    unsafe { std::ptr::write_unaligned(uptr(o, off) as *mut i32, v) };
    Ok(())
}
pub fn n_jdk_internal_misc_Unsafe__putIntVolatile(_this: Ref, o: Ref, off: i64, v: i32) -> Res<()> {
    unsafe { (*(uptr(o, off) as *const AtomicI32)).store(v, Ordering::SeqCst) };
    Ok(())
}

pub fn n_jdk_internal_misc_Unsafe__putLong(_this: Ref, o: Ref, off: i64, v: i64) -> Res<()> {
    unsafe { std::ptr::write_unaligned(uptr(o, off) as *mut i64, v) };
    Ok(())
}
pub fn n_jdk_internal_misc_Unsafe__putLongVolatile(_this: Ref, o: Ref, off: i64, v: i64) -> Res<()> {
    unsafe { (*(uptr(o, off) as *const AtomicI64)).store(v, Ordering::SeqCst) };
    Ok(())
}

pub fn n_jdk_internal_misc_Unsafe__getFloat(_this: Ref, o: Ref, off: i64) -> Res<f32> {
    Ok(unsafe { std::ptr::read_unaligned(uptr(o, off) as *const f32) })
}
pub fn n_jdk_internal_misc_Unsafe__getFloatVolatile(_this: Ref, o: Ref, off: i64) -> Res<f32> {
    Ok(f32::from_bits(unsafe { (*(uptr(o, off) as *const AtomicU32)).load(Ordering::SeqCst) }))
}
pub fn n_jdk_internal_misc_Unsafe__putFloat(_this: Ref, o: Ref, off: i64, v: f32) -> Res<()> {
    unsafe { std::ptr::write_unaligned(uptr(o, off) as *mut f32, v) };
    Ok(())
}
pub fn n_jdk_internal_misc_Unsafe__putFloatVolatile(_this: Ref, o: Ref, off: i64, v: f32) -> Res<()> {
    unsafe { (*(uptr(o, off) as *const AtomicU32)).store(v.to_bits(), Ordering::SeqCst) };
    Ok(())
}

pub fn n_jdk_internal_misc_Unsafe__getDouble(_this: Ref, o: Ref, off: i64) -> Res<f64> {
    Ok(unsafe { std::ptr::read_unaligned(uptr(o, off) as *const f64) })
}
pub fn n_jdk_internal_misc_Unsafe__getDoubleVolatile(_this: Ref, o: Ref, off: i64) -> Res<f64> {
    Ok(f64::from_bits(unsafe { (*(uptr(o, off) as *const AtomicU64)).load(Ordering::SeqCst) }))
}
pub fn n_jdk_internal_misc_Unsafe__putDouble(_this: Ref, o: Ref, off: i64, v: f64) -> Res<()> {
    unsafe { std::ptr::write_unaligned(uptr(o, off) as *mut f64, v) };
    Ok(())
}
pub fn n_jdk_internal_misc_Unsafe__putDoubleVolatile(_this: Ref, o: Ref, off: i64, v: f64) -> Res<()> {
    unsafe { (*(uptr(o, off) as *const AtomicU64)).store(v.to_bits(), Ordering::SeqCst) };
    Ok(())
}

pub fn n_jdk_internal_misc_Unsafe__compareAndExchangeInt(_this: Ref, o: Ref, off: i64, expected: i32, x: i32) -> Res<i32> {
    let a = unsafe { &*(uptr(o, off) as *const AtomicI32) };
    Ok(match a.compare_exchange(expected, x, Ordering::SeqCst, Ordering::SeqCst) {
        Ok(v) | Err(v) => v,
    })
}

pub fn n_jdk_internal_misc_Unsafe__writeback0(_this: Ref, _addr: i64) -> Res<()> { Ok(()) }
pub fn n_jdk_internal_misc_Unsafe__writebackPreSync0(_this: Ref) -> Res<()> {
    std::sync::atomic::fence(Ordering::SeqCst);
    Ok(())
}
pub fn n_jdk_internal_misc_Unsafe__writebackPostSync0(_this: Ref) -> Res<()> {
    std::sync::atomic::fence(Ordering::SeqCst);
    Ok(())
}

pub fn n_jdk_internal_misc_Unsafe__getLoadAverage0(_this: Ref, loadavg: Ref, nelems: i32) -> Res<i32> {
    if loadavg.is_null() || nelems <= 0 {
        return Ok(-1);
    }
    let a = unsafe { &*(loadavg.0 as *const Arr) };
    let n = (nelems as usize).min(a.len as usize).min(3);
    let mut vals = [0.0f64; 3];
    let rc = unsafe { getloadavg(vals.as_mut_ptr(), n as c_int) };
    if rc > 0 {
        let dst = unsafe { std::slice::from_raw_parts_mut((loadavg.0 as *mut u8).add(24) as *mut f64, a.len as usize) };
        for i in 0..(rc as usize).min(dst.len()) {
            dst[i] = vals[i];
        }
    }
    Ok(rc)
}
