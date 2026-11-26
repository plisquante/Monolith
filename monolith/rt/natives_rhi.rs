//! Sıfır-LWJGL RHI (`mono.rhi.NativeRhi` 124 metot) + Gölgelendirici Derleyicisi (`org.lwjgl.util.MonoShader` 38 metot).
//! OpenGL 3.3 Core / GLES 3.0'ı (`libGLESv3.so` / `libGL.so.1`), TouchCore Android Yüzey/Girdi'yi
//! (`libtouchcore_mono.so`), OpenAL'ı (`libopenal.so.1`), shaderc'i (`libshaderc.so`) ve SPIRV-Cross'u (`libspirv-cross.so`) dinamik olarak bağlar.
use crate::rt::{self, Arr, Ref, Res};
use crate::desk::{self, dl_open_any, dl_sym, sym_fn, TcPollSnapshot};
use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uint, c_void};
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};


// ------------------------------------------------------------------ RHI Dünya Elemesi ve Örnek Toplayıcı Durumu
#[derive(Clone, Copy)]
struct SectionSnap {
    sx: i32,
    sy: i32,
    sz: i32,
    has_non_air: bool,
    opaque: bool,
    portals: u64,
    index_count: u32,
    first_index: u32,
    vertex_offset: i32,
}

#[derive(Clone, Copy)]
struct VisibleSec {
    idx: usize,
    lod: i32,
}

#[derive(Clone, Copy)]
struct MeshEntry {
    index_count: u32,
    first_index: u32,
    vertex_offset: i32,
}

#[derive(Clone, Copy)]
struct InstanceEntry {
    kind: i32,
    mesh_id: i32,
    x: f32,
    y: f32,
    z: f32,
    scale: f32,
    radius: f32,
}

struct RhiState {
    initialized: bool,
    backend: i32,
    power: i32,
    frame_open: bool,
    viewport_w: u32,
    viewport_h: u32,
    sections: Vec<SectionSnap>,
    visible: Vec<VisibleSec>,
    dropped_sections: u64,
    meshes: HashMap<i32, MeshEntry>,
    instances: Vec<InstanceEntry>,
    last_batches: i32,
}

static FRAME_EPOCH: AtomicU64 = AtomicU64::new(0);
thread_local! {
    static ARENA_SCOPE_DEPTH: Cell<usize> = const { Cell::new(0) };
}

fn rhi_state() -> &'static Mutex<RhiState> {
    static S: OnceLock<Mutex<RhiState>> = OnceLock::new();
    S.get_or_init(|| {
        Mutex::new(RhiState {
            initialized: false,
            backend: 3,
            power: 1,
            frame_open: false,
            viewport_w: 1280,
            viewport_h: 720,
            sections: Vec::new(),
            visible: Vec::new(),
            dropped_sections: 0,
            meshes: HashMap::new(),
            instances: Vec::new(),
            last_batches: 0,
        })
    })
}

fn check_finite(vals: &[f32]) -> Res<()> {
    if vals.iter().any(|v| !v.is_finite()) {
        return rt::throw("java/lang/IllegalArgumentException", "NativeRhi: non-finite parameter".to_string());
    }
    Ok(())
}

pub fn n_mono_rhi_NativeRhi__init0(backend: i32, power: i32) -> Res<i32> {
    let mut st = rhi_state().lock().unwrap();
    st.initialized = true;
    st.power = power;
    st.backend = match backend {
        1 => 1,
        2 => 2,
        3 => 3,
        _ => if has_gl() { 2 } else { 3 },
    };
    st.sections.clear();
    st.visible.clear();
    st.meshes.clear();
    st.instances.clear();
    st.dropped_sections = 0;
    st.frame_open = false;
    Ok(st.backend)
}

pub fn n_mono_rhi_NativeRhi__beginFrame0(w: i32, h: i32) -> Res<i64> {
    let mut st = rhi_state().lock().unwrap();
    if !st.initialized {
        return rt::throw("java/lang/IllegalArgumentException", "NativeRhi.init() has not been called".to_string());
    }
    st.viewport_w = w.max(1) as u32;
    st.viewport_h = h.max(1) as u32;
    st.visible.clear();
    st.meshes.clear();
    st.instances.clear();
    st.last_batches = 0;
    st.frame_open = true;
    let ep = FRAME_EPOCH.fetch_add(1, Ordering::Relaxed) + 1;
    Ok(ep as i64)
}

pub fn n_mono_rhi_NativeRhi__endFrame0() -> Res<()> {
    let mut st = rhi_state().lock().unwrap();
    st.frame_open = false;
    Ok(())
}

pub fn n_mono_rhi_NativeRhi__pushArenaScope0() -> Res<()> {
    let d = ARENA_SCOPE_DEPTH.with(|c| {
        let v = c.get();
        c.set(v + 1);
        v
    });
    if d >= 16 {
        ARENA_SCOPE_DEPTH.with(|c| c.set(16));
        return rt::throw("java/lang/IllegalArgumentException", "NativeRhi arena scope stack overflow".to_string());
    }
    Ok(())
}

pub fn n_mono_rhi_NativeRhi__popArenaScope0() -> Res<()> {
    let ok = ARENA_SCOPE_DEPTH.with(|c| {
        let v = c.get();
        if v == 0 {
            false
        } else {
            c.set(v - 1);
            true
        }
    });
    if !ok {
        return rt::throw("java/lang/IllegalArgumentException", "NativeRhi popArenaScope without pushArenaScope".to_string());
    }
    Ok(())
}

pub fn n_mono_rhi_NativeRhi__clearSections0() -> Res<()> {
    rhi_state().lock().unwrap().sections.clear();
    Ok(())
}

pub fn n_mono_rhi_NativeRhi__submitSectionSnapshot0(
    sx: i32, sy: i32, sz: i32, has_geom: i32, opaque: i32, portals: i64,
) -> Res<()> {
    n_mono_rhi_NativeRhi__submitSectionSnapshotEx0(sx, sy, sz, has_geom, opaque, portals, 36, 0, 0)
}

#[allow(clippy::too_many_arguments)] // JNI/Java native ABI: arity fixed by the Java signature
pub fn n_mono_rhi_NativeRhi__submitSectionSnapshotEx0(
    sx: i32, sy: i32, sz: i32, has_geom: i32, opaque: i32, portals: i64,
    index_count: i32, first_index: i32, vertex_offset: i32,
) -> Res<()> {
    if index_count < 0 || first_index < 0 {
        return rt::throw("java/lang/IllegalArgumentException", "negative indexCount/firstIndex".to_string());
    }
    let mut st = rhi_state().lock().unwrap();
    if st.sections.len() >= 65536 {
        st.dropped_sections += 1;
        return Ok(());
    }
    st.sections.push(SectionSnap {
        sx, sy, sz,
        has_non_air: has_geom != 0,
        opaque: opaque != 0,
        portals: portals as u64,
        index_count: index_count as u32,
        first_index: first_index as u32,
        vertex_offset,
    });
    Ok(())
}

#[allow(clippy::too_many_arguments)] // JNI/Java native ABI: arity fixed by the Java signature
pub fn n_mono_rhi_NativeRhi__cullWorld0(
    cam_x: f32, cam_y: f32, cam_z: f32, look_x: f32, look_y: f32, look_z: f32, fov_deg: f32, min_pixel_area: f32,
) -> Res<i32> {
    check_finite(&[cam_x, cam_y, cam_z, look_x, look_y, look_z, fov_deg, min_pixel_area])?;
    let mut st = rhi_state().lock().unwrap();
    if !st.initialized || !st.frame_open {
        return rt::throw("java/lang/IllegalArgumentException", "NativeRhi: no open frame".to_string());
    }
    let mut fwd_x = look_x - cam_x;
    let mut fwd_y = look_y - cam_y;
    let mut fwd_z = look_z - cam_z;
    let flen = (fwd_x * fwd_x + fwd_y * fwd_y + fwd_z * fwd_z).sqrt();
    if flen > 1e-5 {
        fwd_x /= flen;
        fwd_y /= flen;
        fwd_z /= flen;
    } else {
        fwd_x = 0.0;
        fwd_y = 0.0;
        fwd_z = -1.0;
    }
    let half_rad = (fov_deg.max(10.0) * 0.5).to_radians();
    let cos_Limit = (half_rad + 0.45).cos().min(-0.15);

    let mut vis: Vec<(f32, VisibleSec)> = Vec::new();
    for (idx, s) in st.sections.iter().enumerate() {
        if !s.has_non_air || s.index_count == 0 {
            continue;
        }
        let cx = s.sx as f32 * 16.0 + 8.0 - cam_x;
        let cy = s.sy as f32 * 16.0 + 8.0 - cam_y;
        let cz = s.sz as f32 * 16.0 + 8.0 - cam_z;
        let dist2 = cx * cx + cy * cy + cz * cz;
        let dist = dist2.sqrt();
        if dist > 540.0 {
            continue;
        }
        if dist > 28.0 {
            let dot = (cx * fwd_x + cy * fwd_y + cz * fwd_z) / dist;
            if dot < cos_Limit {
                continue;
            }
        }
        let lod = if dist < 64.0 { 0 } else if dist < 160.0 { 1 } else { 2 };
        vis.push((dist2, VisibleSec { idx, lod }));
    }
    vis.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    st.visible = vis.into_iter().map(|x| x.1).collect();
    Ok(st.visible.len() as i32)
}

pub fn n_mono_rhi_NativeRhi__visibleSectionCount0() -> Res<i32> {
    Ok(rhi_state().lock().unwrap().visible.len() as i32)
}

pub fn n_mono_rhi_NativeRhi__visibleSectionField0(index: i32, field: i32) -> Res<i32> {
    let st = rhi_state().lock().unwrap();
    if index < 0 || (index as usize) >= st.visible.len() {
        return rt::throw("java/lang/ArrayIndexOutOfBoundsException", index.to_string());
    }
    let v = st.visible[index as usize];
    let s = st.sections[v.idx];
    match field {
        0 => Ok(s.sx),
        1 => Ok(s.sy),
        2 => Ok(s.sz),
        3 => Ok(v.lod),
        _ => rt::throw("java/lang/IllegalArgumentException", "field must be 0..3".to_string()),
    }
}

pub fn n_mono_rhi_NativeRhi__registerMesh0(mesh_id: i32, index_count: i32, first_index: i32, vertex_offset: i32) -> Res<()> {
    if !(0..=65535).contains(&mesh_id) || index_count < 0 || first_index < 0 {
        return rt::throw("java/lang/IllegalArgumentException", "invalid mesh params".to_string());
    }
    let mut st = rhi_state().lock().unwrap();
    if !st.initialized || !st.frame_open {
        return rt::throw("java/lang/IllegalArgumentException", "no open frame".to_string());
    }
    st.meshes.insert(mesh_id, MeshEntry {
        index_count: index_count as u32,
        first_index: first_index as u32,
        vertex_offset,
    });
    Ok(())
}

#[allow(clippy::too_many_arguments)] // JNI/Java native ABI: arity fixed by the Java signature
pub fn n_mono_rhi_NativeRhi__pushInstance0(
    kind: i32, mesh_id: i32, x: f32, y: f32, z: f32, scale: f32, radius: f32, billboard: i32, _uv: i32, _col: i32,
) -> Res<()> {
    if !(0..=8).contains(&kind) || !(0..=3).contains(&billboard) || !(0..=65535).contains(&mesh_id) || radius < 0.0 {
        return rt::throw("java/lang/IllegalArgumentException", "invalid instance params".to_string());
    }
    check_finite(&[x, y, z, scale, radius])?;
    let mut st = rhi_state().lock().unwrap();
    if !st.initialized || !st.frame_open {
        return rt::throw("java/lang/IllegalArgumentException", "no open frame".to_string());
    }
    st.instances.push(InstanceEntry { kind, mesh_id, x, y, z, scale, radius });
    Ok(())
}

#[allow(clippy::too_many_arguments)] // JNI/Java native ABI: arity fixed by the Java signature
pub fn n_mono_rhi_NativeRhi__flushInstances0(
    cam_x: f32, cam_y: f32, cam_z: f32, look_x: f32, look_y: f32, look_z: f32, fov_deg: f32, min_pixel_area: f32,
) -> Res<i32> {
    check_finite(&[cam_x, cam_y, cam_z, look_x, look_y, look_z, fov_deg, min_pixel_area])?;
    let mut st = rhi_state().lock().unwrap();
    if !st.initialized || !st.frame_open {
        return rt::throw("java/lang/IllegalArgumentException", "no open frame".to_string());
    }
    let mut surviving = 0i32;
    let mut batches: std::collections::HashSet<(i32, i32)> = std::collections::HashSet::new();
    for inst in &st.instances {
        if !st.meshes.contains_key(&inst.mesh_id) || inst.scale <= 0.0 || inst.radius <= 0.0 {
            continue;
        }
        let dx = inst.x - cam_x;
        let dy = inst.y - cam_y;
        let dz = inst.z - cam_z;
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();
        if dist <= 512.0 + inst.radius {
            surviving += 1;
            batches.insert((inst.kind, inst.mesh_id));
        }
    }
    st.last_batches = batches.len() as i32;
    st.instances.clear();
    Ok(surviving)
}

pub fn n_mono_rhi_NativeRhi__lastDrawBatches0() -> Res<i32> {
    Ok(rhi_state().lock().unwrap().last_batches)
}
pub fn n_mono_rhi_NativeRhi__arenaBytes0() -> Res<i64> { Ok(65536) }
pub fn n_mono_rhi_NativeRhi__slabCommittedBytes0() -> Res<i64> { Ok(crate::heap::heap_bytes() as i64) }
pub fn n_mono_rhi_NativeRhi__slabReclaimedBytes0() -> Res<i64> {
    Ok(crate::heap::heap_bytes().saturating_sub(crate::heap::live_bytes()) as i64)
}
pub fn n_mono_rhi_NativeRhi__frameEpoch0() -> Res<i64> { Ok(FRAME_EPOCH.load(Ordering::Relaxed) as i64) }
pub fn n_mono_rhi_NativeRhi__threadCount0() -> Res<i64> { Ok(1) }
pub fn n_mono_rhi_NativeRhi__droppedSections0() -> Res<i64> { Ok(rhi_state().lock().unwrap().dropped_sections as i64) }

// ------------------------------------------------------------------ TouchCore Ana Makinesi ve GL/GLES3 Dinamik Dağıtımı

fn tc_lib() -> *mut c_void {
    static H: OnceLock<usize> = OnceLock::new();
    *H.get_or_init(|| dl_open_any(&["libtouchcore_mono.so"]) as usize) as *mut c_void
}


pub fn n_mono_rhi_NativeRhi__hasOsSurface0() -> Res<i32> {
    let h = [tc_lib()];
    if let Some(f) = sym_fn!(&h, "tc_android_has_window", extern "C" fn() -> i32) {
        return Ok((f() != 0) as i32);
    }
    if desk::is_ready() || desk::available() {
        return Ok(1);
    }
    Ok(0)
}
pub fn n_mono_rhi_NativeRhi__attachOsSurface0() -> Res<i32> {
    let h = [tc_lib()];
    if let Some(f) = sym_fn!(&h, "tc_android_init_egl", extern "C" fn(*mut i32, *mut i32) -> i32) {
        let (mut w, mut hp) = (1280, 720);
        if f(&mut w, &mut hp) != 0 {
            let mut st = rhi_state().lock().unwrap();
            st.viewport_w = w.max(1) as u32;
            st.viewport_h = hp.max(1) as u32;
            return Ok(1);
        }
    }
    if desk::try_init(1280, 720) {
        let (w, h) = desk::size();
        let mut st = rhi_state().lock().unwrap();
        st.viewport_w = w as u32;
        st.viewport_h = h as u32;
        return Ok(1);
    }
    Ok(0)
}
pub fn n_mono_rhi_NativeRhi__swapOsSurface0() -> Res<()> {
    // arabellek takası ekran tazelemesini (vsync) bekleyebilir; bu sırada GC bu iş parçacığını beklemesin (Java nesnesi kullanılmaz)
    let _blk = crate::heap::ScopedBlock::enter();
    let h = [tc_lib()];
    gs_frame();
    if let Some(f) = sym_fn!(&h, "tc_android_swap_buffers", extern "C" fn()) {
        f();
        return Ok(());
    }
    desk::swap();
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__osSurfaceWidth0() -> Res<i32> {
    let h = [tc_lib()];
    let (mut w, mut hp) = (1280, 720);
    if let Some(f) = sym_fn!(&h, "tc_android_get_size", extern "C" fn(*mut i32, *mut i32)) {
        f(&mut w, &mut hp);
        return Ok(w);
    }
    if desk::is_ready() { return Ok(desk::size().0); }
    Ok(w)
}
pub fn n_mono_rhi_NativeRhi__osSurfaceHeight0() -> Res<i32> {
    let h = [tc_lib()];
    let (mut w, mut hp) = (1280, 720);
    if let Some(f) = sym_fn!(&h, "tc_android_get_size", extern "C" fn(*mut i32, *mut i32)) {
        f(&mut w, &mut hp);
        return Ok(hp);
    }
    if desk::is_ready() { return Ok(desk::size().1); }
    Ok(hp)
}
pub fn n_mono_rhi_NativeRhi__pollText0(out_ref: Ref) -> Res<i32> {
    if out_ref.is_null() { return Ok(0); }
    let h = [tc_lib()];
    if let Some(f) = sym_fn!(&h, "tc_android_poll_text", extern "C" fn(*mut i32, i32) -> i32) {
        let a = out_ref.0 as *mut Arr;
        let len = unsafe { (*a).len };
        let ptr = unsafe { (a as *mut u8).add(24) as *mut i32 };
        return Ok(f(ptr, len / 3));
    }
    if desk::is_ready() {
        let a = out_ref.0 as *mut Arr;
        let len = unsafe { (*a).len } as usize;
        let buf = unsafe { std::slice::from_raw_parts_mut((a as *mut u8).add(24) as *mut i32, len) };
        return Ok(desk::take_text(buf, len / 3));
    }
    Ok(0)
}
pub fn n_mono_rhi_NativeRhi__setMouseGrabbed0(grabbed: i32) -> Res<()> {
    let h = [tc_lib()];
    if let Some(f) = sym_fn!(&h, "tc_android_set_grab", extern "C" fn(i32)) {
        f(if grabbed != 0 { 1 } else { 0 });
        return Ok(());
    }
    desk::set_grab(grabbed != 0);
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__pollOsInput0(btns_ref: Ref, axes_ref: Ref) -> Res<()> {
    let mut snap = TcPollSnapshot {
        width: 1280, height: 720, keys: [0; 19], _pad: [0; 2],
        hotbar_slot: 0, look_dx: 0.0, look_dy: 0.0, cursor_x: 0.0, cursor_y: 0.0,
    };
    let h = [tc_lib()];
    if let Some(f) = sym_fn!(&h, "tc_android_poll_snapshot", extern "C" fn(*mut TcPollSnapshot)) {
        f(&mut snap);
    } else {
        desk::poll(&mut snap);
    }
    if !btns_ref.is_null() {
        let a = btns_ref.0 as *mut Arr;
        if unsafe { (*a).len } >= 22 {
            let dst = unsafe { std::slice::from_raw_parts_mut((a as *mut u8).add(24) as *mut i32, 22) };
            for (d, &k) in dst[..19].iter_mut().zip(&snap.keys[..19]) { *d = k as i32; }
            dst[19] = snap.hotbar_slot;
            dst[20] = snap.width;
            dst[21] = snap.height;
        }
    }
    if !axes_ref.is_null() {
        let a = axes_ref.0 as *mut Arr;
        if unsafe { (*a).len } >= 4 {
            let dst = unsafe { std::slice::from_raw_parts_mut((a as *mut u8).add(24) as *mut f32, 4) };
            dst[0] = snap.look_dx;
            dst[1] = snap.look_dy;
            dst[2] = snap.cursor_x;
            dst[3] = snap.cursor_y;
        }
    }
    Ok(())
}

fn gl_lib() -> (*mut c_void, bool) {
    static H: OnceLock<(usize, bool)> = OnceLock::new();
    let (p, gles) = *H.get_or_init(|| {
        if cfg!(target_os = "android") {
            let es = dl_open_any(&["libGLESv3.so", "libGLESv2.so.2"]);
            if !es.is_null() {
                return (es as usize, true);
            }
        }
        (desk::gl_library() as usize, false)
    });
    (p as *mut c_void, gles)
}

fn has_gl() -> bool { !gl_lib().0.is_null() }

// ----- GPU tanı anahtarları (ikisi de varsayılan KAPALI; kapalıyken her çağrıda tek bir OnceLock okuması dışında iş yok)
//
// MONO_GL_STATS=1: saniyede bir `[mono:gl_stats]` satırı (unix ms damgalı; başlatıcının frametime.csv'siyle aynı saat):
//   kare sayısı, çizim çağrısı, çizilen indis/köşe (örnek sayısıyla çarpılmış), yüklenen bayt (BufferData/SubData/Copy),
//   eşleme (MapBufferRange) sayısı ve başarısız eşleme, içeriği bozulmuş arabellek (UnmapBuffer false: GLES'te veri tanımsız
//   olur ve yeniden yüklenmesi gerekir), atlanan çizim (taban köşeli çizim işlevi yok ve taban köşe 0 değil), GL hata sayısı.
//   Delik/eksik bölüm görülen anlarda çizim sayısı düşüyor mu, eşleme başarısız mı, bunu gösterir.
// MONO_GL_CHECK=1: her çizim ve arabellek çağrısından sonra glGetError; hata veren çağrı adıyla yazılır (ilk 200 satır).
struct GlDiag {
    stats: bool,
    check: bool,
}
fn gl_diag() -> &'static GlDiag {
    static D: OnceLock<GlDiag> = OnceLock::new();
    D.get_or_init(|| {
        let on = |n: &str| std::env::var(n).is_ok_and(|v| !v.is_empty() && v != "0");
        GlDiag { stats: on("MONO_GL_STATS"), check: on("MONO_GL_CHECK") }
    })
}
static GS_FRAMES: AtomicU64 = AtomicU64::new(0);
static GS_DRAWS: AtomicU64 = AtomicU64::new(0);
static GS_ELEMS: AtomicU64 = AtomicU64::new(0);
static GS_UPLOAD: AtomicU64 = AtomicU64::new(0);
static GS_MAPS: AtomicU64 = AtomicU64::new(0);
static GS_MAP_FAIL: AtomicU64 = AtomicU64::new(0);
static GS_UNMAP_LOST: AtomicU64 = AtomicU64::new(0);
static GS_SKIPPED: AtomicU64 = AtomicU64::new(0);
static GS_ERRORS: AtomicU64 = AtomicU64::new(0);
static GS_LAST_MS: AtomicU64 = AtomicU64::new(0);

#[inline]
fn gs_add(c: &AtomicU64, n: u64) {
    if gl_diag().stats {
        c.fetch_add(n, Ordering::Relaxed);
    }
}
#[inline]
fn gs_draw(count: i32, inst: i32) {
    if gl_diag().stats {
        GS_DRAWS.fetch_add(1, Ordering::Relaxed);
        GS_ELEMS.fetch_add(count.max(0) as u64 * inst.max(1) as u64, Ordering::Relaxed);
    }
}
/// MONO_GL_CHECK açıksa bekleyen GL hatalarını okur ve `what` adıyla yazar.
#[inline]
fn gl_check(what: &str) {
    if !gl_diag().check {
        return;
    }
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glGetError", extern "C" fn() -> u32) {
        for _ in 0..8 {
            let e = f();
            if e == 0 {
                break;
            }
            let n = GS_ERRORS.fetch_add(1, Ordering::Relaxed);
            if n < 200 {
                eprintln!("[mono:gl] {what}: error 0x{e:04x}");
            }
        }
    }
}
fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}
/// Her arabellek takasında: kare sayar, saniyede bir satır yazar ve sayaçları sıfırlar.
fn gs_frame() {
    if !gl_diag().stats {
        return;
    }
    GS_FRAMES.fetch_add(1, Ordering::Relaxed);
    let now = now_ms();
    let last = GS_LAST_MS.load(Ordering::Relaxed);
    if last == 0 {
        GS_LAST_MS.store(now, Ordering::Relaxed);
        return;
    }
    if now.saturating_sub(last) < 1000 {
        return;
    }
    GS_LAST_MS.store(now, Ordering::Relaxed);
    let t = |c: &AtomicU64| c.swap(0, Ordering::Relaxed);
    eprintln!(
        "[mono:gl_stats] t={now} frames={} draws={} elems={} upload_kb={} maps={} map_fail={} unmap_lost={} skipped={} gl_err={}",
        t(&GS_FRAMES), t(&GS_DRAWS), t(&GS_ELEMS), t(&GS_UPLOAD) / 1024, t(&GS_MAPS), t(&GS_MAP_FAIL), t(&GS_UNMAP_LOST),
        t(&GS_SKIPPED), t(&GS_ERRORS)
    );
}

pub fn n_mono_rhi_NativeRhi__glViewport0(x: i32, y: i32, w: i32, h: i32) -> Res<()> {
    {
        let mut st = rhi_state().lock().unwrap();
        st.viewport_w = w.max(1) as u32;
        st.viewport_h = h.max(1) as u32;
    }
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glViewport", extern "C" fn(i32, i32, i32, i32)) { f(x, y, w.max(1), h.max(1)); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glClearColor0(r: f32, g: f32, b: f32, a: f32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glClearColor", extern "C" fn(f32, f32, f32, f32)) { f(r, g, b, a); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glClear0(mask: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glClear", extern "C" fn(u32)) { f(mask as u32); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glEnable0(cap: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glEnable", extern "C" fn(u32)) { f(cap as u32); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glDisable0(cap: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glDisable", extern "C" fn(u32)) { f(cap as u32); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glBlendFunc0(s: i32, d: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glBlendFunc", extern "C" fn(u32, u32)) { f(s as u32, d as u32); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glDepthFunc0(func: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glDepthFunc", extern "C" fn(u32)) { f(func as u32); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glDepthMask0(flag: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glDepthMask", extern "C" fn(u8)) { f((flag != 0) as u8); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glColorMask0(r: i32, g: i32, b: i32, a: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glColorMask", extern "C" fn(u8, u8, u8, u8)) {
        f((r != 0) as u8, (g != 0) as u8, (b != 0) as u8, (a != 0) as u8);
    }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glCullFace0(m: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glCullFace", extern "C" fn(u32)) { f(m as u32); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glFrontFace0(m: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glFrontFace", extern "C" fn(u32)) { f(m as u32); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glScissor0(x: i32, y: i32, w: i32, h: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glScissor", extern "C" fn(i32, i32, i32, i32)) { f(x, y, w, h); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glClearDepth0(d: f64) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glClearDepthf", extern "C" fn(f32)) { f(d as f32); }
    else if let Some(f) = sym_fn!(&libs, "glClearDepth", extern "C" fn(f64)) { f(d); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glDepthRange0(n: f64, fval: f64) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glDepthRangef", extern "C" fn(f32, f32)) { f(n as f32, fval as f32); }
    else if let Some(f) = sym_fn!(&libs, "glDepthRange", extern "C" fn(f64, f64)) { f(n, fval); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glPolygonMode0(_f: i32, _m: i32) -> Res<()> { Ok(()) }
pub fn n_mono_rhi_NativeRhi__glGetError0() -> Res<i32> {
    let libs = [gl_lib().0];
    Ok(sym_fn!(&libs, "glGetError", extern "C" fn() -> u32).map(|f| f() as i32).unwrap_or(0))
}
pub fn n_mono_rhi_NativeRhi__glGetInteger0(pname: i32) -> Res<i32> {
    let libs = [gl_lib().0];
    let mut out = 0i32;
    if let Some(f) = sym_fn!(&libs, "glGetIntegerv", extern "C" fn(u32, *mut i32)) { f(pname as u32, &mut out); }
    if out == 0 {
        out = match pname as u32 {
            0x8A34 => 256,   // GL_UNIFORM_BUFFER_OFFSET_ALIGNMENT
            0x0D33 => 4096,  // GL_MAX_TEXTURE_SIZE
            0x8A30 => 16384, // GL_MAX_UNIFORM_BLOCK_SIZE
            0x88FF => 256,   // GL_MAX_ARRAY_TEXTURE_LAYERS
            _ => 0,
        };
    }
    Ok(out)
}
pub fn n_mono_rhi_NativeRhi__glGetString0(name: i32) -> Res<Ref> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glGetString", extern "C" fn(u32) -> *const c_char) {
        let p = f(name as u32);
        if !p.is_null() {
            return Ok(rt::new_str(unsafe { CStr::from_ptr(p).to_string_lossy().into_owned() }));
        }
    }
    Ok(Ref::NULL)
}

pub fn n_mono_rhi_NativeRhi__glGenBuffer0() -> Res<i32> {
    let libs = [gl_lib().0];
    let mut id = 0u32;
    if let Some(f) = sym_fn!(&libs, "glGenBuffers", extern "C" fn(i32, *mut u32)) { f(1, &mut id); }
    Ok(id as i32)
}
pub fn n_mono_rhi_NativeRhi__glDeleteBuffer0(id: i32) -> Res<()> {
    let libs = [gl_lib().0];
    let uid = id as u32;
    if uid != 0 { if let Some(f) = sym_fn!(&libs, "glDeleteBuffers", extern "C" fn(i32, *const u32)) { f(1, &uid); } }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glBindBuffer0(t: i32, id: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glBindBuffer", extern "C" fn(u32, u32)) { f(t as u32, id as u32); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glBufferData0(t: i32, addr: i64, sz: i64, usage: i32) -> Res<()> {
    gs_add(&GS_UPLOAD, if addr != 0 { sz.max(0) as u64 } else { 0 });
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glBufferData", extern "C" fn(u32, isize, *const c_void, u32)) {
        f(t as u32, sz.max(0) as isize, addr as usize as *const c_void, usage as u32);
    }
    gl_check("glBufferData");
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glBufferSubData0(t: i32, off: i64, addr: i64, sz: i64) -> Res<()> {
    gs_add(&GS_UPLOAD, sz.max(0) as u64);
    let libs = [gl_lib().0];
    if addr != 0 {
        if let Some(f) = sym_fn!(&libs, "glBufferSubData", extern "C" fn(u32, isize, isize, *const c_void)) {
            f(t as u32, off as isize, sz.max(0) as isize, addr as usize as *const c_void);
        }
    }
    gl_check("glBufferSubData");
    Ok(())
}

pub fn n_mono_rhi_NativeRhi__glGenVertexArray0() -> Res<i32> {
    let libs = [gl_lib().0];
    let mut id = 0u32;
    if let Some(f) = sym_fn!(&libs, "glGenVertexArrays", extern "C" fn(i32, *mut u32)) { f(1, &mut id); }
    Ok(id as i32)
}
pub fn n_mono_rhi_NativeRhi__glDeleteVertexArray0(id: i32) -> Res<()> {
    let libs = [gl_lib().0];
    let uid = id as u32;
    if uid != 0 { if let Some(f) = sym_fn!(&libs, "glDeleteVertexArrays", extern "C" fn(i32, *const u32)) { f(1, &uid); } }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glBindVertexArray0(id: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glBindVertexArray", extern "C" fn(u32)) { f(id as u32); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glEnableVertexAttribArray0(idx: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glEnableVertexAttribArray", extern "C" fn(u32)) { f(idx as u32); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glDisableVertexAttribArray0(idx: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glDisableVertexAttribArray", extern "C" fn(u32)) { f(idx as u32); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glVertexAttribPointer0(idx: i32, sz: i32, typ: i32, norm: i32, stride: i32, off: i64) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glVertexAttribPointer", extern "C" fn(u32, i32, u32, u8, i32, *const c_void)) {
        f(idx as u32, sz, typ as u32, (norm != 0) as u8, stride, off as usize as *const c_void);
    }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glVertexAttribIPointer0(idx: i32, sz: i32, typ: i32, stride: i32, off: i64) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glVertexAttribIPointer", extern "C" fn(u32, i32, u32, i32, *const c_void)) {
        f(idx as u32, sz, typ as u32, stride, off as usize as *const c_void);
    }
    Ok(())
}

pub fn n_mono_rhi_NativeRhi__glCreateShader0(kind: i32) -> Res<i32> {
    let libs = [gl_lib().0];
    Ok(sym_fn!(&libs, "glCreateShader", extern "C" fn(u32) -> u32).map(|f| f(kind as u32) as i32).unwrap_or(0))
}
pub fn n_mono_rhi_NativeRhi__glDeleteShader0(sh: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if sh != 0 { if let Some(f) = sym_fn!(&libs, "glDeleteShader", extern "C" fn(u32)) { f(sh as u32); } }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glShaderSource0(sh: i32, src_ref: Ref) -> Res<()> {
    let (h, is_gles) = gl_lib();
    let libs = [h];
    let Some(sfn) = sym_fn!(&libs, "glShaderSource", extern "C" fn(u32, i32, *const *const u8, *const i32)) else { return Ok(()); };
    let raw = rt::str_to_rust(rt::nn_ref(src_ref)?);
    if is_gles {
        let first_line = raw.lines().next().unwrap_or("");
        let is_es = first_line.starts_with("#version") && first_line.contains(" es");
        if !is_es {
            let body = if raw.starts_with("#version") {
                raw.find('\n').map(|i| &raw[i + 1..]).unwrap_or("")
            } else {
                &raw
            };
            let hdr = "#version 300 es\nprecision highp float;\nprecision highp int;\n";
            let ptrs = [hdr.as_ptr(), body.as_ptr()];
            let lens = [hdr.len() as i32, body.len() as i32];
            sfn(sh as u32, 2, ptrs.as_ptr(), lens.as_ptr());
            return Ok(());
        }
    }
    let ptrs = [raw.as_ptr()];
    let lens = [raw.len() as i32];
    sfn(sh as u32, 1, ptrs.as_ptr(), lens.as_ptr());
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glCompileShader0(sh: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glCompileShader", extern "C" fn(u32)) { f(sh as u32); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glGetShaderi0(sh: i32, pname: i32) -> Res<i32> {
    let libs = [gl_lib().0];
    let mut v = 0i32;
    if let Some(f) = sym_fn!(&libs, "glGetShaderiv", extern "C" fn(u32, u32, *mut i32)) { f(sh as u32, pname as u32, &mut v); }
    Ok(v)
}

pub fn n_mono_rhi_NativeRhi__glCreateProgram0() -> Res<i32> {
    let libs = [gl_lib().0];
    Ok(sym_fn!(&libs, "glCreateProgram", extern "C" fn() -> u32).map(|f| f() as i32).unwrap_or(0))
}
pub fn n_mono_rhi_NativeRhi__glDeleteProgram0(p: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if p != 0 { if let Some(f) = sym_fn!(&libs, "glDeleteProgram", extern "C" fn(u32)) { f(p as u32); } }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glAttachShader0(p: i32, sh: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glAttachShader", extern "C" fn(u32, u32)) { f(p as u32, sh as u32); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glLinkProgram0(p: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glLinkProgram", extern "C" fn(u32)) { f(p as u32); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glUseProgram0(p: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glUseProgram", extern "C" fn(u32)) { f(p as u32); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glGetProgrami0(p: i32, pname: i32) -> Res<i32> {
    let libs = [gl_lib().0];
    let mut v = 0i32;
    if let Some(f) = sym_fn!(&libs, "glGetProgramiv", extern "C" fn(u32, u32, *mut i32)) { f(p as u32, pname as u32, &mut v); }
    Ok(v)
}
pub fn n_mono_rhi_NativeRhi__glGetUniformLocation0(p: i32, name_ref: Ref) -> Res<i32> {
    let libs = [gl_lib().0];
    let cn = rt::c_str_arg(name_ref)?;
    Ok(sym_fn!(&libs, "glGetUniformLocation", extern "C" fn(u32, *const c_char) -> i32).map(|f| f(p as u32, cn.as_ptr())).unwrap_or(-1))
}
pub fn n_mono_rhi_NativeRhi__glUniform1i0(loc: i32, v0: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glUniform1i", extern "C" fn(i32, i32)) { f(loc, v0); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glUniform1f0(loc: i32, v0: f32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glUniform1f", extern "C" fn(i32, f32)) { f(loc, v0); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glUniform2f0(loc: i32, v0: f32, v1: f32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glUniform2f", extern "C" fn(i32, f32, f32)) { f(loc, v0, v1); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glUniform3f0(loc: i32, v0: f32, v1: f32, v2: f32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glUniform3f", extern "C" fn(i32, f32, f32, f32)) { f(loc, v0, v1, v2); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glUniform4f0(loc: i32, v0: f32, v1: f32, v2: f32, v3: f32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glUniform4f", extern "C" fn(i32, f32, f32, f32, f32)) { f(loc, v0, v1, v2, v3); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glUniformMatrix4fv0(loc: i32, count: i32, trans: i32, addr: i64) -> Res<()> {
    let libs = [gl_lib().0];
    if addr != 0 {
        if let Some(f) = sym_fn!(&libs, "glUniformMatrix4fv", extern "C" fn(i32, i32, u8, *const f32)) {
            f(loc, count, (trans != 0) as u8, addr as usize as *const f32);
        }
    }
    Ok(())
}

pub fn n_mono_rhi_NativeRhi__glGenTexture0() -> Res<i32> {
    let libs = [gl_lib().0];
    let mut id = 0u32;
    if let Some(f) = sym_fn!(&libs, "glGenTextures", extern "C" fn(i32, *mut u32)) { f(1, &mut id); }
    Ok(id as i32)
}
pub fn n_mono_rhi_NativeRhi__glDeleteTexture0(id: i32) -> Res<()> {
    let libs = [gl_lib().0];
    let uid = id as u32;
    if uid != 0 { if let Some(f) = sym_fn!(&libs, "glDeleteTextures", extern "C" fn(i32, *const u32)) { f(1, &uid); } }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glActiveTexture0(t: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glActiveTexture", extern "C" fn(u32)) { f(t as u32); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glBindTexture0(t: i32, id: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glBindTexture", extern "C" fn(u32, u32)) { f(t as u32, id as u32); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glTexParameteri0(t: i32, p: i32, v: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glTexParameteri", extern "C" fn(u32, u32, i32)) { f(t as u32, p as u32, v); }
    Ok(())
}
#[allow(clippy::too_many_arguments)] // JNI/Java native ABI: arity fixed by the Java signature
pub fn n_mono_rhi_NativeRhi__glTexImage2D0(t: i32, lvl: i32, ifmt: i32, w: i32, h: i32, border: i32, fmt: i32, typ: i32, addr: i64) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glTexImage2D", extern "C" fn(u32, i32, i32, i32, i32, i32, u32, u32, *const c_void)) {
        f(t as u32, lvl, ifmt, w, h, border, fmt as u32, typ as u32, addr as usize as *const c_void);
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)] // JNI/Java native ABI: arity fixed by the Java signature
pub fn n_mono_rhi_NativeRhi__glTexSubImage2D0(t: i32, lvl: i32, xo: i32, yo: i32, w: i32, h: i32, fmt: i32, typ: i32, addr: i64) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glTexSubImage2D", extern "C" fn(u32, i32, i32, i32, i32, i32, u32, u32, *const c_void)) {
        f(t as u32, lvl, xo, yo, w, h, fmt as u32, typ as u32, addr as usize as *const c_void);
    }
    Ok(())
}

pub fn n_mono_rhi_NativeRhi__glGenFramebuffer0() -> Res<i32> {
    let libs = [gl_lib().0];
    let mut id = 0u32;
    if let Some(f) = sym_fn!(&libs, "glGenFramebuffers", extern "C" fn(i32, *mut u32)) { f(1, &mut id); }
    Ok(id as i32)
}
pub fn n_mono_rhi_NativeRhi__glDeleteFramebuffer0(id: i32) -> Res<()> {
    let libs = [gl_lib().0];
    let uid = id as u32;
    if uid != 0 { if let Some(f) = sym_fn!(&libs, "glDeleteFramebuffers", extern "C" fn(i32, *const u32)) { f(1, &uid); } }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glBindFramebuffer0(t: i32, id: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glBindFramebuffer", extern "C" fn(u32, u32)) { f(t as u32, id as u32); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glFramebufferTexture2D0(t: i32, a: i32, tt: i32, tex: i32, lvl: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glFramebufferTexture2D", extern "C" fn(u32, u32, u32, u32, i32)) {
        f(t as u32, a as u32, tt as u32, tex as u32, lvl);
    }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glCheckFramebufferStatus0(t: i32) -> Res<i32> {
    let libs = [gl_lib().0];
    Ok(sym_fn!(&libs, "glCheckFramebufferStatus", extern "C" fn(u32) -> u32).map(|f| f(t as u32) as i32).unwrap_or(0x8CD5))
}

pub fn n_mono_rhi_NativeRhi__glDrawArrays0(m: i32, first: i32, count: i32) -> Res<()> {
    gs_draw(count, 1);
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glDrawArrays", extern "C" fn(u32, i32, i32)) { f(m as u32, first, count); }
    gl_check("glDrawArrays");
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glDrawElements0(m: i32, count: i32, typ: i32, off: i64) -> Res<()> {
    gs_draw(count, 1);
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glDrawElements", extern "C" fn(u32, i32, u32, *const c_void)) {
        f(m as u32, count, typ as u32, off as usize as *const c_void);
    }
    gl_check("glDrawElements");
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glDrawElementsInstanced0(m: i32, count: i32, typ: i32, off: i64, icount: i32) -> Res<()> {
    gs_draw(count, icount);
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glDrawElementsInstanced", extern "C" fn(u32, i32, u32, *const c_void, i32)) {
        f(m as u32, count, typ as u32, off as usize as *const c_void, icount);
    }
    gl_check("glDrawElementsInstanced");
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glDrawArraysInstanced0(m: i32, first: i32, count: i32, icount: i32) -> Res<()> {
    gs_draw(count, icount);
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glDrawArraysInstanced", extern "C" fn(u32, i32, i32, i32)) {
        f(m as u32, first, count, icount);
    }
    gl_check("glDrawArraysInstanced");
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glDrawElementsInstancedBaseVertex0(m: i32, count: i32, typ: i32, off: i64, icount: i32, bv: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glDrawElementsInstancedBaseVertex", extern "C" fn(u32, i32, u32, *const c_void, i32, i32)) {
        gs_draw(count, icount);
        f(m as u32, count, typ as u32, off as usize as *const c_void, icount, bv);
        gl_check("glDrawElementsInstancedBaseVertex");
    } else if bv == 0 {
        n_mono_rhi_NativeRhi__glDrawElementsInstanced0(m, count, typ, off, icount)?;
    } else {
        gs_add(&GS_SKIPPED, 1); // işlev yok ve taban köşe 0 değil: çizilemez (MONO_GL_STATS'ta sayılır)
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)] // JNI/Java native ABI: arity fixed by the Java signature
pub fn n_mono_rhi_NativeRhi__glBlitFramebuffer0(sx0: i32, sy0: i32, sx1: i32, sy1: i32, dx0: i32, dy0: i32, dx1: i32, dy1: i32, mask: i32, filter: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glBlitFramebuffer", extern "C" fn(i32, i32, i32, i32, i32, i32, i32, i32, u32, u32)) {
        f(sx0, sy0, sx1, sy1, dx0, dy0, dx1, dy1, mask as u32, filter as u32);
    }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glBindAttribLocation0(prog: i32, idx: i32, name_ref: Ref) -> Res<()> {
    let libs = [gl_lib().0];
    let cn = rt::c_str_arg(name_ref)?;
    if let Some(f) = sym_fn!(&libs, "glBindAttribLocation", extern "C" fn(u32, u32, *const c_char)) {
        f(prog as u32, idx as u32, cn.as_ptr());
    }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glBindBufferRange0(t: i32, idx: i32, buf: i32, off: i64, sz: i64) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glBindBufferRange", extern "C" fn(u32, u32, u32, isize, isize)) {
        f(t as u32, idx as u32, buf as u32, off as isize, sz as isize);
    }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glCopyBufferSubData0(rtgt: i32, wtgt: i32, roff: i64, woff: i64, sz: i64) -> Res<()> {
    gs_add(&GS_UPLOAD, sz.max(0) as u64);
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glCopyBufferSubData", extern "C" fn(u32, u32, isize, isize, isize)) {
        f(rtgt as u32, wtgt as u32, roff as isize, woff as isize, sz as isize);
    }
    gl_check("glCopyBufferSubData");
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glMapBufferRange0(t: i32, off: i64, len: i64, access: i32) -> Res<i64> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glMapBufferRange", extern "C" fn(u32, isize, isize, u32) -> *mut c_void) {
        let p = f(t as u32, off as isize, len.max(0) as isize, access as u32) as usize as i64;
        gs_add(&GS_MAPS, 1);
        if p == 0 {
            gs_add(&GS_MAP_FAIL, 1);
        }
        gl_check("glMapBufferRange");
        return Ok(p);
    }
    Ok(0)
}
pub fn n_mono_rhi_NativeRhi__glUnmapBuffer0(t: i32) -> Res<i32> {
    let libs = [gl_lib().0];
    let r = sym_fn!(&libs, "glUnmapBuffer", extern "C" fn(u32) -> u8).map(|f| (f(t as u32) != 0) as i32).unwrap_or(1);
    if r == 0 {
        gs_add(&GS_UNMAP_LOST, 1);
    }
    gl_check("glUnmapBuffer");
    Ok(r)
}
pub fn n_mono_rhi_NativeRhi__glFlushMappedBufferRange0(t: i32, off: i64, len: i64) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glFlushMappedBufferRange", extern "C" fn(u32, isize, isize)) {
        f(t as u32, off as isize, len.max(0) as isize);
    }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glGenSampler0() -> Res<i32> {
    let libs = [gl_lib().0];
    let mut id = 0u32;
    if let Some(f) = sym_fn!(&libs, "glGenSamplers", extern "C" fn(i32, *mut u32)) { f(1, &mut id); }
    Ok(id as i32)
}
pub fn n_mono_rhi_NativeRhi__glDeleteSampler0(id: i32) -> Res<()> {
    let libs = [gl_lib().0];
    let uid = id as u32;
    if uid != 0 { if let Some(f) = sym_fn!(&libs, "glDeleteSamplers", extern "C" fn(i32, *const u32)) { f(1, &uid); } }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glBindSampler0(u: i32, s: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glBindSampler", extern "C" fn(u32, u32)) { f(u as u32, s as u32); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glSamplerParameteri0(s: i32, p: i32, v: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glSamplerParameteri", extern "C" fn(u32, u32, i32)) { f(s as u32, p as u32, v); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glSamplerParameterf0(s: i32, p: i32, v: f32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glSamplerParameterf", extern "C" fn(u32, u32, f32)) { f(s as u32, p as u32, v); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glBlendEquationSeparate0(rgb: i32, a: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glBlendEquationSeparate", extern "C" fn(u32, u32)) { f(rgb as u32, a as u32); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glBlendFuncSeparate0(sr: i32, dr: i32, sa: i32, da: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glBlendFuncSeparate", extern "C" fn(u32, u32, u32, u32)) {
        f(sr as u32, dr as u32, sa as u32, da as u32);
    }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glClearBufferfv0(buf: i32, db: i32, arr_ref: Ref) -> Res<()> {
    if arr_ref.is_null() { return Ok(()); }
    let a = arr_ref.0 as *const Arr;
    if unsafe { (*a).len } < 4 { return Ok(()); }
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glClearBufferfv", extern "C" fn(u32, i32, *const f32)) {
        f(buf as u32, db, unsafe { (a as *const u8).add(24) as *const f32 });
    }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glColorMaski0(idx: i32, r: i32, g: i32, b: i32, a: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glColorMaski", extern "C" fn(u32, u8, u8, u8, u8)) {
        f(idx as u32, (r != 0) as u8, (g != 0) as u8, (b != 0) as u8, (a != 0) as u8);
    } else if idx == 0 {
        n_mono_rhi_NativeRhi__glColorMask0(r, g, b, a)?;
    }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glEnablei0(cap: i32, idx: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glEnablei", extern "C" fn(u32, u32)) { f(cap as u32, idx as u32); }
    else if idx == 0 { n_mono_rhi_NativeRhi__glEnable0(cap)?; }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glDisablei0(cap: i32, idx: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glDisablei", extern "C" fn(u32, u32)) { f(cap as u32, idx as u32); }
    else if idx == 0 { n_mono_rhi_NativeRhi__glDisable0(cap)?; }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glDrawBuffers0(arr_ref: Ref) -> Res<()> {
    if arr_ref.is_null() { return Ok(()); }
    let a = arr_ref.0 as *const Arr;
    let n = unsafe { (*a).len };
    if n <= 0 { return Ok(()); }
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glDrawBuffers", extern "C" fn(i32, *const u32)) {
        f(n, unsafe { (a as *const u8).add(24) as *const u32 });
    }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glReadBuffer0(src: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glReadBuffer", extern "C" fn(u32)) { f(src as u32); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glFinish0() -> Res<()> {
    let _blk = crate::heap::ScopedBlock::enter(); // GPU bitene kadar bekler; Java nesnesi kullanılmaz
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glFinish", extern "C" fn()) { f(); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glGetFloat0(pname: i32) -> Res<f32> {
    let libs = [gl_lib().0];
    let mut v = 0.0f32;
    if let Some(f) = sym_fn!(&libs, "glGetFloatv", extern "C" fn(u32, *mut f32)) { f(pname as u32, &mut v); }
    Ok(v)
}
pub fn n_mono_rhi_NativeRhi__glGetInteger640(pname: i32) -> Res<i64> {
    let libs = [gl_lib().0];
    let mut v = 0i64;
    if let Some(f) = sym_fn!(&libs, "glGetInteger64v", extern "C" fn(u32, *mut i64)) { f(pname as u32, &mut v); }
    Ok(v)
}
pub fn n_mono_rhi_NativeRhi__glGetShaderInfoLog0(sh: i32) -> Res<Ref> {
    let libs = [gl_lib().0];
    let mut buf = [0u8; 2048];
    let mut len = 0i32;
    if let Some(f) = sym_fn!(&libs, "glGetShaderInfoLog", extern "C" fn(u32, i32, *mut i32, *mut u8)) {
        f(sh as u32, buf.len() as i32, &mut len, buf.as_mut_ptr());
    }
    let n = (len.max(0) as usize).min(buf.len());
    Ok(rt::new_str(String::from_utf8_lossy(&buf[..n]).into_owned()))
}
pub fn n_mono_rhi_NativeRhi__glGetProgramInfoLog0(prog: i32) -> Res<Ref> {
    let libs = [gl_lib().0];
    let mut buf = [0u8; 2048];
    let mut len = 0i32;
    if let Some(f) = sym_fn!(&libs, "glGetProgramInfoLog", extern "C" fn(u32, i32, *mut i32, *mut u8)) {
        f(prog as u32, buf.len() as i32, &mut len, buf.as_mut_ptr());
    }
    let n = (len.max(0) as usize).min(buf.len());
    Ok(rt::new_str(String::from_utf8_lossy(&buf[..n]).into_owned()))
}
pub fn n_mono_rhi_NativeRhi__glGetUniformBlockIndex0(prog: i32, name_ref: Ref) -> Res<i32> {
    let libs = [gl_lib().0];
    let cn = rt::c_str_arg(name_ref)?;
    Ok(sym_fn!(&libs, "glGetUniformBlockIndex", extern "C" fn(u32, *const c_char) -> u32).map(|f| f(prog as u32, cn.as_ptr()) as i32).unwrap_or(-1))
}
pub fn n_mono_rhi_NativeRhi__glUniformBlockBinding0(prog: i32, idx: i32, b: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glUniformBlockBinding", extern "C" fn(u32, u32, u32)) { f(prog as u32, idx as u32, b as u32); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glPixelStorei0(p: i32, v: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glPixelStorei", extern "C" fn(u32, i32)) { f(p as u32, v); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glPolygonOffset0(factor: f32, units: f32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glPolygonOffset", extern "C" fn(f32, f32)) { f(factor, units); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glReadPixels0(x: i32, y: i32, w: i32, h: i32, fmt: i32, typ: i32, addr: i64) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glReadPixels", extern "C" fn(i32, i32, i32, i32, u32, u32, *mut c_void)) {
        f(x, y, w, h, fmt as u32, typ as u32, addr as usize as *mut c_void);
    }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glTexBuffer0(t: i32, ifmt: i32, buf: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glTexBuffer", extern "C" fn(u32, u32, u32)) { f(t as u32, ifmt as u32, buf as u32); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glGetTexLevelParameteri0(t: i32, lvl: i32, p: i32) -> Res<i32> {
    let libs = [gl_lib().0];
    let mut v = 0i32;
    if let Some(f) = sym_fn!(&libs, "glGetTexLevelParameteriv", extern "C" fn(u32, i32, u32, *mut i32)) {
        f(t as u32, lvl, p as u32, &mut v);
    }
    Ok(v)
}
pub fn n_mono_rhi_NativeRhi__glGenQuery0() -> Res<i32> {
    let libs = [gl_lib().0];
    let mut id = 0u32;
    if let Some(f) = sym_fn!(&libs, "glGenQueries", extern "C" fn(i32, *mut u32)) { f(1, &mut id); }
    Ok(id as i32)
}
pub fn n_mono_rhi_NativeRhi__glDeleteQuery0(id: i32) -> Res<()> {
    let libs = [gl_lib().0];
    let uid = id as u32;
    if uid != 0 { if let Some(f) = sym_fn!(&libs, "glDeleteQueries", extern "C" fn(i32, *const u32)) { f(1, &uid); } }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glQueryCounter0(id: i32, t: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glQueryCounter", extern "C" fn(u32, u32)) { f(id as u32, t as u32); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glGetQueryObjecti0(id: i32, p: i32) -> Res<i32> {
    let libs = [gl_lib().0];
    let mut v = 0u32;
    if let Some(f) = sym_fn!(&libs, "glGetQueryObjectuiv", extern "C" fn(u32, u32, *mut u32)) { f(id as u32, p as u32, &mut v); }
    Ok(v as i32)
}
pub fn n_mono_rhi_NativeRhi__glGetQueryObjectui640(id: i32, p: i32) -> Res<i64> {
    let libs = [gl_lib().0];
    let mut v = 0u64;
    if let Some(f) = sym_fn!(&libs, "glGetQueryObjectui64v", extern "C" fn(u32, u32, *mut u64)) { f(id as u32, p as u32, &mut v); }
    Ok(v as i64)
}
pub fn n_mono_rhi_NativeRhi__glVertexAttribDivisor0(idx: i32, div: i32) -> Res<()> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glVertexAttribDivisor", extern "C" fn(u32, u32)) { f(idx as u32, div as u32); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glFenceSync0(cond: i32, flags: i32) -> Res<i64> {
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glFenceSync", extern "C" fn(u32, u32) -> *mut c_void) {
        let s = f(cond as u32, flags as u32);
        if !s.is_null() { return Ok(s as usize as i64); }
    }
    Ok(1)
}
pub fn n_mono_rhi_NativeRhi__glDeleteSync0(sync: i64) -> Res<()> {
    if sync <= 1 { return Ok(()); }
    let libs = [gl_lib().0];
    if let Some(f) = sym_fn!(&libs, "glDeleteSync", extern "C" fn(*mut c_void)) { f(sync as usize as *mut c_void); }
    Ok(())
}
pub fn n_mono_rhi_NativeRhi__glClientWaitSync0(sync: i64, flags: i32, timeout_ns: i64) -> Res<i32> {
    if sync <= 1 { return Ok(0x911A); }
    let _blk = crate::heap::ScopedBlock::enter(); // zaman aşımına kadar bekleyebilir; Java nesnesi kullanılmaz
    let libs = [gl_lib().0];
    Ok(sym_fn!(&libs, "glClientWaitSync", extern "C" fn(*mut c_void, u32, u64) -> u32)
        .map(|f| f(sync as usize as *mut c_void, flags as u32, timeout_ns as u64) as i32)
        .unwrap_or(0x911A))
}

// ------------------------------------------------------------------ OpenAL uyumlu ses (audio.rs, audio_dev.rs)
// Java tarafı (runtime/java/mono/rhi/NativeRhi.java) adlarını korur; işi Rust karıştırıcısı yapar.
fn al_ctx() -> std::sync::Arc<std::sync::Mutex<crate::audio::Context>> {
    crate::audio_dev::context()
}

#[inline(never)] // exported symbol: the generated code links to it (small leaf fns are otherwise not exported)
pub fn n_mono_rhi_NativeRhi__alcOpenDevice0(_name_ref: Ref) -> Res<i64> {
    crate::audio_dev::start();
    Ok(1)
}
#[inline(never)] // exported symbol: the generated code links to it (small leaf fns are otherwise not exported)
pub fn n_mono_rhi_NativeRhi__alcCloseDevice0(_d: i64) -> Res<i32> {
    crate::audio_dev::stop();
    Ok(1)
}
#[inline(never)] // exported symbol: the generated code links to it (small leaf fns are otherwise not exported)
pub fn n_mono_rhi_NativeRhi__alcCreateContext0(d: i64, _attr: i64) -> Res<i64> {
    Ok(if d != 0 { 1 } else { 0 })
}
#[inline(never)] // exported symbol: the generated code links to it (small leaf fns are otherwise not exported)
pub fn n_mono_rhi_NativeRhi__alcMakeContextCurrent0(c: i64) -> Res<i32> {
    Ok((c != 0) as i32)
}
#[inline(never)] // exported symbol: the generated code links to it (small leaf fns are otherwise not exported)
pub fn n_mono_rhi_NativeRhi__alcDestroyContext0(_c: i64) -> Res<()> {
    Ok(())
}
#[inline(never)] // exported symbol: the generated code links to it (small leaf fns are otherwise not exported)
pub fn n_mono_rhi_NativeRhi__alcGetInteger0(_d: i64, p: i32) -> Res<i32> {
    // ALC_FREQUENCY, ALC_REFRESH, ALC_SYNC, ALC_MONO_SOURCES, ALC_STEREO_SOURCES
    Ok(match p {
        0x1007 => crate::audio::MIX_RATE as i32,
        0x1008 => 60,
        0x1009 => 0,
        0x1010 => 256,
        0x1011 => 64,
        _ => 0,
    })
}
#[inline(never)] // exported symbol: the generated code links to it (small leaf fns are otherwise not exported)
pub fn n_mono_rhi_NativeRhi__alcGetString0(_d: i64, p: i32) -> Res<Ref> {
    // ALC_DEVICE_SPECIFIER, ALC_EXTENSIONS
    Ok(match p {
        0x1005 => rt::new_str("Monolith Audio".to_string()),
        0x1006 => rt::new_str(String::new()),
        _ => Ref::NULL,
    })
}

#[inline(never)] // exported symbol: the generated code links to it (small leaf fns are otherwise not exported)
pub fn n_mono_rhi_NativeRhi__alGenBuffer0() -> Res<i32> {
    Ok(al_ctx().lock().unwrap().gen_buffers(1)[0] as i32)
}
#[inline(never)] // exported symbol: the generated code links to it (small leaf fns are otherwise not exported)
pub fn n_mono_rhi_NativeRhi__alDeleteBuffer0(id: i32) -> Res<()> {
    al_ctx().lock().unwrap().delete_buffers(&[id as u32]);
    Ok(())
}
#[inline(never)] // exported symbol: the generated code links to it (small leaf fns are otherwise not exported)
pub fn n_mono_rhi_NativeRhi__alBufferData0(b: i32, fmt: i32, addr: i64, sz: i32, freq: i32) -> Res<()> {
    let data: &[u8] = if addr == 0 || sz <= 0 {
        &[]
    } else {
        // çağıranın doğrudan bir ByteBuffer'ının adresi: çağrı süresince geçerli
        unsafe { std::slice::from_raw_parts(addr as usize as *const u8, sz as usize) }
    };
    al_ctx().lock().unwrap().buffer_data(b as u32, fmt, data, freq);
    Ok(())
}
#[inline(never)] // exported symbol: the generated code links to it (small leaf fns are otherwise not exported)
pub fn n_mono_rhi_NativeRhi__alGenSource0() -> Res<i32> {
    Ok(al_ctx().lock().unwrap().gen_sources(1)[0] as i32)
}
#[inline(never)] // exported symbol: the generated code links to it (small leaf fns are otherwise not exported)
pub fn n_mono_rhi_NativeRhi__alDeleteSource0(id: i32) -> Res<()> {
    al_ctx().lock().unwrap().delete_sources(&[id as u32]);
    Ok(())
}
#[inline(never)] // exported symbol: the generated code links to it (small leaf fns are otherwise not exported)
pub fn n_mono_rhi_NativeRhi__alSourcePlay0(s: i32) -> Res<()> {
    al_ctx().lock().unwrap().play(s as u32);
    Ok(())
}
#[inline(never)] // exported symbol: the generated code links to it (small leaf fns are otherwise not exported)
pub fn n_mono_rhi_NativeRhi__alSourcePause0(s: i32) -> Res<()> {
    al_ctx().lock().unwrap().pause(s as u32);
    Ok(())
}
#[inline(never)] // exported symbol: the generated code links to it (small leaf fns are otherwise not exported)
pub fn n_mono_rhi_NativeRhi__alSourceStop0(s: i32) -> Res<()> {
    al_ctx().lock().unwrap().stop(s as u32);
    Ok(())
}
#[inline(never)] // exported symbol: the generated code links to it (small leaf fns are otherwise not exported)
pub fn n_mono_rhi_NativeRhi__alSourceQueueBuffer0(s: i32, b: i32) -> Res<()> {
    al_ctx().lock().unwrap().queue_buffers(s as u32, &[b as u32]);
    Ok(())
}
#[inline(never)] // exported symbol: the generated code links to it (small leaf fns are otherwise not exported)
pub fn n_mono_rhi_NativeRhi__alSourceUnqueueBuffer0(s: i32) -> Res<i32> {
    let ctx = al_ctx();
    let mut c = ctx.lock().unwrap();
    Ok(match c.unqueue_buffers(s as u32, 1) {
        Ok(v) => v.first().copied().unwrap_or(0) as i32,
        Err(_) => 0,
    })
}
#[inline(never)] // exported symbol: the generated code links to it (small leaf fns are otherwise not exported)
pub fn n_mono_rhi_NativeRhi__alGetSourcei0(s: i32, p: i32) -> Res<i32> {
    let ctx = al_ctx();
    let mut c = ctx.lock().unwrap();
    Ok(match c.get_source_i(s as u32, p) {
        Ok(v) => v,
        Err(code) => {
            c.fail(code);
            0
        }
    })
}
#[inline(never)] // exported symbol: the generated code links to it (small leaf fns are otherwise not exported)
pub fn n_mono_rhi_NativeRhi__alSourcei0(s: i32, p: i32, v: i32) -> Res<()> {
    al_ctx().lock().unwrap().set_source_i(s as u32, p, v);
    Ok(())
}
#[inline(never)] // exported symbol: the generated code links to it (small leaf fns are otherwise not exported)
pub fn n_mono_rhi_NativeRhi__alSourcef0(s: i32, p: i32, v: f32) -> Res<()> {
    al_ctx().lock().unwrap().set_source_f(s as u32, p, v);
    Ok(())
}
#[inline(never)] // exported symbol: the generated code links to it (small leaf fns are otherwise not exported)
pub fn n_mono_rhi_NativeRhi__alSource3f0(s: i32, p: i32, x: f32, y: f32, z: f32) -> Res<()> {
    al_ctx().lock().unwrap().set_source_3f(s as u32, p, [x, y, z]);
    Ok(())
}
#[inline(never)] // exported symbol: the generated code links to it (small leaf fns are otherwise not exported)
pub fn n_mono_rhi_NativeRhi__alListener3f0(p: i32, x: f32, y: f32, z: f32) -> Res<()> {
    al_ctx().lock().unwrap().set_listener_3f(p, [x, y, z]);
    Ok(())
}
#[inline(never)] // exported symbol: the generated code links to it (small leaf fns are otherwise not exported)
pub fn n_mono_rhi_NativeRhi__alGetError0() -> Res<i32> {
    Ok(al_ctx().lock().unwrap().error())
}
pub fn n_mono_rhi_NativeRhi__wrapDirectByteBuffer0(addr: i64, cap: i32) -> Res<Ref> {
    rt::new_direct_byte_buffer(addr, cap.max(0) as i64)
}

// ------------------------------------------------------------------ org.lwjgl.util.MonoShader (shaderc + SPIRV-Cross)
fn shader_libs() -> [*mut c_void; 3] {
    static H: OnceLock<[usize; 3]> = OnceLock::new();
    let a = *H.get_or_init(|| {
        [
            dl_open_any(&["libmonoshader.so"]) as usize,
            dl_open_any(&["libshaderc.so", "libshaderc_shared.so.1", "libshaderc.so.1"]) as usize,
            dl_open_any(&["libspirv-cross.so", "libspirv-cross-c-shared.so.0"]) as usize,
        ]
    });
    [a[0] as *mut c_void, a[1] as *mut c_void, a[2] as *mut c_void]
}

pub fn n_org_lwjgl_util_MonoShader__scCompilerInit() -> Res<i64> {
    let libs = shader_libs();
    // Gölgelendirici derleyicisi (shaderc) bulunamazsa sessizce 0 döndürmek, oyunda boş mesajlı bir
    // "Couldn't parse GLSL:" hatasına dönüşüyordu (Android'de APK'ya libmonoshader.so konmadığında görüldü).
    // Bunun yerine hangi kütüphanenin eksik olduğunu söyleyen açık bir hata fırlatılır.
    match sym_fn!(&libs, "shaderc_compiler_initialize", extern "C" fn() -> usize) {
        Some(f) => Ok(f() as i64),
        None => rt::throw("java/lang/UnsatisfiedLinkError",
            "shaderc not found: libmonoshader.so (Android: packaged in the APK via build_touchcore_apk.py --shader-lib), \
             or libshaderc.so / libshaderc_shared.so.1 on the library path".to_string()),
    }
}
pub fn n_org_lwjgl_util_MonoShader__scCompilerRelease(c: i64) -> Res<()> {
    let libs = shader_libs();
    if c != 0 { if let Some(f) = sym_fn!(&libs, "shaderc_compiler_release", extern "C" fn(usize)) { f(c as usize); } }
    Ok(())
}
pub fn n_org_lwjgl_util_MonoShader__scOptInit() -> Res<i64> {
    let libs = shader_libs();
    Ok(sym_fn!(&libs, "shaderc_compile_options_initialize", extern "C" fn() -> usize).map(|f| f() as i64).unwrap_or(0))
}
pub fn n_org_lwjgl_util_MonoShader__scOptRelease(o: i64) -> Res<()> {
    let libs = shader_libs();
    if o != 0 { if let Some(f) = sym_fn!(&libs, "shaderc_compile_options_release", extern "C" fn(usize)) { f(o as usize); } }
    Ok(())
}
pub fn n_org_lwjgl_util_MonoShader__scOptMacro(o: i64, name: i64, name_len: i64, val: i64, val_len: i64) -> Res<()> {
    let libs = shader_libs();
    if let Some(f) = sym_fn!(&libs, "shaderc_compile_options_add_macro_definition", extern "C" fn(usize, usize, usize, usize, usize)) {
        f(o as usize, name as usize, name_len as usize, val as usize, val_len as usize);
    }
    Ok(())
}
pub fn n_org_lwjgl_util_MonoShader__scOptAutoBind(o: i64, v: i32) -> Res<()> {
    let libs = shader_libs();
    if let Some(f) = sym_fn!(&libs, "shaderc_compile_options_set_auto_bind_uniforms", extern "C" fn(usize, bool)) {
        f(o as usize, v != 0);
    }
    Ok(())
}
pub fn n_org_lwjgl_util_MonoShader__scOptDebug(o: i64) -> Res<()> {
    let libs = shader_libs();
    if let Some(f) = sym_fn!(&libs, "shaderc_compile_options_set_generate_debug_info", extern "C" fn(usize)) { f(o as usize); }
    Ok(())
}
pub fn n_org_lwjgl_util_MonoShader__scOptLevel(o: i64, level: i32) -> Res<()> {
    let libs = shader_libs();
    if let Some(f) = sym_fn!(&libs, "shaderc_compile_options_set_optimization_level", extern "C" fn(usize, i32)) { f(o as usize, level); }
    Ok(())
}
pub fn n_org_lwjgl_util_MonoShader__scOptTarget(o: i64, target: i32, version: i32) -> Res<()> {
    let libs = shader_libs();
    if let Some(f) = sym_fn!(&libs, "shaderc_compile_options_set_target_env", extern "C" fn(usize, i32, u32)) {
        f(o as usize, target, version as u32);
    }
    Ok(())
}
pub fn n_org_lwjgl_util_MonoShader__scCompile(c: i64, src: i64, src_len: i64, kind: i32, file: i64, entry: i64, opt: i64) -> Res<i64> {
    let libs = shader_libs();
    Ok(sym_fn!(&libs, "shaderc_compile_into_spv", extern "C" fn(usize, usize, usize, i32, usize, usize, usize) -> usize)
        .map(|f| f(c as usize, src as usize, src_len as usize, kind, file as usize, entry as usize, opt as usize) as i64)
        .unwrap_or(0))
}
pub fn n_org_lwjgl_util_MonoShader__scResultRelease(r: i64) -> Res<()> {
    let libs = shader_libs();
    if r != 0 { if let Some(f) = sym_fn!(&libs, "shaderc_result_release", extern "C" fn(usize)) { f(r as usize); } }
    Ok(())
}
pub fn n_org_lwjgl_util_MonoShader__scResultStatus(r: i64) -> Res<i32> {
    let libs = shader_libs();
    Ok(sym_fn!(&libs, "shaderc_result_get_compilation_status", extern "C" fn(usize) -> i32).map(|f| f(r as usize)).unwrap_or(-1))
}
pub fn n_org_lwjgl_util_MonoShader__scResultMessage(r: i64) -> Res<i64> {
    let libs = shader_libs();
    Ok(sym_fn!(&libs, "shaderc_result_get_error_message", extern "C" fn(usize) -> usize).map(|f| f(r as usize) as i64).unwrap_or(0))
}
pub fn n_org_lwjgl_util_MonoShader__scResultBytes(r: i64) -> Res<i64> {
    let libs = shader_libs();
    Ok(sym_fn!(&libs, "shaderc_result_get_bytes", extern "C" fn(usize) -> usize).map(|f| f(r as usize) as i64).unwrap_or(0))
}
pub fn n_org_lwjgl_util_MonoShader__scResultLength(r: i64) -> Res<i64> {
    let libs = shader_libs();
    Ok(sym_fn!(&libs, "shaderc_result_get_length", extern "C" fn(usize) -> usize).map(|f| f(r as usize) as i64).unwrap_or(0))
}

pub fn n_org_lwjgl_util_MonoShader__spCtxCreate(out: i64) -> Res<i32> {
    let libs = shader_libs();
    Ok(sym_fn!(&libs, "spvc_context_create", extern "C" fn(usize) -> i32).map(|f| f(out as usize)).unwrap_or(-1))
}
pub fn n_org_lwjgl_util_MonoShader__spCtxDestroy(ctx: i64) -> Res<()> {
    let libs = shader_libs();
    if ctx != 0 { if let Some(f) = sym_fn!(&libs, "spvc_context_destroy", extern "C" fn(usize)) { f(ctx as usize); } }
    Ok(())
}
pub fn n_org_lwjgl_util_MonoShader__spLastError(ctx: i64) -> Res<i64> {
    let libs = shader_libs();
    Ok(sym_fn!(&libs, "spvc_context_get_last_error_string", extern "C" fn(usize) -> usize).map(|f| f(ctx as usize) as i64).unwrap_or(0))
}
pub fn n_org_lwjgl_util_MonoShader__spParse(ctx: i64, spirv: i64, words: i64, out_ir: i64) -> Res<i32> {
    let libs = shader_libs();
    Ok(sym_fn!(&libs, "spvc_context_parse_spirv", extern "C" fn(usize, usize, usize, usize) -> i32)
        .map(|f| f(ctx as usize, spirv as usize, words as usize, out_ir as usize))
        .unwrap_or(-1))
}
pub fn n_org_lwjgl_util_MonoShader__spCreateCompiler(ctx: i64, backend: i32, ir: i64, mode: i32, out: i64) -> Res<i32> {
    let libs = shader_libs();
    Ok(sym_fn!(&libs, "spvc_context_create_compiler", extern "C" fn(usize, i32, usize, i32, usize) -> i32)
        .map(|f| f(ctx as usize, backend, ir as usize, mode, out as usize))
        .unwrap_or(-1))
}
pub fn n_org_lwjgl_util_MonoShader__spCreateOptions(comp: i64, out: i64) -> Res<i32> {
    let libs = shader_libs();
    Ok(sym_fn!(&libs, "spvc_compiler_create_compiler_options", extern "C" fn(usize, usize) -> i32)
        .map(|f| f(comp as usize, out as usize))
        .unwrap_or(-1))
}
pub fn n_org_lwjgl_util_MonoShader__spInstallOptions(comp: i64, opt: i64) -> Res<i32> {
    let libs = shader_libs();
    Ok(sym_fn!(&libs, "spvc_compiler_install_compiler_options", extern "C" fn(usize, usize) -> i32)
        .map(|f| f(comp as usize, opt as usize))
        .unwrap_or(-1))
}
pub fn n_org_lwjgl_util_MonoShader__spOptBool(opt: i64, option: i32, v: i32) -> Res<i32> {
    let libs = shader_libs();
    Ok(sym_fn!(&libs, "spvc_compiler_options_set_bool", extern "C" fn(usize, i32, u8) -> i32)
        .map(|f| f(opt as usize, option, (v != 0) as u8))
        .unwrap_or(-1))
}
pub fn n_org_lwjgl_util_MonoShader__spOptUint(opt: i64, option: i32, v: i32) -> Res<i32> {
    let libs = shader_libs();
    Ok(sym_fn!(&libs, "spvc_compiler_options_set_uint", extern "C" fn(usize, i32, u32) -> i32)
        .map(|f| f(opt as usize, option, v as u32))
        .unwrap_or(-1))
}
pub fn n_org_lwjgl_util_MonoShader__spCompile(comp: i64, out: i64) -> Res<i32> {
    let libs = shader_libs();
    Ok(sym_fn!(&libs, "spvc_compiler_compile", extern "C" fn(usize, usize) -> i32)
        .map(|f| f(comp as usize, out as usize))
        .unwrap_or(-1))
}
pub fn n_org_lwjgl_util_MonoShader__spShaderResources(comp: i64, out: i64) -> Res<i32> {
    let libs = shader_libs();
    Ok(sym_fn!(&libs, "spvc_compiler_create_shader_resources", extern "C" fn(usize, usize) -> i32)
        .map(|f| f(comp as usize, out as usize))
        .unwrap_or(-1))
}
pub fn n_org_lwjgl_util_MonoShader__spBinaryOffset(comp: i64, id: i32, deco: i32, out: i64) -> Res<i32> {
    let libs = shader_libs();
    Ok(sym_fn!(&libs, "spvc_compiler_get_binary_offset_for_decoration", extern "C" fn(usize, u32, i32, usize) -> u8)
        .map(|f| (f(comp as usize, id as u32, deco, out as usize) != 0) as i32)
        .unwrap_or(0))
}
pub fn n_org_lwjgl_util_MonoShader__spStructSize(comp: i64, ty: i64, out: i64) -> Res<i32> {
    let libs = shader_libs();
    Ok(sym_fn!(&libs, "spvc_compiler_get_declared_struct_size", extern "C" fn(usize, usize, usize) -> i32)
        .map(|f| f(comp as usize, ty as usize, out as usize))
        .unwrap_or(-1))
}
pub fn n_org_lwjgl_util_MonoShader__spDecoration(comp: i64, id: i32, deco: i32) -> Res<i32> {
    let libs = shader_libs();
    Ok(sym_fn!(&libs, "spvc_compiler_get_decoration", extern "C" fn(usize, u32, i32) -> u32)
        .map(|f| f(comp as usize, id as u32, deco) as i32)
        .unwrap_or(0))
}
pub fn n_org_lwjgl_util_MonoShader__spName(comp: i64, id: i32) -> Res<i64> {
    let libs = shader_libs();
    Ok(sym_fn!(&libs, "spvc_compiler_get_name", extern "C" fn(usize, u32) -> usize)
        .map(|f| f(comp as usize, id as u32) as i64)
        .unwrap_or(0))
}
pub fn n_org_lwjgl_util_MonoShader__spTypeHandle(comp: i64, id: i32) -> Res<i64> {
    let libs = shader_libs();
    Ok(sym_fn!(&libs, "spvc_compiler_get_type_handle", extern "C" fn(usize, u32) -> usize)
        .map(|f| f(comp as usize, id as u32) as i64)
        .unwrap_or(0))
}
pub fn n_org_lwjgl_util_MonoShader__spSetEntry(comp: i64, name: i64, model: i32) -> Res<i32> {
    let libs = shader_libs();
    Ok(sym_fn!(&libs, "spvc_compiler_set_entry_point", extern "C" fn(usize, usize, i32) -> i32)
        .map(|f| f(comp as usize, name as usize, model))
        .unwrap_or(-1))
}
pub fn n_org_lwjgl_util_MonoShader__spSetName(comp: i64, id: i32, name: i64) -> Res<()> {
    let libs = shader_libs();
    if let Some(f) = sym_fn!(&libs, "spvc_compiler_set_name", extern "C" fn(usize, u32, usize)) {
        f(comp as usize, id as u32, name as usize);
    }
    Ok(())
}
pub fn n_org_lwjgl_util_MonoShader__spResourceList(res: i64, ty: i32, out_list: i64, out_count: i64) -> Res<i32> {
    let libs = shader_libs();
    Ok(sym_fn!(&libs, "spvc_resources_get_resource_list_for_type", extern "C" fn(usize, i32, usize, usize) -> i32)
        .map(|f| f(res as usize, ty, out_list as usize, out_count as usize))
        .unwrap_or(-1))
}
pub fn n_org_lwjgl_util_MonoShader__spTypeArrayDim(t: i64, d: i32) -> Res<i32> {
    let libs = shader_libs();
    Ok(sym_fn!(&libs, "spvc_type_get_array_dimension", extern "C" fn(usize, u32) -> u32)
        .map(|f| f(t as usize, d as u32) as i32)
        .unwrap_or(0))
}
pub fn n_org_lwjgl_util_MonoShader__spTypeBase(t: i64) -> Res<i32> {
    let libs = shader_libs();
    Ok(sym_fn!(&libs, "spvc_type_get_basetype", extern "C" fn(usize) -> i32).map(|f| f(t as usize)).unwrap_or(0))
}
pub fn n_org_lwjgl_util_MonoShader__spTypeImageDim(t: i64) -> Res<i32> {
    let libs = shader_libs();
    Ok(sym_fn!(&libs, "spvc_type_get_image_dimension", extern "C" fn(usize) -> i32).map(|f| f(t as usize)).unwrap_or(0))
}
pub fn n_org_lwjgl_util_MonoShader__spTypeNumArrayDims(t: i64) -> Res<i32> {
    let libs = shader_libs();
    Ok(sym_fn!(&libs, "spvc_type_get_num_array_dimensions", extern "C" fn(usize) -> u32).map(|f| f(t as usize) as i32).unwrap_or(0))
}
pub fn n_org_lwjgl_util_MonoShader__spTypeVec(t: i64) -> Res<i32> {
    let libs = shader_libs();
    Ok(sym_fn!(&libs, "spvc_type_get_vector_size", extern "C" fn(usize) -> u32).map(|f| f(t as usize) as i32).unwrap_or(0))
}
