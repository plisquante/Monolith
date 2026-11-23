//! TouchCore native client core (Rust). Port of the Zig core that the Android launcher loads; replaces
//! `android/touchcore/jni/touchcore_core.zig` and its runtime modules piece by piece.
pub mod arena;
pub mod pool;
pub mod thread_pool;
