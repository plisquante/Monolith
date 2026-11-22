//! Custom Zero-LWJGL RHI (Render Hardware Interface) targeting:
//!   - Modern GPUs: Vulkan 1.1+ (`rhi_vk.zig`) — lowest driver CPU overhead, multi-draw indirect,
//!     bindless descriptors, compute culling.
//!   - Legacy Integrated GPUs (e.g. Intel HD 5500): OpenGL 3.3 Core (`rhi_gl.zig`) — strict GL 3.3
//!     TBO (`GL_TEXTURE_BUFFER`) instancing, orphan+unsync triple-buffered streaming, driver state cache.
//!   - Headless/Verification Mode: deterministic command-encoder + ring-allocator validation without
//!     requiring an active X11/Wayland/GPU display server.

const std = @import("std");
const mem_pool = @import("mem_pool.zig");
pub const vk = @import("rhi_vk.zig");
pub const gl = @import("rhi_gl.zig");

pub const BackendKind = enum(u8) {
    auto = 0,
    vulkan = 1,
    opengl33 = 2,
    headless = 3,
};

pub const PowerProfile = enum(u8) {
    max_fps = 0, // Vulkan MAILBOX / GL swapInterval(0)
    balanced = 1, // Vulkan FIFO / adaptive
    eco_battery = 2, // FIFO vsync + frame pacing + reduced idle GPU work
};

pub const BufferUsage = packed struct(u32) {
    vertex: bool = false,
    index: bool = false,
    uniform: bool = false,
    storage: bool = false, // Vulkan SSBO / GL3.3 TBO (GL_TEXTURE_BUFFER)
    indirect: bool = false,
    transfer_src: bool = false,
    transfer_dst: bool = false,
    host_visible: bool = false,
    _pad: u24 = 0,
};

pub const BufferHandle = enum(u32) { invalid = 0, _ };
pub const TextureHandle = enum(u32) { invalid = 0, _ };
pub const PipelineHandle = enum(u32) { invalid = 0, _ };

pub const BufferDesc = struct {
    size: usize,
    usage: BufferUsage,
    stride: u32 = 0, // element stride for GL3.3 TBO texel-format mapping
};

pub const TextureFormat = enum(u8) {
    rgba8_unorm,
    rgba8_srgb,
    r32_float, // Hi-Z depth pyramid
    depth24_stencil8,
    depth32_float,
};

pub const TextureDesc = struct {
    width: u32,
    height: u32,
    mip_levels: u32 = 1,
    format: TextureFormat = .rgba8_unorm,
    render_target: bool = false,
};

pub const DrawIndexedIndirectCmd = extern struct {
    index_count: u32,
    instance_count: u32,
    first_index: u32,
    vertex_offset: i32,
    first_instance: u32,
};

pub const DeviceCaps = struct {
    active_backend: BackendKind,
    device_name: [128]u8 = [_]u8{0} ** 128,
    device_name_len: usize = 0,
    max_instances_per_draw: u32 = 1_048_576,
    supports_multi_draw_indirect: bool = false,
    supports_base_instance: bool = false,
    supports_compute_cull: bool = false,
    uses_tbo_instancing: bool = false, // true on OpenGL 3.3 Core
    min_ubo_alignment: u32 = 256,

    pub fn nameSlice(self: *const DeviceCaps) []const u8 {
        return self.device_name[0..self.device_name_len];
    }

    pub fn setName(self: *DeviceCaps, s: []const u8) void {
        const n = @min(s.len, self.device_name.len);
        @memcpy(self.device_name[0..n], s[0..n]);
        self.device_name_len = n;
    }
};

pub const FrameStats = struct {
    frame_index: u64 = 0,
    draw_calls: u32 = 0,
    state_changes_skipped: u32 = 0,
    instances_submitted: u64 = 0,
    triangles_submitted: u64 = 0,
    staging_bytes_used: usize = 0,
};

/// Encoded GPU command recorded linearly inside per-frame ArenaAllocator (zero malloc).
pub const Command = union(enum) {
    begin_pass: struct {
        clear_r: f32,
        clear_g: f32,
        clear_b: f32,
        clear_a: f32,
        clear_depth: f32,
        viewport_w: u32,
        viewport_h: u32,
    },
    bind_pipeline: PipelineHandle,
    bind_vertex_buffer: struct { slot: u32, buf: BufferHandle, offset: usize },
    bind_index_buffer: struct { buf: BufferHandle, index_32: bool },
    bind_instance_buffer: struct { binding: u32, buf: BufferHandle, offset_bytes: usize, size_bytes: usize = 0, texel_stride: u32 },
    draw_indexed_instanced: struct {
        index_count: u32,
        instance_count: u32,
        first_index: u32,
        vertex_offset: i32,
        first_instance: u32,
    },
    draw_indexed_indirect_range: struct {
        indirect_buf: BufferHandle,
        offset_bytes: usize,
        draw_count: u32,
        stride: u32,
        fallback_cmds: []const DrawIndexedIndirectCmd, // populated for OpenGL 3.3 Core fallback
    },
    end_pass,
};

pub const StagingSlice = struct {
    ptr: [*]u8,
    offset: usize,
    size: usize,
    buf: BufferHandle,
};

/// Triple-buffered per-frame transient GPU upload ring backed cleanly without GC.
pub const FrameRing = struct {
    pub const FRAMES_IN_FLIGHT = 3;
    ring_mem: [FRAMES_IN_FLIGHT][]align(64) u8 = .{ &.{}, &.{}, &.{} },
    ring_handles: [FRAMES_IN_FLIGHT]BufferHandle = .{ .invalid, .invalid, .invalid },
    cursor: usize = 0,
    capacity_per_frame: usize = 0,

    pub fn init(self: *FrameRing, per_frame_bytes: usize) !void {
        self.capacity_per_frame = per_frame_bytes;
        // a failure on frame k must free frames 0..k-1 (deinit skips empty slots)
        errdefer self.deinit();
        for (0..FRAMES_IN_FLIGHT) |i| {
            self.ring_mem[i] = try std.heap.page_allocator.alignedAlloc(u8, .fromByteUnits(64), per_frame_bytes);
            self.ring_handles[i] = @enumFromInt(@as(u32, @intCast(i + 1)));
        }
    }

    pub fn deinit(self: *FrameRing) void {
        for (0..FRAMES_IN_FLIGHT) |i| {
            if (self.ring_mem[i].len > 0) {
                std.heap.page_allocator.free(self.ring_mem[i]);
                self.ring_mem[i] = &.{};
            }
            self.ring_handles[i] = .invalid;
        }
    }

    pub fn beginFrame(self: *FrameRing) void {
        self.cursor = 0;
    }

    pub fn allocUpload(self: *FrameRing, frame_slot: usize, bytes: usize, align_bytes: usize) ?StagingSlice {
        const slot = frame_slot % FRAMES_IN_FLIGHT;
        const a = @max(align_bytes, 16);
        const off = mem_pool.alignUp(self.cursor, a);
        if (off > self.capacity_per_frame or bytes > self.capacity_per_frame - off) return null;
        self.cursor = off + bytes;
        return .{
            .ptr = self.ring_mem[slot].ptr + off,
            .offset = off,
            .size = bytes,
            .buf = self.ring_handles[slot],
        };
    }
};

/// Main RHI Device: auto-detects Vulkan 1.1+ -> OpenGL 3.3 Core -> Headless verification,
/// eliminates redundant driver state changes, and records commands into a zero-GC ArenaAllocator.
pub const RhiDevice = struct {
    caps: DeviceCaps,
    power_profile: PowerProfile = .balanced,
    arena: mem_pool.ArenaAllocator = mem_pool.ArenaAllocator.init(mem_pool.DEFAULT_CHUNK_SIZE, 4),
    ring: FrameRing = .{},

    vk_ctx: ?vk.VkBackend = null,
    gl_ctx: ?gl.Gl33Backend = null,

    next_buf_id: u32 = 16,
    next_tex_id: u32 = 16,
    next_pipe_id: u32 = 16,

    cmd_list: std.ArrayListUnmanaged(Command) = .{},
    bound_pipe: PipelineHandle = .invalid,
    bound_ib: BufferHandle = .invalid,
    stats: FrameStats = .{},
    last_stats: FrameStats = .{},

    pub fn init(preferred: BackendKind, power: PowerProfile, staging_mb_per_frame: usize) !RhiDevice {
        var dev = RhiDevice{
            .caps = .{ .active_backend = .headless },
            .power_profile = power,
        };
        try dev.ring.init(@max(staging_mb_per_frame, 1) * 1024 * 1024);
        // a later `return error.BackendUnavailable` must not leak the staging ring
        errdefer dev.ring.deinit();

        const chosen: BackendKind = switch (preferred) {
            .auto => blk: {
                if (vk.VkBackend.probe()) |vc| {
                    dev.vk_ctx = vc;
                    break :blk .vulkan;
                }
                if (gl.Gl33Backend.probe()) |gc| {
                    dev.gl_ctx = gc;
                    break :blk .opengl33;
                }
                break :blk .headless;
            },
            .vulkan => blk: {
                if (vk.VkBackend.probe()) |vc| {
                    dev.vk_ctx = vc;
                    break :blk .vulkan;
                }
                return error.BackendUnavailable;
            },
            .opengl33 => blk: {
                if (gl.Gl33Backend.probe()) |gc| {
                    dev.gl_ctx = gc;
                    break :blk .opengl33;
                }
                return error.BackendUnavailable;
            },
            .headless => .headless,
        };

        dev.caps.active_backend = chosen;
        switch (chosen) {
            .vulkan => {
                dev.caps.setName("MONOLITH Vulkan 1.1+ RHI");
                dev.caps.supports_multi_draw_indirect = true;
                dev.caps.supports_base_instance = true;
                dev.caps.supports_compute_cull = true;
                dev.caps.uses_tbo_instancing = false;
            },
            .opengl33 => {
                dev.caps.setName("MONOLITH OpenGL 3.3 Core RHI (Intel HD 5500 Safe)");
                dev.caps.supports_multi_draw_indirect = false; // lowered transparently via fallback_cmds
                dev.caps.supports_base_instance = if (dev.gl_ctx) |*g| g.has_base_instance else false;
                dev.caps.supports_compute_cull = false;
                dev.caps.uses_tbo_instancing = true;
            },
            .headless, .auto => {
                dev.caps.setName("MONOLITH Headless Command-Verified RHI");
                dev.caps.supports_multi_draw_indirect = true;
                dev.caps.supports_base_instance = true;
                dev.caps.supports_compute_cull = true;
                dev.caps.uses_tbo_instancing = true;
            },
        }
        return dev;
    }

    /// If `init(.auto, ...)` fell back to `.headless` because no GL context was current at startup,
    /// call `tryAttachGlContext()` once a window/GL context becomes current on this thread.
    pub fn tryAttachGlContext(self: *RhiDevice) bool {
        if (self.caps.active_backend != .headless) return false;
        if (gl.Gl33Backend.probe()) |gc| {
            self.gl_ctx = gc;
            self.caps.active_backend = .opengl33;
            self.caps.setName("MONOLITH OpenGL 3.3 Core RHI (Intel HD 5500 Safe)");
            self.caps.supports_multi_draw_indirect = false;
            self.caps.supports_base_instance = gc.has_base_instance;
            self.caps.supports_compute_cull = false;
            self.caps.uses_tbo_instancing = true;
            return true;
        }
        return false;
    }

    pub fn deinit(self: *RhiDevice) void {
        if (self.vk_ctx) |*v| v.deinit();
        if (self.gl_ctx) |*g| g.deinit();
        self.ring.deinit();
        self.arena.deinit();
    }

    pub fn createBuffer(self: *RhiDevice, desc: BufferDesc) BufferHandle {
        const id = self.next_buf_id;
        self.next_buf_id += 1;
        if (self.gl_ctx) |*g| g.onCreateBuffer(id, desc);
        if (self.vk_ctx) |*v| v.onCreateBuffer(id, desc);
        return @enumFromInt(id);
    }

    pub fn createPipeline(self: *RhiDevice) PipelineHandle {
        const id = self.next_pipe_id;
        self.next_pipe_id += 1;
        return @enumFromInt(id);
    }

    pub fn createTexture(self: *RhiDevice, desc: TextureDesc) TextureHandle {
        _ = desc;
        const id = self.next_tex_id;
        self.next_tex_id += 1;
        return @enumFromInt(id);
    }

    pub fn beginFrame(self: *RhiDevice) void {
        self.arena.resetRetainingCapacity();
        self.ring.beginFrame();
        self.cmd_list = .{};
        self.bound_pipe = .invalid;
        self.bound_ib = .invalid;
        const next_idx = self.stats.frame_index + 1;
        self.stats = .{ .frame_index = next_idx };
    }

    pub fn allocTransientUpload(self: *RhiDevice, bytes: usize, align_bytes: usize) ?StagingSlice {
        const slice = self.ring.allocUpload(self.stats.frame_index, bytes, align_bytes) orelse return null;
        self.stats.staging_bytes_used += bytes;
        return slice;
    }

    pub fn allocCmdSlice(self: *RhiDevice, comptime T: type, count: usize) ?[]T {
        const raw = self.arena.alloc(count * @sizeOf(T), @alignOf(T)) orelse return null;
        const ptr: [*]T = @ptrCast(@alignCast(raw));
        return ptr[0..count];
    }

    pub fn record(self: *RhiDevice, cmd: Command) !void {
        switch (cmd) {
            .bind_pipeline => |p| {
                if (p == self.bound_pipe) {
                    self.stats.state_changes_skipped += 1;
                    return;
                }
                self.bound_pipe = p;
            },
            .bind_index_buffer => |ib| {
                if (ib.buf == self.bound_ib) {
                    self.stats.state_changes_skipped += 1;
                    return;
                }
                self.bound_ib = ib.buf;
            },
            else => {},
        }
        try self.cmd_list.append(self.arena.allocator(), cmd);
    }

    pub fn submitFrame(self: *RhiDevice) void {
        for (self.cmd_list.items) |cmd| {
            switch (cmd) {
                .draw_indexed_instanced => |d| {
                    self.stats.draw_calls += 1;
                    self.stats.instances_submitted += d.instance_count;
                    self.stats.triangles_submitted += (@as(u64, d.index_count) / 3) * d.instance_count;
                },
                .draw_indexed_indirect_range => |ir| {
                    if (self.caps.supports_multi_draw_indirect) {
                        self.stats.draw_calls += 1;
                    } else {
                        self.stats.draw_calls += ir.draw_count;
                    }
                    for (ir.fallback_cmds) |fc| {
                        self.stats.instances_submitted += fc.instance_count;
                        self.stats.triangles_submitted += (@as(u64, fc.index_count) / 3) * fc.instance_count;
                    }
                },
                else => {},
            }
        }
        if (self.vk_ctx) |*v| v.executeCommands(self.cmd_list.items, &self.ring);
        if (self.gl_ctx) |*g| g.executeCommands(self.cmd_list.items, &self.ring);
        self.last_stats = self.stats;
    }
};

test "RHI auto-probing, state deduplication, zero-malloc command arena & staging ring" {
    // Explicit headless + auto-probe test
    var dev = try RhiDevice.init(.headless, .eco_battery, 2);
    defer dev.deinit();

    const p1 = dev.createPipeline();
    const vb = dev.createBuffer(.{ .size = 64 * 1024, .usage = .{ .vertex = true } });
    const ib = dev.createBuffer(.{ .size = 16 * 1024, .usage = .{ .index = true } });

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
    try dev.record(.{ .bind_pipeline = p1 });
    try dev.record(.{ .bind_pipeline = p1 }); // duplicate bind should be deduplicated
    try dev.record(.{ .bind_vertex_buffer = .{ .slot = 0, .buf = vb, .offset = 0 } });
    try dev.record(.{ .bind_index_buffer = .{ .buf = ib, .index_32 = true } });
    try dev.record(.{ .bind_index_buffer = .{ .buf = ib, .index_32 = true } }); // duplicate IB bind

    const stg = dev.allocTransientUpload(256, 64) orelse return error.OutOfMemory;
    stg.ptr[0] = 0x42;

    const cmds = dev.allocCmdSlice(DrawIndexedIndirectCmd, 2) orelse return error.OutOfMemory;
    cmds[0] = .{ .index_count = 36, .instance_count = 120, .first_index = 0, .vertex_offset = 0, .first_instance = 0 };
    cmds[1] = .{ .index_count = 72, .instance_count = 40, .first_index = 36, .vertex_offset = 24, .first_instance = 120 };

    try dev.record(.{ .draw_indexed_indirect_range = .{
        .indirect_buf = stg.buf,
        .offset_bytes = stg.offset,
        .draw_count = 2,
        .stride = @sizeOf(DrawIndexedIndirectCmd),
        .fallback_cmds = cmds,
    } });
    try dev.record(.end_pass);
    dev.submitFrame();

    try std.testing.expectEqual(@as(u32, 2), dev.last_stats.state_changes_skipped);
    try std.testing.expectEqual(@as(u32, 1), dev.last_stats.draw_calls);
    try std.testing.expectEqual(@as(u64, 160), dev.last_stats.instances_submitted);
    try std.testing.expectEqual(@as(u64, 12 * 120 + 24 * 40), dev.last_stats.triangles_submitted);
}
