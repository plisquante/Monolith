//! Vanilla-Safe World Render Pipeline:
//!   - 100% non-intrusive: operates strictly on immutable `SectionSnapshot` copies (`16x16x16`),
//!     leaving vanilla tick, redstone, physics, and server state completely untouched.
//!   - 3-Stage Culling Pipeline:
//!       1. Camera FOV 6-plane Frustum AABB Culling (P-vertex test).
//!       2. 6-Face Portal BFS (CaveCulling reachability matrix computed over opaque bitsets)
//!          + Conservative Hierarchical-Z (`HiZPyramid`) screen-space occlusion rejection.
//!       3. Screen-Resolution Pixel-Coverage Culling & Dynamic LOD selection (sub-pixel cull,
//!          LOD0/LOD1/LOD2, front-to-back sort for GPU early-Z power savings).

const std = @import("std");
const mem_pool = @import("mem_pool.zig");
const rhi = @import("rhi.zig");

pub const SECTION_DIM: usize = 16;
pub const SECTION_VOXELS: usize = SECTION_DIM * SECTION_DIM * SECTION_DIM; // 4096

pub const Face = enum(u3) {
    neg_x = 0, // West
    pos_x = 1, // East
    neg_y = 2, // Down
    pos_y = 3, // Up
    neg_z = 4, // North
    pos_z = 5, // South

    pub inline fn opposite(self: Face) Face {
        return @enumFromInt(@intFromEnum(self) ^ 1);
    }

    pub inline fn dx(self: Face) i32 {
        return switch (self) {
            .neg_x => -1,
            .pos_x => 1,
            else => 0,
        };
    }
    pub inline fn dy(self: Face) i32 {
        return switch (self) {
            .neg_y => -1,
            .pos_y => 1,
            else => 0,
        };
    }
    pub inline fn dz(self: Face) i32 {
        return switch (self) {
            .neg_z => -1,
            .pos_z => 1,
            else => 0,
        };
    }
};

pub const LodLevel = enum(u8) {
    lod0_full = 0, // near: full geometry + cutout + mip0
    lod1_merged = 1, // mid: greedy-merged quads, skip tiny alpha-tested clutter
    lod2_coarse = 2, // far/low pixel area: coarse opaque silhouette only
};

pub const Vec3 = struct {
    x: f32,
    y: f32,
    z: f32,

    pub inline fn add(a: Vec3, b: Vec3) Vec3 {
        return .{ .x = a.x + b.x, .y = a.y + b.y, .z = a.z + b.z };
    }
    pub inline fn sub(a: Vec3, b: Vec3) Vec3 {
        return .{ .x = a.x - b.x, .y = a.y - b.y, .z = a.z - b.z };
    }
    pub inline fn dot(a: Vec3, b: Vec3) f32 {
        return a.x * b.x + a.y * b.y + a.z * b.z;
    }
    pub inline fn cross(a: Vec3, b: Vec3) Vec3 {
        return .{
            .x = a.y * b.z - a.z * b.y,
            .y = a.z * b.x - a.x * b.z,
            .z = a.x * b.y - a.y * b.x,
        };
    }
    pub inline fn normalize(a: Vec3) Vec3 {
        const l = @sqrt(a.x * a.x + a.y * a.y + a.z * a.z);
        if (l <= 1e-8) return .{ .x = 0, .y = 0, .z = 1 };
        const inv = 1.0 / l;
        return .{ .x = a.x * inv, .y = a.y * inv, .z = a.z * inv };
    }
};

/// Column-major 4x4 projection/view matrix.
pub const Mat4 = struct {
    m: [16]f32,

    pub fn perspectiveReverseOrStandard(fov_y_rad: f32, aspect: f32, near: f32, far: f32) Mat4 {
        const f = 1.0 / @tan(fov_y_rad * 0.5);
        const nf = 1.0 / (near - far);
        return .{ .m = .{
            f / aspect, 0, 0,                    0,
            0,          f, 0,                    0,
            0,          0, (far + near) * nf,    -1,
            0,          0, 2 * far * near * nf,  0,
        } };
    }

    pub fn lookAt(eye: Vec3, target: Vec3, up_in: Vec3) Mat4 {
        const fwd = Vec3.normalize(Vec3.sub(target, eye));
        const right = Vec3.normalize(Vec3.cross(fwd, up_in));
        const up = Vec3.cross(right, fwd);
        return .{ .m = .{
            right.x,            up.x,            -fwd.x,          0,
            right.y,            up.y,            -fwd.y,          0,
            right.z,            up.z,            -fwd.z,          0,
            -Vec3.dot(right, eye), -Vec3.dot(up, eye), Vec3.dot(fwd, eye), 1,
        } };
    }

    pub fn mul(a: Mat4, b: Mat4) Mat4 {
        var r: Mat4 = .{ .m = [_]f32{0} ** 16 };
        for (0..4) |col| {
            for (0..4) |row| {
                var sum: f32 = 0;
                for (0..4) |k| {
                    sum += a.m[k * 4 + row] * b.m[col * 4 + k];
                }
                r.m[col * 4 + row] = sum;
            }
        }
        return r;
    }

    pub inline fn transformPoint(self: *const Mat4, p: Vec3) [4]f32 {
        return .{
            self.m[0] * p.x + self.m[4] * p.y + self.m[8] * p.z + self.m[12],
            self.m[1] * p.x + self.m[5] * p.y + self.m[9] * p.z + self.m[13],
            self.m[2] * p.x + self.m[6] * p.y + self.m[10] * p.z + self.m[14],
            self.m[3] * p.x + self.m[7] * p.y + self.m[11] * p.z + self.m[15],
        };
    }
};

pub const Plane = struct {
    nx: f32,
    ny: f32,
    nz: f32,
    d: f32,

    pub fn normalize(self: Plane) Plane {
        const len = @sqrt(self.nx * self.nx + self.ny * self.ny + self.nz * self.nz);
        if (len <= 1e-8) return self;
        const inv = 1.0 / len;
        return .{ .nx = self.nx * inv, .ny = self.ny * inv, .nz = self.nz * inv, .d = self.d * inv };
    }
};

/// Gribb-Hartmann 6-plane FOV frustum extracted directly from combined ViewProjection.
pub const Frustum6 = struct {
    planes: [6]Plane,

    pub fn fromViewProj(vp: Mat4) Frustum6 {
        // Row access on column-major Mat4: row r is (m[0+r], m[4+r], m[8+r], m[12+r])
        const r0 = [4]f32{ vp.m[0], vp.m[4], vp.m[8], vp.m[12] };
        const r1 = [4]f32{ vp.m[1], vp.m[5], vp.m[9], vp.m[13] };
        const r2 = [4]f32{ vp.m[2], vp.m[6], vp.m[10], vp.m[14] };
        const r3 = [4]f32{ vp.m[3], vp.m[7], vp.m[11], vp.m[15] };

        return .{ .planes = .{
            (Plane{ .nx = r3[0] + r0[0], .ny = r3[1] + r0[1], .nz = r3[2] + r0[2], .d = r3[3] + r0[3] }).normalize(), // left
            (Plane{ .nx = r3[0] - r0[0], .ny = r3[1] - r0[1], .nz = r3[2] - r0[2], .d = r3[3] - r0[3] }).normalize(), // right
            (Plane{ .nx = r3[0] + r1[0], .ny = r3[1] + r1[1], .nz = r3[2] + r1[2], .d = r3[3] + r1[3] }).normalize(), // bottom
            (Plane{ .nx = r3[0] - r1[0], .ny = r3[1] - r1[1], .nz = r3[2] - r1[2], .d = r3[3] - r1[3] }).normalize(), // top
            (Plane{ .nx = r3[0] + r2[0], .ny = r3[1] + r2[1], .nz = r3[2] + r2[2], .d = r3[3] + r2[3] }).normalize(), // near
            (Plane{ .nx = r3[0] - r2[0], .ny = r3[1] - r2[1], .nz = r3[2] - r2[2], .d = r3[3] - r2[3] }).normalize(), // far
        } };
    }

    /// Fast P-vertex AABB intersection test (zero heap alloc, ~12ns).
    pub inline fn intersectsAabb(self: *const Frustum6, min_p: Vec3, max_p: Vec3) bool {
        for (self.planes) |p| {
            const px = if (p.nx >= 0) max_p.x else min_p.x;
            const py = if (p.ny >= 0) max_p.y else min_p.y;
            const pz = if (p.nz >= 0) max_p.z else min_p.z;
            if (p.nx * px + p.ny * py + p.nz * pz + p.d < 0) return false;
        }
        return true;
    }
};

/// 6-face reachability matrix inside a 16x16x16 chunk section (`bit(in*6 + out) == 1` iff a
/// flood-fill path of non-opaque voxels connects `in` face to `out` face).
pub const SectionPortalMask = struct {
    bits: u64 = 0,

    pub const ALL_OPEN: SectionPortalMask = .{ .bits = (1 << 36) - 1 };

    pub inline fn canPass(self: SectionPortalMask, from: Face, to: Face) bool {
        const idx: u6 = @as(u6, @intFromEnum(from)) * 6 + @as(u6, @intFromEnum(to));
        return ((self.bits >> idx) & 1) != 0;
    }

    pub inline fn setBidirectional(self: *SectionPortalMask, a: Face, b: Face) void {
        const idx1: u6 = @as(u6, @intFromEnum(a)) * 6 + @as(u6, @intFromEnum(b));
        const idx2: u6 = @as(u6, @intFromEnum(b)) * 6 + @as(u6, @intFromEnum(a));
        self.bits |= (@as(u64, 1) << idx1) | (@as(u64, 1) << idx2);
    }
};

/// Computes exact 6-face CaveCulling portal reachability bitmask over a 16x16x16 opaque voxel bitset
/// (`[64]u64`, 1 bit per block). Runs asynchronously on worker pool without touching vanilla ticks.
pub fn computeSectionPortals(opaque_bits: *const [64]u64) SectionPortalMask {
    var visited: [64]u64 = [_]u64{0} ** 64;
    var queue: [SECTION_VOXELS]u16 = undefined;
    var mask = SectionPortalMask{};

    var start_idx: usize = 0;
    while (start_idx < SECTION_VOXELS) : (start_idx += 1) {
        const word = start_idx >> 6;
        const bit = @as(u64, 1) << @as(u6, @intCast(start_idx & 63));
        if (((opaque_bits[word] | visited[word]) & bit) != 0) continue;

        // BFS flood fill for connected transparent component
        var qh: usize = 0;
        var qt: usize = 0;
        queue[qt] = @intCast(start_idx);
        qt += 1;
        visited[word] |= bit;

        var touched_faces: u8 = 0;
        while (qh < qt) {
            const cur_i = queue[qh];
            qh += 1;
            const x = cur_i & 15;
            const z = (cur_i >> 4) & 15;
            const y = cur_i >> 8;

            if (x == 0) touched_faces |= (@as(u8, 1) << @intFromEnum(Face.neg_x));
            if (x == 15) touched_faces |= (@as(u8, 1) << @intFromEnum(Face.pos_x));
            if (y == 0) touched_faces |= (@as(u8, 1) << @intFromEnum(Face.neg_y));
            if (y == 15) touched_faces |= (@as(u8, 1) << @intFromEnum(Face.pos_y));
            if (z == 0) touched_faces |= (@as(u8, 1) << @intFromEnum(Face.neg_z));
            if (z == 15) touched_faces |= (@as(u8, 1) << @intFromEnum(Face.pos_z));

            const neighbors = [6]?u16{
                if (x > 0) cur_i - 1 else null,
                if (x < 15) cur_i + 1 else null,
                if (y > 0) cur_i - 256 else null,
                if (y < 15) cur_i + 256 else null,
                if (z > 0) cur_i - 16 else null,
                if (z < 15) cur_i + 16 else null,
            };
            for (neighbors) |nb_opt| {
                const nb = nb_opt orelse continue;
                const nw = nb >> 6;
                const nbit = @as(u64, 1) << @as(u6, @intCast(nb & 63));
                if (((opaque_bits[nw] | visited[nw]) & nbit) == 0) {
                    visited[nw] |= nbit;
                    queue[qt] = nb;
                    qt += 1;
                }
            }
        }

        if (@popCount(touched_faces) >= 2) {
            for (0..6) |fa| {
                if ((touched_faces & (@as(u8, 1) << @as(u3, @intCast(fa)))) == 0) continue;
                for (0..6) |fb| {
                    if ((touched_faces & (@as(u8, 1) << @as(u3, @intCast(fb)))) == 0) continue;
                    mask.setBidirectional(@enumFromInt(fa), @enumFromInt(fb));
                }
            }
        }
    }
    return mask;
}

/// Conservative Hi-Z Occlusion Pyramid (`64x32` mip0 down to `1x1`) storing MAX normalized depth `[0..1]`
/// per tile (`1.0` = far plane, `0.0` = near plane). An AABB whose minimum NDC depth exceeds the max
/// depth of its conservative Hi-Z tile is 100% occluded cleanly.
pub const HiZPyramid = struct {
    pub const BASE_W: usize = 64;
    pub const BASE_H: usize = 32;
    pub const TOTAL_TEXELS: usize = BASE_W * BASE_H + (BASE_W / 2) * (BASE_H / 2) + (BASE_W / 4) * (BASE_H / 4) +
        (BASE_W / 8) * (BASE_H / 8) + (BASE_W / 16) * (BASE_H / 16) + 2 + 1;

    mip0: [BASE_W * BASE_H]f32 = [_]f32{1.0} ** (BASE_W * BASE_H),
    mip1: [(BASE_W / 2) * (BASE_H / 2)]f32 = [_]f32{1.0} ** ((BASE_W / 2) * (BASE_H / 2)),
    mip2: [(BASE_W / 4) * (BASE_H / 4)]f32 = [_]f32{1.0} ** ((BASE_W / 4) * (BASE_H / 4)),
    mip3: [(BASE_W / 8) * (BASE_H / 8)]f32 = [_]f32{1.0} ** ((BASE_W / 8) * (BASE_H / 8)),

    pub fn clear(self: *HiZPyramid) void {
        @memset(&self.mip0, 1.0);
        @memset(&self.mip1, 1.0);
        @memset(&self.mip2, 1.0);
        @memset(&self.mip3, 1.0);
    }

    /// Rasterizes an opaque foreground occluder section / wall into mip0 conservatively (writes
    /// the furthest depth of the occluder within strictly covered interior pixels).
    pub fn rasterizeOpaqueBox(self: *HiZPyramid, vp: *const Mat4, min_p: Vec3, max_p: Vec3) void {
        const proj = projectAabbNdc(vp, min_p, max_p) orelse return;
        if (proj.near_clip_overlap) return; // camera-intersecting box cannot safely occlude behind near plane
        const x0 = @as(usize, @intFromFloat(@ceil(std.math.clamp((proj.min_x * 0.5 + 0.5) * BASE_W, 0.0, @as(f32, BASE_W)))));
        const x1 = @as(usize, @intFromFloat(@floor(std.math.clamp((proj.max_x * 0.5 + 0.5) * BASE_W, 0.0, @as(f32, BASE_W)))));
        const y0 = @as(usize, @intFromFloat(@ceil(std.math.clamp((proj.min_y * 0.5 + 0.5) * BASE_H, 0.0, @as(f32, BASE_H)))));
        const y1 = @as(usize, @intFromFloat(@floor(std.math.clamp((proj.max_y * 0.5 + 0.5) * BASE_H, 0.0, @as(f32, BASE_H)))));
        if (x1 <= x0 or y1 <= y0) return;

        const occluder_max_z = proj.max_z;
        var y = y0;
        while (y < y1) : (y += 1) {
            var x = x0;
            while (x < x1) : (x += 1) {
                const idx = y * BASE_W + x;
                if (occluder_max_z < self.mip0[idx]) {
                    self.mip0[idx] = occluder_max_z;
                }
            }
        }
    }

    pub fn buildMips(self: *HiZPyramid) void {
        downsampleMax(&self.mip0, BASE_W, BASE_H, &self.mip1);
        downsampleMax(&self.mip1, BASE_W / 2, BASE_H / 2, &self.mip2);
        downsampleMax(&self.mip2, BASE_W / 4, BASE_H / 4, &self.mip3);
    }

    fn downsampleMax(src: []const f32, sw: usize, sh: usize, dst: []f32) void {
        const dw = sw / 2;
        const dh = sh / 2;
        for (0..dh) |y| {
            for (0..dw) |x| {
                const sx = x * 2;
                const sy = y * 2;
                const a = src[sy * sw + sx];
                const b = src[sy * sw + sx + 1];
                const c = src[(sy + 1) * sw + sx];
                const d = src[(sy + 1) * sw + sx + 1];
                dst[y * dw + x] = @max(@max(a, b), @max(c, d));
            }
        }
    }

    pub inline fn isOccluded(self: *const HiZPyramid, proj: ProjectedBox) bool {
        if (proj.near_clip_overlap) return false;
        const fx0 = std.math.clamp((proj.min_x * 0.5 + 0.5) * BASE_W, 0.0, @as(f32, BASE_W - 1));
        const fx1 = std.math.clamp((proj.max_x * 0.5 + 0.5) * BASE_W, 0.0, @as(f32, BASE_W - 1));
        const fy0 = std.math.clamp((proj.min_y * 0.5 + 0.5) * BASE_H, 0.0, @as(f32, BASE_H - 1));
        const fy1 = std.math.clamp((proj.max_y * 0.5 + 0.5) * BASE_H, 0.0, @as(f32, BASE_H - 1));
        const span = @max(fx1 - fx0, fy1 - fy0);

        const level: u2 = if (span <= 1.5) 0 else if (span <= 3.5) 1 else if (span <= 7.5) 2 else 3;
        const shift: u3 = level;
        const w = BASE_W >> shift;
        const h = BASE_H >> shift;
        const tx0 = @min(@as(usize, @intFromFloat(fx0)) >> shift, w - 1);
        const tx1 = @min(@as(usize, @intFromFloat(fx1)) >> shift, w - 1);
        const ty0 = @min(@as(usize, @intFromFloat(fy0)) >> shift, h - 1);
        const ty1 = @min(@as(usize, @intFromFloat(fy1)) >> shift, h - 1);

        const mip: []const f32 = switch (level) {
            0 => &self.mip0,
            1 => &self.mip1,
            2 => &self.mip2,
            3 => &self.mip3,
        };
        var max_hiz: f32 = 0.0;
        var y = ty0;
        while (y <= ty1) : (y += 1) {
            var x = tx0;
            while (x <= tx1) : (x += 1) {
                max_hiz = @max(max_hiz, mip[y * w + x]);
            }
        }
        return proj.min_z > max_hiz + 1e-4;
    }
};

pub const ProjectedBox = struct {
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
    min_z: f32,
    max_z: f32,
    near_clip_overlap: bool,
};

pub fn projectAabbNdc(vp: *const Mat4, min_p: Vec3, max_p: Vec3) ?ProjectedBox {
    var out = ProjectedBox{
        .min_x = 1e9,
        .max_x = -1e9,
        .min_y = 1e9,
        .max_y = -1e9,
        .min_z = 1e9,
        .max_z = -1e9,
        .near_clip_overlap = false,
    };
    const xs = [2]f32{ min_p.x, max_p.x };
    const ys = [2]f32{ min_p.y, max_p.y };
    const zs = [2]f32{ min_p.z, max_p.z };
    for (xs) |x| {
        for (ys) |y| {
            for (zs) |z| {
                const c = vp.transformPoint(.{ .x = x, .y = y, .z = z });
                if (c[3] <= 0.05) {
                    out.near_clip_overlap = true;
                    return out;
                }
                const inv_w = 1.0 / c[3];
                const nx = c[0] * inv_w;
                const ny = c[1] * inv_w;
                const nz = c[2] * inv_w * 0.5 + 0.5;
                out.min_x = @min(out.min_x, nx);
                out.max_x = @max(out.max_x, nx);
                out.min_y = @min(out.min_y, ny);
                out.max_y = @max(out.max_y, ny);
                out.min_z = @min(out.min_z, nz);
                out.max_z = @max(out.max_z, nz);
            }
        }
    }
    if (out.max_x < -1.0 or out.min_x > 1.0 or out.max_y < -1.0 or out.min_y > 1.0 or out.max_z < 0.0) {
        return null;
    }
    return out;
}

/// Immutable chunk section descriptor snapshotted cleanly from vanilla level without blocking ticks.
pub const SectionSnapshot = struct {
    sx: i32,
    sy: i32,
    sz: i32,
    revision: u32 = 1,
    has_non_air: bool = true,
    fully_opaque_occluder: bool = false,
    portals: SectionPortalMask = SectionPortalMask.ALL_OPEN,
    index_count: u32 = 36,
    first_index: u32 = 0,
    vertex_offset: i32 = 0,

    pub inline fn worldMin(self: *const SectionSnapshot) Vec3 {
        return .{
            .x = @floatFromInt(self.sx * 16),
            .y = @floatFromInt(self.sy * 16),
            .z = @floatFromInt(self.sz * 16),
        };
    }
    pub inline fn worldMax(self: *const SectionSnapshot) Vec3 {
        return .{
            .x = @floatFromInt(self.sx * 16 + 16),
            .y = @floatFromInt(self.sy * 16 + 16),
            .z = @floatFromInt(self.sz * 16 + 16),
        };
    }
};

pub const VisibleSection = struct {
    snapshot_idx: u32,
    dist_sq: f32,
    screen_area_px: f32,
    lod: LodLevel,
};

pub const CullStats = struct {
    total_sections: usize = 0,
    frustum_culled: usize = 0,
    portal_culled: usize = 0,
    hiz_occluded: usize = 0,
    subpixel_culled: usize = 0,
    visible_sections: usize = 0,
};

pub const SectionCoordKey = struct { x: i32, y: i32, z: i32 };

/// Executes the complete 3-stage visibility cull (FOV Frustum -> 6-Face Portal BFS -> Hi-Z + Resolution LOD)
/// strictly inside a GC-free `ArenaAllocator`, emitting sorted visible sections and RHI indirect draws.
pub fn cullAndScheduleWorld(
    arena: *mem_pool.ArenaAllocator,
    sections: []const SectionSnapshot,
    camera_pos: Vec3,
    view_proj: Mat4,
    viewport_w: u32,
    viewport_h: u32,
    min_pixel_area: f32,
    hiz: *HiZPyramid,
    out_stats: *CullStats,
) ![]VisibleSection {
    out_stats.* = .{ .total_sections = sections.len };
    if (sections.len == 0) return &.{};

    const alloc = arena.allocator();
    const frustum = Frustum6.fromViewProj(view_proj);

    // Build fast coordinate -> index map inside frame arena (zero malloc!)
    var coord_map = std.AutoHashMapUnmanaged(SectionCoordKey, u32){};
    try coord_map.ensureTotalCapacity(alloc, @intCast(sections.len));
    for (sections, 0..) |s, idx| {
        coord_map.putAssumeCapacity(.{ .x = s.sx, .y = s.sy, .z = s.sz }, @intCast(idx));
    }

    const visited = try alloc.alloc(bool, sections.len);
    @memset(visited, false);
    const entry_mask = try alloc.alloc(u8, sections.len);
    @memset(entry_mask, 0);

    // Step 1: Seed Hi-Z pyramid from nearby opaque occluder sections inside frustum
    hiz.clear();
    const cam_sx: i32 = @intFromFloat(@floor(camera_pos.x / 16.0));
    const cam_sy: i32 = @intFromFloat(@floor(camera_pos.y / 16.0));
    const cam_sz: i32 = @intFromFloat(@floor(camera_pos.z / 16.0));

    for (sections) |s| {
        if (!s.fully_opaque_occluder) continue;
        const mn = s.worldMin();
        const mx = s.worldMax();
        if (!frustum.intersectsAabb(mn, mx)) continue;
        hiz.rasterizeOpaqueBox(&view_proj, mn, mx);
    }
    hiz.buildMips();

    // Step 2: BFS Portal Traversal seeded at camera's current section (or closest visible sections).
    // The state is (section, entry face): a section reached through a *new* face is processed again,
    // because the exits it opens depend on the entry face (a first-come-only visit hides terrain that
    // is reachable only through a later entry). Each pair is enqueued at most once (6 per section).
    const QueueItem = struct { idx: u32, enter_face_bits: u8 };
    var bfs = std.ArrayListUnmanaged(QueueItem){};
    try bfs.ensureTotalCapacity(alloc, sections.len * 7);
    const emitted = try alloc.alloc(bool, sections.len);
    @memset(emitted, false);

    if (coord_map.get(.{ .x = cam_sx, .y = cam_sy, .z = cam_sz })) |start_idx| {
        visited[start_idx] = true;
        entry_mask[start_idx] = 0b111111; // camera inside: can exit through any face
        bfs.appendAssumeCapacity(.{ .idx = start_idx, .enter_face_bits = 0b111111 });
    } else {
        // Fallback when camera sits outside snapshot bounds (e.g., high orbit): seed boundary sections
        for (sections, 0..) |s, idx| {
            if (!frustum.intersectsAabb(s.worldMin(), s.worldMax())) continue;
            visited[idx] = true;
            entry_mask[idx] = 0b111111;
            bfs.appendAssumeCapacity(.{ .idx = @intCast(idx), .enter_face_bits = 0b111111 });
        }
    }

    var visible = std.ArrayListUnmanaged(VisibleSection){};
    try visible.ensureTotalCapacity(alloc, sections.len);

    var head: usize = 0;
    const vw_f: f32 = @floatFromInt(viewport_w);
    const vh_f: f32 = @floatFromInt(viewport_h);

    while (head < bfs.items.len) : (head += 1) {
        const item = bfs.items[head];
        const s = &sections[item.idx];
        const mn = s.worldMin();
        const mx = s.worldMax();

        // Stage A: Frustum check
        if (!frustum.intersectsAabb(mn, mx)) {
            if (!emitted[item.idx]) {
                emitted[item.idx] = true;
                out_stats.frustum_culled += 1;
            }
            continue;
        }

        // Propagate portals outward along faces reachable from `enter_face_bits`
        for (0..6) |out_fi| {
            const out_face: Face = @enumFromInt(out_fi);
            var can_reach = false;
            if (item.enter_face_bits == 0b111111) {
                can_reach = true;
            } else {
                for (0..6) |in_fi| {
                    if ((item.enter_face_bits & (@as(u8, 1) << @as(u3, @intCast(in_fi)))) != 0) {
                        if (s.portals.canPass(@enumFromInt(in_fi), out_face)) {
                            can_reach = true;
                            break;
                        }
                    }
                }
            }
            if (!can_reach) continue;

            const nkey = SectionCoordKey{
                .x = s.sx + out_face.dx(),
                .y = s.sy + out_face.dy(),
                .z = s.sz + out_face.dz(),
            };
            // Directional outward check relative to camera section prevents endless back-forth BFS cycles
            const dir_dot = (nkey.x - s.sx) * (s.sx - cam_sx) +
                (nkey.y - s.sy) * (s.sy - cam_sy) +
                (nkey.z - s.sz) * (s.sz - cam_sz);
            if (item.enter_face_bits != 0b111111 and dir_dot < 0) continue;

            if (coord_map.get(nkey)) |nb_idx| {
                const enter_bit: u8 = @as(u8, 1) << @intFromEnum(out_face.opposite());
                if ((entry_mask[nb_idx] & enter_bit) == 0) {
                    entry_mask[nb_idx] |= enter_bit;
                    visited[nb_idx] = true;
                    bfs.appendAssumeCapacity(.{ .idx = nb_idx, .enter_face_bits = enter_bit });
                }
            }
        }

        // visibility tests do not depend on the entry face: run them once per section
        if (emitted[item.idx]) continue;
        emitted[item.idx] = true;
        if (!s.has_non_air) continue;

        // Stage B: Conservative Hi-Z occlusion + Stage C: Screen resolution pixel area & LOD
        const proj_opt = projectAabbNdc(&view_proj, mn, mx);
        const cx = mn.x + 8.0 - camera_pos.x;
        const cy = mn.y + 8.0 - camera_pos.y;
        const cz = mn.z + 8.0 - camera_pos.z;
        const dist_sq = cx * cx + cy * cy + cz * cz;

        var area_px: f32 = vw_f * vh_f;
        if (proj_opt) |proj| {
            if (!s.fully_opaque_occluder and hiz.isOccluded(proj)) {
                out_stats.hiz_occluded += 1;
                continue;
            }
            if (!proj.near_clip_overlap) {
                const sx0 = std.math.clamp(proj.min_x * 0.5 + 0.5, 0.0, 1.0);
                const sx1 = std.math.clamp(proj.max_x * 0.5 + 0.5, 0.0, 1.0);
                const sy0 = std.math.clamp(proj.min_y * 0.5 + 0.5, 0.0, 1.0);
                const sy1 = std.math.clamp(proj.max_y * 0.5 + 0.5, 0.0, 1.0);
                area_px = (sx1 - sx0) * vw_f * (sy1 - sy0) * vh_f;
                if (area_px < min_pixel_area) {
                    out_stats.subpixel_culled += 1;
                    continue;
                }
            }
        } else {
            out_stats.frustum_culled += 1;
            continue;
        }

        const lod: LodLevel = if (area_px >= 4096.0)
            .lod0_full
        else if (area_px >= 512.0)
            .lod1_merged
        else
            .lod2_coarse;

        visible.appendAssumeCapacity(.{
            .snapshot_idx = item.idx,
            .dist_sq = dist_sq,
            .screen_area_px = area_px,
            .lod = lod,
        });
    }

    // Count sections completely unreached by BFS (portal-occluded underground/behind walls)
    var unvisited_in_frustum: usize = 0;
    for (sections, 0..) |s, i| {
        if (!visited[i]) {
            if (frustum.intersectsAabb(s.worldMin(), s.worldMax())) {
                unvisited_in_frustum += 1;
            } else {
                out_stats.frustum_culled += 1;
            }
        }
    }
    out_stats.portal_culled = unvisited_in_frustum;
    out_stats.visible_sections = visible.items.len;

    // Sort front-to-back for early-Z GPU depth rejection (reduces fragment shading power)
    std.mem.sort(VisibleSection, visible.items, {}, struct {
        fn lessThan(_: void, a: VisibleSection, b: VisibleSection) bool {
            return a.dist_sq < b.dist_sq;
        }
    }.lessThan);

    return visible.items;
}

test "Portal CaveCulling blocks solid underground wall & allows open tunnel" {
    var solid: [64]u64 = [_]u64{0xFFFFFFFFFFFFFFFF} ** 64;
    const solid_mask = computeSectionPortals(&solid);
    try std.testing.expectEqual(@as(u64, 0), solid_mask.bits);

    // Carve an open 2x2 tunnel straight along X from face neg_x (x=0) to pos_x (x=15) at y=8,z=8
    for (0..16) |x| {
        const idx: usize = (8 << 8) | (8 << 4) | x;
        solid[idx >> 6] &= ~(@as(u64, 1) << @as(u6, @intCast(idx & 63)));
    }
    const tunnel_mask = computeSectionPortals(&solid);
    try std.testing.expect(tunnel_mask.canPass(.neg_x, .pos_x));
    try std.testing.expect(tunnel_mask.canPass(.pos_x, .neg_x));
    try std.testing.expect(!tunnel_mask.canPass(.neg_x, .pos_y));
}

test "3-Stage FOV + Portal + Hi-Z + Resolution culling eliminates behind/occluded/subpixel sections" {
    var arena = mem_pool.ArenaAllocator.init(mem_pool.DEFAULT_CHUNK_SIZE, 2);
    defer arena.deinit();

    var sections: [5]SectionSnapshot = undefined;
    // 0: camera section at (0,0,0) - open
    sections[0] = .{ .sx = 0, .sy = 0, .sz = 0, .has_non_air = false, .portals = SectionPortalMask.ALL_OPEN };
    // 1: solid opaque occluder wall right in front of camera at (0,0,-2)
    sections[1] = .{ .sx = 0, .sy = 0, .sz = -2, .fully_opaque_occluder = true, .portals = .{ .bits = 0 } };
    // 2: open intermediate section between camera and wall (0,0,-1)
    sections[2] = .{ .sx = 0, .sy = 0, .sz = -1, .has_non_air = true, .portals = SectionPortalMask.ALL_OPEN };
    // 3: hidden behind the opaque wall at (0,0,-3) -> must be culled by portal AND/OR Hi-Z!
    sections[3] = .{ .sx = 0, .sy = 0, .sz = -3, .has_non_air = true, .portals = SectionPortalMask.ALL_OPEN };
    // 4: behind the camera at (0,0,+2) -> must be FOV-frustum culled!
    sections[4] = .{ .sx = 0, .sy = 0, .sz = 2, .has_non_air = true, .portals = SectionPortalMask.ALL_OPEN };

    const cam = Vec3{ .x = 8, .y = 8, .z = 8 };
    const view = Mat4.lookAt(cam, .{ .x = 8, .y = 8, .z = -8 }, .{ .x = 0, .y = 1, .z = 0 });
    const proj = Mat4.perspectiveReverseOrStandard(std.math.degreesToRadians(70.0), 16.0 / 9.0, 0.5, 512.0);
    const vp = Mat4.mul(proj, view);

    var hiz = HiZPyramid{};
    var stats = CullStats{};
    const visible = try cullAndScheduleWorld(&arena, &sections, cam, vp, 1920, 1080, 4.0, &hiz, &stats);

    // Only sections 2 (in front of wall) and 1 (the wall itself) should survive; 3 & 4 must be culled!
    try std.testing.expectEqual(@as(usize, 2), visible.len);
    for (visible) |v| {
        try std.testing.expect(v.snapshot_idx == 1 or v.snapshot_idx == 2);
    }
    try std.testing.expect(stats.frustum_culled >= 1);
}

test "Portal BFS accumulates every entry face (diamond path must not hide visible terrain)" {
    var arena = mem_pool.ArenaAllocator.init(mem_pool.DEFAULT_CHUNK_SIZE, 2);
    defer arena.deinit();

    // Layout (top view, -z is north):      Q(0,0,-1) -> T(1,0,-1) -> U(2,0,-1)
    //                                      C(0,0,0)  -> P(1,0,0)  ---^ (P reaches T from the south)
    // T only lets light pass west<->east. BFS reaches T first from P (south face), later from Q
    // (west face). U is visible only through Q -> T(west->east). Using only the first entry face
    // of T would wrongly cull U.
    var only_we = SectionPortalMask{};
    only_we.setBidirectional(.neg_x, .pos_x);
    var sections = [_]SectionSnapshot{
        .{ .sx = 0, .sy = 0, .sz = 0, .has_non_air = false },
        .{ .sx = 1, .sy = 0, .sz = 0 },
        .{ .sx = 0, .sy = 0, .sz = -1 },
        .{ .sx = 1, .sy = 0, .sz = -1, .portals = only_we },
        .{ .sx = 2, .sy = 0, .sz = -1 },
    };
    const cam = Vec3{ .x = 8, .y = 8, .z = 8 };
    const view = Mat4.lookAt(cam, .{ .x = 40, .y = 8, .z = -24 }, .{ .x = 0, .y = 1, .z = 0 });
    const proj = Mat4.perspectiveReverseOrStandard(std.math.degreesToRadians(70.0), 16.0 / 9.0, 0.5, 512.0);
    const vp = Mat4.mul(proj, view);

    var hiz = HiZPyramid{};
    var stats = CullStats{};
    const visible = try cullAndScheduleWorld(&arena, &sections, cam, vp, 1920, 1080, 1.0, &hiz, &stats);

    var saw_u = false;
    for (visible) |v| {
        if (v.snapshot_idx == 4) saw_u = true;
    }
    try std.testing.expect(saw_u);
}
