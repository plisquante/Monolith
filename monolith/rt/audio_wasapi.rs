//! Windows ses çıkışı: varsayılan oynatma uç noktasında WASAPI paylaşımlı modu; COM vtable'ları üzerinden çağrılır.
//! Mikser MIX_RATE hızında float32 stereo yazar; AUTOCONVERTPCM ve varsayılan örnek hızı dönüştürücüsü, ses motorunun
//! aygıtın karıştırdığı biçime çevirmesini sağlar.

#![cfg(windows)]

use crate::audio::{Sink, MIX_RATE};
use std::ffi::c_void;
use std::time::Duration;

#[repr(C)]
#[derive(Clone, Copy)]
struct Guid {
    d1: u32,
    d2: u16,
    d3: u16,
    d4: [u8; 8],
}

const CLSID_MM_DEVICE_ENUMERATOR: Guid =
    Guid { d1: 0xBCDE_0395, d2: 0xE52F, d3: 0x467C, d4: [0x8E, 0x3D, 0xC4, 0x57, 0x92, 0x91, 0x69, 0x2E] };
const IID_IMM_DEVICE_ENUMERATOR: Guid =
    Guid { d1: 0xA956_64D2, d2: 0x9614, d3: 0x4F35, d4: [0xA7, 0x46, 0xDE, 0x8D, 0xB6, 0x36, 0x17, 0xE6] };
const IID_IAUDIO_CLIENT: Guid =
    Guid { d1: 0x1CB9_AD4C, d2: 0xDBFA, d3: 0x4C32, d4: [0xB1, 0x78, 0xC2, 0xF5, 0x68, 0xA7, 0x03, 0xB2] };
const IID_IAUDIO_RENDER_CLIENT: Guid =
    Guid { d1: 0xF294_ACFC, d2: 0x3146, d3: 0x4483, d4: [0xA7, 0xBF, 0xAD, 0xDC, 0xA7, 0xC2, 0x60, 0xE2] };

#[repr(C)]
struct WaveFormatEx {
    w_format_tag: u16,
    n_channels: u16,
    n_samples_per_sec: u32,
    n_avg_bytes_per_sec: u32,
    n_block_align: u16,
    w_bits_per_sample: u16,
    cb_size: u16,
}

#[link(name = "ole32")]
extern "system" {
    fn CoInitializeEx(reserved: *mut c_void, co_init: u32) -> i32;
    fn CoCreateInstance(clsid: *const Guid, outer: *mut c_void, ctx: u32, iid: *const Guid, out: *mut *mut c_void) -> i32;
}

const COINIT_MULTITHREADED: u32 = 0x0;
const CLSCTX_ALL: u32 = 0x17;
const E_RENDER: u32 = 0; // EDataFlow.eRender (oynatma)
const E_CONSOLE: u32 = 0; // ERole.eConsole (konsol)
const SHARED: u32 = 0; // AUDCLNT_SHAREMODE_SHARED (paylaşımlı mod)
const STREAM_AUTOCONVERTPCM: u32 = 0x8000_0000;
const STREAM_SRC_DEFAULT_QUALITY: u32 = 0x0800_0000;
const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;
/// 100 ms tampon, 100 ns birimiyle.
const BUFFER_DURATION: i64 = 1_000_000;

// vtable yuvaları (IUnknown: 0 QueryInterface, 1 AddRef, 2 Release)
const SLOT_RELEASE: usize = 2;
const ENUM_GET_DEFAULT_ENDPOINT: usize = 4; // IMMDeviceEnumerator
const DEVICE_ACTIVATE: usize = 3; // IMMDevice
const CLIENT_INITIALIZE: usize = 3; // IAudioClient
const CLIENT_GET_BUFFER_SIZE: usize = 4;
const CLIENT_GET_CURRENT_PADDING: usize = 6;
const CLIENT_START: usize = 10;
const CLIENT_STOP: usize = 11;
const CLIENT_GET_SERVICE: usize = 14;
const RENDER_GET_BUFFER: usize = 3; // IAudioRenderClient
const RENDER_RELEASE_BUFFER: usize = 4;

type ComFn = *const c_void;

unsafe fn slot(obj: *mut c_void, index: usize) -> ComFn {
    let vtbl = *(obj as *const *const ComFn);
    *vtbl.add(index)
}

unsafe fn release(obj: *mut c_void) {
    if !obj.is_null() {
        let f: extern "system" fn(*mut c_void) -> u32 = std::mem::transmute(slot(obj, SLOT_RELEASE));
        f(obj);
    }
}

fn check(hr: i32, what: &str) -> Result<(), String> {
    if hr < 0 {
        Err(format!("{what}: HRESULT 0x{:08X}", hr as u32))
    } else {
        Ok(())
    }
}

pub struct WasapiSink {
    client: *mut c_void,
    render: *mut c_void,
    buffer_frames: u32,
}

// COM nesneleri iş parçacığından bağımsızdır (WASAPI); bu çıkışı yalnızca mikser iş parçacığı kullanır
unsafe impl Send for WasapiSink {}

impl WasapiSink {
    /// Varsayılan çıkış aygıtını MIX_RATE, float32 stereo, paylaşımlı modda açar.
    pub fn open() -> Result<WasapiSink, String> {
        unsafe {
            // S_FALSE ve RPC_E_CHANGED_MODE, iş parçacığının zaten başlatıldığı anlamına gelir; ikisi de sorun değil
            let _ = CoInitializeEx(std::ptr::null_mut(), COINIT_MULTITHREADED);

            let mut enumerator: *mut c_void = std::ptr::null_mut();
            check(
                CoCreateInstance(&CLSID_MM_DEVICE_ENUMERATOR, std::ptr::null_mut(), CLSCTX_ALL, &IID_IMM_DEVICE_ENUMERATOR, &mut enumerator),
                "device enumerator",
            )?;
            let get_default: extern "system" fn(*mut c_void, u32, u32, *mut *mut c_void) -> i32 =
                std::mem::transmute(slot(enumerator, ENUM_GET_DEFAULT_ENDPOINT));
            let mut device: *mut c_void = std::ptr::null_mut();
            let hr = get_default(enumerator, E_RENDER, E_CONSOLE, &mut device);
            release(enumerator);
            check(hr, "default output endpoint")?;
            if device.is_null() {
                return Err("default output endpoint: none".to_string());
            }

            let activate: extern "system" fn(*mut c_void, *const Guid, u32, *mut c_void, *mut *mut c_void) -> i32 =
                std::mem::transmute(slot(device, DEVICE_ACTIVATE));
            let mut client: *mut c_void = std::ptr::null_mut();
            let hr = activate(device, &IID_IAUDIO_CLIENT, CLSCTX_ALL, std::ptr::null_mut(), &mut client);
            release(device);
            check(hr, "activate audio client")?;

            let fmt = WaveFormatEx {
                w_format_tag: WAVE_FORMAT_IEEE_FLOAT,
                n_channels: 2,
                n_samples_per_sec: MIX_RATE,
                n_avg_bytes_per_sec: MIX_RATE * 8,
                n_block_align: 8,
                w_bits_per_sample: 32,
                cb_size: 0,
            };
            let initialize: extern "system" fn(*mut c_void, u32, u32, i64, i64, *const WaveFormatEx, *const c_void) -> i32 =
                std::mem::transmute(slot(client, CLIENT_INITIALIZE));
            let hr = initialize(
                client, SHARED, STREAM_AUTOCONVERTPCM | STREAM_SRC_DEFAULT_QUALITY, BUFFER_DURATION, 0, &fmt, std::ptr::null(),
            );
            if hr < 0 {
                release(client);
                return Err(format!("initialize: HRESULT 0x{:08X}", hr as u32));
            }

            let get_size: extern "system" fn(*mut c_void, *mut u32) -> i32 = std::mem::transmute(slot(client, CLIENT_GET_BUFFER_SIZE));
            let mut buffer_frames = 0u32;
            if let Err(e) = check(get_size(client, &mut buffer_frames), "buffer size") {
                release(client);
                return Err(e);
            }

            let get_service: extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> i32 =
                std::mem::transmute(slot(client, CLIENT_GET_SERVICE));
            let mut render: *mut c_void = std::ptr::null_mut();
            if let Err(e) = check(get_service(client, &IID_IAUDIO_RENDER_CLIENT, &mut render), "render service") {
                release(client);
                return Err(e);
            }

            let start: extern "system" fn(*mut c_void) -> i32 = std::mem::transmute(slot(client, CLIENT_START));
            if let Err(e) = check(start(client), "start") {
                release(render);
                release(client);
                return Err(e);
            }
            Ok(WasapiSink { client, render, buffer_frames })
        }
    }

    fn padding(&self) -> Result<u32, String> {
        unsafe {
            let f: extern "system" fn(*mut c_void, *mut u32) -> i32 = std::mem::transmute(slot(self.client, CLIENT_GET_CURRENT_PADDING));
            let mut p = 0u32;
            check(f(self.client, &mut p), "current padding")?;
            Ok(p)
        }
    }
}

impl Sink for WasapiSink {
    fn write(&mut self, interleaved: &[f32]) {
        let total = interleaved.len() / 2;
        let mut done = 0usize;
        while done < total {
            let padding = match self.padding() {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("audio: {e}");
                    return;
                }
            };
            let free = self.buffer_frames.saturating_sub(padding) as usize;
            if free == 0 {
                // motor kuyruktaki kareleri gerçek zamanlı çalar; yer açılmasını bekle
                std::thread::sleep(Duration::from_millis(2));
                continue;
            }
            let n = free.min(total - done);
            unsafe {
                let get_buffer: extern "system" fn(*mut c_void, u32, *mut *mut u8) -> i32 =
                    std::mem::transmute(slot(self.render, RENDER_GET_BUFFER));
                let mut data: *mut u8 = std::ptr::null_mut();
                let hr = get_buffer(self.render, n as u32, &mut data);
                if hr < 0 || data.is_null() {
                    eprintln!("audio: GetBuffer failed: HRESULT 0x{:08X}", hr as u32);
                    return;
                }
                std::ptr::copy_nonoverlapping(interleaved[2 * done..].as_ptr() as *const u8, data, n * 8);
                let release_buffer: extern "system" fn(*mut c_void, u32, u32) -> i32 =
                    std::mem::transmute(slot(self.render, RENDER_RELEASE_BUFFER));
                release_buffer(self.render, n as u32, 0);
            }
            done += n;
        }
    }
}

impl Drop for WasapiSink {
    fn drop(&mut self) {
        unsafe {
            let stop: extern "system" fn(*mut c_void) -> i32 = std::mem::transmute(slot(self.client, CLIENT_STOP));
            stop(self.client);
            release(self.render);
            release(self.client);
        }
    }
}
