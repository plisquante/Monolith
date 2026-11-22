//! Zero-LWJGL RHI + World Render + GPU Instancing Java/Native Bridge.
//! Exposes complete native APIs for Java clients & Minecraft hooks:
//!   - Probe & initialize MONOLITH RHI (Vulkan 1.1+ / OpenGL 3.3 Core / Headless)
//!   - Bracket frames (`beginFrame` / `endFrame`) & explicit Java arena scopes (`pushArenaScope` / `popArenaScope`)
//!   - Vanilla-safe `SectionSnapshot` submission + 3-stage cull (`cullWorld`: FOV + Portal + Hi-Z + Resolution LOD)
//!   - Unified GPU-driven non-block instancing (`registerMesh`, `pushInstance`, `flushInstances`)

const std = @import("std");
const rt = @import("rt.zig");
const P = @import("classes.zig");
const mem_pool = @import("mem_pool.zig");
const rhi = @import("rhi.zig");
const wr = @import("world_render.zig");
const gi = @import("gpu_instancing.zig");
const gl_shim = @import("gl_shim.zig");
const al_shim = @import("al_shim.zig");

const E = rt.E;

pub const MAX_SECTION_SNAPSHOTS: usize = 65536;
pub const MAX_MESH_ID: i32 = 65535;

pub const ClientRenderState = struct {
    lock: std.Thread.Mutex = .{},
    dev: ?rhi.RhiDevice = null,
    preferred_backend: rhi.BackendKind = .auto,
    world_pipe: rhi.PipelineHandle = .invalid,
    instanced_pipe: rhi.PipelineHandle = .invalid,
    hiz: wr.HiZPyramid = .{},
    batcher: gi.InstanceBatcher = .{},
    /// Frame memory owned by the bridge itself (NOT a thread's scratch arena): Java may call the
    /// bridge from several threads, and a thread's `popArenaScope` must never rewind memory the
    /// batcher / culler still references. Reset once per `beginFrame`, guarded by `lock`.
    frame_arena: mem_pool.ArenaAllocator = .{},
    frame_open: bool = false,
    sections: std.ArrayListUnmanaged(wr.SectionSnapshot) = .{},
    last_visible: []wr.VisibleSection = &.{},
    last_cull: wr.CullStats = .{},
    last_inst: gi.InstancingStats = .{},
    dropped_sections: u64 = 0,
    viewport_w: u32 = 1920,
    viewport_h: u32 = 1080,
};

pub const rhi_st: *ClientRenderState = rt.Shared(ClientRenderState, "rhi", 64 * 1024).ptr;

pub fn initClientRhi(preferred: rhi.BackendKind, power: rhi.PowerProfile) !rhi.BackendKind {
    rhi_st.lock.lock();
    defer rhi_st.lock.unlock();
    if (rhi_st.dev == null) {
        var dev = try rhi.RhiDevice.init(preferred, power, 4);
        errdefer dev.deinit();
        // In a sharded build this struct lives in zero-filled C storage: Zig field defaults are NOT
        // applied there, so every non-zero default is set explicitly.
        rhi_st.frame_arena = mem_pool.ArenaAllocator.init(mem_pool.DEFAULT_CHUNK_SIZE, 2);
        rhi_st.frame_open = false;
        rhi_st.preferred_backend = preferred;
        rhi_st.last_visible = &.{};
        rhi_st.viewport_w = 1920;
        rhi_st.viewport_h = 1080;
        rhi_st.batcher = .{};
        rhi_st.dev = dev;
        rhi_st.world_pipe = rhi_st.dev.?.createPipeline();
        rhi_st.instanced_pipe = rhi_st.dev.?.createPipeline();
    }
    return rhi_st.dev.?.caps.active_backend;
}

pub fn shutdownClientRhi() void {
    rhi_st.lock.lock();
    defer rhi_st.lock.unlock();
    if (rhi_st.dev) |*d| {
        d.deinit();
        rhi_st.dev = null;
    }
    // the batcher's buffers live in frame_arena: drop both together
    rhi_st.batcher = .{};
    rhi_st.last_visible = &.{};
    rhi_st.frame_arena.deinit();
    rhi_st.frame_open = false;
    rhi_st.sections.deinit(std.heap.c_allocator);
    rhi_st.sections = .{};
    rhi_st.dropped_sections = 0;
    // the client renderer is gone: collect what it held
    rt.gcNow();
}

fn needFrame() E!*rhi.RhiDevice {
    const d = &(rhi_st.dev orelse return rt.throwKind(.IllegalArgument, "NativeRhi.init() has not been called"));
    if (!rhi_st.frame_open) return rt.throwKind(.IllegalArgument, "NativeRhi: no open frame (call beginFrame() first)");
    return d;
}

fn checkFinite(vals: []const f32) E!void {
    for (vals) |v| {
        if (!std.math.isFinite(v)) return rt.throwKind(.IllegalArgument, "NativeRhi: non-finite camera/instance parameter");
    }
}

fn buildCameraViewProj(cam_x: f32, cam_y: f32, cam_z: f32, look_x: f32, look_y: f32, look_z: f32, fov_deg: f32, vw: u32, vh: u32) wr.Mat4 {
    const aspect = @as(f32, @floatFromInt(@max(vw, 1))) / @as(f32, @floatFromInt(@max(vh, 1)));
    const cam = wr.Vec3{ .x = cam_x, .y = cam_y, .z = cam_z };
    const tgt = wr.Vec3{ .x = look_x, .y = look_y, .z = look_z };
    const view = wr.Mat4.lookAt(cam, tgt, .{ .x = 0, .y = 1, .z = 0 });
    const proj = wr.Mat4.perspectiveReverseOrStandard(std.math.degreesToRadians(@max(fov_deg, 10.0)), aspect, 0.5, 512.0);
    return wr.Mat4.mul(proj, view);
}

// ------------------------------------------------------------------ Shared RHI helpers (non-exported to avoid duplicate symbols across shards)
pub fn mono_rhi_init(backend_code: i32, power_code: i32) i32 {
    const bk: rhi.BackendKind = switch (backend_code) {
        1 => .vulkan,
        2 => .opengl33,
        3 => .headless,
        else => .auto,
    };
    const pw: rhi.PowerProfile = switch (power_code) {
        0 => .max_fps,
        2 => .eco_battery,
        else => .balanced,
    };
    const active = initClientRhi(bk, pw) catch return -1;
    return @intFromEnum(active);
}

/// Frame bracket shared by the C ABI and the Java natives. Returns the new frame epoch.
fn beginFrameImpl(vw: u32, vh: u32) E!u64 {
    const ep = rt.beginFrame();
    rhi_st.lock.lock();
    defer rhi_st.lock.unlock();
    const d = &(rhi_st.dev orelse return rt.throwKind(.IllegalArgument, "NativeRhi.init() has not been called"));
    if (rhi_st.preferred_backend != .headless and d.caps.active_backend == .headless) {
        _ = d.tryAttachGlContext();
    }
    rhi_st.viewport_w = @max(vw, 1);
    rhi_st.viewport_h = @max(vh, 1);
    d.beginFrame();
    // everything the previous frame's batcher/culler held lives in frame_arena: reset it first
    rhi_st.batcher = .{};
    rhi_st.last_visible = &.{};
    rhi_st.frame_arena.resetRetainingCapacity();
    // Hi-Z holds the previous cull's occluders (another camera): stale depth would wrongly hide
    // instances if this frame never calls cullWorld before flushInstances.
    rhi_st.hiz.clear();
    rhi_st.frame_open = false;
    rhi_st.batcher.beginFrame(&rhi_st.frame_arena, 256) catch return rt.throwKind(.OutOfMemory, "NativeRhi frame arena");
    rhi_st.frame_open = true;
    return ep;
}

pub fn mono_rhi_begin_frame(vw: u32, vh: u32) u64 {
    return beginFrameImpl(vw, vh) catch 0;
}

pub fn mono_rhi_end_frame() void {
    rhi_st.lock.lock();
    if (rhi_st.dev) |*d| {
        if (rhi_st.frame_open) d.submitFrame();
    }
    rhi_st.frame_open = false;
    rhi_st.lock.unlock();
    rt.endFrame();
}

pub fn mono_rhi_active_backend() i32 {
    rhi_st.lock.lock();
    defer rhi_st.lock.unlock();
    const d = &(rhi_st.dev orelse return -1);
    return @intFromEnum(d.caps.active_backend);
}

// ------------------------------------------------------------------ Java NativeRhi entry points
pub fn J_mono_rhi_NativeRhi__init0(backend: i32, power: i32) E!i32 {
    const r = mono_rhi_init(backend, power);
    if (r < 0) return rt.throwKind(.IllegalArgument, "NativeRhi.init: requested graphics backend is unavailable");
    return r;
}

var first_frame_marked: bool = false;
var first_swap_marked: bool = false;

pub fn J_mono_rhi_NativeRhi__beginFrame0(w: i32, h: i32) E!i64 {
    if (!first_frame_marked) {
        first_frame_marked = true;
        rt.startupMark("first frame begins");
    }
    return @intCast(try beginFrameImpl(@intCast(@max(w, 1)), @intCast(@max(h, 1))));
}

pub fn J_mono_rhi_NativeRhi__endFrame0() E!void {
    mono_rhi_end_frame();
}

threadlocal var java_scope_stack: [16]mem_pool.ArenaMark = undefined;
threadlocal var java_scope_depth: usize = 0;

pub fn J_mono_rhi_NativeRhi__pushArenaScope0() E!void {
    if (java_scope_depth >= java_scope_stack.len)
        return rt.throwKind(.IllegalArgument, "NativeRhi arena scope stack overflow (max 16 nested scopes)");
    java_scope_stack[java_scope_depth] = rt.pushArenaScope();
    java_scope_depth += 1;
}

pub fn J_mono_rhi_NativeRhi__popArenaScope0() E!void {
    if (java_scope_depth == 0)
        return rt.throwKind(.IllegalArgument, "NativeRhi popArenaScope() without a matching pushArenaScope()");
    java_scope_depth -= 1;
    rt.popArenaScope(java_scope_stack[java_scope_depth]);
}

pub fn J_mono_rhi_NativeRhi__clearSections0() E!void {
    rhi_st.lock.lock();
    defer rhi_st.lock.unlock();
    rhi_st.sections.clearRetainingCapacity();
}

pub fn J_mono_rhi_NativeRhi__submitSectionSnapshot0(sx: i32, sy: i32, sz: i32, has_geometry: i32, opaque_occluder: i32, portal_bits: i64) E!void {
    return J_mono_rhi_NativeRhi__submitSectionSnapshotEx0(sx, sy, sz, has_geometry, opaque_occluder, portal_bits, 36, 0, 0);
}

pub fn J_mono_rhi_NativeRhi__submitSectionSnapshotEx0(
    sx: i32,
    sy: i32,
    sz: i32,
    has_geometry: i32,
    opaque_occluder: i32,
    portal_bits: i64,
    index_count: i32,
    first_index: i32,
    vertex_offset: i32,
) E!void {
    if (index_count < 0 or first_index < 0)
        return rt.throwKind(.IllegalArgument, "NativeRhi.submitSectionSnapshotEx: negative indexCount/firstIndex");
    rhi_st.lock.lock();
    defer rhi_st.lock.unlock();
    if (rhi_st.sections.items.len >= MAX_SECTION_SNAPSHOTS) {
        // Hard cap: an unbounded sections list is the one unbounded growth path in the bridge
        // (clearSections is the documented per-frame reset). Counted, never silent.
        rhi_st.dropped_sections += 1;
        return;
    }
    rhi_st.sections.append(std.heap.c_allocator, .{
        .sx = sx,
        .sy = sy,
        .sz = sz,
        .has_non_air = has_geometry != 0,
        .fully_opaque_occluder = opaque_occluder != 0,
        .portals = .{ .bits = @bitCast(portal_bits) },
        .index_count = @intCast(index_count),
        .first_index = @intCast(first_index),
        .vertex_offset = vertex_offset,
    }) catch return rt.throwKind(.OutOfMemory, "NativeRhi section snapshot list");
}

pub fn J_mono_rhi_NativeRhi__cullWorld0(cam_x: f32, cam_y: f32, cam_z: f32, look_x: f32, look_y: f32, look_z: f32, fov_deg: f32, min_pixel_area: f32) E!i32 {
    try checkFinite(&.{ cam_x, cam_y, cam_z, look_x, look_y, look_z, fov_deg, min_pixel_area });
    rhi_st.lock.lock();
    defer rhi_st.lock.unlock();
    const d = try needFrame();
    const vp = buildCameraViewProj(cam_x, cam_y, cam_z, look_x, look_y, look_z, fov_deg, rhi_st.viewport_w, rhi_st.viewport_h);
    const vis = wr.cullAndScheduleWorld(
        &rhi_st.frame_arena,
        rhi_st.sections.items,
        .{ .x = cam_x, .y = cam_y, .z = cam_z },
        vp,
        rhi_st.viewport_w,
        rhi_st.viewport_h,
        min_pixel_area,
        &rhi_st.hiz,
        &rhi_st.last_cull,
    ) catch return rt.throwKind(.OutOfMemory, "NativeRhi world culling");

    rhi_st.last_visible = vis;
    d.record(.{ .bind_pipeline = rhi_st.world_pipe }) catch return rt.throwKind(.OutOfMemory, "NativeRhi command list");
    for (vis) |v| {
        const s = &rhi_st.sections.items[v.snapshot_idx];
        d.record(.{ .draw_indexed_instanced = .{
            .index_count = s.index_count,
            .instance_count = 1,
            .first_index = s.first_index,
            .vertex_offset = s.vertex_offset,
            .first_instance = 0,
        } }) catch return rt.throwKind(.OutOfMemory, "NativeRhi command list");
    }
    return @intCast(vis.len);
}

pub fn J_mono_rhi_NativeRhi__visibleSectionCount0() E!i32 {
    rhi_st.lock.lock();
    defer rhi_st.lock.unlock();
    return @intCast(rhi_st.last_visible.len);
}

pub fn J_mono_rhi_NativeRhi__visibleSectionField0(index: i32, field: i32) E!i32 {
    rhi_st.lock.lock();
    defer rhi_st.lock.unlock();
    if (index < 0 or @as(usize, @intCast(index)) >= rhi_st.last_visible.len)
        return rt.throwKind(.ArrayIndex, "NativeRhi.visibleSectionField index out of bounds");
    const v = rhi_st.last_visible[@intCast(index)];
    const s = &rhi_st.sections.items[v.snapshot_idx];
    return switch (field) {
        0 => s.sx,
        1 => s.sy,
        2 => s.sz,
        3 => @intFromEnum(v.lod),
        else => rt.throwKind(.IllegalArgument, "NativeRhi.visibleSectionField: field must be 0..3"),
    };
}

pub fn J_mono_rhi_NativeRhi__glViewport0(x: i32, y: i32, w: i32, h: i32) E!void {
    rhi_st.lock.lock();
    defer rhi_st.lock.unlock();
    rhi_st.viewport_w = @intCast(@max(w, 1));
    rhi_st.viewport_h = @intCast(@max(h, 1));
    if (rhi_st.dev) |*d| {
        if (d.gl_ctx) |*g| {
            if (g.procs.glViewport) |vp| {
                vp(x, y, @max(w, 1), @max(h, 1));
                return;
            }
        }
    }
    gl_shim.viewport(x, y, w, h);
}

pub fn J_mono_rhi_NativeRhi__glClearColor0(r: f32, g: f32, b: f32, a: f32) E!void {
    rhi_st.lock.lock();
    defer rhi_st.lock.unlock();
    if (rhi_st.dev) |*d| {
        if (d.gl_ctx) |*gl| {
            if (gl.procs.glClearColor) |cc| {
                cc(r, g, b, a);
                return;
            }
        }
    }
    gl_shim.clearColor(r, g, b, a);
}

pub fn J_mono_rhi_NativeRhi__glClear0(mask: i32) E!void {
    rhi_st.lock.lock();
    defer rhi_st.lock.unlock();
    if (rhi_st.dev) |*d| {
        if (d.gl_ctx) |*gl| {
            if (gl.procs.glClear) |cl| {
                cl(@bitCast(mask));
                return;
            }
        }
    }
    gl_shim.clear(@bitCast(mask));
}

pub fn J_mono_rhi_NativeRhi__registerMesh0(mesh_id: i32, index_count: i32, first_index: i32, vertex_offset: i32) E!void {
    if (mesh_id < 0 or mesh_id > MAX_MESH_ID)
        return rt.throwKind(.IllegalArgument, "NativeRhi.registerMesh: meshId must be in 0..65535");
    if (index_count < 0 or first_index < 0)
        return rt.throwKind(.IllegalArgument, "NativeRhi.registerMesh: negative indexCount/firstIndex");
    rhi_st.lock.lock();
    defer rhi_st.lock.unlock();
    _ = try needFrame();
    rhi_st.batcher.registerMesh(&rhi_st.frame_arena, .{
        .mesh_id = @intCast(mesh_id),
        .index_count = @intCast(index_count),
        .first_index = @intCast(first_index),
        .vertex_offset = vertex_offset,
    }) catch return rt.throwKind(.OutOfMemory, "NativeRhi mesh table");
}

pub fn J_mono_rhi_NativeRhi__pushInstance0(kind: i32, mesh_id: i32, x: f32, y: f32, z: f32, scale: f32, radius: f32, billboard: i32, uv_rect: i32, color_light: i32) E!void {
    if (kind < 0 or kind > 8) return rt.throwKind(.IllegalArgument, "NativeRhi.pushInstance: unknown entity kind");
    if (billboard < 0 or billboard > 3) return rt.throwKind(.IllegalArgument, "NativeRhi.pushInstance: unknown billboard mode");
    if (mesh_id < 0 or mesh_id > MAX_MESH_ID) return rt.throwKind(.IllegalArgument, "NativeRhi.pushInstance: meshId must be in 0..65535");
    try checkFinite(&.{ x, y, z, scale, radius });
    if (radius < 0) return rt.throwKind(.IllegalArgument, "NativeRhi.pushInstance: negative radius");
    rhi_st.lock.lock();
    defer rhi_st.lock.unlock();
    _ = try needFrame();
    const k: gi.EntityKind = @enumFromInt(@as(u4, @intCast(kind)));
    const bb: gi.BillboardMode = @enumFromInt(@as(u2, @intCast(billboard)));
    rhi_st.batcher.pushTranslationEntity(
        &rhi_st.frame_arena,
        k,
        @intCast(mesh_id),
        .{ .x = x, .y = y, .z = z },
        scale,
        radius,
        bb,
        @bitCast(uv_rect),
        @bitCast(color_light),
    ) catch return rt.throwKind(.OutOfMemory, "NativeRhi instance buffer");
}

pub fn J_mono_rhi_NativeRhi__flushInstances0(cam_x: f32, cam_y: f32, cam_z: f32, look_x: f32, look_y: f32, look_z: f32, fov_deg: f32, min_pixel_area: f32) E!i32 {
    try checkFinite(&.{ cam_x, cam_y, cam_z, look_x, look_y, look_z, fov_deg, min_pixel_area });
    rhi_st.lock.lock();
    defer rhi_st.lock.unlock();
    const d = try needFrame();
    const vp = buildCameraViewProj(cam_x, cam_y, cam_z, look_x, look_y, look_z, fov_deg, rhi_st.viewport_w, rhi_st.viewport_h);
    rhi_st.batcher.flushToRhi(
        &rhi_st.frame_arena,
        d,
        rhi_st.instanced_pipe,
        vp,
        &rhi_st.hiz,
        rhi_st.viewport_w,
        rhi_st.viewport_h,
        min_pixel_area,
    ) catch return rt.throwKind(.OutOfMemory, "NativeRhi instance flush");
    rhi_st.last_inst = rhi_st.batcher.stats;
    return @intCast(rhi_st.last_inst.surviving_instances);
}

pub fn J_mono_rhi_NativeRhi__lastDrawBatches0() E!i32 {
    rhi_st.lock.lock();
    defer rhi_st.lock.unlock();
    return @intCast(rhi_st.last_inst.draw_batches_emitted);
}

pub fn J_mono_rhi_NativeRhi__arenaBytes0() E!i64 {
    // calling thread's transient arena; the render thread is the one that matters for frame scopes
    return @intCast(rt.cur.scratch_arena.committed_bytes);
}
pub fn J_mono_rhi_NativeRhi__slabCommittedBytes0() E!i64 {
    return @intCast(rt.st.slab_pool.committed_bytes.load(.monotonic));
}
pub fn J_mono_rhi_NativeRhi__slabReclaimedBytes0() E!i64 {
    return @intCast(rt.st.slab_pool.reclaimed_bytes.load(.monotonic));
}
pub fn J_mono_rhi_NativeRhi__frameEpoch0() E!i64 {
    return @intCast(rt.st.frame_epoch.load(.monotonic));
}
pub fn J_mono_rhi_NativeRhi__threadCount0() E!i64 {
    return @intCast(rt.st.thread_count.load(.monotonic));
}
pub fn J_mono_rhi_NativeRhi__droppedSections0() E!i64 {
    rhi_st.lock.lock();
    defer rhi_st.lock.unlock();
    return @intCast(rhi_st.dropped_sections);
}

// ------------------------------------------------------------------ Android Surface, Input & Direct GL->GLES3 Dispatch
pub fn J_mono_rhi_NativeRhi__hasOsSurface0() E!i32 {
    return if (gl_shim.hasOsSurface()) 1 else 0;
}

pub fn J_mono_rhi_NativeRhi__attachOsSurface0() E!i32 {
    var w: i32 = 1280;
    var h: i32 = 720;
    if (!gl_shim.attachOsSurface(&w, &h)) return 0;
    rhi_st.lock.lock();
    rhi_st.viewport_w = @intCast(@max(w, 1));
    rhi_st.viewport_h = @intCast(@max(h, 1));
    rhi_st.lock.unlock();
    return 1;
}

pub fn J_mono_rhi_NativeRhi__swapOsSurface0() E!void {
    if (!first_swap_marked) {
        first_swap_marked = true;
        rt.startupMark("first frame presented");
    }
    gl_shim.swapOsSurface();
}

pub fn J_mono_rhi_NativeRhi__osSurfaceWidth0() E!i32 {
    return gl_shim.osSurfaceSize().w;
}

pub fn J_mono_rhi_NativeRhi__osSurfaceHeight0() E!i32 {
    return gl_shim.osSurfaceSize().h;
}

pub fn J_mono_rhi_NativeRhi__pollText0(out_ref: rt.Ref) E!i32 {
    const ao = out_ref orelse return 0;
    const arr = rt.asArray(ao);
    const dst = rt.arrayData(i32, arr);
    return gl_shim.pollText(dst[0..@intCast(arr.length)]);
}

pub fn J_mono_rhi_NativeRhi__setMouseGrabbed0(grabbed: i32) E!void {
    gl_shim.setMouseGrabbed(grabbed != 0);
}

pub fn J_mono_rhi_NativeRhi__pollOsInput0(btns_ref: rt.Ref, axes_ref: rt.Ref) E!void {
    const snap = gl_shim.pollSnapshot();
    if (btns_ref) |bo| {
        const arr = rt.asArray(bo);
        const dst = rt.arrayData(i32, arr);
        if (arr.length >= 22) {
            dst[0] = snap.key_w;
            dst[1] = snap.key_a;
            dst[2] = snap.key_s;
            dst[3] = snap.key_d;
            dst[4] = snap.key_jump;
            dst[5] = snap.key_sneak;
            dst[6] = snap.key_sprint;
            dst[7] = snap.key_lmb;
            dst[8] = snap.key_rmb;
            dst[9] = snap.key_esc;
            dst[10] = snap.key_e;
            dst[11] = snap.key_q;
            dst[12] = snap.key_f;
            dst[13] = snap.key_t;
            dst[14] = snap.key_f3;
            dst[15] = snap.key_f5;
            dst[16] = snap.key_tab;
            dst[17] = snap.third_person;
            dst[18] = snap.cursor_visible;
            dst[19] = snap.hotbar_slot;
            dst[20] = snap.width;
            dst[21] = snap.height;
        }
    }
    if (axes_ref) |ao| {
        const arr = rt.asArray(ao);
        const dst = rt.arrayData(f32, arr);
        if (arr.length >= 4) {
            dst[0] = snap.look_dx;
            dst[1] = snap.look_dy;
            dst[2] = snap.cursor_x;
            dst[3] = snap.cursor_y;
        }
    }
}

fn jstrToUtf8Z(s_ref: rt.Ref) E![:0]u8 {
    const u16s = rt.hooks.string_chars(try rt.nn(s_ref));
    const utf8 = std.unicode.utf16LeToUtf8Alloc(rt.gpa, u16s) catch return rt.throwKind(.OutOfMemory, null);
    defer rt.gpa.free(utf8);
    return rt.gpa.dupeZ(u8, utf8) catch return rt.throwKind(.OutOfMemory, null);
}

pub fn J_mono_rhi_NativeRhi__glEnable0(cap: i32) E!void { gl_shim.enable(@bitCast(cap)); }
pub fn J_mono_rhi_NativeRhi__glDisable0(cap: i32) E!void { gl_shim.disable(@bitCast(cap)); }
pub fn J_mono_rhi_NativeRhi__glBlendFunc0(s: i32, d: i32) E!void { gl_shim.blendFunc(@bitCast(s), @bitCast(d)); }
pub fn J_mono_rhi_NativeRhi__glDepthFunc0(func: i32) E!void { gl_shim.depthFunc(@bitCast(func)); }
pub fn J_mono_rhi_NativeRhi__glDepthMask0(flag: i32) E!void { gl_shim.depthMask(if (flag != 0) 1 else 0); }
pub fn J_mono_rhi_NativeRhi__glColorMask0(r: i32, g: i32, b: i32, a: i32) E!void {
    gl_shim.colorMask(if (r != 0) 1 else 0, if (g != 0) 1 else 0, if (b != 0) 1 else 0, if (a != 0) 1 else 0);
}
pub fn J_mono_rhi_NativeRhi__glCullFace0(mode: i32) E!void { gl_shim.cullFace(@bitCast(mode)); }
pub fn J_mono_rhi_NativeRhi__glFrontFace0(mode: i32) E!void { gl_shim.frontFace(@bitCast(mode)); }
pub fn J_mono_rhi_NativeRhi__glScissor0(x: i32, y: i32, w: i32, h: i32) E!void { gl_shim.scissor(x, y, w, h); }
pub fn J_mono_rhi_NativeRhi__glClearDepth0(d: f64) E!void { gl_shim.clearDepth(d); }
pub fn J_mono_rhi_NativeRhi__glDepthRange0(n: f64, fval: f64) E!void { gl_shim.depthRange(n, fval); }
pub fn J_mono_rhi_NativeRhi__glPolygonMode0(_: i32, _: i32) E!void {} // Core GLES3 has no glPolygonMode
pub fn J_mono_rhi_NativeRhi__glGetError0() E!i32 { return @bitCast(gl_shim.getError()); }
pub fn J_mono_rhi_NativeRhi__glGetInteger0(pname: i32) E!i32 { return gl_shim.getInteger(@bitCast(pname)); }
pub fn J_mono_rhi_NativeRhi__glGetString0(name: i32) E!rt.Ref {
    const s = gl_shim.getString(@bitCast(name));
    if (s.len == 0) return null;
    return rt.newStringUtf8(s);
}

pub fn J_mono_rhi_NativeRhi__glGenBuffer0() E!i32 { return gl_shim.genBuffer(); }
pub fn J_mono_rhi_NativeRhi__glDeleteBuffer0(id: i32) E!void { gl_shim.deleteBuffer(id); }
pub fn J_mono_rhi_NativeRhi__glBindBuffer0(t: i32, id: i32) E!void { gl_shim.bindBuffer(@bitCast(t), id); }
pub fn J_mono_rhi_NativeRhi__glBufferData0(t: i32, addr: i64, sz: i64, usage: i32) E!void { gl_shim.bufferData(@bitCast(t), addr, sz, @bitCast(usage)); }
pub fn J_mono_rhi_NativeRhi__glBufferSubData0(t: i32, off: i64, addr: i64, sz: i64) E!void { gl_shim.bufferSubData(@bitCast(t), off, addr, sz); }

pub fn J_mono_rhi_NativeRhi__glGenVertexArray0() E!i32 { return gl_shim.genVertexArray(); }
pub fn J_mono_rhi_NativeRhi__glDeleteVertexArray0(id: i32) E!void { gl_shim.deleteVertexArray(id); }
pub fn J_mono_rhi_NativeRhi__glBindVertexArray0(id: i32) E!void { gl_shim.bindVertexArray(id); }
pub fn J_mono_rhi_NativeRhi__glEnableVertexAttribArray0(idx: i32) E!void { gl_shim.enableVertexAttribArray(@bitCast(idx)); }
pub fn J_mono_rhi_NativeRhi__glDisableVertexAttribArray0(idx: i32) E!void { gl_shim.disableVertexAttribArray(@bitCast(idx)); }
pub fn J_mono_rhi_NativeRhi__glVertexAttribPointer0(idx: i32, sz: i32, typ: i32, norm: i32, stride: i32, off: i64) E!void {
    gl_shim.vertexAttribPointer(@bitCast(idx), sz, @bitCast(typ), if (norm != 0) 1 else 0, stride, off);
}
pub fn J_mono_rhi_NativeRhi__glVertexAttribIPointer0(idx: i32, sz: i32, typ: i32, stride: i32, off: i64) E!void {
    gl_shim.vertexAttribIPointer(@bitCast(idx), sz, @bitCast(typ), stride, off);
}

pub fn J_mono_rhi_NativeRhi__glCreateShader0(kind: i32) E!i32 { return gl_shim.createShader(@bitCast(kind)); }
pub fn J_mono_rhi_NativeRhi__glDeleteShader0(sh: i32) E!void { gl_shim.deleteShader(sh); }
pub fn J_mono_rhi_NativeRhi__glShaderSource0(sh: i32, src_ref: rt.Ref) E!void {
    const utf8 = try jstrToUtf8Z(src_ref);
    defer rt.gpa.free(utf8);
    gl_shim.shaderSourceAdapted(sh, utf8);
}
pub fn J_mono_rhi_NativeRhi__glCompileShader0(sh: i32) E!void { gl_shim.compileShader(sh); }
pub fn J_mono_rhi_NativeRhi__glGetShaderi0(sh: i32, pname: i32) E!i32 { return gl_shim.getShaderi(sh, @bitCast(pname)); }

pub fn J_mono_rhi_NativeRhi__glCreateProgram0() E!i32 { return gl_shim.createProgram(); }
pub fn J_mono_rhi_NativeRhi__glDeleteProgram0(prog: i32) E!void { gl_shim.deleteProgram(prog); }
pub fn J_mono_rhi_NativeRhi__glAttachShader0(prog: i32, sh: i32) E!void { gl_shim.attachShader(prog, sh); }
pub fn J_mono_rhi_NativeRhi__glLinkProgram0(prog: i32) E!void { gl_shim.linkProgram(prog); }
pub fn J_mono_rhi_NativeRhi__glUseProgram0(prog: i32) E!void { gl_shim.useProgram(prog); }
pub fn J_mono_rhi_NativeRhi__glGetProgrami0(prog: i32, pname: i32) E!i32 { return gl_shim.getProgrami(prog, @bitCast(pname)); }
pub fn J_mono_rhi_NativeRhi__glGetUniformLocation0(prog: i32, name_ref: rt.Ref) E!i32 {
    const utf8z = try jstrToUtf8Z(name_ref);
    defer rt.gpa.free(utf8z);
    return gl_shim.getUniformLocation(prog, utf8z.ptr);
}
pub fn J_mono_rhi_NativeRhi__glUniform1i0(loc: i32, v0: i32) E!void { gl_shim.uniform1i(loc, v0); }
pub fn J_mono_rhi_NativeRhi__glUniform1f0(loc: i32, v0: f32) E!void { gl_shim.uniform1f(loc, v0); }
pub fn J_mono_rhi_NativeRhi__glUniform2f0(loc: i32, v0: f32, v1: f32) E!void { gl_shim.uniform2f(loc, v0, v1); }
pub fn J_mono_rhi_NativeRhi__glUniform3f0(loc: i32, v0: f32, v1: f32, v2: f32) E!void { gl_shim.uniform3f(loc, v0, v1, v2); }
pub fn J_mono_rhi_NativeRhi__glUniform4f0(loc: i32, v0: f32, v1: f32, v2: f32, v3: f32) E!void { gl_shim.uniform4f(loc, v0, v1, v2, v3); }
pub fn J_mono_rhi_NativeRhi__glUniformMatrix4fv0(loc: i32, count: i32, trans: i32, addr: i64) E!void {
    gl_shim.uniformMatrix4fv(loc, count, if (trans != 0) 1 else 0, addr);
}

pub fn J_mono_rhi_NativeRhi__glGenTexture0() E!i32 { return gl_shim.genTexture(); }
pub fn J_mono_rhi_NativeRhi__glDeleteTexture0(id: i32) E!void { gl_shim.deleteTexture(id); }
pub fn J_mono_rhi_NativeRhi__glActiveTexture0(t: i32) E!void { gl_shim.activeTexture(@bitCast(t)); }
pub fn J_mono_rhi_NativeRhi__glBindTexture0(t: i32, id: i32) E!void { gl_shim.bindTexture(@bitCast(t), id); }
pub fn J_mono_rhi_NativeRhi__glTexParameteri0(t: i32, p: i32, v: i32) E!void { gl_shim.texParameteri(@bitCast(t), @bitCast(p), v); }
pub fn J_mono_rhi_NativeRhi__glTexImage2D0(t: i32, lvl: i32, ifmt: i32, w: i32, h: i32, border: i32, fmt: i32, typ: i32, addr: i64) E!void {
    gl_shim.texImage2D(@bitCast(t), lvl, ifmt, w, h, border, @bitCast(fmt), @bitCast(typ), addr);
}
pub fn J_mono_rhi_NativeRhi__glTexSubImage2D0(t: i32, lvl: i32, xo: i32, yo: i32, w: i32, h: i32, fmt: i32, typ: i32, addr: i64) E!void {
    gl_shim.texSubImage2D(@bitCast(t), lvl, xo, yo, w, h, @bitCast(fmt), @bitCast(typ), addr);
}

pub fn J_mono_rhi_NativeRhi__glGenFramebuffer0() E!i32 { return gl_shim.genFramebuffer(); }
pub fn J_mono_rhi_NativeRhi__glDeleteFramebuffer0(id: i32) E!void { gl_shim.deleteFramebuffer(id); }
pub fn J_mono_rhi_NativeRhi__glBindFramebuffer0(t: i32, id: i32) E!void { gl_shim.bindFramebuffer(@bitCast(t), id); }
pub fn J_mono_rhi_NativeRhi__glFramebufferTexture2D0(t: i32, a: i32, tt: i32, tex: i32, lvl: i32) E!void {
    gl_shim.framebufferTexture2D(@bitCast(t), @bitCast(a), @bitCast(tt), tex, lvl);
}
pub fn J_mono_rhi_NativeRhi__glCheckFramebufferStatus0(t: i32) E!i32 { return gl_shim.checkFramebufferStatus(@bitCast(t)); }

pub fn J_mono_rhi_NativeRhi__glDrawArrays0(m: i32, first: i32, count: i32) E!void { gl_shim.drawArrays(@bitCast(m), first, count); }
pub fn J_mono_rhi_NativeRhi__glDrawElements0(m: i32, count: i32, typ: i32, off: i64) E!void { gl_shim.drawElements(@bitCast(m), count, @bitCast(typ), off); }
pub fn J_mono_rhi_NativeRhi__glDrawElementsInstanced0(m: i32, count: i32, typ: i32, off: i64, icount: i32) E!void {
    gl_shim.drawElementsInstanced(@bitCast(m), count, @bitCast(typ), off, icount);
}
pub fn J_mono_rhi_NativeRhi__glDrawArraysInstanced0(m: i32, first: i32, count: i32, icount: i32) E!void {
    gl_shim.drawArraysInstanced(@bitCast(m), first, count, icount);
}
pub fn J_mono_rhi_NativeRhi__glDrawElementsInstancedBaseVertex0(m: i32, count: i32, typ: i32, off: i64, icount: i32, bv: i32) E!void {
    gl_shim.drawElementsInstancedBaseVertex(@bitCast(m), count, @bitCast(typ), off, icount, bv);
}
pub fn J_mono_rhi_NativeRhi__glBlitFramebuffer0(sx0: i32, sy0: i32, sx1: i32, sy1: i32, dx0: i32, dy0: i32, dx1: i32, dy1: i32, mask: i32, filter: i32) E!void {
    gl_shim.blitFramebuffer(sx0, sy0, sx1, sy1, dx0, dy0, dx1, dy1, @bitCast(mask), @bitCast(filter));
}
pub fn J_mono_rhi_NativeRhi__glBindAttribLocation0(prog: i32, idx: i32, name_ref: rt.Ref) E!void {
    const utf8z = try jstrToUtf8Z(name_ref);
    defer rt.gpa.free(utf8z);
    gl_shim.bindAttribLocation(prog, idx, utf8z.ptr);
}
pub fn J_mono_rhi_NativeRhi__glBindBufferRange0(t: i32, idx: i32, buf: i32, off: i64, sz: i64) E!void {
    gl_shim.bindBufferRange(@bitCast(t), idx, buf, off, sz);
}
pub fn J_mono_rhi_NativeRhi__glCopyBufferSubData0(rtgt: i32, wtgt: i32, roff: i64, woff: i64, sz: i64) E!void {
    gl_shim.copyBufferSubData(@bitCast(rtgt), @bitCast(wtgt), roff, woff, sz);
}
pub fn J_mono_rhi_NativeRhi__glMapBufferRange0(t: i32, off: i64, len: i64, access: i32) E!i64 {
    return gl_shim.mapBufferRange(@bitCast(t), off, len, @bitCast(access));
}
pub fn J_mono_rhi_NativeRhi__glUnmapBuffer0(t: i32) E!i32 {
    return if (gl_shim.unmapBuffer(@bitCast(t))) 1 else 0;
}
pub fn J_mono_rhi_NativeRhi__glFlushMappedBufferRange0(t: i32, off: i64, len: i64) E!void {
    gl_shim.flushMappedBufferRange(@bitCast(t), off, len);
}
pub fn J_mono_rhi_NativeRhi__glGenSampler0() E!i32 { return gl_shim.genSampler(); }
pub fn J_mono_rhi_NativeRhi__glDeleteSampler0(id: i32) E!void { gl_shim.deleteSampler(id); }
pub fn J_mono_rhi_NativeRhi__glBindSampler0(u: i32, s: i32) E!void { gl_shim.bindSampler(u, s); }
pub fn J_mono_rhi_NativeRhi__glSamplerParameteri0(s: i32, p: i32, v: i32) E!void { gl_shim.samplerParameteri(s, @bitCast(p), v); }
pub fn J_mono_rhi_NativeRhi__glSamplerParameterf0(s: i32, p: i32, v: f32) E!void { gl_shim.samplerParameterf(s, @bitCast(p), v); }
pub fn J_mono_rhi_NativeRhi__glBlendEquationSeparate0(rgb: i32, a: i32) E!void { gl_shim.blendEquationSeparate(@bitCast(rgb), @bitCast(a)); }
pub fn J_mono_rhi_NativeRhi__glBlendFuncSeparate0(sr: i32, dr: i32, sa: i32, da: i32) E!void {
    gl_shim.blendFuncSeparate(@bitCast(sr), @bitCast(dr), @bitCast(sa), @bitCast(da));
}
pub fn J_mono_rhi_NativeRhi__glClearBufferfv0(buf: i32, db: i32, arr_ref: rt.Ref) E!void {
    if (arr_ref == null) return;
    const arr = rt.asArray(arr_ref.?);
    if (arr.length < 4) return;
    gl_shim.clearBufferfv(@bitCast(buf), db, rt.arrayData(f32, arr));
}
pub fn J_mono_rhi_NativeRhi__glColorMaski0(idx: i32, r: i32, g: i32, b: i32, a: i32) E!void {
    gl_shim.colorMaski(idx, if (r != 0) 1 else 0, if (g != 0) 1 else 0, if (b != 0) 1 else 0, if (a != 0) 1 else 0);
}
pub fn J_mono_rhi_NativeRhi__glEnablei0(cap: i32, idx: i32) E!void { gl_shim.enablei(@bitCast(cap), idx); }
pub fn J_mono_rhi_NativeRhi__glDisablei0(cap: i32, idx: i32) E!void { gl_shim.disablei(@bitCast(cap), idx); }
pub fn J_mono_rhi_NativeRhi__glDrawBuffers0(arr_ref: rt.Ref) E!void {
    if (arr_ref == null) return;
    const arr = rt.asArray(arr_ref.?);
    if (arr.length <= 0) return;
    gl_shim.drawBuffers(arr.length, rt.arrayData(u32, arr));
}
pub fn J_mono_rhi_NativeRhi__glReadBuffer0(src: i32) E!void { gl_shim.readBuffer(@bitCast(src)); }
pub fn J_mono_rhi_NativeRhi__glFinish0() E!void { gl_shim.finish(); }
pub fn J_mono_rhi_NativeRhi__glGetFloat0(pname: i32) E!f32 { return gl_shim.getFloat(@bitCast(pname)); }
pub fn J_mono_rhi_NativeRhi__glGetInteger640(pname: i32) E!i64 { return gl_shim.getInteger64(@bitCast(pname)); }
pub fn J_mono_rhi_NativeRhi__glGetShaderInfoLog0(sh: i32) E!rt.Ref {
    var buf: [2048]u8 = undefined;
    return rt.newStringUtf8(gl_shim.getShaderInfoLog(sh, &buf));
}
pub fn J_mono_rhi_NativeRhi__glGetProgramInfoLog0(prog: i32) E!rt.Ref {
    var buf: [2048]u8 = undefined;
    return rt.newStringUtf8(gl_shim.getProgramInfoLog(prog, &buf));
}
pub fn J_mono_rhi_NativeRhi__glGetUniformBlockIndex0(prog: i32, name_ref: rt.Ref) E!i32 {
    const utf8z = try jstrToUtf8Z(name_ref);
    defer rt.gpa.free(utf8z);
    return gl_shim.getUniformBlockIndex(prog, utf8z.ptr);
}
pub fn J_mono_rhi_NativeRhi__glUniformBlockBinding0(prog: i32, idx: i32, b: i32) E!void { gl_shim.uniformBlockBinding(prog, idx, b); }
pub fn J_mono_rhi_NativeRhi__glPixelStorei0(p: i32, v: i32) E!void { gl_shim.pixelStorei(@bitCast(p), v); }
pub fn J_mono_rhi_NativeRhi__glPolygonOffset0(f: f32, u: f32) E!void { gl_shim.polygonOffset(f, u); }
pub fn J_mono_rhi_NativeRhi__glReadPixels0(x: i32, y: i32, w: i32, h: i32, fmt: i32, typ: i32, addr: i64) E!void {
    gl_shim.readPixels(x, y, w, h, @bitCast(fmt), @bitCast(typ), addr);
}
pub fn J_mono_rhi_NativeRhi__glTexBuffer0(t: i32, ifmt: i32, buf: i32) E!void { gl_shim.texBuffer(@bitCast(t), @bitCast(ifmt), buf); }
pub fn J_mono_rhi_NativeRhi__glGetTexLevelParameteri0(t: i32, lvl: i32, p: i32) E!i32 {
    return gl_shim.getTexLevelParameteri(@bitCast(t), lvl, @bitCast(p));
}
pub fn J_mono_rhi_NativeRhi__glGenQuery0() E!i32 { return gl_shim.genQuery(); }
pub fn J_mono_rhi_NativeRhi__glDeleteQuery0(id: i32) E!void { gl_shim.deleteQuery(id); }
pub fn J_mono_rhi_NativeRhi__glQueryCounter0(id: i32, t: i32) E!void { gl_shim.queryCounter(id, @bitCast(t)); }
pub fn J_mono_rhi_NativeRhi__glGetQueryObjecti0(id: i32, p: i32) E!i32 { return gl_shim.getQueryObjecti(id, @bitCast(p)); }
pub fn J_mono_rhi_NativeRhi__glGetQueryObjectui640(id: i32, p: i32) E!i64 { return gl_shim.getQueryObjectui64(id, @bitCast(p)); }
pub fn J_mono_rhi_NativeRhi__glVertexAttribDivisor0(idx: i32, div: i32) E!void {
    gl_shim.vertexAttribDivisor(@bitCast(idx), @bitCast(div));
}
pub fn J_mono_rhi_NativeRhi__glFenceSync0(cond: i32, flags: i32) E!i64 {
    return gl_shim.fenceSync(@bitCast(cond), @bitCast(flags));
}
pub fn J_mono_rhi_NativeRhi__glDeleteSync0(sync: i64) E!void { gl_shim.deleteSync(sync); }
pub fn J_mono_rhi_NativeRhi__glClientWaitSync0(sync: i64, flags: i32, timeout_ns: i64) E!i32 {
    return gl_shim.clientWaitSync(sync, @bitCast(flags), timeout_ns);
}

// ---- Optional dynamic OpenAL dispatch ----
pub fn J_mono_rhi_NativeRhi__alcOpenDevice0(name_ref: rt.Ref) E!i64 {
    if (name_ref == null) return al_shim.alcOpenDevice(null);
    const utf8z = try jstrToUtf8Z(name_ref);
    defer rt.gpa.free(utf8z);
    return al_shim.alcOpenDevice(utf8z.ptr);
}
pub fn J_mono_rhi_NativeRhi__alcCloseDevice0(d: i64) E!i32 { return if (al_shim.alcCloseDevice(d)) 1 else 0; }
pub fn J_mono_rhi_NativeRhi__alcCreateContext0(d: i64, attr: i64) E!i64 { return al_shim.alcCreateContext(d, attr); }
pub fn J_mono_rhi_NativeRhi__alcMakeContextCurrent0(c: i64) E!i32 { return if (al_shim.alcMakeContextCurrent(c)) 1 else 0; }
pub fn J_mono_rhi_NativeRhi__alcDestroyContext0(c: i64) E!void { al_shim.alcDestroyContext(c); }
pub fn J_mono_rhi_NativeRhi__alcGetInteger0(d: i64, p: i32) E!i32 { return al_shim.alcGetInteger(d, p); }
pub fn J_mono_rhi_NativeRhi__alcGetString0(d: i64, p: i32) E!rt.Ref { return rt.newStringUtf8(al_shim.alcGetString(d, p)); }
pub fn J_mono_rhi_NativeRhi__alGenBuffer0() E!i32 { return al_shim.alGenId(); }
pub fn J_mono_rhi_NativeRhi__alDeleteBuffer0(id: i32) E!void { al_shim.alDeleteBuffer(id); }
pub fn J_mono_rhi_NativeRhi__alBufferData0(b: i32, fmt: i32, addr: i64, sz: i32, freq: i32) E!void { al_shim.alBufferData(b, fmt, addr, sz, freq); }
pub fn J_mono_rhi_NativeRhi__alGenSource0() E!i32 { return al_shim.alGenSourceId(); }
pub fn J_mono_rhi_NativeRhi__alDeleteSource0(id: i32) E!void { al_shim.alDeleteSource(id); }
pub fn J_mono_rhi_NativeRhi__alSourcePlay0(s: i32) E!void { al_shim.alSourcePlay(s); }
pub fn J_mono_rhi_NativeRhi__alSourcePause0(s: i32) E!void { al_shim.alSourcePause(s); }
pub fn J_mono_rhi_NativeRhi__alSourceStop0(s: i32) E!void { al_shim.alSourceStop(s); }
pub fn J_mono_rhi_NativeRhi__alSourceQueueBuffer0(s: i32, b: i32) E!void { al_shim.alSourceQueueBuffer(s, b); }
pub fn J_mono_rhi_NativeRhi__alSourceUnqueueBuffer0(s: i32) E!i32 { return al_shim.alSourceUnqueueBuffer(s); }
pub fn J_mono_rhi_NativeRhi__alGetSourcei0(s: i32, p: i32) E!i32 { return al_shim.alGetSourcei(s, p); }
pub fn J_mono_rhi_NativeRhi__alSourcei0(s: i32, p: i32, v: i32) E!void { al_shim.alSourcei(s, p, v); }
pub fn J_mono_rhi_NativeRhi__alSourcef0(s: i32, p: i32, v: f32) E!void { al_shim.alSourcef(s, p, v); }
pub fn J_mono_rhi_NativeRhi__alSource3f0(s: i32, p: i32, x: f32, y: f32, z: f32) E!void { al_shim.alSource3f(s, p, x, y, z); }
pub fn J_mono_rhi_NativeRhi__alListener3f0(p: i32, x: f32, y: f32, z: f32) E!void { al_shim.alListener3f(p, x, y, z); }
pub fn J_mono_rhi_NativeRhi__alGetError0() E!i32 { return al_shim.alGetError(); }
pub fn J_mono_rhi_NativeRhi__wrapDirectByteBuffer0(addr: i64, cap: i32) E!rt.Ref {
    return P.newDirectByteBuffer(addr, @intCast(@max(cap, 0)));
}
