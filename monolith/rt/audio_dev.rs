//! `audio.rs` içindeki karıştırıcı için ses çıkışı ve OpenAL uyumlu yerel fonksiyonların arkasındaki süreç çapında ses motoru.
//!
//! Aygıtlar: Linux'ta ALSA (libasound.so.2 çalışma zamanında dlopen ile açılır; derleme zamanında hiçbir şey bağlanmaz ya da gerekmez),
//! ve çıkış yoksa (ses kartı yoksa ya da sesi olmayan bir sunucuysa) gerçek zamanlı olarak kendi hızını ayarlayan sessiz bir aygıt.
//! Windows'ta WASAPI (audio_wasapi.rs), Android'de AAudio (libaaudio.so, dlopen). macOS hedef değildir.
//!
//! Motor tek bir karıştırıcı iş parçacığıdır; paylaşılan `audio::Context`'ten blokları üretip aygıta yazar. Oyun iş parçacığı
//! bağlamı yerel fonksiyonlar üzerinden değiştirir; her blok için kısa bir kilit tek eşzamanlama noktasıdır.

use crate::audio::{Context, Sink, MIX_RATE};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

const BLOCK_FRAMES: usize = 1024;

#[cfg(target_os = "linux")]
mod alsa {
    use super::*;
    use std::ffi::{c_char, c_int, c_long, c_uint, c_void, CStr, CString};

    // ----- ALSA (libasound.so.2): ara yüzlü S16 oynatma için gereken birkaç çağrı
    const SND_PCM_STREAM_PLAYBACK: c_int = 0;
    const SND_PCM_FORMAT_S16_LE: c_int = 2;
    const SND_PCM_ACCESS_RW_INTERLEAVED: c_int = 3;

    extern "C" {
        fn dlopen(file: *const c_char, flags: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    }
    const RTLD_NOW: c_int = 2;

    pub(super) struct Alsa {
        open: unsafe extern "C" fn(*mut *mut c_void, *const c_char, c_int, c_int) -> c_int,
        set_params: unsafe extern "C" fn(*mut c_void, c_int, c_int, c_uint, c_uint, c_int, c_uint) -> c_int,
        writei: unsafe extern "C" fn(*mut c_void, *const c_void, c_long) -> c_long,
        recover: unsafe extern "C" fn(*mut c_void, c_int, c_int) -> c_int,
        close: unsafe extern "C" fn(*mut c_void) -> c_int,
        strerror: unsafe extern "C" fn(c_int) -> *const c_char,
    }

    pub(super) fn alsa() -> Option<&'static Alsa> {
        static A: OnceLock<Option<Alsa>> = OnceLock::new();
        A.get_or_init(|| unsafe {
            let h = dlopen(c"libasound.so.2".as_ptr(), RTLD_NOW);
            if h.is_null() {
                return None;
            }
            let sym = |n: &CStr| {
                let p = dlsym(h, n.as_ptr());
                if p.is_null() { None } else { Some(p) }
            };
            Some(Alsa {
                open: std::mem::transmute::<*mut c_void, unsafe extern "C" fn(*mut *mut c_void, *const c_char, c_int, c_int) -> c_int>(sym(c"snd_pcm_open")?),
                set_params: std::mem::transmute::<*mut c_void, unsafe extern "C" fn(*mut c_void, c_int, c_int, c_uint, c_uint, c_int, c_uint) -> c_int>(sym(c"snd_pcm_set_params")?),
                writei: std::mem::transmute::<*mut c_void, unsafe extern "C" fn(*mut c_void, *const c_void, c_long) -> c_long>(sym(c"snd_pcm_writei")?),
                recover: std::mem::transmute::<*mut c_void, unsafe extern "C" fn(*mut c_void, c_int, c_int) -> c_int>(sym(c"snd_pcm_recover")?),
                close: std::mem::transmute::<*mut c_void, unsafe extern "C" fn(*mut c_void) -> c_int>(sym(c"snd_pcm_close")?),
                strerror: std::mem::transmute::<*mut c_void, unsafe extern "C" fn(c_int) -> *const c_char>(sym(c"snd_strerror")?),
            })
        })
        .as_ref()
    }

    pub struct AlsaSink {
        api: &'static Alsa,
        pcm: *mut c_void,
        pub device: String,
    }

    // bu tutamacı yalnızca karıştırıcı iş parçacığı kullanır
    unsafe impl Send for AlsaSink {}

    impl AlsaSink {
        pub fn open(device: &str) -> Result<AlsaSink, String> {
            let api = alsa().ok_or("libasound.so.2 not found")?;
            let name = CString::new(device).map_err(|_| "device name contains NUL")?;
            let mut pcm: *mut c_void = std::ptr::null_mut();
            unsafe {
                let r = (api.open)(&mut pcm, name.as_ptr(), SND_PCM_STREAM_PLAYBACK, 0);
                if r < 0 {
                    return Err(format!("open {device}: {}", strerror(api, r)));
                }
                // yumuşak yeniden örnekleme açık, gecikme 50 ms
                let r = (api.set_params)(pcm, SND_PCM_FORMAT_S16_LE, SND_PCM_ACCESS_RW_INTERLEAVED, 2, MIX_RATE, 1, 50_000);
                if r < 0 {
                    (api.close)(pcm);
                    return Err(format!("set params on {device}: {}", strerror(api, r)));
                }
            }
            Ok(AlsaSink { api, pcm, device: device.to_string() })
        }
    }

    fn strerror(api: &Alsa, code: c_int) -> String {
        unsafe { CStr::from_ptr((api.strerror)(code)).to_string_lossy().into_owned() }
    }

    impl Sink for AlsaSink {
        fn write(&mut self, interleaved: &[f32]) {
            let mut pcm16: Vec<i16> = Vec::with_capacity(interleaved.len());
            for &v in interleaved {
                pcm16.push((v.clamp(-1.0, 1.0) * 32767.0) as i16);
            }
            let mut done = 0usize; // frames
            let total = pcm16.len() / 2;
            while done < total {
                let n = unsafe { (self.api.writei)(self.pcm, pcm16[done * 2..].as_ptr() as *const c_void, (total - done) as c_long) };
                if n < 0 {
                    // yetersiz veri (-EPIPE) ya da askıya alma: kurtar ve yeniden dene; başka hatalar bir kurtarmadan sonra atılır
                    let r = unsafe { (self.api.recover)(self.pcm, n as c_int, 1) };
                    if r < 0 {
                        eprintln!("audio: write failed on {}: {}", self.device, strerror(self.api, r));
                        return;
                    }
                    continue;
                }
                done += n as usize;
            }
        }
    }

    impl Drop for AlsaSink {
        fn drop(&mut self) {
            unsafe { (self.api.close)(self.pcm) };
        }
    }
}

#[cfg(target_os = "linux")]
use alsa::AlsaSink;

// ----- Android: AAudio (libaaudio.so, API 26+). Derleme zamanında bağlanmaz; çalışma anında dlopen ile açılır, böylece
// kütüphane yoksa motor yine yüklenir ve sessiz aygıta düşer. Sabitler NDK r30 aaudio/AAudio.h'den.
#[cfg(target_os = "android")]
mod aaudio {
    use super::*;
    use std::ffi::{c_char, c_int, c_void, CStr};

    const AAUDIO_DIRECTION_OUTPUT: i32 = 0;
    const AAUDIO_FORMAT_PCM_FLOAT: i32 = 2;
    const AAUDIO_SHARING_MODE_SHARED: i32 = 1;
    const AAUDIO_PERFORMANCE_MODE_NONE: i32 = 10;
    const AAUDIO_USAGE_GAME: i32 = 14;
    const AAUDIO_ERROR_DISCONNECTED: i32 = -899;

    extern "C" {
        fn dlopen(file: *const c_char, flags: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    }
    const RTLD_NOW: c_int = 2;

    type B = *mut c_void; // AAudioStreamBuilder*
    type S = *mut c_void; // AAudioStream*
    pub(super) struct Api {
        create_builder: unsafe extern "C" fn(*mut B) -> i32,
        set_direction: unsafe extern "C" fn(B, i32),
        set_rate: unsafe extern "C" fn(B, i32),
        set_channels: unsafe extern "C" fn(B, i32),
        set_format: unsafe extern "C" fn(B, i32),
        set_sharing: unsafe extern "C" fn(B, i32),
        set_perf: unsafe extern "C" fn(B, i32),
        /// API 28+: yoksa atlanır
        set_usage: Option<unsafe extern "C" fn(B, i32)>,
        open: unsafe extern "C" fn(B, *mut S) -> i32,
        delete_builder: unsafe extern "C" fn(B) -> i32,
        start: unsafe extern "C" fn(S) -> i32,
        write: unsafe extern "C" fn(S, *const c_void, i32, i64) -> i32,
        close: unsafe extern "C" fn(S) -> i32,
        text: unsafe extern "C" fn(i32) -> *const c_char,
    }

    pub(super) fn api() -> Option<&'static Api> {
        static A: OnceLock<Option<Api>> = OnceLock::new();
        A.get_or_init(|| unsafe {
            let h = dlopen(c"libaaudio.so".as_ptr(), RTLD_NOW);
            if h.is_null() {
                return None;
            }
            let sym = |n: &CStr| {
                let p = dlsym(h, n.as_ptr());
                if p.is_null() { None } else { Some(p) }
            };
            macro_rules! f {
                ($n:literal, $t:ty) => {
                    std::mem::transmute::<*mut c_void, $t>(sym($n)?)
                };
            }
            Some(Api {
                create_builder: f!(c"AAudio_createStreamBuilder", unsafe extern "C" fn(*mut B) -> i32),
                set_direction: f!(c"AAudioStreamBuilder_setDirection", unsafe extern "C" fn(B, i32)),
                set_rate: f!(c"AAudioStreamBuilder_setSampleRate", unsafe extern "C" fn(B, i32)),
                set_channels: f!(c"AAudioStreamBuilder_setChannelCount", unsafe extern "C" fn(B, i32)),
                set_format: f!(c"AAudioStreamBuilder_setFormat", unsafe extern "C" fn(B, i32)),
                set_sharing: f!(c"AAudioStreamBuilder_setSharingMode", unsafe extern "C" fn(B, i32)),
                set_perf: f!(c"AAudioStreamBuilder_setPerformanceMode", unsafe extern "C" fn(B, i32)),
                set_usage: sym(c"AAudioStreamBuilder_setUsage")
                    .map(|p| std::mem::transmute::<*mut c_void, unsafe extern "C" fn(B, i32)>(p)),
                open: f!(c"AAudioStreamBuilder_openStream", unsafe extern "C" fn(B, *mut S) -> i32),
                delete_builder: f!(c"AAudioStreamBuilder_delete", unsafe extern "C" fn(B) -> i32),
                start: f!(c"AAudioStream_requestStart", unsafe extern "C" fn(S) -> i32),
                write: f!(c"AAudioStream_write", unsafe extern "C" fn(S, *const c_void, i32, i64) -> i32),
                close: f!(c"AAudioStream_close", unsafe extern "C" fn(S) -> i32),
                text: f!(c"AAudio_convertResultToText", unsafe extern "C" fn(i32) -> *const c_char),
            })
        })
        .as_ref()
    }

    fn text(api: &Api, r: i32) -> String {
        unsafe { CStr::from_ptr((api.text)(r)).to_string_lossy().into_owned() }
    }

    /// Engelleyen yazımlı (AAudioStream_write) çıkış akışı: karıştırıcı iş parçacığı ALSA'daki gibi blok blok yazar.
    pub struct AAudioSink {
        api: &'static Api,
        stream: S,
        /// art arda hata sayısı (aygıt kaybında yeniden açma denemeleri sınırlansın)
        fails: u32,
    }

    // akışı yalnızca karıştırıcı iş parçacığı kullanır
    unsafe impl Send for AAudioSink {}

    fn open_stream(api: &Api) -> Result<S, String> {
        unsafe {
            let mut b: B = std::ptr::null_mut();
            let r = (api.create_builder)(&mut b);
            if r != 0 {
                return Err(format!("createStreamBuilder: {}", text(api, r)));
            }
            (api.set_direction)(b, AAUDIO_DIRECTION_OUTPUT);
            (api.set_rate)(b, MIX_RATE as i32);
            (api.set_channels)(b, 2);
            (api.set_format)(b, AAUDIO_FORMAT_PCM_FLOAT);
            (api.set_sharing)(b, AAUDIO_SHARING_MODE_SHARED);
            (api.set_perf)(b, AAUDIO_PERFORMANCE_MODE_NONE);
            if let Some(u) = api.set_usage {
                u(b, AAUDIO_USAGE_GAME);
            }
            let mut s: S = std::ptr::null_mut();
            let r = (api.open)(b, &mut s);
            (api.delete_builder)(b);
            if r != 0 {
                return Err(format!("openStream: {}", text(api, r)));
            }
            let r = (api.start)(s);
            if r != 0 {
                (api.close)(s);
                return Err(format!("requestStart: {}", text(api, r)));
            }
            Ok(s)
        }
    }

    impl AAudioSink {
        pub fn open() -> Result<AAudioSink, String> {
            let api = api().ok_or("libaaudio.so not found")?;
            Ok(AAudioSink { api, stream: open_stream(api)?, fails: 0 })
        }
    }

    impl Sink for AAudioSink {
        fn write(&mut self, interleaved: &[f32]) {
            let total = (interleaved.len() / 2) as i32;
            let mut done = 0i32;
            while done < total {
                if self.stream.is_null() {
                    // aygıt kayboldu ve yeniden açılamadı: gerçek zamanda bekle (karıştırıcı hızını korusun)
                    PacedSilence.write(&interleaved[(done as usize) * 2..]);
                    return;
                }
                let n = unsafe {
                    (self.api.write)(self.stream, interleaved[(done as usize) * 2..].as_ptr() as *const c_void, total - done, 1_000_000_000)
                };
                if n < 0 {
                    // kulaklık takıldı/çıkarıldı gibi aygıt değişimi: akış kullanılamaz, kapatılıp yeniden açılır
                    unsafe { (self.api.close)(self.stream) };
                    self.stream = std::ptr::null_mut();
                    self.fails += 1;
                    if n != AAUDIO_ERROR_DISCONNECTED || self.fails > 5 {
                        eprintln!("audio: AAudio write failed: {}; playing silently", text(self.api, n));
                    }
                    if self.fails <= 5 {
                        match open_stream(self.api) {
                            Ok(s) => {
                                self.stream = s;
                                eprintln!("audio: AAudio stream reopened");
                            }
                            Err(e) => eprintln!("audio: AAudio reopen failed: {e}"),
                        }
                    }
                    continue;
                }
                if n > 0 {
                    self.fails = 0;
                }
                done += n;
            }
        }
    }

    impl Drop for AAudioSink {
        fn drop(&mut self) {
            if !self.stream.is_null() {
                unsafe { (self.api.close)(self.stream) };
            }
        }
    }
}

/// Gerçek zamanda bekleyen sessiz çıkış; böylece karıştırıcı iş parçacığı bir aygıtla aynı hızda ilerler.
pub struct PacedSilence;

impl Sink for PacedSilence {
    fn write(&mut self, interleaved: &[f32]) {
        let frames = interleaved.len() / 2;
        std::thread::sleep(Duration::from_secs_f64(frames as f64 / MIX_RATE as f64));
    }
}

/// Motorun açtığı aygıt: ALSA'da `MONO_AUDIO_DEVICE` (varsayılan "default"), değilse sessiz aygıt.
/// Çıkışı ve hangisinin kullanıldığını söyleyen bir satırı döndürür (bir kez yazdırılır).
pub fn open_device() -> (Box<dyn Sink + Send>, String) {
    let want = std::env::var("MONO_AUDIO_DEVICE").unwrap_or_else(|_| "default".to_string());
    if std::env::var_os("MONO_AUDIO_SILENT").is_some() {
        return (Box::new(PacedSilence), "audio: silent output (MONO_AUDIO_SILENT)".to_string());
    }
    #[cfg(target_os = "linux")]
    {
        match AlsaSink::open(&want) {
            Ok(s) => {
                let msg = format!("audio: ALSA device {}", s.device);
                (Box::new(s), msg)
            }
            Err(e) => (Box::new(PacedSilence), format!("audio: no output device ({e}); playing silently")),
        }
    }
    #[cfg(windows)]
    {
        let _ = want;
        match crate::audio_wasapi::WasapiSink::open() {
            Ok(s) => (Box::new(s), "audio: WASAPI default output".to_string()),
            Err(e) => (Box::new(PacedSilence), format!("audio: no output device ({e}); playing silently")),
        }
    }
    #[cfg(target_os = "android")]
    {
        let _ = want;
        match aaudio::AAudioSink::open() {
            Ok(s) => (Box::new(s), "audio: AAudio output".to_string()),
            Err(e) => (Box::new(PacedSilence), format!("audio: no output device ({e}); playing silently")),
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "android", windows)))]
    {
        let _ = want;
        (Box::new(PacedSilence), "audio: this platform has no output device (playing silently)".to_string())
    }
}

// ----- motor (süreç başına bir tane)

pub struct Engine {
    pub ctx: Arc<Mutex<Context>>,
    stop: Arc<AtomicBool>,
}

static ENGINE: Mutex<Option<Engine>> = Mutex::new(None);
static CONTEXT: OnceLock<Arc<Mutex<Context>>> = OnceLock::new();

/// Paylaşılan OpenAL uyumlu bağlam (ilk kullanımda oluşturulur; bir aygıt açıkken motor onu çizer).
pub fn context() -> Arc<Mutex<Context>> {
    CONTEXT.get_or_init(|| Arc::new(Mutex::new(Context::new()))).clone()
}

/// Çıkış aygıtını açar ve karıştırıcı iş parçacığını başlatır. Açıkken tekrar çağrılması bir şey yapmaz (idempotent).
pub fn start() {
    let mut e = ENGINE.lock().unwrap();
    if e.is_some() {
        return;
    }
    let ctx = context();
    let stop = Arc::new(AtomicBool::new(false));
    let (mut sink, msg) = open_device();
    eprintln!("{msg}");
    let (c2, s2) = (ctx.clone(), stop.clone());
    std::thread::Builder::new()
        .name("mono-audio".into())
        .spawn(move || {
            let mut block = vec![0.0f32; BLOCK_FRAMES * 2];
            while !s2.load(Ordering::Acquire) {
                c2.lock().unwrap().mix(&mut block);
                sink.write(&block);
            }
        })
        .expect("audio thread");
    *e = Some(Engine { ctx, stop });
}

/// Karıştırıcı iş parçacığını durdurur (iş parçacığı bitince aygıt serbest bırakılır).
pub fn stop() {
    if let Some(e) = ENGINE.lock().unwrap().take() {
        e.stop.store(true, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn alsa_null_device_accepts_frames() {
        // "null" PCM her alsa-lib kurulumunda vardır; libasound yoksa test atlanır
        if alsa::alsa().is_none() {
            return;
        }
        let mut s = AlsaSink::open("null").expect("null pcm");
        s.write(&vec![0.25f32; 2 * 4410]);
    }

    #[test]
    fn paced_silence_takes_real_time() {
        let t = std::time::Instant::now();
        let mut s = PacedSilence;
        s.write(&vec![0.0f32; 2 * 4410]); // 0.1 s of audio
        assert!(t.elapsed() >= Duration::from_millis(90));
    }
}
