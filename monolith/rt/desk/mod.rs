//! Masaüstü yüzeyi: işletim sistemi penceresi, OpenGL bağlamı, klavye, fare ve metin girdisi ve OpenGL giriş
//! noktalarının bulunması. Her platformun kendi gerçekleştirimi var; hepsinde aynı fonksiyonlar:
//!
//!   try_init(w, h) -> bool      pencereyi açar, OpenGL bağlamını etkin yapar (tekrarlanabilir)
//!   is_ready() -> bool          pencere açık
//!   size() -> (i32, i32)        istemci alanı, piksel
//!   set_grab(on)                fare yakalama: imleç gizlenir, bakış hareketi göreli
//!   poll(&mut snap) -> bool     pencere iletilerini işler, girdi anlık görüntüsünü doldurur
//!   take_text(out, max) -> i32  kuyruktaki metin ve düzenleme tuşları (her biri 3 tam sayı: tür, değer, basılı)
//!   swap()                      arka tamponu gösterir
//!   proc_address(name)          geçerli bağlamın OpenGL giriş noktası (yoksa null)
//!
//! Linux: x11.rs (X11 + GLX, kütüphaneler çalışma zamanında açılır). Windows: win32.rs (Win32 + WGL, sistem DLL'leri).
//! Diğer hedeflerde pencere yoktur (try_init false döner).
//! Oyun bu durumda yüzeysiz çalışır; yerel katman bunu oyuna dürüstçe bildirir.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};

/// TouchCore ana biriminin kullandığı düzende girdi anlık görüntüsü (natives_rhi.rs bunu `poll` ile okur).
#[repr(C)]
pub struct TcPollSnapshot {
    pub width: i32,
    pub height: i32,
    pub keys: [u8; 19],
    pub _pad: [u8; 2],
    pub hotbar_slot: i32,
    pub look_dx: f32,
    pub look_dy: f32,
    pub cursor_x: f32,
    pub cursor_y: f32,
}

// ----- çalışma zamanında açılan paylaşımlı kütüphaneler

#[cfg(unix)]
mod loader {
    use super::*;

    #[cfg(target_os = "linux")]
    #[link(name = "dl")]
    extern "C" {
        fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    }
    #[cfg(not(target_os = "linux"))]
    extern "C" {
        fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    }

    const RTLD_NOW: c_int = 2;
    const RTLD_GLOBAL: c_int = 0x100;

    pub unsafe fn open(path: &CStr) -> *mut c_void {
        dlopen(path.as_ptr(), RTLD_NOW | RTLD_GLOBAL)
    }
    pub unsafe fn sym(handle: *mut c_void, name: *const c_char) -> *mut c_void {
        dlsym(handle, name)
    }
}

#[cfg(windows)]
mod loader {
    use super::*;

    extern "system" {
        fn LoadLibraryA(name: *const c_char) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
    }

    pub unsafe fn open(path: &CStr) -> *mut c_void {
        LoadLibraryA(path.as_ptr())
    }
    pub unsafe fn sym(handle: *mut c_void, name: *const c_char) -> *mut c_void {
        GetProcAddress(handle, name)
    }
}

/// `names` içinden yükleyicinin bulduğu ilk kütüphaneyi açar. Sonra çalıştırılabilir dosyanın dizinine (ve
/// `natives` alt dizinine) bakar. Hiçbiri açılmazsa null döner.
pub fn dl_open_any(names: &[&str]) -> *mut c_void {
    for &n in names {
        if let Ok(cs) = CString::new(n) {
            let h = unsafe { loader::open(&cs) };
            if !h.is_null() {
                return h;
            }
        }
    }
    // MONO_NATIVES: başlatıcının yerel kütüphane klasörü (Android'de uygulamanın nativeLibraryDir'i;
    // libmonoshader.so orada). Başlatıcı bu değişkeni ayarlar; tanımlı değilse (masaüstü) atlanır.
    if let Some(dir) = std::env::var_os("MONO_NATIVES") {
        for &n in names {
            let p = std::path::Path::new(&dir).join(n);
            if let Ok(cs) = CString::new(p.to_string_lossy().as_bytes()) {
                let h = unsafe { loader::open(&cs) };
                if !h.is_null() {
                    return h;
                }
            }
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for &n in names {
                for sub in ["natives", "."] {
                    let p = dir.join(sub).join(n);
                    if let Ok(cs) = CString::new(p.to_string_lossy().as_bytes()) {
                        let h = unsafe { loader::open(&cs) };
                        if !h.is_null() {
                            return h;
                        }
                    }
                }
            }
        }
    }
    std::ptr::null_mut()
}

/// `name` (NUL ile biten bayt dizisi) kütüphanelerde aranır. Hiçbir kütüphanenin doğrudan dışa aktarmadığı bir GL
/// giriş noktası (Windows'ta 3.x fonksiyonları geçerli bağlamdan gelir) geçerli bağlamdan istenir.
pub fn dl_sym(libs: &[*mut c_void], name: &[u8]) -> *const c_void {
    for &h in libs {
        if !h.is_null() {
            let s = unsafe { loader::sym(h, name.as_ptr() as *const c_char) };
            if !s.is_null() {
                return s as *const c_void;
            }
        }
    }
    if let Ok(s) = std::str::from_utf8(&name[..name.len().saturating_sub(1)]) {
        if s.starts_with("gl") {
            return proc_address(s);
        }
    }
    std::ptr::null()
}

/// Kütüphanelerde bir sembol arar; `sym_fn!(&[lib], "ad", extern "C" fn(...) -> ...)` tipli işaretçi verir.
macro_rules! sym_fn {
    ($libs:expr, $name:literal, $ty:ty) => {{
        let p = dl_sym($libs, concat!($name, "\0").as_bytes());
        if p.is_null() { None } else { Some(unsafe { std::mem::transmute::<*const c_void, $ty>(p) }) }
    }};
}
pub(crate) use sym_fn;

// ----- platformun yüzeyi

#[cfg(target_os = "linux")]
mod x11;
#[cfg(target_os = "linux")]
pub use x11::*;

#[cfg(windows)]
mod win32;
#[cfg(windows)]
pub use win32::*;

#[cfg(not(any(target_os = "linux", windows)))]
mod none {
    use super::*;

    pub fn try_init(_w: i32, _h: i32) -> bool { false }
    pub fn is_ready() -> bool { false }
    pub fn size() -> (i32, i32) { (1280, 720) }
    pub fn set_grab(_on: bool) {}
    pub fn poll(_snap: &mut TcPollSnapshot) -> bool { false }
    pub fn take_text(_out: &mut [i32], _max: usize) -> i32 { 0 }
    pub fn swap() {}
    pub fn proc_address(_name: &str) -> *const c_void { std::ptr::null() }
}
#[cfg(not(any(target_os = "linux", windows)))]
pub use none::*;

/// Bu süreçte pencere açılıp açılamayacağı: Linux'ta ekran gerekir; Windows'ta her zaman vardır.
pub fn available() -> bool {
    if cfg!(target_os = "linux") {
        return std::env::var_os("DISPLAY").is_some();
    }
    cfg!(windows)
}

/// Bu platformun OpenGL kütüphanesi (bir kez açılır); açılamazsa null.
pub fn gl_library() -> *mut c_void {
    static H: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *H.get_or_init(|| {
        #[cfg(target_os = "linux")]
        {
            let gl = dl_open_any(&["libGL.so.1", "libOpenGL.so.0"]);
            if !gl.is_null() { return gl as usize; }
        }
        #[cfg(windows)]
        {
            let gl = dl_open_any(&["opengl32.dll"]);
            if !gl.is_null() { return gl as usize; }
        }
        0
    }) as *mut c_void
}
