//! TouchCore native client core (Rust). Port of the Zig core that the Android launcher loads; replaces
//! `android/touchcore/jni/touchcore_core.zig` and its runtime modules piece by piece.
pub mod arena;
pub mod cull;
pub mod instancing;
pub mod pool;
pub mod thread_pool;
pub mod world;
