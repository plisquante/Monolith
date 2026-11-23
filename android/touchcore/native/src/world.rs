//! Visibility core of the world renderer. Port of the geometry parts of `world_render.zig`: column-major matrices,
//! Gribb-Hartmann frustum, 6-face portal reachability per 16^3 section, and the conservative Hi-Z pyramid.
//! Everything works on immutable section snapshots; nothing here touches game state.
//!
//! Not in this module yet: the BFS section traversal, LOD selection and the sorted draw schedule of the Zig original.

pub const SECTION_DIM: usize = 16;
pub const SECTION_VOXELS: usize = SECTION_DIM * SECTION_DIM * SECTION_DIM;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Face {
    NegX = 0,
    PosX = 1,
    NegY = 2,
    PosY = 3,
    NegZ = 4,
    PosZ = 5,
}

impl Face {
    pub const ALL: [Face; 6] = [Face::NegX, Face::PosX, Face::NegY, Face::PosY, Face::NegZ, Face::PosZ];

    pub fn opposite(self) -> Face {
        Self::ALL[(self as usize) ^ 1]
    }
}

impl std::ops::Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, b: Vec3) -> Vec3 {
        Vec3::new(self.x - b.x, self.y - b.y, self.z - b.z)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Vec3 { x, y, z }
    }
    pub fn dot(self, b: Vec3) -> f32 {
        self.x * b.x + self.y * b.y + self.z * b.z
    }
    pub fn cross(self, b: Vec3) -> Vec3 {
        Vec3::new(self.y * b.z - self.z * b.y, self.z * b.x - self.x * b.z, self.x * b.y - self.y * b.x)
    }
    /// Unit vector; degenerate input yields +Z, as in the Zig original.
    pub fn normalize(self) -> Vec3 {
        let l = (self.x * self.x + self.y * self.y + self.z * self.z).sqrt();
        if l <= 1e-8 {
            return Vec3::new(0.0, 0.0, 1.0);
        }
        let inv = 1.0 / l;
        Vec3::new(self.x * inv, self.y * inv, self.z * inv)
    }
}

/// Column-major 4x4 matrix: element (row r, column c) is `m[c * 4 + r]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat4 {
    pub m: [f32; 16],
}

impl Mat4 {
    pub const IDENTITY: Mat4 = Mat4 {
        m: [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0],
    };

    pub fn perspective(fov_y_rad: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
        let f = 1.0 / (fov_y_rad * 0.5).tan();
        let nf = 1.0 / (near - far);
        Mat4 {
            m: [
                f / aspect, 0.0, 0.0, 0.0,
                0.0, f, 0.0, 0.0,
                0.0, 0.0, (far + near) * nf, -1.0,
                0.0, 0.0, 2.0 * far * near * nf, 0.0,
            ],
        }
    }

    pub fn look_at(eye: Vec3, target: Vec3, up_in: Vec3) -> Mat4 {
        let fwd = (target - eye).normalize();
        let right = fwd.cross(up_in).normalize();
        let up = right.cross(fwd);
        Mat4 {
            m: [
                right.x, up.x, -fwd.x, 0.0,
                right.y, up.y, -fwd.y, 0.0,
                right.z, up.z, -fwd.z, 0.0,
                -right.dot(eye), -up.dot(eye), fwd.dot(eye), 1.0,
            ],
        }
    }

    pub fn mul(&self, b: &Mat4) -> Mat4 {
        let mut r = [0.0f32; 16];
        for col in 0..4 {
            for row in 0..4 {
                let mut sum = 0.0f32;
                for k in 0..4 {
                    sum += self.m[k * 4 + row] * b.m[col * 4 + k];
                }
                r[col * 4 + row] = sum;
            }
        }
        Mat4 { m: r }
    }

    /// Homogeneous transform of a point: (x, y, z, w).
    pub fn transform_point(&self, p: Vec3) -> [f32; 4] {
        let m = &self.m;
        [
            m[0] * p.x + m[4] * p.y + m[8] * p.z + m[12],
            m[1] * p.x + m[5] * p.y + m[9] * p.z + m[13],
            m[2] * p.x + m[6] * p.y + m[10] * p.z + m[14],
            m[3] * p.x + m[7] * p.y + m[11] * p.z + m[15],
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    pub nx: f32,
    pub ny: f32,
    pub nz: f32,
    pub d: f32,
}

impl Plane {
    pub fn normalized(self) -> Plane {
        let len = (self.nx * self.nx + self.ny * self.ny + self.nz * self.nz).sqrt();
        if len <= 1e-8 {
            return self;
        }
        let inv = 1.0 / len;
        Plane { nx: self.nx * inv, ny: self.ny * inv, nz: self.nz * inv, d: self.d * inv }
    }
}

/// Gribb-Hartmann six-plane frustum taken from a combined view-projection matrix.
#[derive(Clone, Copy, Debug)]
pub struct Frustum6 {
    pub planes: [Plane; 6],
}

impl Frustum6 {
    pub fn from_view_proj(vp: &Mat4) -> Frustum6 {
        let m = &vp.m;
        let r0 = [m[0], m[4], m[8], m[12]];
        let r1 = [m[1], m[5], m[9], m[13]];
        let r2 = [m[2], m[6], m[10], m[14]];
        let r3 = [m[3], m[7], m[11], m[15]];
        let plane = |r: &[f32; 4], sign: f32| {
            Plane {
                nx: r3[0] + sign * r[0],
                ny: r3[1] + sign * r[1],
                nz: r3[2] + sign * r[2],
                d: r3[3] + sign * r[3],
            }
            .normalized()
        };
        Frustum6 {
            planes: [
                plane(&r0, 1.0),  // left
                plane(&r0, -1.0), // right
                plane(&r1, 1.0),  // bottom
                plane(&r1, -1.0), // top
                plane(&r2, 1.0),  // near
                plane(&r2, -1.0), // far
            ],
        }
    }

    /// Positive-vertex test: rejects a box only when it lies fully behind one plane.
    pub fn intersects_aabb(&self, min_p: Vec3, max_p: Vec3) -> bool {
        self.planes.iter().all(|p| {
            let px = if p.nx >= 0.0 { max_p.x } else { min_p.x };
            let py = if p.ny >= 0.0 { max_p.y } else { min_p.y };
            let pz = if p.nz >= 0.0 { max_p.z } else { min_p.z };
            // same as the Zig test `dist < 0 -> outside`: a NaN distance counts as inside
            let dist = p.nx * px + p.ny * py + p.nz * pz + p.d;
            !matches!(dist.partial_cmp(&0.0), Some(std::cmp::Ordering::Less))
        })
    }
}

/// Reachability between the six faces of one section: bit `from * 6 + to` is set when a path of non-opaque
/// voxels joins them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SectionPortalMask {
    pub bits: u64,
}

impl SectionPortalMask {
    pub const ALL_OPEN: SectionPortalMask = SectionPortalMask { bits: (1u64 << 36) - 1 };

    pub fn can_pass(&self, from: Face, to: Face) -> bool {
        (self.bits >> (from as u32 * 6 + to as u32)) & 1 != 0
    }

    pub fn set_bidirectional(&mut self, a: Face, b: Face) {
        self.bits |= (1u64 << (a as u32 * 6 + b as u32)) | (1u64 << (b as u32 * 6 + a as u32));
    }
}

/// Flood fill over the non-opaque voxels of a 16^3 section. `opaque` holds one bit per voxel, index `y*256 + z*16 + x`.
pub fn compute_section_portals(opaque: &[u64; 64]) -> SectionPortalMask {
    let mut visited = [0u64; 64];
    let mut queue = [0u16; SECTION_VOXELS];
    let mut mask = SectionPortalMask { bits: 0 };

    for start in 0..SECTION_VOXELS {
        let (word, bit) = (start >> 6, 1u64 << (start & 63));
        if (opaque[word] | visited[word]) & bit != 0 {
            continue;
        }
        let (mut qh, mut qt) = (0usize, 0usize);
        queue[qt] = start as u16;
        qt += 1;
        visited[word] |= bit;

        let mut touched: u8 = 0;
        while qh < qt {
            let cur = queue[qh] as usize;
            qh += 1;
            let x = cur & 15;
            let z = (cur >> 4) & 15;
            let y = cur >> 8;
            if x == 0 {
                touched |= 1 << Face::NegX as u8;
            }
            if x == 15 {
                touched |= 1 << Face::PosX as u8;
            }
            if y == 0 {
                touched |= 1 << Face::NegY as u8;
            }
            if y == 15 {
                touched |= 1 << Face::PosY as u8;
            }
            if z == 0 {
                touched |= 1 << Face::NegZ as u8;
            }
            if z == 15 {
                touched |= 1 << Face::PosZ as u8;
            }
            let neighbors = [
                (x > 0).then(|| cur - 1),
                (x < 15).then(|| cur + 1),
                (y > 0).then(|| cur - 256),
                (y < 15).then(|| cur + 256),
                (z > 0).then(|| cur - 16),
                (z < 15).then(|| cur + 16),
            ];
            for nb in neighbors.into_iter().flatten() {
                let (nw, nbit) = (nb >> 6, 1u64 << (nb & 63));
                if (opaque[nw] | visited[nw]) & nbit == 0 {
                    visited[nw] |= nbit;
                    queue[qt] = nb as u16;
                    qt += 1;
                }
            }
        }

        if touched.count_ones() >= 2 {
            for fa in Face::ALL {
                if touched & (1 << fa as u8) == 0 {
                    continue;
                }
                for fb in Face::ALL {
                    if touched & (1 << fb as u8) != 0 {
                        mask.set_bidirectional(fa, fb);
                    }
                }
            }
        }
    }
    mask
}

/// Box projected to normalized device coordinates; `near_clip_overlap` means it reaches behind the near plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectedBox {
    pub min_x: f32,
    pub max_x: f32,
    pub min_y: f32,
    pub max_y: f32,
    pub min_z: f32,
    pub max_z: f32,
    pub near_clip_overlap: bool,
}

pub fn project_aabb_ndc(vp: &Mat4, min_p: Vec3, max_p: Vec3) -> Option<ProjectedBox> {
    let mut out = ProjectedBox {
        min_x: 1e9,
        max_x: -1e9,
        min_y: 1e9,
        max_y: -1e9,
        min_z: 1e9,
        max_z: -1e9,
        near_clip_overlap: false,
    };
    for x in [min_p.x, max_p.x] {
        for y in [min_p.y, max_p.y] {
            for z in [min_p.z, max_p.z] {
                let c = vp.transform_point(Vec3::new(x, y, z));
                if c[3] <= 0.05 {
                    out.near_clip_overlap = true;
                    return Some(out);
                }
                let inv_w = 1.0 / c[3];
                let (nx, ny) = (c[0] * inv_w, c[1] * inv_w);
                let nz = c[2] * inv_w * 0.5 + 0.5;
                out.min_x = out.min_x.min(nx);
                out.max_x = out.max_x.max(nx);
                out.min_y = out.min_y.min(ny);
                out.max_y = out.max_y.max(ny);
                out.min_z = out.min_z.min(nz);
                out.max_z = out.max_z.max(nz);
            }
        }
    }
    if out.max_x < -1.0 || out.min_x > 1.0 || out.max_y < -1.0 || out.min_y > 1.0 || out.max_z < 0.0 {
        return None;
    }
    Some(out)
}

fn clamp_f32(v: f32, lo: f32, hi: f32) -> f32 {
    v.max(lo).min(hi)
}

/// Conservative Hi-Z pyramid: per tile, the farthest normalized depth of the occluders drawn into it (1.0 is far).
pub struct HiZPyramid {
    mip0: [f32; Self::W * Self::H],
    mip1: [f32; (Self::W / 2) * (Self::H / 2)],
    mip2: [f32; (Self::W / 4) * (Self::H / 4)],
    mip3: [f32; (Self::W / 8) * (Self::H / 8)],
}

impl HiZPyramid {
    pub const W: usize = 64;
    pub const H: usize = 32;

    pub fn new() -> Self {
        HiZPyramid {
            mip0: [1.0; Self::W * Self::H],
            mip1: [1.0; (Self::W / 2) * (Self::H / 2)],
            mip2: [1.0; (Self::W / 4) * (Self::H / 4)],
            mip3: [1.0; (Self::W / 8) * (Self::H / 8)],
        }
    }

    pub fn clear(&mut self) {
        self.mip0.fill(1.0);
        self.mip1.fill(1.0);
        self.mip2.fill(1.0);
        self.mip3.fill(1.0);
    }

    /// Writes an opaque occluder's farthest depth into the texels it fully covers.
    pub fn rasterize_opaque_box(&mut self, vp: &Mat4, min_p: Vec3, max_p: Vec3) {
        let Some(p) = project_aabb_ndc(vp, min_p, max_p) else { return };
        if p.near_clip_overlap {
            return; // a box that crosses the near plane cannot safely occlude anything behind it
        }
        let (w, h) = (Self::W as f32, Self::H as f32);
        let x0 = clamp_f32((p.min_x * 0.5 + 0.5) * w, 0.0, w).ceil() as usize;
        let x1 = clamp_f32((p.max_x * 0.5 + 0.5) * w, 0.0, w).floor() as usize;
        let y0 = clamp_f32((p.min_y * 0.5 + 0.5) * h, 0.0, h).ceil() as usize;
        let y1 = clamp_f32((p.max_y * 0.5 + 0.5) * h, 0.0, h).floor() as usize;
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        for y in y0..y1 {
            for x in x0..x1 {
                let t = &mut self.mip0[y * Self::W + x];
                if p.max_z < *t {
                    *t = p.max_z;
                }
            }
        }
    }

    pub fn build_mips(&mut self) {
        downsample_max(&self.mip0, Self::W, Self::H, &mut self.mip1);
        downsample_max(&self.mip1, Self::W / 2, Self::H / 2, &mut self.mip2);
        downsample_max(&self.mip2, Self::W / 4, Self::H / 4, &mut self.mip3);
    }

    /// True when the box is behind the farthest occluder depth of every Hi-Z texel it covers.
    pub fn is_occluded(&self, p: &ProjectedBox) -> bool {
        if p.near_clip_overlap {
            return false;
        }
        let (w, h) = (Self::W as f32, Self::H as f32);
        let fx0 = clamp_f32((p.min_x * 0.5 + 0.5) * w, 0.0, w - 1.0);
        let fx1 = clamp_f32((p.max_x * 0.5 + 0.5) * w, 0.0, w - 1.0);
        let fy0 = clamp_f32((p.min_y * 0.5 + 0.5) * h, 0.0, h - 1.0);
        let fy1 = clamp_f32((p.max_y * 0.5 + 0.5) * h, 0.0, h - 1.0);
        let span = (fx1 - fx0).max(fy1 - fy0);
        let level: u32 = if span <= 1.5 {
            0
        } else if span <= 3.5 {
            1
        } else if span <= 7.5 {
            2
        } else {
            3
        };
        let (mw, mh) = (Self::W >> level, Self::H >> level);
        let tx0 = ((fx0 as usize) >> level).min(mw - 1);
        let tx1 = ((fx1 as usize) >> level).min(mw - 1);
        let ty0 = ((fy0 as usize) >> level).min(mh - 1);
        let ty1 = ((fy1 as usize) >> level).min(mh - 1);
        let mip: &[f32] = match level {
            0 => &self.mip0,
            1 => &self.mip1,
            2 => &self.mip2,
            _ => &self.mip3,
        };
        let mut max_hiz = 0.0f32;
        for y in ty0..=ty1 {
            for x in tx0..=tx1 {
                max_hiz = max_hiz.max(mip[y * mw + x]);
            }
        }
        p.min_z > max_hiz + 1e-4
    }
}

impl Default for HiZPyramid {
    fn default() -> Self {
        Self::new()
    }
}

fn downsample_max(src: &[f32], sw: usize, sh: usize, dst: &mut [f32]) {
    let (dw, dh) = (sw / 2, sh / 2);
    for y in 0..dh {
        for x in 0..dw {
            let (sx, sy) = (x * 2, y * 2);
            let a = src[sy * sw + sx];
            let b = src[sy * sw + sx + 1];
            let c = src[(sy + 1) * sw + sx];
            let d = src[(sy + 1) * sw + sx + 1];
            dst[y * dw + x] = a.max(b).max(c.max(d));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_section_connects_all_faces() {
        let mask = compute_section_portals(&[0u64; 64]);
        assert_eq!(mask, SectionPortalMask::ALL_OPEN);
        assert!(mask.can_pass(Face::NegX, Face::PosZ));
    }

    #[test]
    fn sealed_section_has_no_portals() {
        // every voxel opaque: no transparent component, so nothing connects
        let mask = compute_section_portals(&[u64::MAX; 64]);
        assert_eq!(mask.bits, 0);
    }

    #[test]
    fn frustum_contains_a_box_in_front_and_rejects_one_behind() {
        let eye = Vec3::new(0.0, 0.0, 0.0);
        let view = Mat4::look_at(eye, Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 1.0, 0.0));
        let proj = Mat4::perspective(70f32.to_radians(), 1.0, 0.1, 100.0);
        let fr = Frustum6::from_view_proj(&proj.mul(&view));
        assert!(fr.intersects_aabb(Vec3::new(-1.0, -1.0, -6.0), Vec3::new(1.0, 1.0, -4.0)));
        assert!(!fr.intersects_aabb(Vec3::new(-1.0, -1.0, 50.0), Vec3::new(1.0, 1.0, 60.0)));
    }

    #[test]
    fn hiz_cleared_pyramid_occludes_nothing() {
        let hiz = HiZPyramid::new();
        let p = ProjectedBox {
            min_x: -0.5, max_x: 0.5, min_y: -0.5, max_y: 0.5, min_z: 0.5, max_z: 0.6, near_clip_overlap: false,
        };
        assert!(!hiz.is_occluded(&p));
    }

    #[test]
    fn hiz_occluder_hides_a_box_behind_it() {
        let eye = Vec3::new(0.0, 0.0, 0.0);
        let view = Mat4::look_at(eye, Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 1.0, 0.0));
        let vp = Mat4::perspective(70f32.to_radians(), 2.0, 0.1, 100.0).mul(&view);
        let mut hiz = HiZPyramid::new();
        hiz.rasterize_opaque_box(&vp, Vec3::new(-4.0, -4.0, -6.0), Vec3::new(4.0, 4.0, -5.0));
        hiz.build_mips();
        let behind = project_aabb_ndc(&vp, Vec3::new(-1.0, -1.0, -30.0), Vec3::new(1.0, 1.0, -20.0)).unwrap();
        assert!(hiz.is_occluded(&behind));
    }
}
