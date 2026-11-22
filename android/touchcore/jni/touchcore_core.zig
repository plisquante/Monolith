//! TouchCore For Monolith — Native MONOLITH Client Core Substrate (aarch64-linux-android)
//!
//! Combines MONOLITH's GC-free `ArenaAllocator`, 3-epoch `SlabPool`, `WorkerPool`, `RhiDevice`,
//! 3-stage `world_render` culling (FOV Frustum + CaveCulling Portals + Hi-Z + Resolution LOD),
//! and GPU-driven `InstanceBatcher` into a clean C ABI consumed by `touchcore_android.c`.
const std = @import("std");
const mem_pool = @import("mem_pool.zig");
const thread_pool = @import("thread_pool.zig");
const rhi = @import("rhi.zig");
const wr = @import("world_render.zig");
const gi = @import("gpu_instancing.zig");

pub const Telemetry = extern struct {
    frame_epoch: u64,
    arena_bytes: u64,
    slab_committed_bytes: u64,
    slab_reclaimed_bytes: u64,
    visible_sections: u32,
    frustum_culled: u32,
    portal_culled: u32,
    hiz_culled: u32,
    draw_batches: u32,
    surviving_instances: u32,
    worker_threads: u32,
    rhi_active_backend: i32,
};

const PortalJob = struct {
    grids: [*]const [64]u64,
    masks: [*]wr.SectionPortalMask,
};

fn buildPortalJob(ctx: ?*anyopaque, idx: usize) void {
    const job: *const PortalJob = @ptrCast(@alignCast(ctx.?));
    job.masks[idx] = wr.computeSectionPortals(&job.grids[idx]);
}

const State = struct {
    mu: std.Thread.Mutex = .{},
    initialized: bool = false,
    pool: mem_pool.SlabPool = .{},
    frame_arena: mem_pool.ArenaAllocator = .{},
    workers: thread_pool.WorkerPool = .{},
    dev: ?rhi.RhiDevice = null,
    inst_pipe: rhi.PipelineHandle = .invalid,
    hiz: wr.HiZPyramid = .{},
    batcher: gi.InstanceBatcher = .{},
    frame_epoch: u64 = 0,
    last_telemetry: Telemetry = std.mem.zeroes(Telemetry),
    sections: [512]wr.SectionSnapshot = undefined,
    section_count: usize = 0,
};

var st: State = .{};

pub export fn tc_zig_init(backend_pref: i32, power_profile: i32, worker_count: i32) callconv(.c) i32 {
    st.mu.lock();
    defer st.mu.unlock();
    if (st.initialized) {
        return @intFromEnum(if (st.dev) |*d| d.caps.active_backend else rhi.BackendKind.headless);
    }
    st.pool = mem_pool.SlabPool{};
    st.frame_arena = mem_pool.ArenaAllocator.init(mem_pool.DEFAULT_CHUNK_SIZE, 4);
    st.workers = thread_pool.WorkerPool{};
    const req: usize = if (worker_count <= 0) 2 else @min(@as(usize, @intCast(worker_count)), 4);
    st.workers.start(req) catch {};

    const bk: rhi.BackendKind = switch (backend_pref) {
        1 => .vulkan,
        2 => .opengl33,
        3 => .headless,
        else => .auto,
    };
    const pw: rhi.PowerProfile = switch (power_profile) {
        0 => .max_fps,
        2 => .eco_battery,
        else => .balanced,
    };
    st.dev = rhi.RhiDevice.init(bk, pw, 4) catch null;
    if (st.dev) |*d| {
        st.inst_pipe = d.createPipeline();
    }

    // Precompute portal visibility masks on WorkerPool across an 8x4x8 chunk neighborhood.
    // Keep the 131 KB grid buffer off the pthread stack (Android Bionic stacks are small).
    const grids = std.heap.page_allocator.alloc([64]u64, 256) catch null;
    const masks = std.heap.page_allocator.alloc(wr.SectionPortalMask, 256) catch null;
    defer {
        if (grids) |g| std.heap.page_allocator.free(g);
        if (masks) |m| std.heap.page_allocator.free(m);
    }
    if (grids != null and masks != null) {
        const g_slice = grids.?;
        const m_slice = masks.?;
        for (g_slice, 0..) |*g, i| {
            if ((i % 7) == 5) {
                @memset(g, 0xFFFFFFFFFFFFFFFF); // solid occluder chunk
            } else {
                @memset(g, 0);
            }
        }
        var job = PortalJob{ .grids = g_slice.ptr, .masks = m_slice.ptr };
        var wg = thread_pool.WaitGroup{};
        st.workers.dispatchBatch(buildPortalJob, &job, 256, &wg);
        wg.wait();
    }

    var idx: usize = 0;
    var sx: i32 = -4;
    while (sx < 4) : (sx += 1) {
        var sz: i32 = -4;
        while (sz < 4) : (sz += 1) {
            var sy: i32 = 0;
            while (sy < 4) : (sy += 1) {
                if (idx >= st.sections.len) break;
                const solid = (idx % 7) == 5 and sy == 0;
                const portal_mask = if (!solid and masks != null) masks.?[idx & 255] else wr.SectionPortalMask{ .bits = 0 };
                st.sections[idx] = .{
                    .sx = sx,
                    .sy = sy,
                    .sz = sz,
                    .has_non_air = (sy == 0 or sy == 1),
                    .fully_opaque_occluder = solid,
                    .portals = portal_mask,
                };
                idx += 1;
            }
        }
    }
    st.section_count = idx;
    st.frame_epoch = 0;
    st.initialized = true;
    return @intFromEnum(if (st.dev) |*d| d.caps.active_backend else rhi.BackendKind.headless);
}

pub export fn tc_zig_tick_frame(
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
    out_tel: ?*Telemetry,
) callconv(.c) void {
    st.mu.lock();
    defer st.mu.unlock();
    if (!st.initialized) return;

    st.frame_epoch +%= 1;
    st.frame_arena.resetRetainingCapacity();
    st.hiz.clear();
    _ = st.pool.advanceAndReclaim();

    // Exercise 3-epoch zero-STW slab churn gently per frame to keep epoch accounting active
    if (st.pool.refillBatch(mem_pool.sizeClass(64), 4)) |slot_ptr| {
        st.pool.retireDeferred(slot_ptr, 64);
    }

    const vw = @max(viewport_w, 1);
    const vh = @max(viewport_h, 1);
    const aspect = @as(f32, @floatFromInt(vw)) / @as(f32, @floatFromInt(vh));
    const cam = wr.Vec3{ .x = cam_x, .y = cam_y, .z = cam_z };
    const tgt = wr.Vec3{ .x = look_x, .y = look_y, .z = look_z };
    const view = wr.Mat4.lookAt(cam, tgt, .{ .x = 0, .y = 1, .z = 0 });
    const proj = wr.Mat4.perspectiveReverseOrStandard(std.math.degreesToRadians(@max(fov_deg, 30.0)), aspect, 0.1, 256.0);
    const vp = wr.Mat4.mul(proj, view);

    var cstats = wr.CullStats{};
    const vis = wr.cullAndScheduleWorld(
        &st.frame_arena,
        st.sections[0..st.section_count],
        cam,
        vp,
        vw,
        vh,
        4.0,
        &st.hiz,
        &cstats,
    ) catch &[_]wr.VisibleSection{};
    _ = vis;

    var surviving: u32 = 0;
    var batches: u32 = 0;
    if (st.dev) |*d| {
        d.beginFrame();
        st.batcher = .{};
        if (st.batcher.beginFrame(&st.frame_arena, @max(entity_count, 32))) |_| {
            _ = st.batcher.registerMesh(&st.frame_arena, .{ .mesh_id = 1, .index_count = 36, .first_index = 0, .vertex_offset = 0 }) catch {};
            _ = st.batcher.registerMesh(&st.frame_arena, .{ .mesh_id = 2, .index_count = 12, .first_index = 36, .vertex_offset = 0 }) catch {};
            const n_ent = @min(entity_count, 256);
            var i: u32 = 0;
            while (i < n_ent) : (i += 1) {
                const fx: f32 = @floatFromInt(@as(i32, @intCast(i % 16)) - 8);
                const fz: f32 = @floatFromInt(@as(i32, @intCast(i / 16)) - 8);
                const kind: gi.EntityKind = if ((i & 3) == 0) .particle else .block_display;
                const mid: u16 = if (kind == .particle) 2 else 1;
                st.batcher.pushTranslationEntity(&st.frame_arena, kind, mid, .{ .x = fx * 2.5, .y = 3.0, .z = fz * 2.5 }, 0.8, 0.8, .fixed, i, 0xffffffff) catch break;
            }
            st.batcher.flushToRhi(&st.frame_arena, d, st.inst_pipe, vp, &st.hiz, vw, vh, 2.0) catch {};
            surviving = @intCast(st.batcher.stats.surviving_instances);
            batches = @intCast(st.batcher.stats.draw_batches_emitted);
        } else |_| {}
        d.submitFrame();
    }

    st.last_telemetry = .{
        .frame_epoch = st.frame_epoch,
        .arena_bytes = st.frame_arena.committed_bytes,
        .slab_committed_bytes = st.pool.committed_bytes.load(.monotonic),
        .slab_reclaimed_bytes = st.pool.reclaimed_bytes.load(.monotonic),
        .visible_sections = @intCast(cstats.visible_sections),
        .frustum_culled = @intCast(cstats.frustum_culled),
        .portal_culled = @intCast(cstats.portal_culled),
        .hiz_culled = @intCast(cstats.hiz_occluded),
        .draw_batches = batches,
        .surviving_instances = surviving,
        .worker_threads = @intCast(st.workers.num_workers),
        .rhi_active_backend = if (st.dev) |*d| @intFromEnum(d.caps.active_backend) else 3,
    };
    if (out_tel) |ptr| ptr.* = st.last_telemetry;
}

pub export fn tc_zig_shutdown() callconv(.c) void {
    st.mu.lock();
    defer st.mu.unlock();
    if (!st.initialized) return;
    if (st.dev) |*d| {
        d.deinit();
        st.dev = null;
    }
    st.workers.stop();
    st.frame_arena.deinit();
    st.pool.deinit();
    st.initialized = false;
}
