//! TouchCore yerel istemci çekirdeği (Rust). Android başlatıcı bu crate'i yükler (`libtouchcore_native.a`,
//! `tools/build_touchcore_apk.py` ile bağlanır). Kaldırılmış olan Zig çekirdeğinden taşındı.
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
