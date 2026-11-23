//! X11 + GLX masaüstü yüzeyi (Linux): pencere, OpenGL 3.3 core bağlamı ve klavye/fare girdisi.
//! libX11.so.6 ve libGL.so.1 çalışma zamanında açılır; hiçbiri bağlanmaz. Bu dizindeki diğer yüzeylerle aynı sözleşme
//! (bkz. mod.rs).
use super::*;
use std::ffi::CString;
use std::os::raw::c_char;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

static INIT_OK: AtomicBool = AtomicBool::new(false);
static DPY: AtomicUsize = AtomicUsize::new(0);
static WIN: AtomicUsize = AtomicUsize::new(0);
static CTX: AtomicUsize = AtomicUsize::new(0);
static GLX_SWAP: AtomicUsize = AtomicUsize::new(0);
static X11: AtomicUsize = AtomicUsize::new(0);
static SIZE_W: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(1280);
static SIZE_H: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(720);
static GRABBED: AtomicBool = AtomicBool::new(false);

/// MONO_WINDOW_SIZE=WxH overrides the initial window size.
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

pub fn set_grab(on: bool) {
    GRABBED.store(on, Ordering::Release);
    let mut st = INPUT.lock().unwrap();
    st.warped = false;
}

/// Desktop input state, filled from X11 events in the layout of the TouchCore snapshot.
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

// keysym -> snapshot key index (same order as SDLEvents.BTN_KEYCODE; 7 and 8 are the mouse buttons)
fn key_index(ks: u64) -> Option<usize> {
    Some(match ks {
        0x77 | 0x57 => 0, 0x61 | 0x41 => 1, 0x73 | 0x53 => 2, 0x64 | 0x44 => 3, // w a s d
        0x20 => 4, 0xffe1 | 0xffe2 => 5, 0xffe3 | 0xffe4 => 6,                       // space shift ctrl
        0xff1b => 9, 0x65 | 0x45 => 10, 0x71 | 0x51 => 11, 0x66 | 0x46 => 12,        // esc e q f
        0x74 | 0x54 => 13, 0xffc0 => 14, 0xffc2 => 15, 0xff09 => 16,                  // t F3 F5 tab
        _ => return None,
    })
}
// keysym -> SDL keycode of the editing keys the game reads through the text queue
fn edit_key(ks: u64) -> Option<i32> {
    Some(match ks {
        0xff08 => 8, 0xff0d | 0xff8d => 13, 0xffff => 127,
        0xff53 => 0x4000004F, 0xff51 => 0x40000050, 0xff54 => 0x40000051, 0xff52 => 0x40000052,
        0xff50 => 0x4000004A, 0xff57 => 0x4000004D,
        _ => return None,
    })
}

fn rd_i32(ev: &[u8; 192], off: usize) -> i32 { i32::from_ne_bytes(ev[off..off + 4].try_into().unwrap()) }
fn rd_u32(ev: &[u8; 192], off: usize) -> u32 { u32::from_ne_bytes(ev[off..off + 4].try_into().unwrap()) }

/// Drains the X11 event queue into INPUT.
pub fn pump() {
    let dpy = DPY.load(Ordering::Acquire) as *mut c_void;
    let x11 = X11.load(Ordering::Acquire) as *mut c_void;
    if dpy.is_null() || x11.is_null() { return; }
    let (Some(pending), Some(next)) = (
        sym_fn!(&[x11], "XPending", extern "C" fn(*mut c_void) -> i32),
        sym_fn!(&[x11], "XNextEvent", extern "C" fn(*mut c_void, *mut u8) -> i32),
    ) else { return; };
    let lookup = sym_fn!(&[x11], "XLookupString", extern "C" fn(*mut u8, *mut u8, i32, *mut u64, *mut c_void) -> i32);
    let warp = sym_fn!(&[x11], "XWarpPointer", extern "C" fn(*mut c_void, usize, usize, i32, i32, u32, u32, i32, i32) -> i32);
    let grabbed = GRABBED.load(Ordering::Acquire);
    let (w, h) = size();
    let mut st = INPUT.lock().unwrap();
    while pending(dpy) > 0 {
        let mut ev = [0u8; 192];
        next(dpy, ev.as_mut_ptr());
        match rd_i32(&ev, 0) {
            2 | 3 => { // KeyPress, KeyRelease
                let down = rd_i32(&ev, 0) == 2;
                let mut buf = [0u8; 32];
                let mut ks: u64 = 0;
                let n = match lookup { Some(f) => f(ev.as_mut_ptr(), buf.as_mut_ptr(), 32, &mut ks, std::ptr::null_mut()), None => 0 };
                if let Some(i) = key_index(ks) { st.keys[i] = down as u8; }
                if (0x31..=0x39).contains(&ks) && down { st.slot = (ks - 0x31) as i32; }
                if let Some(k) = edit_key(ks) {
                    st.text.push_back([2, k, down as i32]);
                } else if down && n > 0 {
                    if let Ok(t) = std::str::from_utf8(&buf[..n as usize]) {
                        for c in t.chars() {
                            if !c.is_control() { st.text.push_back([1, c as i32, 0]); }
                        }
                    }
                }
            }
            4 | 5 => { // ButtonPress, ButtonRelease
                let down = rd_i32(&ev, 0) == 4;
                match rd_u32(&ev, 84) { 1 => st.keys[7] = down as u8, 3 => st.keys[8] = down as u8, _ => {} }
            }
            6 => { // MotionNotify
                let (x, y) = (rd_i32(&ev, 64) as f32, rd_i32(&ev, 68) as f32);
                if grabbed {
                    let (mx, my) = ((w / 2) as f32, (h / 2) as f32);
                    if st.warped && (x - mx).abs() < 1.0 && (y - my).abs() < 1.0 { continue; }
                    if st.warped { st.dx += x - mx; st.dy += y - my; }
                    st.warped = true;
                    if let Some(f) = warp {
                        f(dpy, 0, WIN.load(Ordering::Acquire), 0, 0, 0, 0, w / 2, h / 2);
                    }
                } else {
                    st.cx = x; st.cy = y;
                }
            }
            22 => { // ConfigureNotify
                let (nw, nh) = (rd_i32(&ev, 56), rd_i32(&ev, 60));
                if nw > 0 && nh > 0 { SIZE_W.store(nw, Ordering::Release); SIZE_H.store(nh, Ordering::Release); }
            }
            _ => {}
        }
    }
}

pub fn poll(snap: &mut TcPollSnapshot) -> bool {
    if !is_ready() { return false; }
    pump();
    let (w, h) = size();
    let mut st = INPUT.lock().unwrap();
    snap.width = w;
    snap.height = h;
    snap.keys = st.keys;
    snap.keys[18] = (!GRABBED.load(Ordering::Acquire)) as u8; // cursor mode while the mouse is not grabbed
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

// `sym_fn!` carries its own `unsafe` (needed in safe callers such as `pump`); the block below is already unsafe.
#[allow(unused_unsafe)]
pub fn try_init(w: i32, h: i32) -> bool {
    if INIT_OK.load(Ordering::Acquire) {
        return true;
    }
    if std::env::var_os("DISPLAY").is_none() {
        return false;
    }
    let x11 = dl_open_any(&["libX11.so.6"]);
    let gl = dl_open_any(&["libGL.so.1", "libGL.so"]);
    if x11.is_null() || gl.is_null() {
        return false;
    }
    unsafe {
        let x_open: Option<extern "C" fn(*const c_char) -> *mut c_void> =
            sym_fn!(&[x11], "XOpenDisplay", extern "C" fn(*const c_char) -> *mut c_void);
        let x_def_scr: Option<extern "C" fn(*mut c_void) -> i32> =
            sym_fn!(&[x11], "XDefaultScreen", extern "C" fn(*mut c_void) -> i32);
        let x_root: Option<extern "C" fn(*mut c_void, i32) -> usize> =
            sym_fn!(&[x11], "XRootWindow", extern "C" fn(*mut c_void, i32) -> usize);
        let x_create_simple: Option<extern "C" fn(*mut c_void, usize, i32, i32, u32, u32, u32, usize, usize) -> usize> =
            sym_fn!(&[x11], "XCreateSimpleWindow", extern "C" fn(*mut c_void, usize, i32, i32, u32, u32, u32, usize, usize) -> usize);
        let glx_choose_fb: Option<extern "C" fn(*mut c_void, i32, *const i32, *mut i32) -> *mut usize> =
            sym_fn!(&[gl], "glXChooseFBConfig", extern "C" fn(*mut c_void, i32, *const i32, *mut i32) -> *mut usize);
        let glx_new_ctx: Option<extern "C" fn(*mut c_void, usize, i32, *mut c_void, i32) -> *mut c_void> =
            sym_fn!(&[gl], "glXCreateNewContext", extern "C" fn(*mut c_void, usize, i32, *mut c_void, i32) -> *mut c_void);
        let glx_make_curr: Option<extern "C" fn(*mut c_void, usize, *mut c_void) -> i32> =
            sym_fn!(&[gl], "glXMakeCurrent", extern "C" fn(*mut c_void, usize, *mut c_void) -> i32);
        let glx_swap: Option<extern "C" fn(*mut c_void, usize)> =
            sym_fn!(&[gl], "glXSwapBuffers", extern "C" fn(*mut c_void, usize));
        let (Some(x_open), Some(x_def_scr), Some(x_root), Some(x_create_simple), Some(glx_choose_fb), Some(glx_new_ctx), Some(glx_make_curr)) =
            (x_open, x_def_scr, x_root, x_create_simple, glx_choose_fb, glx_new_ctx, glx_make_curr) else {
            return false;
        };
        let dpy = x_open(std::ptr::null());
        if dpy.is_null() { return false; }
        let scr = x_def_scr(dpy);
        let root = x_root(dpy, scr);
        let (w, h) = window_size(w, h);
        let win = x_create_simple(dpy, root, 0, 0, w.max(1) as u32, h.max(1) as u32, 0, 0, 0);
        if win == 0 { return false; }
        // title, input events, then show the window (an unmapped window is never visible)
        if let Some(f) = sym_fn!(&[x11], "XStoreName", extern "C" fn(*mut c_void, usize, *const c_char) -> i32) {
            f(dpy, win, b"Monolith\0".as_ptr() as *const c_char);
        }
        if let Some(f) = sym_fn!(&[x11], "XSelectInput", extern "C" fn(*mut c_void, usize, i64) -> i32) {
            // Key, Button press/release, PointerMotion, StructureNotify, FocusChange
            f(dpy, win, (1 << 0) | (1 << 1) | (1 << 2) | (1 << 3) | (1 << 6) | (1 << 17) | (1 << 21));
        }
        if let Some(f) = sym_fn!(&[x11], "XMapWindow", extern "C" fn(*mut c_void, usize) -> i32) {
            f(dpy, win);
        }
        if let Some(f) = sym_fn!(&[x11], "XFlush", extern "C" fn(*mut c_void) -> i32) {
            f(dpy);
        }
        X11.store(x11 as usize, Ordering::Release);
        SIZE_W.store(w, Ordering::Release);
        SIZE_H.store(h, Ordering::Release);
        let attrs: [i32; 15] = [
            0x8010, 1, // GLX_DRAWABLE_TYPE, GLX_WINDOW_BIT
            0x8011, 1, // GLX_RENDER_TYPE, GLX_RGBA_BIT
            5, 1,      // GLX_DOUBLEBUFFER, True
            8, 8,      // GLX_RED_SIZE
            9, 8,      // GLX_GREEN_SIZE
            10, 8,     // GLX_BLUE_SIZE
            12, 24,    // GLX_DEPTH_SIZE
            0,
        ];
        let mut n = 0i32;
        let fbs = glx_choose_fb(dpy, scr, attrs.as_ptr(), &mut n);
        if fbs.is_null() || n <= 0 { return false; }
        let fb = *fbs;
        let mut ctx = std::ptr::null_mut();
        if let Some(get_proc) = sym_fn!(&[gl], "glXGetProcAddressARB", extern "C" fn(*const u8) -> *const c_void) {
            let p = get_proc(b"glXCreateContextAttribsARB\0".as_ptr());
            if !p.is_null() {
                let create_attribs: extern "C" fn(*mut c_void, usize, *mut c_void, i32, *const i32) -> *mut c_void =
                    std::mem::transmute(p);
                let cattrs: [i32; 7] = [
                    0x2091, 3, // GLX_CONTEXT_MAJOR_VERSION_ARB
                    0x2092, 3, // GLX_CONTEXT_MINOR_VERSION_ARB
                    0x9126, 1, // GLX_CONTEXT_PROFILE_MASK_ARB, CORE
                    0,
                ];
                ctx = create_attribs(dpy, fb, std::ptr::null_mut(), 1, cattrs.as_ptr());
            }
        }
        if ctx.is_null() {
            ctx = glx_new_ctx(dpy, fb, 0x8014, std::ptr::null_mut(), 1);
        }
        if ctx.is_null() { return false; }
        if glx_make_curr(dpy, win, ctx) == 0 { return false; }
        DPY.store(dpy as usize, Ordering::Release);
        WIN.store(win, Ordering::Release);
        CTX.store(ctx as usize, Ordering::Release);
        if let Some(sw) = glx_swap {
            GLX_SWAP.store(sw as usize, Ordering::Release);
        }
        INIT_OK.store(true, Ordering::Release);
        true
    }
}

pub fn swap() {
    let dpy = DPY.load(Ordering::Acquire) as *mut c_void;
    let win = WIN.load(Ordering::Acquire);
    let sw = GLX_SWAP.load(Ordering::Acquire);
    if !dpy.is_null() && win != 0 && sw != 0 {
        let f: extern "C" fn(*mut c_void, usize) = unsafe { std::mem::transmute(sw) };
        f(dpy, win);
    }
}

pub fn is_ready() -> bool {
    INIT_OK.load(Ordering::Acquire)
}

/// glXGetProcAddressARB ile OpenGL giriş noktası (doğrudan yüklenir; böylece arama dl_sym'e dönüp sonsuz döngüye girmez).
pub fn proc_address(name: &str) -> *const c_void {
    let Ok(cs) = CString::new(name) else { return std::ptr::null() };
    let gl = gl_library();
    if gl.is_null() { return std::ptr::null(); }
    let p = unsafe { loader::sym(gl, c"glXGetProcAddressARB".as_ptr() as *const c_char) };
    if p.is_null() { return std::ptr::null(); }
    let get: extern "C" fn(*const c_char) -> *const c_void = unsafe { std::mem::transmute(p) };
    get(cs.as_ptr())
}
