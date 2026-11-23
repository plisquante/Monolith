//! POSIX C functions that the runtime declares, implemented for Windows on top of the Win32 API. The natives call them
//! by their POSIX names; this module provides the definitions so that the Windows executable links. Each function
//! returns the POSIX error convention and sets errno (ENOSYS, 38) for what Windows does not offer, so a missing
//! feature fails at the call instead of silently succeeding.

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

// ----- dynamic loading (dlfcn.h) on LoadLibrary / GetProcAddress

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

// ----- system configuration (unistd.h, sys/utsname.h)

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

// ----- signals: Windows has no sigaction; the runtime's null-pointer trap is not installed on this platform

#[no_mangle]
pub unsafe extern "C" fn sigaction(_sig: c_int, _act: *const u8, _oldact: *mut u8) -> c_int {
    fail_errno(ENOSYS);
    -1
}

// ----- memory mapping: anonymous mappings through VirtualAlloc; file mappings are not supported here

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
