//! VarHandle access modes (field, static field, array element, byte-array view). Port of runtime/varhandle.zig.
//! `St` is the JVM stack type of a variable type T (byte/short/char/boolean widen to i32).
use crate::rt::{self, Arr, Obj, Ref, Res};
use std::sync::atomic::*;

pub trait Vt: Copy {
    type St: Copy;
    const IS_REF: bool = false;
    fn to_st(self) -> Self::St;
    fn from_st(s: Self::St) -> Self;
    /// # Safety
    /// `p` must be valid and aligned for reads of `Self`.
    unsafe fn load(p: *mut Self) -> Self;
    /// # Safety
    /// `p` must be valid and aligned for writes of `Self`.
    unsafe fn store(p: *mut Self, v: Self);
    /// strong CAS; returns the previous value
    /// # Safety
    /// `p` must be valid and aligned for reads and writes of `Self`.
    unsafe fn cas(p: *mut Self, e: Self, n: Self) -> Self;
    fn bits_eq(a: Self, b: Self) -> bool;
    fn add(a: Self, b: Self) -> Self;
    fn bit(op: u8, a: Self, b: Self) -> Self;
}

macro_rules! vt_int {
    ($t:ty, $st:ty, $at:ty) => {
        impl Vt for $t {
            type St = $st;
            fn to_st(self) -> $st { self as $st }
            fn from_st(s: $st) -> Self { s as $t }
            unsafe fn load(p: *mut Self) -> Self { <$at>::from_ptr(p).load(Ordering::SeqCst) }
            unsafe fn store(p: *mut Self, v: Self) { <$at>::from_ptr(p).store(v, Ordering::SeqCst) }
            unsafe fn cas(p: *mut Self, e: Self, n: Self) -> Self {
                match <$at>::from_ptr(p).compare_exchange(e, n, Ordering::SeqCst, Ordering::SeqCst) { Ok(v) => v, Err(v) => v }
            }
            fn bits_eq(a: Self, b: Self) -> bool { a == b }
            fn add(a: Self, b: Self) -> Self { a.wrapping_add(b) }
            fn bit(op: u8, a: Self, b: Self) -> Self { match op { 1 => a | b, 2 => a & b, _ => a ^ b } }
        }
    };
}
vt_int!(i8, i32, AtomicI8);
vt_int!(i16, i32, AtomicI16);
vt_int!(u16, i32, AtomicU16);
vt_int!(i32, i32, AtomicI32);
vt_int!(i64, i64, AtomicI64);

macro_rules! vt_float {
    ($t:ty, $bits:ty, $at:ty) => {
        impl Vt for $t {
            type St = $t;
            fn to_st(self) -> $t { self }
            fn from_st(s: $t) -> Self { s }
            unsafe fn load(p: *mut Self) -> Self { <$t>::from_bits(<$at>::from_ptr(p as *mut $bits).load(Ordering::SeqCst)) }
            unsafe fn store(p: *mut Self, v: Self) { <$at>::from_ptr(p as *mut $bits).store(v.to_bits(), Ordering::SeqCst) }
            unsafe fn cas(p: *mut Self, e: Self, n: Self) -> Self {
                match <$at>::from_ptr(p as *mut $bits).compare_exchange(e.to_bits(), n.to_bits(), Ordering::SeqCst, Ordering::SeqCst) {
                    Ok(v) => <$t>::from_bits(v), Err(v) => <$t>::from_bits(v),
                }
            }
            fn bits_eq(a: Self, b: Self) -> bool { a.to_bits() == b.to_bits() }
            fn add(a: Self, b: Self) -> Self { a + b }
            fn bit(_: u8, _: Self, _: Self) -> Self { rt::abort("bitwise VarHandle op on a floating point variable") }
        }
    };
}
vt_float!(f32, u32, AtomicU32);
vt_float!(f64, u64, AtomicU64);

impl Vt for Ref {
    type St = Ref;
    const IS_REF: bool = true;
    fn to_st(self) -> Ref { self }
    fn from_st(s: Ref) -> Self { s }
    unsafe fn load(p: *mut Self) -> Self { Ref(AtomicPtr::from_ptr(p as *mut *mut Obj).load(Ordering::SeqCst)) }
    unsafe fn store(p: *mut Self, v: Self) {
        crate::heap::satb_overwrite(p as *mut *mut Obj);
        AtomicPtr::from_ptr(p as *mut *mut Obj).store(v.0, Ordering::SeqCst)
    }
    unsafe fn cas(p: *mut Self, e: Self, n: Self) -> Self {
        crate::heap::satb_overwrite(p as *mut *mut Obj);
        match AtomicPtr::from_ptr(p as *mut *mut Obj).compare_exchange(e.0, n.0, Ordering::SeqCst, Ordering::SeqCst) { Ok(v) => Ref(v), Err(v) => Ref(v) }
    }
    fn bits_eq(a: Self, b: Self) -> bool { a == b }
    fn add(_: Self, _: Self) -> Self { rt::abort("getAndAdd on a reference variable") }
    fn bit(_: u8, _: Self, _: Self) -> Self { rt::abort("bitwise VarHandle op on a reference variable") }
}

#[inline(always)]
unsafe fn vh_i32(vh: Ref, off: usize) -> i32 { *((vh.0 as *mut u8).add(off) as *const i32) }
#[inline(always)]
unsafe fn vh_i64(vh: Ref, off: usize) -> i64 { *((vh.0 as *mut u8).add(off) as *const i64) }
#[inline(always)]
unsafe fn vh_i8(vh: Ref, off: usize) -> i8 { *((vh.0 as *mut u8).add(off) as *const i8) }

fn oob<R>(idx: i32, len: i32) -> Res<R> {
    rt::throw("java/lang/ArrayIndexOutOfBoundsException", format!("Index {} out of bounds for length {}", idx, len))
}

fn address<T: Vt>(vh: Ref, obj: Ref, idx: i32, nc: u8) -> Res<*mut T> {
    rt::nn(vh)?;
    unsafe {
        let h = rt::hooks();
        let kind = vh_i32(vh, h.vh_kind);
        let off = vh_i64(vh, h.vh_offset);
        match nc {
            0 => {
                if kind != 1 { return rt::throw("java/lang/UnsupportedOperationException", "VarHandle coordinates mismatch (static)".to_string()); }
                Ok(off as usize as *mut T)
            }
            1 => {
                if kind != 0 { return rt::throw("java/lang/UnsupportedOperationException", "VarHandle coordinates mismatch (field)".to_string()); }
                let o = rt::nn(obj)?;
                Ok((o as *mut u8).offset(off as isize) as *mut T)
            }
            _ => {
                if kind != 2 { return rt::throw("java/lang/UnsupportedOperationException", "VarHandle coordinates mismatch (array)".to_string()); }
                let o = rt::nn(obj)? as *mut Arr;
                if (idx as u32) >= ((*o).len as u32) { return oob(idx, (*o).len); }
                Ok((o as *mut u8).add(24).add(idx as usize * std::mem::size_of::<T>()) as *mut T)
            }
        }
    }
}

fn store_check<T: Vt>(obj: Ref, v: T, nc: u8) -> Res<()> {
    // reference arrays share one runtime class: the element type check is not modelled (see docs/MONOLITH.md)
    Ok(())
}

/// byteArrayViewVarHandle: (byte[] array, int byteIndex), unaligned, explicit byte order.
fn view_ptr<T: Vt>(vh: Ref, obj: Ref, idx: i32) -> Res<Option<*mut u8>> {
    unsafe {
        if vh_i32(vh, rt::hooks().vh_kind) != 3 { return Ok(None); }
        let a = rt::nn(obj)? as *mut Arr;
        let n = std::mem::size_of::<T>() as i64;
        if idx < 0 || idx as i64 + n > (*a).len as i64 { return oob(idx, (*a).len); }
        Ok(Some((a as *mut u8).add(24).add(idx as usize)))
    }
}
fn view_big(vh: Ref) -> bool { unsafe { vh_i8(vh, rt::hooks().vh_bigendian) != 0 } }

unsafe fn view_read<T: Vt>(p: *mut u8, big: bool) -> T {
    let n = std::mem::size_of::<T>();
    let mut buf = [0u8; 8];
    std::ptr::copy_nonoverlapping(p, buf.as_mut_ptr(), n);
    if big { buf[..n].reverse(); }
    std::ptr::read_unaligned(buf.as_ptr() as *const T)
}
unsafe fn view_write<T: Vt>(p: *mut u8, v: T, big: bool) {
    let n = std::mem::size_of::<T>();
    let mut buf = [0u8; 8];
    std::ptr::write_unaligned(buf.as_mut_ptr() as *mut T, v);
    if big { buf[..n].reverse(); }
    std::ptr::copy_nonoverlapping(buf.as_ptr(), p, n);
}

pub fn get<T: Vt>(volatile: bool, vh: Ref, obj: Ref, idx: i32, nc: u8) -> Res<T::St> {
    if nc == 2 && !T::IS_REF {
        if let Some(bp) = view_ptr::<T>(vh, obj, idx)? {
            return Ok(unsafe { view_read::<T>(bp, view_big(vh)) }.to_st());
        }
    }
    let p = address::<T>(vh, obj, idx, nc)?;
    Ok(unsafe { if volatile { T::load(p) } else { std::ptr::read(p) } }.to_st())
}
pub fn set<T: Vt>(volatile: bool, vh: Ref, obj: Ref, idx: i32, nc: u8, v: T::St) -> Res<()> {
    if nc == 2 && !T::IS_REF {
        if let Some(bp) = view_ptr::<T>(vh, obj, idx)? {
            unsafe { view_write::<T>(bp, T::from_st(v), view_big(vh)) };
            return Ok(());
        }
    }
    let p = address::<T>(vh, obj, idx, nc)?;
    let x = T::from_st(v);
    store_check::<T>(obj, x, nc)?;
    unsafe { crate::heap::satb_log_range(p as *const u8, std::mem::size_of::<T>()) };
    unsafe { if volatile { T::store(p, x) } else { std::ptr::write(p, x) } };
    Ok(())
}
pub fn cas<T: Vt>(vh: Ref, obj: Ref, idx: i32, nc: u8, e: T::St, n: T::St) -> Res<i32> {
    let p = address::<T>(vh, obj, idx, nc)?;
    let (ee, nn_) = (T::from_st(e), T::from_st(n));
    store_check::<T>(obj, nn_, nc)?;
    Ok(T::bits_eq(unsafe { T::cas(p, ee, nn_) }, ee) as i32)
}
pub fn cax<T: Vt>(vh: Ref, obj: Ref, idx: i32, nc: u8, e: T::St, n: T::St) -> Res<T::St> {
    let p = address::<T>(vh, obj, idx, nc)?;
    let nn_ = T::from_st(n);
    store_check::<T>(obj, nn_, nc)?;
    Ok(unsafe { T::cas(p, T::from_st(e), nn_) }.to_st())
}
pub fn get_and_set<T: Vt>(vh: Ref, obj: Ref, idx: i32, nc: u8, n: T::St) -> Res<T::St> {
    let p = address::<T>(vh, obj, idx, nc)?;
    let nn_ = T::from_st(n);
    store_check::<T>(obj, nn_, nc)?;
    loop {
        let old = unsafe { T::load(p) };
        if T::bits_eq(unsafe { T::cas(p, old, nn_) }, old) { return Ok(old.to_st()); }
    }
}
/// op: 0 add, 1 or, 2 and, 3 xor
pub fn get_and_op<T: Vt>(op: u8, vh: Ref, obj: Ref, idx: i32, nc: u8, d: T::St) -> Res<T::St> {
    let p = address::<T>(vh, obj, idx, nc)?;
    let dv = T::from_st(d);
    loop {
        let old = unsafe { T::load(p) };
        let new = if op == 0 { T::add(old, dv) } else { T::bit(op, old, dv) };
        if T::bits_eq(unsafe { T::cas(p, old, new) }, old) { return Ok(old.to_st()); }
    }
}

fn seg_ptr<T: Vt>(addr: i64) -> Res<*mut T> {
    if T::IS_REF { return rt::throw("java/lang/UnsupportedOperationException", "MemorySegment VarHandle object access".to_string()); }
    if addr == 0 { return rt::throw("java/lang/NullPointerException", "null MemorySegment address".to_string()); }
    Ok(addr as usize as *mut T)
}
pub fn get_seg<T: Vt>(volatile: bool, addr: i64) -> Res<T::St> {
    let p = seg_ptr::<T>(addr)?;
    Ok(unsafe { if volatile { T::load(p) } else { std::ptr::read_unaligned(p) } }.to_st())
}
pub fn set_seg<T: Vt>(volatile: bool, addr: i64, v: T::St) -> Res<()> {
    let p = seg_ptr::<T>(addr)?;
    let x = T::from_st(v);
    unsafe { if volatile { T::store(p, x) } else { std::ptr::write_unaligned(p, x) } };
    Ok(())
}
pub fn cas_seg<T: Vt>(addr: i64, e: T::St, n: T::St) -> Res<i32> {
    let p = seg_ptr::<T>(addr)?;
    let (ee, nn_) = (T::from_st(e), T::from_st(n));
    Ok(T::bits_eq(unsafe { T::cas(p, ee, nn_) }, ee) as i32)
}
pub fn cax_seg<T: Vt>(addr: i64, e: T::St, n: T::St) -> Res<T::St> {
    let p = seg_ptr::<T>(addr)?;
    Ok(unsafe { T::cas(p, T::from_st(e), T::from_st(n)) }.to_st())
}
pub fn get_and_set_seg<T: Vt>(addr: i64, n: T::St) -> Res<T::St> {
    let p = seg_ptr::<T>(addr)?;
    let nn_ = T::from_st(n);
    loop {
        let old = unsafe { T::load(p) };
        if T::bits_eq(unsafe { T::cas(p, old, nn_) }, old) { return Ok(old.to_st()); }
    }
}
pub fn get_and_op_seg<T: Vt>(op: u8, addr: i64, d: T::St) -> Res<T::St> {
    let p = seg_ptr::<T>(addr)?;
    let dv = T::from_st(d);
    loop {
        let old = unsafe { T::load(p) };
        let new = if op == 0 { T::add(old, dv) } else { T::bit(op, old, dv) };
        if T::bits_eq(unsafe { T::cas(p, old, new) }, old) { return Ok(old.to_st()); }
    }
}
