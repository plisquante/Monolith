// Masaüstü arka ucunun duman testi: pencere açılır, OpenGL temizler ve bir pikseli geri okur, tamponlar değişir,
// girdi yoklanır, fare yakalama açılıp kapanır ve ses aygıtına bir ton gönderilir (aygıt yoksa sessiz aygıta; bu
// bildirilir). Her hata 1 koduyla çıkar; hiçbir şey sessizce atlanmaz.
// .github/workflows/platforms.yml tarafından Linux (Xvfb altında), Windows ve macOS'ta çalıştırılır.
// Derleme ve çalıştırma: rustc --edition 2021 -O tests/rust/desktop_main.rs -o desktop_smoke && ./desktop_smoke
#[path = "../../monolith/rt/desk/mod.rs"]
pub mod desk;
#[path = "../../monolith/rt/audio.rs"]
pub mod audio;
#[path = "../../monolith/rt/audio_dev.rs"]
pub mod audio_dev;
#[path = "../../monolith/rt/audio_wasapi.rs"]
pub mod audio_wasapi;
#[path = "../../monolith/rt/audio_coreaudio.rs"]
pub mod audio_coreaudio;

use audio::Sink;
use std::ffi::{c_void, CStr};

const GL_COLOR_BUFFER_BIT: u32 = 0x4000;
const GL_RGBA: u32 = 0x1908;
const GL_UNSIGNED_BYTE: u32 = 0x1401;
const GL_RENDERER: u32 = 0x1F01;

fn fail(msg: &str) -> ! {
    eprintln!("desktop smoke: FAILED: {msg}");
    std::process::exit(1);
}

/// Kütüphaneden (GL 1.x) veya bulunamazsa geçerli bağlamdan (GL 3.x) bir GL giriş noktası.
fn proc<F: Copy>(lib: *mut c_void, name: &CStr) -> F {
    let mut p = desk::dl_sym(&[lib], name.to_bytes_with_nul());
    if p.is_null() {
        p = desk::proc_address(name.to_str().unwrap());
    }
    if p.is_null() {
        fail(&format!("OpenGL entry point {} is missing", name.to_string_lossy()));
    }
    unsafe { std::mem::transmute_copy::<*const c_void, F>(&p) }
}

fn main() {
    if !desk::available() {
        fail("no display for this process (DISPLAY is not set)");
    }
    if !desk::try_init(640, 360) {
        fail("the window or its OpenGL context did not open");
    }
    if !desk::is_ready() {
        fail("the window is not ready after try_init");
    }
    let (w, h) = desk::size();
    if w <= 0 || h <= 0 {
        fail(&format!("bad window size {w}x{h}"));
    }

    let lib = desk::gl_library();
    if lib.is_null() {
        fail("no OpenGL library on this platform");
    }
    let clear_color: extern "C" fn(f32, f32, f32, f32) = proc(lib, c"glClearColor");
    let clear: extern "C" fn(u32) = proc(lib, c"glClear");
    let read_pixels: extern "C" fn(i32, i32, i32, i32, u32, u32, *mut u8) = proc(lib, c"glReadPixels");
    let get_string: extern "C" fn(u32) -> *const u8 = proc(lib, c"glGetString");

    clear_color(1.0, 0.0, 0.0, 1.0);
    clear(GL_COLOR_BUFFER_BIT);
    let mut px = [0u8; 4];
    read_pixels(0, 0, 1, 1, GL_RGBA, GL_UNSIGNED_BYTE, px.as_mut_ptr());
    if px != [255, 0, 0, 255] {
        fail(&format!("the cleared pixel reads {px:?}, expected [255, 0, 0, 255]"));
    }
    desk::swap();

    let renderer = {
        let p = get_string(GL_RENDERER);
        if p.is_null() { "unknown".to_string() } else { unsafe { CStr::from_ptr(p as *const _) }.to_string_lossy().into_owned() }
    };

    let mut snap: desk::TcPollSnapshot = unsafe { std::mem::zeroed() };
    if !desk::poll(&mut snap) {
        fail("poll reports no window");
    }
    desk::set_grab(true);
    desk::poll(&mut snap);
    desk::set_grab(false);
    let mut text = [0i32; 9];
    let _ = desk::take_text(&mut text, 3);

    let (mut sink, device) = audio_dev::open_device();
    println!("{device}");
    let rate = audio::MIX_RATE as f64;
    let mut block = vec![0.0f32; 1024 * 2];
    for b in 0..(audio::MIX_RATE as usize / 1024 / 2) {
        for f in 0..1024 {
            let t = (b * 1024 + f) as f64 / rate;
            let v = (2.0 * std::f64::consts::PI * 440.0 * t).sin() as f32 * 0.2;
            block[2 * f] = v;
            block[2 * f + 1] = v;
        }
        sink.write(&block);
    }
    println!("desktop smoke: ok ({w}x{h}, renderer: {renderer}; {device})");
}
