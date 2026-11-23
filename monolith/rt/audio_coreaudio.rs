//! macOS ses çıkışı: AudioQueue (AudioToolbox), MIX_RATE hızında float32 stereo. Mikser iş parçacığı blokları bir
//! halka tampona yazar; kuyruğun geri çağrısı halkayı donanımın sıradaki çalacağı tampona taşır ve mikser geride
//! kalırsa kalanı sessizlikle doldurur.

#![cfg(target_os = "macos")]

use crate::audio::{Sink, MIX_RATE};
use std::collections::VecDeque;
use std::ffi::c_void;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

#[repr(C)]
struct AudioStreamBasicDescription {
    sample_rate: f64,
    format_id: u32,
    format_flags: u32,
    bytes_per_packet: u32,
    frames_per_packet: u32,
    bytes_per_frame: u32,
    channels_per_frame: u32,
    bits_per_channel: u32,
    reserved: u32,
}

#[repr(C)]
struct AudioQueueBuffer {
    capacity_bytes: u32,
    audio_data: *mut c_void,
    byte_size: u32,
    user_data: *mut c_void,
    packet_capacity: u32,
    packet_descriptions: *mut c_void,
    packet_count: u32,
}

type OutputCallback = extern "C" fn(*mut c_void, *mut c_void, *mut AudioQueueBuffer);

#[link(name = "AudioToolbox", kind = "framework")]
extern "C" {
    fn AudioQueueNewOutput(
        format: *const AudioStreamBasicDescription, callback: OutputCallback, user: *mut c_void,
        run_loop: *mut c_void, run_loop_mode: *mut c_void, flags: u32, queue: *mut *mut c_void,
    ) -> i32;
    fn AudioQueueAllocateBuffer(queue: *mut c_void, bytes: u32, buffer: *mut *mut AudioQueueBuffer) -> i32;
    fn AudioQueueEnqueueBuffer(queue: *mut c_void, buffer: *mut AudioQueueBuffer, packets: u32, descs: *const c_void) -> i32;
    fn AudioQueueStart(queue: *mut c_void, start_time: *const c_void) -> i32;
    fn AudioQueueStop(queue: *mut c_void, immediate: u8) -> i32;
    fn AudioQueueDispose(queue: *mut c_void, immediate: u8) -> i32;
}

const LINEAR_PCM: u32 = 0x6C70_636D; // 'lpcm'
const FLOAT_PACKED: u32 = 1 | 8; // kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked
const BUFFERS: usize = 3;
const BUFFER_FRAMES: usize = 1024;
const FRAME_BYTES: u32 = 8; // iki float32 örnek
const BUFFER_BYTES: u32 = BUFFER_FRAMES as u32 * FRAME_BYTES;
/// Halka yaklaşık üç donanım tamponu kadar ses tutar; dolunca mikser bekler.
const RING_SAMPLES: usize = BUFFERS * BUFFER_FRAMES * 2;

struct Shared {
    ring: Mutex<VecDeque<f32>>,
    space: Condvar,
}

/// Kuyruktaki örnekleri `buffer`a taşır (halka boşsa sessizlik) ve tamponu kuyruğa geri verir.
fn fill(shared: &Shared, queue: *mut c_void, buffer: *mut AudioQueueBuffer) {
    unsafe {
        let b = &mut *buffer;
        let out = std::slice::from_raw_parts_mut(b.audio_data as *mut f32, BUFFER_FRAMES * 2);
        {
            let mut ring = shared.ring.lock().unwrap();
            let n = ring.len().min(out.len());
            for (i, s) in ring.drain(..n).enumerate() {
                out[i] = s;
            }
            for s in &mut out[n..] {
                *s = 0.0;
            }
            shared.space.notify_all();
        }
        b.byte_size = BUFFER_BYTES;
        AudioQueueEnqueueBuffer(queue, buffer, 0, std::ptr::null());
    }
}

extern "C" fn output_callback(user: *mut c_void, queue: *mut c_void, buffer: *mut AudioQueueBuffer) {
    let shared = unsafe { &*(user as *const Shared) };
    fill(shared, queue, buffer);
}

pub struct AudioQueueSink {
    queue: *mut c_void,
    shared: Arc<Shared>,
    user: *mut c_void,
}

// kuyruk ve halka, kilit altında mikser iş parçacığı (yazma) ve kuyruk iş parçacığı (geri çağrı) tarafından kullanılır
unsafe impl Send for AudioQueueSink {}

impl AudioQueueSink {
    /// Varsayılan çıkışı MIX_RATE, float32 stereo olarak açar ve kuyruğu başlatır.
    pub fn open() -> Result<AudioQueueSink, String> {
        let shared = Arc::new(Shared { ring: Mutex::new(VecDeque::with_capacity(RING_SAMPLES)), space: Condvar::new() });
        // geri çağrı bu ham başvuruyu alır; kuyruk yok edildikten sonra Drop içinde serbest bırakılır
        let user = Arc::into_raw(shared.clone()) as *mut c_void;
        let format = AudioStreamBasicDescription {
            sample_rate: MIX_RATE as f64,
            format_id: LINEAR_PCM,
            format_flags: FLOAT_PACKED,
            bytes_per_packet: FRAME_BYTES,
            frames_per_packet: 1,
            bytes_per_frame: FRAME_BYTES,
            channels_per_frame: 2,
            bits_per_channel: 32,
            reserved: 0,
        };
        unsafe {
            let mut queue: *mut c_void = std::ptr::null_mut();
            let status = AudioQueueNewOutput(&format, output_callback, user, std::ptr::null_mut(), std::ptr::null_mut(), 0, &mut queue);
            if status != 0 {
                drop(Arc::from_raw(user as *const Shared));
                return Err(format!("AudioQueueNewOutput: OSStatus {status}"));
            }
            // tamponlar başlamadan önce sessizlikle doldurulup kuyruğa alınır; böylece ilk geri çağrının verisi vardır
            for _ in 0..BUFFERS {
                let mut buffer: *mut AudioQueueBuffer = std::ptr::null_mut();
                let status = AudioQueueAllocateBuffer(queue, BUFFER_BYTES, &mut buffer);
                if status != 0 {
                    AudioQueueDispose(queue, 1);
                    drop(Arc::from_raw(user as *const Shared));
                    return Err(format!("AudioQueueAllocateBuffer: OSStatus {status}"));
                }
                fill(&shared, queue, buffer);
            }
            let status = AudioQueueStart(queue, std::ptr::null());
            if status != 0 {
                AudioQueueDispose(queue, 1);
                drop(Arc::from_raw(user as *const Shared));
                return Err(format!("AudioQueueStart: OSStatus {status}"));
            }
            Ok(AudioQueueSink { queue, shared, user })
        }
    }
}

impl Sink for AudioQueueSink {
    fn write(&mut self, interleaved: &[f32]) {
        let mut ring = self.shared.ring.lock().unwrap();
        // yer açılmasını bekle; tüketmeyi bırakan aygıt mikseri sonsuza dek durdurmamalı
        let mut waited = 0;
        while ring.len() + interleaved.len() > RING_SAMPLES && waited < 50 {
            let (guard, timeout) = self.shared.space.wait_timeout(ring, Duration::from_millis(10)).unwrap();
            ring = guard;
            if timeout.timed_out() {
                waited += 1;
            }
        }
        if ring.len() + interleaved.len() > RING_SAMPLES {
            ring.clear(); // aygıt durdu; blok atılır, mikser temposunu korur
            eprintln!("audio: output stopped consuming samples");
        }
        ring.extend(interleaved.iter().copied());
    }
}

impl Drop for AudioQueueSink {
    fn drop(&mut self) {
        unsafe {
            AudioQueueStop(self.queue, 1);
            AudioQueueDispose(self.queue, 1);
            drop(Arc::from_raw(self.user as *const Shared));
        }
    }
}
