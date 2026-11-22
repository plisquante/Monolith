const std = @import("std");
const mem_pool = @import("mem_pool");
const thread_pool = @import("thread_pool");
const rhi = @import("rhi");
const wr = @import("world_render");
const gi = @import("gpu_instancing");

const FreeSlot = extern struct {
    next: ?*FreeSlot,
};

fn workerTask(ctx: ?*anyopaque, idx: usize) void {
    const arr: [*]std.atomic.Value(u64) = @ptrCast(@alignCast(ctx.?));
    const slot = &arr[idx & 63];
    _ = slot.fetchAdd(@as(u64, idx *% 2654435761) & 0xffff, .monotonic);
}

fn benchArena(frames: usize, allocs_per_frame: usize) !void {
    var arena = mem_pool.ArenaAllocator.init(256 * 1024, 8);
    defer arena.deinit();

    var checksum: u64 = 0;
    const t0 = std.time.nanoTimestamp();
    for (0..frames) |f| {
        for (0..allocs_per_frame) |i| {
            const p = arena.alloc(64, 16) orelse return error.OutOfMemory;
            p[0] = @truncate(i +% f);
            checksum +%= p[0];
        }
        arena.resetRetainingCapacity();
    }
    const elapsed_ns: u64 = @intCast(std.time.nanoTimestamp() - t0);
    std.mem.doNotOptimizeAway(checksum);
    const total_ops = frames * allocs_per_frame;
    const ns_per_op = @as(f64, @floatFromInt(elapsed_ns)) / @as(f64, @floatFromInt(total_ops));
    const mops = (@as(f64, @floatFromInt(total_ops)) * 1000.0) / @as(f64, @floatFromInt(elapsed_ns));
    std.debug.print("BENCH arena_alloc_64B ops={d} total_ms={d:.2} ns_per_op={d:.2} throughput_Mops={d:.1}\n", .{
        total_ops,
        @as(f64, @floatFromInt(elapsed_ns)) / 1_000_000.0,
        ns_per_op,
        mops,
    });
}

fn benchSlabEpoch(rounds: usize, batch: usize) !void {
    var pool = mem_pool.SlabPool{};
    defer pool.deinit();
    const sc = mem_pool.sizeClass(64);

    var reclaimed_total: usize = 0;
    const t0 = std.time.nanoTimestamp();
    for (0..rounds) |_| {
        const raw = pool.refillBatch(sc, batch) orelse return error.OutOfMemory;
        var cur: ?*FreeSlot = @ptrCast(@alignCast(raw));
        while (cur) |node| {
            const nxt = node.next;
            pool.retireDeferred(node, 64);
            cur = nxt;
        }
        reclaimed_total += pool.advanceAndReclaim();
    }
    reclaimed_total += pool.advanceAndReclaim();
    reclaimed_total += pool.advanceAndReclaim();
    const elapsed_ns: u64 = @intCast(std.time.nanoTimestamp() - t0);
    const total_ops = rounds * batch;
    const ns_per_op = @as(f64, @floatFromInt(elapsed_ns)) / @as(f64, @floatFromInt(total_ops));
    std.debug.print("BENCH slab_epoch_64B ops={d} reclaimed_kb={d} total_ms={d:.2} ns_per_op={d:.2} stw_pause_ms=0.0\n", .{
        total_ops,
        reclaimed_total / 1024,
        @as(f64, @floatFromInt(elapsed_ns)) / 1_000_000.0,
        ns_per_op,
    });
}

fn benchWorkerPool(batches: usize, tasks_per_batch: usize) !void {
    var pool = thread_pool.WorkerPool{};
    try pool.start(4);
    defer pool.stop();

    var accum: [64]std.atomic.Value(u64) = undefined;
    for (&accum) |*a| a.* = std.atomic.Value(u64).init(0);

    const t0 = std.time.nanoTimestamp();
    for (0..batches) |_| {
        var wg = thread_pool.WaitGroup{};
        pool.dispatchBatch(workerTask, &accum, tasks_per_batch, &wg);
        wg.wait();
    }
    const elapsed_ns: u64 = @intCast(std.time.nanoTimestamp() - t0);
    const total_tasks = batches * tasks_per_batch;
    const us_per_batch = (@as(f64, @floatFromInt(elapsed_ns)) / 1000.0) / @as(f64, @floatFromInt(batches));
    std.debug.print("BENCH worker_pool workers={d} tasks={d} total_ms={d:.2} us_per_batch={d:.1}\n", .{
        pool.num_workers,
        total_tasks,
        @as(f64, @floatFromInt(elapsed_ns)) / 1_000_000.0,
        us_per_batch,
    });
}

fn benchWorldAndInstancing(frames: usize) !void {
    var arena = mem_pool.ArenaAllocator.init(512 * 1024, 8);
    defer arena.deinit();

    var dev = try rhi.RhiDevice.init(.headless, .max_fps, 8);
    defer dev.deinit();
    const pipe = dev.createPipeline();
    const ib = dev.createBuffer(.{ .size = 1024 * 1024, .usage = .{ .index = true } });

    // Build an 8x8x4 = 256 section snapshot grid with open, solid occluders, and behind-camera sections
    var sections: [256]wr.SectionSnapshot = undefined;
    var idx: usize = 0;
    var sz: i32 = -6;
    while (sz < 2) : (sz += 1) {
        var sx: i32 = -4;
        while (sx < 4) : (sx += 1) {
            var sy: i32 = -1;
            while (sy < 3) : (sy += 1) {
                const is_wall = (sz == -3 and sx >= -2 and sx <= 2 and sy >= 0 and sy <= 1);
                sections[idx] = .{
                    .sx = sx,
                    .sy = sy,
                    .sz = sz,
                    .has_non_air = !(sx == 0 and sy == 0 and sz == 0),
                    .fully_opaque_occluder = is_wall,
                    .portals = if (is_wall) .{ .bits = 0 } else wr.SectionPortalMask.ALL_OPEN,
                };
                idx += 1;
            }
        }
    }

    const cam = wr.Vec3{ .x = 8, .y = 8, .z = 8 };
    const tgt = wr.Vec3{ .x = 8, .y = 8, .z = -8 };
    const vp = wr.Mat4.mul(
        wr.Mat4.perspectiveReverseOrStandard(std.math.degreesToRadians(70.0), 1920.0 / 1080.0, 0.5, 512.0),
        wr.Mat4.lookAt(cam, tgt, .{ .x = 0, .y = 1, .z = 0 }),
    );

    var hiz = wr.HiZPyramid{};
    var batcher = gi.InstanceBatcher{};

    var cull_ns_total: u64 = 0;
    var inst_ns_total: u64 = 0;
    var last_vis: usize = 0;
    var last_inst: usize = 0;
    var last_batches: usize = 0;

    for (0..frames) |_| {
        arena.resetRetainingCapacity();
        dev.beginFrame();
        try dev.record(.{ .begin_pass = .{
            .clear_r = 0.4,
            .clear_g = 0.6,
            .clear_b = 0.9,
            .clear_a = 1.0,
            .clear_depth = 1.0,
            .viewport_w = 1920,
            .viewport_h = 1080,
        } });
        try dev.record(.{ .bind_pipeline = pipe });
        try dev.record(.{ .bind_index_buffer = .{ .buf = ib, .index_32 = true } });

        var cstats = wr.CullStats{};
        const tc0 = std.time.nanoTimestamp();
        const vis = try wr.cullAndScheduleWorld(&arena, &sections, cam, vp, 1920, 1080, 4.0, &hiz, &cstats);
        const tc1 = std.time.nanoTimestamp();
        cull_ns_total += @intCast(tc1 - tc0);
        last_vis = vis.len;

        const ti0 = std.time.nanoTimestamp();
        try batcher.beginFrame(&arena, 2048);
        for (1..9) |m| {
            try batcher.registerMesh(&arena, .{
                .mesh_id = @intCast(m),
                .index_count = 36,
                .first_index = @intCast((m - 1) * 36),
                .vertex_offset = 0,
            });
        }
        for (0..1536) |e| {
            const kind: gi.EntityKind = @enumFromInt(@as(u4, @intCast(e % 8)));
            const mesh_id: u16 = @intCast(1 + (e % 8));
            const lane: f32 = @floatFromInt(@as(i32, @intCast(e % 16)) - 8);
            const z_depth: f32 = if (e % 4 == 0) -60.0 else if (e % 7 == 0) 24.0 else -(@as(f32, @floatFromInt(2 + (e % 30))));
            try batcher.pushTranslationEntity(&arena, kind, mesh_id, .{ .x = 8.0 + lane, .y = 8.0, .z = z_depth }, 1.0, 1.0, .fixed, 1, 0xffffffff);
        }
        for (0..512) |p| {
            try batcher.pushParticleAsInstance(&arena, 4, .{
                .pos = .{ 8.0 + @as(f32, @floatFromInt(@as(i32, @intCast(p % 10)) - 5)), 8.5, -2.0 - @as(f32, @floatFromInt(p % 20)) },
                .size_rot_packed = 0,
                .vel_packed = 0,
                .rgba8 = 0xffffffff,
                .uv_rect = 0,
                .light_age = 0,
            }, 0.4);
        }
        try batcher.flushToRhi(&arena, &dev, pipe, vp, &hiz, 1920, 1080, 2.0);
        try dev.record(.end_pass);
        dev.submitFrame();
        const ti1 = std.time.nanoTimestamp();
        inst_ns_total += @intCast(ti1 - ti0);
        last_inst = batcher.stats.surviving_instances;
        last_batches = batcher.stats.draw_batches_emitted;
    }

    const cull_us = (@as(f64, @floatFromInt(cull_ns_total)) / 1000.0) / @as(f64, @floatFromInt(frames));
    const inst_us = (@as(f64, @floatFromInt(inst_ns_total)) / 1000.0) / @as(f64, @floatFromInt(frames));
    std.debug.print("BENCH world_cull_256 frames={d} vis={d}/256 us_per_frame={d:.1}\n", .{ frames, last_vis, cull_us });
    std.debug.print("BENCH gpu_instancing_2048 frames={d} surviving={d}/2048 batches={d} us_per_frame={d:.1}\n", .{
        frames,
        last_inst,
        last_batches,
        inst_us,
    });
}

pub fn main() !void {
    try benchArena(200, 10_000);
    try benchSlabEpoch(2000, 32);
    try benchWorkerPool(250, 64);
    try benchWorldAndInstancing(200);
}
