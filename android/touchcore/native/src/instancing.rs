//! GPU örnekleme toplayıcısı: blok dışı varlıkları (ekran varlıkları, yaratıklar, oyuncular, düşen eşyalar, blok
//! varlıkları, parçacıklar) toplar, eler, sağ kalanları çizim dizilerine sıralar ve dizi başına bir örnek tamponu artı bir
//! dolaylı komut üretir. `gpu_instancing.zig`'in taşınmış hâli, yalnızca CPU tarafı.
//!
//! Zig sürümü sonucu doğrudan bir RHI aygıtına itiyordu. Burada sonuç bir `FrameDraw` değeridir; GPU
//! arka ucu (Vulkan ya da GL 3.3) onu tüketir. Toplu iş kurulumunu GPU çağrılarından uzak tutmak, onun herhangi bir ana
//! makinede test edilebilmesini sağlar.
use std::collections::HashMap;

use crate::cull::View;
use crate::world::{project_aabb_ndc, Frustum6, HiZPyramid, Vec3};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum EntityKind {
    BlockDisplay = 0,
    ItemDisplay = 1,
    TextDisplay = 2,
    Mob = 3,
    Player = 4,
    DroppedItem = 5,
    Chest = 6,
    Sign = 7,
    Particle = 8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Billboard {
    Fixed = 0,
    Vertical = 1,
    Horizontal = 2,
    Center = 3,
}

/// 64 baytlık örnek tanımlayıcısı; bir Vulkan SSBO'su ve bir GL TBO'nun dört RGBA32F texel'i için aynı yerleşim.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct GpuInstance {
    pub row0: [f32; 4],
    pub row1: [f32; 4],
    pub row2: [f32; 4],
    /// paketlenmiş normalleştirilmiş atlas hücresi (u0, v0, u1, v1), her biri 8 bit
    pub uv_rect: u32,
    /// RGB rengi (24 bit) ve blok/gökyüzü ışığı (8 bit)
    pub color_light: u32,
    /// tür (4 bit) | billboard (2 bit, 4. bitten başlar) | hasar parıltısı (bit 6) | özel
    pub flags: u32,
    /// alt 16 bit: ağ kimliği; üst 16 bit: sınırlayıcı yarıçap * 256
    pub mesh_and_radius: u32,
}

const _: () = assert!(std::mem::size_of::<GpuInstance>() == 64);

impl GpuInstance {
    pub fn make_flags(kind: EntityKind, billboard: Billboard, hurt_flash: bool) -> u32 {
        kind as u32 | ((billboard as u32) << 4) | ((hurt_flash as u32) << 6)
    }

    pub fn mesh_id(&self) -> u16 {
        self.mesh_and_radius as u16
    }

    pub fn radius(&self) -> f32 {
        (self.mesh_and_radius >> 16) as f32 / 256.0
    }

    pub fn pack_mesh_radius(mesh_id: u16, r: f32) -> u32 {
        let clamped = (r * 256.0).clamp(1.0, 65535.0);
        mesh_id as u32 | ((clamped as u32) << 16)
    }

    pub fn world_center(&self) -> Vec3 {
        Vec3::new(self.row0[3], self.row1[3], self.row2[3])
    }

    fn kind_bits(&self) -> u32 {
        self.flags & 0xF
    }

    /// Yarı saydam türler her opak türden sonra çizilir.
    pub fn is_translucent(&self) -> bool {
        let k = self.kind_bits();
        k == EntityKind::Particle as u32 || k == EntityKind::TextDisplay as u32
    }

    /// Sıralama anahtarı: opak yarı saydamdan önce, sonra ağ kimliğine göre.
    pub fn run_key(&self) -> u32 {
        ((self.is_translucent() as u32) << 16) | self.mesh_id() as u32
    }
}

/// 32 baytlık parçacık durumu.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct GpuParticle {
    pub pos: [f32; 3],
    pub size_rot_packed: u32,
    pub vel_packed: u32,
    pub rgba8: u32,
    pub uv_rect: u32,
    pub light_age: u32,
}

const _: () = assert!(std::mem::size_of::<GpuParticle>() == 32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeshProto {
    pub mesh_id: u16,
    pub index_count: u32,
    pub first_index: u32,
    pub vertex_offset: i32,
}

/// `VkDrawIndexedIndirectCommand` ve GL `DrawElementsIndirectCommand` yerleşimiyle eşleşir.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct DrawIndexedIndirect {
    pub index_count: u32,
    pub instance_count: u32,
    pub first_index: u32,
    pub vertex_offset: i32,
    pub first_instance: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InstancingStats {
    pub submitted_total: usize,
    pub frustum_culled: usize,
    pub hiz_culled: usize,
    pub subpixel_culled: usize,
    /// bir çizim dizisine yerleştirilen örnekler
    pub surviving_instances: usize,
    /// ağı hiç kaydedilmemiş örnekler (hiç çizilmez, çizilmiş sayılmaz)
    pub unregistered_mesh_instances: usize,
    pub draw_batches_emitted: usize,
}

/// GPU arka ucunun bu karede yüklemesi ve çizmesi gerekenler.
#[derive(Default, Debug, PartialEq)]
pub struct FrameDraw {
    pub instances: Vec<GpuInstance>,
    pub commands: Vec<DrawIndexedIndirect>,
}

#[derive(Default)]
pub struct InstanceBatcher {
    raw: Vec<GpuInstance>,
    meshes: HashMap<u16, MeshProto>,
    frame: FrameDraw,
    stats: InstancingStats,
}

impl InstanceBatcher {
    /// Karenin varlıklarını temizler ve depolamalarını korur.
    pub fn begin_frame(&mut self, reserve_instances: usize) {
        self.raw.clear();
        self.raw.reserve(reserve_instances);
        self.meshes.clear();
        self.stats = InstancingStats::default();
    }

    pub fn register_mesh(&mut self, proto: MeshProto) {
        self.meshes.insert(proto.mesh_id, proto);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn push_entity(
        &mut self,
        kind: EntityKind,
        mesh_id: u16,
        pos: Vec3,
        scale: f32,
        radius: f32,
        billboard: Billboard,
        uv_rect: u32,
        color_light: u32,
    ) {
        self.raw.push(GpuInstance {
            row0: [scale, 0.0, 0.0, pos.x],
            row1: [0.0, scale, 0.0, pos.y],
            row2: [0.0, 0.0, scale, pos.z],
            uv_rect,
            color_light,
            flags: GpuInstance::make_flags(kind, billboard, false),
            mesh_and_radius: GpuInstance::pack_mesh_radius(mesh_id, radius * scale),
        });
    }

    pub fn push_particle(&mut self, particle_mesh_id: u16, p: &GpuParticle, size: f32) {
        self.raw.push(GpuInstance {
            row0: [size, 0.0, 0.0, p.pos[0]],
            row1: [0.0, size, 0.0, p.pos[1]],
            row2: [0.0, 0.0, size, p.pos[2]],
            uv_rect: p.uv_rect,
            color_light: p.rgba8,
            flags: GpuInstance::make_flags(EntityKind::Particle, Billboard::Center, false),
            mesh_and_radius: GpuInstance::pack_mesh_radius(particle_mesh_id, size * 0.866),
        });
    }

    pub fn stats(&self) -> &InstancingStats {
        &self.stats
    }

    /// Son boşaltılan kare: çizim sırasında örnekler ve dizi başına bir dolaylı komut.
    pub fn frame(&self) -> &FrameDraw {
        &self.frame
    }

    /// Eler (frustum, Hi-Z, ekran alanı), sağ kalanları dizilere sıralar ve karenin çizim verisini kurar.
    pub fn flush(&mut self, view: &View, hiz: &HiZPyramid) {
        self.frame.instances.clear();
        self.frame.commands.clear();
        self.stats = InstancingStats { submitted_total: self.raw.len(), ..InstancingStats::default() };
        if self.raw.is_empty() {
            return;
        }
        let frustum = Frustum6::from_view_proj(&view.view_proj);
        let (vw, vh) = (view.viewport.0 as f32, view.viewport.1 as f32);

        for inst in &self.raw {
            let c = inst.world_center();
            let r = inst.radius();
            let (mn, mx) = (Vec3::new(c.x - r, c.y - r, c.z - r), Vec3::new(c.x + r, c.y + r, c.z + r));
            if !frustum.intersects_aabb(mn, mx) {
                self.stats.frustum_culled += 1;
                continue;
            }
            let Some(proj) = project_aabb_ndc(&view.view_proj, mn, mx) else {
                self.stats.frustum_culled += 1;
                continue;
            };
            if hiz.is_occluded(&proj) {
                self.stats.hiz_culled += 1;
                continue;
            }
            if !proj.near_clip_overlap {
                let cx = |v: f32| (v * 0.5 + 0.5).clamp(0.0, 1.0);
                let area_px = (cx(proj.max_x) - cx(proj.min_x)) * vw * (cx(proj.max_y) - cx(proj.min_y)) * vh;
                if area_px < view.min_pixel_area {
                    self.stats.subpixel_culled += 1;
                    continue;
                }
            }
            if !self.meshes.contains_key(&inst.mesh_id()) {
                self.stats.unregistered_mesh_instances += 1;
                continue;
            }
            self.frame.instances.push(*inst);
        }
        if self.frame.instances.is_empty() {
            return;
        }

        // Önce opak diziler (ağa göre gruplanmış), sonra yarı saydamlar; böylece karışım tamamlanmış opak geçişin üzerine biner.
        self.frame.instances.sort_unstable_by_key(GpuInstance::run_key);

        let instances = &self.frame.instances;
        let mut run_start = 0usize;
        while run_start < instances.len() {
            let key = instances[run_start].run_key();
            let mid = instances[run_start].mesh_id();
            let mut run_end = run_start + 1;
            while run_end < instances.len() && instances[run_end].run_key() == key {
                run_end += 1;
            }
            // her sağ kalanın kayıtlı bir ağı var, bu yüzden her dizi bir komut olur
            let proto = self.meshes[&mid];
            self.frame.commands.push(DrawIndexedIndirect {
                index_count: proto.index_count,
                instance_count: (run_end - run_start) as u32,
                first_index: proto.first_index,
                vertex_offset: proto.vertex_offset,
                first_instance: run_start as u32,
            });
            run_start = run_end;
        }
        self.stats.draw_batches_emitted = self.frame.commands.len();
        self.stats.surviving_instances = self.frame.instances.len();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Mat4;

    fn vp_from(cam: Vec3, target: Vec3) -> Mat4 {
        let view = Mat4::look_at(cam, target, Vec3::new(0.0, 1.0, 0.0));
        Mat4::perspective(70f32.to_radians(), 16.0 / 9.0, 0.5, 512.0).mul(&view)
    }

    fn proto(id: u16, index_count: u32, first_index: u32, vertex_offset: i32) -> MeshProto {
        MeshProto { mesh_id: id, index_count, first_index, vertex_offset }
    }

    #[test]
    fn batches_many_kinds_into_few_runs_and_culls_occluded_and_behind() {
        let cam = Vec3::new(0.0, 10.0, 20.0);
        let vp = vp_from(cam, Vec3::new(0.0, 10.0, 0.0));
        let mut hiz = HiZPyramid::new();
        hiz.clear();
        // kameranın hemen önünde opak bir duvar
        hiz.rasterize_opaque_box(&vp, Vec3::new(-16.0, 0.0, 5.0), Vec3::new(16.0, 20.0, 10.0));
        hiz.build_mips();

        let mut b = InstanceBatcher::default();
        b.begin_frame(256);
        b.register_mesh(proto(1, 36, 0, 0));
        b.register_mesh(proto(2, 72, 36, 24));
        b.register_mesh(proto(3, 48, 108, 60));
        b.register_mesh(proto(4, 6, 156, 90));

        b.push_entity(EntityKind::BlockDisplay, 1, Vec3::new(-1.0, 10.0, 13.0), 1.0, 1.0, Billboard::Fixed, 0x0101, 0xFFFF_FFFF);
        b.push_entity(EntityKind::ItemDisplay, 1, Vec3::new(0.0, 10.0, 13.0), 0.8, 0.8, Billboard::Center, 0x0202, 0xFF00_FFFF);
        b.push_entity(EntityKind::TextDisplay, 4, Vec3::new(1.0, 11.0, 13.0), 1.2, 1.0, Billboard::Vertical, 0x0303, 0xFFFF_FFFF);
        b.push_entity(EntityKind::Mob, 2, Vec3::new(2.0, 10.0, 13.0), 1.0, 1.2, Billboard::Fixed, 0x0404, 0xFFFF_FFFF);
        b.push_entity(EntityKind::Player, 2, Vec3::new(-2.0, 10.0, 13.0), 1.0, 1.2, Billboard::Fixed, 0x0505, 0xFFFF_FFFF);
        b.push_entity(EntityKind::Chest, 3, Vec3::new(0.0, 9.0, 13.0), 1.0, 0.9, Billboard::Fixed, 0x0606, 0xFFFF_FFFF);
        b.push_entity(EntityKind::Sign, 3, Vec3::new(3.0, 10.0, 13.0), 1.0, 0.8, Billboard::Fixed, 0x0707, 0xFFFF_FFFF);
        let p = GpuParticle { pos: [0.0, 10.5, 14.0], size_rot_packed: 0, vel_packed: 0, rgba8: 0xFF88_00FF, uv_rect: 0x1010, light_age: 0 };
        b.push_particle(4, &p, 0.5);
        b.push_entity(EntityKind::Mob, 2, Vec3::new(0.0, 10.0, -10.0), 1.0, 1.0, Billboard::Fixed, 0, 0xFFFF_FFFF);
        b.push_entity(EntityKind::DroppedItem, 1, Vec3::new(0.0, 10.0, 30.0), 1.0, 0.5, Billboard::Center, 0, 0xFFFF_FFFF);

        let view = View { camera: cam, view_proj: vp, viewport: (1920, 1080), min_pixel_area: 2.0 };
        b.flush(&view, &hiz);
        let frame = b.frame();
        assert_eq!(frame.instances.len(), 8);
        assert_eq!(frame.commands.len(), 4);
        let s = *b.stats();
        assert_eq!(s.submitted_total, 10);
        assert_eq!(s.surviving_instances, 8);
        assert_eq!(s.hiz_culled, 1);
        assert_eq!(s.frustum_culled, 1);
        assert_eq!(s.draw_batches_emitted, 4);
    }

    #[test]
    fn translucent_runs_come_last_and_unregistered_meshes_are_not_drawn() {
        let cam = Vec3::new(0.0, 10.0, 20.0);
        let vp = vp_from(cam, Vec3::new(0.0, 10.0, 0.0));
        let hiz = HiZPyramid::new();
        let mut b = InstanceBatcher::default();
        b.begin_frame(16);
        b.register_mesh(proto(1, 36, 0, 0));
        b.register_mesh(proto(2, 6, 36, 8));
        let p = GpuParticle { pos: [0.0, 10.0, 5.0], size_rot_packed: 0, vel_packed: 0, rgba8: 0xFFFF_FFFF, uv_rect: 0, light_age: 0 };
        b.push_particle(1, &p, 0.5);
        b.push_entity(EntityKind::BlockDisplay, 1, Vec3::new(0.0, 10.0, 5.0), 1.0, 1.0, Billboard::Fixed, 0, 0xFFFF_FFFF);
        b.push_entity(EntityKind::Mob, 2, Vec3::new(1.0, 10.0, 5.0), 1.0, 1.0, Billboard::Fixed, 0, 0xFFFF_FFFF);
        b.push_entity(EntityKind::Mob, 9, Vec3::new(-1.0, 10.0, 5.0), 1.0, 1.0, Billboard::Fixed, 0, 0xFFFF_FFFF);
        let view = View { camera: cam, view_proj: vp, viewport: (1920, 1080), min_pixel_area: 1.0 };
        b.flush(&view, &hiz);
        let frame = b.frame();
        let s = *b.stats();
        assert_eq!(s.draw_batches_emitted, 3);
        assert_eq!(s.surviving_instances, 3);
        assert_eq!(s.unregistered_mesh_instances, 1);
        let last = frame.commands[2];
        assert_eq!(last.index_count, 36);
        assert_eq!(last.instance_count, 1);
        assert_eq!(last.first_instance, 2);
    }

    #[test]
    fn radius_round_trips_through_the_packed_field() {
        let packed = GpuInstance::pack_mesh_radius(7, 1.5);
        let inst = GpuInstance {
            row0: [0.0; 4], row1: [0.0; 4], row2: [0.0; 4], uv_rect: 0, color_light: 0, flags: 0, mesh_and_radius: packed,
        };
        assert_eq!(inst.mesh_id(), 7);
        assert!((inst.radius() - 1.5).abs() < 1.0 / 256.0);
    }
}
