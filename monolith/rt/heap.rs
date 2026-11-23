//! Heap facade. The runtime allocates, runs write barriers and collects only through this module.
//! It currently forwards to the tracing collector in `gc`; the collector can be replaced behind this
//! module without touching the natives, the generated code or the class model.
pub use crate::gc::*;
