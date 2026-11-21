//! Monolith çekirdeği: vanilla ile birebir aynı olması gereken saf hesaplamalar.
//!
//! Bu crate `unsafe` içermez. Yalnızca bellek-güvenli Rust kullanır.
#![forbid(unsafe_code)]

pub mod random;

pub use random::JavaRandom;
