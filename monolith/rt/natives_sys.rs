//! System, Process, Module, StackWalker, MethodHandle/MethodHandleNatives, CgroupMetrics,
//! Continuation, Foreign/LibFallback (libffi), Management, NativeLibraries, and JFR natives.
use crate::gc::ScopedBlock;
use crate::natives_io::{loaded_lib_handles, register_lib_handle};
use crate::rt::{self, Arr, FLoc, Ref, Res};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_long, c_uint, c_ulong, c_void};
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[repr(C)]
struct SysInfo {
    uptime: c_long,
    loads: [c_ulong; 3],
    totalram: c_ulong,
    freeram: c_ulong,
    sharedram: c_ulong,
    bufferram: c_ulong,
    totalswap: c_ulong,
    freeswap: c_ulong,
    procs: u16,
    pad: u16,
    totalhigh: c_ulong,
    freehigh: c_ulong,
    mem_unit: c_uint,
    _f: [u8; 256],
}

#[link(name = "c")]
extern "C" {
    fn __errno_location() -> *mut c_int;
    fn strerror(errnum: c_int) -> *const c_char;
    fn getpid() -> c_int;
    fn kill(pid: c_int, sig: c_int) -> c_int;
    fn waitpid(pid: c_int, status: *mut c_int, options: c_int) -> c_int;
    fn usleep(usec: c_uint) -> c_int;
    fn sysinfo(info: *mut SysInfo) -> c_int;
    fn pipe2(fds: *mut c_int, flags: c_int) -> c_int;
    fn close(fd: c_int) -> c_int;
    fn malloc(size: usize) -> *mut c_void;
    fn posix_spawnp(
        pid: *mut c_int,
        file: *const c_char,
        fa: *const c_void,
        attr: *const c_void,
        argv: *const *const c_char,
        envp: *const *const c_char,
    ) -> c_int;
    fn posix_spawn_file_actions_init(fa: *mut c_void) -> c_int;
    fn posix_spawn_file_actions_destroy(fa: *mut c_void) -> c_int;
    fn posix_spawn_file_actions_adddup2(fa: *mut c_void, fd: c_int, newfd: c_int) -> c_int;
    fn posix_spawn_file_actions_addclose(fa: *mut c_void, fd: c_int) -> c_int;
    fn posix_spawn_file_actions_addchdir_np(fa: *mut c_void, path: *const c_char) -> c_int;
    static environ: *const *const c_char;
}

#[link(name = "dl")]
extern "C" {
    fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlclose(handle: *mut c_void) -> c_int;
    fn dlerror() -> *const c_char;
}

#[link(name = "ffi")]
extern "C" {
    static ffi_type_void: u8;
    static ffi_type_uint8: u8;
    static ffi_type_sint8: u8;
    static ffi_type_uint16: u8;
    static ffi_type_sint16: u8;
    static ffi_type_uint32: u8;
    static ffi_type_sint32: u8;
    static ffi_type_uint64: u8;
    static ffi_type_sint64: u8;
    static ffi_type_float: u8;
    static ffi_type_double: u8;
    static ffi_type_pointer: u8;
    fn ffi_prep_cif(cif: *mut c_void, abi: c_int, nargs: c_uint, rtype: *mut c_void, atypes: *mut c_void) -> c_int;
    fn ffi_prep_cif_var(
        cif: *mut c_void, abi: c_int, nfixed: c_uint, ntotal: c_uint, rtype: *mut c_void, atypes: *mut c_void,
    ) -> c_int;
    fn ffi_get_struct_offsets(abi: c_int, st: *mut c_void, offsets: *mut usize) -> c_int;
    fn ffi_call(cif: *mut c_void, fn_ptr: *const c_void, rvalue: *mut c_void, avalue: *mut *mut c_void);
}

const EINTR: i32 = 4;
const ESRCH: i32 = 3;
const RTLD_NOW: c_int = 2;
const RTLD_GLOBAL: c_int = 0x100;

static START_MS: AtomicI64 = AtomicI64::new(0);
static START_INSTANT: OnceLock<Instant> = OnceLock::new();

fn start_ms() -> i64 {
    let cur = START_MS.load(Ordering::Relaxed);
    if cur != 0 {
        return cur;
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
    START_MS.store(now, Ordering::Relaxed);
    let _ = START_INSTANT.get_or_init(Instant::now);
    now
}

fn new_bytes(b: &[u8]) -> Res<Ref> {
    let a = rt::new_array(b'B', b.len() as i32)?;
    unsafe { rt::byte_slice_mut(a)?.copy_from_slice(b) };
    Ok(a)
}

// ------------------------------------------------------------------ Module
pub fn n_java_lang_Module__defineModule0(_m: Ref, _open: i32, _ver: Ref, _loc: Ref, _pkgs: Ref) -> Res<()> { Ok(()) }
pub fn n_java_lang_Module__addReads0(_a: Ref, _b: Ref) -> Res<()> { Ok(()) }
pub fn n_java_lang_Module__addExports0(_a: Ref, _pkg: Ref, _to: Ref) -> Res<()> { Ok(()) }
pub fn n_java_lang_Module__addExportsToAll0(_a: Ref, _pkg: Ref) -> Res<()> { Ok(()) }
pub fn n_java_lang_Module__addExportsToAllUnnamed0(_a: Ref, _pkg: Ref) -> Res<()> { Ok(()) }

// ------------------------------------------------------------------ ProcessEnvironment / ProcessHandleImpl / ProcessImpl
pub fn n_java_lang_ProcessEnvironment__environ() -> Res<Ref> {
    let mut pairs: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
    unsafe {
        let mut p = environ;
        while !p.is_null() && !(*p).is_null() {
            let s = CStr::from_ptr(*p).to_bytes();
            if let Some(eq) = s.iter().position(|&b| b == b'=') {
                if eq > 0 {
                    pairs.push((s[..eq].to_vec(), s[eq + 1..].to_vec()));
                }
            }
            p = p.add(1);
        }
    }
    let b_arr_cls = rt::array_class_of(&rt::CLS_P_BYTE);
    let out = rt::new_obj_array(b_arr_cls, (pairs.len() * 2) as i32)?;
    for (i, (k, v)) in pairs.into_iter().enumerate() {
        let kb = new_bytes(&k)?;
        let vb = new_bytes(&v)?;
        rt::aastore_ref(out, (i * 2) as i32, kb)?;
        rt::aastore_ref(out, (i * 2 + 1) as i32, vb)?;
    }
    Ok(out)
}

pub fn n_java_lang_ProcessHandleImpl__initNative() -> Res<()> { Ok(()) }
pub fn n_java_lang_ProcessHandleImpl__getCurrentPid0() -> Res<i64> { Ok(unsafe { getpid() as i64 }) }

pub fn n_java_lang_ProcessHandleImpl__isAlive0(pid: i64) -> Res<i64> {
    if unsafe { kill(pid as c_int, 0) } != 0 && unsafe { *__errno_location() } == ESRCH {
        return Ok(-1);
    }
    Ok(0)
}

pub fn n_java_lang_ProcessHandleImpl__destroy0(pid: i64, _start: i64, force: i32) -> Res<i32> {
    Ok((unsafe { kill(pid as c_int, if force != 0 { 9 } else { 15 }) } == 0) as i32)
}

pub fn n_java_lang_ProcessHandleImpl__waitForProcessExit0(pid: i64, reap: i32) -> Res<i32> {
    let _blk = ScopedBlock::enter();
    let mut status: c_int = 0;
    if reap != 0 {
        loop {
            let r = unsafe { waitpid(pid as c_int, &mut status, 0) };
            if r >= 0 {
                break;
            }
            if unsafe { *__errno_location() } == EINTR {
                continue;
            }
            return Ok(-1);
        }
        if (status & 0x7f) == 0 {
            return Ok((status >> 8) & 0xff);
        }
        return Ok(0x80 + (status & 0x7f));
    }
    while unsafe { kill(pid as c_int, 0) } == 0 {
        unsafe { usleep(10_000) };
    }
    Ok(0)
}

pub fn n_java_lang_ProcessImpl__init() -> Res<()> { Ok(()) }

fn split_nul(block: &[u8], count: usize) -> Vec<CString> {
    let mut out = Vec::with_capacity(count);
    let mut pos = 0usize;
    for _ in 0..count {
        let end = block[pos..].iter().position(|&b| b == 0).map(|d| pos + d).unwrap_or(block.len());
        out.push(CString::new(&block[pos..end]).unwrap_or_else(|_| CString::new("").unwrap()));
        pos = (end + 1).min(block.len());
    }
    out
}

pub fn n_java_lang_ProcessImpl__forkAndExec(
    _this: Ref, _mode: i32, _helper: Ref, prog: Ref, args: Ref, argc: i32, envb: Ref, envc: i32,
    dir: Ref, fds: Ref, redirect: i32,
) -> Res<i32> {
    let pb = unsafe { rt::byte_slice(prog)? };
    let pend = pb.iter().position(|&b| b == 0).unwrap_or(pb.len());
    let prog_c = CString::new(&pb[..pend]).unwrap_or_else(|_| CString::new("").unwrap());
    let rest = if args.is_null() {
        Vec::new()
    } else {
        split_nul(unsafe { rt::byte_slice(args)? }, argc.max(0) as usize)
    };
    let mut argv: Vec<*const c_char> = Vec::with_capacity(rest.len() + 2);
    argv.push(prog_c.as_ptr());
    for r in &rest {
        argv.push(r.as_ptr());
    }
    argv.push(std::ptr::null());

    let _env_store: Vec<CString>;
    let mut env_ptrs: Vec<*const c_char> = Vec::new();
    let envp = if envb.is_null() {
        unsafe { environ }
    } else {
        _env_store = split_nul(unsafe { rt::byte_slice(envb)? }, envc.max(0) as usize);
        for e in &_env_store {
            env_ptrs.push(e.as_ptr());
        }
        env_ptrs.push(std::ptr::null());
        env_ptrs.as_ptr()
    };

    let _ = rt::nn(fds)?;
    let fd_arr = unsafe { std::slice::from_raw_parts_mut((fds.0 as *mut u8).add(24) as *mut i32, 3) };
    let mut child_fd = [0 as c_int; 3];
    let mut parent_fd = [-1 as c_int; 3];
    let mut to_close = Vec::new();
    for i in 0..3usize {
        if i == 2 && redirect != 0 {
            child_fd[2] = -2;
            continue;
        }
        if fd_arr[i] == -1 {
            let mut p = [0 as c_int; 2];
            if unsafe { pipe2(p.as_mut_ptr(), 0o2000000) } != 0 {
                for &c in &to_close { unsafe { close(c) }; }
                return rt::throw("java/io/IOException", unsafe { CStr::from_ptr(strerror(*__errno_location())).to_string_lossy().into_owned() });
            }
            if i == 0 {
                child_fd[0] = p[0];
                parent_fd[0] = p[1];
                to_close.push(p[0]);
            } else {
                child_fd[i] = p[1];
                parent_fd[i] = p[0];
                to_close.push(p[1]);
            }
        } else {
            child_fd[i] = fd_arr[i];
        }
    }

    let mut fa = [0u64; 16]; // >= 80 bytes for posix_spawn_file_actions_t
    unsafe { posix_spawn_file_actions_init(fa.as_mut_ptr() as *mut c_void) };
    for i in 0..3usize {
        if child_fd[i] != -2 {
            unsafe { posix_spawn_file_actions_adddup2(fa.as_mut_ptr() as *mut c_void, child_fd[i], i as c_int) };
        }
    }
    if child_fd[2] == -2 {
        unsafe { posix_spawn_file_actions_adddup2(fa.as_mut_ptr() as *mut c_void, 1, 2) };
    }
    for &pf in &parent_fd {
        if pf >= 0 {
            unsafe { posix_spawn_file_actions_addclose(fa.as_mut_ptr() as *mut c_void, pf) };
        }
    }
    let dir_c: Option<CString>;
    if !dir.is_null() {
        let db = unsafe { rt::byte_slice(dir)? };
        let dend = db.iter().position(|&b| b == 0).unwrap_or(db.len());
        dir_c = CString::new(&db[..dend]).ok();
        if let Some(ref dc) = dir_c {
            let crc = unsafe { posix_spawn_file_actions_addchdir_np(fa.as_mut_ptr() as *mut c_void, dc.as_ptr()) };
            if crc != 0 {
                unsafe { posix_spawn_file_actions_destroy(fa.as_mut_ptr() as *mut c_void) };
                for &c in &to_close { unsafe { close(c) }; }
                for &p in &parent_fd { if p >= 0 { unsafe { close(p) }; } }
                return rt::throw("java/io/IOException", format!("error={}, cannot set chdir", crc));
            }
        }
    }

    let mut pid: c_int = 0;
    let rc = {
        let _blk = ScopedBlock::enter();
        unsafe { posix_spawnp(&mut pid, prog_c.as_ptr(), fa.as_ptr() as *const c_void, std::ptr::null(), argv.as_ptr(), envp) }
    };
    unsafe { posix_spawn_file_actions_destroy(fa.as_mut_ptr() as *mut c_void) };
    for &c in &to_close { unsafe { close(c) }; }
    if rc != 0 {
        for &p in &parent_fd { if p >= 0 { unsafe { close(p) }; } }
        let msg = unsafe { CStr::from_ptr(strerror(rc)).to_string_lossy().into_owned() };
        return rt::throw("java/io/IOException", format!("error={}, {}", rc, msg));
    }
    for i in 0..3usize {
        fd_arr[i] = parent_fd[i];
    }
    Ok(pid)
}

// ------------------------------------------------------------------ StackWalker / MethodHandle / MethodHandleNatives
static METHOD_ADDR_TABLE: OnceLock<Vec<(usize, &'static rt::Class, &'static rt::MInfo)>> = OnceLock::new();

fn method_addr_table() -> &'static [(usize, &'static rt::Class, &'static rt::MInfo)] {
    METHOD_ADDR_TABLE.get_or_init(|| {
        let mut v = Vec::new();
        for &c in rt::hooks().all_classes {
            for m in c.methods {
                let p = m.fp.0 as usize;
                if p != 0 && p != (rt::unreachable_slot as *const () as usize) {
                    v.push((p, c, m));
                }
            }
        }
        v.sort_unstable_by_key(|e| e.0);
        v
    })
}

#[cfg(target_os = "linux")]
unsafe extern "C" {
    fn backtrace(buffer: *mut *mut c_void, size: c_int) -> c_int;
}

#[cfg(target_os = "windows")]
unsafe extern "system" {
    fn RtlCaptureStackBackTrace(frames_to_skip: u32, frames_to_capture: u32, back_trace: *mut *mut c_void, back_trace_hash: *mut u32) -> u16;
}

pub fn n_java_lang_StackWalker__frames0() -> Res<Ref> {
    let mut raw = [std::ptr::null_mut::<c_void>(); 128];
    let n: usize = {
        #[cfg(target_os = "linux")]
        {
            let cnt = unsafe { backtrace(raw.as_mut_ptr(), raw.len() as c_int) };
            if cnt > 0 { cnt as usize } else { 0 }
        }
        #[cfg(target_os = "windows")]
        {
            unsafe { RtlCaptureStackBackTrace(0, raw.len() as u32, raw.as_mut_ptr(), std::ptr::null_mut()) as usize }
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            0
        }
    };
    let table = method_addr_table();
    let mut frames: Vec<(Ref, &'static str, &'static str)> = Vec::new();
    for &ptr in &raw[..n] {
        let ip = ptr as usize;
        if ip == 0 { continue; }
        let idx = table.partition_point(|e| e.0 <= ip);
        if idx > 0 {
            let (start, c, m) = table[idx - 1];
            if ip.saturating_sub(start) < 512 * 1024 {
                frames.push((rt::class_mirror(c)?, m.name, m.desc));
            }
        }
    }
    let obj_cls = rt::hooks().object_class;
    let cls_cls = rt::hooks().class_class;
    let str_cls = rt::hooks().string_class;
    let len = frames.len() as i32;
    let ca = rt::new_obj_array(cls_cls, len)?;
    let na = rt::new_obj_array(str_cls, len)?;
    let da = rt::new_obj_array(str_cls, len)?;
    let fa = rt::new_obj_array(str_cls, len)?;
    let la = rt::new_array(b'I', len)?;
    for (i, (cm, mname, mdesc)) in frames.into_iter().enumerate() {
        let idx = i as i32;
        rt::aastore_ref(ca, idx, cm)?;
        rt::aastore_ref(na, idx, rt::new_str(mname.to_string()))?;
        rt::aastore_ref(da, idx, rt::new_str(mdesc.to_string()))?;
        rt::aastore_ref(fa, idx, Ref::NULL)?;
    }
    let out = rt::new_obj_array(obj_cls, 5)?;
    rt::aastore_ref(out, 0, ca)?;
    rt::aastore_ref(out, 1, na)?;
    rt::aastore_ref(out, 2, da)?;
    rt::aastore_ref(out, 3, fa)?;
    rt::aastore_ref(out, 4, la)?;
    Ok(out)
}

pub fn n_java_lang_invoke_MethodHandle__invoke(this: Ref, args: Ref) -> Res<Ref> {
    rt::call_reflective("java/lang/invoke/MethodHandle", "j2zInvoke", "([Ljava/lang/Object;)Ljava/lang/Object;", this, &[args])
}

fn mn_field(mn: Ref) -> Res<(&'static rt::Class, &'static rt::FInfo)> {
    let o = rt::nn_ref(mn)?;
    let cm = rt::nn_ref(rt::get_ref(o, "clazz"))?;
    let c = rt::class_of_mirror(cm);
    let name = rt::str_to_rust(rt::nn_ref(rt::get_ref(o, "name"))?);
    let flags = rt::get_i32(o, "flags");
    let is_static = (flags & 0x0008) != 0;
    if let Some((owner, f)) = c.find_field(&name, is_static) {
        return Ok((owner, f));
    }
    if let Some((owner, f)) = c.find_field(&name, !is_static) {
        return Ok((owner, f));
    }
    rt::throw("java/lang/NoSuchFieldError", format!("{}.{}", c.name.replace('/', "."), name))
}

pub fn n_java_lang_invoke_MethodHandleNatives__objectFieldOffset(mn: Ref) -> Res<i64> {
    let (_, f) = mn_field(mn)?;
    match f.loc {
        FLoc::Off(off) => Ok(off as i64),
        FLoc::Static(_) => rt::throw("java/lang/InternalError", "static field in objectFieldOffset".to_string()),
    }
}

pub fn n_java_lang_invoke_MethodHandleNatives__staticFieldOffset(mn: Ref) -> Res<i64> {
    let (_, f) = mn_field(mn)?;
    match f.loc {
        FLoc::Static(ref sp) => Ok(sp.0 as usize as i64),
        FLoc::Off(_) => rt::throw("java/lang/InternalError", "instance field in staticFieldOffset".to_string()),
    }
}

pub fn n_java_lang_invoke_MethodHandleNatives__staticFieldBase(_mn: Ref) -> Res<Ref> {
    Ok(Ref::NULL)
}

pub fn n_java_lang_invoke_MethodHandleNatives__getMemberVMInfo(mn: Ref) -> Res<Ref> {
    let out = rt::new_obj_array(rt::hooks().object_class, 2)?;
    let o = rt::nn_ref(mn)?;
    let flags = rt::get_i32(o, "flags");
    let vm_idx: i64 = if (flags & 0x0004_0000) != 0 {
        n_java_lang_invoke_MethodHandleNatives__objectFieldOffset(mn).unwrap_or(0)
    } else {
        0
    };
    if let Some(lcls) = rt::find_class("java/lang/Long") {
        let lo = rt::alloc(lcls);
        rt::set_i64(lo, "value", vm_idx);
        rt::aastore_ref(out, 0, lo)?;
    }
    rt::aastore_ref(out, 1, mn)?;
    Ok(out)
}

pub fn n_java_lang_invoke_MethodHandleNatives__resolve(
    self_mn: Ref, _caller: Ref, _lookup_mode: i32, speculative: i32,
) -> Res<Ref> {
    let o = rt::nn_ref(self_mn)?;
    let cm = rt::get_ref(o, "clazz");
    let name_ref = rt::get_ref(o, "name");
    if cm.is_null() || name_ref.is_null() {
        return Ok(if speculative != 0 { Ref::NULL } else { self_mn });
    }
    let c = rt::class_of_mirror(cm);
    let name = rt::str_to_rust(name_ref);
    let mut flags = rt::get_i32(o, "flags");
    if (flags & 0x0004_0000) != 0 {
        let is_static = (flags & 0x0008) != 0;
        if let Some((_, f)) = c.find_field(&name, is_static).or_else(|| c.find_field(&name, !is_static)) {
            flags = (flags & !0xffff) | (f.access as i32);
            rt::set_i32(o, "flags", flags);
            return Ok(self_mn);
        }
    } else {
        let mut cur: Option<&'static rt::Class> = Some(c);
        while let Some(kc) = cur {
            if let Some(m) = kc.methods.iter().find(|m| m.name == name) {
                flags = (flags & !0xffff) | (m.access as i32);
                rt::set_i32(o, "flags", flags);
                return Ok(self_mn);
            }
            cur = kc.super_;
        }
    }
    if speculative != 0 {
        Ok(Ref::NULL)
    } else {
        rt::throw("java/lang/NoSuchMethodError", format!("{}.{}", c.name.replace('/', "."), name))
    }
}

// ------------------------------------------------------------------ Cgroup / Continuation / Foreign / Perf / Management
pub fn n_jdk_internal_platform_CgroupMetrics__isUseContainerSupport() -> Res<i32> { Ok(0) }

pub fn n_jdk_internal_platform_CgroupMetrics__getTotalMemorySize0() -> Res<i64> {
    let mut si: SysInfo = unsafe { std::mem::zeroed() };
    if unsafe { sysinfo(&mut si) } != 0 { Ok(0) } else { Ok((si.totalram as u64 * si.mem_unit as u64) as i64) }
}

pub fn n_jdk_internal_platform_CgroupMetrics__getTotalSwapSize0() -> Res<i64> {
    let mut si: SysInfo = unsafe { std::mem::zeroed() };
    if unsafe { sysinfo(&mut si) } != 0 { Ok(0) } else { Ok((si.totalswap as u64 * si.mem_unit as u64) as i64) }
}

pub fn n_jdk_internal_vm_ContinuationSupport__isSupported0() -> Res<i32> { Ok(0) }
pub fn n_jdk_internal_vm_Continuation__registerNatives() -> Res<()> { Ok(()) }
pub fn n_jdk_internal_vm_Continuation__pin() -> Res<()> { Ok(()) }
pub fn n_jdk_internal_vm_Continuation__unpin() -> Res<()> { Ok(()) }
pub fn n_jdk_internal_vm_ForeignLinkerSupport__isSupported0() -> Res<i32> { Ok(1) }

pub fn n_jdk_internal_perf_Perf__registerNatives() -> Res<()> { Ok(()) }
pub fn n_jdk_internal_perf_Perf__createLong(_this: Ref, _name: Ref, _var: i32, _units: i32, value: i64) -> Res<Ref> {
    let p = unsafe { malloc(8) as *mut i64 };
    if p.is_null() {
        return rt::throw("java/lang/OutOfMemoryError", "perf counter".to_string());
    }
    unsafe { *p = value };
    rt::new_direct_byte_buffer(p as usize as i64, 8)
}

pub fn n_jdk_internal_misc_ScopedMemoryAccess__closeScope0(_this: Ref, _session: Ref, _err: Ref) -> Res<()> {
    std::sync::atomic::fence(Ordering::SeqCst);
    Ok(())
}
pub fn n_java_awt_AWTEvent__initIDs() -> Res<()> { Ok(()) }
pub fn n_java_awt_Component__initIDs() -> Res<()> { Ok(()) }
pub fn n_java_awt_Toolkit__initIDs() -> Res<()> { Ok(()) }

pub fn n_sun_management_VMManagementImpl__getVersion0() -> Res<Ref> { Ok(rt::new_str("4.0".to_string())) }
pub fn n_sun_management_VMManagementImpl__initOptionalSupportFields() -> Res<()> { Ok(()) }
pub fn n_sun_management_VMManagementImpl__isThreadCpuTimeEnabled(_this: Ref) -> Res<i32> { Ok(0) }
pub fn n_sun_management_VMManagementImpl__isThreadAllocatedMemoryEnabled(_this: Ref) -> Res<i32> { Ok(0) }
pub fn n_sun_management_VMManagementImpl__getProcessId(_this: Ref) -> Res<i32> { Ok(unsafe { getpid() }) }
pub fn n_sun_management_VMManagementImpl__getVmArguments0(_this: Ref) -> Res<Ref> { rt::string_array(&[]) }
pub fn n_sun_management_VMManagementImpl__getStartupTime(_this: Ref) -> Res<i64> { Ok(start_ms()) }
pub fn n_sun_management_VMManagementImpl__getUptime0(_this: Ref) -> Res<i64> {
    let _ = start_ms();
    Ok(START_INSTANT.get_or_init(Instant::now).elapsed().as_millis() as i64)
}

pub fn n_sun_management_MemoryImpl__getMemoryPools0() -> Res<Ref> {
    rt::new_obj_array(rt::hooks().object_class, 0)
}
pub fn n_sun_management_MemoryImpl__getMemoryManagers0() -> Res<Ref> {
    rt::new_obj_array(rt::hooks().object_class, 0)
}
pub fn n_sun_management_MemoryImpl__getMemoryUsage0(_this: Ref, heap: i32) -> Res<Ref> {
    let c = rt::find_class("java/lang/management/MemoryUsage")
        .ok_or_else(|| rt::Thrown { obj: (rt::hooks().throw_new)("java/lang/InternalError", rt::new_str("MemoryUsage missing".to_string())) })?;
    let o = rt::alloc(c);
    rt::set_i64(o, "init", 0);
    if heap != 0 {
        let used = crate::gc::live_bytes() as i64;
        let comm = crate::gc::heap_bytes().max(used as usize) as i64;
        rt::set_i64(o, "used", used);
        rt::set_i64(o, "committed", comm);
        rt::set_i64(o, "max", -1);
    } else {
        rt::set_i64(o, "used", 0);
        rt::set_i64(o, "committed", 0);
        rt::set_i64(o, "max", -1);
    }
    Ok(o)
}
pub fn n_sun_management_ThreadImpl__dumpThreads0(_ids: Ref, _locks: i32, _syncs: i32, _depth: i32) -> Res<Ref> {
    rt::throw("java/lang/UnsupportedOperationException", "ThreadMXBean.dumpThreads unsupported".to_string())
}

// ------------------------------------------------------------------ NativeLibraries / dlopen / dlsym
pub fn n_jdk_internal_loader_NativeLibraries__findBuiltinLib(name: Ref) -> Res<Ref> {
    if name.is_null() {
        return Ok(Ref::NULL);
    }
    let s = rt::str_to_rust(name);
    if s.contains("fallbackLinker") {
        return Ok(name);
    }
    Ok(Ref::NULL)
}

pub fn n_jdk_internal_loader_NativeLibraries__load(
    impl_obj: Ref, name: Ref, is_builtin: i32, throw_fail: i32,
) -> Res<i32> {
    let io = rt::nn_ref(impl_obj)?;
    if is_builtin != 0 {
        rt::set_i64(io, "handle", 1);
        return Ok(1);
    }
    let cp = rt::c_str_arg(name)?;
    let handle = unsafe { dlopen(cp.as_ptr(), RTLD_NOW | RTLD_GLOBAL) };
    if handle.is_null() {
        if throw_fail != 0 {
            let err = unsafe { dlerror() };
            let msg = if err.is_null() { cp.to_string_lossy().into_owned() } else { unsafe { CStr::from_ptr(err).to_string_lossy().into_owned() } };
            return rt::throw("java/lang/UnsatisfiedLinkError", msg);
        }
        return Ok(0);
    }
    register_lib_handle(handle);
    rt::set_i64(io, "handle", handle as usize as i64);
    Ok(1)
}

pub fn n_jdk_internal_loader_NativeLibraries__unload(_name: Ref, is_builtin: i32, handle: i64) -> Res<()> {
    if is_builtin == 0 && handle > 1 {
        unsafe { dlclose(handle as usize as *mut c_void) };
    }
    Ok(())
}

pub fn n_jdk_internal_loader_NativeLibrary__findEntry0(handle: i64, name: Ref) -> Res<i64> {
    let cn = rt::c_str_arg(name)?;
    if handle > 1 {
        let sym = unsafe { dlsym(handle as usize as *mut c_void, cn.as_ptr()) };
        return Ok(sym as usize as i64);
    }
    for h in loaded_lib_handles() {
        let sym = unsafe { dlsym(h as *mut c_void, cn.as_ptr()) };
        if !sym.is_null() {
            return Ok(sym as usize as i64);
        }
    }
    let sym = unsafe { dlsym(std::ptr::null_mut(), cn.as_ptr()) };
    Ok(sym as usize as i64)
}

pub fn n_jdk_internal_loader_RawNativeLibraries__load0(impl_obj: Ref, path: Ref) -> Res<i32> {
    n_jdk_internal_loader_NativeLibraries__load(impl_obj, path, 0, 0)
}

// ------------------------------------------------------------------ Foreign Linker (LibFallback over real libffi + NativeEntryPoint)
pub fn n_jdk_internal_foreign_abi_NativeEntryPoint__registerNatives() -> Res<()> { Ok(()) }
pub fn n_jdk_internal_foreign_abi_NativeEntryPoint__makeDowncallStub(
    _mt: Ref, _abi: Ref, _args: Ref, _rets: Ref, _needs_ret: i32, _rank: i32, _needs_t: i32,
) -> Res<i64> {
    Ok(0)
}
pub fn n_jdk_internal_foreign_abi_NativeEntryPoint__freeDowncallStub0(_addr: i64) -> Res<i32> { Ok(1) }

pub fn n_jdk_internal_foreign_abi_fallback_LibFallback__init() -> Res<i32> { Ok(1) }
pub fn n_jdk_internal_foreign_abi_fallback_LibFallback__sizeofCif() -> Res<i64> { Ok(32) }
pub fn n_jdk_internal_foreign_abi_fallback_LibFallback__ffi_default_abi() -> Res<i32> {
    Ok(if cfg!(target_arch = "x86_64") { 2 } else { 1 })
}
pub fn n_jdk_internal_foreign_abi_fallback_LibFallback__ffi_type_struct() -> Res<i32> { Ok(13) }
pub fn n_jdk_internal_foreign_abi_fallback_LibFallback__ffi_type_void() -> Res<i64> { Ok(&raw const ffi_type_void as usize as i64) }
pub fn n_jdk_internal_foreign_abi_fallback_LibFallback__ffi_type_uint8() -> Res<i64> { Ok(&raw const ffi_type_uint8 as usize as i64) }
pub fn n_jdk_internal_foreign_abi_fallback_LibFallback__ffi_type_sint8() -> Res<i64> { Ok(&raw const ffi_type_sint8 as usize as i64) }
pub fn n_jdk_internal_foreign_abi_fallback_LibFallback__ffi_type_uint16() -> Res<i64> { Ok(&raw const ffi_type_uint16 as usize as i64) }
pub fn n_jdk_internal_foreign_abi_fallback_LibFallback__ffi_type_sint16() -> Res<i64> { Ok(&raw const ffi_type_sint16 as usize as i64) }
pub fn n_jdk_internal_foreign_abi_fallback_LibFallback__ffi_type_sint32() -> Res<i64> { Ok(&raw const ffi_type_sint32 as usize as i64) }
pub fn n_jdk_internal_foreign_abi_fallback_LibFallback__ffi_type_sint64() -> Res<i64> { Ok(&raw const ffi_type_sint64 as usize as i64) }
pub fn n_jdk_internal_foreign_abi_fallback_LibFallback__ffi_type_float() -> Res<i64> { Ok(&raw const ffi_type_float as usize as i64) }
pub fn n_jdk_internal_foreign_abi_fallback_LibFallback__ffi_type_double() -> Res<i64> { Ok(&raw const ffi_type_double as usize as i64) }
pub fn n_jdk_internal_foreign_abi_fallback_LibFallback__ffi_type_pointer() -> Res<i64> { Ok(&raw const ffi_type_pointer as usize as i64) }
pub fn n_jdk_internal_foreign_abi_fallback_LibFallback__ffi_sizeof_short() -> Res<i32> { Ok(std::mem::size_of::<i16>() as i32) }
pub fn n_jdk_internal_foreign_abi_fallback_LibFallback__ffi_sizeof_int() -> Res<i32> { Ok(std::mem::size_of::<c_int>() as i32) }
pub fn n_jdk_internal_foreign_abi_fallback_LibFallback__ffi_sizeof_long() -> Res<i32> { Ok(std::mem::size_of::<c_long>() as i32) }
pub fn n_jdk_internal_foreign_abi_fallback_LibFallback__ffi_sizeof_wchar() -> Res<i32> { Ok(4) }
pub fn n_jdk_internal_foreign_abi_fallback_LibFallback__alignof_long_long() -> Res<i32> { Ok(std::mem::align_of::<i64>() as i32) }
pub fn n_jdk_internal_foreign_abi_fallback_LibFallback__alignof_double() -> Res<i32> { Ok(std::mem::align_of::<f64>() as i32) }

pub fn n_jdk_internal_foreign_abi_fallback_LibFallback__ffi_prep_cif(
    cif: i64, abi: i32, nargs: i32, rtype: i64, atypes: i64,
) -> Res<i32> {
    Ok(unsafe {
        ffi_prep_cif(
            cif as usize as *mut c_void, abi, nargs as c_uint,
            rtype as usize as *mut c_void, atypes as usize as *mut c_void,
        )
    })
}

pub fn n_jdk_internal_foreign_abi_fallback_LibFallback__ffi_prep_cif_var(
    cif: i64, abi: i32, nfixed: i32, ntotal: i32, rtype: i64, atypes: i64,
) -> Res<i32> {
    Ok(unsafe {
        ffi_prep_cif_var(
            cif as usize as *mut c_void, abi, nfixed as c_uint, ntotal as c_uint,
            rtype as usize as *mut c_void, atypes as usize as *mut c_void,
        )
    })
}

pub fn n_jdk_internal_foreign_abi_fallback_LibFallback__ffi_get_struct_offsets(
    abi: i32, st: i64, offsets: i64,
) -> Res<i32> {
    Ok(unsafe { ffi_get_struct_offsets(abi, st as usize as *mut c_void, offsets as usize as *mut usize) })
}

pub fn n_jdk_internal_foreign_abi_fallback_LibFallback__doDowncall(
    cif: i64, fn_ptr: i64, rvalue: i64, avalues: i64, _capt_obj: Ref, _capt_off: i64, _capt_mask: i32, _args: Ref, _n: i32,
) -> Res<()> {
    let _blk = ScopedBlock::enter();
    unsafe {
        ffi_call(
            cif as usize as *mut c_void,
            fn_ptr as usize as *const c_void,
            rvalue as usize as *mut c_void,
            avalues as usize as *mut *mut c_void,
        );
    }
    Ok(())
}

// ------------------------------------------------------------------ jdk.jfr.internal.JVM
static JFR_CHUNK_START: AtomicI64 = AtomicI64::new(0);
static JFR_RECORDING: AtomicI64 = AtomicI64::new(0);
static JFR_NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn jfr_state() -> &'static Mutex<(String, String, HashMap<String, i64>)> {
    static S: OnceLock<Mutex<(String, String, HashMap<String, i64>)>> = OnceLock::new();
    S.get_or_init(|| Mutex::new((String::new(), "/tmp".to_string(), HashMap::new())))
}

pub fn n_jdk_jfr_internal_JVM__registerNatives() -> Res<()> { Ok(()) }
pub fn n_jdk_jfr_internal_JVM__isAvailable() -> Res<i32> { Ok(0) }
pub fn n_jdk_jfr_internal_JVM__isContainerized() -> Res<i32> { Ok(0) }
pub fn n_jdk_jfr_internal_JVM__isRecording() -> Res<i32> { Ok((JFR_RECORDING.load(Ordering::Relaxed) != 0) as i32) }
pub fn n_jdk_jfr_internal_JVM__createJFR(_recreate: i32) -> Res<i32> { Ok(1) }
pub fn n_jdk_jfr_internal_JVM__destroyJFR() -> Res<i32> { JFR_RECORDING.store(0, Ordering::Relaxed); Ok(1) }
pub fn n_jdk_jfr_internal_JVM__beginRecording() -> Res<()> {
    JFR_RECORDING.store(1, Ordering::Relaxed);
    JFR_CHUNK_START.store(START_INSTANT.get_or_init(Instant::now).elapsed().as_nanos() as i64, Ordering::Relaxed);
    Ok(())
}
pub fn n_jdk_jfr_internal_JVM__endRecording() -> Res<()> { JFR_RECORDING.store(0, Ordering::Relaxed); Ok(()) }
pub fn n_jdk_jfr_internal_JVM__markChunkFinal() -> Res<()> { Ok(()) }
pub fn n_jdk_jfr_internal_JVM__shouldRotateDisk() -> Res<i32> { Ok(0) }
pub fn n_jdk_jfr_internal_JVM__counterTime() -> Res<i64> { Ok(START_INSTANT.get_or_init(Instant::now).elapsed().as_nanos() as i64) }
pub fn n_jdk_jfr_internal_JVM__nanosNow() -> Res<i64> { n_jdk_jfr_internal_JVM__counterTime() }
pub fn n_jdk_jfr_internal_JVM__getChunkStartNanos() -> Res<i64> { Ok(JFR_CHUNK_START.load(Ordering::Relaxed)) }
pub fn n_jdk_jfr_internal_JVM__getTimeConversionFactor() -> Res<f64> { Ok(1.0) }
pub fn n_jdk_jfr_internal_JVM__getPid() -> Res<Ref> { Ok(rt::new_str(unsafe { getpid() }.to_string())) }
pub fn n_jdk_jfr_internal_JVM__hostTotalMemory() -> Res<i64> { n_jdk_internal_platform_CgroupMetrics__getTotalMemorySize0() }
pub fn n_jdk_jfr_internal_JVM__hostTotalSwapMemory() -> Res<i64> { n_jdk_internal_platform_CgroupMetrics__getTotalSwapSize0() }
pub fn n_jdk_jfr_internal_JVM__getUnloadedEventClassCount() -> Res<i64> { Ok(0) }
pub fn n_jdk_jfr_internal_JVM__getAllowedToDoEventRetransforms() -> Res<i32> { Ok(0) }
pub fn n_jdk_jfr_internal_JVM__isInstrumented(_cls: Ref) -> Res<i32> { Ok(0) }
pub fn n_jdk_jfr_internal_JVM__isExcluded__90b84b(_cls: Ref) -> Res<i32> { Ok(0) }
pub fn n_jdk_jfr_internal_JVM__isExcluded__d564e2(_thr: Ref) -> Res<i32> { Ok(0) }
pub fn n_jdk_jfr_internal_JVM__exclude(_thr: Ref) -> Res<()> { Ok(()) }
pub fn n_jdk_jfr_internal_JVM__include(_thr: Ref) -> Res<()> { Ok(()) }
pub fn n_jdk_jfr_internal_JVM__emitEvent(_id: i64, _start: i64, _dur: i64) -> Res<i32> { Ok(0) }
pub fn n_jdk_jfr_internal_JVM__emitDataLoss(_bytes: i64) -> Res<()> { Ok(()) }
pub fn n_jdk_jfr_internal_JVM__emitOldObjectSamples(_cutoff: i64, _emit_all: i32, _skip_bfs: i32) -> Res<()> { Ok(()) }
pub fn n_jdk_jfr_internal_JVM__flush__7cc1a4() -> Res<()> { Ok(()) }
pub fn n_jdk_jfr_internal_JVM__getAllEventClasses() -> Res<Ref> {
    if let Some(c) = rt::find_class("java/util/Collections") {
        return rt::call_reflective(c.name, "emptyList", "()Ljava/util/List;", Ref::NULL, &[]);
    }
    Ok(Ref::NULL)
}
pub fn n_jdk_jfr_internal_JVM__getConfiguration(_cls: Ref) -> Res<Ref> { Ok(Ref::NULL) }
pub fn n_jdk_jfr_internal_JVM__setConfiguration(_cls: Ref, _cfg: Ref) -> Res<i32> { Ok(0) }
pub fn n_jdk_jfr_internal_JVM__getTypeId__567886(name: Ref) -> Res<i64> {
    let k = if name.is_null() { String::new() } else { rt::str_to_rust(name) };
    let mut g = jfr_state().lock().unwrap();
    let id = *g.2.entry(k).or_insert_with(|| JFR_NEXT_ID.fetch_add(1, Ordering::Relaxed) as i64);
    Ok(id)
}
pub fn n_jdk_jfr_internal_JVM__getTypeId__f6cbe3(c_mirror: Ref) -> Res<i64> {
    if c_mirror.is_null() { return Ok(0); }
    let nm = rt::class_of_mirror(c_mirror).name.to_string();
    let mut g = jfr_state().lock().unwrap();
    let id = *g.2.entry(nm).or_insert_with(|| JFR_NEXT_ID.fetch_add(1, Ordering::Relaxed) as i64);
    Ok(id)
}
pub fn n_jdk_jfr_internal_JVM__setOutput(out: Ref) -> Res<()> {
    let s = if out.is_null() { String::new() } else { rt::str_to_rust(out) };
    jfr_state().lock().unwrap().0 = s;
    Ok(())
}
pub fn n_jdk_jfr_internal_JVM__setDumpPath(path: Ref) -> Res<()> {
    let s = if path.is_null() { String::new() } else { rt::str_to_rust(path) };
    jfr_state().lock().unwrap().1 = s;
    Ok(())
}
pub fn n_jdk_jfr_internal_JVM__getDumpPath() -> Res<Ref> {
    Ok(rt::new_str(jfr_state().lock().unwrap().1.clone()))
}
pub fn n_jdk_jfr_internal_JVM__setRepositoryLocation(_dir: Ref) -> Res<()> { Ok(()) }
pub fn n_jdk_jfr_internal_JVM__setEnabled(_id: i64, _on: i32) -> Res<()> { Ok(()) }
pub fn n_jdk_jfr_internal_JVM__setThreshold(_id: i64, _ns: i64) -> Res<i32> { Ok(1) }
pub fn n_jdk_jfr_internal_JVM__setThrottle(_id: i64, _sz: i64, _win: i64) -> Res<i32> { Ok(1) }
pub fn n_jdk_jfr_internal_JVM__setStackTraceEnabled(_id: i64, _on: i32) -> Res<()> { Ok(()) }
pub fn n_jdk_jfr_internal_JVM__setStackDepth(_depth: i32) -> Res<()> { Ok(()) }
pub fn n_jdk_jfr_internal_JVM__setGlobalBufferCount(_cnt: i64) -> Res<()> { Ok(()) }
pub fn n_jdk_jfr_internal_JVM__setGlobalBufferSize(_sz: i64) -> Res<()> { Ok(()) }
pub fn n_jdk_jfr_internal_JVM__setThreadBufferSize(_sz: i64) -> Res<()> { Ok(()) }
pub fn n_jdk_jfr_internal_JVM__setMemorySize(_sz: i64) -> Res<()> { Ok(()) }
pub fn n_jdk_jfr_internal_JVM__setFileNotification(_delta: i64) -> Res<()> { Ok(()) }
pub fn n_jdk_jfr_internal_JVM__setMethodSamplingPeriod(_id: i64, _ms: i64) -> Res<()> { Ok(()) }
pub fn n_jdk_jfr_internal_JVM__setMethodTraceFilters(_c: Ref, _m: Ref, _a: Ref, _o: Ref) -> Res<Ref> { rt::new_array(b'J', 0) }
pub fn n_jdk_jfr_internal_JVM__setCPUPeriod(_ns: i64) -> Res<()> { Ok(()) }
pub fn n_jdk_jfr_internal_JVM__setCPURate(_rate: f64) -> Res<()> { Ok(()) }
pub fn n_jdk_jfr_internal_JVM__setMiscellaneous(_id: i64, _v: i64) -> Res<()> { Ok(()) }
pub fn n_jdk_jfr_internal_JVM__storeMetadataDescriptor(_bytes: Ref) -> Res<()> { Ok(()) }
pub fn n_jdk_jfr_internal_JVM__registerStackFilter(_cls: Ref, _meth: Ref) -> Res<i64> { Ok(JFR_NEXT_ID.fetch_add(1, Ordering::Relaxed) as i64) }
pub fn n_jdk_jfr_internal_JVM__unregisterStackFilter(_id: i64) -> Res<()> { Ok(()) }
pub fn n_jdk_jfr_internal_JVM__drainStaleMethodTracerIds() -> Res<Ref> { rt::new_array(b'J', 0) }
pub fn n_jdk_jfr_internal_JVM__retransformClasses(_cls: Ref) -> Res<()> { Ok(()) }
pub fn n_jdk_jfr_internal_JVM__subscribeLogLevel(_tag: Ref, _level: i32) -> Res<()> { Ok(()) }
pub fn n_jdk_jfr_internal_JVM__log(_tag: i32, _level: i32, _msg: Ref) -> Res<()> { Ok(()) }
pub fn n_jdk_jfr_internal_JVM__logEvent(_level: i32, _lines: Ref, _sys: i32) -> Res<()> { Ok(()) }
pub fn n_jdk_jfr_internal_JVM__uncaughtException(_t: Ref, _e: Ref) -> Res<()> { Ok(()) }
pub fn n_jdk_jfr_internal_JVM__abort(msg: Ref) -> Res<()> {
    let s = if msg.is_null() { "JFR abort".to_string() } else { rt::str_to_rust(msg) };
    rt::abort(&s)
}
