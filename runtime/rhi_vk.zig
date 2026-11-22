//! Vulkan 1.1+ Dynamic RHI Backend (zero hard link dependencies on libvulkan-dev).
//! Probes `libvulkan.so.1` at runtime via `std.DynLib`, checks physical GPU availability
//! (`VK_API_VERSION_1_1+`), resolves instance/device command entry points, and drives
//! low-overhead command submission & multi-draw indirect.

const std = @import("std");
const rhi = @import("rhi.zig");

pub const VK_SUCCESS: i32 = 0;
pub const VK_STRUCTURE_TYPE_APPLICATION_INFO: u32 = 0;
pub const VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO: u32 = 1;
pub const VK_PHYSICAL_DEVICE_TYPE_DISCRETE_GPU: u32 = 2;
pub const VK_PHYSICAL_DEVICE_TYPE_INTEGRATED_GPU: u32 = 1;

pub inline fn vkMakeApiVersion(variant: u32, major: u32, minor: u32, patch: u32) u32 {
    return (variant << 29) | (major << 22) | (minor << 12) | patch;
}

pub const VkApplicationInfo = extern struct {
    sType: u32 = VK_STRUCTURE_TYPE_APPLICATION_INFO,
    pNext: ?*const anyopaque = null,
    pApplicationName: ?[*:0]const u8 = "J2ZIG-MC",
    applicationVersion: u32 = 1,
    pEngineName: ?[*:0]const u8 = "J2ZIG-RHI",
    engineVersion: u32 = 1,
    apiVersion: u32 = vkMakeApiVersion(0, 1, 1, 0),
};

pub const VkInstanceCreateInfo = extern struct {
    sType: u32 = VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO,
    pNext: ?*const anyopaque = null,
    flags: u32 = 0,
    pApplicationInfo: ?*const VkApplicationInfo = null,
    enabledLayerCount: u32 = 0,
    ppEnabledLayerNames: ?[*]const [*:0]const u8 = null,
    enabledExtensionCount: u32 = 0,
    ppEnabledExtensionNames: ?[*]const [*:0]const u8 = null,
};

pub const VkPhysicalDeviceProperties = extern struct {
    apiVersion: u32,
    driverVersion: u32,
    vendorID: u32,
    deviceID: u32,
    deviceType: u32,
    deviceName: [256]u8,
    pipelineCacheUUID: [16]u8,
    limits_pad: [560]u8,
};

pub const VkInstance = ?*opaque {};
pub const VkPhysicalDevice = ?*opaque {};
pub const VkDevice = ?*opaque {};
pub const VkCommandBuffer = ?*opaque {};
pub const VkBuffer = ?*opaque {};

const PFN_vkVoidFunction = ?*const fn () callconv(.c) void;
const PFN_vkGetInstanceProcAddr = *const fn (VkInstance, [*:0]const u8) callconv(.c) PFN_vkVoidFunction;
const PFN_vkCreateInstance = *const fn (*const VkInstanceCreateInfo, ?*const anyopaque, *VkInstance) callconv(.c) i32;
const PFN_vkDestroyInstance = *const fn (VkInstance, ?*const anyopaque) callconv(.c) void;
const PFN_vkEnumeratePhysicalDevices = *const fn (VkInstance, *u32, ?[*]VkPhysicalDevice) callconv(.c) i32;
const PFN_vkGetPhysicalDeviceProperties = *const fn (VkPhysicalDevice, *VkPhysicalDeviceProperties) callconv(.c) void;

pub const VkCmdDrawIndexedFn = *const fn (VkCommandBuffer, u32, u32, u32, i32, u32) callconv(.c) void;
pub const VkCmdDrawIndexedIndirectFn = *const fn (VkCommandBuffer, VkBuffer, u64, u32, u32) callconv(.c) void;

pub const VkProcs = struct {
    vkCmdDrawIndexed: ?VkCmdDrawIndexedFn = null,
    vkCmdDrawIndexedIndirect: ?VkCmdDrawIndexedIndirectFn = null,
};

pub const VkBackend = struct {
    const MAX_HANDLES = 512;

    lib: std.DynLib,
    instance: VkInstance = null,
    phys_device: VkPhysicalDevice = null,
    active_cmd_buf: VkCommandBuffer = null,
    vk_buffers: [MAX_HANDLES]VkBuffer = [_]VkBuffer{null} ** MAX_HANDLES,
    procs: VkProcs = .{},
    api_version: u32 = 0,
    vendor_id: u32 = 0,
    device_type: u32 = 0,
    gpu_name: [256]u8 = [_]u8{0} ** 256,
    vkDestroyInstance: PFN_vkDestroyInstance,
    submitted_indirect_batches: u64 = 0,

    pub fn probe() ?VkBackend {
        var lib = std.DynLib.open("libvulkan.so.1") catch
            (std.DynLib.open("libvulkan.so") catch return null);
        const get_proc = lib.lookup(PFN_vkGetInstanceProcAddr, "vkGetInstanceProcAddr") orelse {
            lib.close();
            return null;
        };
        const create_fn_raw = get_proc(null, "vkCreateInstance") orelse {
            lib.close();
            return null;
        };
        const create_fn: PFN_vkCreateInstance = @ptrCast(create_fn_raw);

        const app = VkApplicationInfo{};
        const ci = VkInstanceCreateInfo{ .pApplicationInfo = &app };
        var inst: VkInstance = null;
        if (create_fn(&ci, null, &inst) != VK_SUCCESS or inst == null) {
            lib.close();
            return null;
        }
        const destroy_raw = get_proc(inst, "vkDestroyInstance") orelse {
            lib.close();
            return null;
        };
        const destroy_fn: PFN_vkDestroyInstance = @ptrCast(destroy_raw);

        const enum_raw = get_proc(inst, "vkEnumeratePhysicalDevices") orelse {
            destroy_fn(inst, null);
            lib.close();
            return null;
        };
        const enum_fn: PFN_vkEnumeratePhysicalDevices = @ptrCast(enum_raw);

        var count: u32 = 0;
        if (enum_fn(inst, &count, null) != VK_SUCCESS or count == 0) {
            destroy_fn(inst, null);
            lib.close();
            return null;
        }
        var devs: [8]VkPhysicalDevice = [_]VkPhysicalDevice{null} ** 8;
        var cap: u32 = @min(count, @as(u32, 8));
        _ = enum_fn(inst, &cap, &devs);

        var chosen_pd: VkPhysicalDevice = devs[0];
        var chosen_props: VkPhysicalDeviceProperties = std.mem.zeroes(VkPhysicalDeviceProperties);
        if (get_proc(inst, "vkGetPhysicalDeviceProperties")) |props_raw| {
            const props_fn: PFN_vkGetPhysicalDeviceProperties = @ptrCast(props_raw);
            var best_score: i32 = -1;
            for (devs[0..cap]) |pd| {
                var p: VkPhysicalDeviceProperties = std.mem.zeroes(VkPhysicalDeviceProperties);
                props_fn(pd, &p);
                if (p.apiVersion < vkMakeApiVersion(0, 1, 1, 0)) continue;
                const score: i32 = switch (p.deviceType) {
                    VK_PHYSICAL_DEVICE_TYPE_DISCRETE_GPU => 100,
                    VK_PHYSICAL_DEVICE_TYPE_INTEGRATED_GPU => 50,
                    else => 10,
                };
                if (score > best_score) {
                    best_score = score;
                    chosen_pd = pd;
                    chosen_props = p;
                }
            }
            if (best_score < 0) {
                destroy_fn(inst, null);
                lib.close();
                return null;
            }
        }

        var procs = VkProcs{};
        if (get_proc(inst, "vkCmdDrawIndexed")) |f| procs.vkCmdDrawIndexed = @ptrCast(f);
        if (get_proc(inst, "vkCmdDrawIndexedIndirect")) |f| procs.vkCmdDrawIndexedIndirect = @ptrCast(f);

        return VkBackend{
            .lib = lib,
            .instance = inst,
            .phys_device = chosen_pd,
            .procs = procs,
            .api_version = chosen_props.apiVersion,
            .vendor_id = chosen_props.vendorID,
            .device_type = chosen_props.deviceType,
            .gpu_name = chosen_props.deviceName,
            .vkDestroyInstance = destroy_fn,
        };
    }

    pub fn deinit(self: *VkBackend) void {
        if (self.instance != null) {
            self.vkDestroyInstance(self.instance, null);
            self.instance = null;
        }
        self.lib.close();
    }

    pub fn onCreateBuffer(self: *VkBackend, id: u32, desc: rhi.BufferDesc) void {
        _ = self;
        _ = id;
        _ = desc;
    }

    pub fn executeCommands(self: *VkBackend, cmds: []const rhi.Command, ring: *rhi.FrameRing) void {
        _ = ring;
        const cb = self.active_cmd_buf;
        for (cmds) |cmd| {
            switch (cmd) {
                .draw_indexed_instanced => |d| {
                    if (cb != null) {
                        if (self.procs.vkCmdDrawIndexed) |fn_di| {
                            fn_di(cb, d.index_count, d.instance_count, d.first_index, d.vertex_offset, d.first_instance);
                        }
                    }
                },
                .draw_indexed_indirect_range => |ir| {
                    self.submitted_indirect_batches += 1;
                    if (cb != null) {
                        const bid = @intFromEnum(ir.indirect_buf);
                        const vk_buf = if (bid < MAX_HANDLES) self.vk_buffers[bid] else null;
                        if (vk_buf != null) {
                            if (self.procs.vkCmdDrawIndexedIndirect) |fn_ind| {
                                fn_ind(cb, vk_buf, ir.offset_bytes, ir.draw_count, ir.stride);
                            }
                        }
                    }
                },
                else => {},
            }
        }
    }
};
