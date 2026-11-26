//! Wayland + EGL masaüstü yüzeyi (Linux): pencere, OpenGL 3.3 core bağlamı ve klavye/fare girdisi.
//! x11.rs ile aynı sözleşme (bkz. mod.rs). Hiçbir kütüphane bağlanmaz, hepsi çalışma zamanında açılır:
//!   libwayland-client.so.0   Wayland'ın konuşma kütüphanesi (zorunlu)
//!   libwayland-egl.so.1      Wayland penceresini EGL'ye bağlar (zorunlu)
//!   libEGL.so.1              OpenGL bağlamını açar (zorunlu)
//!   libxkbcommon.so.0        tuş kodunu harfe çevirir: Türkçe ş, ı, ğ gibi (yoksa yalnızca ABD düzeni)
//!   libwayland-cursor.so.0   imleç resmi (yoksa imleç resmi ayarlanmaz)
//!
//! Salağa anlatır gibi, Wayland nasıl çalışır:
//!  * Pencere yöneticisi ile (buna "compositor" denir) bir soket üzerinden konuşuruz. Her şey bir "nesne"dir
//!    (yüzey, klavye, fare ...). Nesneye "istek" göndeririz, nesne bize "olay" yollar.
//!  * Önce "kayıt defteri"nden (registry) neleri desteklediklerini öğreniriz ve ihtiyacımız olanları bağlarız.
//!  * Pencere için üç nesne gerekir: wl_surface (boyanacak yüzey), xdg_surface ve xdg_toplevel (bu yüzeyi bir pencere
//!    yapar). Pencere yöneticisi "configure" olayıyla boyutu söyler, biz "ack" ile onaylarız. Ondan sonra çizebiliriz.
//!  * Çizim EGL ile yapılır: wl_egl_window bir wl_surface'e OpenGL'in çizebileceği tamponlar verir.
//!  * X11'in aksine Wayland imleci oynatamaz (warp yok). Fare "yakalama" (oyun bakışı) için iki ek protokol vardır:
//!    göreli fare (relative-pointer: imleç değil hareket miktarı gelir) ve imleç kilidi (pointer-constraints).
//!  * Tuş tekrarı da yoktur: tuşa basılı tutulunca tekrarı biz üretiriz (repeat_info: hız ve gecikme).
//!
//! Sınırlar (bilinçli): ölçek 1 (yüksek çözünürlüklü ekranda pencere yöneticisi büyütür), GNOME gibi sunucu tarafı
//! başlık çubuğu vermeyen ortamlarda pencere çerçevesiz açılır, ölü tuşlar (compose) desteklenmez.
//! Pencere kapatma isteği SIGTERM'e çevrilir: oyun bunu zaten temiz kapanış (dünya kaydı) olarak işler.
use super::*;
use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

// ----------------------------------------------------------------------------------------------------- durum

static INIT_OK: AtomicBool = AtomicBool::new(false);
static SIZE_W: AtomicI32 = AtomicI32::new(1280);
static SIZE_H: AtomicI32 = AtomicI32::new(720);
/// Pencere yöneticisinin son önerdiği boyut (configure olayı); onaylanınca uygulanır.
static PEND_W: AtomicI32 = AtomicI32::new(0);
static PEND_H: AtomicI32 = AtomicI32::new(0);
static CONFIGURED: AtomicBool = AtomicBool::new(false);
static GRABBED: AtomicBool = AtomicBool::new(false);

// Wayland nesneleri (adresleri, 0 = yok)
static DISPLAY: AtomicUsize = AtomicUsize::new(0);
static REGISTRY: AtomicUsize = AtomicUsize::new(0);
static COMPOSITOR: AtomicUsize = AtomicUsize::new(0);
static SHM: AtomicUsize = AtomicUsize::new(0);
static SEAT: AtomicUsize = AtomicUsize::new(0);
static POINTER: AtomicUsize = AtomicUsize::new(0);
static KEYBOARD: AtomicUsize = AtomicUsize::new(0);
static XDG_BASE: AtomicUsize = AtomicUsize::new(0);
static SURFACE: AtomicUsize = AtomicUsize::new(0);
static XDG_SURF: AtomicUsize = AtomicUsize::new(0);
static TOPLEVEL: AtomicUsize = AtomicUsize::new(0);
static REL_MGR: AtomicUsize = AtomicUsize::new(0);
static REL_PTR: AtomicUsize = AtomicUsize::new(0);
static CONSTRAINTS: AtomicUsize = AtomicUsize::new(0);
static LOCKED: AtomicUsize = AtomicUsize::new(0);
static DECO_MGR: AtomicUsize = AtomicUsize::new(0);
// imleç
static CURSOR_SURF: AtomicUsize = AtomicUsize::new(0);
static CURSOR_HX: AtomicI32 = AtomicI32::new(0);
static CURSOR_HY: AtomicI32 = AtomicI32::new(0);
static ENTER_SERIAL: AtomicU32 = AtomicU32::new(0);
/// Son uygulanan imleç durumu: 0 bilinmiyor, 1 görünür, 2 gizli.
static CURSOR_STATE: AtomicU32 = AtomicU32::new(0);
// EGL
static EGL_DPY: AtomicUsize = AtomicUsize::new(0);
static EGL_CTX: AtomicUsize = AtomicUsize::new(0);
static EGL_SURF: AtomicUsize = AtomicUsize::new(0);
static EGL_WIN: AtomicUsize = AtomicUsize::new(0);
// xkbcommon
static XKB_CTX: AtomicUsize = AtomicUsize::new(0);
static XKB_MAP: AtomicUsize = AtomicUsize::new(0);
static XKB_STATE: AtomicUsize = AtomicUsize::new(0);

/// MONO_WINDOW_SIZE=WxH, ilk pencere boyutunu geçersiz kılar (x11.rs ile aynı).
fn window_size(w: i32, h: i32) -> (i32, i32) {
    if let Ok(v) = std::env::var("MONO_WINDOW_SIZE") {
        if let Some((a, b)) = v.split_once('x') {
            if let (Ok(a), Ok(b)) = (a.trim().parse::<i32>(), b.trim().parse::<i32>()) {
                if a > 0 && b > 0 { return (a, b); }
            }
        }
    }
    (w, h)
}

pub fn size() -> (i32, i32) {
    (SIZE_W.load(Ordering::Acquire), SIZE_H.load(Ordering::Acquire))
}

pub fn is_ready() -> bool {
    INIT_OK.load(Ordering::Acquire)
}

/// Fare yakalama isteğini kaydeder. Asıl kilitleme `pump` içinde, olay işleyen iş parçacığında yapılır:
/// Wayland istekleri her iş parçacığından gidebilir ama durumu tek yerden yönetmek hatayı azaltır.
pub fn set_grab(on: bool) {
    GRABBED.store(on, Ordering::Release);
    let mut st = INPUT.lock().unwrap();
    st.dx = 0.0;
    st.dy = 0.0;
}

// ------------------------------------------------------------------------------------- girdi durumu

/// Masaüstü girdi durumu; Wayland olaylarından, TouchCore anlık görüntüsünün düzeninde doldurulur (x11.rs ile aynı).
struct Input {
    keys: [u8; 19],
    slot: i32,
    cx: f32,
    cy: f32,
    dx: f32,
    dy: f32,
    text: std::collections::VecDeque<[i32; 3]>,
    /// Tuş tekrarı: basılı tutulan tuşun xkb kodu (0 = yok), bir sonraki tekrar zamanı, tekrar aralığı.
    rep_key: u32,
    rep_next: Option<Instant>,
    rep_gap: Duration,
    rep_delay: Duration,
}
static INPUT: Mutex<Input> = Mutex::new(Input {
    keys: [0; 19], slot: 0, cx: 0.0, cy: 0.0, dx: 0.0, dy: 0.0, text: std::collections::VecDeque::new(),
    rep_key: 0, rep_next: None, rep_gap: Duration::from_millis(33), rep_delay: Duration::from_millis(500),
});

pub fn poll(snap: &mut TcPollSnapshot) -> bool {
    if !is_ready() { return false; }
    pump();
    let (w, h) = size();
    let mut st = INPUT.lock().unwrap();
    snap.width = w;
    snap.height = h;
    snap.keys = st.keys;
    snap.keys[18] = (!GRABBED.load(Ordering::Acquire)) as u8; // fare yakalı değilken imleç kipi
    snap.hotbar_slot = st.slot;
    snap.look_dx = st.dx;
    snap.look_dy = st.dy;
    st.dx = 0.0;
    st.dy = 0.0;
    snap.cursor_x = if w > 0 { st.cx / w as f32 } else { 0.0 };
    snap.cursor_y = if h > 0 { st.cy / h as f32 } else { 0.0 };
    true
}

pub fn take_text(out: &mut [i32], max: usize) -> i32 {
    let mut st = INPUT.lock().unwrap();
    let mut n = 0;
    while n < max {
        let Some(t) = st.text.pop_front() else { break };
        out[3 * n..3 * n + 3].copy_from_slice(&t);
        n += 1;
    }
    n as i32
}

// ------------------------------------------------------------------------- C kütüphane işlevleri (libc)

#[repr(C)]
struct PollFd {
    fd: c_int,
    events: i16,
    revents: i16,
}

extern "C" {
    // sözleşmedeki `poll` (girdi anlık görüntüsü) ile karışmasın diye C'nin poll(2) işlevi `c_poll` adıyla alınır
    #[link_name = "poll"]
    fn c_poll(fds: *mut PollFd, n: u64, timeout: c_int) -> c_int;
    // region.rs ile aynı imza: aynı C işlevinin farklı imzalarla bildirilmesi uyarı verir (-1 adresi hata demektir)
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, off: i64) -> *mut u8;
    fn munmap(addr: *mut c_void, len: usize) -> c_int;
    fn close(fd: c_int) -> c_int;
    fn kill(pid: c_int, sig: c_int) -> c_int;
}

/// MONO_WAYLAND_DEBUG=1: olay günlüğü (stderr). Girdi gelmiyorsa nedenini görmek için.
fn debug_on() -> bool {
    static D: OnceLock<bool> = OnceLock::new();
    *D.get_or_init(|| std::env::var_os("MONO_WAYLAND_DEBUG").is_some_and(|v| !v.is_empty() && v != "0"))
}
macro_rules! dlog {
    ($($a:tt)*) => { if debug_on() { eprintln!("[mono:wayland] {}", format!($($a)*)); } };
}

/// Pencere kapatma isteği ve bağlantı kaybı: SIGTERM. Oyun SIGTERM'i temiz kapanış olarak işler (dünya kaydedilir).
fn request_exit(why: &str) {
    eprintln!("[mono:wayland] {why}: SIGTERM ile kapatiliyor");
    unsafe { kill(std::process::id() as c_int, 15) };
}

// -------------------------------------------------------------------- Wayland arayüz tanımları (protokol)

/// libwayland'ın bir mesajı (istek ya da olay): ad, imza (argüman türleri) ve nesne türleri.
#[repr(C)]
struct WlMessage {
    name: *const c_char,
    signature: *const c_char,
    types: *const *const WlInterface,
}

/// libwayland'ın bir arayüzü: ad, sürüm, istekler ve olaylar.
#[repr(C)]
struct WlInterface {
    name: *const c_char,
    version: c_int,
    method_count: c_int,
    methods: *const WlMessage,
    event_count: c_int,
    events: *const WlMessage,
}

/// wl_array: olaylarla gelen bayt dizisi (xdg_toplevel durumları, tuşlar).
#[repr(C)]
struct WlArray {
    size: usize,
    alloc: usize,
    data: *mut c_void,
}

struct Ifaces {
    xdg_wm_base: *const WlInterface,
    xdg_surface: *const WlInterface,
    xdg_toplevel: *const WlInterface,
    rel_mgr: *const WlInterface,
    rel_ptr: *const WlInterface,
    constraints: *const WlInterface,
    locked: *const WlInterface,
    deco_mgr: *const WlInterface,
    deco: *const WlInterface,
}
unsafe impl Send for Ifaces {}
unsafe impl Sync for Ifaces {}

fn leak_c(s: &str) -> *const c_char {
    CString::new(s).unwrap().into_raw()
}

/// Çalışma zamanında bir arayüz tanımı kurar. Çekirdek arayüzler (wl_surface gibi) libwayland-client'ın içinde hazırdır,
/// ama xdg-shell ve eklenti protokollerinin tanımları uygulamaya aittir; onları burada elle yazıyoruz.
/// İmza harfleri: i=tamsayı u=işaretsiz s=metin o=nesne n=yeni nesne a=dizi f=kesirli h=dosya, ?=boş olabilir,
/// başındaki rakam "bu sürümden beri". Nesne türü dizileri boş (null) bırakılır: libwayland bunları yalnızca
/// gelen olaydaki nesneleri doğrulamak için kullanır ve bizim olaylarımızda nesne argümanı yoktur.
fn mk(name: &str, version: i32, methods: &[(&str, &str)], events: &[(&str, &str)]) -> *const WlInterface {
    fn msgs(list: &[(&str, &str)]) -> *const WlMessage {
        let v: Vec<WlMessage> = list
            .iter()
            .map(|(n, s)| {
                let argc = s.chars().filter(|c| c.is_ascii_alphabetic()).count();
                let types: &'static mut [*const WlInterface] = Box::leak(vec![std::ptr::null(); argc].into_boxed_slice());
                WlMessage { name: leak_c(n), signature: leak_c(s), types: types.as_ptr() }
            })
            .collect();
        Box::leak(v.into_boxed_slice()).as_ptr()
    }
    let i = Box::new(WlInterface {
        name: leak_c(name),
        version,
        method_count: methods.len() as c_int,
        methods: msgs(methods),
        event_count: events.len() as c_int,
        events: msgs(events),
    });
    Box::leak(i) as *const WlInterface
}

fn ifaces() -> &'static Ifaces {
    static I: OnceLock<Ifaces> = OnceLock::new();
    I.get_or_init(|| Ifaces {
        // xdg-shell (kararlı): pencere yaşam döngüsü
        xdg_wm_base: mk("xdg_wm_base", 1,
            &[("destroy", ""), ("create_positioner", "n"), ("get_xdg_surface", "no"), ("pong", "u")],
            &[("ping", "u")]),
        xdg_surface: mk("xdg_surface", 1,
            &[("destroy", ""), ("get_toplevel", "n"), ("get_popup", "n?oo"), ("set_window_geometry", "iiii"), ("ack_configure", "u")],
            &[("configure", "u")]),
        xdg_toplevel: mk("xdg_toplevel", 1,
            &[("destroy", ""), ("set_parent", "?o"), ("set_title", "s"), ("set_app_id", "s"), ("show_window_menu", "ouii"),
              ("move", "ou"), ("resize", "ouu"), ("set_max_size", "ii"), ("set_min_size", "ii"), ("set_maximized", ""),
              ("unset_maximized", ""), ("set_fullscreen", "?o"), ("unset_fullscreen", ""), ("set_minimized", "")],
            &[("configure", "iia"), ("close", ""), ("configure_bounds", "4ii"), ("wm_capabilities", "5a")]),
        // göreli fare: kilitliyken imleç yerine hareket miktarı gelir
        rel_mgr: mk("zwp_relative_pointer_manager_v1", 1,
            &[("destroy", ""), ("get_relative_pointer", "no")], &[]),
        rel_ptr: mk("zwp_relative_pointer_v1", 1,
            &[("destroy", "")], &[("relative_motion", "uuffff")]),
        // imleç kilidi: imleç pencereden çıkmasın
        constraints: mk("zwp_pointer_constraints_v1", 1,
            &[("destroy", ""), ("lock_pointer", "noo?ou"), ("confine_pointer", "noo?ou")], &[]),
        locked: mk("zwp_locked_pointer_v1", 1,
            &[("destroy", ""), ("set_cursor_position_hint", "ff"), ("set_region", "?o")],
            &[("locked", ""), ("unlocked", "")]),
        // sunucu tarafı başlık çubuğu isteği (KDE, wlroots'ta var; GNOME'da yok)
        deco_mgr: mk("zxdg_decoration_manager_v1", 1,
            &[("destroy", ""), ("get_toplevel_decoration", "no")], &[]),
        deco: mk("zxdg_toplevel_decoration_v1", 1,
            &[("destroy", ""), ("set_mode", "u"), ("unset_mode", "")], &[("configure", "u")]),
    })
}

// --------------------------------------------------------------------------- kütüphane işlev tabloları

/// libwayland'ın değişken argümanlı istek gönderme işlevleri (C'de `...`).
type MarshalFlags = unsafe extern "C" fn(*mut c_void, u32, *const WlInterface, u32, u32, ...) -> *mut c_void;
type MarshalPlain = unsafe extern "C" fn(*mut c_void, u32, ...);
type MarshalCtor = unsafe extern "C" fn(*mut c_void, u32, *const WlInterface, ...) -> *mut c_void;
type MarshalCtorVer = unsafe extern "C" fn(*mut c_void, u32, *const WlInterface, u32, ...) -> *mut c_void;

struct Wl {
    lib: *mut c_void,
    connect: extern "C" fn(*const c_char) -> *mut c_void,
    disconnect: extern "C" fn(*mut c_void),
    roundtrip: extern "C" fn(*mut c_void) -> c_int,
    dispatch_pending: extern "C" fn(*mut c_void) -> c_int,
    flush: extern "C" fn(*mut c_void) -> c_int,
    prepare_read: extern "C" fn(*mut c_void) -> c_int,
    read_events: extern "C" fn(*mut c_void) -> c_int,
    cancel_read: extern "C" fn(*mut c_void),
    get_fd: extern "C" fn(*mut c_void) -> c_int,
    add_listener: extern "C" fn(*mut c_void, *const c_void, *mut c_void) -> c_int,
    get_version: extern "C" fn(*mut c_void) -> u32,
    proxy_destroy: extern "C" fn(*mut c_void),
    /// Yeni sürüm (libwayland 1.20+). Yoksa eski üç işlevden biri kullanılır.
    flags: Option<MarshalFlags>,
    plain: Option<MarshalPlain>,
    ctor: Option<MarshalCtor>,
    ctor_ver: Option<MarshalCtorVer>,
}
unsafe impl Send for Wl {}
unsafe impl Sync for Wl {}

static WL: OnceLock<Option<Wl>> = OnceLock::new();

fn wl() -> Option<&'static Wl> {
    WL.get_or_init(|| {
        let lib = dl_open_any(&["libwayland-client.so.0", "libwayland-client.so"]);
        if lib.is_null() { return None; }
        let l = [lib];
        // Eski istek işlevleri yalnızca yeni olan yoksa kullanılır. MONO_WAYLAND_LEGACY=1 eski yolu zorlar (deneme için).
        let legacy = std::env::var_os("MONO_WAYLAND_LEGACY").is_some_and(|v| !v.is_empty() && v != "0");
        Some(Wl {
            lib,
            connect: sym_fn!(&l, "wl_display_connect", extern "C" fn(*const c_char) -> *mut c_void)?,
            disconnect: sym_fn!(&l, "wl_display_disconnect", extern "C" fn(*mut c_void))?,
            roundtrip: sym_fn!(&l, "wl_display_roundtrip", extern "C" fn(*mut c_void) -> c_int)?,
            dispatch_pending: sym_fn!(&l, "wl_display_dispatch_pending", extern "C" fn(*mut c_void) -> c_int)?,
            flush: sym_fn!(&l, "wl_display_flush", extern "C" fn(*mut c_void) -> c_int)?,
            prepare_read: sym_fn!(&l, "wl_display_prepare_read", extern "C" fn(*mut c_void) -> c_int)?,
            read_events: sym_fn!(&l, "wl_display_read_events", extern "C" fn(*mut c_void) -> c_int)?,
            cancel_read: sym_fn!(&l, "wl_display_cancel_read", extern "C" fn(*mut c_void))?,
            get_fd: sym_fn!(&l, "wl_display_get_fd", extern "C" fn(*mut c_void) -> c_int)?,
            add_listener: sym_fn!(&l, "wl_proxy_add_listener", extern "C" fn(*mut c_void, *const c_void, *mut c_void) -> c_int)?,
            get_version: sym_fn!(&l, "wl_proxy_get_version", extern "C" fn(*mut c_void) -> u32)?,
            proxy_destroy: sym_fn!(&l, "wl_proxy_destroy", extern "C" fn(*mut c_void))?,
            flags: if legacy { None } else { sym_fn!(&l, "wl_proxy_marshal_flags", MarshalFlags) },
            plain: sym_fn!(&l, "wl_proxy_marshal", MarshalPlain),
            ctor: sym_fn!(&l, "wl_proxy_marshal_constructor", MarshalCtor),
            ctor_ver: sym_fn!(&l, "wl_proxy_marshal_constructor_versioned", MarshalCtorVer),
        })
    })
    .as_ref()
}

/// Çekirdek arayüz (libwayland-client'ın içindeki veri sembolü): ör. "wl_surface_interface".
fn core_iface(name: &'static str) -> *const WlInterface {
    let Some(w) = wl() else { return std::ptr::null() };
    let Ok(c) = CString::new(name) else { return std::ptr::null() };
    unsafe { loader::sym(w.lib, c.as_ptr()) as *const WlInterface }
}

/// Yeni nesne üretmeyen bir istek gönderir. `destroy` doğruysa istek nesneyi yok eden (destructor) istektir.
/// Eski libwayland'da (flags yok) aynı iş `wl_proxy_marshal` + `wl_proxy_destroy` ile yapılır.
macro_rules! send {
    ($w:expr, $p:expr, $op:expr, $destroy:expr $(, $a:expr)*) => {{
        let w: &Wl = $w;
        let p: *mut c_void = $p as *mut c_void;
        if let Some(m) = w.flags {
            unsafe { m(p, $op, std::ptr::null(), (w.get_version)(p), $destroy as u32 $(, $a)*); }
        } else if let Some(m) = w.plain {
            unsafe { m(p, $op $(, $a)*); }
            if $destroy { (w.proxy_destroy)(p); }
        }
    }};
}

/// Yeni bir nesne üreten istek gönderir (`n` argümanı için null verilir); yeni nesnenin adresini döndürür.
/// `$ver`: yeni nesnenin sürümü.
macro_rules! send_new {
    ($w:expr, $p:expr, $op:expr, $iface:expr, $ver:expr $(, $a:expr)*) => {{
        let w: &Wl = $w;
        let p: *mut c_void = $p as *mut c_void;
        let mut out: *mut c_void = std::ptr::null_mut();
        if let Some(m) = w.flags {
            out = unsafe { m(p, $op, $iface, $ver, 0u32 $(, $a)*) };
        } else if let Some(m) = w.ctor_ver {
            out = unsafe { m(p, $op, $iface, $ver $(, $a)*) };
        } else if let Some(m) = w.ctor {
            out = unsafe { m(p, $op, $iface $(, $a)*) };
        }
        out
    }};
}

const NIL: *mut c_void = std::ptr::null_mut();

// -------------------------------------------------------------------------- olay dinleyicileri (callback)

/// Kullanmadığımız olaylar için boş işlev: C çağıran argümanları kendisi temizler, argümansız bir işlev güvenle çağrılır.
extern "C" fn noop() {}

/// Dinleyici tablosu: olayların sırasıyla işlev adresleri.
#[repr(transparent)]
struct Listener<const N: usize>([*const (); N]);
unsafe impl<const N: usize> Sync for Listener<N> {}

fn fixed(v: i32) -> f32 {
    v as f32 / 256.0 // wl_fixed_t: 24.8 sabit noktalı sayı
}

// wl_registry: sunucunun desteklediği nesneler
extern "C" fn reg_global(_d: *mut c_void, reg: *mut c_void, name: u32, iface: *const c_char, ver: u32) {
    let Some(w) = wl() else { return };
    let iname = unsafe { std::ffi::CStr::from_ptr(iface) }.to_string_lossy().into_owned();
    let i = ifaces();
    // (hedef, arayüz, istenen en yüksek sürüm)
    let (slot, iface_ptr, want): (&AtomicUsize, *const WlInterface, u32) = match iname.as_str() {
        "wl_compositor" => (&COMPOSITOR, core_iface("wl_compositor_interface"), 4),
        "wl_shm" => (&SHM, core_iface("wl_shm_interface"), 1),
        "wl_seat" => (&SEAT, core_iface("wl_seat_interface"), 5),
        "xdg_wm_base" => (&XDG_BASE, i.xdg_wm_base, 1),
        "zwp_relative_pointer_manager_v1" => (&REL_MGR, i.rel_mgr, 1),
        "zwp_pointer_constraints_v1" => (&CONSTRAINTS, i.constraints, 1),
        "zxdg_decoration_manager_v1" => (&DECO_MGR, i.deco_mgr, 1),
        _ => return,
    };
    if iface_ptr.is_null() || slot.load(Ordering::Acquire) != 0 { return; }
    let v = ver.min(want);
    let iface_name = unsafe { (*iface_ptr).name };
    let obj = send_new!(w, reg, 0, iface_ptr, v, name, iface_name, v, NIL);
    if obj.is_null() { return; }
    slot.store(obj as usize, Ordering::Release);
    if iname == "wl_seat" {
        static SEAT_L: Listener<2> = Listener([seat_caps as *const (), noop as *const ()]);
        (w.add_listener)(obj, SEAT_L.0.as_ptr() as *const c_void, NIL);
    } else if iname == "xdg_wm_base" {
        static BASE_L: Listener<1> = Listener([base_ping as *const ()]);
        (w.add_listener)(obj, BASE_L.0.as_ptr() as *const c_void, NIL);
    }
}

// xdg_wm_base: pencere yöneticisi "yaşıyor musun?" diye sorar, "pong" ile cevap veririz (yoksa pencere donmuş sayılır)
extern "C" fn base_ping(_d: *mut c_void, base: *mut c_void, serial: u32) {
    let Some(w) = wl() else { return };
    send!(w, base, 3, false, serial);
}

// wl_seat: bu koltukta fare ve klavye var mı? Yetenekler sonradan değişebilir (USB fare takılıp sökülünce):
// eklenen aygıt için nesne alınır, çıkarılan aygıtın nesnesi bırakılır (yoksa aygıt geri gelince olay almayız).
extern "C" fn seat_caps(_d: *mut c_void, seat: *mut c_void, caps: u32) {
    let Some(w) = wl() else { return };
    dlog!("wl_seat yetenekleri: {caps:#b} (1=fare, 2=klavye)");
    let ver = (w.get_version)(seat);
    // wl_pointer.release (istek 1) ve wl_keyboard.release (istek 0) sürüm 3'ten beri var; eskisinde yalnızca nesne atılır
    let release = |obj: usize, op: u32| {
        if ver >= 3 { send!(w, obj, op, true); } else { (w.proxy_destroy)(obj as *mut c_void); }
    };
    if caps & 1 == 0 && POINTER.load(Ordering::Acquire) != 0 {
        // fareye bağlı nesneler de geçersiz olur: kilit ve göreli fare önce yok edilir
        let (lk, rp) = (LOCKED.swap(0, Ordering::AcqRel), REL_PTR.swap(0, Ordering::AcqRel));
        if lk != 0 { send!(w, lk, 0, true); }
        if rp != 0 { send!(w, rp, 0, true); }
        release(POINTER.swap(0, Ordering::AcqRel), 1);
        CURSOR_STATE.store(0, Ordering::Release);
        ENTER_SERIAL.store(0, Ordering::Release);
    }
    if caps & 2 == 0 && KEYBOARD.load(Ordering::Acquire) != 0 {
        release(KEYBOARD.swap(0, Ordering::AcqRel), 0);
        let mut st = INPUT.lock().unwrap();
        st.keys = [0; 19];
        st.rep_key = 0;
        st.rep_next = None;
    }
    if caps & 1 != 0 && POINTER.load(Ordering::Acquire) == 0 {
        let p = send_new!(w, seat, 0, core_iface("wl_pointer_interface"), (w.get_version)(seat), NIL);
        if !p.is_null() {
            // enter, leave, motion, button, axis, frame, axis_source, axis_stop, axis_discrete, axis_value120, axis_relative_direction
            static PTR_L: Listener<11> = Listener([
                ptr_enter as *const (), ptr_leave as *const (), ptr_motion as *const (), ptr_button as *const (),
                noop as *const (), noop as *const (), noop as *const (), noop as *const (), noop as *const (),
                noop as *const (), noop as *const (),
            ]);
            (w.add_listener)(p, PTR_L.0.as_ptr() as *const c_void, NIL);
            POINTER.store(p as usize, Ordering::Release);
        }
    }
    if caps & 2 != 0 && KEYBOARD.load(Ordering::Acquire) == 0 {
        let k = send_new!(w, seat, 1, core_iface("wl_keyboard_interface"), (w.get_version)(seat), NIL);
        if !k.is_null() {
            // keymap, enter, leave, key, modifiers, repeat_info
            static KB_L: Listener<6> = Listener([
                kb_keymap as *const (), kb_enter as *const (), kb_leave as *const (), kb_key as *const (),
                kb_mods as *const (), kb_repeat as *const (),
            ]);
            (w.add_listener)(k, KB_L.0.as_ptr() as *const c_void, NIL);
            KEYBOARD.store(k as usize, Ordering::Release);
        }
    }
}

// ----- fare

extern "C" fn ptr_enter(_d: *mut c_void, _p: *mut c_void, serial: u32, _s: *mut c_void, sx: i32, sy: i32) {
    dlog!("fare pencereye girdi ({}, {})", fixed(sx), fixed(sy));
    ENTER_SERIAL.store(serial, Ordering::Release);
    CURSOR_STATE.store(0, Ordering::Release); // her girişte imleç resmi yeniden ayarlanmalı
    let mut st = INPUT.lock().unwrap();
    st.cx = fixed(sx);
    st.cy = fixed(sy);
}

extern "C" fn ptr_leave(_d: *mut c_void, _p: *mut c_void, _serial: u32, _s: *mut c_void) {
    dlog!("fare pencereden cikti");
}

extern "C" fn ptr_motion(_d: *mut c_void, _p: *mut c_void, _time: u32, sx: i32, sy: i32) {
    let (x, y) = (fixed(sx), fixed(sy));
    let mut st = INPUT.lock().unwrap();
    // Yakalıyken hareket göreli fareden gelir. Göreli fare yoksa (eski pencere yöneticisi) mutlak hareketin farkı
    // kullanılır: imleç pencere kenarına varınca durur, ama hiç değilse bakış çalışır.
    if GRABBED.load(Ordering::Acquire) && REL_PTR.load(Ordering::Acquire) == 0 {
        st.dx += x - st.cx;
        st.dy += y - st.cy;
    }
    if !GRABBED.load(Ordering::Acquire) || REL_PTR.load(Ordering::Acquire) == 0 {
        st.cx = x;
        st.cy = y;
    }
}

extern "C" fn ptr_button(_d: *mut c_void, _p: *mut c_void, _serial: u32, _time: u32, button: u32, state: u32) {
    // Linux düğme kodları: BTN_LEFT 0x110, BTN_RIGHT 0x111 (x11.rs gibi yalnızca sol ve sağ)
    let down = (state == 1) as u8;
    let mut st = INPUT.lock().unwrap();
    match button {
        0x110 => st.keys[7] = down,
        0x111 => st.keys[8] = down,
        _ => {}
    }
}

// zwp_locked_pointer_v1: kilit gerçekten devreye girdi mi? (pencere odakta değilken kilit beklemede kalır)
extern "C" fn lock_locked(_d: *mut c_void, _l: *mut c_void) {
    dlog!("imlec kilitlendi");
}
extern "C" fn lock_unlocked(_d: *mut c_void, _l: *mut c_void) {
    dlog!("imlec kilidi kalkti (pencere odagi gitti ya da sunucu birakti)");
}

extern "C" fn rel_motion(_d: *mut c_void, _r: *mut c_void, _hi: u32, _lo: u32, dx: i32, dy: i32, _udx: i32, _udy: i32) {
    if GRABBED.load(Ordering::Acquire) {
        let mut st = INPUT.lock().unwrap();
        st.dx += fixed(dx);
        st.dy += fixed(dy);
    }
}

// ----- klavye

struct Xkb {
    new_state: extern "C" fn(*mut c_void) -> *mut c_void,
    unref_state: extern "C" fn(*mut c_void),
    unref_map: extern "C" fn(*mut c_void),
    map_from_string: extern "C" fn(*mut c_void, *const c_char, u32, u32) -> *mut c_void,
    ctx_new: extern "C" fn(u32) -> *mut c_void,
    update_mask: extern "C" fn(*mut c_void, u32, u32, u32, u32, u32, u32) -> u32,
    one_sym: extern "C" fn(*mut c_void, u32) -> u32,
    utf8: extern "C" fn(*mut c_void, u32, *mut c_char, usize) -> c_int,
    repeats: extern "C" fn(*mut c_void, u32) -> c_int,
}
unsafe impl Send for Xkb {}
unsafe impl Sync for Xkb {}

static XKB: OnceLock<Option<Xkb>> = OnceLock::new();

fn xkb() -> Option<&'static Xkb> {
    XKB.get_or_init(|| {
        let lib = dl_open_any(&["libxkbcommon.so.0", "libxkbcommon.so"]);
        if lib.is_null() { return None; }
        let l = [lib];
        Some(Xkb {
            new_state: sym_fn!(&l, "xkb_state_new", extern "C" fn(*mut c_void) -> *mut c_void)?,
            unref_state: sym_fn!(&l, "xkb_state_unref", extern "C" fn(*mut c_void))?,
            unref_map: sym_fn!(&l, "xkb_keymap_unref", extern "C" fn(*mut c_void))?,
            map_from_string: sym_fn!(&l, "xkb_keymap_new_from_string", extern "C" fn(*mut c_void, *const c_char, u32, u32) -> *mut c_void)?,
            ctx_new: sym_fn!(&l, "xkb_context_new", extern "C" fn(u32) -> *mut c_void)?,
            update_mask: sym_fn!(&l, "xkb_state_update_mask", extern "C" fn(*mut c_void, u32, u32, u32, u32, u32, u32) -> u32)?,
            one_sym: sym_fn!(&l, "xkb_state_key_get_one_sym", extern "C" fn(*mut c_void, u32) -> u32)?,
            utf8: sym_fn!(&l, "xkb_state_key_get_utf8", extern "C" fn(*mut c_void, u32, *mut c_char, usize) -> c_int)?,
            repeats: sym_fn!(&l, "xkb_keymap_key_repeats", extern "C" fn(*mut c_void, u32) -> c_int)?,
        })
    })
    .as_ref()
}

// xkbcommon yokken kullanılan basit ABD düzeni: Linux tuş kodu -> keysym (yalnızca oyun tuşları, harf, rakam, düzenleme)
fn fallback_sym(code: u32, shift: bool) -> u32 {
    let letters = "qwertyuiop"; // tuş kodları 16..=25
    let row2 = "asdfghjkl"; // 30..=38
    let row3 = "zxcvbnm"; // 44..=50
    let ch = |c: char| if shift { c.to_ascii_uppercase() as u32 } else { c as u32 };
    match code {
        16..=25 => ch(letters.chars().nth((code - 16) as usize).unwrap()),
        30..=38 => ch(row2.chars().nth((code - 30) as usize).unwrap()),
        44..=50 => ch(row3.chars().nth((code - 44) as usize).unwrap()),
        2..=10 => 0x31 + (code - 2),
        11 => 0x30,
        57 => 0x20,
        42 => 0xffe1, 54 => 0xffe2, 29 => 0xffe3, 97 => 0xffe4,
        1 => 0xff1b, 15 => 0xff09, 14 => 0xff08, 28 => 0xff0d, 96 => 0xff8d, 111 => 0xffff,
        105 => 0xff51, 103 => 0xff52, 106 => 0xff53, 108 => 0xff54, 102 => 0xff50, 107 => 0xff57,
        61 => 0xffc0, 63 => 0xffc2,
        _ => 0,
    }
}

/// Bir tuş olayını girdi durumuna işler. `repeat` doğruysa yalnızca metin/düzenleme olayı üretir (tekrar).
/// x11.rs'in KeyPress/KeyRelease işiyle aynı sonuç: oyun tuşları, hotbar rakamı, metin kuyruğu.
fn key_event(st: &mut Input, kc: u32, down: bool, repeat: bool) {
    let xs = XKB_STATE.load(Ordering::Acquire) as *mut c_void;
    let (ks, text): (u64, Option<String>) = match (xkb(), xs.is_null()) {
        (Some(x), false) => {
            let ks = (x.one_sym)(xs, kc) as u64;
            let mut buf = [0 as c_char; 16];
            let n = (x.utf8)(xs, kc, buf.as_mut_ptr(), buf.len());
            let t = if n > 0 {
                let b: Vec<u8> = buf[..n as usize].iter().map(|&c| c as u8).collect();
                String::from_utf8(b).ok()
            } else { None };
            (ks, t)
        }
        _ => {
            // xkbcommon yok: ABD düzeni (Shift basılıysa büyük harf)
            let shift = st.keys[5] != 0;
            let ks = fallback_sym(kc - 8, shift) as u64;
            let t = if (0x20..0x7f).contains(&ks) { Some((ks as u8 as char).to_string()) } else { None };
            (ks, t)
        }
    };
    if !repeat {
        if let Some(i) = super::x11::key_index(ks) { st.keys[i] = down as u8; }
        if (0x31..=0x39).contains(&ks) && down { st.slot = (ks - 0x31) as i32; }
    }
    if let Some(k) = super::x11::edit_key(ks) {
        if down || !repeat { st.text.push_back([2, k, down as i32]); }
    } else if down {
        if let Some(t) = text {
            for c in t.chars() {
                if !c.is_control() { st.text.push_back([1, c as i32, 0]); }
            }
        }
    }
}

extern "C" fn kb_keymap(_d: *mut c_void, _k: *mut c_void, format: u32, fd: c_int, size: u32) {
    dlog!("klavye haritasi geldi (bicim {format}, {size} bayt)");
    // format 1 = xkb metin biçimi; dosya tanımlayıcısını her durumda kapatmalıyız
    let mut ok = false;
    if format == 1 {
        if let Some(x) = xkb() {
            let map = unsafe { mmap(std::ptr::null_mut(), size as usize, 1, 2, fd, 0) }; // PROT_READ, MAP_PRIVATE
            if map as isize != -1 {
                let mut ctx = XKB_CTX.load(Ordering::Acquire) as *mut c_void;
                if ctx.is_null() {
                    ctx = (x.ctx_new)(0);
                    XKB_CTX.store(ctx as usize, Ordering::Release);
                }
                if !ctx.is_null() {
                    let km = (x.map_from_string)(ctx, map as *const c_char, 1, 0);
                    if !km.is_null() {
                        let state = (x.new_state)(km);
                        let (old_s, old_m) = (XKB_STATE.swap(state as usize, Ordering::AcqRel), XKB_MAP.swap(km as usize, Ordering::AcqRel));
                        if old_s != 0 { (x.unref_state)(old_s as *mut c_void); }
                        if old_m != 0 { (x.unref_map)(old_m as *mut c_void); }
                        ok = !state.is_null();
                    }
                }
                unsafe { munmap(map as *mut c_void, size as usize) };
            }
        }
    }
    if !ok && format == 1 { eprintln!("[mono:wayland] klavye haritasi okunamadi: ABD duzeni kullaniliyor"); }
    unsafe { close(fd) };
}

extern "C" fn kb_enter(_d: *mut c_void, _k: *mut c_void, _serial: u32, _s: *mut c_void, _keys: *mut WlArray) {
    dlog!("klavye odagi pencereye geldi");
}

// Klavye odağı gidince basılı sayılan her tuş bırakılır: yoksa tuş "takılı" kalır (bakış, yürüyüş).
extern "C" fn kb_leave(_d: *mut c_void, _k: *mut c_void, _serial: u32, _s: *mut c_void) {
    dlog!("klavye odagi pencereden gitti");
    let mut st = INPUT.lock().unwrap();
    st.keys = [0; 19];
    st.rep_key = 0;
}

extern "C" fn kb_key(_d: *mut c_void, _k: *mut c_void, _serial: u32, _time: u32, key: u32, state: u32) {
    let kc = key + 8; // xkb kodları Linux kodlarından 8 fazladır
    let down = state != 0;
    dlog!("tus kodu {key} {}", if down { "basildi" } else { "birakildi" });
    let mut st = INPUT.lock().unwrap();
    key_event(&mut st, kc, down, false);
    let xm = XKB_MAP.load(Ordering::Acquire) as *mut c_void;
    let repeats = match (xkb(), xm.is_null()) {
        (Some(x), false) => (x.repeats)(xm, kc) != 0,
        _ => true,
    };
    if down && repeats {
        st.rep_key = kc;
        st.rep_next = Some(Instant::now() + st.rep_delay);
    } else if !down && st.rep_key == kc {
        st.rep_key = 0;
        st.rep_next = None;
    }
}

extern "C" fn kb_mods(_d: *mut c_void, _k: *mut c_void, _serial: u32, dep: u32, lat: u32, lock: u32, group: u32) {
    let xs = XKB_STATE.load(Ordering::Acquire) as *mut c_void;
    if let (Some(x), false) = (xkb(), xs.is_null()) {
        (x.update_mask)(xs, dep, lat, lock, 0, 0, group);
    }
    // xkbcommon yokken Shift durumu key_event'te keys[5]'ten okunur
}

extern "C" fn kb_repeat(_d: *mut c_void, _k: *mut c_void, rate: i32, delay: i32) {
    let mut st = INPUT.lock().unwrap();
    if rate > 0 { st.rep_gap = Duration::from_millis((1000 / rate.max(1)) as u64); }
    if delay >= 0 { st.rep_delay = Duration::from_millis(delay as u64); }
    // rate 0: tekrar kapalı
    if rate == 0 { st.rep_key = 0; }
}

// ----- pencere

extern "C" fn tl_configure(_d: *mut c_void, _t: *mut c_void, w: i32, h: i32, _states: *mut WlArray) {
    if w > 0 && h > 0 {
        PEND_W.store(w, Ordering::Release);
        PEND_H.store(h, Ordering::Release);
    }
}

extern "C" fn tl_close(_d: *mut c_void, _t: *mut c_void) {
    request_exit("pencere kapatma istegi");
}

// xdg_surface.configure: pencere yöneticisi bir durum önerdi; "ack" ile onaylamadan çizim yapılamaz.
extern "C" fn xs_configure(_d: *mut c_void, xs: *mut c_void, serial: u32) {
    let Some(w) = wl() else { return };
    send!(w, xs, 4, false, serial);
    let (nw, nh) = (PEND_W.load(Ordering::Acquire), PEND_H.load(Ordering::Acquire));
    if nw > 0 && nh > 0 {
        SIZE_W.store(nw, Ordering::Release);
        SIZE_H.store(nh, Ordering::Release);
        let ew = EGL_WIN.load(Ordering::Acquire);
        if ew != 0 {
            if let Some(f) = egl_win_fns().resize {
                f(ew as *mut c_void, nw, nh, 0, 0);
            }
        }
    }
    CONFIGURED.store(true, Ordering::Release);
}

// ------------------------------------------------------------------------------------------------- EGL

struct EglFns {
    get_display: extern "C" fn(*mut c_void) -> *mut c_void,
    get_platform_display: Option<extern "C" fn(u32, *mut c_void, *const isize) -> *mut c_void>,
    initialize: extern "C" fn(*mut c_void, *mut i32, *mut i32) -> u32,
    terminate: extern "C" fn(*mut c_void) -> u32,
    bind_api: extern "C" fn(u32) -> u32,
    choose_config: extern "C" fn(*mut c_void, *const i32, *mut *mut c_void, i32, *mut i32) -> u32,
    get_config_attrib: extern "C" fn(*mut c_void, *mut c_void, i32, *mut i32) -> u32,
    create_context: extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *const i32) -> *mut c_void,
    create_window_surface: extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *const i32) -> *mut c_void,
    make_current: extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void) -> u32,
    swap_buffers: extern "C" fn(*mut c_void, *mut c_void) -> u32,
    get_proc_address: extern "C" fn(*const c_char) -> *const c_void,
    get_error: extern "C" fn() -> i32,
}
unsafe impl Send for EglFns {}
unsafe impl Sync for EglFns {}

static EGL: OnceLock<Option<EglFns>> = OnceLock::new();

fn egl() -> Option<&'static EglFns> {
    EGL.get_or_init(|| {
        let lib = dl_open_any(&["libEGL.so.1", "libEGL.so"]);
        if lib.is_null() { return None; }
        let l = [lib];
        Some(EglFns {
            get_display: sym_fn!(&l, "eglGetDisplay", extern "C" fn(*mut c_void) -> *mut c_void)?,
            get_platform_display: sym_fn!(&l, "eglGetPlatformDisplay", extern "C" fn(u32, *mut c_void, *const isize) -> *mut c_void),
            initialize: sym_fn!(&l, "eglInitialize", extern "C" fn(*mut c_void, *mut i32, *mut i32) -> u32)?,
            terminate: sym_fn!(&l, "eglTerminate", extern "C" fn(*mut c_void) -> u32)?,
            bind_api: sym_fn!(&l, "eglBindAPI", extern "C" fn(u32) -> u32)?,
            choose_config: sym_fn!(&l, "eglChooseConfig", extern "C" fn(*mut c_void, *const i32, *mut *mut c_void, i32, *mut i32) -> u32)?,
            get_config_attrib: sym_fn!(&l, "eglGetConfigAttrib", extern "C" fn(*mut c_void, *mut c_void, i32, *mut i32) -> u32)?,
            create_context: sym_fn!(&l, "eglCreateContext", extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *const i32) -> *mut c_void)?,
            create_window_surface: sym_fn!(&l, "eglCreateWindowSurface", extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *const i32) -> *mut c_void)?,
            make_current: sym_fn!(&l, "eglMakeCurrent", extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void) -> u32)?,
            swap_buffers: sym_fn!(&l, "eglSwapBuffers", extern "C" fn(*mut c_void, *mut c_void) -> u32)?,
            get_proc_address: sym_fn!(&l, "eglGetProcAddress", extern "C" fn(*const c_char) -> *const c_void)?,
            get_error: sym_fn!(&l, "eglGetError", extern "C" fn() -> i32)?,
        })
    })
    .as_ref()
}

/// libwayland-egl: Wayland yüzeyine OpenGL'in çizebileceği tampon veren "yerel pencere".
struct EglWinFns {
    create: Option<extern "C" fn(*mut c_void, i32, i32) -> *mut c_void>,
    resize: Option<extern "C" fn(*mut c_void, i32, i32, i32, i32)>,
    destroy: Option<extern "C" fn(*mut c_void)>,
}

fn egl_win_fns() -> &'static EglWinFns {
    static F: OnceLock<EglWinFns> = OnceLock::new();
    F.get_or_init(|| {
        let lib = dl_open_any(&["libwayland-egl.so.1", "libwayland-egl.so"]);
        let l = [lib];
        EglWinFns {
            create: if lib.is_null() { None } else { sym_fn!(&l, "wl_egl_window_create", extern "C" fn(*mut c_void, i32, i32) -> *mut c_void) },
            resize: if lib.is_null() { None } else { sym_fn!(&l, "wl_egl_window_resize", extern "C" fn(*mut c_void, i32, i32, i32, i32)) },
            destroy: if lib.is_null() { None } else { sym_fn!(&l, "wl_egl_window_destroy", extern "C" fn(*mut c_void)) },
        }
    })
}

/// EGL bağlamını kurar: ekran, ayar (config), OpenGL 3.3 core bağlamı, pencere yüzeyi, geçerli yap.
fn init_egl(display: *mut c_void, surface: *mut c_void, w: i32, h: i32) -> Result<(), String> {
    let e = egl().ok_or("libEGL.so.1 acilamadi")?;
    let ew = egl_win_fns();
    let (Some(create), Some(_)) = (ew.create, ew.resize) else { return Err("libwayland-egl.so.1 acilamadi".into()) };
    {
        // Önce Wayland platformunu açıkça isteyen yeni yol (EGL 1.5), yoksa eski eglGetDisplay
        let mut dpy = std::ptr::null_mut();
        if let Some(gp) = e.get_platform_display {
            dpy = gp(0x31D8, display, std::ptr::null()); // EGL_PLATFORM_WAYLAND_KHR
        }
        if dpy.is_null() { dpy = (e.get_display)(display); }
        if dpy.is_null() { return Err("EGL ekrani alinamadi".into()); }
        let (mut maj, mut min) = (0i32, 0i32);
        if (e.initialize)(dpy, &mut maj, &mut min) == 0 { return Err(format!("eglInitialize basarisiz (hata {:#x})", (e.get_error)())); }
        EGL_DPY.store(dpy as usize, Ordering::Release);
        if (e.bind_api)(0x30A2) == 0 { return Err("eglBindAPI(OpenGL) basarisiz".into()); } // EGL_OPENGL_API
        let attrs: [i32; 15] = [
            0x3033, 0x0004, // EGL_SURFACE_TYPE, EGL_WINDOW_BIT
            0x3040, 0x0008, // EGL_RENDERABLE_TYPE, EGL_OPENGL_BIT
            0x3024, 8,      // EGL_RED_SIZE
            0x3023, 8,      // EGL_GREEN_SIZE
            0x3022, 8,      // EGL_BLUE_SIZE
            0x3025, 24,     // EGL_DEPTH_SIZE
            0x3038, 0, 0,   // EGL_NONE
        ];
        let mut cfgs = [std::ptr::null_mut::<c_void>(); 64];
        let mut n = 0i32;
        if (e.choose_config)(dpy, attrs.as_ptr(), cfgs.as_mut_ptr(), 64, &mut n) == 0 || n <= 0 {
            return Err("uygun EGL ayari yok".into());
        }
        // Alfa kanalsız ayar tercih edilir: alfalı tampon pencereyi yarı saydam gösterebilir.
        let mut cfg = cfgs[0];
        for &c in &cfgs[..n as usize] {
            let mut a = 1i32;
            if (e.get_config_attrib)(dpy, c, 0x3021, &mut a) != 0 && a == 0 { cfg = c; break; } // EGL_ALPHA_SIZE
        }
        let core: [i32; 7] = [0x3098, 3, 0x30FB, 3, 0x30FD, 1, 0x3038]; // 3.3, core profil
        let mut ctx = (e.create_context)(dpy, cfg, std::ptr::null_mut(), core.as_ptr());
        if ctx.is_null() {
            let none: [i32; 1] = [0x3038];
            ctx = (e.create_context)(dpy, cfg, std::ptr::null_mut(), none.as_ptr());
        }
        if ctx.is_null() { return Err(format!("OpenGL baglami olusmadi (hata {:#x})", (e.get_error)())); }
        let win = create(surface, w.max(1), h.max(1));
        if win.is_null() { return Err("wl_egl_window olusmadi".into()); }
        EGL_WIN.store(win as usize, Ordering::Release);
        let surf = (e.create_window_surface)(dpy, cfg, win, std::ptr::null());
        if surf.is_null() { return Err(format!("EGL yuzeyi olusmadi (hata {:#x})", (e.get_error)())); }
        if (e.make_current)(dpy, surf, surf, ctx) == 0 { return Err("eglMakeCurrent basarisiz".into()); }
        EGL_CTX.store(ctx as usize, Ordering::Release);
        EGL_SURF.store(surf as usize, Ordering::Release);
    }
    Ok(())
}

// --------------------------------------------------------------------------------- imleç ve fare kilidi

/// Tema imlecini (varsa) ayarlar: libwayland-cursor ile bir imleç resmi yükler ve kendi yüzeyine koyar.
fn load_cursor(w: &Wl) {
    let shm = SHM.load(Ordering::Acquire);
    let comp = COMPOSITOR.load(Ordering::Acquire);
    if shm == 0 || comp == 0 { return; }
    let lib = dl_open_any(&["libwayland-cursor.so.0", "libwayland-cursor.so"]);
    if lib.is_null() { return; }
    let l = [lib];
    let (Some(load), Some(get), Some(buf)) = (
        sym_fn!(&l, "wl_cursor_theme_load", extern "C" fn(*const c_char, c_int, *mut c_void) -> *mut c_void),
        sym_fn!(&l, "wl_cursor_theme_get_cursor", extern "C" fn(*mut c_void, *const c_char) -> *mut c_void),
        sym_fn!(&l, "wl_cursor_image_get_buffer", extern "C" fn(*mut c_void) -> *mut c_void),
    ) else { return };
    let size = std::env::var("XCURSOR_SIZE").ok().and_then(|s| s.parse::<c_int>().ok()).unwrap_or(24);
    let theme_name = std::env::var("XCURSOR_THEME").ok().and_then(|s| CString::new(s).ok());
    let theme = load(theme_name.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()), size, shm as *mut c_void);
    if theme.is_null() { return; }
    // imleç adı: yaygın "left_ptr", yoksa "default"
    let mut cur = get(theme, c"left_ptr".as_ptr());
    if cur.is_null() { cur = get(theme, c"default".as_ptr()); }
    if cur.is_null() { return; }
    // wl_cursor { image_count: u32, images: *mut *mut wl_cursor_image, name: *mut c_char }
    let (count, images) = unsafe { (*(cur as *const u32), *((cur as *const usize).add(1)) as *const *const u32) };
    if count == 0 || images.is_null() { return; }
    let img = unsafe { *images };
    if img.is_null() { return; }
    // wl_cursor_image { width, height, hotspot_x, hotspot_y, delay: u32 }
    let (iw, ih, hx, hy) = unsafe { (*img as i32, *img.add(1) as i32, *img.add(2) as i32, *img.add(3) as i32) };
    let buffer = buf(img as *mut c_void);
    if buffer.is_null() { return; }
    let surf = send_new!(w, comp, 0, core_iface("wl_surface_interface"), (w.get_version)(comp as *mut c_void), NIL);
    if surf.is_null() { return; }
    send!(w, surf, 1, false, buffer, 0i32, 0i32); // attach
    send!(w, surf, 2, false, 0i32, 0i32, iw, ih); // damage
    send!(w, surf, 6, false); // commit
    CURSOR_SURF.store(surf as usize, Ordering::Release);
    CURSOR_HX.store(hx, Ordering::Release);
    CURSOR_HY.store(hy, Ordering::Release);
}

/// İmleci göster (tema resmiyle) ya da gizle. Wayland'da imleç resmini istemci belirler.
fn apply_cursor(w: &Wl, hidden: bool) {
    let ptr = POINTER.load(Ordering::Acquire);
    if ptr == 0 { return; }
    let want = if hidden { 2 } else { 1 };
    if CURSOR_STATE.load(Ordering::Acquire) == want { return; }
    let serial = ENTER_SERIAL.load(Ordering::Acquire);
    if serial == 0 { return; } // fare henüz pencereye girmedi
    let surf = if hidden { 0 } else { CURSOR_SURF.load(Ordering::Acquire) };
    if !hidden && surf == 0 { return; } // imleç resmi yok: pencere yöneticisinin kendi imleci kalır
    send!(w, ptr, 0, false, serial, surf as *mut c_void, CURSOR_HX.load(Ordering::Acquire), CURSOR_HY.load(Ordering::Acquire));
    CURSOR_STATE.store(want, Ordering::Release);
}

/// İstenen yakalama durumunu uygular: imleci kilitler/bırakır ve gizler/gösterir.
fn apply_grab(w: &Wl) {
    let want = GRABBED.load(Ordering::Acquire);
    let locked = LOCKED.load(Ordering::Acquire);
    let (ptr, surf) = (POINTER.load(Ordering::Acquire), SURFACE.load(Ordering::Acquire));
    if want && locked == 0 && ptr != 0 && surf != 0 {
        let cons = CONSTRAINTS.load(Ordering::Acquire);
        if cons != 0 {
            // lock_pointer(yeni nesne, yüzey, fare, bölge=yok, ömür=kalıcı)
            let lp = send_new!(w, cons, 1, ifaces().locked, (w.get_version)(cons as *mut c_void), NIL, surf as *mut c_void, ptr as *mut c_void, NIL, 2u32);
            if !lp.is_null() {
                static LOCK_L: Listener<2> = Listener([lock_locked as *const (), lock_unlocked as *const ()]);
                (w.add_listener)(lp, LOCK_L.0.as_ptr() as *const c_void, NIL);
                LOCKED.store(lp as usize, Ordering::Release);
            } else {
                dlog!("lock_pointer istegi nesne vermedi");
            }
        } else {
            dlog!("sunucuda zwp_pointer_constraints_v1 yok: imlec kilitlenemez, yakalama yalnizca imleci gizler");
        }
        // göreli fare: kilitliyken hareket miktarı buradan gelir
        let mgr = REL_MGR.load(Ordering::Acquire);
        if mgr != 0 && REL_PTR.load(Ordering::Acquire) == 0 {
            let rp = send_new!(w, mgr, 1, ifaces().rel_ptr, (w.get_version)(mgr as *mut c_void), NIL, ptr as *mut c_void);
            if !rp.is_null() {
                static REL_L: Listener<1> = Listener([rel_motion as *const ()]);
                (w.add_listener)(rp, REL_L.0.as_ptr() as *const c_void, NIL);
                REL_PTR.store(rp as usize, Ordering::Release);
            }
        }
    } else if !want && locked != 0 {
        send!(w, locked, 0, true); // zwp_locked_pointer_v1.destroy
        LOCKED.store(0, Ordering::Release);
    }
    apply_cursor(w, want);
}

// --------------------------------------------------------------------------------------- olay döngüsü

/// Wayland olaylarını okuyup işler (engelleme yok), imleç/kilit durumunu uygular, tuş tekrarını üretir.
pub fn pump() {
    let Some(w) = wl() else { return };
    let dpy = DISPLAY.load(Ordering::Acquire) as *mut c_void;
    if dpy.is_null() { return; }
    apply_grab(w);
    // Standart güvenli okuma düzeni: önce bekleyenleri işle, sonra soketten oku (bloklamadan), sonra yine işle.
    while (w.prepare_read)(dpy) != 0 {
        if (w.dispatch_pending)(dpy) < 0 { request_exit("Wayland baglantisi koptu"); return; }
    }
    (w.flush)(dpy);
    let mut pfd = PollFd { fd: (w.get_fd)(dpy), events: 1, revents: 0 }; // POLLIN
    if unsafe { c_poll(&mut pfd, 1, 0) } > 0 {
        if (w.read_events)(dpy) < 0 { request_exit("Wayland baglantisi koptu"); return; }
    } else {
        (w.cancel_read)(dpy);
    }
    if (w.dispatch_pending)(dpy) < 0 { request_exit("Wayland baglantisi koptu"); return; }
    // tuş tekrarı
    let now = Instant::now();
    let mut st = INPUT.lock().unwrap();
    if st.rep_key != 0 {
        let mut guard = 0;
        while let Some(t) = st.rep_next {
            if now < t || guard > 20 { break; }
            let kc = st.rep_key;
            key_event(&mut st, kc, true, true);
            st.rep_next = Some(t + st.rep_gap);
            guard += 1;
        }
    }
}

// ------------------------------------------------------------------------------------ açılış ve kapanış

/// Başarısızlıkta ya da kapanışta açılan her şeyi bırakır; böylece aynı süreçte X11'e geri dönülebilir.
fn cleanup() {
    let (dpy_egl, ctx, surf, win) = (
        EGL_DPY.swap(0, Ordering::AcqRel), EGL_CTX.swap(0, Ordering::AcqRel),
        EGL_SURF.swap(0, Ordering::AcqRel), EGL_WIN.swap(0, Ordering::AcqRel),
    );
    let _ = (ctx, surf);
    if dpy_egl != 0 {
        if let Some(e) = egl() {
            (e.make_current)(dpy_egl as *mut c_void, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut());
            (e.terminate)(dpy_egl as *mut c_void);
        }
    }
    if win != 0 {
        if let Some(f) = egl_win_fns().destroy { f(win as *mut c_void); }
    }
    let display = DISPLAY.swap(0, Ordering::AcqRel);
    if display != 0 {
        if let Some(w) = wl() { (w.disconnect)(display as *mut c_void); }
    }
    for a in [&REGISTRY, &COMPOSITOR, &SHM, &SEAT, &POINTER, &KEYBOARD, &XDG_BASE, &SURFACE, &XDG_SURF, &TOPLEVEL,
              &REL_MGR, &REL_PTR, &CONSTRAINTS, &LOCKED, &DECO_MGR, &CURSOR_SURF] {
        a.store(0, Ordering::Release);
    }
    CONFIGURED.store(false, Ordering::Release);
    CURSOR_STATE.store(0, Ordering::Release);
    ENTER_SERIAL.store(0, Ordering::Release);
}

fn fail(msg: &str) -> bool {
    eprintln!("[mono:wayland] {msg}");
    cleanup();
    false
}

pub fn try_init(w: i32, h: i32) -> bool {
    if INIT_OK.load(Ordering::Acquire) {
        return true;
    }
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return false;
    }
    let Some(wl) = wl() else { return fail("libwayland-client.so.0 acilamadi") };
    let display = (wl.connect)(std::ptr::null());
    if display.is_null() { return fail("Wayland sunucusuna baglanilamadi (WAYLAND_DISPLAY)"); }
    DISPLAY.store(display as usize, Ordering::Release);

    // 1) kayıt defteri: sunucunun desteklediği nesneleri öğren ve bağla
    let registry = send_new!(wl, display, 1, core_iface("wl_registry_interface"), (wl.get_version)(display), NIL);
    if registry.is_null() { return fail("wl_registry alinamadi"); }
    REGISTRY.store(registry as usize, Ordering::Release);
    static REG_L: Listener<2> = Listener([reg_global as *const (), noop as *const ()]);
    (wl.add_listener)(registry, REG_L.0.as_ptr() as *const c_void, NIL);
    if (wl.roundtrip)(display) < 0 || (wl.roundtrip)(display) < 0 { return fail("Wayland roundtrip basarisiz"); }
    let (comp, base) = (COMPOSITOR.load(Ordering::Acquire), XDG_BASE.load(Ordering::Acquire));
    if comp == 0 { return fail("sunucuda wl_compositor yok"); }
    if base == 0 { return fail("sunucuda xdg_wm_base yok (pencere acilamaz)"); }
    if (wl.roundtrip)(display) < 0 { return fail("Wayland roundtrip basarisiz"); } // koltuk yetenekleri ve klavye haritası gelsin

    // 2) pencere: wl_surface -> xdg_surface -> xdg_toplevel
    let surface = send_new!(wl, comp, 0, core_iface("wl_surface_interface"), (wl.get_version)(comp as *mut c_void), NIL);
    if surface.is_null() { return fail("wl_surface olusmadi"); }
    SURFACE.store(surface as usize, Ordering::Release);
    let i = ifaces();
    let xs = send_new!(wl, base, 2, i.xdg_surface, (wl.get_version)(base as *mut c_void), NIL, surface);
    if xs.is_null() { return fail("xdg_surface olusmadi"); }
    XDG_SURF.store(xs as usize, Ordering::Release);
    static XS_L: Listener<1> = Listener([xs_configure as *const ()]);
    (wl.add_listener)(xs, XS_L.0.as_ptr() as *const c_void, NIL);
    let tl = send_new!(wl, xs, 1, i.xdg_toplevel, (wl.get_version)(xs), NIL);
    if tl.is_null() { return fail("xdg_toplevel olusmadi"); }
    TOPLEVEL.store(tl as usize, Ordering::Release);
    // configure, close, configure_bounds, wm_capabilities
    static TL_L: Listener<4> = Listener([tl_configure as *const (), tl_close as *const (), noop as *const (), noop as *const ()]);
    (wl.add_listener)(tl, TL_L.0.as_ptr() as *const c_void, NIL);
    send!(wl, tl, 2, false, c"Monolith".as_ptr()); // set_title
    send!(wl, tl, 3, false, c"monolith".as_ptr()); // set_app_id
    // sunucu tarafı başlık çubuğu iste (destekleyen pencere yöneticilerinde)
    let dm = DECO_MGR.load(Ordering::Acquire);
    if dm != 0 {
        let deco = send_new!(wl, dm, 1, i.deco, (wl.get_version)(dm as *mut c_void), NIL, tl);
        if !deco.is_null() { send!(wl, deco, 1, false, 2u32); } // set_mode(server_side)
    }
    // Pencere opak: alfa kanalı yüzünden saydam görünmesin
    let region = send_new!(wl, comp, 1, core_iface("wl_region_interface"), (wl.get_version)(comp as *mut c_void));
    if !region.is_null() {
        send!(wl, region, 1, false, 0i32, 0i32, 16384i32, 16384i32); // add
        send!(wl, surface, 4, false, region); // set_opaque_region
        send!(wl, region, 0, true); // destroy
    }
    let (rw, rh) = window_size(w, h);
    PEND_W.store(0, Ordering::Release);
    PEND_H.store(0, Ordering::Release);
    send!(wl, surface, 6, false); // commit: pencere yöneticisi ilk configure'u göndersin
    for _ in 0..20 {
        if CONFIGURED.load(Ordering::Acquire) { break; }
        if (wl.roundtrip)(display) < 0 { return fail("pencere ilk configure olayini almadan baglanti koptu"); }
    }
    if !CONFIGURED.load(Ordering::Acquire) { return fail("pencere yoneticisi configure olayi gondermedi"); }
    // boyut: pencere yöneticisi söylediyse o, yoksa istenen
    let (fw, fh) = match (SIZE_W.load(Ordering::Acquire), PEND_W.load(Ordering::Acquire)) {
        (_, pw) if pw > 0 => (pw, PEND_H.load(Ordering::Acquire)),
        _ => (rw, rh),
    };
    SIZE_W.store(fw, Ordering::Release);
    SIZE_H.store(fh, Ordering::Release);

    // 3) OpenGL: EGL bağlamı
    if let Err(e) = init_egl(display, surface, fw, fh) { return fail(&format!("EGL: {e}")); }
    load_cursor(wl);
    (wl.flush)(display);
    INIT_OK.store(true, Ordering::Release);
    true
}

pub fn swap() {
    let (dpy, surf) = (EGL_DPY.load(Ordering::Acquire), EGL_SURF.load(Ordering::Acquire));
    if dpy != 0 && surf != 0 {
        if let Some(e) = egl() {
            (e.swap_buffers)(dpy as *mut c_void, surf as *mut c_void);
        }
    }
}

/// eglGetProcAddress ile OpenGL giriş noktası.
pub fn proc_address(name: &str) -> *const c_void {
    let Ok(cs) = CString::new(name) else { return std::ptr::null() };
    match egl() {
        Some(e) => (e.get_proc_address)(cs.as_ptr()),
        None => std::ptr::null(),
    }
}
