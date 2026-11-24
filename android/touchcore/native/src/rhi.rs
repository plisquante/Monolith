//! Render donanım arayüzü: durum tekilleştirmeli bir komut kaydedici, kare başına bir hazırlama halkası, kare
//! istatistikleri ve bir arka uç trait'i. `rhi.zig`'in (aygıt ve kaydedici) taşınmış hâli. Arka uçlar `Backend` aracılığıyla takılır.
//! Burada yalnızca headless arka uç var; Vulkan ya da OpenGL 3.3 istemek, bu arka uçlar taşınıp bir cihazda
//! doğrulanana kadar yüksek sesle başarısız olur.
use crate::instancing::DrawIndexedIndirect;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendKind {
    Auto,
    Vulkan,
    OpenGl33,
    Headless,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerProfile {
    /// platformun izin verdiği yerde vsync kapalı
    MaxFps,
    Balanced,
    /// vsync açık, kare hızı ayarı, daha az boşta GPU işi
    EcoBattery,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PipelineHandle(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BufferHandle(pub u32);

#[derive(Debug, PartialEq, Eq)]
pub enum RhiError {
    /// istenen arka uç henüz taşınmadı
    UnsupportedBackend(BackendKind),
    /// hazırlama halkası bu karedeki yüklemeyi tutamaz
    OutOfStaging,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameStats {
    pub frame_index: u64,
    pub draw_calls: u32,
    pub state_changes_skipped: u32,
    pub instances_submitted: u64,
    pub triangles_submitted: u64,
    pub staging_bytes_used: usize,
}

/// Karenin depolamasında tutulan geri düşüş komutları aralığı (komut başına sıfır tahsis).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FallbackRange {
    start: usize,
    len: usize,
}

impl FallbackRange {
    /// Bu aralığın, `Backend::execute`'a geçirilen karenin geri düşüş listesindeki konumları.
    pub fn range(&self) -> std::ops::Range<usize> {
        self.start..self.start + self.len
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    BindPipeline(PipelineHandle),
    BindIndexBuffer { buf: BufferHandle, index_32: bool },
    BindInstanceBuffer { binding: u32, buf: BufferHandle, offset_bytes: usize, size_bytes: usize, texel_stride: u32 },
    DrawIndexedInstanced { index_count: u32, instance_count: u32, first_index: u32, vertex_offset: i32, first_instance: u32 },
    DrawIndexedIndirectRange { indirect_buf: BufferHandle, offset_bytes: usize, draw_count: u32, stride: u32, fallback: FallbackRange },
    /// Karenin hazırlama halkasının mutlak halka ofseti `src_offset`'ten başlayan `len` baytını, `dst_offset`'te `dst` içine
    /// kopyalar. `RhiDevice::copy_to_buffer` tarafından yazılır.
    CopyStaging { src_offset: usize, len: usize, dst: BufferHandle, dst_offset: usize },
}

/// Bir yüklemenin bu kare için hazırlama halkasında nereye indiği.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StagingSlice {
    pub buffer: BufferHandle,
    pub offset: usize,
    pub len: usize,
}

/// Üç karelik yükleme alanı: kaydedilmekte olan kare ve GPU'da hâlâ uçuşta olan iki kare.
pub struct FrameRing {
    per_frame_bytes: usize,
    used: [usize; Self::FRAMES_IN_FLIGHT],
    data: Vec<u8>,
}

impl FrameRing {
    pub const FRAMES_IN_FLIGHT: usize = 3;

    pub fn new(per_frame_bytes: usize) -> Self {
        FrameRing { per_frame_bytes, used: [0; Self::FRAMES_IN_FLIGHT], data: vec![0; per_frame_bytes * Self::FRAMES_IN_FLIGHT] }
    }

    fn begin(&mut self, slot: usize) {
        self.used[slot] = 0;
    }

    /// `bytes`'ı yuvanın bölgesine hizalı bir ofsette kopyalar.
    fn alloc(&mut self, slot: usize, bytes: &[u8], align: usize) -> Option<(usize, usize)> {
        let align = align.max(1);
        let base = slot * self.per_frame_bytes;
        let off = (self.used[slot] + align - 1) & !(align - 1);
        let end = off.checked_add(bytes.len())?;
        if end > self.per_frame_bytes {
            return None;
        }
        self.data[base + off..base + end].copy_from_slice(bytes);
        self.used[slot] = end;
        Some((base + off, bytes.len()))
    }

    pub fn bytes(&self) -> &[u8] {
        &self.data
    }
}

/// Bir arka ucun bir karenin komutlarıyla ne yapması gerektiği.
pub trait Backend {
    fn execute(&mut self, commands: &[Command], fallback: &[DrawIndexedIndirect], ring: &FrameRing);
}

/// Komutları tüketir ve hiçbir şey çizmez. Testler ve GPU'suz makineler için kullanılır.
#[derive(Default)]
pub struct Headless;

impl Backend for Headless {
    fn execute(&mut self, _commands: &[Command], _fallback: &[DrawIndexedIndirect], _ring: &FrameRing) {}
}

pub struct RhiDevice {
    backend_kind: BackendKind,
    power: PowerProfile,
    ring: FrameRing,
    next_buf: u32,
    next_pipe: u32,
    commands: Vec<Command>,
    fallback: Vec<DrawIndexedIndirect>,
    bound_pipe: Option<PipelineHandle>,
    bound_ib: Option<BufferHandle>,
    stats: FrameStats,
    last_stats: FrameStats,
    frame_slot: usize,
}

impl RhiDevice {
    /// `Auto` ve `Headless` headless aygıta çözülür. Vulkan ve OpenGL 3.3, taşınana kadar reddedilir.
    pub fn init(preferred: BackendKind, power: PowerProfile, staging_mb_per_frame: usize) -> Result<Self, RhiError> {
        match preferred {
            BackendKind::Auto | BackendKind::Headless => {}
            other => return Err(RhiError::UnsupportedBackend(other)),
        }
        Ok(RhiDevice {
            backend_kind: BackendKind::Headless,
            power,
            ring: FrameRing::new(staging_mb_per_frame * 1024 * 1024),
            next_buf: 16,
            next_pipe: 16,
            commands: Vec::new(),
            fallback: Vec::new(),
            bound_pipe: None,
            bound_ib: None,
            stats: FrameStats::default(),
            last_stats: FrameStats::default(),
            frame_slot: 0,
        })
    }

    pub fn backend(&self) -> BackendKind {
        self.backend_kind
    }

    pub fn power(&self) -> PowerProfile {
        self.power
    }

    pub fn create_buffer(&mut self) -> BufferHandle {
        self.next_buf += 1;
        BufferHandle(self.next_buf)
    }

    pub fn create_pipeline(&mut self) -> PipelineHandle {
        self.next_pipe += 1;
        PipelineHandle(self.next_pipe)
    }

    pub fn begin_frame(&mut self) {
        self.frame_slot = (self.frame_slot + 1) % FrameRing::FRAMES_IN_FLIGHT;
        self.ring.begin(self.frame_slot);
        self.commands.clear();
        self.fallback.clear();
        self.bound_pipe = None;
        self.bound_ib = None;
        let next = self.stats.frame_index + 1;
        self.stats = FrameStats { frame_index: next, ..FrameStats::default() };
    }

    /// Bir yüklemeyi bu karenin hazırlama bölgesine kopyalar.
    pub fn upload(&mut self, bytes: &[u8], align: usize) -> Result<StagingSlice, RhiError> {
        let (offset, len) = self.ring.alloc(self.frame_slot, bytes, align).ok_or(RhiError::OutOfStaging)?;
        self.stats.staging_bytes_used += bytes.len();
        Ok(StagingSlice { buffer: BufferHandle(1), offset, len })
    }

    /// Bir komutu kaydeder. Aynı boru hattını ya da indeks tamponunu yeniden bağlamak atılır ve sayılır.
    pub fn record(&mut self, cmd: Command) {
        match cmd {
            Command::BindPipeline(p) => {
                if self.bound_pipe == Some(p) {
                    self.stats.state_changes_skipped += 1;
                    return;
                }
                self.bound_pipe = Some(p);
            }
            Command::BindIndexBuffer { buf, .. } => {
                if self.bound_ib == Some(buf) {
                    self.stats.state_changes_skipped += 1;
                    return;
                }
                self.bound_ib = Some(buf);
            }
            _ => {}
        }
        self.commands.push(cmd);
    }

    /// Dolaylı bir aralığı kaydeder ve çoklu çizim olmayan arka uçlar için CPU tarafı geri düşüş komutlarını tutar.
    pub fn record_indirect(&mut self, indirect_buf: BufferHandle, offset_bytes: usize, cmds: &[DrawIndexedIndirect]) {
        let start = self.fallback.len();
        self.fallback.extend_from_slice(cmds);
        let stride = std::mem::size_of::<DrawIndexedIndirect>() as u32;
        self.record(Command::DrawIndexedIndirectRange {
            indirect_buf,
            offset_bytes,
            draw_count: cmds.len() as u32,
            stride,
            fallback: FallbackRange { start, len: cmds.len() },
        });
    }

    /// Bir hazırlama yüklemesini aygıt tamponuna kopyalar. Kopya arka uçta, komut sırasıyla çalışır.
    pub fn copy_to_buffer(&mut self, src: StagingSlice, dst: BufferHandle, dst_offset: usize) {
        self.record(Command::CopyStaging { src_offset: src.offset, len: src.len, dst, dst_offset });
    }

    fn fallback_of(&self, r: FallbackRange) -> &[DrawIndexedIndirect] {
        &self.fallback[r.start..r.start + r.len]
    }

    /// Kareyi hesaba katar ve arka uca verir.
    pub fn submit_frame(&mut self, backend: &mut dyn Backend) {
        let multi_draw = false; // no back end reports multi-draw indirect yet
        for cmd in &self.commands {
            match *cmd {
                Command::DrawIndexedInstanced { index_count, instance_count, .. } => {
                    self.stats.draw_calls += 1;
                    self.stats.instances_submitted += instance_count as u64;
                    self.stats.triangles_submitted += (index_count as u64 / 3) * instance_count as u64;
                }
                Command::DrawIndexedIndirectRange { draw_count, fallback, .. } => {
                    self.stats.draw_calls += if multi_draw { 1 } else { draw_count };
                    for fc in &self.fallback[fallback.start..fallback.start + fallback.len] {
                        self.stats.instances_submitted += fc.instance_count as u64;
                        self.stats.triangles_submitted += (fc.index_count as u64 / 3) * fc.instance_count as u64;
                    }
                }
                _ => {}
            }
        }
        backend.execute(&self.commands, &self.fallback, &self.ring);
        self.last_stats = self.stats;
    }

    pub fn last_stats(&self) -> &FrameStats {
        &self.last_stats
    }

    pub fn commands(&self) -> &[Command] {
        &self.commands
    }

    /// Bu karede kaydedilen dolaylı bir aralığın geri düşüş komutları.
    pub fn fallback_commands(&self, range: FallbackRange) -> &[DrawIndexedIndirect] {
        self.fallback_of(range)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_state_is_deduplicated_and_counted() {
        let mut dev = RhiDevice::init(BackendKind::Headless, PowerProfile::MaxFps, 1).unwrap();
        let p = dev.create_pipeline();
        dev.begin_frame();
        dev.record(Command::BindPipeline(p));
        dev.record(Command::BindPipeline(p));
        dev.record(Command::BindPipeline(p));
        assert_eq!(dev.commands().len(), 1);
        dev.submit_frame(&mut Headless);
        assert_eq!(dev.last_stats().state_changes_skipped, 2, "the two repeated binds are dropped and counted");
    }

    #[test]
    fn indirect_range_counts_each_fallback_command() {
        let mut dev = RhiDevice::init(BackendKind::Headless, PowerProfile::Balanced, 1).unwrap();
        dev.begin_frame();
        let cmds = [
            DrawIndexedIndirect { index_count: 36, instance_count: 2, first_index: 0, vertex_offset: 0, first_instance: 0 },
            DrawIndexedIndirect { index_count: 6, instance_count: 3, first_index: 36, vertex_offset: 8, first_instance: 2 },
        ];
        dev.record_indirect(BufferHandle(20), 0, &cmds);
        dev.submit_frame(&mut Headless);
        let s = dev.last_stats();
        assert_eq!(s.draw_calls, 2, "no multi-draw: one draw per command");
        assert_eq!(s.instances_submitted, 5);
        assert_eq!(s.triangles_submitted, 12 * 2 + 2 * 3);
    }

    #[test]
    fn staging_ring_is_aligned_and_bounded() {
        let mut dev = RhiDevice::init(BackendKind::Headless, PowerProfile::Balanced, 1).unwrap();
        dev.begin_frame();
        let a = dev.upload(&[1, 2, 3], 1).unwrap();
        let b = dev.upload(&[4; 64], 64).unwrap();
        assert_eq!(b.offset % 64, 0);
        assert!(b.offset >= a.offset + 3);
        let too_big = vec![0u8; 2 * 1024 * 1024];
        assert_eq!(dev.upload(&too_big, 1), Err(RhiError::OutOfStaging));
    }

    #[test]
    fn staging_copy_is_recorded_with_its_ring_offset() {
        let mut dev = RhiDevice::init(BackendKind::Headless, PowerProfile::Balanced, 1).unwrap();
        dev.begin_frame();
        let s = dev.upload(&[1, 2, 3, 4], 4).unwrap();
        dev.copy_to_buffer(s, BufferHandle(7), 16);
        assert_eq!(
            dev.commands(),
            &[Command::CopyStaging { src_offset: s.offset, len: 4, dst: BufferHandle(7), dst_offset: 16 }]
        );
    }

    #[test]
    fn unported_back_ends_fail_loudly() {
        assert_eq!(
            RhiDevice::init(BackendKind::Vulkan, PowerProfile::MaxFps, 1).err(),
            Some(RhiError::UnsupportedBackend(BackendKind::Vulkan))
        );
        assert!(RhiDevice::init(BackendKind::OpenGl33, PowerProfile::MaxFps, 1).is_err());
    }
}
