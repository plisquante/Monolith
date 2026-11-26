// Kalıcı sanal fare (yalnızca sınama aracı): wlr-virtual-pointer protokolüyle bir fare aygıtı açar ve komut satırından
// aldığı komutları sırayla çalıştırır, aygıt bütün süre boyunca yaşar (wlrctl her çalışmada aygıtı kapatıyor).
//   komutlar: sleep:MS   move:DX,DY   abs:X,Y   click:left|right   quit
#![allow(warnings)]
use std::ffi::{c_char, c_int, c_void, CString};

#[repr(C)] struct Msg { name: *const c_char, sig: *const c_char, types: *const *const Iface }
#[repr(C)] struct Iface { name: *const c_char, version: c_int, mc: c_int, methods: *const Msg, ec: c_int, events: *const Msg }
extern "C" { fn dlopen(f: *const c_char, fl: c_int) -> *mut c_void; fn dlsym(h: *mut c_void, s: *const c_char) -> *mut c_void; }

fn leak(s: &str) -> *const c_char { CString::new(s).unwrap().into_raw() }
fn mk(name: &str, ver: i32, methods: &[(&str, &str)], events: &[(&str, &str)]) -> *const Iface {
    fn m(l: &[(&str, &str)]) -> *const Msg {
        let v: Vec<Msg> = l.iter().map(|(n, s)| {
            let argc = s.chars().filter(|c| c.is_ascii_alphabetic()).count();
            let t: &'static mut [*const Iface] = Box::leak(vec![std::ptr::null(); argc].into_boxed_slice());
            Msg { name: leak(n), sig: leak(s), types: t.as_ptr() }
        }).collect();
        Box::leak(v.into_boxed_slice()).as_ptr()
    }
    Box::leak(Box::new(Iface { name: leak(name), version: ver, mc: methods.len() as c_int, methods: m(methods), ec: events.len() as c_int, events: m(events) })) as *const Iface
}

type Flags = unsafe extern "C" fn(*mut c_void, u32, *const Iface, u32, u32, ...) -> *mut c_void;
static mut MARSHAL: Option<Flags> = None;
static mut SEAT: usize = 0;
static mut MGR: usize = 0;
static mut SEAT_IF: usize = 0;
static mut MGR_IF: *const Iface = std::ptr::null();
static mut GET_VER: Option<extern "C" fn(*mut c_void) -> u32> = None;

extern "C" fn global(_d: *mut c_void, reg: *mut c_void, name: u32, iface: *const c_char, ver: u32) {
    unsafe {
        let n = std::ffi::CStr::from_ptr(iface).to_str().unwrap();
        let m = MARSHAL.unwrap();
        if n == "wl_seat" && SEAT == 0 {
            let i = SEAT_IF as *const Iface;
            SEAT = m(reg, 0, i, 1, 0u32, name, (*i).name, 1u32, std::ptr::null_mut::<c_void>()) as usize;
        } else if n == "zwlr_virtual_pointer_manager_v1" && MGR == 0 {
            let v = ver.min(1);
            MGR = m(reg, 0, MGR_IF, v, 0u32, name, (*MGR_IF).name, v, std::ptr::null_mut::<c_void>()) as usize;
        }
    }
}
extern "C" fn noop() {}
#[repr(transparent)] struct L<const N: usize>([*const (); N]);
unsafe impl<const N: usize> Sync for L<N> {}
static REG_L: L<2> = L([global as *const (), noop as *const ()]);

fn main() {
    unsafe {
        let lib = dlopen(c"libwayland-client.so.0".as_ptr(), 2 | 0x100);
        let f = |n: &std::ffi::CStr| dlsym(lib, n.as_ptr());
        let connect: extern "C" fn(*const c_char) -> *mut c_void = std::mem::transmute(f(c"wl_display_connect"));
        let roundtrip: extern "C" fn(*mut c_void) -> c_int = std::mem::transmute(f(c"wl_display_roundtrip"));
        let flush: extern "C" fn(*mut c_void) -> c_int = std::mem::transmute(f(c"wl_display_flush"));
        let add: extern "C" fn(*mut c_void, *const c_void, *mut c_void) -> c_int = std::mem::transmute(f(c"wl_proxy_add_listener"));
        GET_VER = Some(std::mem::transmute(f(c"wl_proxy_get_version")));
        MARSHAL = Some(std::mem::transmute(f(c"wl_proxy_marshal_flags")));
        SEAT_IF = f(c"wl_seat_interface") as usize;
        let reg_if = f(c"wl_registry_interface") as *const Iface;
        MGR_IF = mk("zwlr_virtual_pointer_manager_v1", 1,
            &[("create_virtual_pointer", "?on"), ("destroy", "")], &[]);
        let vp_if = mk("zwlr_virtual_pointer_v1", 1,
            &[("motion", "uff"), ("motion_absolute", "uuuuu"), ("button", "uuu"), ("axis", "uuf"), ("frame", ""),
              ("axis_source", "u"), ("axis_stop", "uu"), ("axis_discrete", "uufi"), ("destroy", "")], &[]);
        let dpy = connect(std::ptr::null());
        assert!(!dpy.is_null(), "baglanti yok");
        let m = MARSHAL.unwrap();
        let reg = m(dpy, 1, reg_if, 1, 0u32, std::ptr::null_mut::<c_void>());
        add(reg, REG_L.0.as_ptr() as *const c_void, std::ptr::null_mut());
        roundtrip(dpy);
        roundtrip(dpy);
        assert!(SEAT != 0 && MGR != 0, "koltuk ya da sanal fare yoneticisi yok (seat={SEAT} mgr={MGR})");
        let vp = m(MGR as *mut c_void, 0, vp_if, 1, 0u32, SEAT as *mut c_void, std::ptr::null_mut::<c_void>());
        flush(dpy);
        roundtrip(dpy);
        println!("sanal fare hazir");
        let mut t: u32 = 1000;
        let fx = |v: f64| (v * 256.0) as i32;
        for cmd in std::env::args().skip(1) {
            let (k, v) = cmd.split_once(':').unwrap_or((cmd.as_str(), ""));
            match k {
                "sleep" => std::thread::sleep(std::time::Duration::from_millis(v.parse().unwrap())),
                "move" => {
                    let (a, b) = v.split_once(',').unwrap();
                    m(vp, 0, std::ptr::null(), 1, 0u32, t, fx(a.parse().unwrap()), fx(b.parse().unwrap()));
                    m(vp, 4, std::ptr::null(), 1, 0u32);
                }
                "abs" => {
                    let (a, b) = v.split_once(',').unwrap();
                    m(vp, 1, std::ptr::null(), 1, 0u32, t, a.parse::<u32>().unwrap(), b.parse::<u32>().unwrap(), 1280u32, 720u32);
                    m(vp, 4, std::ptr::null(), 1, 0u32);
                }
                "click" => {
                    let b: u32 = if v == "right" { 0x111 } else { 0x110 };
                    m(vp, 2, std::ptr::null(), 1, 0u32, t, b, 1u32); m(vp, 4, std::ptr::null(), 1, 0u32);
                    flush(dpy); std::thread::sleep(std::time::Duration::from_millis(60)); t += 60;
                    m(vp, 2, std::ptr::null(), 1, 0u32, t, b, 0u32); m(vp, 4, std::ptr::null(), 1, 0u32);
                }
                "quit" => break,
                _ => eprintln!("bilinmeyen komut {cmd}"),
            }
            flush(dpy);
            t += 20;
        }
        roundtrip(dpy);
    }
}
