// Wayland yüzeyi sınaması: desk modülünü aynen içine alır, sözleşmedeki işlevleri sırayla dener ve gelen girdiyi yazdırır.
//   kullanım: wl_probe <saniye> [grab_baslangic_sn grab_bitis_sn]
#![allow(warnings)]
#[path = "../../monolith/rt/desk/mod.rs"]
mod desk;
use std::ffi::{c_void, CStr};
use std::time::{Duration, Instant};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let secs: f32 = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(5.0);
    let grab_on: Option<f32> = a.get(2).and_then(|s| s.parse().ok());
    let grab_off: Option<f32> = a.get(3).and_then(|s| s.parse().ok());
    println!("available={}", desk::available());
    let ok = desk::try_init(640, 480);
    println!("try_init={} ready={} size={:?}", ok, desk::is_ready(), desk::size());
    if !ok { std::process::exit(2); }

    // OpenGL: sürüm ve boyama
    let get_string: extern "C" fn(u32) -> *const i8 = unsafe { std::mem::transmute(desk::proc_address("glGetString")) };
    let ver = unsafe { CStr::from_ptr(get_string(0x1F02)) }.to_string_lossy().into_owned();
    let rend = unsafe { CStr::from_ptr(get_string(0x1F01)) }.to_string_lossy().into_owned();
    println!("GL_VERSION={ver} | GL_RENDERER={rend}");
    let clear_color: extern "C" fn(f32, f32, f32, f32) = unsafe { std::mem::transmute(desk::proc_address("glClearColor")) };
    let clear: extern "C" fn(u32) = unsafe { std::mem::transmute(desk::proc_address("glClear")) };
    let read: extern "C" fn(i32, i32, i32, i32, u32, u32, *mut u8) = unsafe { std::mem::transmute(desk::proc_address("glReadPixels")) };
    clear_color(1.0, 0.0, 0.0, 1.0);
    clear(0x4000);
    let mut px = [0u8; 4];
    read(1, 1, 1, 1, 0x1908, 0x1401, px.as_mut_ptr());
    println!("PIXEL(1,1)={:?} (kirmizi bekleniyor: [255,0,0,*])", px);
    desk::swap();

    let t0 = Instant::now();
    let mut snap: desk::TcPollSnapshot = unsafe { std::mem::zeroed() };
    let (mut last_keys, mut last_cur, mut last_size) = ([0u8; 19], (-1.0f32, -1.0f32), (0, 0));
    let (mut sum_dx, mut sum_dy, mut grabbed) = (0.0f32, 0.0f32, false);
    let mut frames = 0u32; let (mut did_on, mut did_off) = (false, false);
    while t0.elapsed().as_secs_f32() < secs {
        let t = t0.elapsed().as_secs_f32();
        if let Some(g) = grab_on { if !did_on && t >= g { desk::set_grab(true); grabbed = true; did_on = true; println!("[{t:.1}s] set_grab(true)"); } }
        if let Some(g) = grab_off { if !did_off && t >= g { desk::set_grab(false); grabbed = false; did_off = true; println!("[{t:.1}s] set_grab(false)"); } }
        desk::poll(&mut snap);
        if snap.keys != last_keys {
            println!("[{t:.1}s] KEYS {:?}", snap.keys);
            last_keys = snap.keys;
        }
        if (snap.cursor_x, snap.cursor_y) != last_cur {
            println!("[{t:.1}s] CURSOR {:.3},{:.3}", snap.cursor_x, snap.cursor_y);
            last_cur = (snap.cursor_x, snap.cursor_y);
        }
        if (snap.width, snap.height) != last_size {
            println!("[{t:.1}s] SIZE {}x{} hotbar={}", snap.width, snap.height, snap.hotbar_slot);
            last_size = (snap.width, snap.height);
        }
        if snap.look_dx != 0.0 || snap.look_dy != 0.0 {
            sum_dx += snap.look_dx; sum_dy += snap.look_dy;
            println!("[{t:.1}s] LOOK dx={:.2} dy={:.2}", snap.look_dx, snap.look_dy);
        }
        let mut buf = [0i32; 3 * 16];
        let n = desk::take_text(&mut buf, 16);
        for i in 0..n as usize {
            let (kind, val, down) = (buf[3 * i], buf[3 * i + 1], buf[3 * i + 2]);
            if kind == 1 { println!("[{t:.1}s] TEXT char {:?} (U+{:04X})", char::from_u32(val as u32).unwrap_or('?'), val); }
            else { println!("[{t:.1}s] TEXT edit key={val:#x} down={down}"); }
        }
        // her kare: mavi-yeşil dönüşümlü boya + sun
        let c = (frames % 60) as f32 / 60.0;
        clear_color(0.0, c, 1.0 - c, 1.0);
        clear(0x4000);
        desk::swap();
        frames += 1;
        std::thread::sleep(Duration::from_millis(8));
    }
    println!("OZET kare={} toplam_look=({:.2},{:.2}) son_boyut={:?}", frames, sum_dx, sum_dy, desk::size());
}
