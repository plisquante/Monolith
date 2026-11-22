//! OpenAL-compatible software mixer (the subset Minecraft uses), in Rust, without dependencies.
//!
//! Model: buffers hold decoded PCM; sources play a queue of buffers with gain, pitch, looping and a 3D position
//! (inverse-distance-clamped attenuation, the OpenAL default). `mix` renders the sources into an interleaved stereo
//! f32 block; an output device (a `Sink`) takes the blocks. Errors and states follow the OpenAL 1.1 specification.
//!
//! This file has no crate dependencies and is compiled both into the runtime (`mod audio` in lib.rs) and on its own
//! for the tests: `rustc --test monolith/rt/audio.rs -o audio_tests && ./audio_tests`.

#![allow(dead_code)]

use std::collections::{HashMap, VecDeque};

// ----- OpenAL constants (values from al.h / alc.h, 1.1)
pub const AL_NO_ERROR: i32 = 0;
pub const AL_INVALID_NAME: i32 = 0xA001;
pub const AL_INVALID_ENUM: i32 = 0xA002;
pub const AL_INVALID_VALUE: i32 = 0xA003;
pub const AL_INVALID_OPERATION: i32 = 0xA004;
pub const AL_FALSE: i32 = 0;
pub const AL_TRUE: i32 = 1;
pub const AL_POSITION: i32 = 0x1004;
pub const AL_GAIN: i32 = 0x100A;
pub const AL_PITCH: i32 = 0x1003;
pub const AL_LOOPING: i32 = 0x1007;
pub const AL_BUFFER: i32 = 0x1009;
pub const AL_SOURCE_STATE: i32 = 0x1010;
pub const AL_INITIAL: i32 = 0x1011;
pub const AL_PLAYING: i32 = 0x1012;
pub const AL_PAUSED: i32 = 0x1013;
pub const AL_STOPPED: i32 = 0x1014;
pub const AL_BUFFERS_QUEUED: i32 = 0x1015;
pub const AL_BUFFERS_PROCESSED: i32 = 0x1016;
pub const AL_FORMAT_MONO8: i32 = 0x1100;
pub const AL_FORMAT_MONO16: i32 = 0x1101;
pub const AL_FORMAT_STEREO8: i32 = 0x1102;
pub const AL_FORMAT_STEREO16: i32 = 0x1103;

/// Output rate of the mixer. Buffers at other rates are resampled (linear interpolation).
pub const MIX_RATE: u32 = 44_100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Mono8,
    Mono16,
    Stereo8,
    Stereo16,
}

impl Format {
    pub fn from_al(f: i32) -> Option<Format> {
        match f {
            AL_FORMAT_MONO8 => Some(Format::Mono8),
            AL_FORMAT_MONO16 => Some(Format::Mono16),
            AL_FORMAT_STEREO8 => Some(Format::Stereo8),
            AL_FORMAT_STEREO16 => Some(Format::Stereo16),
            _ => None,
        }
    }
    fn channels(self) -> usize {
        match self {
            Format::Mono8 | Format::Mono16 => 1,
            _ => 2,
        }
    }
}

pub struct Buffer {
    pub format: Format,
    pub freq: u32,
    /// decoded frames, stereo interleaved (mono is duplicated), f32 in [-1, 1]
    pub frames: Vec<f32>,
    /// the number of sources that have this buffer queued or current (a buffer may not be deleted while in use)
    pub in_use: u32,
}

impl Buffer {
    fn frame_count(&self) -> usize {
        self.frames.len() / 2
    }
}

/// Decodes PCM bytes of an AL buffer into stereo f32 frames.
pub fn decode(format: Format, freq: u32, data: &[u8]) -> Buffer {
    let mut frames = Vec::new();
    match format {
        Format::Mono8 => {
            for &b in data {
                let v = (b as f32 - 128.0) / 128.0;
                frames.push(v);
                frames.push(v);
            }
        }
        Format::Mono16 => {
            for c in data.chunks_exact(2) {
                let v = i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0;
                frames.push(v);
                frames.push(v);
            }
        }
        Format::Stereo8 => {
            for &b in data {
                frames.push((b as f32 - 128.0) / 128.0);
            }
        }
        Format::Stereo16 => {
            for c in data.chunks_exact(2) {
                frames.push(i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0);
            }
        }
    }
    Buffer { format, freq, frames, in_use: 0 }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    Initial,
    Playing,
    Paused,
    Stopped,
}

pub struct Source {
    pub state: State,
    pub queue: VecDeque<u32>,
    /// the buffer currently rendered (front of `queue`) and the position inside it, in frames of the buffer
    pub pos: f64,
    /// number of buffers that finished playing and were not unqueued yet (AL_BUFFERS_PROCESSED)
    pub processed: VecDeque<u32>,
    pub gain: f32,
    pub pitch: f32,
    pub looping: bool,
    pub position: [f32; 3],
}

impl Source {
    fn new() -> Source {
        Source {
            state: State::Initial,
            queue: VecDeque::new(),
            pos: 0.0,
            processed: VecDeque::new(),
            gain: 1.0,
            pitch: 1.0,
            looping: false,
            position: [0.0, 0.0, 0.0],
        }
    }
}

#[derive(Default)]
pub struct Context {
    pub buffers: HashMap<u32, Buffer>,
    pub sources: HashMap<u32, Source>,
    pub listener_pos: [f32; 3],
    pub listener_gain: f32,
    /// error of the last failed call (AL_NO_ERROR when none); `error()` returns it and clears it
    err: i32,
    next_id: u32,
}

impl Context {
    pub fn new() -> Context {
        Context { listener_gain: 1.0, next_id: 1, ..Default::default() }
    }

    fn fail(&mut self, code: i32) -> i32 {
        if self.err == AL_NO_ERROR {
            self.err = code;
        }
        code
    }

    pub fn error(&mut self) -> i32 {
        std::mem::replace(&mut self.err, AL_NO_ERROR)
    }

    fn id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    // ----- buffers
    pub fn gen_buffers(&mut self, n: usize) -> Vec<u32> {
        (0..n)
            .map(|_| {
                let id = self.id();
                self.buffers.insert(id, Buffer { format: Format::Mono8, freq: 0, frames: Vec::new(), in_use: 0 });
                id
            })
            .collect()
    }

    pub fn buffer_data(&mut self, id: u32, format: i32, data: &[u8], freq: i32) -> i32 {
        let Some(fmt) = Format::from_al(format) else { return self.fail(AL_INVALID_ENUM) };
        if freq <= 0 {
            return self.fail(AL_INVALID_VALUE);
        }
        let in_use = match self.buffers.get(&id) {
            Some(b) => b.in_use,
            None => return self.fail(AL_INVALID_NAME),
        };
        if in_use > 0 {
            return self.fail(AL_INVALID_OPERATION);
        }
        let mut b = decode(fmt, freq as u32, data);
        b.in_use = 0;
        self.buffers.insert(id, b);
        AL_NO_ERROR
    }

    pub fn delete_buffers(&mut self, ids: &[u32]) -> i32 {
        for &id in ids {
            match self.buffers.get(&id) {
                None => return self.fail(AL_INVALID_NAME),
                Some(b) if b.in_use > 0 => return self.fail(AL_INVALID_OPERATION),
                _ => {}
            }
        }
        for &id in ids {
            self.buffers.remove(&id);
        }
        AL_NO_ERROR
    }

    // ----- sources
    pub fn gen_sources(&mut self, n: usize) -> Vec<u32> {
        (0..n)
            .map(|_| {
                let id = self.id();
                self.sources.insert(id, Source::new());
                id
            })
            .collect()
    }

    pub fn delete_sources(&mut self, ids: &[u32]) -> i32 {
        for &id in ids {
            if !self.sources.contains_key(&id) {
                return self.fail(AL_INVALID_NAME);
            }
        }
        for &id in ids {
            if let Some(s) = self.sources.remove(&id) {
                for b in s.queue.iter().chain(s.processed.iter()) {
                    if let Some(bb) = self.buffers.get_mut(b) {
                        bb.in_use = bb.in_use.saturating_sub(1);
                    }
                }
            }
        }
        AL_NO_ERROR
    }

    pub fn queue_buffers(&mut self, src: u32, bufs: &[u32]) -> i32 {
        if !self.sources.contains_key(&src) {
            return self.fail(AL_INVALID_NAME);
        }
        for b in bufs {
            if !self.buffers.contains_key(b) {
                return self.fail(AL_INVALID_NAME);
            }
        }
        for &b in bufs {
            self.buffers.get_mut(&b).unwrap().in_use += 1;
            self.sources.get_mut(&src).unwrap().queue.push_back(b);
        }
        AL_NO_ERROR
    }

    /// Removes buffers that finished playing (AL_BUFFERS_PROCESSED); returns them in queue order.
    pub fn unqueue_buffers(&mut self, src: u32, count: usize) -> Result<Vec<u32>, i32> {
        let s = match self.sources.get_mut(&src) {
            Some(s) => s,
            None => return Err(self.fail(AL_INVALID_NAME)),
        };
        if count > s.processed.len() {
            return Err(self.fail(AL_INVALID_VALUE));
        }
        let out: Vec<u32> = s.processed.drain(..count).collect();
        for b in &out {
            if let Some(bb) = self.buffers.get_mut(b) {
                bb.in_use = bb.in_use.saturating_sub(1);
            }
        }
        Ok(out)
    }

    pub fn play(&mut self, src: u32) -> i32 {
        match self.sources.get_mut(&src) {
            None => self.fail(AL_INVALID_NAME),
            Some(s) => {
                if s.queue.is_empty() && s.state != State::Paused {
                    return AL_NO_ERROR;
                }
                if s.state == State::Initial || s.state == State::Stopped {
                    s.pos = 0.0;
                }
                s.state = State::Playing;
                AL_NO_ERROR
            }
        }
    }

    pub fn pause(&mut self, src: u32) -> i32 {
        match self.sources.get_mut(&src) {
            None => self.fail(AL_INVALID_NAME),
            Some(s) => {
                if s.state == State::Playing {
                    s.state = State::Paused;
                }
                AL_NO_ERROR
            }
        }
    }

    pub fn stop(&mut self, src: u32) -> i32 {
        let Some(s) = self.sources.get_mut(&src) else { return self.fail(AL_INVALID_NAME) };
        if s.state == State::Playing || s.state == State::Paused {
            s.state = State::Stopped;
            // AL: stop keeps the queue; everything that was played is processed
            let done: Vec<u32> = s.queue.drain(..).collect();
            for b in done {
                s.processed.push_back(b);
            }
            s.pos = 0.0;
        }
        AL_NO_ERROR
    }

    pub fn get_source_i(&self, src: u32, param: i32) -> Result<i32, i32> {
        let Some(s) = self.sources.get(&src) else { return Err(AL_INVALID_NAME) };
        Ok(match param {
            AL_SOURCE_STATE => match s.state {
                State::Initial => AL_INITIAL,
                State::Playing => AL_PLAYING,
                State::Paused => AL_PAUSED,
                State::Stopped => AL_STOPPED,
            },
            AL_BUFFERS_QUEUED => s.queue.len() as i32 + s.processed.len() as i32,
            AL_BUFFERS_PROCESSED => s.processed.len() as i32,
            AL_LOOPING => s.looping as i32,
            AL_BUFFER => s.queue.front().or(s.processed.front()).copied().unwrap_or(0) as i32,
            _ => return Err(AL_INVALID_ENUM),
        })
    }

    pub fn set_source_i(&mut self, src: u32, param: i32, value: i32) -> i32 {
        let Some(s) = self.sources.get_mut(&src) else { return self.fail(AL_INVALID_NAME) };
        match param {
            AL_LOOPING => s.looping = value != AL_FALSE,
            AL_BUFFER => {
                if value != 0 && !self.buffers.contains_key(&(value as u32)) {
                    return self.fail(AL_INVALID_VALUE);
                }
                let s = self.sources.get_mut(&src).unwrap();
                if s.state == State::Playing || s.state == State::Paused {
                    return self.fail(AL_INVALID_OPERATION);
                }
                let old: Vec<u32> = s.queue.drain(..).chain(s.processed.drain(..)).collect();
                for b in old {
                    if let Some(bb) = self.buffers.get_mut(&b) {
                        bb.in_use = bb.in_use.saturating_sub(1);
                    }
                }
                if value != 0 {
                    self.queue_buffers(src, &[value as u32]);
                }
                let s = self.sources.get_mut(&src).unwrap();
                s.state = State::Initial;
                s.pos = 0.0;
            }
            _ => return self.fail(AL_INVALID_ENUM),
        }
        AL_NO_ERROR
    }

    pub fn set_source_f(&mut self, src: u32, param: i32, value: f32) -> i32 {
        let Some(s) = self.sources.get_mut(&src) else { return self.fail(AL_INVALID_NAME) };
        match param {
            AL_GAIN => {
                if value < 0.0 {
                    return self.fail(AL_INVALID_VALUE);
                }
                s.gain = value;
            }
            AL_PITCH => {
                if value <= 0.0 {
                    return self.fail(AL_INVALID_VALUE);
                }
                s.pitch = value;
            }
            _ => return self.fail(AL_INVALID_ENUM),
        }
        AL_NO_ERROR
    }

    pub fn set_source_3f(&mut self, src: u32, param: i32, v: [f32; 3]) -> i32 {
        let Some(s) = self.sources.get_mut(&src) else { return self.fail(AL_INVALID_NAME) };
        match param {
            AL_POSITION => s.position = v,
            _ => return self.fail(AL_INVALID_ENUM),
        }
        AL_NO_ERROR
    }

    pub fn set_listener_3f(&mut self, param: i32, v: [f32; 3]) -> i32 {
        match param {
            AL_POSITION => self.listener_pos = v,
            _ => return self.fail(AL_INVALID_ENUM),
        }
        AL_NO_ERROR
    }

    /// Renders `out.len()/2` stereo frames at MIX_RATE into `out` (interleaved L, R). Sources that are not playing
    /// contribute nothing. Finished buffers move to `processed`; a source with an empty queue stops.
    pub fn mix(&mut self, out: &mut [f32]) {
        for v in out.iter_mut() {
            *v = 0.0;
        }
        let frames = out.len() / 2;
        let ids: Vec<u32> = self.sources.keys().copied().collect();
        for sid in ids {
            // take the source out so the buffers can be read while it is updated
            let mut s = match self.sources.remove(&sid) {
                Some(s) => s,
                None => continue,
            };
            if s.state == State::Playing {
                let mut i = 0usize;
                while i < frames {
                    let Some(&bid) = s.queue.front() else {
                        s.state = State::Stopped;
                        s.pos = 0.0;
                        break;
                    };
                    let Some(buf) = self.buffers.get(&bid) else {
                        s.queue.pop_front();
                        s.pos = 0.0;
                        continue;
                    };
                    let n = buf.frame_count();
                    if n == 0 {
                        s.queue.pop_front();
                        s.processed.push_back(bid);
                        s.pos = 0.0;
                        continue;
                    }
                    let (gl, gr) = pan_gains(&self.listener_pos, &s.position, s.gain * self.listener_gain);
                    let step = buf.freq as f64 / MIX_RATE as f64 * s.pitch as f64;
                    let idx = s.pos as usize;
                    let frac = (s.pos - idx as f64) as f32;
                    let l0 = buf.frames[idx * 2];
                    let r0 = buf.frames[idx * 2 + 1];
                    let (l1, r1) = if idx + 1 < n {
                        (buf.frames[(idx + 1) * 2], buf.frames[(idx + 1) * 2 + 1])
                    } else if s.looping {
                        (buf.frames[0], buf.frames[1])
                    } else {
                        (l0, r0)
                    };
                    out[i * 2] += (l0 + (l1 - l0) * frac) * gl;
                    out[i * 2 + 1] += (r0 + (r1 - r0) * frac) * gr;
                    i += 1;
                    s.pos += step;
                    if s.pos >= n as f64 {
                        if s.looping {
                            s.pos -= n as f64 * (s.pos / n as f64).floor();
                        } else {
                            // the buffer is done: it is processed at this very frame (OpenAL reports STOPPED right
                            // away when the last one ends); the fraction carries over to the next buffer
                            s.pos -= n as f64;
                            s.queue.pop_front();
                            s.processed.push_back(bid);
                            if s.queue.is_empty() {
                                s.state = State::Stopped;
                                s.pos = 0.0;
                                break;
                            }
                        }
                    }
                }
            }
            self.sources.insert(sid, s);
        }
    }
}

/// Gains (left, right) from the source position relative to the listener: OpenAL default attenuation
/// (inverse distance clamped, reference distance 1, rolloff 1) and a constant-power pan on the x axis.
fn pan_gains(listener: &[f32; 3], src: &[f32; 3], gain: f32) -> (f32, f32) {
    let dx = src[0] - listener[0];
    let dy = src[1] - listener[1];
    let dz = src[2] - listener[2];
    let dist = (dx * dx + dy * dy + dz * dz).sqrt();
    let atten = if dist <= 1.0 { 1.0 } else { 1.0 / dist };
    // position 0,0,0 means "not positioned" in the stereo/mono sense: no attenuation, centered
    let (atten, pan) = if src == &[0.0, 0.0, 0.0] { (1.0, 0.0) } else { (atten, (dx / dist.max(1e-6)).clamp(-1.0, 1.0)) };
    let angle = (pan + 1.0) * std::f32::consts::FRAC_PI_4;
    let g = gain * atten;
    (g * angle.cos(), g * angle.sin())
}

/// Output of rendered blocks (a device, or a file in tests).
pub trait Sink {
    fn write(&mut self, interleaved: &[f32]);
}

/// Writes 16-bit PCM WAV, 44.1 kHz stereo.
pub struct WavSink {
    pub data: Vec<u8>,
}

impl WavSink {
    pub fn new() -> WavSink {
        WavSink { data: Vec::new() }
    }
    /// The file image: RIFF header followed by the samples.
    pub fn wav_bytes(&self) -> Vec<u8> {
        let data_len = self.data.len() as u32;
        let mut h = Vec::with_capacity(44 + self.data.len());
        h.extend_from_slice(b"RIFF");
        h.extend_from_slice(&(36 + data_len).to_le_bytes());
        h.extend_from_slice(b"WAVEfmt ");
        h.extend_from_slice(&16u32.to_le_bytes());
        h.extend_from_slice(&1u16.to_le_bytes());
        h.extend_from_slice(&2u16.to_le_bytes());
        h.extend_from_slice(&MIX_RATE.to_le_bytes());
        h.extend_from_slice(&(MIX_RATE * 4).to_le_bytes());
        h.extend_from_slice(&4u16.to_le_bytes());
        h.extend_from_slice(&16u16.to_le_bytes());
        h.extend_from_slice(b"data");
        h.extend_from_slice(&data_len.to_le_bytes());
        h.extend_from_slice(&self.data);
        h
    }
}

impl Sink for WavSink {
    fn write(&mut self, interleaved: &[f32]) {
        for &v in interleaved {
            let s = (v.clamp(-1.0, 1.0) * 32767.0) as i16;
            self.data.extend_from_slice(&s.to_le_bytes());
        }
    }
}

/// Renders `seconds` of the context into a sink, in blocks of 1024 frames.
pub fn render(ctx: &mut Context, sink: &mut dyn Sink, seconds: f64) {
    let total = (seconds * MIX_RATE as f64) as usize;
    let mut block = vec![0.0f32; 1024 * 2];
    let mut done = 0usize;
    while done < total {
        let n = (total - done).min(1024);
        ctx.mix(&mut block[..n * 2]);
        sink.write(&block[..n * 2]);
        done += n;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine_pcm16(freq: f64, seconds: f64, rate: u32) -> Vec<u8> {
        let n = (seconds * rate as f64) as usize;
        let mut out = Vec::with_capacity(n * 2);
        for i in 0..n {
            let v = (2.0 * std::f64::consts::PI * freq * i as f64 / rate as f64).sin();
            out.extend_from_slice(&((v * 16000.0) as i16).to_le_bytes());
        }
        out
    }

    fn zero_crossings(x: &[f32]) -> usize {
        x.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count()
    }

    #[test]
    fn sine_plays_at_its_frequency() {
        let mut ctx = Context::new();
        let b = ctx.gen_buffers(1)[0];
        assert_eq!(ctx.buffer_data(b, AL_FORMAT_MONO16, &sine_pcm16(440.0, 1.0, 44_100), 44_100), AL_NO_ERROR);
        let s = ctx.gen_sources(1)[0];
        ctx.set_source_3f(s, AL_POSITION, [0.0, 0.0, 0.0]);
        ctx.queue_buffers(s, &[b]);
        assert_eq!(ctx.play(s), AL_NO_ERROR);
        let mut sink = WavSink::new();
        render(&mut ctx, &mut sink, 1.0);
        let left: Vec<f32> = sink.data.chunks_exact(4).map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32767.0).collect();
        // 440 Hz: 880 zero crossings per second (+-2 for the edges)
        let zc = zero_crossings(&left);
        assert!((878..=882).contains(&zc), "zero crossings {zc}");
        assert_eq!(ctx.get_source_i(s, AL_SOURCE_STATE), Ok(AL_STOPPED));
        assert_eq!(ctx.get_source_i(s, AL_BUFFERS_PROCESSED), Ok(1));
    }

    #[test]
    fn resampling_keeps_pitch() {
        // 440 Hz recorded at 22050 Hz, played at 44100: one second of the buffer lasts half a second
        let mut ctx = Context::new();
        let b = ctx.gen_buffers(1)[0];
        ctx.buffer_data(b, AL_FORMAT_MONO16, &sine_pcm16(440.0, 1.0, 22_050), 22_050);
        let s = ctx.gen_sources(1)[0];
        ctx.queue_buffers(s, &[b]);
        ctx.play(s);
        let mut sink = WavSink::new();
        render(&mut ctx, &mut sink, 1.0);
        let left: Vec<f32> = sink.data.chunks_exact(4).map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32767.0).collect();
        let zc = zero_crossings(&left[..44_100 / 2]);
        assert!((438..=442).contains(&zc), "zero crossings in the first half second: {zc}");
        assert_eq!(ctx.get_source_i(s, AL_SOURCE_STATE), Ok(AL_STOPPED));
    }

    #[test]
    fn queue_and_unqueue_follow_al_rules() {
        let mut ctx = Context::new();
        let bufs = ctx.gen_buffers(3);
        for &b in &bufs {
            ctx.buffer_data(b, AL_FORMAT_STEREO16, &vec![0u8; 4 * 100], 44_100); // 100 frames each
        }
        let s = ctx.gen_sources(1)[0];
        ctx.queue_buffers(s, &bufs);
        assert_eq!(ctx.get_source_i(s, AL_BUFFERS_QUEUED), Ok(3));
        assert_eq!(ctx.get_source_i(s, AL_BUFFERS_PROCESSED), Ok(0));
        // a queued buffer cannot be replaced or deleted
        assert_eq!(ctx.buffer_data(bufs[0], AL_FORMAT_MONO8, &[1, 2], 8000), AL_INVALID_OPERATION);
        assert_eq!(ctx.delete_buffers(&[bufs[1]]), AL_INVALID_OPERATION);
        assert_eq!(ctx.error(), AL_INVALID_OPERATION);
        ctx.play(s);
        let mut sink = WavSink::new();
        render(&mut ctx, &mut sink, 250.0 / 44_100.0); // 250 frames: two buffers done, the third half played
        assert_eq!(ctx.get_source_i(s, AL_BUFFERS_PROCESSED), Ok(2));
        assert_eq!(ctx.unqueue_buffers(s, 1).unwrap(), vec![bufs[0]]);
        assert_eq!(ctx.unqueue_buffers(s, 5), Err(AL_INVALID_VALUE));
        assert_eq!(ctx.error(), AL_INVALID_VALUE);
        assert_eq!(ctx.get_source_i(s, AL_SOURCE_STATE), Ok(AL_PLAYING));
        render(&mut ctx, &mut sink, 1.0);
        assert_eq!(ctx.get_source_i(s, AL_SOURCE_STATE), Ok(AL_STOPPED));
        assert_eq!(ctx.unqueue_buffers(s, 2).unwrap(), vec![bufs[1], bufs[2]]);
        // unqueued buffers are free again
        assert_eq!(ctx.delete_buffers(&bufs), AL_NO_ERROR);
    }

    #[test]
    fn looping_never_stops_until_stopped() {
        let mut ctx = Context::new();
        let b = ctx.gen_buffers(1)[0];
        ctx.buffer_data(b, AL_FORMAT_MONO8, &[255u8; 50], 44_100);
        let s = ctx.gen_sources(1)[0];
        ctx.queue_buffers(s, &[b]);
        ctx.set_source_i(s, AL_LOOPING, AL_TRUE);
        ctx.play(s);
        let mut sink = WavSink::new();
        render(&mut ctx, &mut sink, 0.1);
        assert_eq!(ctx.get_source_i(s, AL_SOURCE_STATE), Ok(AL_PLAYING));
        assert_eq!(ctx.get_source_i(s, AL_BUFFERS_PROCESSED), Ok(0));
        ctx.stop(s);
        assert_eq!(ctx.get_source_i(s, AL_SOURCE_STATE), Ok(AL_STOPPED));
        assert_eq!(ctx.get_source_i(s, AL_BUFFERS_PROCESSED), Ok(1));
    }

    #[test]
    fn pause_resume_and_invalid_calls() {
        let mut ctx = Context::new();
        let b = ctx.gen_buffers(1)[0];
        ctx.buffer_data(b, AL_FORMAT_MONO8, &[200u8; 4410], 44_100);
        let s = ctx.gen_sources(1)[0];
        ctx.queue_buffers(s, &[b]);
        ctx.play(s);
        ctx.pause(s);
        assert_eq!(ctx.get_source_i(s, AL_SOURCE_STATE), Ok(AL_PAUSED));
        let mut sink = WavSink::new();
        render(&mut ctx, &mut sink, 0.05);
        assert!(sink.data.iter().all(|&x| x == 0), "paused source is silent");
        ctx.play(s);
        assert_eq!(ctx.get_source_i(s, AL_SOURCE_STATE), Ok(AL_PLAYING));
        assert_eq!(ctx.set_source_f(s, AL_GAIN, -1.0), AL_INVALID_VALUE);
        assert_eq!(ctx.error(), AL_INVALID_VALUE);
        assert_eq!(ctx.set_source_f(s, AL_PITCH, 0.0), AL_INVALID_VALUE);
        assert_eq!(ctx.set_source_f(s, 0x9999, 1.0), AL_INVALID_ENUM);
        // only the first error since the last read is kept
        assert_eq!(ctx.error(), AL_INVALID_VALUE);
        assert_eq!(ctx.error(), AL_NO_ERROR);
        assert_eq!(ctx.play(999), AL_INVALID_NAME);
        assert_eq!(ctx.error(), AL_INVALID_NAME);
        assert_eq!(ctx.buffer_data(b, 0x1234, &[0], 8000), AL_INVALID_ENUM);
        assert_eq!(ctx.buffer_data(b, AL_FORMAT_MONO8, &[0], 0), AL_INVALID_VALUE);
        assert_eq!(ctx.buffer_data(777, AL_FORMAT_MONO8, &[0], 8000), AL_INVALID_NAME);
        assert_eq!(ctx.error(), AL_INVALID_ENUM); // the first of the errors since the last read
        assert_eq!(ctx.error(), AL_NO_ERROR);
    }

    #[test]
    fn gain_and_attenuation() {
        let quiet = pan_gains(&[0.0, 0.0, 0.0], &[0.0, 0.0, 10.0], 1.0);
        let near = pan_gains(&[0.0, 0.0, 0.0], &[0.0, 0.0, 2.0], 1.0);
        let half = pan_gains(&[0.0, 0.0, 0.0], &[0.0, 0.0, 0.0], 0.5);
        assert!(quiet.0 < near.0 && near.0 < 1.0, "attenuation with distance");
        assert!((half.0 - 0.5 * std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6, "centered, gain 0.5 (equal power)");
        let left = pan_gains(&[0.0, 0.0, 0.0], &[-5.0, 0.0, 0.0], 1.0);
        let right = pan_gains(&[0.0, 0.0, 0.0], &[5.0, 0.0, 0.0], 1.0);
        assert!(left.0 > left.1 && right.1 > right.0, "pan follows x");
    }

    #[test]
    fn wav_file_has_a_valid_header() {
        let mut sink = WavSink::new();
        sink.write(&[0.0, 0.0, 0.5, -0.5]);
        let f = sink.wav_bytes();
        assert_eq!(&f[0..4], b"RIFF");
        assert_eq!(&f[8..12], b"WAVE");
        assert_eq!(f.len(), 44 + 8);
        assert_eq!(u32::from_le_bytes([f[40], f[41], f[42], f[43]]), 8);
    }
}

#[cfg(not(test))]
fn _unused() {}
