//! Windows yüzeyi: OpenGL 3.3 core bağlamı (WGL) olan bir Win32 penceresi; klavye, fare ve metin girdisi.
//! user32, gdi32 ve opengl32 sistem DLL'leridir; doğrudan bağlanır. Pencere ve iletileri, try_init'i çağıran
//! iş parçacığına (oyunun ana iş parçacığı) aittir. 3.3 core bağlamı yoksa (yalnızca GDI sürücüsü) eski bağlam tutulur,
//! pencere yine açılır; oyun eksik GL sürümünü kendisi bildirir.

use super::*;
use std::ffi::c_char;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

type Hwnd = *mut c_void;
type Hdc = *mut c_void;
type Hglrc = *mut c_void;
type Hinstance = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Point {
    x: i32,
    y: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct Msg {
    hwnd: Hwnd,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    pt: Point,
}

type WndProc = unsafe extern "system" fn(Hwnd, u32, usize, isize) -> isize;

#[repr(C)]
struct WndClassExW {
    cb_size: u32,
    style: u32,
    wnd_proc: Option<WndProc>,
    cb_cls_extra: i32,
    cb_wnd_extra: i32,
    hinstance: Hinstance,
    hicon: *mut c_void,
    hcursor: *mut c_void,
    hbr_background: *mut c_void,
    menu_name: *const u16,
    class_name: *const u16,
    hicon_sm: *mut c_void,
}

/// PIXELFORMATDESCRIPTOR, 40 bayt.
#[repr(C)]
#[derive(Default)]
struct PixelFormatDescriptor {
    n_size: u16,
    n_version: u16,
    dw_flags: u32,
    i_pixel_type: u8,
    c_color_bits: u8,
    c_red_bits: u8,
    c_red_shift: u8,
    c_green_bits: u8,
    c_green_shift: u8,
    c_blue_bits: u8,
    c_blue_shift: u8,
    c_alpha_bits: u8,
    c_alpha_shift: u8,
    c_accum_bits: u8,
    c_accum_red_bits: u8,
    c_accum_green_bits: u8,
    c_accum_blue_bits: u8,
    c_accum_alpha_bits: u8,
    c_depth_bits: u8,
    c_stencil_bits: u8,
    c_aux_buffers: u8,
    i_layer_type: u8,
    b_reserved: u8,
    dw_layer_mask: u32,
    dw_visible_mask: u32,
    dw_damage_mask: u32,
}

#[link(name = "user32")]
extern "system" {
    fn RegisterClassExW(wc: *const WndClassExW) -> u16;
    fn CreateWindowExW(
        ex_style: u32, class: *const u16, title: *const u16, style: u32, x: i32, y: i32, w: i32, h: i32,
        parent: Hwnd, menu: *mut c_void, inst: Hinstance, param: *mut c_void,
    ) -> Hwnd;
    fn ShowWindow(hwnd: Hwnd, cmd: i32) -> i32;
    fn GetDC(hwnd: Hwnd) -> Hdc;
    fn DefWindowProcW(hwnd: Hwnd, msg: u32, wp: usize, lp: isize) -> isize;
    fn PeekMessageW(msg: *mut Msg, hwnd: Hwnd, min: u32, max: u32, remove: u32) -> i32;
    fn TranslateMessage(msg: *const Msg) -> i32;
    fn DispatchMessageW(msg: *const Msg) -> isize;
    fn AdjustWindowRect(rect: *mut Rect, style: u32, menu: i32) -> i32;
    fn ClientToScreen(hwnd: Hwnd, p: *mut Point) -> i32;
    fn SetCursorPos(x: i32, y: i32) -> i32;
    fn ShowCursor(show: i32) -> i32;
    fn LoadCursorW(inst: Hinstance, name: *const u16) -> *mut c_void;
}

#[link(name = "gdi32")]
extern "system" {
    fn ChoosePixelFormat(dc: Hdc, pfd: *const PixelFormatDescriptor) -> i32;
    fn SetPixelFormat(dc: Hdc, format: i32, pfd: *const PixelFormatDescriptor) -> i32;
    fn SwapBuffers(dc: Hdc) -> i32;
}

#[link(name = "opengl32")]
extern "system" {
    fn wglCreateContext(dc: Hdc) -> Hglrc;
    fn wglMakeCurrent(dc: Hdc, rc: Hglrc) -> i32;
    fn wglDeleteContext(rc: Hglrc) -> i32;
    fn wglGetProcAddress(name: *const c_char) -> *mut c_void;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> Hinstance;
}

const CS_OWNDC: u32 = 0x0020;
const WS_OVERLAPPEDWINDOW: u32 = 0x00CF_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const CW_USEDEFAULT: i32 = i32::MIN;
const SW_SHOW: i32 = 5;
const PM_REMOVE: u32 = 1;
const IDC_ARROW: usize = 32512;
const PFD_DRAW_TO_WINDOW: u32 = 0x4;
const PFD_SUPPORT_OPENGL: u32 = 0x20;
const PFD_DOUBLEBUFFER: u32 = 0x1;
const WM_SIZE: u32 = 0x0005;
const WM_KEYDOWN: u32 = 0x0100;
const WM_KEYUP: u32 = 0x0101;
const WM_CHAR: u32 = 0x0102;
const WM_SYSKEYDOWN: u32 = 0x0104;
const WM_SYSKEYUP: u32 = 0x0105;
const WM_MOUSEMOVE: u32 = 0x0200;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_RBUTTONDOWN: u32 = 0x0204;
const WM_RBUTTONUP: u32 = 0x0205;
const WGL_CONTEXT_MAJOR_VERSION_ARB: i32 = 0x2091;
const WGL_CONTEXT_MINOR_VERSION_ARB: i32 = 0x2092;
const WGL_CONTEXT_PROFILE_MASK_ARB: i32 = 0x9126;
const WGL_CONTEXT_CORE_PROFILE_BIT_ARB: i32 = 0x1;

static INIT_OK: AtomicBool = AtomicBool::new(false);
static HWND: AtomicUsize = AtomicUsize::new(0);
static HDC: AtomicUsize = AtomicUsize::new(0);
static HGLRC: AtomicUsize = AtomicUsize::new(0);
static SIZE_W: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(1280);
static SIZE_H: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(720);
static GRABBED: AtomicBool = AtomicBool::new(false);

/// Masaüstü girdi durumu (X11 yüzeyiyle aynı düzen ve anlam).
struct Input {
    keys: [u8; 19],
    slot: i32,
    cx: f32,
    cy: f32,
    dx: f32,
    dy: f32,
    warped: bool,
    high_surrogate: u16,
    text: std::collections::VecDeque<[i32; 3]>,
}
static INPUT: Mutex<Input> = Mutex::new(Input {
    keys: [0; 19], slot: 0, cx: 0.0, cy: 0.0, dx: 0.0, dy: 0.0, warped: false, high_surrogate: 0,
    text: std::collections::VecDeque::new(),
});

/// MONO_WINDOW_SIZE=WxH ilk istemci boyutunu geçersiz kılar.
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

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Sanal tuş kodu -> anlık görüntü tuş indeksi (X11 yüzeyiyle aynı sıra; 7 ve 8 fare tuşlarıdır).
fn key_index(vk: u32) -> Option<usize> {
    Some(match vk {
        0x57 => 0, 0x41 => 1, 0x53 => 2, 0x44 => 3, // W A S D
        0x20 => 4, 0x10 | 0xA0 | 0xA1 => 5, 0x11 | 0xA2 | 0xA3 => 6,
        0x1B => 9, 0x45 => 10, 0x51 => 11, 0x46 => 12,
        0x54 => 13, 0x72 => 14, 0x74 => 15, 0x09 => 16,
        _ => return None,
    })
}

/// Sanal tuş kodu -> metin kuyruğu tuşu (X11 yüzeyiyle aynı değerler).
fn edit_key(vk: u32) -> Option<i32> {
    Some(match vk {
        0x08 => 8, 0x0D => 13, 0x2E => 127,
        0x27 => 0x4000_004F, 0x25 => 0x4000_0050, 0x28 => 0x4000_0051, 0x26 => 0x4000_0052,
        0x24 => 0x4000_004A, 0x23 => 0x4000_004D,
        _ => return None,
    })
}

fn lo16(v: isize) -> i32 { (v & 0xFFFF) as i16 as i32 }
fn hi16(v: isize) -> i32 { ((v >> 16) & 0xFFFF) as i16 as i32 }

unsafe extern "system" fn wnd_proc(hwnd: Hwnd, msg: u32, wp: usize, lp: isize) -> isize {
    match msg {
        WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP => {
            let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
            let vk = wp as u32;
            let mut st = INPUT.lock().unwrap();
            if let Some(i) = key_index(vk) { st.keys[i] = down as u8; }
            if (0x31..=0x39).contains(&vk) && down { st.slot = (vk - 0x31) as i32; }
            if let Some(k) = edit_key(vk) { st.text.push_back([2, k, down as i32]); }
        }
        WM_CHAR => {
            let unit = wp as u16;
            let mut st = INPUT.lock().unwrap();
            let cp: Option<u32> = if (0xD800..=0xDBFF).contains(&unit) {
                st.high_surrogate = unit;
                None
            } else if (0xDC00..=0xDFFF).contains(&unit) && st.high_surrogate != 0 {
                let hi = st.high_surrogate as u32;
                st.high_surrogate = 0;
                Some(0x10000 + ((hi - 0xD800) << 10) + (unit as u32 - 0xDC00))
            } else {
                st.high_surrogate = 0;
                Some(unit as u32)
            };
            if let Some(cp) = cp {
                if cp >= 0x20 && cp != 0x7F && char::from_u32(cp).is_some() {
                    st.text.push_back([1, cp as i32, 0]);
                }
            }
        }
        WM_LBUTTONDOWN | WM_LBUTTONUP => {
            INPUT.lock().unwrap().keys[7] = (msg == WM_LBUTTONDOWN) as u8;
        }
        WM_RBUTTONDOWN | WM_RBUTTONUP => {
            INPUT.lock().unwrap().keys[8] = (msg == WM_RBUTTONDOWN) as u8;
        }
        WM_MOUSEMOVE => {
            let (x, y) = (lo16(lp), hi16(lp));
            if GRABBED.load(Ordering::Acquire) {
                let (w, h) = (SIZE_W.load(Ordering::Acquire), SIZE_H.load(Ordering::Acquire));
                let (mx, my) = (w / 2, h / 2);
                let target = {
                    let mut st = INPUT.lock().unwrap();
                    if st.warped && (x - mx).abs() < 1 && (y - my).abs() < 1 {
                        None
                    } else {
                        if st.warped {
                            st.dx += (x - mx) as f32;
                            st.dy += (y - my) as f32;
                        }
                        st.warped = true;
                        Some(Point { x: mx, y: my })
                    }
                };
                if let Some(mut p) = target {
                    ClientToScreen(hwnd, &mut p);
                    SetCursorPos(p.x, p.y);
                }
            } else {
                let mut st = INPUT.lock().unwrap();
                st.cx = x as f32;
                st.cy = y as f32;
            }
        }
        WM_SIZE => {
            let (w, h) = (lo16(lp), hi16(lp));
            if w > 0 && h > 0 {
                SIZE_W.store(w, Ordering::Release);
                SIZE_H.store(h, Ordering::Release);
            }
        }
        _ => {}
    }
    DefWindowProcW(hwnd, msg, wp, lp)
}

/// Kuyruktaki tüm pencere iletilerini gönderir (pencere yordamı INPUT'u doldurur). Burada kilit tutulmaz, çünkü
/// yordam kendisi alır.
fn pump() {
    unsafe {
        let mut m: Msg = std::mem::zeroed();
        while PeekMessageW(&mut m, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
            TranslateMessage(&m);
            DispatchMessageW(&m);
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
    let hwnd = HWND.load(Ordering::Acquire) as Hwnd;
    if hwnd.is_null() { return; }
    unsafe {
        ShowCursor(if on { 0 } else { 1 });
        if on {
            let (w, h) = size();
            let mut p = Point { x: w / 2, y: h / 2 };
            ClientToScreen(hwnd, &mut p);
            SetCursorPos(p.x, p.y);
        }
    }
}

pub fn try_init(w: i32, h: i32) -> bool {
    if INIT_OK.load(Ordering::Acquire) {
        return true;
    }
    let (w, h) = window_size(w, h);
    unsafe {
        let inst = GetModuleHandleW(std::ptr::null());
        let class = wide("MonolithWindow");
        let wc = WndClassExW {
            cb_size: std::mem::size_of::<WndClassExW>() as u32,
            style: CS_OWNDC,
            wnd_proc: Some(wnd_proc),
            cb_cls_extra: 0,
            cb_wnd_extra: 0,
            hinstance: inst,
            hicon: std::ptr::null_mut(),
            hcursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW as *const u16),
            hbr_background: std::ptr::null_mut(),
            menu_name: std::ptr::null(),
            class_name: class.as_ptr(),
            hicon_sm: std::ptr::null_mut(),
        };
        // ikinci kayıt "class already exists" ile başarısız olur; pencere yine de bu sınıftan oluşturulabilir
        RegisterClassExW(&wc);
        let mut rc = Rect { left: 0, top: 0, right: w.max(1), bottom: h.max(1) };
        AdjustWindowRect(&mut rc, WS_OVERLAPPEDWINDOW, 0);
        let title = wide("Monolith");
        let hwnd = CreateWindowExW(
            0, class.as_ptr(), title.as_ptr(), WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT, CW_USEDEFAULT, rc.right - rc.left, rc.bottom - rc.top,
            std::ptr::null_mut(), std::ptr::null_mut(), inst, std::ptr::null_mut(),
        );
        if hwnd.is_null() {
            return false;
        }
        let hdc = GetDC(hwnd);
        if hdc.is_null() {
            return false;
        }
        let pfd = PixelFormatDescriptor {
            n_size: std::mem::size_of::<PixelFormatDescriptor>() as u16,
            n_version: 1,
            dw_flags: PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER,
            c_color_bits: 32,
            c_depth_bits: 24,
            ..Default::default()
        };
        let format = ChoosePixelFormat(hdc, &pfd);
        if format == 0 || SetPixelFormat(hdc, format, &pfd) == 0 {
            return false;
        }
        // eski bağlam, core profil GL fonksiyonlarının wglGetProcAddress ile yüklenebilmesini sağlar
        let legacy = wglCreateContext(hdc);
        if legacy.is_null() || wglMakeCurrent(hdc, legacy) == 0 {
            return false;
        }
        let mut ctx = legacy;
        let create_attribs = wglGetProcAddress(c"wglCreateContextAttribsARB".as_ptr());
        if !create_attribs.is_null() && (create_attribs as usize) > 3 {
            let create: unsafe extern "system" fn(Hdc, Hglrc, *const i32) -> Hglrc = std::mem::transmute(create_attribs);
            let attribs = [
                WGL_CONTEXT_MAJOR_VERSION_ARB, 3,
                WGL_CONTEXT_MINOR_VERSION_ARB, 3,
                WGL_CONTEXT_PROFILE_MASK_ARB, WGL_CONTEXT_CORE_PROFILE_BIT_ARB,
                0,
            ];
            let core = create(hdc, std::ptr::null_mut(), attribs.as_ptr());
            if !core.is_null() {
                wglMakeCurrent(hdc, std::ptr::null_mut());
                wglDeleteContext(legacy);
                if wglMakeCurrent(hdc, core) != 0 {
                    ctx = core;
                } else {
                    return false;
                }
            }
        }
        ShowWindow(hwnd, SW_SHOW);
        HWND.store(hwnd as usize, Ordering::Release);
        HDC.store(hdc as usize, Ordering::Release);
        HGLRC.store(ctx as usize, Ordering::Release);
        SIZE_W.store(w.max(1), Ordering::Release);
        SIZE_H.store(h.max(1), Ordering::Release);
        INIT_OK.store(true, Ordering::Release);
    }
    true
}

pub fn is_ready() -> bool {
    INIT_OK.load(Ordering::Acquire)
}

pub fn swap() {
    let hdc = HDC.load(Ordering::Acquire) as Hdc;
    if !hdc.is_null() {
        unsafe { SwapBuffers(hdc); }
    }
}

/// Geçerli bağlamın OpenGL 3.x giriş noktası (wglGetProcAddress; GL 1.1 fonksiyonları opengl32.dll'den gelir).
pub fn proc_address(name: &str) -> *const c_void {
    let Ok(cs) = std::ffi::CString::new(name) else { return std::ptr::null() };
    let p = unsafe { wglGetProcAddress(cs.as_ptr()) } as usize;
    // wglGetProcAddress başarısızlığı 0, 1, 2, 3 veya -1 ile bildirir
    if p <= 3 || p == usize::MAX { std::ptr::null() } else { p as *const c_void }
}
