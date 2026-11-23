//! macOS yüzeyi: NSOpenGLContext olan bir AppKit penceresi (OpenGL 4.1 core, macOS'un sunduğu en yeni core profil;
//! oyun gölgelendiricilerinin kullandığı GLSL 3.30'u içerir), klavye, fare ve metin girdisi. Objective-C çalışma zamanı
//! burada tanımlanan imzalarla objc_msgSend üzerinden çağrılır; NSRect dönüşleri yalnızca x86_64'te objc_msgSend_stret
//! kullanır (arm64 bu yapıyı kayıtlarda döndürür). Boyutlar ve imleç konumları arka depo piksellerindedir (Retina
//! çarpanı uygulanmış). Olaylar pencereyi açan iş parçacığında, yani oyunun ana iş parçacığında okunur.

use super::*;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

type Id = *mut c_void;
type Sel = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct NSPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct NSRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

#[link(name = "objc")]
extern "C" {
    fn objc_getClass(name: *const c_char) -> Id;
    fn sel_registerName(name: *const c_char) -> Sel;
    fn objc_msgSend();
}

#[cfg(target_arch = "x86_64")]
#[link(name = "objc")]
extern "C" {
    fn objc_msgSend_stret();
}

#[link(name = "AppKit", kind = "framework")]
extern "C" {}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGAssociateMouseAndMouseCursorPosition(connected: i8) -> i32;
}

static INIT_OK: AtomicBool = AtomicBool::new(false);
static GRABBED: AtomicBool = AtomicBool::new(false);
static APP: AtomicUsize = AtomicUsize::new(0);
static VIEW: AtomicUsize = AtomicUsize::new(0);
static CTX: AtomicUsize = AtomicUsize::new(0);
/// Nokta başına arka depo pikseli (Retina ekranlarda 2.0).
static SCALE_BITS: AtomicU64 = AtomicU64::new(0x3FF0_0000_0000_0000); // 1.0
static SIZE_W: AtomicI32 = AtomicI32::new(1280);
static SIZE_H: AtomicI32 = AtomicI32::new(720);

/// Masaüstü girdi durumu (X11 yüzeyiyle aynı düzen ve anlam).
struct Input {
    keys: [u8; 19],
    slot: i32,
    cx: f32,
    cy: f32,
    dx: f32,
    dy: f32,
    warped: bool,
    text: std::collections::VecDeque<[i32; 3]>,
}
static INPUT: Mutex<Input> = Mutex::new(Input {
    keys: [0; 19], slot: 0, cx: 0.0, cy: 0.0, dx: 0.0, dy: 0.0, warped: false,
    text: std::collections::VecDeque::new(),
});

/// MONO_WINDOW_SIZE=WxH ilk boyutu nokta cinsinden geçersiz kılar.
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

fn scale() -> f64 {
    f64::from_bits(SCALE_BITS.load(Ordering::Acquire))
}

// ----- Objective-C mesajları (her imza, kendi çağrı yerinde kullanılan imzadır)

fn sel(name: &CStr) -> Sel {
    unsafe { sel_registerName(name.as_ptr()) }
}

fn class(name: &CStr) -> Id {
    unsafe { objc_getClass(name.as_ptr()) }
}

fn msg_fn<F: Copy>() -> F {
    unsafe { std::mem::transmute_copy::<*const c_void, F>(&(objc_msgSend as *const c_void)) }
}

unsafe fn id0(obj: Id, s: &CStr) -> Id {
    let f: extern "C" fn(Id, Sel) -> Id = msg_fn();
    f(obj, sel(s))
}
unsafe fn u0(obj: Id, s: &CStr) -> u64 {
    let f: extern "C" fn(Id, Sel) -> u64 = msg_fn();
    f(obj, sel(s))
}
unsafe fn f0(obj: Id, s: &CStr) -> f64 {
    let f: extern "C" fn(Id, Sel) -> f64 = msg_fn();
    f(obj, sel(s))
}
unsafe fn void0(obj: Id, s: &CStr) {
    let f: extern "C" fn(Id, Sel) = msg_fn();
    f(obj, sel(s))
}
unsafe fn void_id(obj: Id, s: &CStr, a: Id) {
    let f: extern "C" fn(Id, Sel, Id) = msg_fn();
    f(obj, sel(s), a)
}
unsafe fn void_i8(obj: Id, s: &CStr, a: i8) {
    let f: extern "C" fn(Id, Sel, i8) = msg_fn();
    f(obj, sel(s), a)
}
unsafe fn void_i64(obj: Id, s: &CStr, a: i64) {
    let f: extern "C" fn(Id, Sel, i64) = msg_fn();
    f(obj, sel(s), a)
}
unsafe fn id_cstr(obj: Id, s: &CStr, a: *const c_char) -> Id {
    let f: extern "C" fn(Id, Sel, *const c_char) -> Id = msg_fn();
    f(obj, sel(s), a)
}
unsafe fn id_attrs(obj: Id, s: &CStr, a: *const u32) -> Id {
    let f: extern "C" fn(Id, Sel, *const u32) -> Id = msg_fn();
    f(obj, sel(s), a)
}
unsafe fn id_id_id(obj: Id, s: &CStr, a: Id, b: Id) -> Id {
    let f: extern "C" fn(Id, Sel, Id, Id) -> Id = msg_fn();
    f(obj, sel(s), a, b)
}
unsafe fn window_init(obj: Id, s: &CStr, rect: NSRect, mask: u64, backing: u64, defer: i8) -> Id {
    let f: extern "C" fn(Id, Sel, NSRect, u64, u64, i8) -> Id = msg_fn();
    f(obj, sel(s), rect, mask, backing, defer)
}
unsafe fn next_event(app: Id, mask: u64, date: Id, mode: Id) -> Id {
    let f: extern "C" fn(Id, Sel, u64, Id, Id, i8) -> Id = msg_fn();
    f(app, sel(c"nextEventMatchingMask:untilDate:inMode:dequeue:"), mask, date, mode, 1)
}
unsafe fn point0(obj: Id, s: &CStr) -> NSPoint {
    let f: extern "C" fn(Id, Sel) -> NSPoint = msg_fn();
    f(obj, sel(s))
}
/// Bir görünümün dikdörtgeni. x86_64'te 32 baytlık yapı gizli bir işaretçiyle döner (objc_msgSend_stret).
#[cfg(target_arch = "x86_64")]
unsafe fn rect0(obj: Id, s: &CStr) -> NSRect {
    let f: extern "C" fn(*mut NSRect, Id, Sel) = std::mem::transmute_copy(&(objc_msgSend_stret as *const c_void));
    let mut r = NSRect::default();
    f(&mut r, obj, sel(s));
    r
}
#[cfg(not(target_arch = "x86_64"))]
unsafe fn rect0(obj: Id, s: &CStr) -> NSRect {
    let f: extern "C" fn(Id, Sel) -> NSRect = msg_fn();
    f(obj, sel(s))
}

// ----- tuşlar: macOS sanal tuş kodları (Carbon kVK_* değerleri)

/// Tuş kodu -> anlık görüntü tuş indeksi (X11 yüzeyiyle aynı sıra).
fn key_index(code: u16) -> Option<usize> {
    Some(match code {
        0x0D => 0, 0x00 => 1, 0x01 => 2, 0x02 => 3, // W A S D
        0x31 => 4, 0x35 => 9, 0x0E => 10, 0x0C => 11, 0x03 => 12,
        0x11 => 13, 0x63 => 14, 0x60 => 15, 0x30 => 16,
        _ => return None,
    })
}

/// Rakam tuş kodu -> hotbar yuvası ("1" için 0).
fn digit_slot(code: u16) -> Option<i32> {
    Some(match code {
        0x12 => 0, 0x13 => 1, 0x14 => 2, 0x15 => 3, 0x17 => 4, 0x16 => 5, 0x1A => 6, 0x1C => 7, 0x19 => 8,
        _ => return None,
    })
}

/// Düzenleme tuş kodu -> metin kuyruğu tuşu (X11 yüzeyiyle aynı değerler).
fn edit_key(code: u16) -> Option<i32> {
    Some(match code {
        0x33 => 8, 0x24 | 0x4C => 13, 0x75 => 127,
        0x7C => 0x4000_004F, 0x7B => 0x4000_0050, 0x7D => 0x4000_0051, 0x7E => 0x4000_0052,
        0x73 => 0x4000_004A, 0x77 => 0x4000_004D,
        _ => return None,
    })
}

/// Tek bir NSEvent'i işler (türler NSEvent.h'den).
unsafe fn handle_event(ev: Id) {
    let ty = u0(ev, c"type");
    match ty {
        10 | 11 => {
            // KeyDown, KeyUp (tuşa basma, bırakma)
            let code = (u0(ev, c"keyCode") & 0xFFFF) as u16;
            let down = ty == 10;
            let mut st = INPUT.lock().unwrap();
            if let Some(i) = key_index(code) { st.keys[i] = down as u8; }
            if let Some(slot) = digit_slot(code) { if down { st.slot = slot; } }
            if let Some(k) = edit_key(code) { st.text.push_back([2, k, down as i32]); }
            if down {
                let s = id0(ev, c"characters");
                if !s.is_null() {
                    let p = id0(s, c"UTF8String") as *const c_char;
                    if !p.is_null() {
                        for c in CStr::from_ptr(p).to_string_lossy().chars() {
                            let cp = c as u32;
                            // denetim karakterleri ve AppKit'in işlev tuşları için kullandığı özel kullanım kodları
                            if cp >= 0x20 && cp != 0x7F && !(0xF700..=0xF8FF).contains(&cp) {
                                st.text.push_back([1, cp as i32, 0]);
                            }
                        }
                    }
                }
            }
        }
        12 => {
            // FlagsChanged: değiştirici tuşlar
            let code = (u0(ev, c"keyCode") & 0xFFFF) as u16;
            let flags = u0(ev, c"modifierFlags");
            let mut st = INPUT.lock().unwrap();
            match code {
                0x38 | 0x3C => st.keys[5] = (flags & 0x0002_0000 != 0) as u8, // shift (NSEventModifierFlagShift)
                0x3B | 0x3E => st.keys[6] = (flags & 0x0004_0000 != 0) as u8, // control (NSEventModifierFlagControl)
                _ => {}
            }
        }
        1 | 2 => INPUT.lock().unwrap().keys[7] = (ty == 1) as u8,  // sol düğme
        3 | 4 => INPUT.lock().unwrap().keys[8] = (ty == 3) as u8,  // sağ düğme
        5 | 6 | 7 => {
            // MouseMoved, LeftMouseDragged, RightMouseDragged (fare hareketi)
            if GRABBED.load(Ordering::Acquire) {
                // yakalama sırasında imleç fareden ayrılır; olay ham hareketi taşır
                let (dx, dy) = (f0(ev, c"deltaX") as f32, f0(ev, c"deltaY") as f32);
                let mut st = INPUT.lock().unwrap();
                st.dx += dx;
                st.dy += dy;
            } else {
                let p = point0(ev, c"locationInWindow");
                let s = scale();
                let h = SIZE_H.load(Ordering::Acquire) as f64;
                let mut st = INPUT.lock().unwrap();
                st.cx = (p.x * s) as f32;
                st.cy = (h - p.y * s) as f32; // AppKit'te y ekseni yukarı bakar; oyunda aşağı
            }
        }
        _ => {}
    }
}

/// Görünümün boyutunu piksel olarak okur; değişmişse GL bağlamını günceller.
fn refresh_size() {
    let view = VIEW.load(Ordering::Acquire) as Id;
    if view.is_null() { return; }
    let b = unsafe { rect0(view, c"bounds") };
    let s = scale();
    let (w, h) = ((b.w * s).round() as i32, (b.h * s).round() as i32);
    if w > 0 && h > 0 && (w != SIZE_W.load(Ordering::Acquire) || h != SIZE_H.load(Ordering::Acquire)) {
        SIZE_W.store(w, Ordering::Release);
        SIZE_H.store(h, Ordering::Release);
        let ctx = CTX.load(Ordering::Acquire) as Id;
        if !ctx.is_null() { unsafe { void0(ctx, c"update") }; }
    }
}

/// Kuyruktaki her olayı işler, sonra AppKit pencereleri güncellemesine izin verir.
fn pump() {
    let app = APP.load(Ordering::Acquire) as Id;
    if app.is_null() { return; }
    unsafe {
        let mode = id_cstr(class(c"NSString"), c"stringWithUTF8String:", c"kCFRunLoopDefaultMode".as_ptr());
        let date = id0(class(c"NSDate"), c"distantPast");
        loop {
            let ev = next_event(app, u64::MAX, date, mode);
            if ev.is_null() { break; }
            handle_event(ev);
            void_id(app, c"sendEvent:", ev);
        }
        void0(app, c"updateWindows");
    }
    refresh_size();
}

pub fn poll(snap: &mut TcPollSnapshot) -> bool {
    if !is_ready() { return false; }
    pump();
    let (w, h) = size();
    let mut st = INPUT.lock().unwrap();
    snap.width = w;
    snap.height = h;
    snap.keys = st.keys;
    snap.keys[18] = (!GRABBED.load(Ordering::Acquire)) as u8; // fare yakalanmadığında imleç modu
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

pub fn size() -> (i32, i32) {
    (SIZE_W.load(Ordering::Acquire), SIZE_H.load(Ordering::Acquire))
}

pub fn set_grab(on: bool) {
    if GRABBED.swap(on, Ordering::AcqRel) == on { return; }
    INPUT.lock().unwrap().warped = false;
    if !is_ready() { return; }
    unsafe {
        // imleç artık fareyi izlemez; bakış hareketi olayların ham farklarından gelir
        CGAssociateMouseAndMouseCursorPosition((!on) as i8);
        if on { void0(class(c"NSCursor"), c"hide") } else { void0(class(c"NSCursor"), c"unhide") }
    }
}

pub fn try_init(w: i32, h: i32) -> bool {
    if INIT_OK.load(Ordering::Acquire) {
        return true;
    }
    let (w, h) = window_size(w, h);
    unsafe {
        let app = id0(class(c"NSApplication"), c"sharedApplication");
        if app.is_null() {
            return false;
        }
        void_i64(app, c"setActivationPolicy:", 0); // NSApplicationActivationPolicyRegular (normal uygulama)
        void0(app, c"finishLaunching");
        void_i8(app, c"activateIgnoringOtherApps:", 1);

        let rect = NSRect { x: 0.0, y: 0.0, w: w.max(1) as f64, h: h.max(1) as f64 };
        let style = 1 | 2 | 4 | 8; // başlıklı, kapatılabilir, küçültülebilir, boyutlandırılabilir
        let win = window_init(id0(class(c"NSWindow"), c"alloc"), c"initWithContentRect:styleMask:backing:defer:", rect, style, 2, 0);
        if win.is_null() {
            return false;
        }
        let title = id_cstr(class(c"NSString"), c"stringWithUTF8String:", c"Monolith".as_ptr());
        void_id(win, c"setTitle:", title);
        void_i8(win, c"setAcceptsMouseMovedEvents:", 1);
        let view = id0(win, c"contentView");
        if view.is_null() {
            return false;
        }
        void_i8(view, c"setWantsBestResolutionOpenGLSurface:", 1);

        // OpenGL 4.1 core profili (0x4100), çift tamponlu RGBA ve 24 bit derinlik tamponu
        let attrs: [u32; 8] = [99, 0x4100, 5, 8, 24, 12, 24, 0];
        let format = id_attrs(id0(class(c"NSOpenGLPixelFormat"), c"alloc"), c"initWithAttributes:", attrs.as_ptr());
        if format.is_null() {
            return false;
        }
        let ctx = id_id_id(id0(class(c"NSOpenGLContext"), c"alloc"), c"initWithFormat:shareContext:", format, std::ptr::null_mut());
        if ctx.is_null() {
            return false;
        }
        void_id(ctx, c"setView:", view);
        void0(ctx, c"makeCurrentContext");
        void_id(win, c"makeKeyAndOrderFront:", std::ptr::null_mut());

        APP.store(app as usize, Ordering::Release);
        VIEW.store(view as usize, Ordering::Release);
        CTX.store(ctx as usize, Ordering::Release);
        SCALE_BITS.store(f0(win, c"backingScaleFactor").to_bits(), Ordering::Release);
        INIT_OK.store(true, Ordering::Release);
    }
    refresh_size();
    true
}

pub fn is_ready() -> bool {
    INIT_OK.load(Ordering::Acquire)
}

pub fn swap() {
    let ctx = CTX.load(Ordering::Acquire) as Id;
    if !ctx.is_null() {
        unsafe { void0(ctx, c"flushBuffer") };
    }
}

/// OpenGL çerçevesinden OpenGL giriş noktası (oyunun kullandığı 1.2'den 4.1'e kadar fonksiyonları dışa aktarır).
pub fn proc_address(name: &str) -> *const c_void {
    let Ok(cs) = CString::new(name) else { return std::ptr::null() };
    let gl = gl_library();
    if gl.is_null() {
        return std::ptr::null();
    }
    unsafe { loader::sym(gl, cs.as_ptr()) as *const c_void }
}
