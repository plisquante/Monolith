//! Client session & per-frame RHI lifecycle hooks (mono.rhi bridge bring-up/frame-bracket/teardown).
//!
//! Safety contract (same as patches/world_height.zig):
//!   * `"orig": true` — the translated original is called for the whole game logic; these
//!     hooks add side effects around it without altering vanilla tick/physics/server logic.
//!   * Opt-in via `MONO_RHI=1` or `MONO_NO_LWJGL=1`. Zero overhead beyond one cached check otherwise.
//!   * Any RHI init failure degrades to a printed warning and the untouched game — startup never aborts.
const std = @import("std");
const rt = @import("rt.zig");
const Ref = rt.Ref;

const bridge = @import("rhi_bridge.zig");

fn rhiEnabled() bool {
    if (rt.getenv("MONO_NO_LWJGL")) |nl| {
        if (nl.len > 0 and nl[0] != '0') return true;
    }
    const v = rt.getenv("MONO_RHI") orelse return false;
    return v.len > 0 and v[0] != '0';
}

fn preferredBackend() @import("rhi.zig").BackendKind {
    const b = rt.getenv("MONO_RHI_BACKEND") orelse return .auto;
    if (std.mem.eql(u8, b, "vulkan")) return .vulkan;
    if (std.mem.eql(u8, b, "opengl") or std.mem.eql(u8, b, "opengl33")) return .opengl33;
    if (std.mem.eql(u8, b, "headless")) return .headless;
    return .auto;
}

fn statsEnabled() bool {
    const v = rt.getenv("MONO_RHI_STATS") orelse return false;
    return v.len > 0 and v[0] != '0';
}

fn rhiBringUp() void {
    if (!rhiEnabled()) return;
    _ = bridge.initClientRhi(preferredBackend(), .balanced) catch |e| {
        std.debug.print("monolith: RHI init failed ({s}); continuing with vanilla rendering\n", .{@errorName(e)});
        return;
    };
    std.debug.print("monolith: RHI active (backend id {d})\n", .{bridge.mono_rhi_active_backend()});
}

fn rhiTearDown() void {
    if (!rhiEnabled()) return;
    if (statsEnabled()) {
        std.debug.print("monolith: RHI stats — epoch={d} threads={d} arena={d}B slab={d}B reclaimed={d}B sections-dropped={d}\n", .{
            rt.st.frame_epoch.load(.monotonic),
            rt.st.thread_count.load(.monotonic),
            rt.cur.scratch_arena.committed_bytes,
            rt.st.slab_pool.committed_bytes.load(.monotonic),
            rt.st.slab_pool.reclaimed_bytes.load(.monotonic),
            bridge.rhi_st.dropped_sections,
        });
    }
    bridge.shutdownClientRhi();
}

/// net.minecraft.client.main.Main.main(String[])V — brackets the whole client session.
pub fn clientMain(orig: anytype, a0: Ref) rt.E!void {
    rhiBringUp();
    orig(a0) catch |e| {
        rhiTearDown();
        return e;
    };
    rhiTearDown();
}

/// net.minecraft.client.Minecraft.run()V — same brackets, for clients where Main is a thin wrapper.
pub fn minecraftRun(orig: anytype, this: Ref) rt.E!void {
    rhiBringUp();
    orig(this) catch |e| {
        rhiTearDown();
        return e;
    };
    rhiTearDown();
}

/// net.minecraft.client.Minecraft.runTick(Z)V — brackets each render/tick frame with
/// `mono_rhi_begin_frame` (epoch advance + lazy GL context attach) and `mono_rhi_end_frame`
/// (command submission + SlabPool epoch reclaim).
pub fn minecraftRunTick(orig: anytype, this: Ref, render_tick: i32) rt.E!void {
    if (!rhiEnabled()) return orig(this, render_tick);
    if (render_tick != 0) {
        _ = bridge.mono_rhi_begin_frame(bridge.rhi_st.viewport_w, bridge.rhi_st.viewport_h);
    } else {
        _ = rt.beginFrame();
    }
    orig(this, render_tick) catch |e| {
        if (render_tick != 0) {
            bridge.mono_rhi_end_frame();
        } else {
            rt.endFrame();
        }
        return e;
    };
    if (render_tick != 0) {
        bridge.mono_rhi_end_frame();
    } else {
        rt.endFrame();
    }
}
