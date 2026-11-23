//! API for hand-written patches (`patches/rust/*.rs`).
//!
//! A patch replaces one translated method with Rust code. It is compiled into the shard of the class it patches,
//! inside a module that already holds the bindings the manifest asked for (field offsets, statics, class
//! references, callable methods). This module holds the parts that do not depend on the program: raw field access,
//! array views, and the `MONO_PATCH_VERIFY` self-check.
use crate::rt::{self, Obj, Ref};
use std::sync::OnceLock;

/// True when `MONO_PATCH_VERIFY` is set: every patched method that returns a primitive runs the original and the
/// patch, compares the raw bits and aborts on the first difference.
pub fn verify() -> bool {
    static V: OnceLock<bool> = OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MONO_PATCH_VERIFY").is_some_and(|v| !v.is_empty() && v != "0"))
}

pub fn mismatch(what: &str) -> ! {
    rt::abort(&format!("patch mismatch in {what}: the patch and the translated original disagree"))
}

/// Equality on the raw bits (so NaNs and -0.0 are compared exactly like the Java result).
pub trait Same {
    fn same(&self, other: &Self) -> bool;
}
macro_rules! same_int { ($($t:ty),*) => { $(impl Same for $t { #[inline] fn same(&self, o: &Self) -> bool { *self == *o } })* } }
same_int!(i32, i64, i8, i16, u16);
impl Same for f32 { #[inline] fn same(&self, o: &Self) -> bool { self.to_bits() == o.to_bits() } }
impl Same for f64 { #[inline] fn same(&self, o: &Self) -> bool { self.to_bits() == o.to_bits() } }

/// Reads a field of `o` at byte offset `off` (offsets come from the manifest bindings: `F_<name>`).
/// Booleans, bytes, chars and shorts are stored in their own width, references as `Ref`.
///
/// # Safety
/// `o` must be a non-null object that has a field of type `T` at `off`.
#[inline(always)]
pub unsafe fn get<T: Copy>(o: Ref, off: usize) -> T {
    *((o.0 as *const u8).add(off) as *const T)
}

/// # Safety
/// Same contract as [`get`]. Reference stores need no write barrier: the collector is non-moving and scans stacks and
/// heap conservatively.
#[inline(always)]
pub unsafe fn set<T>(o: Ref, off: usize, v: T) {
    let p = (o.0 as *mut u8).add(off);
    // a patch may store a reference: the overwritten bytes are logged for the SATB marker (cheap when no cycle runs)
    crate::gc::satb_log_range(p as *const u8, std::mem::size_of::<T>());
    *(p as *mut T) = v;
}

/// The class pointer of an object, for identity checks against a `class_<name>()` binding.
///
/// # Safety
/// `o` must be non-null.
#[inline(always)]
pub unsafe fn class_ptr(o: Ref) -> *const rt::Class {
    (*(o.0 as *const Obj)).class as *const rt::Class
}

/// Length of an array object.
///
/// # Safety
/// `a` must be a non-null array.
#[inline(always)]
pub unsafe fn array_len(a: Ref) -> i32 {
    (*(a.0 as *const rt::Arr)).len
}

/// The elements of an array object as a slice.
///
/// # Safety
/// `a` must be a non-null array whose element type is `T` (`i64` for `long[]`, `i32` for `int[]`, `Ref` for
/// object arrays...).
#[inline(always)]
pub unsafe fn array<'a, T>(a: Ref) -> &'a mut [T] {
    std::slice::from_raw_parts_mut((a.0 as *mut u8).add(std::mem::size_of::<rt::Arr>()) as *mut T, array_len(a) as usize)
}
