//! `android/touchcore/jni/touchcore_android.c` tarafından tüketilen C ABI: `tc_init`, `tc_tick_frame`, `tc_shutdown`.
//! Motor, Zig orijinalindeki gibi bir mutex'in arkasında süreç çapında tek bir değerdir. Her giriş noktası herhangi bir
//! iş parçacığından çağrılabilir; `tc_init`'ten önceki (ya da `tc_shutdown`'dan sonraki) çağrı hiçbir şey yapmaz.
use std::ffi::c_void;
use std::sync::{Mutex, MutexGuard};

use crate::arena::Arena;
use crate::cull::{cull_and_schedule_world, CullScratch, CullStats, SectionSnapshot, View};
use crate::instancing::{Billboard, EntityKind, InstanceBatcher, MeshProto};
use crate::pool::{size_class, SlabPool};
use crate::rhi::{BackendKind, Backend, Command, Headless, PipelineHandle, PowerProfile, RhiDevice};
use crate::thread_pool::{WaitGroup, WorkerPool};
use crate::world::{compute_section_portals, HiZPyramid, Mat4, SectionPortalMask, Vec3};

/// Başlatıcının gösterdiği kare başına sayılar. Yerleşim sabittir: `touchcore_android.c` aynı yapıyı bildirir.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Telemetry {
    pub frame_epoch: u64,
    pub arena_bytes: u64,
    pub slab_committed_bytes: u64,
    pub slab_reclaimed_bytes: u64,
    pub visible_sections: u32,
    pub frustum_culled: u32,
    pub portal_culled: u32,
    pub hiz_culled: u32,
    pub draw_batches: u32,
    pub surviving_instances: u32,
    pub worker_threads: u32,
    pub rhi_active_backend: i32,
}

const SECTION_CAP: usize = 512;
const PORTAL_GRIDS: usize = 256;
const MIN_PIXEL_AREA: f32 = 4.0;

struct Engine {
    workers: WorkerPool,
    arena: Arena,
    pool: SlabPool,
    dev: Option<RhiDevice>,
    pipe: PipelineHandle,
    hiz: HiZPyramid,
    batcher: InstanceBatcher,
    cull: CullScratch,
    sections: Vec<SectionSnapshot>,
    frame_epoch: u64,
    last: Telemetry,
}

static ENGINE: Mutex<Option<Engine>> = Mutex::new(None);

fn lock() -> MutexGuard<'static, Option<Engine>> {
    ENGINE.lock().unwrap_or_else(|e| e.into_inner())
}

fn backend_of(preferred: i32) -> BackendKind {
    match preferred {
        1 => BackendKind::Vulkan,
        2 => BackendKind::OpenGl33,
        3 => BackendKind::Headless,
        _ => BackendKind::Auto,
    }
}

fn power_of(profile: i32) -> PowerProfile {
    match profile {
        0 => PowerProfile::MaxFps,
        2 => PowerProfile::EcoBattery,
        _ => PowerProfile::Balanced,
    }
}

/// Başlatıcının okuduğu arka uç kodu: Auto 0, Vulkan 1, OpenGL 3.3 2, Headless 3.
fn backend_code(k: BackendKind) -> i32 {
    match k {
        BackendKind::Auto => 0,
        BackendKind::Vulkan => 1,
        BackendKind::OpenGl33 => 2,
        BackendKind::Headless => 3,
    }
}

/// İşçi havuzu için portal işi: indeks başına bir 16^3 bölümü.
struct PortalJob {
    grids: *const [u64; 64],
    masks: *mut SectionPortalMask,
}

unsafe fn portal_task(ctx: *mut c_void, idx: usize) {
    let job = &*(ctx as *const PortalJob);
    *job.masks.add(idx) = compute_section_portals(&*job.grids.add(idx));
}

fn build_sections(workers: &WorkerPool) -> Vec<SectionSnapshot> {
    // her sekizinci bölüm katı bir örtücüdür: voksellerinin hepsi opaktır, geri kalanı açıktır
    let mut grids = vec![[0u64; 64]; PORTAL_GRIDS];
    for (i, g) in grids.iter_mut().enumerate() {
        if i % 7 == 5 {
            *g = [u64::MAX; 64];
        }
    }
    let mut masks = vec![SectionPortalMask { bits: 0 }; PORTAL_GRIDS];
    let job = PortalJob { grids: grids.as_ptr(), masks: masks.as_mut_ptr() };
    let wg = WaitGroup::new();
    // GÜVENLİK: `job`, `grids`, `masks` ve `wg` aşağıdaki beklemeden sonra da yaşar.
    unsafe { workers.dispatch_batch(portal_task, &job as *const PortalJob as *mut c_void, PORTAL_GRIDS, &wg) };
    wg.wait();

    let mut out = Vec::with_capacity(SECTION_CAP);
    let mut idx = 0usize;
    for sx in -4..4 {
        for sz in -4..4 {
            for sy in 0..4 {
                if idx >= SECTION_CAP {
                    break;
                }
                let solid = idx % 7 == 5 && sy == 0;
                let portals = if solid { SectionPortalMask { bits: 0 } } else { masks[idx & 255] };
                out.push(SectionSnapshot {
                    has_non_air: sy == 0 || sy == 1,
                    fully_opaque_occluder: solid,
                    portals,
                    ..SectionSnapshot::at(sx, sy, sz)
                });
                idx += 1;
            }
        }
    }
    out
}

/// Motoru başlatır. Etkin arka uç kodunu döndürür (bkz. `backend_code`).
///
/// # Safety
/// Başlatıcının JNI iş parçacığından çağrılır; hiçbir işaretçi söz konusu değildir.
#[no_mangle]
pub extern "C" fn tc_init(backend_pref: i32, power_profile: i32, worker_count: i32) -> i32 {
    let mut guard = lock();
    if let Some(e) = guard.as_ref() {
        return e.dev.as_ref().map_or(backend_code(BackendKind::Headless), |d| backend_code(d.backend()));
    }
    let mut workers = WorkerPool::new();
    let req = if worker_count <= 0 { 2 } else { (worker_count as usize).min(4) };
    if workers.start(req).is_err() {
        workers = WorkerPool::new();
    }
    let mut dev = RhiDevice::init(backend_of(backend_pref), power_of(power_profile), 4).ok();
    let pipe = dev.as_mut().map_or(PipelineHandle(0), |d| d.create_pipeline());
    let sections = build_sections(&workers);
    let code = dev.as_ref().map_or(backend_code(BackendKind::Headless), |d| backend_code(d.backend()));
    *guard = Some(Engine {
        workers,
        arena: Arena::new(crate::arena::DEFAULT_CHUNK_SIZE, 4),
        pool: SlabPool::new(),
        dev,
        pipe,
        hiz: HiZPyramid::new(),
        batcher: InstanceBatcher::default(),
        cull: CullScratch::default(),
        sections,
        frame_epoch: 0,
        last: Telemetry::default(),
    });
    code
}

/// Bir kare çalıştırır: bölümleri eler, örnek toplu işini kurar ve gönderir. Null değilse `out_tel`'i doldurur.
///
/// # Safety
/// `out_tel` null değilse, yazılabilir bir `Telemetry`'ye işaret etmelidir.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn tc_tick_frame(
    cam_x: f32,
    cam_y: f32,
    cam_z: f32,
    look_x: f32,
    look_y: f32,
    look_z: f32,
    fov_deg: f32,
    viewport_w: u32,
    viewport_h: u32,
    entity_count: u32,
    out_tel: *mut Telemetry,
) {
    let mut guard = lock();
    let Some(e) = guard.as_mut() else { return };
    e.frame_epoch = e.frame_epoch.wrapping_add(1);
    e.arena.reset_retaining_capacity();
    e.hiz.clear();
    e.pool.advance_and_reclaim();
    // her karede biraz yuva dalgalanması, çağ muhasebesinin çalışır kalmasını sağlar
    if let Some(sc) = size_class(64) {
        if let Some(slot) = e.pool.alloc_slot(sc) {
            // GÜVENLİK: `slot`, `alloc_slot(sc)`'den geldi ve bu çağrıdan sonra kullanılmaz.
            e.pool.retire(slot, sc);
        }
    }

    let vw = viewport_w.max(1);
    let vh = viewport_h.max(1);
    let aspect = vw as f32 / vh as f32;
    let camera = Vec3::new(cam_x, cam_y, cam_z);
    let target = Vec3::new(look_x, look_y, look_z);
    let view_m = Mat4::look_at(camera, target, Vec3::new(0.0, 1.0, 0.0));
    let proj = Mat4::perspective(fov_deg.max(30.0).to_radians(), aspect, 0.1, 256.0);
    let view_proj = proj.mul(&view_m);
    let view = View { camera, view_proj, viewport: (vw, vh), min_pixel_area: MIN_PIXEL_AREA };

    let mut cstats = CullStats::default();
    let _visible = cull_and_schedule_world(&mut e.cull, &e.sections, &view, &mut e.hiz, &mut cstats);

    let (mut surviving, mut batches) = (0u32, 0u32);
    if let Some(dev) = e.dev.as_mut() {
        dev.begin_frame();
        e.batcher.begin_frame(entity_count.max(32) as usize);
        e.batcher.register_mesh(MeshProto { mesh_id: 1, index_count: 36, first_index: 0, vertex_offset: 0 });
        e.batcher.register_mesh(MeshProto { mesh_id: 2, index_count: 12, first_index: 36, vertex_offset: 0 });
        for i in 0..entity_count.min(256) {
            let fx = (i % 16) as f32 - 8.0;
            let fz = (i / 16) as f32 - 8.0;
            let particle = i & 3 == 0;
            let (kind, mid) = if particle { (EntityKind::Particle, 2) } else { (EntityKind::BlockDisplay, 1) };
            e.batcher.push_entity(kind, mid, Vec3::new(fx * 2.5, 3.0, fz * 2.5), 0.8, 0.8, Billboard::Fixed, i, 0xFFFF_FFFF);
        }
        e.batcher.flush(&view, &e.hiz);
        let frame = e.batcher.frame();
        surviving = e.batcher.stats().surviving_instances as u32;
        batches = e.batcher.stats().draw_batches_emitted as u32;
        if !frame.instances.is_empty() {
            let inst_bytes = as_bytes(&frame.instances);
            if let Ok(slice) = dev.upload(inst_bytes, 64) {
                dev.record(Command::BindPipeline(e.pipe));
                dev.record(Command::BindInstanceBuffer {
                    binding: 0,
                    buf: slice.buffer,
                    offset_bytes: slice.offset,
                    size_bytes: slice.len,
                    texel_stride: 64,
                });
                let cmds = frame.commands.as_slice();
                if let Ok(ind) = dev.upload(as_bytes(cmds), 16) {
                    dev.record_indirect(ind.buffer, ind.offset, cmds);
                }
            }
        }
        dev.submit_frame(&mut Headless as &mut dyn Backend);
    }

    let active = e.dev.as_ref().map_or(backend_code(BackendKind::Headless), |d| backend_code(d.backend()));
    e.last = Telemetry {
        frame_epoch: e.frame_epoch,
        arena_bytes: e.arena.committed_bytes() as u64,
        slab_committed_bytes: e.pool.stats().committed_bytes as u64,
        slab_reclaimed_bytes: e.pool.stats().reclaimed_bytes as u64,
        visible_sections: cstats.visible_sections as u32,
        frustum_culled: cstats.frustum_culled as u32,
        portal_culled: cstats.portal_culled as u32,
        hiz_culled: cstats.hiz_occluded as u32,
        draw_batches: batches,
        surviving_instances: surviving,
        worker_threads: e.workers.num_workers() as u32,
        rhi_active_backend: active,
    };
    if !out_tel.is_null() {
        *out_tel = e.last;
    }
}

/// İşçileri durdurur ve motoru serbest bırakır. Birden fazla kez çağırmak güvenlidir.
#[no_mangle]
pub extern "C" fn tc_shutdown() {
    let mut guard = lock();
    if let Some(mut e) = guard.take() {
        e.dev = None;
        e.workers.stop();
    }
}

/// Düz veri değerlerinden oluşan bir dilimi yükleme için bayt olarak görüntüler.
fn as_bytes<T: Copy>(items: &[T]) -> &[u8] {
    // GÜVENLİK: crate bunu yalnızca okuyucu için önemli dolgusu olmayan `repr(C)` değerlerde kullanır
    // (`GpuInstance`, `DrawIndexedIndirect`); bayt görünümü tam olarak dilimin belleğini kapsar.
    unsafe { std::slice::from_raw_parts(items.as_ptr() as *const u8, std::mem::size_of_val(items)) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instancing::GpuInstance;

    #[test]
    fn tick_before_init_is_a_no_op_and_shutdown_is_idempotent() {
        tc_shutdown();
        tc_shutdown();
        let mut tel = Telemetry::default();
        // GÜVENLİK: `tel`, çağrı süresince geçerli ve yazılabilir bir Telemetry'dir.
        unsafe { tc_tick_frame(0.0, 0.0, 0.0, 0.0, 0.0, -1.0, 70.0, 1920, 1080, 10, &mut tel) };
        assert_eq!(tel.frame_epoch, 0);
    }

    #[test]
    fn init_tick_shutdown_round_trip() {
        let code = tc_init(3, 0, 2);
        assert_eq!(code, backend_code(BackendKind::Headless));
        let mut tel = Telemetry::default();
        // GÜVENLİK: `tel` geçerli ve yazılabilirdir.
        unsafe { tc_tick_frame(8.0, 8.0, 8.0, 8.0, 8.0, -8.0, 70.0, 1920, 1080, 64, &mut tel) };
        assert_eq!(tel.frame_epoch, 1);
        assert_eq!(tel.rhi_active_backend, backend_code(BackendKind::Headless));
        assert!(tel.worker_threads >= 1);
        assert!(tel.slab_committed_bytes > 0);
        tc_shutdown();
        // GÜVENLİK: yukarıdaki gibi.
        unsafe { tc_tick_frame(0.0, 0.0, 0.0, 0.0, 0.0, -1.0, 70.0, 1, 1, 0, &mut tel) };
        assert_eq!(tel.frame_epoch, 1, "no frame runs after shutdown");
    }

    #[test]
    fn unported_back_end_falls_back_to_reporting_headless() {
        let code = tc_init(1, 1, 1);
        assert_eq!(code, backend_code(BackendKind::Headless), "Vulkan is refused until ported; the engine runs headless");
        tc_shutdown();
    }

    #[test]
    fn instance_bytes_match_the_declared_layout() {
        let v = [GpuInstance {
            row0: [1.0; 4], row1: [0.0; 4], row2: [0.0; 4], uv_rect: 0, color_light: 0, flags: 0, mesh_and_radius: 0,
        }];
        assert_eq!(as_bytes(&v).len(), 64);
    }
}
