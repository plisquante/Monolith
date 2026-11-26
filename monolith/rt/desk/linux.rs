//! Linux masaüstü yüzeyi seçicisi: X11 (x11.rs) ya da Wayland (wayland.rs). Çalışma zamanında seçilir.
//! Hangisinin açıldığı `BACKEND`te tutulur; sözleşmedeki her işlev (bkz. mod.rs) açılan yüzeye yönlenir.
//!
//! Seçim, `MONO_DISPLAY_BACKEND` ortam değişkeniyle:
//!   (boş) ya da "auto"   DISPLAY varsa X11 (eski davranış, değişmedi); yalnızca WAYLAND_DISPLAY varsa Wayland.
//!                        Seçilen açılamazsa öbürü denenir.
//!   "x11"                yalnızca X11.
//!   "wayland"            yalnızca Wayland; açılamazsa neden stderr'e yazılır (X11'e düşülmez).
//! Varsayılanın X11 kalması bilinçli: Wayland yüzeyi yeni; gerçek masaüstlerinde denendikten sonra "auto" Wayland'ı
//! ilk sıraya alabilir (tek satır: `prefer_wayland`).
use super::*;
use std::sync::atomic::{AtomicU8, Ordering};

const NONE: u8 = 0;
const X11: u8 = 1;
const WAYLAND: u8 = 2;
static BACKEND: AtomicU8 = AtomicU8::new(NONE);

/// Hangi yüzeylerin hangi sırayla deneneceği: (önce, sonra).
fn order() -> (u8, Option<u8>) {
    let has_x = std::env::var_os("DISPLAY").is_some();
    let has_w = std::env::var_os("WAYLAND_DISPLAY").is_some();
    match std::env::var("MONO_DISPLAY_BACKEND").as_deref() {
        Ok("x11") => (X11, None),
        Ok("wayland") => (WAYLAND, None),
        _ => {
            let prefer_wayland = !has_x && has_w;
            if prefer_wayland { (WAYLAND, if has_x { Some(X11) } else { None }) } else { (X11, if has_w { Some(WAYLAND) } else { None }) }
        }
    }
}

fn open(which: u8, w: i32, h: i32) -> bool {
    match which {
        X11 => x11::try_init(w, h),
        WAYLAND => wayland::try_init(w, h),
        _ => false,
    }
}

pub fn try_init(w: i32, h: i32) -> bool {
    if BACKEND.load(Ordering::Acquire) != NONE {
        return is_ready();
    }
    let (first, second) = order();
    for which in std::iter::once(first).chain(second) {
        if open(which, w, h) {
            BACKEND.store(which, Ordering::Release);
            return true;
        }
    }
    false
}

pub fn is_ready() -> bool {
    match BACKEND.load(Ordering::Acquire) {
        X11 => x11::is_ready(),
        WAYLAND => wayland::is_ready(),
        _ => false,
    }
}

pub fn size() -> (i32, i32) {
    match BACKEND.load(Ordering::Acquire) {
        WAYLAND => wayland::size(),
        _ => x11::size(),
    }
}

pub fn set_grab(on: bool) {
    match BACKEND.load(Ordering::Acquire) {
        X11 => x11::set_grab(on),
        WAYLAND => wayland::set_grab(on),
        _ => {}
    }
}

pub fn poll(snap: &mut TcPollSnapshot) -> bool {
    match BACKEND.load(Ordering::Acquire) {
        X11 => x11::poll(snap),
        WAYLAND => wayland::poll(snap),
        _ => false,
    }
}

pub fn take_text(out: &mut [i32], max: usize) -> i32 {
    match BACKEND.load(Ordering::Acquire) {
        X11 => x11::take_text(out, max),
        WAYLAND => wayland::take_text(out, max),
        _ => 0,
    }
}

pub fn swap() {
    match BACKEND.load(Ordering::Acquire) {
        X11 => x11::swap(),
        WAYLAND => wayland::swap(),
        _ => {}
    }
}

/// Açılmış yüzey Wayland ise eglGetProcAddress, değilse (X11 ya da henüz açılmamış) glXGetProcAddressARB.
pub fn proc_address(name: &str) -> *const c_void {
    match BACKEND.load(Ordering::Acquire) {
        WAYLAND => wayland::proc_address(name),
        _ => x11::proc_address(name),
    }
}
