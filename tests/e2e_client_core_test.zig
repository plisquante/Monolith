//! Unified End-to-End Integration Test across all 4 Client Core subsystems:
//!   1. GC-Free Memory (`ArenaAllocator` + `SlabPool` epoch reclaim) + `WorkerPool` parallel portal build
//!   2. Zero-LWJGL RHI (`RhiDevice` probe, triple-buffered staging ring, command encoding, state dedup)
//!   3. Vanilla-Safe World Render Pipeline (FOV Frustum + CaveCulling Portals + Hi-Z Occlusion + LOD)
//!   4. GPU-Driven Unified Non-Block Instancing (`BlockDisplay`, `ItemDisplay`, `TextDisplay`, mobs,
//!      players, dropped items, chests, signs, particles) coalesced into RHI indirect draws.

const std = @import("std");
const mem_pool = @import("mem_pool");
const thread_pool = @import("thread_pool");
const rhi = @import("rhi");
const wr = @import("world_render");
const gi = @import("gpu_instancing");

const PortalBuildJob = struct {
    grids: []const [64]u64,
    out_masks: []wr.SectionPortalMask,
};

fn buildPortalTask(ctx: ?*anyopaque, idx: usize) void {
    const job: *PortalBuildJob = @ptrCast(@alignCast(ctx.?));
    job.out_masks[idx] = wr.computeSectionPortals(&job.grids[idx]);
}

test "Full 4-subsystem Client Core E2E: GC-free + WorkerPool + RHI + WorldRender + GPU Instancing" {
    // 1. GC-free memory substrate + parallel WorkerPool
    var pool = mem_pool.SlabPool{};
    defer pool.deinit();

    var frame_arena = mem_pool.ArenaAllocator.init(mem_pool.DEFAULT_CHUNK_SIZE, 4);
    defer frame_arena.deinit();

    var workers = thread_pool.WorkerPool{};
    try workers.start(2);
    defer workers.stop();

    // Immutable vanilla chunk section voxel grids (must never be modified by renderer/workers)
    var grids: [6][64]u64 = undefined;
    for (&grids) |*g| @memset(g, 0); // start open air
    @memset(&grids[1], 0xFFFFFFFFFFFFFFFF); // section 1 is a 100% solid stone wall occluder!

    var portal_masks: [6]wr.SectionPortalMask = undefined;
    var job = PortalBuildJob{ .grids = &grids, .out_masks = &portal_masks };
    var wg = thread_pool.WaitGroup{};
    workers.dispatchBatch(buildPortalTask, &job, 6, &wg);
    wg.wait();

    try std.testing.expectEqual(@as(u64, 0), portal_masks[1].bits);
    try std.testing.expect(portal_masks[0].canPass(.neg_z, .pos_z));

    // Verify vanilla block grid stayed bit-for-bit untouched
    try std.testing.expectEqual(@as(u64, 0xFFFFFFFFFFFFFFFF), grids[1][0]);

    const sections = [_]wr.SectionSnapshot{
        .{ .sx = 0, .sy = 0, .sz = 0, .has_non_air = false, .portals = portal_masks[0] }, // camera
        .{ .sx = 0, .sy = 0, .sz = -2, .has_non_air = true, .fully_opaque_occluder = true, .portals = portal_masks[1] }, // solid wall
        .{ .sx = 0, .sy = 0, .sz = -1, .has_non_air = true, .portals = portal_masks[2] }, // foreground chunk
        .{ .sx = 0, .sy = 0, .sz = -3, .has_non_air = true, .portals = portal_masks[3] }, // occluded behind wall
        .{ .sx = 0, .sy = 0, .sz = 3, .has_non_air = true, .portals = portal_masks[4] }, // behind camera (FOV culled)
        .{ .sx = 50, .sy = 0, .sz = -40, .has_non_air = true, .portals = portal_masks[5] }, // far outside frustum
    };

    // 2. Zero-LWJGL RHI init (auto probe tries Vulkan -> OpenGL3.3 -> Headless without crashing)
    var dev = try rhi.RhiDevice.init(.auto, .eco_battery, 4);
    defer dev.deinit();

    const world_pipe = dev.createPipeline();
    const inst_pipe = dev.createPipeline();
    const vb = dev.createBuffer(.{ .size = 128 * 1024, .usage = .{ .vertex = true } });
    const ib = dev.createBuffer(.{ .size = 32 * 1024, .usage = .{ .index = true } });

    const cam = wr.Vec3{ .x = 8, .y = 8, .z = 8 };
    const view = wr.Mat4.lookAt(cam, .{ .x = 8, .y = 8, .z = -8 }, .{ .x = 0, .y = 1, .z = 0 });
    const proj = wr.Mat4.perspectiveReverseOrStandard(std.math.degreesToRadians(70.0), 16.0 / 9.0, 0.5, 512.0);
    const vp = wr.Mat4.mul(proj, view);

    var hiz = wr.HiZPyramid{};
    var batcher = gi.InstanceBatcher{};

    // Run 6 consecutive simulated render frames validating Arena reset & 3-epoch SlabPool reclaim
    for (0..6) |frame_i| {
        frame_arena.resetRetainingCapacity();
        dev.beginFrame();
        _ = pool.advanceAndReclaim();

        // Simulate a transient SlabPool retirement every frame
        if (pool.refillBatch(mem_pool.sizeClass(64), 8)) |slot_ptr| {
            pool.retireDeferred(slot_ptr, 64);
        }

        // 3. World render culling
        var cull_stats = wr.CullStats{};
        const vis = try wr.cullAndScheduleWorld(
            &frame_arena,
            &sections,
            cam,
            vp,
            1920,
            1080,
            4.0,
            &hiz,
            &cull_stats,
        );
        try std.testing.expectEqual(@as(usize, 2), vis.len);
        try std.testing.expectEqual(@as(u32, 2), vis[0].snapshot_idx); // front-to-back order!
        try std.testing.expectEqual(@as(u32, 1), vis[1].snapshot_idx);

        try dev.record(.{ .begin_pass = .{
            .clear_r = 0.52,
            .clear_g = 0.72,
            .clear_b = 0.98,
            .clear_a = 1.0,
            .clear_depth = 1.0,
            .viewport_w = 1920,
            .viewport_h = 1080,
        } });
        try dev.record(.{ .bind_pipeline = world_pipe });
        try dev.record(.{ .bind_vertex_buffer = .{ .slot = 0, .buf = vb, .offset = 0 } });
        try dev.record(.{ .bind_index_buffer = .{ .buf = ib, .index_32 = true } });

        for (vis) |v| {
            const sec = &sections[v.snapshot_idx];
            try dev.record(.{ .draw_indexed_instanced = .{
                .index_count = sec.index_count,
                .instance_count = 1,
                .first_index = sec.first_index,
                .vertex_offset = sec.vertex_offset,
                .first_instance = 0,
            } });
        }

        // 4. Unified GPU-driven instancing for Display Entities, mobs, players, items, chests, signs & particles
        try batcher.beginFrame(&frame_arena, 64);
        try batcher.registerMesh(&frame_arena, .{ .mesh_id = 10, .index_count = 36, .first_index = 0, .vertex_offset = 0 });
        try batcher.registerMesh(&frame_arena, .{ .mesh_id = 20, .index_count = 72, .first_index = 36, .vertex_offset = 24 });
        try batcher.registerMesh(&frame_arena, .{ .mesh_id = 30, .index_count = 48, .first_index = 108, .vertex_offset = 60 });
        try batcher.registerMesh(&frame_arena, .{ .mesh_id = 40, .index_count = 6, .first_index = 156, .vertex_offset = 90 });

        // Visible entities between camera (z=8) and wall (z=-16)
        try batcher.pushTranslationEntity(&frame_arena, .block_display, 10, .{ .x = 8, .y = 8, .z = -2 }, 1.0, 1.0, .fixed, 1, 0xFFFFFFFF);
        try batcher.pushTranslationEntity(&frame_arena, .item_display, 10, .{ .x = 9, .y = 8, .z = -2 }, 0.8, 0.8, .center, 2, 0xFFFFFFFF);
        try batcher.pushTranslationEntity(&frame_arena, .text_display, 40, .{ .x = 7, .y = 9, .z = -2 }, 1.2, 1.0, .vertical, 3, 0xFFFFFFFF);
        try batcher.pushTranslationEntity(&frame_arena, .mob, 20, .{ .x = 6, .y = 8, .z = -4 }, 1.0, 1.2, .fixed, 4, 0xFFFFFFFF);
        try batcher.pushTranslationEntity(&frame_arena, .player, 20, .{ .x = 10, .y = 8, .z = -4 }, 1.0, 1.2, .fixed, 5, 0xFFFFFFFF);
        try batcher.pushTranslationEntity(&frame_arena, .dropped_item, 10, .{ .x = 8, .y = 7.5, .z = -1 }, 0.5, 0.5, .center, 6, 0xFFFFFFFF);
        try batcher.pushTranslationEntity(&frame_arena, .chest, 30, .{ .x = 8, .y = 7, .z = -5 }, 1.0, 0.9, .fixed, 7, 0xFFFFFFFF);
        try batcher.pushTranslationEntity(&frame_arena, .sign, 30, .{ .x = 9, .y = 8, .z = -5 }, 1.0, 0.8, .fixed, 8, 0xFFFFFFFF);
        try batcher.pushParticleAsInstance(&frame_arena, 40, .{
            .pos = .{ 8, 8.5, -1 },
            .size_rot_packed = 0,
            .vel_packed = 0,
            .rgba8 = 0xFFAA22FF,
            .uv_rect = 9,
            .light_age = 0,
        }, 0.4);
        // Occluded mob behind wall (section 1 covers x=[0..16], y=[0..16], z=[-32..-16])
        try batcher.pushTranslationEntity(&frame_arena, .mob, 20, .{ .x = 8, .y = 8, .z = -40 }, 1.0, 1.0, .fixed, 10, 0xFFFFFFFF);
        // Frustum-culled entity behind camera
        try batcher.pushTranslationEntity(&frame_arena, .player, 20, .{ .x = 8, .y = 8, .z = 25 }, 1.0, 1.0, .fixed, 11, 0xFFFFFFFF);

        try batcher.flushToRhi(&frame_arena, &dev, inst_pipe, vp, &hiz, 1920, 1080, 2.0);
        try dev.record(.end_pass);
        dev.submitFrame();

        try std.testing.expectEqual(@as(usize, 11), batcher.stats.submitted_total);
        try std.testing.expectEqual(@as(usize, 9), batcher.stats.surviving_instances);
        try std.testing.expectEqual(@as(usize, 1), batcher.stats.hiz_culled);
        try std.testing.expectEqual(@as(usize, 1), batcher.stats.frustum_culled);
        try std.testing.expectEqual(@as(usize, 4), batcher.stats.draw_batches_emitted);

        if (frame_i >= 3) {
            try std.testing.expect(pool.reclaimed_bytes.load(.monotonic) > 0);
        }
    }
}
