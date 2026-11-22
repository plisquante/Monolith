//! LevelRenderer / World Frame Render Hooks (`MONO_RHI=1` or `MONO_NO_LWJGL=1` opt-in).
//!
//! Safety contract:
//!   * `"orig": true` — delegates directly to translated vanilla `LevelRenderer` so game ticks,
//!     redstone, physics, and server state stay 100% untouched.
//!   * Clears cached section snapshots on world reload (`onAllChanged`), keeps RHI viewport
//!     dimensions synchronized (`onResize`), and advances quiescent SlabPool epoch reclamation
//!     on client world ticks (`onClientTick`). (Chunk setup itself allocates persistent world
//!     objects and intentionally stays outside transient `ArenaScope` rewinds.)
const std = @import("std");
const rt = @import("rt.zig");
const Ref = rt.Ref;

const bridge = @import("rhi_bridge.zig");

fn rhiActive() bool {
    if (rt.getenv("MONO_NO_LWJGL")) |nl| {
        if (nl.len > 0 and nl[0] != '0') return true;
    }
    const v = rt.getenv("MONO_RHI") orelse return false;
    return v.len > 0 and v[0] != '0';
}

/// net.minecraft.client.renderer.LevelRenderer.setupRender(...) / cullTerrain(...) bracket:
/// clears stale frame sections before vanilla traversal rebuilds visible section list.
pub fn onSetupRender(orig: anytype, this: Ref, a0: Ref, a1: Ref, a2: i32, a3: i32) rt.E!void {
    if (!rhiActive()) return orig(this, a0, a1, a2, a3);
    bridge.rhi_st.lock.lock();
    bridge.rhi_st.sections.clearRetainingCapacity();
    bridge.rhi_st.lock.unlock();
    return orig(this, a0, a1, a2, a3);
}

/// net.minecraft.client.renderer.LevelRenderer.needsUpdate()V — notification hook without allocating.
pub fn onAllChanged(orig: anytype, this: Ref) rt.E!void {
    if (rhiActive()) {
        bridge.rhi_st.lock.lock();
        bridge.rhi_st.sections.clearRetainingCapacity();
        bridge.rhi_st.lock.unlock();
    }
    return orig(this);
}

/// net.minecraft.client.renderer.GameRenderer.resize(II)V — keeps RHI viewport synchronized.
pub fn onResize(orig: anytype, this: Ref, w: i32, h: i32) rt.E!void {
    if (rhiActive()) {
        bridge.J_mono_rhi_NativeRhi__glViewport0(0, 0, w, h) catch {};
    }
    return orig(this, w, h);
}

/// net.minecraft.client.multiplayer.ClientLevel.tick(BooleanSupplier)V — advances epoch
/// reclamation every client world tick so non-render retired buffers do not accumulate.
pub fn onClientTick(orig: anytype, this: Ref, has_time_left: Ref) rt.E!void {
    if (!rhiActive()) return orig(this, has_time_left);
    _ = rt.gc();
    return orig(this, has_time_left);
}
