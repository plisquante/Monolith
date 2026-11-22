//! GPU-Driven Unified Non-Block Instancing Engine:
//! Compacts all non-block scene entities — Display Entities (BlockDisplay, ItemDisplay,
//! TextDisplay), mobs, players, dropped items, block entities (chests, signs, banners),
//! and compact particles — into cacheline-aligned GPU instance buffers (`64B` / `32B`)
//! and emits minimal RHI indirect/instanced draw batches across Vulkan and OpenGL 3.3 Core.

const std = @import("std");
const mem_pool = @import("mem_pool.zig");
const rhi = @import("rhi.zig");
const wr = @import("world_render.zig");

pub const EntityKind = enum(u4) {
    block_display = 0,
    item_display = 1,
    text_display = 2,
    mob = 3,
    player = 4,
    dropped_item = 5,
    chest = 6,
    sign = 7,
    particle = 8,
};

pub const BillboardMode = enum(u2) {
    fixed = 0,
    vertical = 1,
    horizontal = 2,
    center = 3,
};

/// 64-byte cacheline-aligned GPU instance descriptor shared verbatim by Vulkan SSBO & GL3.3 TBO (4x RGBA32F texels).
pub const GpuInstance = extern struct {
    row0: [4]f32, // m00, m01, m02, tx
    row1: [4]f32, // m10, m11, m12, ty
    row2: [4]f32, // m20, m21, m22, tz
    uv_rect: u32, // packed (u0:u8, v0:u8, u1:u8, v1:u8) normalized atlas cell
    color_light: u32, // RGB8 tint (24b) + block/sky light (8b)
    flags: u32, // EntityKind(4b) | BillboardMode(2b) | hurt_flash(1b) | custom(25b)
    mesh_and_radius: u32, // lower 16b = mesh_id, upper 16b = bounding radius * 256 (max 255.9 blocks)

    comptime {
        if (@sizeOf(GpuInstance) != 64) @compileError("GpuInstance must remain 64 bytes (4x RGBA32F)");
    }

    pub fn makeFlags(kind: EntityKind, billboard: BillboardMode, hurt_flash: bool) u32 {
        return @as(u32, @intFromEnum(kind)) |
            (@as(u32, @intFromEnum(billboard)) << 4) |
            (@as(u32, @intFromBool(hurt_flash)) << 6);
    }

    pub inline fn meshId(self: *const GpuInstance) u16 {
        return @truncate(self.mesh_and_radius);
    }

    pub inline fn radius(self: *const GpuInstance) f32 {
        const q: u16 = @truncate(self.mesh_and_radius >> 16);
        return @as(f32, @floatFromInt(q)) / 256.0;
    }

    pub inline fn packMeshRadius(mesh_id: u16, r: f32) u32 {
        const clamped = std.math.clamp(r * 256.0, 1.0, 65535.0);
        const rq: u32 = @intFromFloat(clamped);
        return @as(u32, mesh_id) | (rq << 16);
    }

    pub inline fn worldCenter(self: *const GpuInstance) wr.Vec3 {
        return .{ .x = self.row0[3], .y = self.row1[3], .z = self.row2[3] };
    }
};

/// 32-byte compact GPU particle instance (interpolated GPU-side cleanly with zero CPU matrix rebuild).
pub const GpuParticle = extern struct {
    pos: [3]f32,
    size_rot_packed: u32,
    vel_packed: u32, // 3x quantized f10/i10 velocity components
    rgba8: u32,
    uv_rect: u32,
    light_age: u32,

    comptime {
        if (@sizeOf(GpuParticle) != 32) @compileError("GpuParticle must remain 32 bytes (2x RGBA32F)");
    }
};

pub const MeshProto = struct {
    mesh_id: u16,
    index_count: u32,
    first_index: u32,
    vertex_offset: i32,
};

pub const InstancingStats = struct {
    submitted_total: usize = 0,
    frustum_culled: usize = 0,
    hiz_culled: usize = 0,
    subpixel_culled: usize = 0,
    /// instances that were actually put into a draw batch
    surviving_instances: usize = 0,
    /// instances that survived culling but whose mesh was never registered (not drawn)
    unregistered_mesh_instances: usize = 0,
    draw_batches_emitted: usize = 0,
};

/// Translucent kinds must be drawn after all opaque ones.
inline fn isTranslucent(inst: *const GpuInstance) bool {
    const k = inst.flags & 0xF;
    return k == @intFromEnum(EntityKind.particle) or k == @intFromEnum(EntityKind.text_display);
}

/// Sort/run key: (translucent, mesh_id). Opaque instances order before translucent ones.
inline fn runKey(inst: *const GpuInstance) u32 {
    return (@as(u32, @intFromBool(isTranslucent(inst))) << 16) | inst.meshId();
}

/// Collects all non-block entities/displays/particles for a frame cleanly into an `ArenaAllocator`,
/// culls them against frustum + Hi-Z + resolution area, sorts/compacts by `mesh_id`, uploads once
/// into the RHI transient ring, and emits compact multi-draw indirect / instanced batches.
pub const InstanceBatcher = struct {
    raw_items: std.ArrayListUnmanaged(GpuInstance) = .{},
    mesh_table: std.AutoHashMapUnmanaged(u16, MeshProto) = .{},
    stats: InstancingStats = .{},

    pub fn beginFrame(self: *InstanceBatcher, arena: *mem_pool.ArenaAllocator, reserve_instances: usize) !void {
        self.raw_items = .{};
        self.mesh_table = .{};
        self.stats = .{};
        const alloc = arena.allocator();
        if (reserve_instances > 0) {
            try self.raw_items.ensureTotalCapacity(alloc, reserve_instances);
        }
    }

    pub fn registerMesh(self: *InstanceBatcher, arena: *mem_pool.ArenaAllocator, proto: MeshProto) !void {
        try self.mesh_table.put(arena.allocator(), proto.mesh_id, proto);
    }

    pub fn pushTranslationEntity(
        self: *InstanceBatcher,
        arena: *mem_pool.ArenaAllocator,
        kind: EntityKind,
        mesh_id: u16,
        pos: wr.Vec3,
        scale: f32,
        radius: f32,
        billboard: BillboardMode,
        uv_rect: u32,
        color_light: u32,
    ) !void {
        try self.raw_items.append(arena.allocator(), .{
            .row0 = .{ scale, 0, 0, pos.x },
            .row1 = .{ 0, scale, 0, pos.y },
            .row2 = .{ 0, 0, scale, pos.z },
            .uv_rect = uv_rect,
            .color_light = color_light,
            .flags = GpuInstance.makeFlags(kind, billboard, false),
            .mesh_and_radius = GpuInstance.packMeshRadius(mesh_id, radius * scale),
        });
    }

    pub fn pushParticleAsInstance(
        self: *InstanceBatcher,
        arena: *mem_pool.ArenaAllocator,
        particle_mesh_id: u16,
        p: GpuParticle,
        size: f32,
    ) !void {
        try self.raw_items.append(arena.allocator(), .{
            .row0 = .{ size, 0, 0, p.pos[0] },
            .row1 = .{ 0, size, 0, p.pos[1] },
            .row2 = .{ 0, 0, size, p.pos[2] },
            .uv_rect = p.uv_rect,
            .color_light = p.rgba8,
            .flags = GpuInstance.makeFlags(.particle, .center, false),
            .mesh_and_radius = GpuInstance.packMeshRadius(particle_mesh_id, size * 0.866),
        });
    }

    /// Runs GPU/CPU culling -> sorts survivors by `mesh_id` -> uploads compact 64B instances ->
    /// emits single indirect multi-draw (Vulkan) or minimal instanced batches (GL 3.3 Core).
    pub fn flushToRhi(
        self: *InstanceBatcher,
        arena: *mem_pool.ArenaAllocator,
        dev: *rhi.RhiDevice,
        pipe: rhi.PipelineHandle,
        view_proj: wr.Mat4,
        hiz: *const wr.HiZPyramid,
        viewport_w: u32,
        viewport_h: u32,
        min_pixel_area: f32,
    ) !void {
        self.stats = .{ .submitted_total = self.raw_items.items.len };
        if (self.raw_items.items.len == 0) return;

        const alloc = arena.allocator();
        const frustum = wr.Frustum6.fromViewProj(view_proj);
        const vw_f: f32 = @floatFromInt(viewport_w);
        const vh_f: f32 = @floatFromInt(viewport_h);

        var survivors = std.ArrayListUnmanaged(GpuInstance){};
        try survivors.ensureTotalCapacity(alloc, self.raw_items.items.len);

        for (self.raw_items.items) |inst| {
            const c = inst.worldCenter();
            const r = inst.radius();
            const mn = wr.Vec3{ .x = c.x - r, .y = c.y - r, .z = c.z - r };
            const mx = wr.Vec3{ .x = c.x + r, .y = c.y + r, .z = c.z + r };

            if (!frustum.intersectsAabb(mn, mx)) {
                self.stats.frustum_culled += 1;
                continue;
            }
            const proj = wr.projectAabbNdc(&view_proj, mn, mx) orelse {
                self.stats.frustum_culled += 1;
                continue;
            };
            if (hiz.isOccluded(proj)) {
                self.stats.hiz_culled += 1;
                continue;
            }
            if (!proj.near_clip_overlap) {
                const area_px = (std.math.clamp(proj.max_x * 0.5 + 0.5, 0, 1) -
                    std.math.clamp(proj.min_x * 0.5 + 0.5, 0, 1)) * vw_f *
                    (std.math.clamp(proj.max_y * 0.5 + 0.5, 0, 1) -
                    std.math.clamp(proj.min_y * 0.5 + 0.5, 0, 1)) * vh_f;
                if (area_px < min_pixel_area) {
                    self.stats.subpixel_culled += 1;
                    continue;
                }
            }
            if (!self.mesh_table.contains(inst.meshId())) {
                // never upload or count instances whose mesh was not registered
                self.stats.unregistered_mesh_instances += 1;
                continue;
            }
            survivors.appendAssumeCapacity(inst);
        }

        if (survivors.items.len == 0) return;

        // Draw order: every opaque instance first (grouped by mesh to coalesce draws), then every
        // translucent one (particles, text) so alpha blending composes over the finished opaque pass.
        // Sorting by mesh alone interleaved translucent and opaque draws.
        std.mem.sort(GpuInstance, survivors.items, {}, struct {
            fn lessThan(_: void, a: GpuInstance, b: GpuInstance) bool {
                return runKey(&a) < runKey(&b);
            }
        }.lessThan);

        // Single contiguous upload into triple-buffered RHI staging ring (Vulkan SSBO / GL3.3 TBO)
        const payload_bytes = survivors.items.len * @sizeOf(GpuInstance);
        const stg = dev.allocTransientUpload(payload_bytes, 64) orelse return error.OutOfStagingMemory;
        @memcpy(stg.ptr[0..payload_bytes], std.mem.sliceAsBytes(survivors.items));

        // Count runs of registered meshes and emit RHI DrawIndexedIndirectCmd array
        var unique_meshes: usize = 0;
        var prev_key: ?u32 = null;
        for (survivors.items) |it| {
            const key = runKey(&it);
            if (prev_key == null or prev_key.? != key) {
                if (self.mesh_table.contains(it.meshId())) unique_meshes += 1;
                prev_key = key;
            }
        }
        if (unique_meshes == 0) {
            self.stats.unregistered_mesh_instances = survivors.items.len;
            return;
        }

        const cmds = dev.allocCmdSlice(rhi.DrawIndexedIndirectCmd, unique_meshes) orelse return error.OutOfMemory;
        var cmd_i: usize = 0;
        var drawn: usize = 0;
        var run_start: usize = 0;
        while (run_start < survivors.items.len) {
            const key = runKey(&survivors.items[run_start]);
            const mid = survivors.items[run_start].meshId();
            var run_end = run_start + 1;
            while (run_end < survivors.items.len and runKey(&survivors.items[run_end]) == key) : (run_end += 1) {}
            if (self.mesh_table.get(mid)) |proto| {
                cmds[cmd_i] = .{
                    .index_count = proto.index_count,
                    .instance_count = @intCast(run_end - run_start),
                    .first_index = proto.first_index,
                    .vertex_offset = proto.vertex_offset,
                    .first_instance = @intCast(run_start),
                };
                cmd_i += 1;
                drawn += run_end - run_start;
            } else {
                // never silently counted as drawn: a mesh that was not registered cannot be rendered
                self.stats.unregistered_mesh_instances += run_end - run_start;
            }
            run_start = run_end;
        }
        self.stats.draw_batches_emitted = cmd_i;
        self.stats.surviving_instances = drawn;

        try dev.record(.{ .bind_pipeline = pipe });
        try dev.record(.{ .bind_instance_buffer = .{
            .binding = 0,
            .buf = stg.buf,
            .offset_bytes = stg.offset,
            .size_bytes = payload_bytes,
            .texel_stride = 64,
        } });
        const ind_bytes = cmd_i * @sizeOf(rhi.DrawIndexedIndirectCmd);
        const ind_stg = dev.allocTransientUpload(ind_bytes, 16) orelse return error.OutOfStagingMemory;
        @memcpy(ind_stg.ptr[0..ind_bytes], std.mem.sliceAsBytes(cmds[0..cmd_i]));
        try dev.record(.{ .draw_indexed_indirect_range = .{
            .indirect_buf = ind_stg.buf,
            .offset_bytes = ind_stg.offset,
            .draw_count = @intCast(cmd_i),
            .stride = @sizeOf(rhi.DrawIndexedIndirectCmd),
            .fallback_cmds = cmds[0..cmd_i],
        } });
    }
};

test "GPU instancing batches Display entities, mobs, items, chests, signs & particles into minimal indirect draws" {
    var arena = mem_pool.ArenaAllocator.init(mem_pool.DEFAULT_CHUNK_SIZE, 2);
    defer arena.deinit();

    var dev = try rhi.RhiDevice.init(.headless, .max_fps, 2);
    defer dev.deinit();
    const pipe = dev.createPipeline();

    const cam = wr.Vec3{ .x = 0, .y = 10, .z = 20 };
    const view = wr.Mat4.lookAt(cam, .{ .x = 0, .y = 10, .z = 0 }, .{ .x = 0, .y = 1, .z = 0 });
    const proj = wr.Mat4.perspectiveReverseOrStandard(std.math.degreesToRadians(70.0), 16.0 / 9.0, 0.5, 512.0);
    const vp = wr.Mat4.mul(proj, view);

    var hiz = wr.HiZPyramid{};
    hiz.clear();
    // Place an opaque wall right at z=5..10 covering center screen
    hiz.rasterizeOpaqueBox(&vp, .{ .x = -16, .y = 0, .z = 5 }, .{ .x = 16, .y = 20, .z = 10 });
    hiz.buildMips();

    dev.beginFrame();
    var batcher = InstanceBatcher{};
    try batcher.beginFrame(&arena, 256);

    // 4 distinct non-block mesh prototypes: Display block/item, Mob/Player, Chest/Sign, Particle quad
    try batcher.registerMesh(&arena, .{ .mesh_id = 1, .index_count = 36, .first_index = 0, .vertex_offset = 0 });
    try batcher.registerMesh(&arena, .{ .mesh_id = 2, .index_count = 72, .first_index = 36, .vertex_offset = 24 });
    try batcher.registerMesh(&arena, .{ .mesh_id = 3, .index_count = 48, .first_index = 108, .vertex_offset = 60 });
    try batcher.registerMesh(&arena, .{ .mesh_id = 4, .index_count = 6, .first_index = 156, .vertex_offset = 90 });

    // Visible in front of wall (z=12..14)
    try batcher.pushTranslationEntity(&arena, .block_display, 1, .{ .x = -1, .y = 10, .z = 13 }, 1.0, 1.0, .fixed, 0x0101, 0xFFFFFFFF);
    try batcher.pushTranslationEntity(&arena, .item_display, 1, .{ .x = 0, .y = 10, .z = 13 }, 0.8, 0.8, .center, 0x0202, 0xFF00FFFF);
    try batcher.pushTranslationEntity(&arena, .text_display, 4, .{ .x = 1, .y = 11, .z = 13 }, 1.2, 1.0, .vertical, 0x0303, 0xFFFFFFFF);
    try batcher.pushTranslationEntity(&arena, .mob, 2, .{ .x = 2, .y = 10, .z = 13 }, 1.0, 1.2, .fixed, 0x0404, 0xFFFFFFFF);
    try batcher.pushTranslationEntity(&arena, .player, 2, .{ .x = -2, .y = 10, .z = 13 }, 1.0, 1.2, .fixed, 0x0505, 0xFFFFFFFF);
    try batcher.pushTranslationEntity(&arena, .chest, 3, .{ .x = 0, .y = 9, .z = 13 }, 1.0, 0.9, .fixed, 0x0606, 0xFFFFFFFF);
    try batcher.pushTranslationEntity(&arena, .sign, 3, .{ .x = 3, .y = 10, .z = 13 }, 1.0, 0.8, .fixed, 0x0707, 0xFFFFFFFF);
    try batcher.pushParticleAsInstance(&arena, 4, .{
        .pos = .{ 0, 10.5, 14 },
        .size_rot_packed = 0,
        .vel_packed = 0,
        .rgba8 = 0xFF8800FF,
        .uv_rect = 0x1010,
        .light_age = 0,
    }, 0.5);

    // Occluded behind wall at z=-10 -> must be Hi-Z culled!
    try batcher.pushTranslationEntity(&arena, .mob, 2, .{ .x = 0, .y = 10, .z = -10 }, 1.0, 1.0, .fixed, 0, 0xFFFFFFFF);
    // Behind camera at z=30 -> must be frustum culled!
    try batcher.pushTranslationEntity(&arena, .dropped_item, 1, .{ .x = 0, .y = 10, .z = 30 }, 1.0, 0.5, .center, 0, 0xFFFFFFFF);

    try batcher.flushToRhi(&arena, &dev, pipe, vp, &hiz, 1920, 1080, 2.0);
    dev.submitFrame();

    try std.testing.expectEqual(@as(usize, 10), batcher.stats.submitted_total);
    try std.testing.expectEqual(@as(usize, 8), batcher.stats.surviving_instances);
    try std.testing.expectEqual(@as(usize, 1), batcher.stats.hiz_culled);
    try std.testing.expectEqual(@as(usize, 1), batcher.stats.frustum_culled);
    // 8 diverse non-block objects merged into 4 instanced batches in 1 indirect range call!
    try std.testing.expectEqual(@as(usize, 4), batcher.stats.draw_batches_emitted);
    try std.testing.expectEqual(@as(u32, 1), dev.last_stats.draw_calls);
    try std.testing.expectEqual(@as(u64, 8), dev.last_stats.instances_submitted);
}

test "translucent instances draw after opaque ones and unregistered meshes are never counted as drawn" {
    var arena = mem_pool.ArenaAllocator.init(mem_pool.DEFAULT_CHUNK_SIZE, 2);
    defer arena.deinit();
    var dev = try rhi.RhiDevice.init(.headless, .max_fps, 2);
    defer dev.deinit();
    const pipe = dev.createPipeline();

    const cam = wr.Vec3{ .x = 0, .y = 10, .z = 20 };
    const view = wr.Mat4.lookAt(cam, .{ .x = 0, .y = 10, .z = 0 }, .{ .x = 0, .y = 1, .z = 0 });
    const proj = wr.Mat4.perspectiveReverseOrStandard(std.math.degreesToRadians(70.0), 16.0 / 9.0, 0.5, 512.0);
    const vp = wr.Mat4.mul(proj, view);
    var hiz = wr.HiZPyramid{};

    dev.beginFrame();
    var batcher = InstanceBatcher{};
    try batcher.beginFrame(&arena, 16);
    // mesh 1 is shared by an opaque block display and a translucent particle; mesh 9 is never registered
    try batcher.registerMesh(&arena, .{ .mesh_id = 1, .index_count = 36, .first_index = 0, .vertex_offset = 0 });
    try batcher.registerMesh(&arena, .{ .mesh_id = 2, .index_count = 6, .first_index = 36, .vertex_offset = 8 });
    try batcher.pushParticleAsInstance(&arena, 1, .{ .pos = .{ 0, 10, 5 }, .size_rot_packed = 0, .vel_packed = 0, .rgba8 = 0xFFFFFFFF, .uv_rect = 0, .light_age = 0 }, 0.5);
    try batcher.pushTranslationEntity(&arena, .block_display, 1, .{ .x = 0, .y = 10, .z = 5 }, 1.0, 1.0, .fixed, 0, 0xFFFFFFFF);
    try batcher.pushTranslationEntity(&arena, .mob, 2, .{ .x = 1, .y = 10, .z = 5 }, 1.0, 1.0, .fixed, 0, 0xFFFFFFFF);
    try batcher.pushTranslationEntity(&arena, .mob, 9, .{ .x = -1, .y = 10, .z = 5 }, 1.0, 1.0, .fixed, 0, 0xFFFFFFFF);
    try batcher.flushToRhi(&arena, &dev, pipe, vp, &hiz, 1920, 1080, 1.0);

    // the shared mesh 1 must NOT merge across the opaque/translucent boundary: 3 batches (1 opaque, 2 opaque, 1 translucent)
    try std.testing.expectEqual(@as(usize, 3), batcher.stats.draw_batches_emitted);
    try std.testing.expectEqual(@as(usize, 3), batcher.stats.surviving_instances);
    try std.testing.expectEqual(@as(usize, 1), batcher.stats.unregistered_mesh_instances);

    // find the indirect command list: the translucent particle run is the LAST command
    var found = false;
    for (dev.cmd_list.items) |c| switch (c) {
        .draw_indexed_indirect_range => |ir| {
            found = true;
            try std.testing.expectEqual(@as(usize, 3), ir.fallback_cmds.len);
            const last = ir.fallback_cmds[2];
            try std.testing.expectEqual(@as(u32, 36), last.index_count); // mesh 1 (particle quad stand-in)
            try std.testing.expectEqual(@as(u32, 1), last.instance_count);
            try std.testing.expectEqual(@as(u32, 2), last.first_instance);
        },
        else => {},
    };
    try std.testing.expect(found);
}
