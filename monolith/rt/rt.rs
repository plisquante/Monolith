//! Monolith çalışma zamanı, ilk parça: sayısal kodun ihtiyaç duyduğu şeyler. Java'nın anlamı birebir korunur:
//! taşmalı tamsayı aritmetiği, maskelenmiş kaydırma sayıları, doyuran float-tamsayı dönüşümleri ve `sıfıra bölme` hatası.
//! Acemi notu: Java'da tamsayı taşması hata vermez, sarar (wrap). Rust'ta bu davranış `wrapping_*` fonksiyonlarıyla açıkça yazılır.
#![allow(dead_code, non_snake_case)]

/// Fırlatılmış bir Java istisnası: bir Throwable nesnesine referans.
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

// ------------------------------------------------------------------ nesne modeli
use std::alloc::{alloc_zeroed, Layout};

/// Nesne başlığı, 16 bayt: sınıf, kimlik karması ve GC bitleri. Düzeni Zig çalışma zamanıyla aynıdır.
#[repr(C)]
pub struct Obj {
    pub class: &'static Class,
    pub hash: u32,
    pub gc: u32,
}

/// Bir vtable içindeki fonksiyon işaretçisi.
/// Ham işaretçiler `Sync` değildir; statik değişkenlerde kullanılabilmesi için bu sarmalayıcı gerekir.
#[repr(transparent)]
pub struct Fp(pub *const ());
unsafe impl Sync for Fp {}

#[repr(C)]
pub struct Class {
    pub name: &'static str,
    pub super_: Option<&'static Class>,
    pub size: usize,
    pub vtable: &'static [Fp],
    /// dizi eleman türü (b'Z', b'B', b'C', b'S', b'I', b'J', b'F', b'D', b'A); dizi değilse 0
    pub elem: u8,
    /// nesne ve çok boyutlu diziler için bileşen sınıfı (dizi değilse ya da 1 boyutlu ilkel dizi ise None)
    pub elem_class: Option<&'static Class>,
    /// tembel olarak oluşturulan, bileşeni `self` olan 1 boyutlu dizi sınıfı
    pub array_cache: ArrCache,
    /// arayüz metot tablosu: (seçici, fonksiyon) çiftleri, seçiciye göre sıralı; miras alınan ve default metotlar dahil
    pub itab: &'static [IEnt],
    /// bu sınıfın doğrudan ya da miras yoluyla uyguladığı her arayüz (düz liste)
    pub ifaces: &'static [&'static Class],
    /// bildirilen alanlar: örnek alan ofsetleri ve statik alan adresleri. Unsafe, VarHandle ve yansıma bunu kullanır.
    pub fields: &'static [FInfo],
    /// süper sınıflar boyunca düzleştirilmiş örnek referans alanı ofsetleri; kesin (precise) GC izlemesi için
    pub ref_offsets: &'static [u32],
    /// kesin GC kök taraması için statik referans alanlarının işaretçileri
    pub static_refs: &'static [SPtr],
    /// bildirilen metotlar (`<clinit>` hariç); yansıma için
    pub methods: &'static [MInfo],
    /// isteğe bağlı sınıf yansıma verisi: açıklamalar (annotation), sabit havuzu, jenerikler ve iç içe sınıf bilgisi
    pub refl: Option<&'static ClassRefl>,
    /// tembel olarak oluşturulan java.lang.Class nesnesi
    pub mirror: Mirror,
    /// 0: örnek sınıf, 1: arayüz, 2: dizi, 3: ilkel
    pub kind: u8,
    pub access: u16,
    /// sınıf başlatıcısı (gerekmiyorsa None)
    pub init: Option<extern "C" fn() -> CVoid>,
    /// bir enum sınıfının Enum.values() dizisi
    pub enum_values: Option<extern "C" fn() -> CRes<Ref>>,
}
pub struct ArrCache(pub std::sync::atomic::AtomicPtr<Class>);
unsafe impl Sync for ArrCache {}
impl ArrCache {
    // sabit, `static` değil: statik başlatıcıdaki her kullanım kendi hücresini alır
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
    /// Bu sınıfta ya da süper sınıfında bildirilmiş bir alanı bulur.
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
    // sabit, `static` değil: statik başlatıcıdaki her kullanım kendi hücresini alır
    #[allow(clippy::declare_interior_mutable_const)]
    pub const NEW: Mirror = Mirror(std::sync::atomic::AtomicPtr::new(std::ptr::null_mut()));
}
pub struct IEnt(pub u32, pub Fp);
unsafe impl Sync for IEnt {}

/// Bir referans: null ya da bir nesneye işaretçi.
/// Bu dilimde henüz çöp toplayıcı yok: bellek hiç serbest bırakılmaz.
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

/// Üretilen programın başlangıçta yaptığı kayıt: çalışma zamanının ihtiyaç duyduğu sınıflar ve geri çağrılar.
/// Bu fonksiyon, Java kodu çalışmadan önce bir kez çağrılır.
pub struct ServiceEntry {
    pub service: &'static str,
    pub create: fn() -> Res<Ref>,
}
unsafe impl Sync for ServiceEntry {}

pub struct Hooks {
    pub object_class: &'static Class,
    pub string_class: &'static Class,
    /// dizi sınıfları, Z B C S I J F D A sırasıyla
    pub arrays: [&'static Class; 9],
    pub array_ifaces: &'static [&'static Class],
    /// verilen mesajla adlandırılmış sınıftan bir istisna nesnesi oluşturur (mesaj bir String ya da null)
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
/// Çalışma zamanı kancaları. Herhangi bir Java kodu çalışmadan önce `init_hooks` tarafından bir kez ayarlanır; o zamana kadar null.
static HOOKS: std::sync::atomic::AtomicPtr<Hooks> = std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());
pub fn init_hooks(h: Hooks) {
    // bilerek sızdırılır: kancalar süreç boyunca yaşar ve kilitsiz okunur
    HOOKS.store(Box::leak(Box::new(h)), std::sync::atomic::Ordering::Release);
    crate::prof::init();
    crate::cpuprof::init();
    crate::diag::init();
    crate::crash::install();
    crate::ncount::install();
}
#[inline(always)]
pub fn hooks() -> &'static Hooks {
    unsafe { HOOKS.load(std::sync::atomic::Ordering::Acquire).as_ref() }.expect("Monolith: runtime hooks not registered")
}

/// MONO_TRACE_EX bir kez okunur. İstisnalar çok sık fırlatılır (sınıf aramaları, ayrıştırma ve akış kontrolünde bile),
/// ortam değişkenini okumak genel bir kilit alır ve bellek ayırır; bu yüzden değer bir kez okunup saklanır.
fn trace_ex() -> Option<&'static str> {
    static V: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    V.get_or_init(|| std::env::var("MONO_TRACE_EX").ok()).as_deref()
}

#[cold]
#[inline(never)]
pub fn throw<T>(class: &'static str, msg: String) -> Res<T> {
    if let Some(v) = trace_ex() {
        if v != "err" || class != "java/lang/ClassNotFoundException" {
            eprintln!("[mono:throw] {} ({})\n{}", class, msg, std::backtrace::Backtrace::force_capture());
        }
    }
    let m = if msg.is_empty() && class == "java/lang/NullPointerException" { Ref::NULL } else { new_str(msg) };
    Err(Thrown { obj: (hooks().throw_new)(class, m) })
}

/// `athrow`: nesneyi fırlatır. Nesne null ise NullPointerException fırlatır.
#[cold]
#[inline(never)]
pub fn throw_ref<T>(r: Ref) -> Res<T> {
    if r.is_null() {
        return throw("java/lang/NullPointerException", String::new());
    }
    if let Some(v) = trace_ex() {
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
            unsafe { satb_store_r((o.0 as *mut u8).add(off as usize) as *mut Ref, v); }
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

/// # Güvenlik
/// `arr` canlı bir bayt dizisi olmalıdır. Dönen dilim dizinin ömründen uzun yaşamamalı ve canlı bir `&mut` ile örtüşmemelidir.
pub unsafe fn byte_slice<'a>(arr: Ref) -> Res<&'a [u8]> {
    let a = nn(arr)? as *const Arr;
    let len = (*a).len as usize;
    Ok(std::slice::from_raw_parts((a as *const u8).add(24), len))
}

/// # Güvenlik
/// `arr` canlı bir bayt dizisi olmalıdır. Çağıran, dilimin ömrü boyunca baytlarına münhasır erişime sahip olmalıdır.
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

pub use crate::heap::{recycle as recycle_scope, safepoint as gc_safepoint, ArenaGuard, ThreadGuard};
pub use crate::heap::weak_read;

/// Java `volatile` alan okuması (çevrilmiş getfield/getstatic). JSR-133'ün ARM eşlemesi: yükleme, ardından edinme çiti.
/// Eskiden düz okumaydı: x86'da (TSO) sorun çıkmaz ama arm64'te sonraki okumalar öne geçer, kilitsiz yapılar (ForkJoinPool,
/// CompletableFuture, AbstractConsecutiveExecutor) yayımlanan görevi eksik görür ve görev kaybolur (S26: chunk üretimi ve
/// kaynak yükleme kilitlenmeleri). `read_volatile` derleyicinin okumayı döngü dışına almasını da engeller.
/// # Safety
/// `p` hizalı ve geçerli bir alan adresi olmalı.
#[inline(always)]
pub unsafe fn vol_load<T: Copy>(p: *const T) -> T {
    let v = std::ptr::read_volatile(p);
    std::sync::atomic::fence(std::sync::atomic::Ordering::Acquire);
    v
}
/// Java `volatile` alan yazımı: serbest bırakma çiti, yazma, ardından tam çit (sonraki volatile okumayla sıralı tutarlılık).
/// # Safety
/// `p` hizalı ve geçerli bir alan adresi olmalı.
#[inline(always)]
pub unsafe fn vol_store<T: Copy>(p: *mut T, v: T) {
    std::sync::atomic::fence(std::sync::atomic::Ordering::Release);
    std::ptr::write_volatile(p, v);
    std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
}
/// Volatile referans yazımında SATB bariyerinin etrafındaki çitler (yazma satb_store içinde yapılır).
#[inline(always)]
pub fn vol_fence_before() {
    std::sync::atomic::fence(std::sync::atomic::Ordering::Release);
}
#[inline(always)]
pub fn vol_fence_after() {
    std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
}

#[inline(always)]
pub fn gc_thread_guard() -> ThreadGuard {
    ThreadGuard::enter()
}

/// Bölgede tahsis (bkz. region.rs). Üretilen kod yalnızca MONO_REGION_CLASSES ile seçilen sınıflar için kullanır.
pub fn alloc_region(class: &'static Class) -> Ref {
    crate::region::alloc_region(class)
}

/// `if_acmpeq/ne`: bölge nesnesinin iletmesini izleyerek karşılaştırır (asıl ve kopya aynı nesnedir).
#[inline(always)]
pub fn ref_eq(a: Ref, b: Ref) -> bool {
    crate::region::ref_eq(a, b)
}

pub fn alloc(class: &'static Class) -> Ref {
    Ref(crate::heap::alloc_obj(class, class.size.max(16)))
}

/// Kaçış analizi sayesinde, onu oluşturan fonksiyonun çerçevesinde tutulan bir nesne.
/// Analiz, nesnenin o çağrı ya da döngü turu bitince öldüğünü ispatlamıştır.
/// `p`, bu tahsis noktasının fonksiyon içindeki yuvasıdır. `new` her çalıştığında temizlenir ve sınıf başlığı yazılır.
/// Çöp toplayıcı bu nesneyi yok sayar (heap adresi değildir); referans verdiği nesneleri yığınları tarayarak bulur.
/// # Güvenlik
/// `p`, nesnenin ömrü boyunca `class.size.max(16)` bayt yazma için geçerli ve hizalı olmalı; nesne yaşadığı sürece canlı kalmalıdır.
#[inline(always)]
pub unsafe fn stack_obj(p: *mut u8, class: &'static Class) -> Ref {
    unsafe {
        std::ptr::write_bytes(p, 0, class.size.max(16));
        let o = p as *mut Obj;
        (*o).class = class;
        (*o).gc = crate::heap::GC_ARENA;
        Ref(o)
    }
}

/// Kapsamı bitince geri dönüştürülen örneklerin `new`'ü (for-each döngüsü yineleyicileri gibi).
pub fn alloc_recyclable(class: &'static Class, slot: usize) -> Ref {
    crate::heap::alloc_recyclable(class, slot)
}
pub fn alloc_arena(class: &'static Class) -> Ref {
    crate::heap::alloc_arena(class, class.size.max(16))
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

/// invokeinterface: alıcının sınıfında `sel` seçicisinin uygulaması.
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

// ---------------------------------------------------------------- çağrı yeri önbelleği (MONO_ICACHE=1 ile üretilen kod)
//
// Acemi notu: bir arayüz çağrısı (`list.size()` gibi, `list` bir List arayüzü) hangi metodun çalışacağını nesnenin sınıfına
// bakarak bulur. `itab_find` bunu her çağrıda sınıfın arayüz tablosunda ikili aramayla yapar (ölçüm: ~6 ns). Oysa bir çağrı
// yeri pratikte çoğunlukla hep aynı sınıfı görür. Önbellek o çağrı yerinin İLK gördüğü sınıfı ve bulunan metodu saklar;
// sonraki çağrılarda sınıf aynıysa tek bir karşılaştırmayla metodu verir (~1,5 ns). HotSpot buna "inline cache" der.
// Aynı fikir `checkcast`/`instanceof` için de kullanılır: başarıyla geçmiş sınıf saklanır, aynı sınıf bir daha zincir
// gezmeden geçer (ölçüm: ~9 ns -> ~1 ns).
//
// Doğruluk: sınıf -> metot ve sınıf -> "şunun alt türü mü" eşlemeleri program boyunca değişmez, yani saklanan sonuç hiç
// bayatlamaz. Kayıt bir kez yazılır (CAS ile, null'dan dolu hale) ve bir daha değişmez; okuyan iş parçacığı ya boş görür ya
// da tam yazılmış kaydı (kayıt yazıldıktan SONRA işaretçi yayımlanır). Başka sınıf gören çağrılar eski yavaş yoldan gider,
// yani en kötü durumda bugünkü hızdır. Bellek: çağrı yeri başına en çok bir 16 baytlık kayıt.

/// Bir çağrı yerinin önbelleği. Üretilen kodda her çağrı yeri kendi `static`'ini taşır.
pub struct ICache(std::sync::atomic::AtomicPtr<ICEntry>);
#[repr(C)]
pub struct ICEntry {
    // `class` ve `val` ilk iki alan kalmalı: satır içine alınmış `hit` (üretilmiş parçalarda) bu yerleşimi okur
    class: *const Class,
    val: *const (),
    /// MONO_IC_POLY=1: aynı çağrı yerinin gördüğü sonraki sınıflar (zincir, en çok IC_POLY_MAX kayıt; yalnızca eklenir)
    next: std::sync::atomic::AtomicPtr<ICEntry>,
}

/// MONO_IC_POLY (çalışma zamanı, varsayılan AÇIK; =0 kapatır): çağrı yeri önbelleği çok girişli olur. Eski yol yalnızca İLK görülen
/// sınıfı saklar; başka sınıf gören çağrı yeri (çok biçimli: List'in birkaç uygulaması gibi) her seferinde yavaş yoldan gider
/// (`itab_find` ikili arama, `instance_of` üst sınıf zinciri). S26 v14 profilinde `itab_find_ic_slow` + `is_subclass` CPU'nun
/// %5-7'si. Açıkken yavaş yol önce zincirde arar, bulamazsa hesaplayıp zincirin sonuna ekler. Kayıtlar hiç değişmez ve
/// silinmez (sınıf -> metot eşlemesi program boyunca sabit), okuyucu ya boş ya da tam yazılmış kayıt görür.
const IC_POLY_MAX: usize = 8;
fn ic_poly() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    // 27 Kasım 2026'dan beri varsayılan AÇIK (S26: itab_find_ic_slow + is_subclass + fill %6,2 -> %3,6); "0" eski yolu getirir
    *V.get_or_init(|| std::env::var("MONO_IC_POLY").map_or(true, |v| v != "0"))
}
impl Default for ICache {
    fn default() -> Self {
        Self::new()
    }
}
impl ICache {
    pub const fn new() -> Self {
        ICache(std::sync::atomic::AtomicPtr::new(std::ptr::null_mut()))
    }
    /// Kayıtlıysa ve sınıf eşleşiyorsa saklanan değer.
    #[inline(always)]
    fn hit(&self, c: &'static Class) -> Option<*const ()> {
        let p = self.0.load(std::sync::atomic::Ordering::Acquire);
        if !p.is_null() {
            let e = unsafe { &*p };
            if std::ptr::eq(e.class, c) {
                return Some(e.val);
            }
        }
        None
    }
    /// Önbellek boşsa (c, val) kaydını yazar; doluysa (başka sınıf için yazılmışsa) dokunmaz.
    #[cold]
    #[inline(never)]
    fn fill(&self, c: &'static Class, val: *const ()) {
        use std::sync::atomic::Ordering;
        if !self.0.load(Ordering::Relaxed).is_null() {
            return;
        }
        let b = Box::into_raw(Box::new(ICEntry { class: c, val, next: std::sync::atomic::AtomicPtr::new(std::ptr::null_mut()) }));
        if self.0.compare_exchange(std::ptr::null_mut(), b, Ordering::AcqRel, Ordering::Acquire).is_err() {
            drop(unsafe { Box::from_raw(b) }); // başka iş parçacığı önce yazdı
        }
    }
    /// MONO_IC_POLY: zincirin tamamında (ilk kayıt dahil) `c` için saklanan değer.
    #[inline]
    fn poly_find(&self, c: &'static Class) -> Option<*const ()> {
        use std::sync::atomic::Ordering;
        let mut p = self.0.load(Ordering::Acquire);
        let mut n = 0;
        while !p.is_null() && n <= IC_POLY_MAX {
            let e = unsafe { &*p };
            if std::ptr::eq(e.class, c) {
                return Some(e.val);
            }
            p = e.next.load(Ordering::Acquire);
            n += 1;
        }
        None
    }
    /// MONO_IC_POLY: (c, val) kaydını zincirin sonuna ekler (zincir doluysa ya da yarışta kaybedilirse eklemez).
    #[cold]
    #[inline(never)]
    fn poly_add(&self, c: &'static Class, val: *const ()) {
        use std::sync::atomic::Ordering;
        let head = self.0.load(Ordering::Acquire);
        if head.is_null() {
            self.fill(c, val);
            return;
        }
        let mut p = head;
        for _ in 0..IC_POLY_MAX {
            let e = unsafe { &*p };
            if std::ptr::eq(e.class, c) {
                return; // başka iş parçacığı ekledi
            }
            let nx = e.next.load(Ordering::Acquire);
            if nx.is_null() {
                let b = Box::into_raw(Box::new(ICEntry { class: c, val, next: std::sync::atomic::AtomicPtr::new(std::ptr::null_mut()) }));
                if e.next.compare_exchange(std::ptr::null_mut(), b, Ordering::AcqRel, Ordering::Acquire).is_err() {
                    drop(unsafe { Box::from_raw(b) });
                }
                return;
            }
            p = nx;
        }
    }
}

/// `itab_find`'in önbellekli hali (arayüz çağrısı).
#[inline(always)]
pub fn itab_find_ic(ic: &'static ICache, c: &'static Class, sel: u32) -> *const () {
    if let Some(f) = ic.hit(c) {
        return f;
    }
    itab_find_ic_slow(ic, c, sel)
}
#[cold]
#[inline(never)]
fn itab_find_ic_slow(ic: &'static ICache, c: &'static Class, sel: u32) -> *const () {
    if ic_poly() {
        if let Some(f) = ic.poly_find(c) {
            return f;
        }
        let f = itab_find(c, sel);
        ic.poly_add(c, f);
        return f;
    }
    let f = itab_find(c, sel);
    ic.fill(c, f);
    f
}

/// `checkcast`'in önbellekli hali: null ya da tam hedef sınıf satır içinde, daha önce geçmiş sınıf önbellekten geçer.
#[inline(always)]
pub fn checkcast_ic(r: Ref, target: &'static Class, ic: &'static ICache) -> Res<Ref> {
    if r.is_null() {
        return Ok(r);
    }
    let c = unsafe { (*r.0).class };
    if std::ptr::eq(c, target) || ic.hit(c).is_some() {
        return Ok(r);
    }
    checkcast_ic_slow(r, target, ic)
}
#[cold]
#[inline(never)]
fn checkcast_ic_slow(r: Ref, target: &'static Class, ic: &'static ICache) -> Res<Ref> {
    if ic_poly() {
        let c = unsafe { (*r.0).class };
        if ic.poly_find(c).is_some() {
            return Ok(r);
        }
        let r = checkcast(r, target)?;
        ic.poly_add(c, std::ptr::null());
        return Ok(r);
    }
    let r = checkcast(r, target)?;
    ic.fill(unsafe { (*r.0).class }, std::ptr::null());
    Ok(r)
}

/// `instance_of`'un önbellekli hali: yalnızca "evet" sonuçları saklanır.
#[inline(always)]
pub fn instance_of_ic(r: Ref, target: &'static Class, ic: &'static ICache) -> bool {
    if r.is_null() {
        return false;
    }
    let c = unsafe { (*r.0).class };
    if std::ptr::eq(c, target) || ic.hit(c).is_some() {
        return true;
    }
    instance_of_ic_slow(r, target, ic)
}
#[cold]
#[inline(never)]
fn instance_of_ic_slow(r: Ref, target: &'static Class, ic: &'static ICache) -> bool {
    if ic_poly() {
        let c = unsafe { (*r.0).class };
        if ic.poly_find(c).is_some() {
            return true;
        }
        let yes = instance_of(r, target);
        if yes {
            ic.poly_add(c, std::ptr::null());
        }
        return yes;
    }
    let yes = instance_of(r, target);
    if yes {
        ic.fill(unsafe { (*r.0).class }, std::ptr::null());
    }
    yes
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

// ------------------------------------------------------------------ dizgiler
/// Overlay sınıf kütüphanesindeki java.lang.String yerleşimi. Üreteç alan sırasını denetler.
#[repr(C)]
pub struct StrObj {
    pub hdr: Obj,
    pub value: Ref,
    pub hash: i32,
    pub hash_is_zero: i8,
    pub coder: i8,
}
/// UTF-16 kod birimlerinden yeni bir String.
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
/// Literal (sabit) dizge: her kullanım noktası için bir kez oluşturulur. İş parçacığı güvenli CAS ve GC kökü kaydı yapılır.
/// Bir dizge değerinin tekil (kanonik) örneği. `String.intern()` ve dizge literalleri bu tabloyu paylaşır,
/// böylece `"x" == name.intern()` doğrudur. JDK yansıtılmış üye adlarını `==` ile karşılaştırır (örn. AnnotationInvocationHandler).
pub fn intern_string(s: Ref) -> Ref {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static TABLE: OnceLock<Mutex<HashMap<Vec<u16>, usize>>> = OnceLock::new();
    let key = str_units(s);
    let mut t = TABLE.get_or_init(|| Mutex::new(HashMap::new())).lock().unwrap();
    let ptr = *t.entry(key).or_insert_with(|| {
        crate::heap::pin_ref(s);
        s.0 as usize
    });
    Ref(ptr as *mut Obj)
}
/// # Güvenlik
/// `slot`, bir `Ref` için okuma ve yazmaya geçerli olmalı; eşzamanlama olmadan eşzamanlı erişilmemelidir.
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
            crate::heap::register_static_slot(slot);
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

// ------------------------------------------------------------------ diziler
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
            crate::heap::register_array_class(ac);
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
    let p = crate::heap::alloc_obj(ac, size) as *mut Arr;
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
    let r = crate::heap::alloc_arena(arr_class(e), size);
    unsafe { (*(r.0 as *mut Arr)).len = len };
    Ok(r)
}
pub fn new_obj_array_arena(elem_c: &'static Class, len: i32) -> Res<Ref> {
    if len < 0 {
        return throw("java/lang/NegativeArraySizeException", len.to_string());
    }
    let size = 24 + 8 * len as usize;
    let r = crate::heap::alloc_arena(array_class_of(elem_c), size);
    unsafe { (*(r.0 as *mut Arr)).len = len };
    Ok(r)
}
/// `multianewarray`: `leaf` türünden, `depth` boyutlu dizi (isteğe bağlı nesne bileşen sınıfı `elem_c` ile), verilen `dims` uzunluklarıyla.
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
/// # Güvenlik
/// `gc::satb_store` ile aynı sözleşme: `slot` geçerli ve hizalı bir `Ref` konumu olmalı.
#[inline(always)]
pub unsafe fn satb_store(slot: *mut Ref, v: Ref) {
    crate::heap::satb_store(slot, v)
}

/// Bölge farkında referans yazımı: `slot` bölge nesnesinin içindeyse değer olduğu gibi (kurucu), değilse bölge nesnesi yığına
/// kopyalanıp kopya yazılır (docs/PIPELINE_VE_YOL_HARITASI_TR.md bölüm 18). Üretilen kod bunu yalnızca bölge sınıfı seçilince kullanır.
///
/// # Safety
/// `satb_store` ile aynı sözleşme (clippy yalnızca İngilizce "Safety" başlığını tanır).
#[inline(always)]
pub unsafe fn satb_store_r(slot: *mut Ref, v: Ref) {
    crate::heap::satb_store(slot, crate::region::guard_into(slot as usize, v))
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
    unsafe { crate::heap::satb_store((d as *mut Ref).add(i as usize), crate::region::guard(v)) };
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

// kaydırmalar: Java kaydırma sayısını işlenenin genişliğine maskeler; `wrapping_sh*` aynısını yapar.
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

// karşılaştırmalar: JVM belirtimine göre NaN durumu. fcmpl/dcmpl -1, fcmpg/dcmpg 1 verir.
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

// dönüşümler: Rust'ın `as` operatörü float'ı doyurur ve NaN'ı 0'a çevirir. Java kuralı tam olarak budur.
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

/// `bastore` hem byte[] hem boolean[] için çalışır. Boolean elemanlar yalnızca en düşük biti tutar.
pub fn bastore(a: Ref, i: i32, v: i32) -> Res<()> {
    let d = checked(a, i)?;
    let z = unsafe { (*a.0).class.elem } == b'Z';
    unsafe { *(d as *mut i8).add(i as usize) = if z { (v & 1) as i8 } else { v as i8 } };
    Ok(())
}


// ------------------------------------------------------------------ çeşitli çalışma zamanı hizmetleri
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

/// Bir çalışma zamanı sınıfının java.lang.Class örneği. Overlay Class'ın ilk alanı `monoClass` adlı uzun tamsayıdır.
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

/// Monitörler sync.rs içinde yaşar.
pub fn monitorenter(r: Ref) -> Res<()> {
    crate::sync::enter(r)
}
pub fn monitorexit(r: Ref) -> Res<()> {
    crate::sync::exit(r)
}

pub fn set_main_thread(t: Ref) {
    crate::sync::set_current(t);
    crate::heap::set_thread_obj(t);
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
/// Ayna nesnesi -> işaretçi denetimi. Class nesneleri null olamaz.
pub fn nn_ref(r: Ref) -> Res<Ref> {
    nn(r)?;
    Ok(r)
}

/// Bir String nesnesinin UTF-16 kod birimleri.
pub fn str_units(r: Ref) -> Vec<u16> {
    unsafe {
        let v = (*(r.0 as *mut StrObj)).value;
        let n = (*(v.0 as *mut Arr)).len as usize;
        std::slice::from_raw_parts((v.0 as *mut u8).add(24) as *const u16, n).to_vec()
    }
}

// ------------------------------------------------------------------ sınıf başlatma (JLS 12.4.2)
use std::sync::atomic::{AtomicU64, Ordering};
const ST_DONE: u64 = 2;
const ST_ERROR: u64 = 3;
fn init_lock() -> &'static (std::sync::Mutex<()>, std::sync::Condvar) {
    static L: std::sync::OnceLock<(std::sync::Mutex<()>, std::sync::Condvar)> = std::sync::OnceLock::new();
    L.get_or_init(|| (std::sync::Mutex::new(()), std::sync::Condvar::new()))
}
/// Ok(true) dönerse çağıran başlatıcıyı çalıştırmalıdır. Ok(false) ise başlatma bitmiştir ya da bu iş parçacığı tarafından çalıştırılıyordur.
/// Hızlı yol (başlatma bitmiş) çağıranın içine alınır: S26 v8 CPU profilinde `init_enter` kendi süresi %3,3'tü ve neredeyse
/// tamamı bu yükle + karşılaştırma için yapılan çapraz crate çağrısıydı. Davranış aynı; geri kalanı `init_enter_slow`.
#[inline(always)]
pub fn init_enter(st: &AtomicU64, name: &str) -> Res<bool> {
    if st.load(Ordering::Acquire) == ST_DONE {
        return Ok(false);
    }
    init_enter_slow(st, name)
}
#[cold]
#[inline(never)]
fn init_enter_slow(st: &AtomicU64, name: &str) -> Res<bool> {
    let me = crate::sync::tid() * 4 + 1;
    let _blk = crate::heap::ScopedBlock::enter();
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
                return Err(Thrown { obj: class_init_error(name) });
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
    if !ok && trace_ex().is_some() {
        eprintln!("[mono:init_fail]\n{}", std::backtrace::Backtrace::force_capture());
    }
    let (m, cv) = init_lock();
    let _g = m.lock().unwrap();
    st.store(if ok { ST_DONE } else { ST_ERROR }, Ordering::Release);
    cv.notify_all();
}

// ------------------------------------------------------------------ çağıran sınıf, program argümanları, sistem özellikleri
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
/// Programın komut satırı. Paylaşımlı kütüphane ana makinesi (Android) bunu `set_host_args` ile kurar;
/// çalıştırılabilir dosya ise süreç argümanlarını işletim sisteminden okur.
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
/// `-Dkey=value` biçimindeki argümanlar sistem özelliğidir; diğerleri program argümanıdır.
pub fn user_props() -> Vec<String> {
    let mut v: Vec<String> = cmdline().into_iter().filter(|a| a.starts_with("-D")).map(|a| a[2..].to_string()).collect();
    // Netty'nin yerel taşıyıcıları (epoll ve kqueue) Rust'ta öykünülen JNI kodudur (174 native). Onlar olmadan Netty
    // düz Java olan NIO taşıyıcısını kullanır ve yerel kod gerektirmez. Bir başlatıcı ya da kullanıcı yine de değeri değiştirebilir.
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

// ---------------------------------------------------------------- yığın derinliği
// Java programları sınırsız özyinelemede StackOverflowError bekler. Üretilen metotlar girişte `stack_check` çağırır.
// Bu fonksiyon yığın işaretçisini, iş parçacığının alt sınırıyla karşılaştırır. Alt sınır işletim sisteminden alınır.
// Alt sınır, iş parçacığı ilk soruşunda alınır.
thread_local! {
    static STACK_LOW: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static IN_OVERFLOW: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

unsafe extern "C" {
    fn pthread_self() -> usize;
    fn pthread_getattr_np(thread: usize, attr: *mut [u64; 16]) -> i32;
    fn pthread_attr_getstack(attr: *const [u64; 16], addr: *mut *mut std::os::raw::c_void, size: *mut usize) -> i32;
    fn pthread_attr_destroy(attr: *mut [u64; 16]) -> i32;
}

/// Bu iş parçacığının kullanılabilir en düşük yığın adresi. Çalışma zamanının kendi çerçeveleri için bir pay bırakılır.
#[cold]
#[inline(never)]
fn stack_low_init() -> usize {
    let mut attr = [0u64; 16];
    // GÜVENLİK: `attr`, glibc ve bionic'te pthread_attr_t için yeterince büyüktür. pthread_getattr_np tarafından ilklenir
    // ve fonksiyon dönmeden önce yok edilir.
    unsafe {
        if pthread_getattr_np(pthread_self(), &mut attr) != 0 {
            return 0;
        }
        let mut addr: *mut std::os::raw::c_void = std::ptr::null_mut();
        let mut size: usize = 0;
        let ok = pthread_attr_getstack(&attr, &mut addr, &mut size) == 0;
        pthread_attr_destroy(&mut attr);
        if !ok || addr.is_null() {
            return 0;
        }
        let margin = (256 * 1024).min(size / 4);
        addr as usize + margin
    }
}

/// Mevcut iş parçacığının yığını neredeyse tükendiğinde StackOverflowError fırlatır.
#[inline(always)]
pub fn stack_check() -> Res<()> {
    let marker = 0u8;
    let sp = std::ptr::addr_of!(marker) as usize;
    let mut low = STACK_LOW.with(|c| c.get());
    if low == 0 {
        low = stack_low_init();
        STACK_LOW.with(|c| c.set(low));
        if low == 0 {
            return Ok(()); // the OS would not tell the bounds: no check, as before
        }
    }
    // Hata nesnesi kurulurken kurucuları tampon bölgede çalışır. Bu kurucular yeniden hata fırlatmamalıdır;
    if sp < low && !IN_OVERFLOW.with(|c| c.get()) {
        IN_OVERFLOW.with(|c| c.set(true));
        let e = stack_overflow();
        IN_OVERFLOW.with(|c| c.set(false));
        return e;
    }
    Ok(())
}

#[cold]
#[inline(never)]
fn stack_overflow<T>() -> Res<T> {
    throw("java/lang/StackOverflowError", String::new())
}

/// Statik başlatıcısı başarısız olan sınıfların özgün istisnaları, sınıf adına göre tutulur. Sabitlenir:
/// çöp toplayıcı bunları serbest bırakmamalıdır, çünkü sonraki erişimler bunları bildirir.
static INIT_FAILURES: std::sync::Mutex<Vec<(String, usize)>> = std::sync::Mutex::new(Vec::new());

/// Bir throwable'ın `Class.toString` benzeri metni: "java.lang.RuntimeException: mesaj".
fn throwable_text(obj: Ref) -> String {
    let cls = unsafe { (*obj.0).class.name }.replace('/', ".");
    let msg = get_ref(obj, "detailMessage");
    if msg.is_null() {
        cls
    } else {
        format!("{}: {}", cls, str_to_rust(msg))
    }
}

/// Başlatma başarısız olduktan sonraki her erişimin fırlattığı NoClassDefFoundError. HotSpot ilk hatayı
/// açıklayan bir ExceptionInInitializerError'ı nedeni olarak ekler.
fn class_init_error(name: &str) -> Ref {
    let dotted = name.replace('/', ".");
    let first = INIT_FAILURES.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(n, _)| n == name).map(|(_, p)| *p);
    let ncdfe = (hooks().throw_new)("java/lang/NoClassDefFoundError", new_str(format!("Could not initialize class {}", dotted)));
    if let Some(p) = first {
        let orig = Ref(p as *mut Obj);
        let thread = std::thread::current().name().unwrap_or("main").to_string();
        let text = format!("Exception {} [in thread \"{}\"]", throwable_text(orig), thread);
        let cause = (hooks().throw_new)("java/lang/ExceptionInInitializerError", new_str(text));
        set_ref(ncdfe, "cause", cause);
    }
    ncdfe
}

/// JLS 12.4.2: statik başlatıcıdan kaçan bir istisna ExceptionInInitializerError içine sarılır. Nedeni özgün istisnadır.
/// Error ise olduğu gibi yayılır ve sınıf kullanılamaz hâle gelir.
pub fn init_failure(name: &str, r: Res<()>) -> Res<()> {
    let Err(t) = r else { return Ok(()) };
    if let Some(err) = find_class("java/lang/Error") {
        if instance_of(t.obj, err) {
            return Err(t);
        }
    }
    crate::heap::pin_ref(t.obj);
    INIT_FAILURES.lock().unwrap_or_else(|e| e.into_inner()).push((name.to_string(), t.obj.0 as usize));
    let wrapped = (hooks().throw_new)("java/lang/ExceptionInInitializerError", Ref::NULL);
    set_ref(wrapped, "cause", t.obj);
    Err(Thrown { obj: wrapped })
}

// ---------------------------------------------------------------- yığın gölgesi (Throwable yığın izleri)
// Üretilen her metot girişte bir çerçeve etiketi iter ve guard bırakıldığında çıkarır (`?` ile dönüşlerde de).
// Acemi notu: Yığın izi (stack trace), hangi metotların o an açık olduğunu bu etiketlerden okuyarak oluşur.
// `capture_backtrace` etiketleri en içten başlayarak okur ve `Throwable.fillInStackTrace` için kullanır.
#[derive(Clone, Copy)]
struct FrameTag {
    class: &'static str,
    method: &'static str,
    desc: &'static str,
    file: &'static str,
    /// İşlenmekte olan ifadenin kaynak satırı (ilk satır işaretçisi gelene kadar -1)
    line: i32,
}

thread_local! {
    static FRAMES: std::cell::RefCell<Vec<FrameTag>> = const { std::cell::RefCell::new(Vec::new()) };
}

pub struct FrameGuard;

impl Drop for FrameGuard {
    fn drop(&mut self) {
        // İş parçacığı sonlanırken yuva zaten yok olmuş olabilir. O zaman çıkarılacak bir şey yoktur.
        let _ = FRAMES.try_with(|f| {
            if let Ok(mut v) = f.try_borrow_mut() {
                v.pop();
            }
        });
    }
}

/// Mevcut çerçevenin çalıştığı kaynak satırını işaretler. Her ifadenin başında çağrılır.
#[inline(always)]
pub fn set_line(line: i32) {
    let _ = FRAMES.try_with(|f| {
        if let Ok(mut v) = f.try_borrow_mut() {
            if let Some(top) = v.last_mut() {
                top.line = line;
            }
        }
    });
}

#[inline(always)]
pub fn enter_frame(class: &'static str, method: &'static str, desc: &'static str, file: &'static str) -> FrameGuard {
    let _ = FRAMES.try_with(|f| {
        if let Ok(mut v) = f.try_borrow_mut() {
            v.push(FrameTag { class, method, desc, file, line: -1 });
        }
    });
    FrameGuard
}

// ---------------------------------------------------------------- hızlı gölge yığın (MONO_FRAMES=fast ile üretilen kod)
//
// Acemi notu: yukarıdaki eski yol her metot çağrısında 72 baytlık bir etiketi (dört metin + satır) bir `RefCell<Vec>`'e iter.
// Bunun için her çağrıda: iş parçacığına özel değişkene "hâlâ yaşıyor mu" sorusu (`try_with`), RefCell kilidi, Vec
// kapasite denetimi ve 72 bayt kopyalama yapılır; ayrıca `stack_check` aynı işi ayrı bir değişken için tekrarlar.
// Ölçüm (fib(35), ~30 milyon çağrı): eski yol 245 ms, bu yol 112 ms, hiç defter tutmamak 43 ms.
//
// Bu yolda:
//   * Her metodun sınıf/metot/tanımlayıcı/dosya bilgisi derleme anında bir kez `static FI: FrameInfo` olarak yazılır;
//     çağrıda yalnızca ona işaretçi (8 bayt) ve satır (4 bayt) itilir.
//   * Çerçeveler kilitsiz, ham bir tamponda tutulur (yalnızca kendi iş parçacığı dokunur, kilit gereksiz).
//   * Yığın taşması denetimi aynı yapının içindeki alt sınırla yapılır (ikinci bir thread-local erişimi yok).
// Bilgi eskisiyle birebir aynıdır: yığın izleri, StackWalker ve hata raporları değişmez.
// Bir derleme ya tamamen eski ya tamamen yeni yolla üretilir; iki yol aynı programda karışmaz.

/// Bir metodun sabit kimliği: derleme anında üretilir, programın ömrü boyunca yaşar.
pub struct FrameInfo {
    pub class: &'static str,
    pub method: &'static str,
    pub desc: &'static str,
    pub file: &'static str,
}

#[derive(Clone, Copy)]
struct FastSlot {
    info: *const FrameInfo,
    /// İşlenmekte olan ifadenin kaynak satırı (ilk satır işaretçisi gelene kadar -1)
    line: i32,
}

struct FastStack {
    /// çerçeve tamponu (ilk kullanımda ayrılır, dolunca iki katına büyür)
    p: *mut FastSlot,
    len: usize,
    cap: usize,
    /// yığın alt sınırı: 0 = henüz sorulmadı, 1 = işletim sistemi söylemedi (denetim yok), diğer = sınır
    low: usize,
    /// StackOverflowError nesnesi kurulurken tampon bölgede çalışılıyor (bu sırada yeniden fırlatılmaz)
    in_overflow: bool,
}

thread_local! {
    // Drop'u olmayan, sabit ilk değerli thread-local: erişimi tek bir adres hesabıdır ("hâlâ yaşıyor mu" denetimi yok).
    static FAST: std::cell::UnsafeCell<FastStack> = const {
        std::cell::UnsafeCell::new(FastStack { p: std::ptr::null_mut(), len: 0, cap: 0, low: 0, in_overflow: false })
    };
    // Yalnızca iş parçacığı biterken FAST'in tamponunu geri vermek için var; ilk büyütmede kaydedilir.
    static FAST_FREE: FastFree = const { FastFree };
}

/// Çerçeve yığınına erişim. Varsayılan: `thread_local!` (masaüstünde yerel TLS, tek adres hesabı). ANDROID'DE `std` yerel TLS
/// kullanmaz: her erişim `pthread_getspecific` çağrısıdır (S26 CPU profili: set_line, çerçeve girişi/çıkışı ve libc'deki
/// bu çağrı toplam CPU'nun yaklaşık %60'ı). `--cfg mono_native_tls` (MONO_NATIVE_TLS=1 ile Android derlemesi) ile gerçek
/// yerel TLS (`#[thread_local]`, global-dynamic/TLSDESC modeli) kullanılır. Bayrak yoksa eski yol.
#[cfg(not(mono_native_tls))]
#[inline(always)]
fn with_fast<R>(f: impl FnOnce(&mut FastStack) -> R) -> R {
    FAST.with(|c| f(unsafe { &mut *c.get() }))
}

#[cfg(mono_native_tls)]
#[thread_local]
static mut FAST_NATIVE: FastStack = FastStack { p: std::ptr::null_mut(), len: 0, cap: 0, low: 0, in_overflow: false };

#[cfg(all(mono_native_tls, not(all(mono_tls_slot, target_arch = "aarch64"))))]
#[inline(always)]
fn with_fast<R>(f: impl FnOnce(&mut FastStack) -> R) -> R {
    f(unsafe { &mut *std::ptr::addr_of_mut!(FAST_NATIVE) })
}

/// `--cfg mono_tls_slot` (MONO_TLS_SLOT=1, varsayılan kapalı): dlopen ile yüklenen kütüphanede `#[thread_local]` erişimi
/// TLSDESC dinamik çözücüsüne giden bir çağrıdır (S26 v8 profili: `[linker64]` CPU'nun %10,6'sı, her Java çağrısında bir
/// kez). Bionic, iş parçacığı denetim bloğunda uygulamalara bir yuva ayırır: `TLS_SLOT_APP` = 2, arm64'te
/// `tpidr_el0 + 2*8`, API 29 ve sonrası ("Available for use by apps in API level 29 and later", bionic
/// libc/platform/bionic/tls_defines.h). Yuvaya bu iş parçacığının `FAST_NATIVE` adresi bir kez yazılır; sonraki erişimler
/// `mrs` + `ldr` (çağrı yok). Varsayım: süreçteki başka kod bu yuvayı kullanmıyor; boşsa (0) biz doldururuz, doluysa onu
/// kendi adresimiz sayarız. Bu yüzden anahtar arkasında.
#[cfg(all(mono_native_tls, mono_tls_slot, target_arch = "aarch64"))]
const TLS_SLOT_APP: usize = 2;

#[cfg(all(mono_native_tls, mono_tls_slot, target_arch = "aarch64"))]
#[cold]
#[inline(never)]
unsafe fn fast_slot_init(slot: *mut *mut FastStack) -> *mut FastStack {
    let p = std::ptr::addr_of_mut!(FAST_NATIVE);
    *slot = p;
    p
}

#[cfg(all(mono_native_tls, mono_tls_slot, target_arch = "aarch64"))]
#[inline(always)]
fn with_fast<R>(f: impl FnOnce(&mut FastStack) -> R) -> R {
    unsafe {
        let tp: *mut *mut FastStack;
        // tpidr_el0 iş parçacığı boyunca değişmez: pure + nomem, derleyici aynı işlevde tekrar okumayı birleştirebilir
        std::arch::asm!("mrs {}, tpidr_el0", out(reg) tp, options(pure, nomem, nostack, preserves_flags));
        let slot = tp.add(TLS_SLOT_APP);
        let mut p = *slot;
        if p.is_null() {
            p = fast_slot_init(slot);
        }
        f(&mut *p)
    }
}

struct FastFree;
impl Drop for FastFree {
    fn drop(&mut self) {
        // FAST'in Drop'u olmadığı için bu noktada hâlâ erişilebilir.
        with_fast(|f| unsafe {
            fast_unregister(f);
            if f.cap > 0 {
                drop(Vec::from_raw_parts(f.p, 0, f.cap));
                f.p = std::ptr::null_mut();
                f.cap = 0;
                f.len = 0;
            }
        });
    }
}

// ---- MONO_DIAG takılma dökümü: iş parçacıklarının çerçeve yığınlarının adresleri. Yalnızca MONO_DIAG açıkken doldurulur
// (ilk çerçeve tamponu ayrılırken kaydolur, iş parçacığı biterken silinir). diag.rs, kareler durduğunda (çizim iş parçacığı bir
// yerde bekliyorsa) bunlardan her Java iş parçacığının o anki yığın izini yazar. Okuma kilitsizdir ve çalışan iş parçacıklarının
// yığını o an değişiyor olabilir: döküm yalnızca tanı içindir (en çok MAX_DUMP_FRAMES çerçeve, işaretçiler denetlenir).
static FAST_REG: std::sync::Mutex<Vec<(usize, i32)>> = std::sync::Mutex::new(Vec::new());
#[cfg(any(target_os = "linux", target_os = "android"))]
fn os_tid() -> i32 {
    extern "C" {
        fn gettid() -> i32;
    }
    unsafe { gettid() }
}
#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn os_tid() -> i32 {
    0
}
fn fast_register(f: &mut FastStack) {
    if crate::diag::on() {
        if let Ok(mut r) = FAST_REG.lock() {
            r.push((f as *mut FastStack as usize, os_tid()));
        }
    }
}
fn fast_unregister(f: &mut FastStack) {
    if crate::diag::on() {
        if let Ok(mut r) = FAST_REG.lock() {
            let a = f as *mut FastStack as usize;
            r.retain(|e| e.0 != a);
        }
    }
}
/// Kayıtlı her iş parçacığının yığın izi (en içten başlayarak, en çok `max` çerçeve).
pub fn dump_fast_stacks(max: usize) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let Ok(r) = FAST_REG.lock() else { return out };
    for &(addr, tid) in r.iter() {
        let name = std::fs::read_to_string(format!("/proc/self/task/{tid}/comm")).unwrap_or_default();
        let f = unsafe { &*(addr as *const FastStack) };
        let (p, len, cap) = (f.p, f.len, f.cap);
        let _ = writeln!(out, "-- {} #{} ({} çerçeve)", name.trim(), tid, len);
        if p.is_null() || len > cap {
            continue;
        }
        for i in (0..len).rev().take(max) {
            let s = unsafe { *p.add(i) };
            if s.info.is_null() {
                continue;
            }
            let fi = unsafe { &*s.info };
            let _ = writeln!(out, "    at {}.{}({}:{})", fi.class.replace('/', "."), fi.method, fi.file, s.line);
        }
    }
    out
}

#[cold]
#[inline(never)]
fn fast_grow(f: &mut FastStack) {
    let nc = if f.cap == 0 { 1024 } else { f.cap * 2 };
    let mut v: Vec<FastSlot> = Vec::with_capacity(nc);
    unsafe {
        std::ptr::copy_nonoverlapping(f.p, v.as_mut_ptr(), f.len);
        if f.cap > 0 {
            drop(Vec::from_raw_parts(f.p, 0, f.cap));
        } else {
            FAST_FREE.with(|_| ()); // ilk ayırma: iş parçacığı bitince tamponu geri verecek nesneyi kaydet
            fast_register(f);
        }
    }
    f.p = v.as_mut_ptr();
    f.cap = nc;
    std::mem::forget(v);
}

#[cold]
#[inline(never)]
fn fast_low_init(f: &mut FastStack) {
    let l = stack_low_init();
    f.low = if l == 0 { 1 } else { l };
}

#[cold]
#[inline(never)]
fn fast_overflow() -> Res<FastGuard> {
    with_fast(|f| f.in_overflow = true);
    let e = stack_overflow();
    with_fast(|f| f.in_overflow = false);
    e
}

pub struct FastGuard;

impl Drop for FastGuard {
    #[inline(always)]
    fn drop(&mut self) {
        with_fast(|f| f.len -= 1);
    }
}

/// Hızlı yol: yığın taşmasını denetler (eski `stack_check`'in işi) ve metodun çerçevesini iter.
#[inline(always)]
pub fn enter_frame_s(info: &'static FrameInfo) -> Res<FastGuard> {
    let marker = 0u8;
    let sp = std::ptr::addr_of!(marker) as usize;
    let over = with_fast(|f| unsafe {
        if f.low == 0 {
            fast_low_init(f);
        }
        if sp < f.low && !f.in_overflow {
            return true;
        }
        if f.len == f.cap {
            fast_grow(f);
        }
        *f.p.add(f.len) = FastSlot { info, line: -1 };
        f.len += 1;
        false
    });
    if over {
        return fast_overflow();
    }
    Ok(FastGuard)
}

/// Hızlı yol: mevcut çerçevenin kaynak satırını işaretler.
#[inline(always)]
pub fn set_line_s(line: i32) {
    with_fast(|f| unsafe {
        if f.len > 0 {
            (*f.p.add(f.len - 1)).line = line;
        }
    });
}

/// Bir Java çerçevesi: (sınıf iç adı, metot, tanımlayıcı, kaynak dosya, satır).
pub type FrameRow = (&'static str, &'static str, &'static str, &'static str, i32);

/// Hızlı yol kullanılıyorsa onun çerçeveleri (en içten başlayarak), yoksa None.
fn fast_frames() -> Option<Vec<FrameRow>> {
    with_fast(|f| unsafe {
        let f = &*f;
        if f.len == 0 {
            return None;
        }
        Some(
            (0..f.len)
                .rev()
                .map(|i| {
                    let s = *f.p.add(i);
                    let fi = &*s.info;
                    (fi.class, fi.method, fi.desc, fi.file, s.line)
                })
                .collect(),
        )
    })
}

/// Bu iş parçacığındaki her çerçeve, en içten başlayarak: (sınıf iç adı, metot, tanımlayıcı, kaynak dosya, satır).
pub fn frames_snapshot() -> Vec<FrameRow> {
    // C9 kipi (MONO_FRAMES=dwarf ile üretilmiş program): çerçeveler makine yığınından çıkarılır (jtrace.rs)
    if let Some(v) = crate::jtrace::frames() {
        return v;
    }
    if let Some(v) = fast_frames() {
        return v;
    }
    FRAMES.with(|f| f.borrow().iter().rev().map(|t| (t.class, t.method, t.desc, t.file, t.line)).collect())
}

/// Çerçeve başına bir satır, en içten başlayarak: sınıf (noktalı), metot, kaynak dosya (boş olabilir), satır (bilinmiyorsa -1),
/// sekmelerle ayrılmış. Kurucu çerçeveleri ve en üstteki `fillInStackTrace` atlanır. HotSpot da böyle yapar.
pub fn capture_backtrace() -> String {
    // iki yol (eski ve hızlı) aynı listeyi verir: frames_snapshot en içten başlayarak döndürür
    let frames = frames_snapshot();
    let mut out = String::new();
    for (class, method, _desc, file, line) in frames
        .into_iter()
        // Gizli lambda vekil sınıfları (`Owner$$Lambda$N`) HotSpot izlerinde görünmez.
        .filter(|t| !t.0.contains("$$Lambda$"))
        // Her Throwable alt sınıfı fillInStackTrace'i geçersiz kılabilir. JDK'nın NullPointerException'ı da böyle yapar.
        .skip_while(|t| t.1 == "<init>" || t.1 == "fillInStackTrace")
    {
        out.push_str(&format!("{}\t{}\t{}\t{}\n", class.replace('/', "."), method, file, line));
    }
    out.trim_end_matches('\n').to_string()
}
