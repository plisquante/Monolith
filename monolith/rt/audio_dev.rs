//! Audio output for the mixer in audio.rs, and the process-wide audio engine behind the OpenAL-compatible natives.
//!
//! Devices: ALSA on Linux (libasound.so.2 opened at run time with dlopen; nothing is linked or needed at build time),
//! and a silent device that paces itself in real time when no output exists (no sound card, or a server without audio).
//! Windows (WASAPI), macOS (CoreAudio) and Android (AAudio) devices are separate work (docs/IMPL_SPEC.md, B3).
//!
//! The engine is one mixer thread that renders blocks from the shared `audio::Context` into the device. The game
//! thread changes the context through the natives; a short lock per block is the only synchronisation.

use crate::audio::{Context, Sink, MIX_RATE};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

const BLOCK_FRAMES: usize = 1024;

#[cfg(target_os = "linux")]
mod alsa {
    use super::*;
    use std::ffi::{c_char, c_int, c_long, c_uint, c_void, CStr, CString};

    // ----- ALSA (libasound.so.2), the few calls needed for interleaved S16 playback
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
                open: std::mem::transmute(sym(c"snd_pcm_open")?),
                set_params: std::mem::transmute(sym(c"snd_pcm_set_params")?),
                writei: std::mem::transmute(sym(c"snd_pcm_writei")?),
                recover: std::mem::transmute(sym(c"snd_pcm_recover")?),
                close: std::mem::transmute(sym(c"snd_pcm_close")?),
                strerror: std::mem::transmute(sym(c"snd_strerror")?),
            })
        })
        .as_ref()
    }

    pub struct AlsaSink {
        api: &'static Alsa,
        pcm: *mut c_void,
        pub device: String,
    }

    // the handle is used by the mixer thread only
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
                // soft resampling on, 50 ms latency
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
                    // underrun (-EPIPE) or suspend: recover and retry; anything else is dropped after one recover
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

/// Silent output that waits in real time, so the mixer thread keeps the same pace as a device.
pub struct PacedSilence;

impl Sink for PacedSilence {
    fn write(&mut self, interleaved: &[f32]) {
        let frames = interleaved.len() / 2;
        std::thread::sleep(Duration::from_secs_f64(frames as f64 / MIX_RATE as f64));
    }
}

/// The device the engine opens: `MONO_AUDIO_DEVICE` (default "default") on ALSA, otherwise the silent device.
/// Returns the sink and a line that says what is in use (printed once).
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
    #[cfg(not(target_os = "linux"))]
    {
        let _ = want;
        (Box::new(PacedSilence), "audio: this platform has no output device yet (Windows WASAPI and macOS CoreAudio are not written); playing silently".to_string())
    }
}

// ----- the engine (one per process)

pub struct Engine {
    pub ctx: Arc<Mutex<Context>>,
    stop: Arc<AtomicBool>,
}

static ENGINE: Mutex<Option<Engine>> = Mutex::new(None);
static CONTEXT: OnceLock<Arc<Mutex<Context>>> = OnceLock::new();

/// The shared OpenAL-compatible context (created on first use; the engine renders it once a device is open).
pub fn context() -> Arc<Mutex<Context>> {
    CONTEXT.get_or_init(|| Arc::new(Mutex::new(Context::new()))).clone()
}

/// Opens the output device and starts the mixer thread. Idempotent while open.
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

/// Stops the mixer thread (the device is released when the thread ends).
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
        // the "null" PCM exists in every alsa-lib install; a missing libasound skips the test
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
