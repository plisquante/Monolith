//! Monolith runtime, first slice: the pieces numeric code needs. Java semantics are kept exactly:
//! wrapping integer arithmetic, masked shift counts, saturating float-to-int conversions, `/ by zero`.
#![allow(dead_code, non_snake_case)]

/// A thrown Java exception: a reference to a Throwable object.
#[derive(Debug)]
pub struct Thrown {
    pub obj: Ref,
}
pub type Res<T> = Result<T, Thrown>;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CRes<T> {
    pub err: Ref,
    pub val: T,
}
impl<T> CRes<T> {
    #[inline(always)]
    pub fn res(self) -> Res<T> {
        if self.err.is_null() { Ok(self.val) } else { Err(Thrown { obj: self.err }) }
    }
}
#[inline(always)]
pub fn cres<T: Default>(r: Res<T>) -> CRes<T> {
    match r {
        Ok(val) => CRes { err: Ref::NULL, val },
        Err(t) => CRes { err: t.obj, val: Default::default() },
    }
}
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct CVoid(pub Ref);
impl CVoid {
    #[inline(always)]
    pub fn res(self) -> Res<()> {
        if self.0.is_null() { Ok(()) } else { Err(Thrown { obj: self.0 }) }
    }
}
#[inline(always)]
pub fn cvoid(r: Res<()>) -> CVoid {
    match r {
        Ok(()) => CVoid(Ref::NULL),
        Err(t) => CVoid(t.obj),
    }
}

// ------------------------------------------------------------------ object model
use std::alloc::{alloc_zeroed, Layout};

/// Object header, 16 bytes: class, identity hash, GC bits (same layout as the Zig runtime).
#[repr(C)]
pub struct Obj {
    pub class: &'static Class,
    pub hash: u32,
    pub gc: u32,
}

/// A function pointer in a vtable (raw pointers are not `Sync`, statics need it).
#[repr(transparent)]
pub struct Fp(pub *const ());
unsafe impl Sync for Fp {}

#[repr(C)]
pub struct Class {
    pub name: &'static str,
    pub super_: Option<&'static Class>,
    pub size: usize,
    pub vtable: &'static [Fp],
    /// array element kind (b'Z', b'B', b'C', b'S', b'I', b'J', b'F', b'D', b'A'), 0 for non-arrays
    pub elem: u8,
    /// component class for object/multi-dim arrays (None for non-arrays or 1D primitive arrays)
    pub elem_class: Option<&'static Class>,
    /// lazily allocated 1D array Class with `self` as component
    pub array_cache: ArrCache,
    /// interface method table: (selector, function), sorted by selector; includes inherited and default methods
    pub itab: &'static [IEnt],
    /// every interface this class implements, directly or inherited (flat)
    pub ifaces: &'static [&'static Class],
    /// declared fields (instance offsets and static field addresses), for Unsafe/VarHandle/reflection
    pub fields: &'static [FInfo],
    /// instance reference field byte offsets (flattened across superclasses) for precise GC tracing
    pub ref_offsets: &'static [u32],
    /// static reference field pointers for precise GC root scanning
    pub static_refs: &'static [SPtr],
    /// declared methods (excluding <clinit>), for reflection
    pub methods: &'static [MInfo],
    /// optional class reflection metadata (annotations, constant pool, generics, nesting)
    pub refl: Option<&'static ClassRefl>,
    /// lazily created java.lang.Class object
    pub mirror: Mirror,
    /// 0 instance, 1 interface, 2 array, 3 primitive
    pub kind: u8,
    pub access: u16,
    /// class initializer (None when the class needs none)
    pub init: Option<extern "C" fn() -> CVoid>,
    /// Enum.values() array of an enum class
    pub enum_values: Option<extern "C" fn() -> CRes<Ref>>,
}
pub struct ArrCache(pub std::sync::atomic::AtomicPtr<Class>);
unsafe impl Sync for ArrCache {}
impl ArrCache {
    // a const, not a static: each use in a static initializer gets its own cell
    #[allow(clippy::declare_interior_mutable_const)]
    pub const NEW: ArrCache = ArrCache(std::sync::atomic::AtomicPtr::new(std::ptr::null_mut()));
}
pub struct SPtr(pub *mut u8);
unsafe impl Sync for SPtr {}
pub enum FLoc {
    Off(u32),
    Static(SPtr),
}
pub struct FInfo {
    pub name: &'static str,
    pub desc: &'static str,
    pub access: u16,
    pub loc: FLoc,
    pub sig: &'static str,
    pub ann: &'static [u8],
}
unsafe impl Sync for FInfo {}
#[repr(C)]
pub struct MInfo {
    pub name: &'static str,
    pub desc: &'static str,
    pub access: u16,
    pub shape: u32,
    pub fp: Fp,
    pub sig: &'static str,
    pub ann: &'static [u8],
    pub pann: &'static [u8],
    pub dflt: &'static [u8],
    pub ex: &'static [&'static str],
}
unsafe impl Sync for MInfo {}
#[repr(C)]
pub enum CpEnt {
    None,
    Int(i32),
    Float(f32),
    Long(i64),
    Double(f64),
    Str(&'static str),
}
unsafe impl Sync for CpEnt {}
#[repr(C)]
pub struct ClassRefl {
    pub sig: &'static str,
    pub ann: &'static [u8],
    pub cpool: &'static [CpEnt],
    pub is_inner: bool,
    pub outer: &'static str,
    pub inner_name: &'static str,
    pub inner_flags: u16,
    pub enc_class: &'static str,
    pub enc_name: &'static str,
    pub enc_desc: &'static str,
    pub declared: &'static [&'static str],
    pub nest_host: &'static str,
}
unsafe impl Sync for ClassRefl {}
impl Class {
    /// Finds a declared field in this class or a superclass.
    pub fn find_field(&'static self, name: &str, is_static: bool) -> Option<(&'static Class, &'static FInfo)> {
        let mut k: Option<&'static Class> = Some(self);
        while let Some(c) = k {
            for f in c.fields {
                if f.name == name && matches!(f.loc, FLoc::Static(_)) == is_static {
                    return Some((c, f));
                }
            }
            k = c.super_;
        }
        None
    }
}
unsafe impl Sync for Class {}
pub struct Mirror(pub std::sync::atomic::AtomicPtr<Obj>);
impl Mirror {
    // a const, not a static: each use in a static initializer gets its own cell
    #[allow(clippy::declare_interior_mutable_const)]
    pub const NEW: Mirror = Mirror(std::sync::atomic::AtomicPtr::new(std::ptr::null_mut()));
}
pub struct IEnt(pub u32, pub Fp);
unsafe impl Sync for IEnt {}

/// A reference: null or a pointer to an object (no GC yet in this slice: memory is never freed).
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Ref(pub *mut Obj);
impl Ref {
    pub const NULL: Ref = Ref(std::ptr::null_mut());
    #[inline(always)]
    pub fn is_null(self) -> bool {
        self.0.is_null()
    }
}
impl Default for Ref {
    fn default() -> Self {
        Ref::NULL
    }
}

/// Startup registration by the generated program: the classes and callbacks the runtime needs.
pub struct ServiceEntry {
    pub service: &'static str,
    pub create: fn() -> Res<Ref>,
}
unsafe impl Sync for ServiceEntry {}

pub struct Hooks {
    pub object_class: &'static Class,
    pub string_class: &'static Class,
    /// array classes in the order Z B C S I J F D A
    pub arrays: [&'static Class; 9],
    pub array_ifaces: &'static [&'static Class],
    /// creates an exception object of the named class with the given message (a String or null)
    pub class_class: &'static Class,
    pub all_classes: &'static [&'static Class],
    pub vh_kind: usize,
    pub vh_offset: usize,
    pub vh_bigendian: usize,
    pub thread_run: fn(Ref) -> Res<()>,
    pub throw_new: fn(&str, Ref) -> Ref,
    pub uncaught: fn(Ref) -> !,
    pub reflect_invoke: fn(u32, *const (), Ref, Ref) -> Res<Ref>,
    pub services: &'static [ServiceEntry],
    pub new_unix_exception: fn(i32) -> Res<Ref>,
    pub new_direct_byte_buffer: fn(i64, i64) -> Res<Ref>,
}
/// Runtime hooks, set once by `init_hooks` before any Java code runs. Null until then.
static HOOKS: std::sync::atomic::AtomicPtr<Hooks> = std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());
#[cfg(target_os = "linux")]
extern "C" fn segv_handler(sig: std::os::raw::c_int, info: *const usize, _ctx: *const u8) {
    unsafe extern "C" {
        fn backtrace(buffer: *mut *mut std::os::raw::c_void, size: std::os::raw::c_int) -> std::os::raw::c_int;
        fn backtrace_symbols_fd(buffer: *const *mut std::os::raw::c_void, size: std::os::raw::c_int, fd: std::os::raw::c_int);
        fn _exit(status: std::os::raw::c_int) -> !;
    }
    let addr = if !info.is_null() { unsafe { *info.add(2) } } else { 0 };
    let mut hdr = *b"\n[mono:SIGSEGV] addr=0x0000000000000000 backtrace:\n";
    let hex = b"0123456789abcdef";
    for i in 0..16 {
        hdr[23 + i] = hex[(addr >> ((15 - i) * 4)) & 0xf];
    }
    unsafe {
        let _ = libc_write(2, hdr.as_ptr() as *const std::os::raw::c_void, hdr.len());
    }
    let mut buf = [std::ptr::null_mut::<std::os::raw::c_void>(); 48];
    let n = unsafe { backtrace(buf.as_mut_ptr(), 48) };
    unsafe {
        backtrace_symbols_fd(buf.as_ptr(), n, 2);
        _exit(128 + sig);
    }
}
#[cfg(target_os = "linux")]
#[repr(C)]
struct StackT {
    ss_sp: *mut std::os::raw::c_void,
    ss_flags: std::os::raw::c_int,
    ss_size: usize,
}
#[cfg(target_os = "linux")]
#[repr(C)]
struct SigAction {
    sa_handler: usize,
    sa_mask: [u64; 16],
    sa_flags: std::os::raw::c_int,
    sa_restorer: usize,
}
#[cfg(target_os = "linux")]
unsafe extern "C" {
    #[link_name = "write"]
    fn libc_write(fd: std::os::raw::c_int, buf: *const std::os::raw::c_void, count: usize) -> isize;
    fn sigaltstack(ss: *const StackT, old_ss: *mut StackT) -> std::os::raw::c_int;
    fn sigaction(sig: std::os::raw::c_int, act: *const u8, oldact: *mut u8) -> std::os::raw::c_int;
}
pub fn init_hooks(h: Hooks) {
    // leaked on purpose: the hooks live for the whole process and are read without a lock
    HOOKS.store(Box::leak(Box::new(h)), std::sync::atomic::Ordering::Release);
    crate::prof::init();
    #[cfg(target_os = "linux")]
    unsafe {
        let alt = Box::leak(vec![0u8; 65536].into_boxed_slice());
        let ss = StackT { ss_sp: alt.as_mut_ptr() as *mut std::os::raw::c_void, ss_flags: 0, ss_size: alt.len() };
        let _ = sigaltstack(&ss, std::ptr::null_mut());
        let sa = SigAction {
            sa_handler: segv_handler as *const () as usize,
            sa_mask: [0u64; 16],
            sa_flags: 0x08000004u32 as i32, // SA_ONSTACK | SA_SIGINFO
            sa_restorer: 0,
        };
        let _ = sigaction(11, (&sa as *const SigAction).cast(), std::ptr::null_mut());
    }
}
#[inline(always)]
pub fn hooks() -> &'static Hooks {
    unsafe { HOOKS.load(std::sync::atomic::Ordering::Acquire).as_ref() }.expect("Monolith: runtime hooks not registered")
}

#[cold]
#[inline(never)]
pub fn throw<T>(class: &'static str, msg: String) -> Res<T> {
    if let Ok(v) = std::env::var("MONO_TRACE_EX") {
        if v != "err" || class != "java/lang/ClassNotFoundException" {
            eprintln!("[mono:throw] {} ({})\n{}", class, msg, std::backtrace::Backtrace::force_capture());
        }
    }
    let m = if msg.is_empty() && class == "java/lang/NullPointerException" { Ref::NULL } else { new_str(msg) };
    Err(Thrown { obj: (hooks().throw_new)(class, m) })
}

/// `athrow`: throws the object, or NullPointerException for null.
#[cold]
#[inline(never)]
pub fn throw_ref<T>(r: Ref) -> Res<T> {
    if r.is_null() {
        return throw("java/lang/NullPointerException", String::new());
    }
    if let Ok(v) = std::env::var("MONO_TRACE_EX") {
        let cls = unsafe { (*r.0).class.name };
        if v != "err" || cls != "java/lang/ClassNotFoundException" {
            eprintln!("[mono:athrow] {}\n{}", cls, std::backtrace::Backtrace::force_capture());
        }
    }
    Err(Thrown { obj: r })
}

#[inline(always)]
pub fn throw_ref_err<T>(ex: Ref) -> Res<T> {
    throw_ref(ex)
}

pub fn new_unix_exception(errno: i32) -> Res<Ref> {
    (hooks().new_unix_exception)(errno)
}

pub fn throw_unix_errno<T>(errno: i32) -> Res<T> {
    let ex = new_unix_exception(errno)?;
    Err(Thrown { obj: ex })
}

pub fn new_direct_byte_buffer(addr: i64, cap: i64) -> Res<Ref> {
    (hooks().new_direct_byte_buffer)(addr, cap)
}

pub fn set_ref(o: Ref, name: &str, v: Ref) {
    if o.is_null() { return; }
    let c = unsafe { (*o.0).class };
    if let Some((_, f)) = c.find_field(name, false) {
        if let FLoc::Off(off) = f.loc {
            unsafe { satb_store((o.0 as *mut u8).add(off as usize) as *mut Ref, v); }
        }
    }
}

pub fn get_ref(o: Ref, name: &str) -> Ref {
    if o.is_null() { return Ref::NULL; }
    let c = unsafe { (*o.0).class };
    if let Some((_, f)) = c.find_field(name, false) {
        if let FLoc::Off(off) = f.loc {
            return unsafe { *((o.0 as *const u8).add(off as usize) as *const Ref) };
        }
    }
    Ref::NULL
}

pub fn set_i32(o: Ref, name: &str, v: i32) {
    if o.is_null() { return; }
    let c = unsafe { (*o.0).class };
    if let Some((_, f)) = c.find_field(name, false) {
        if let FLoc::Off(off) = f.loc {
            unsafe { *((o.0 as *mut u8).add(off as usize) as *mut i32) = v; }
        }
    }
}

pub fn get_i32(o: Ref, name: &str) -> i32 {
    if o.is_null() { return 0; }
    let c = unsafe { (*o.0).class };
    if let Some((_, f)) = c.find_field(name, false) {
        if let FLoc::Off(off) = f.loc {
            return unsafe { *((o.0 as *const u8).add(off as usize) as *const i32) };
        }
    }
    0
}

pub fn set_i16(o: Ref, name: &str, v: i16) {
    if o.is_null() { return; }
    let c = unsafe { (*o.0).class };
    if let Some((_, f)) = c.find_field(name, false) {
        if let FLoc::Off(off) = f.loc {
            unsafe { *((o.0 as *mut u8).add(off as usize) as *mut i16) = v; }
        }
    }
}

pub fn box_i32(v: i32) -> Res<Ref> {
    if let Some(c) = find_class("java/lang/Integer") {
        let o = alloc(c);
        set_i32(o, "value", v);
        return Ok(o);
    }
    throw("java/lang/InternalError", "java/lang/Integer missing".to_string())
}

pub fn call_reflective(class_name: &str, mname: &str, desc: &str, recv: Ref, args: &[Ref]) -> Res<Ref> {
    let c = find_class(class_name)
        .ok_or_else(|| Thrown { obj: (hooks().throw_new)("java/lang/InternalError", new_str(format!("missing class {}", class_name))) })?;
    if let Some(i) = c.init {
        i().res()?;
    }
    let m = c.methods.iter().find(|m| m.name == mname && m.desc == desc)
        .ok_or_else(|| Thrown { obj: (hooks().throw_new)("java/lang/InternalError", new_str(format!("missing method {}.{}{}", class_name, mname, desc))) })?;
    if m.shape == 0 || m.fp.0.is_null() {
        return throw("java/lang/InternalError", format!("method not compiled {}.{}{}", class_name, mname, desc));
    }
    let arr = new_obj_array(hooks().object_class, args.len() as i32)?;
    for (i, &a) in args.iter().enumerate() {
        aastore_ref(arr, i as i32, a)?;
    }
    (hooks().reflect_invoke)(m.shape, m.fp.0, recv, arr)
}

pub fn set_i64(o: Ref, name: &str, v: i64) {
    if o.is_null() { return; }
    let c = unsafe { (*o.0).class };
    if let Some((_, f)) = c.find_field(name, false) {
        if let FLoc::Off(off) = f.loc {
            unsafe { *((o.0 as *mut u8).add(off as usize) as *mut i64) = v; }
        }
    }
}

pub fn get_i64(o: Ref, name: &str) -> i64 {
    if o.is_null() { return 0; }
    let c = unsafe { (*o.0).class };
    if let Some((_, f)) = c.find_field(name, false) {
        if let FLoc::Off(off) = f.loc {
            return unsafe { *((o.0 as *const u8).add(off as usize) as *const i64) };
        }
    }
    0
}

pub fn set_bool(o: Ref, name: &str, v: bool) {
    if o.is_null() { return; }
    let c = unsafe { (*o.0).class };
    if let Some((_, f)) = c.find_field(name, false) {
        if let FLoc::Off(off) = f.loc {
            unsafe { *((o.0 as *mut u8).add(off as usize) as *mut i8) = if v { 1 } else { 0 }; }
        }
    }
}

pub fn get_bool(o: Ref, name: &str) -> bool {
    if o.is_null() { return false; }
    let c = unsafe { (*o.0).class };
    if let Some((_, f)) = c.find_field(name, false) {
        if let FLoc::Off(off) = f.loc {
            return unsafe { *((o.0 as *const u8).add(off as usize) as *const i8) != 0 };
        }
    }
    false
}

pub fn fd_of(fd_obj: Ref) -> Res<i32> {
    let o = nn_ref(fd_obj)?;
    Ok(get_i32(o, "fd"))
}

pub fn set_fd(fd_obj: Ref, fd: i32) -> Res<()> {
    let o = nn_ref(fd_obj)?;
    set_i32(o, "fd", fd);
    Ok(())
}

/// # Safety
/// `arr` must be a live byte array. The returned slice must not outlive the array and must not alias a live `&mut`.
pub unsafe fn byte_slice<'a>(arr: Ref) -> Res<&'a [u8]> {
    let a = nn(arr)? as *const Arr;
    let len = (*a).len as usize;
    Ok(std::slice::from_raw_parts((a as *const u8).add(24), len))
}

/// # Safety
/// `arr` must be a live byte array. The caller must hold exclusive access to its bytes for the slice's lifetime.
pub unsafe fn byte_slice_mut<'a>(arr: Ref) -> Res<&'a mut [u8]> {
    let a = nn(arr)? as *mut Arr;
    let len = (*a).len as usize;
    Ok(std::slice::from_raw_parts_mut((a as *mut u8).add(24), len))
}

pub fn c_str_arg(s: Ref) -> Res<std::ffi::CString> {
    let rust_s = str_to_rust(nn_ref(s)?);
    std::ffi::CString::new(rust_s)
        .map_err(|_| Thrown { obj: (hooks().throw_new)("java/lang/IllegalArgumentException", new_str("Nul byte in path".to_string())) })
}

pub use crate::gc::{recycle as recycle_scope, safepoint as gc_safepoint, ArenaGuard, ThreadGuard};

#[inline(always)]
pub fn gc_thread_guard() -> ThreadGuard {
    ThreadGuard::enter()
}

pub fn alloc(class: &'static Class) -> Ref {
    Ref(crate::gc::alloc_obj(class, class.size.max(16)))
}

/// An object kept in the frame of the function that creates it (the escape analysis proved it dies with that call or loop
/// iteration). `p` is the function's slot for this allocation site; it is cleared and given the class header on every
/// execution of the `new`. The collector ignores it (not a heap address) and finds what it references by scanning stacks.
/// # Safety
/// `p` must be valid and aligned for `class.size.max(16)` bytes of writes, and must stay alive for the object's lifetime.
#[inline(always)]
pub unsafe fn stack_obj(p: *mut u8, class: &'static Class) -> Ref {
    unsafe {
        std::ptr::write_bytes(p, 0, class.size.max(16));
        let o = p as *mut Obj;
        (*o).class = class;
        (*o).gc = crate::gc::GC_ARENA;
        Ref(o)
    }
}

/// `new` of a class whose instances are recycled at the end of their scope (iterators of for-each loops).
pub fn alloc_recyclable(class: &'static Class, slot: usize) -> Ref {
    crate::gc::alloc_recyclable(class, slot)
}
pub fn alloc_arena(class: &'static Class) -> Ref {
    crate::gc::alloc_arena(class, class.size.max(16))
}

#[inline(always)]
pub fn nn(r: Ref) -> Res<*mut Obj> {
    if r.is_null() {
        return throw("java/lang/NullPointerException", String::new());
    }
    Ok(r.0)
}

pub fn instance_of(r: Ref, target: &'static Class) -> bool {
    if r.is_null() {
        return false;
    }
    is_subclass(unsafe { (*r.0).class }, target)
}

/// invokeinterface: the implementation of selector `sel` for the receiver's class.
pub fn itab_find(c: &'static Class, sel: u32) -> *const () {
    let t = c.itab;
    match t.binary_search_by_key(&sel, |e| e.0) {
        Ok(i) => t[i].1 .0,
        Err(_) => {
            eprintln!("Monolith: AbstractMethodError: {} has no method for selector {}", c.name, sel);
            std::process::exit(70)
        }
    }
}

pub fn checkcast(r: Ref, target: &'static Class) -> Res<Ref> {
    if r.is_null() || instance_of(r, target) {
        return Ok(r);
    }
    let from = unsafe { (*r.0).class.name };
    throw("java/lang/ClassCastException", format!("class {} cannot be cast to class {}", from.replace('/', "."), target.name.replace('/', ".")))
}

pub fn unsupported() -> ! {
    eprintln!("Monolith: call of an unsupported (JDK) method");
    std::process::exit(70);
}

// ------------------------------------------------------------------ strings
/// Layout of java.lang.String in the class library overlay (the generator checks the field order).
#[repr(C)]
pub struct StrObj {
    pub hdr: Obj,
    pub value: Ref,
    pub hash: i32,
    pub hash_is_zero: i8,
    pub coder: i8,
}
/// A new String from UTF-16 code units.
pub fn mk_string(chars: &[u16]) -> Ref {
    let arr = new_array(b'C', chars.len() as i32).expect("Monolith: out of memory");
    unsafe {
        let d = (arr.0 as *mut u8).add(24) as *mut u16;
        std::ptr::copy_nonoverlapping(chars.as_ptr(), d, chars.len());
    }
    let r = alloc(hooks().string_class);
    unsafe {
        let s = r.0 as *mut StrObj;
        (*s).value = arr;
        (*s).coder = 1;
    }
    r
}
pub fn new_str(s: String) -> Ref {
    let u: Vec<u16> = s.encode_utf16().collect();
    mk_string(&u)
}
/// Interned literal: created once per use site (thread-safe CAS + GC root registration).
/// Canonical instance of a string value: `String.intern()` and string literals share this table, so
/// `"x" == name.intern()` holds (the JDK compares reflected member names with `==`, e.g. AnnotationInvocationHandler).
pub fn intern_string(s: Ref) -> Ref {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static TABLE: OnceLock<Mutex<HashMap<Vec<u16>, usize>>> = OnceLock::new();
    let key = str_units(s);
    let mut t = TABLE.get_or_init(|| Mutex::new(HashMap::new())).lock().unwrap();
    let ptr = *t.entry(key).or_insert_with(|| {
        crate::gc::pin_ref(s);
        s.0 as usize
    });
    Ref(ptr as *mut Obj)
}
/// # Safety
/// `slot` must be valid for reads and writes of a `Ref` and must not be accessed concurrently without synchronization.
pub unsafe fn lit(slot: *mut Ref, data: &'static [u16]) -> Ref {
    use std::sync::atomic::{AtomicPtr, Ordering};
    let ap = &*(slot as *const AtomicPtr<Obj>);
    let p = ap.load(Ordering::Acquire);
    if !p.is_null() {
        return Ref(p);
    }
    let r = intern_string(mk_string(data));
    match ap.compare_exchange(std::ptr::null_mut(), r.0, Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) => {
            crate::gc::register_static_slot(slot);
            r
        }
        Err(old) => Ref(old),
    }
}
pub fn str_to_rust(r: Ref) -> String {
    if r.is_null() {
        return "null".to_string();
    }
    unsafe {
        let v = (*(r.0 as *mut StrObj)).value;
        let n = (*(v.0 as *mut Arr)).len as usize;
        let d = (v.0 as *mut u8).add(24) as *const u16;
        String::from_utf16_lossy(std::slice::from_raw_parts(d, n))
    }
}

// ------------------------------------------------------------------ arrays
#[repr(C)]
pub struct Arr {
    pub hdr: Obj,
    pub len: i32,
    pub _pad: i32,
}
fn esize(e: u8) -> usize {
    match e {
        b'Z' | b'B' => 1,
        b'C' | b'S' => 2,
        b'I' | b'F' => 4,
        _ => 8,
    }
}
fn arr_class(e: u8) -> &'static Class {
    let a = &hooks().arrays;
    match e {
        b'Z' => a[0],
        b'B' => a[1],
        b'C' => a[2],
        b'S' => a[3],
        b'I' => a[4],
        b'J' => a[5],
        b'F' => a[6],
        b'D' => a[7],
        _ => a[8],
    }
}
pub fn array_class_of(elem_c: &'static Class) -> &'static Class {
    use std::sync::atomic::Ordering;
    let h = hooks();
    if elem_c.kind == 3 {
        return match elem_c.name {
            "boolean" => h.arrays[0], "byte" => h.arrays[1], "char" => h.arrays[2], "short" => h.arrays[3],
            "int" => h.arrays[4], "long" => h.arrays[5], "float" => h.arrays[6], "double" => h.arrays[7],
            _ => h.arrays[8],
        };
    }
    if std::ptr::eq(elem_c, h.object_class) {
        return h.arrays[8];
    }
    let p = elem_c.array_cache.0.load(Ordering::Acquire);
    if !p.is_null() {
        return unsafe { &*p };
    }
    let name: &'static str = Box::leak(
        if elem_c.kind == 2 { format!("[{}", elem_c.name) } else { format!("[L{};", elem_c.name) }.into_boxed_str(),
    );
    let boxed = Box::new(Class {
        name,
        super_: Some(h.object_class),
        size: 24,
        vtable: h.object_class.vtable,
        elem: b'A',
        elem_class: Some(elem_c),
        array_cache: ArrCache::NEW,
        itab: &[],
        ifaces: h.array_ifaces,
        fields: &[],
        ref_offsets: &[],
        static_refs: &[],
        methods: &[],
        refl: None,
        mirror: Mirror::NEW,
        kind: 2,
        access: 0x411,
        init: None,
        enum_values: None,
    });
    let raw = Box::into_raw(boxed);
    match elem_c.array_cache.0.compare_exchange(std::ptr::null_mut(), raw, Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) => {
            let ac = unsafe { &*raw };
            crate::gc::register_array_class(ac);
            ac
        }
        Err(old) => {
            unsafe {
                let b = Box::from_raw(raw);
                let _ = Box::from_raw(b.name as *const str as *mut str);
            }
            unsafe { &*old }
        }
    }
}
pub fn new_array_with_class(ac: &'static Class, e: u8, len: i32) -> Res<Ref> {
    if len < 0 {
        return throw("java/lang/NegativeArraySizeException", len.to_string());
    }
    let size = 24 + esize(e) * len as usize;
    let p = crate::gc::alloc_obj(ac, size) as *mut Arr;
    unsafe { (*p).len = len };
    Ok(Ref(p as *mut Obj))
}
pub fn new_array(e: u8, len: i32) -> Res<Ref> {
    new_array_with_class(arr_class(e), e, len)
}
pub fn new_obj_array(elem_c: &'static Class, len: i32) -> Res<Ref> {
    new_array_with_class(array_class_of(elem_c), b'A', len)
}
pub fn new_array_arena(e: u8, len: i32) -> Res<Ref> {
    if len < 0 {
        return throw("java/lang/NegativeArraySizeException", len.to_string());
    }
    let size = 24 + esize(e) * len as usize;
    let r = crate::gc::alloc_arena(arr_class(e), size);
    unsafe { (*(r.0 as *mut Arr)).len = len };
    Ok(r)
}
pub fn new_obj_array_arena(elem_c: &'static Class, len: i32) -> Res<Ref> {
    if len < 0 {
        return throw("java/lang/NegativeArraySizeException", len.to_string());
    }
    let size = 24 + 8 * len as usize;
    let r = crate::gc::alloc_arena(array_class_of(elem_c), size);
    unsafe { (*(r.0 as *mut Arr)).len = len };
    Ok(r)
}
/// `multianewarray`: `depth` dimensions of `leaf` (plus optional object component class `elem_c`), `dims` lengths given.
pub fn new_multi(elem_c: Option<&'static Class>, leaf: u8, depth: usize, dims: &[i32]) -> Res<Ref> {
    for d in dims {
        if *d < 0 {
            return throw("java/lang/NegativeArraySizeException", d.to_string());
        }
    }
    fn comp_class(elem_c: Option<&'static Class>, leaf: u8, rem_depth: usize) -> &'static Class {
        let mut c = match elem_c {
            Some(ec) => ec,
            None => match leaf {
                b'Z' => &CLS_P_BOOLEAN, b'B' => &CLS_P_BYTE, b'C' => &CLS_P_CHAR, b'S' => &CLS_P_SHORT,
                b'I' => &CLS_P_INT, b'J' => &CLS_P_LONG, b'F' => &CLS_P_FLOAT, b'D' => &CLS_P_DOUBLE,
                _ => hooks().object_class,
            },
        };
        for _ in 0..rem_depth {
            c = array_class_of(c);
        }
        c
    }
    fn go(elem_c: Option<&'static Class>, leaf: u8, depth: usize, dims: &[i32]) -> Res<Ref> {
        if depth == 1 || dims.len() == 1 {
            if depth == 1 {
                return match elem_c {
                    Some(ec) => new_obj_array(ec, dims[0]),
                    None => new_array(leaf, dims[0]),
                };
            }
            return new_obj_array(comp_class(elem_c, leaf, depth - 1), dims[0]);
        }
        let outer = new_obj_array(comp_class(elem_c, leaf, depth - 1), dims[0])?;
        for i in 0..dims[0] as usize {
            let inner = go(elem_c, leaf, depth - 1, &dims[1..])?;
            unsafe { satb_store((data(outer) as *mut Ref).add(i), inner) };
        }
        Ok(outer)
    }
    go(elem_c, leaf, depth, dims)
}
#[inline(always)]
fn data(a: Ref) -> *mut u8 {
    unsafe { (a.0 as *mut u8).add(24) }
}
#[inline(always)]
pub fn alen(a: Ref) -> Res<i32> {
    let p = nn(a)?;
    Ok(unsafe { (*(p as *mut Arr)).len })
}
#[inline(always)]
fn checked(a: Ref, i: i32) -> Res<*mut u8> {
    let p = nn(a)?;
    let len = unsafe { (*(p as *mut Arr)).len };
    if i < 0 || i >= len {
        return throw("java/lang/ArrayIndexOutOfBoundsException", format!("Index {} out of bounds for length {}", i, len));
    }
    Ok(data(a))
}
#[inline(always)]
pub fn aload<T: Copy>(a: Ref, i: i32) -> Res<T> {
    let d = checked(a, i)?;
    Ok(unsafe { *(d as *const T).add(i as usize) })
}
#[inline(always)]
pub fn astore<T: Copy>(a: Ref, i: i32, v: T) -> Res<()> {
    let d = checked(a, i)?;
    unsafe { *(d as *mut T).add(i as usize) = v };
    Ok(())
}
/// Referans yazma bariyeri; üretilen kod da bunu kullanır (rustgen: putfield/putstatic).
/// # Safety
/// Same contract as `gc::satb_store`: `slot` must be a valid, aligned `Ref` location.
#[inline(always)]
pub unsafe fn satb_store(slot: *mut Ref, v: Ref) {
    crate::gc::satb_store(slot, v)
}

pub fn aastore_ref(a: Ref, i: i32, v: Ref) -> Res<()> {
    let d = checked(a, i)?;
    if !v.is_null() {
        let ac = unsafe { (*(a.0)).class };
        if let Some(ec) = ac.elem_class {
            let vc = unsafe { (*v.0).class };
            if !is_subclass(vc, ec) {
                return throw("java/lang/ArrayStoreException", vc.name.replace('/', "."));
            }
        }
    }
    unsafe { crate::gc::satb_store((d as *mut Ref).add(i as usize), v) };
    Ok(())
}

#[inline(always)]
pub fn idiv(a: i32, b: i32) -> Res<i32> {
    if b == 0 {
        return throw("java/lang/ArithmeticException", "/ by zero".to_string());
    }
    Ok(a.wrapping_div(b))
}
#[inline(always)]
pub fn irem(a: i32, b: i32) -> Res<i32> {
    if b == 0 {
        return throw("java/lang/ArithmeticException", "/ by zero".to_string());
    }
    Ok(a.wrapping_rem(b))
}
#[inline(always)]
pub fn ldiv(a: i64, b: i64) -> Res<i64> {
    if b == 0 {
        return throw("java/lang/ArithmeticException", "/ by zero".to_string());
    }
    Ok(a.wrapping_div(b))
}
#[inline(always)]
pub fn lrem(a: i64, b: i64) -> Res<i64> {
    if b == 0 {
        return throw("java/lang/ArithmeticException", "/ by zero".to_string());
    }
    Ok(a.wrapping_rem(b))
}

// shifts: Java masks the count to the operand width, wrapping_sh* does the same
#[inline(always)]
pub fn ishl(a: i32, b: i32) -> i32 {
    a.wrapping_shl(b as u32)
}
#[inline(always)]
pub fn ishr(a: i32, b: i32) -> i32 {
    a.wrapping_shr(b as u32)
}
#[inline(always)]
pub fn iushr(a: i32, b: i32) -> i32 {
    (a as u32).wrapping_shr(b as u32) as i32
}
#[inline(always)]
pub fn lshl(a: i64, b: i32) -> i64 {
    a.wrapping_shl(b as u32)
}
#[inline(always)]
pub fn lshr(a: i64, b: i32) -> i64 {
    a.wrapping_shr(b as u32)
}
#[inline(always)]
pub fn lushr(a: i64, b: i32) -> i64 {
    (a as u64).wrapping_shr(b as u32) as i64
}

// comparisons (NaN handling per the JVM spec: fcmpl/dcmpl give -1, fcmpg/dcmpg give 1)
#[inline(always)]
pub fn lcmp(a: i64, b: i64) -> i32 {
    (a > b) as i32 - (a < b) as i32
}
#[inline(always)]
pub fn fcmpl(a: f32, b: f32) -> i32 {
    if a.is_nan() || b.is_nan() { -1 } else { (a > b) as i32 - (a < b) as i32 }
}
#[inline(always)]
pub fn fcmpg(a: f32, b: f32) -> i32 {
    if a.is_nan() || b.is_nan() { 1 } else { (a > b) as i32 - (a < b) as i32 }
}
#[inline(always)]
pub fn dcmpl(a: f64, b: f64) -> i32 {
    if a.is_nan() || b.is_nan() { -1 } else { (a > b) as i32 - (a < b) as i32 }
}
#[inline(always)]
pub fn dcmpg(a: f64, b: f64) -> i32 {
    if a.is_nan() || b.is_nan() { 1 } else { (a > b) as i32 - (a < b) as i32 }
}

// conversions: Rust's `as` from float saturates and maps NaN to 0, exactly the Java rule
#[inline(always)]
pub fn f2i(x: f32) -> i32 { x as i32 }
#[inline(always)]
pub fn f2l(x: f32) -> i64 { x as i64 }
#[inline(always)]
pub fn d2i(x: f64) -> i32 { x as i32 }
#[inline(always)]
pub fn d2l(x: f64) -> i64 { x as i64 }
#[inline(always)]
pub fn i2b(x: i32) -> i32 { (x as i8) as i32 }
#[inline(always)]
pub fn i2c(x: i32) -> i32 { (x as u16) as i32 }
#[inline(always)]
pub fn i2s(x: i32) -> i32 { (x as i16) as i32 }

/// `bastore` serves byte[] and boolean[]: boolean elements keep only the low bit.
pub fn bastore(a: Ref, i: i32, v: i32) -> Res<()> {
    let d = checked(a, i)?;
    let z = unsafe { (*a.0).class.elem } == b'Z';
    unsafe { *(d as *mut i8).add(i as usize) = if z { (v & 1) as i8 } else { v as i8 } };
    Ok(())
}


// ------------------------------------------------------------------ misc runtime services
pub fn abort(msg: &str) -> ! {
    eprintln!("Monolith: {}", msg);
    std::process::exit(70)
}
pub fn missing_native<T>(what: &str) -> Res<T> {
    abort(&format!("native method not implemented: {}", what))
}
pub fn unsupported_code<T>(what: &str) -> Res<T> {
    abort(&format!("code the Rust back end could not translate was reached: {}", what))
}
pub extern "C" fn unreachable_slot() -> ! {
    abort("call of a method the reachability analysis did not keep")
}

/// java.lang.Class instance of a runtime class (first field of the overlay Class is the long `monoClass`).
#[repr(C)]
pub struct ClassObj {
    pub hdr: Obj,
    pub mono_class: i64,
}
pub fn class_mirror(c: &'static Class) -> Res<Ref> {
    use std::sync::atomic::Ordering;
    let p = c.mirror.0.load(Ordering::Acquire);
    if !p.is_null() {
        return Ok(Ref(p));
    }
    let r = alloc(hooks().class_class);
    unsafe { (*(r.0 as *mut ClassObj)).mono_class = c as *const Class as i64 };
    match c.mirror.0.compare_exchange(std::ptr::null_mut(), r.0, Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) => Ok(r),
        Err(old) => Ok(Ref(old)),
    }
}
pub fn class_of_mirror(m: Ref) -> &'static Class {
    unsafe { &*((*(m.0 as *mut ClassObj)).mono_class as *const Class) }
}

pub fn array_clone(a: Ref) -> Res<Ref> {
    let p = nn(a)? as *mut Arr;
    let c = unsafe { (*p).hdr.class };
    if c.elem == 0 {
        return throw("java/lang/InternalError", "clone of a non-array".to_string());
    }
    let n = unsafe { (*p).len };
    let r = new_array_with_class(c, c.elem, n)?;
    let bytes = esize(c.elem) * n as usize;
    unsafe { std::ptr::copy_nonoverlapping((p as *const u8).add(24), (r.0 as *mut u8).add(24), bytes) };
    Ok(r)
}

/// Monitors live in sync.rs.
pub fn monitorenter(r: Ref) -> Res<()> {
    crate::sync::enter(r)
}
pub fn monitorexit(r: Ref) -> Res<()> {
    crate::sync::exit(r)
}

pub fn set_main_thread(t: Ref) {
    crate::sync::set_current(t);
    crate::gc::set_thread_obj(t);
}

pub fn is_subclass(sub: &'static Class, sup: &'static Class) -> bool {
    if std::ptr::eq(sub, sup) {
        return true;
    }
    if sub.kind == 2 {
        if sup.kind == 2 {
            if sub.elem == b'A' && sup.elem == b'A' {
                return match (sub.elem_class, sup.elem_class) {
                    (Some(se), Some(de)) => is_subclass(se, de),
                    _ => true,
                };
            }
            return sub.elem == sup.elem;
        }
        return std::ptr::eq(sup, hooks().object_class) || hooks().array_ifaces.iter().any(|i| std::ptr::eq(*i, sup));
    }
    let mut c: Option<&'static Class> = Some(sub);
    while let Some(k) = c {
        if std::ptr::eq(k, sup) || k.ifaces.iter().any(|i| std::ptr::eq(*i, sup)) {
            return true;
        }
        c = k.super_;
    }
    false
}

macro_rules! prim_class {
    ($n:ident, $name:expr) => {
        pub static $n: Class = Class { name: $name, super_: None, size: 0, vtable: &[], elem: 0, elem_class: None, array_cache: ArrCache::NEW, itab: &[], ifaces: &[], fields: &[], ref_offsets: &[], static_refs: &[], methods: &[], refl: None, mirror: Mirror::NEW, kind: 3, access: 0x411, init: None, enum_values: None };
    };
}
prim_class!(CLS_P_BOOLEAN, "boolean");
prim_class!(CLS_P_BYTE, "byte");
prim_class!(CLS_P_CHAR, "char");
prim_class!(CLS_P_SHORT, "short");
prim_class!(CLS_P_INT, "int");
prim_class!(CLS_P_LONG, "long");
prim_class!(CLS_P_FLOAT, "float");
prim_class!(CLS_P_DOUBLE, "double");
prim_class!(CLS_P_VOID, "void");
pub fn prim_class(name: &str) -> Option<&'static Class> {
    Some(match name {
        "boolean" => &CLS_P_BOOLEAN, "byte" => &CLS_P_BYTE, "char" => &CLS_P_CHAR, "short" => &CLS_P_SHORT,
        "int" => &CLS_P_INT, "long" => &CLS_P_LONG, "float" => &CLS_P_FLOAT, "double" => &CLS_P_DOUBLE,
        "void" => &CLS_P_VOID, _ => return None,
    })
}
pub fn class_for_desc(desc: &str) -> Option<&'static Class> {
    let b = desc.as_bytes();
    if b.is_empty() { return None; }
    match b[0] {
        b'[' => class_for_desc(&desc[1..]).map(array_class_of),
        b'L' => find_class(&desc[1..desc.len() - 1]),
        b'Z' => Some(&CLS_P_BOOLEAN), b'B' => Some(&CLS_P_BYTE), b'C' => Some(&CLS_P_CHAR), b'S' => Some(&CLS_P_SHORT),
        b'I' => Some(&CLS_P_INT), b'J' => Some(&CLS_P_LONG), b'F' => Some(&CLS_P_FLOAT), b'D' => Some(&CLS_P_DOUBLE),
        b'V' => Some(&CLS_P_VOID),
        _ => None,
    }
}
pub fn elem_size(e: u8) -> usize { esize(e) }
/// Mirror object -> pointer check (`Class` objects must not be null).
pub fn nn_ref(r: Ref) -> Res<Ref> {
    nn(r)?;
    Ok(r)
}

/// UTF-16 code units of a String object.
pub fn str_units(r: Ref) -> Vec<u16> {
    unsafe {
        let v = (*(r.0 as *mut StrObj)).value;
        let n = (*(v.0 as *mut Arr)).len as usize;
        std::slice::from_raw_parts((v.0 as *mut u8).add(24) as *const u16, n).to_vec()
    }
}

// ------------------------------------------------------------------ class initialization (JLS 12.4.2)
use std::sync::atomic::{AtomicU64, Ordering};
const ST_DONE: u64 = 2;
const ST_ERROR: u64 = 3;
fn init_lock() -> &'static (std::sync::Mutex<()>, std::sync::Condvar) {
    static L: std::sync::OnceLock<(std::sync::Mutex<()>, std::sync::Condvar)> = std::sync::OnceLock::new();
    L.get_or_init(|| (std::sync::Mutex::new(()), std::sync::Condvar::new()))
}
/// Returns Ok(true) when the caller must run the initializer; Ok(false) when it is done or being run by this thread.
pub fn init_enter(st: &AtomicU64, name: &str) -> Res<bool> {
    let v = st.load(Ordering::Acquire);
    if v == ST_DONE {
        return Ok(false);
    }
    let me = crate::sync::tid() * 4 + 1;
    let _blk = crate::gc::ScopedBlock::enter();
    let (m, cv) = init_lock();
    let trace = std::env::var_os("MONO_TRACE_INIT").is_some();
    let mut g = m.lock().unwrap();
    loop {
        match st.load(Ordering::Acquire) {
            0 => {
                st.store(me, Ordering::Release);
                return Ok(true);
            }
            ST_DONE => return Ok(false),
            ST_ERROR => {
                drop(g);
                return throw("java/lang/NoClassDefFoundError", format!("Could not initialize class {}", name.replace('/', ".")));
            }
            x if x == me => return Ok(false),
            _ => {
                if trace {
                    eprintln!("[init_wait] me={} waiting on {} (state={})", me, name, st.load(Ordering::Relaxed));
                }
                g = cv.wait(g).unwrap();
            }
        }
    }
}
pub fn init_leave(st: &AtomicU64, ok: bool) {
    if !ok && std::env::var_os("MONO_TRACE_EX").is_some() {
        eprintln!("[mono:init_fail]\n{}", std::backtrace::Backtrace::force_capture());
    }
    let (m, cv) = init_lock();
    let _g = m.lock().unwrap();
    st.store(if ok { ST_DONE } else { ST_ERROR }, Ordering::Release);
    cv.notify_all();
}

// ------------------------------------------------------------------ caller class, program arguments, system properties
thread_local! { static CALLER: std::cell::Cell<Option<&'static Class>> = const { std::cell::Cell::new(None) }; }
pub fn set_caller(c: &'static Class) {
    CALLER.with(|x| x.set(Some(c)));
}
pub fn caller() -> Option<&'static Class> {
    CALLER.with(|x| x.get())
}
pub fn find_class(dotted: &str) -> Option<&'static Class> {
    let n = dotted.replace('.', "/");
    if n.starts_with('[') {
        return class_for_desc(&n);
    }
    let all = hooks().all_classes;
    all.binary_search_by_key(&n.as_str(), |c| c.name).ok().map(|i| all[i])
}
/// Command line of the program. A shared library host (Android) installs it with `set_host_args`;
/// an executable reads the process arguments.
static HOST_ARGS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
pub fn set_host_args(args: Vec<String>) {
    let _ = HOST_ARGS.set(args);
}
fn cmdline() -> Vec<String> {
    match HOST_ARGS.get() {
        Some(a) => a.clone(),
        None => std::env::args().skip(1).collect(),
    }
}
/// `-Dkey=value` arguments are system properties; the others are program arguments.
pub fn user_props() -> Vec<String> {
    let mut v: Vec<String> = cmdline().into_iter().filter(|a| a.starts_with("-D")).map(|a| a[2..].to_string()).collect();
    // Netty's native transports (epoll/kqueue) are JNI code emulated in Rust (174 natives). Without them Netty uses its
    // NIO transport, which is plain Java and needs no native code. A launcher or user can still set the value.
    if !v.iter().any(|p| p.starts_with("io.netty.transport.noNative=")) {
        v.push("io.netty.transport.noNative=true".to_string());
    }
    v
}
pub fn program_args() -> Res<Ref> {
    let args: Vec<String> = cmdline().into_iter().filter(|a| !a.starts_with("-D")).collect();
    let arr = new_obj_array(hooks().string_class, args.len() as i32)?;
    for (i, a) in args.iter().enumerate() {
        aastore_ref(arr, i as i32, new_str(a.clone()))?;
    }
    Ok(arr)
}
pub fn string_array(items: &[String]) -> Res<Ref> {
    let arr = new_obj_array(hooks().string_class, items.len() as i32)?;
    for (i, a) in items.iter().enumerate() {
        aastore_ref(arr, i as i32, new_str(a.clone()))?;
    }
    Ok(arr)
}
