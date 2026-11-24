//! Üç aşamalı bölüm elemesi: frustum, sonra Hi-Z örtmeli portal BFS, sonra ekran alanı LOD'u. `world_render.zig` içindeki
//! `cullAndScheduleWorld`'in taşınmış hâli. Zig sürümü ara belleğini her karede bir arenadan oyuyordu; burada
//! `CullScratch` aynı tamponlara sahiptir ve kapasitelerini korur, bu yüzden kararlı durumdaki bir kare
//! hiçbir şey ayırmaz.
use std::collections::HashMap;

use crate::world::{project_aabb_ndc, Face, Frustum6, HiZPyramid, Mat4, SectionPortalMask, Vec3, SECTION_DIM};

pub const ALL_FACES: u8 = 0b11_1111;

/// Bir 16^3 bölümünün, oyunu bloke etmeden seviyeden alınmış değişmez anlık görüntüsü.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SectionSnapshot {
    pub sx: i32,
    pub sy: i32,
    pub sz: i32,
    pub revision: u32,
    pub has_non_air: bool,
    pub fully_opaque_occluder: bool,
    pub portals: SectionPortalMask,
    pub index_count: u32,
    pub first_index: u32,
    pub vertex_offset: i32,
}

impl SectionSnapshot {
    pub fn at(sx: i32, sy: i32, sz: i32) -> Self {
        SectionSnapshot {
            sx,
            sy,
            sz,
            revision: 1,
            has_non_air: true,
            fully_opaque_occluder: false,
            portals: SectionPortalMask::ALL_OPEN,
            index_count: 36,
            first_index: 0,
            vertex_offset: 0,
        }
    }

    pub fn world_min(&self) -> Vec3 {
        let d = SECTION_DIM as i32;
        Vec3::new((self.sx * d) as f32, (self.sy * d) as f32, (self.sz * d) as f32)
    }

    pub fn world_max(&self) -> Vec3 {
        let d = SECTION_DIM as i32;
        Vec3::new((self.sx * d + d) as f32, (self.sy * d + d) as f32, (self.sz * d + d) as f32)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lod {
    /// yakın: tam geometri
    Full,
    /// orta: birleştirilmiş dörtgenler
    Merged,
    /// uzak: kaba siluet
    Coarse,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisibleSection {
    pub snapshot_idx: u32,
    pub dist_sq: f32,
    pub screen_area_px: f32,
    pub lod: Lod,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CullStats {
    pub total_sections: usize,
    pub frustum_culled: usize,
    pub portal_culled: usize,
    pub hiz_occluded: usize,
    pub subpixel_culled: usize,
    pub visible_sections: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Coord {
    x: i32,
    y: i32,
    z: i32,
}

#[derive(Clone, Copy)]
struct QueueItem {
    idx: u32,
    enter_face_bits: u8,
}

/// Kareler arasında yeniden kullanılan tamponlar. Kapasiteleri `cull_and_schedule_world` çağrıları boyunca korunur.
#[derive(Default)]
pub struct CullScratch {
    coord_map: HashMap<Coord, u32>,
    visited: Vec<bool>,
    entry_mask: Vec<u8>,
    emitted: Vec<bool>,
    bfs: Vec<QueueItem>,
    visible: Vec<VisibleSection>,
}

fn section_of(camera: f32) -> i32 {
    (camera / SECTION_DIM as f32).floor() as i32
}

/// Kameranın bu karede gördüğü.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub camera: Vec3,
    pub view_proj: Mat4,
    pub viewport: (u32, u32),
    /// bundan az piksel kaplayan bölümler atılır
    pub min_pixel_area: f32,
}

/// Tam boru hattını çalıştırır. Dönen dilim `scratch` içinde yaşar ve önden arkaya sıralıdır.
pub fn cull_and_schedule_world<'a>(
    scratch: &'a mut CullScratch,
    sections: &[SectionSnapshot],
    view: &View,
    hiz: &mut HiZPyramid,
    out: &mut CullStats,
) -> &'a [VisibleSection] {
    let (camera, view_proj, viewport, min_pixel_area) = (view.camera, &view.view_proj, view.viewport, view.min_pixel_area);
    *out = CullStats { total_sections: sections.len(), ..CullStats::default() };
    scratch.visible.clear();
    if sections.is_empty() {
        return &scratch.visible;
    }
    let n = sections.len();
    let frustum = Frustum6::from_view_proj(view_proj);

    scratch.coord_map.clear();
    scratch.coord_map.reserve(n);
    for (idx, s) in sections.iter().enumerate() {
        scratch.coord_map.insert(Coord { x: s.sx, y: s.sy, z: s.sz }, idx as u32);
    }
    scratch.visited.clear();
    scratch.visited.resize(n, false);
    scratch.entry_mask.clear();
    scratch.entry_mask.resize(n, 0);
    scratch.emitted.clear();
    scratch.emitted.resize(n, false);
    scratch.bfs.clear();
    scratch.bfs.reserve(n * 7);
    scratch.visible.reserve(n);

    // Adım 1: Hi-Z piramidini frustum içindeki opak örtücülerden tohumla.
    hiz.clear();
    for s in sections.iter().filter(|s| s.fully_opaque_occluder) {
        let (mn, mx) = (s.world_min(), s.world_max());
        if frustum.intersects_aabb(mn, mx) {
            hiz.rasterize_opaque_box(view_proj, mn, mx);
        }
    }
    hiz.build_mips();

    // Adım 2: portal gezintisi. Bir bölüm her giriş yüzü için bir kez kuyruğa alınır: açtığı çıkışlar o yüze bağlıdır.
    let (cam_sx, cam_sy, cam_sz) = (section_of(camera.x), section_of(camera.y), section_of(camera.z));
    match scratch.coord_map.get(&Coord { x: cam_sx, y: cam_sy, z: cam_sz }).copied() {
        Some(start) => {
            scratch.visited[start as usize] = true;
            scratch.entry_mask[start as usize] = ALL_FACES;
            scratch.bfs.push(QueueItem { idx: start, enter_face_bits: ALL_FACES });
        }
        None => {
            // kamera anlık görüntünün dışında (yüksek yörünge): görüştaki her bölümü tohumla
            for (idx, s) in sections.iter().enumerate() {
                if frustum.intersects_aabb(s.world_min(), s.world_max()) {
                    scratch.visited[idx] = true;
                    scratch.entry_mask[idx] = ALL_FACES;
                    scratch.bfs.push(QueueItem { idx: idx as u32, enter_face_bits: ALL_FACES });
                }
            }
        }
    }

    let (vw, vh) = (viewport.0 as f32, viewport.1 as f32);
    let mut head = 0usize;
    while head < scratch.bfs.len() {
        let item = scratch.bfs[head];
        head += 1;
        let i = item.idx as usize;
        let s = sections[i];
        let (mn, mx) = (s.world_min(), s.world_max());

        // Aşama A: frustum.
        if !frustum.intersects_aabb(mn, mx) {
            if !scratch.emitted[i] {
                scratch.emitted[i] = true;
                out.frustum_culled += 1;
            }
            continue;
        }

        // Giriş yüzlerinden erişilebilen her yüz boyunca yay.
        for out_face in Face::ALL {
            let can_reach = item.enter_face_bits == ALL_FACES
                || Face::ALL.iter().any(|&in_face| {
                    item.enter_face_bits & (1 << in_face as u8) != 0 && s.portals.can_pass(in_face, out_face)
                });
            if !can_reach {
                continue;
            }
            let nkey = Coord {
                x: s.sx + out_dx(out_face),
                y: s.sy + out_dy(out_face),
                z: s.sz + out_dz(out_face),
            };
            // Yalnızca kamera bölümüne göre dışa doğru hareketler: BFS'in ileri geri sekmesini engeller.
            let dir_dot = (nkey.x - s.sx) * (s.sx - cam_sx)
                + (nkey.y - s.sy) * (s.sy - cam_sy)
                + (nkey.z - s.sz) * (s.sz - cam_sz);
            if item.enter_face_bits != ALL_FACES && dir_dot < 0 {
                continue;
            }
            if let Some(&nb) = scratch.coord_map.get(&nkey) {
                let enter_bit = 1u8 << (out_face.opposite() as u8);
                let nbi = nb as usize;
                if scratch.entry_mask[nbi] & enter_bit == 0 {
                    scratch.entry_mask[nbi] |= enter_bit;
                    scratch.visited[nbi] = true;
                    scratch.bfs.push(QueueItem { idx: nb, enter_face_bits: enter_bit });
                }
            }
        }

        // Görünürlük testleri giriş yüzüne bağlı değildir: bölüm başına bir kez çalıştır.
        if scratch.emitted[i] {
            continue;
        }
        scratch.emitted[i] = true;
        if !s.has_non_air {
            continue;
        }

        // Aşama B ve C: Hi-Z örtme, sonra ekran kapsamı ve LOD.
        let c = [mn.x + 8.0 - camera.x, mn.y + 8.0 - camera.y, mn.z + 8.0 - camera.z];
        let dist_sq = c[0] * c[0] + c[1] * c[1] + c[2] * c[2];
        let mut area_px = vw * vh;
        match project_aabb_ndc(view_proj, mn, mx) {
            None => {
                out.frustum_culled += 1;
                continue;
            }
            Some(proj) => {
                if !s.fully_opaque_occluder && hiz.is_occluded(&proj) {
                    out.hiz_occluded += 1;
                    continue;
                }
                if !proj.near_clip_overlap {
                    let sx0 = (proj.min_x * 0.5 + 0.5).clamp(0.0, 1.0);
                    let sx1 = (proj.max_x * 0.5 + 0.5).clamp(0.0, 1.0);
                    let sy0 = (proj.min_y * 0.5 + 0.5).clamp(0.0, 1.0);
                    let sy1 = (proj.max_y * 0.5 + 0.5).clamp(0.0, 1.0);
                    area_px = (sx1 - sx0) * vw * (sy1 - sy0) * vh;
                    if area_px < min_pixel_area {
                        out.subpixel_culled += 1;
                        continue;
                    }
                }
            }
        }

        let lod = if area_px >= 4096.0 {
            Lod::Full
        } else if area_px >= 512.0 {
            Lod::Merged
        } else {
            Lod::Coarse
        };
        scratch.visible.push(VisibleSection { snapshot_idx: item.idx, dist_sq, screen_area_px: area_px, lod });
    }

    // Gezintinin hiç ulaşmadığı bölümler: duvarların arkasında ve yeraltında ya da frustumun dışında.
    let mut unvisited_in_frustum = 0usize;
    for (i, s) in sections.iter().enumerate() {
        if !scratch.visited[i] {
            if frustum.intersects_aabb(s.world_min(), s.world_max()) {
                unvisited_in_frustum += 1;
            } else {
                out.frustum_culled += 1;
            }
        }
    }
    out.portal_culled = unvisited_in_frustum;
    out.visible_sections = scratch.visible.len();

    // Önden arkaya: GPU'da erken-Z reddi.
    scratch
        .visible
        .sort_unstable_by(|a, b| a.dist_sq.partial_cmp(&b.dist_sq).unwrap_or(std::cmp::Ordering::Equal));
    &scratch.visible
}

fn out_dx(f: Face) -> i32 {
    match f {
        Face::NegX => -1,
        Face::PosX => 1,
        _ => 0,
    }
}

fn out_dy(f: Face) -> i32 {
    match f {
        Face::NegY => -1,
        Face::PosY => 1,
        _ => 0,
    }
}

fn out_dz(f: Face) -> i32 {
    match f {
        Face::NegZ => -1,
        Face::PosZ => 1,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view_proj(cam: Vec3, target: Vec3, aspect: f32) -> Mat4 {
        let view = Mat4::look_at(cam, target, Vec3::new(0.0, 1.0, 0.0));
        let proj = Mat4::perspective(70f32.to_radians(), aspect, 0.5, 512.0);
        proj.mul(&view)
    }

    #[test]
    fn culling_drops_sections_behind_a_wall_and_behind_the_camera() {
        let sections = [
            SectionSnapshot { has_non_air: false, ..SectionSnapshot::at(0, 0, 0) },
            SectionSnapshot { fully_opaque_occluder: true, portals: SectionPortalMask { bits: 0 }, ..SectionSnapshot::at(0, 0, -2) },
            SectionSnapshot::at(0, 0, -1),
            SectionSnapshot::at(0, 0, -3),
            SectionSnapshot::at(0, 0, 2),
        ];
        let cam = Vec3::new(8.0, 8.0, 8.0);
        let vp = view_proj(cam, Vec3::new(8.0, 8.0, -8.0), 16.0 / 9.0);
        let mut scratch = CullScratch::default();
        let mut hiz = HiZPyramid::new();
        let mut stats = CullStats::default();
        let view = View { camera: cam, view_proj: vp, viewport: (1920, 1080), min_pixel_area: 4.0 };
        let visible = cull_and_schedule_world(&mut scratch, &sections, &view, &mut hiz, &mut stats);
        let mut ids: Vec<u32> = visible.iter().map(|v| v.snapshot_idx).collect();
        ids.sort_unstable();
        assert_eq!(ids, vec![1, 2]);
        assert!(stats.frustum_culled >= 1);
    }

    #[test]
    fn portal_traversal_keeps_every_entry_face() {
        // T'ye önce güneyden, sonra batıdan ulaşılır; yalnızca batı-doğu geçer. U yalnızca batı üzerinden görülür.
        let only_we = {
            let mut m = SectionPortalMask { bits: 0 };
            m.set_bidirectional(Face::NegX, Face::PosX);
            m
        };
        let sections = [
            SectionSnapshot { has_non_air: false, ..SectionSnapshot::at(0, 0, 0) },
            SectionSnapshot::at(1, 0, 0),
            SectionSnapshot::at(0, 0, -1),
            SectionSnapshot { portals: only_we, ..SectionSnapshot::at(1, 0, -1) },
            SectionSnapshot::at(2, 0, -1),
        ];
        let cam = Vec3::new(8.0, 8.0, 8.0);
        let vp = view_proj(cam, Vec3::new(40.0, 8.0, -24.0), 16.0 / 9.0);
        let mut scratch = CullScratch::default();
        let mut hiz = HiZPyramid::new();
        let mut stats = CullStats::default();
        let view = View { camera: cam, view_proj: vp, viewport: (1920, 1080), min_pixel_area: 1.0 };
        let visible = cull_and_schedule_world(&mut scratch, &sections, &view, &mut hiz, &mut stats);
        assert!(visible.iter().any(|v| v.snapshot_idx == 4));
    }

    #[test]
    fn empty_input_yields_nothing() {
        let mut scratch = CullScratch::default();
        let mut hiz = HiZPyramid::new();
        let mut stats = CullStats::default();
        let vp = Mat4::IDENTITY;
        let view = View { camera: Vec3::default(), view_proj: vp, viewport: (1, 1), min_pixel_area: 0.0 };
        let visible = cull_and_schedule_world(&mut scratch, &[], &view, &mut hiz, &mut stats);
        assert!(visible.is_empty());
        assert_eq!(stats.total_sections, 0);
    }
}
