//! TouchCore native client core (Rust). The Android launcher loads this crate (`libtouchcore_native.a`, linked
//! by `tools/build_touchcore_apk.py`). It was ported from the Zig core, which has been removed.
pub mod arena;
pub mod cull;
#[cfg(target_os = "android")]
pub mod gles;
pub mod ffi;
pub mod instancing;
pub mod pool;
pub mod rhi;
pub mod thread_pool;
pub mod world;
