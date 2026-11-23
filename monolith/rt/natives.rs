//! Native methods of the class library overlay. Value types follow the JVM stack: boolean/byte/char/short/int are i32.
//! Names: `n_<class>__<method>` (class with `/` -> `_`);
//! overloaded natives of one class get a `__<desc hash>` suffix, shown by the generator in its missing-natives list.
#![allow(dead_code, non_snake_case, unused_imports)]
use crate::rt::{self, Ref, Res, Class, Obj, Arr, StrObj};
use crate::*;
use std::io::Write;

pub use crate::natives_reflect::*;
pub use crate::natives_io::*;
pub use crate::natives_nio::*;
pub use crate::natives_net::*;
pub use crate::natives_sys::*;
pub use crate::natives_jni::*;
pub use crate::natives_rhi::*;

fn arr_data(a: Ref) -> *mut u8 {
    unsafe { (a.0 as *mut u8).add(24) }
}

// ------------------------------------------------------------------ java.lang.Object
pub fn n_java_lang_Object__getClass(this: Ref) -> Res<Ref> {
    let p = rt::nn(this)?;
    rt::class_mirror(unsafe { (*p).class })
}
static HASH_SEED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x2545_F491);
pub fn identity_hash(r: Ref) -> i32 {
    if r.is_null() {
        return 0;
    }
    unsafe {
        let o = r.0;
        if (*o).hash == 0 {
            let mut x = HASH_SEED.load(std::sync::atomic::Ordering::Relaxed);
            loop {
                let mut y = x;
                y ^= y << 13;
                y ^= y >> 17;
                y ^= y << 5;
                if y == 0 {
                    y = 1;
                }
                match HASH_SEED.compare_exchange(x, y, std::sync::atomic::Ordering::Relaxed, std::sync::atomic::Ordering::Relaxed) {
                    Ok(_) => {
                        (*o).hash = y & 0x7fff_ffff | 1;
                        break;
                    }
                    Err(c) => x = c,
                }
            }
        }
        (*o).hash as i32
    }
}
pub fn n_java_lang_Object__hashCode(this: Ref) -> Res<i32> {
    rt::nn(this)?;
    Ok(identity_hash(this))
}
pub fn n_java_lang_Object__clone(this: Ref) -> Res<Ref> {
    let p = rt::nn(this)?;
    let c = unsafe { (*p).class };
    if c.elem != 0 {
        return rt::array_clone(this);
    }
    let r = rt::alloc(c);
    unsafe { std::ptr::copy_nonoverlapping((p as *const u8).add(16), (r.0 as *mut u8).add(16), c.size - 16) };
    Ok(r)
}

// ------------------------------------------------------------------ System / Runtime
pub fn n_java_lang_System__arraycopy(src: Ref, sp: i32, dst: Ref, dp: i32, n: i32) -> Res<()> {
    let s = rt::nn(src)? as *mut Arr;
    let d = rt::nn(dst)? as *mut Arr;
    unsafe {
        let (sc, dc) = ((*s).hdr.class, (*d).hdr.class);
        if sc.elem == 0 || dc.elem == 0 {
            return rt::throw("java/lang/ArrayStoreException", "arraycopy: source type is not an array".to_string());
        }
        if (sc.elem == b'A') != (dc.elem == b'A') || (sc.elem != b'A' && sc.elem != dc.elem) {
            return rt::throw("java/lang/ArrayStoreException", "arraycopy: type mismatch".to_string());
        }
        if sp < 0 || dp < 0 || n < 0 || sp as i64 + n as i64 > (*s).len as i64 || dp as i64 + n as i64 > (*d).len as i64 {
            return rt::throw("java/lang/ArrayIndexOutOfBoundsException", format!("arraycopy: last source index {} out of bounds for length {}", sp as i64 + n as i64, (*s).len));
        }
        if sc.elem == b'A' {
            if let (Some(se), Some(de)) = (sc.elem_class, dc.elem_class) {
                if !rt::is_subclass(se, de) {
                    let sptr = arr_data(src) as *const Ref;
                    let dptr = arr_data(dst) as *mut Ref;
                    // the overwritten elements are snapshot values (SATB)
                    crate::heap::satb_log_range(dptr.add(dp as usize) as *const u8, n as usize * std::mem::size_of::<Ref>());
                    let same = src.0 == dst.0;
                    for k in 0..n as usize {
                        let i = if same && sp < dp { n as usize - 1 - k } else { k };
                        let v = *sptr.add(sp as usize + i);
                        if !v.is_null() && !rt::is_subclass((*v.0).class, de) {
                            return rt::throw("java/lang/ArrayStoreException", format!("arraycopy: element type {} is not compatible with {}", (*v.0).class.name.replace('/', "."), de.name.replace('/', ".")));
                        }
                        *dptr.add(dp as usize + i) = v;
                    }
                    return Ok(());
                }
            }
        }
        let es = rt::elem_size(sc.elem);
        if sc.elem == b'A' {
            // reference array: the overwritten elements are snapshot values (SATB)
            crate::heap::satb_log_range((arr_data(dst)).add(dp as usize * es), n as usize * es);
        }
        std::ptr::copy((arr_data(src)).add(sp as usize * es), (arr_data(dst)).add(dp as usize * es), n as usize * es);
    }
    Ok(())
}
pub fn n_java_lang_System__currentTimeMillis() -> Res<i64> {
    Ok(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0))
}
pub fn n_java_lang_System__nanoTime() -> Res<i64> {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    Ok(START.get_or_init(std::time::Instant::now).elapsed().as_nanos() as i64 + 1_000_000_000)
}
pub fn n_java_lang_System__identityHashCode(o: Ref) -> Res<i32> {
    Ok(identity_hash(o))
}
pub fn n_java_lang_System__write0(fd: i32, b: Ref, off: i32, len: i32) -> Res<()> {
    let a = rt::nn(b)? as *mut Arr;
    unsafe {
        if off < 0 || len < 0 || off as i64 + len as i64 > (*a).len as i64 {
            return rt::throw("java/lang/ArrayIndexOutOfBoundsException", format!("write0 {} {}", off, len));
        }
        let data = std::slice::from_raw_parts(arr_data(b).add(off as usize), len as usize);
        if fd == 2 {
            let mut e = std::io::stderr().lock();
            let _ = e.write_all(data);
            let _ = e.flush();
        } else {
            let mut o = std::io::stdout().lock();
            let _ = o.write_all(data);
            let _ = o.flush();
        }
    }
    Ok(())
}
pub fn n_java_lang_System__getenv0(name: Ref) -> Res<Ref> {
    let n = rt::str_to_rust(name);
    Ok(match std::env::var(&n) {
        Ok(v) => rt::new_str(v),
        Err(_) => Ref::NULL,
    })
}
pub fn n_java_lang_Runtime__availableProcessors(this: Ref) -> Res<i32> {
    Ok(std::thread::available_parallelism().map(|n| n.get() as i32).unwrap_or(1))
}
pub fn n_java_lang_Runtime__maxMemory(this: Ref) -> Res<i64> {
    Ok(i64::MAX)
}

// ------------------------------------------------------------------ numerics
pub fn n_java_lang_Double__doubleToRawLongBits(v: f64) -> Res<i64> {
    Ok(v.to_bits() as i64)
}
pub fn n_java_lang_Float__floatToRawIntBits(v: f32) -> Res<i32> {
    Ok(v.to_bits() as i32)
}

// ------------------------------------------------------------------ Throwable / threads
pub fn n_java_lang_Throwable__backtrace0(this: Ref) -> Res<Ref> {
    Ok(Ref::NULL)
}
pub fn n_java_lang_NullPointerException__getExtendedNPEMessage(this: Ref) -> Res<Ref> {
    Ok(Ref::NULL)
}

// ------------------------------------------------------------------ java.lang.Class
pub fn n_java_lang_Class__getName0(this: Ref) -> Res<Ref> {
    let c = rt::class_of_mirror(rt::nn_ref(this)?);
    Ok(rt::new_str(c.name.replace('/', ".")))
}
pub fn n_java_lang_Class__kind0(this: Ref) -> Res<i32> {
    Ok(rt::class_of_mirror(rt::nn_ref(this)?).kind as i32)
}
pub fn n_java_lang_Class__isBoot0(this: Ref) -> Res<i32> {
    let c = rt::class_of_mirror(rt::nn_ref(this)?);
    let n = c.name;
    let boot = n.starts_with("java/")
        || n.starts_with("javax/")
        || n.starts_with("sun/")
        || n.starts_with("jdk/")
        || n.starts_with("com/sun/crypto/")
        || n.starts_with("org/ietf/jgss/")
        || n.starts_with("org/w3c/dom/")
        || n.starts_with("org/xml/sax/");
    Ok(boot as i32)
}
pub fn n_java_lang_Class__isAssignableFrom0(this: Ref, other: Ref) -> Res<i32> {
    let a = rt::class_of_mirror(rt::nn_ref(this)?);
    let b = rt::class_of_mirror(rt::nn_ref(other)?);
    Ok(rt::is_subclass(b, a) as i32)
}
pub fn n_java_lang_Class__getPrimitiveClass(name: Ref) -> Res<Ref> {
    match rt::prim_class(&rt::str_to_rust(name)) {
        Some(c) => rt::class_mirror(c),
        None => rt::throw("java/lang/ClassNotFoundException", rt::str_to_rust(name)),
    }
}
pub fn n_java_lang_Class__component0(this: Ref) -> Res<Ref> {
    let c = rt::class_of_mirror(rt::nn_ref(this)?);
    if let Some(ec) = c.elem_class {
        return rt::class_mirror(ec);
    }
    Ok(match c.elem {
        0 => Ref::NULL,
        b'Z' => rt::class_mirror(&rt::CLS_P_BOOLEAN)?,
        b'B' => rt::class_mirror(&rt::CLS_P_BYTE)?,
        b'C' => rt::class_mirror(&rt::CLS_P_CHAR)?,
        b'S' => rt::class_mirror(&rt::CLS_P_SHORT)?,
        b'I' => rt::class_mirror(&rt::CLS_P_INT)?,
        b'J' => rt::class_mirror(&rt::CLS_P_LONG)?,
        b'F' => rt::class_mirror(&rt::CLS_P_FLOAT)?,
        b'D' => rt::class_mirror(&rt::CLS_P_DOUBLE)?,
        _ => rt::class_mirror(c.super_.unwrap())?,
    })
}

// ------------------------------------------------------------------ jdk.internal.misc.Unsafe
pub fn n_jdk_internal_misc_Unsafe__registerNatives() -> Res<()> {
    Ok(())
}
pub fn n_jdk_internal_misc_ScopedMemoryAccess__registerNatives() -> Res<()> {
    Ok(())
}
fn addr(o: Ref, off: i64) -> *mut u8 {
    if o.is_null() { off as *mut u8 } else { unsafe { (o.0 as *mut u8).offset(off as isize) } }
}
pub fn n_jdk_internal_misc_Unsafe__arrayBaseOffset0(this: Ref, c: Ref) -> Res<i32> {
    Ok(24)
}
pub fn n_jdk_internal_misc_Unsafe__arrayIndexScale0(this: Ref, c: Ref) -> Res<i32> {
    let k = rt::class_of_mirror(rt::nn_ref(c)?);
    Ok(rt::elem_size(k.elem) as i32)
}
pub fn n_jdk_internal_misc_Unsafe__objectFieldOffset1(this: Ref, c: Ref, name: Ref) -> Res<i64> {
    let n = rt::str_to_rust(name);
    match rt::class_of_mirror(rt::nn_ref(c)?).find_field(&n, false) {
        Some((_, f)) => match f.loc {
            rt::FLoc::Off(o) => Ok(o as i64),
            _ => unreachable!(),
        },
        None => rt::throw("java/lang/InternalError", n),
    }
}
pub fn n_jdk_internal_misc_Unsafe__getByte(this: Ref, o: Ref, off: i64) -> Res<i32> {
    Ok(unsafe { *(addr(o, off) as *const i8) } as i32)
}
pub fn n_jdk_internal_misc_Unsafe__getShort(this: Ref, o: Ref, off: i64) -> Res<i32> {
    Ok(unsafe { (addr(o, off) as *const i16).read_unaligned() } as i32)
}
pub fn n_jdk_internal_misc_Unsafe__getInt(this: Ref, o: Ref, off: i64) -> Res<i32> {
    Ok(unsafe { (addr(o, off) as *const i32).read_unaligned() })
}
pub fn n_jdk_internal_misc_Unsafe__getLong(this: Ref, o: Ref, off: i64) -> Res<i64> {
    Ok(unsafe { (addr(o, off) as *const i64).read_unaligned() })
}
pub fn n_jdk_internal_misc_Unsafe__getReferenceVolatile(this: Ref, o: Ref, off: i64) -> Res<Ref> {
    Ok(unsafe { (addr(o, off) as *const Ref).read() })
}

// ------------------------------------------------------------------ misc
pub fn n_java_util_concurrent_atomic_AtomicLong__cas(this: Ref, expected: i64, new: i64) -> Res<i32> {
    let p = rt::nn(this)?;
    let a = unsafe { &*((p as *mut u8).add(16) as *const std::sync::atomic::AtomicI64) };
    Ok(a.compare_exchange(expected, new, std::sync::atomic::Ordering::SeqCst, std::sync::atomic::Ordering::SeqCst).is_ok() as i32)
}
pub fn n_java_lang_reflect_Array__newArray(c: Ref, n: i32) -> Res<Ref> {
    let k = rt::class_of_mirror(rt::nn_ref(c)?);
    if k.kind == 3 {
        let e = match k.name {
            "boolean" => b'Z', "byte" => b'B', "char" => b'C', "short" => b'S',
            "int" => b'I', "long" => b'J', "float" => b'F', "double" => b'D',
            _ => b'A',
        };
        rt::new_array(e, n)
    } else {
        rt::new_obj_array(k, n)
    }
}

// ------------------------------------------------------------------ Unsafe: raw memory
macro_rules! unsafe_put {
    ($name:ident, $t:ty, $arg:ty) => {
        pub fn $name(this: Ref, o: Ref, off: i64, v: $arg) -> Res<()> {
            unsafe { (addr(o, off) as *mut $t).write_unaligned(v as $t) };
            Ok(())
        }
    };
}
unsafe_put!(n_jdk_internal_misc_Unsafe__putByte, i8, i32);
unsafe_put!(n_jdk_internal_misc_Unsafe__putBoolean, i8, i32);
unsafe_put!(n_jdk_internal_misc_Unsafe__putShort, i16, i32);
unsafe_put!(n_jdk_internal_misc_Unsafe__putChar, u16, i32);
unsafe_put!(n_jdk_internal_misc_Unsafe__putInt, i32, i32);
unsafe_put!(n_jdk_internal_misc_Unsafe__putLong, i64, i64);
unsafe_put!(n_jdk_internal_misc_Unsafe__putFloat, f32, f32);
unsafe_put!(n_jdk_internal_misc_Unsafe__putDouble, f64, f64);
unsafe_put!(n_jdk_internal_misc_Unsafe__putIntVolatile, i32, i32);
pub fn n_jdk_internal_misc_Unsafe__putReference(this: Ref, o: Ref, off: i64, v: Ref) -> Res<()> {
    unsafe { crate::heap::satb_store(addr(o, off) as *mut Ref, v) };
    Ok(())
}
pub fn n_jdk_internal_misc_Unsafe__putReferenceVolatile(this: Ref, o: Ref, off: i64, v: Ref) -> Res<()> {
    unsafe {
        crate::heap::satb_overwrite(addr(o, off) as *mut *mut Obj);
        std::sync::atomic::AtomicPtr::from_ptr(addr(o, off) as *mut *mut Obj).store(v.0, std::sync::atomic::Ordering::SeqCst)
    };
    Ok(())
}
pub fn n_jdk_internal_misc_Unsafe__getReference(this: Ref, o: Ref, off: i64) -> Res<Ref> {
    Ok(unsafe { (addr(o, off) as *const Ref).read() })
}
pub fn n_jdk_internal_misc_Unsafe__getIntVolatile(this: Ref, o: Ref, off: i64) -> Res<i32> {
    Ok(unsafe { std::sync::atomic::AtomicI32::from_ptr(addr(o, off) as *mut i32).load(std::sync::atomic::Ordering::SeqCst) })
}
pub fn n_jdk_internal_misc_Unsafe__getLongVolatile(this: Ref, o: Ref, off: i64) -> Res<i64> {
    Ok(unsafe { std::sync::atomic::AtomicI64::from_ptr(addr(o, off) as *mut i64).load(std::sync::atomic::Ordering::SeqCst) })
}
pub fn n_jdk_internal_misc_Unsafe__compareAndSetInt(this: Ref, o: Ref, off: i64, e: i32, n: i32) -> Res<i32> {
    Ok(unsafe { std::sync::atomic::AtomicI32::from_ptr(addr(o, off) as *mut i32).compare_exchange(e, n, std::sync::atomic::Ordering::SeqCst, std::sync::atomic::Ordering::SeqCst).is_ok() } as i32)
}
pub fn n_jdk_internal_misc_Unsafe__compareAndSetLong(this: Ref, o: Ref, off: i64, e: i64, n: i64) -> Res<i32> {
    Ok(unsafe { std::sync::atomic::AtomicI64::from_ptr(addr(o, off) as *mut i64).compare_exchange(e, n, std::sync::atomic::Ordering::SeqCst, std::sync::atomic::Ordering::SeqCst).is_ok() } as i32)
}
pub fn n_jdk_internal_misc_Unsafe__compareAndExchangeLong(this: Ref, o: Ref, off: i64, e: i64, n: i64) -> Res<i64> {
    Ok(unsafe { match std::sync::atomic::AtomicI64::from_ptr(addr(o, off) as *mut i64).compare_exchange(e, n, std::sync::atomic::Ordering::SeqCst, std::sync::atomic::Ordering::SeqCst) { Ok(v) => v, Err(v) => v } })
}
pub fn n_jdk_internal_misc_Unsafe__compareAndSetReference(this: Ref, o: Ref, off: i64, e: Ref, n: Ref) -> Res<i32> {
    Ok(unsafe {
        crate::heap::satb_overwrite(addr(o, off) as *mut *mut Obj);
        std::sync::atomic::AtomicPtr::from_ptr(addr(o, off) as *mut *mut Obj).compare_exchange(e.0, n.0, std::sync::atomic::Ordering::SeqCst, std::sync::atomic::Ordering::SeqCst).is_ok()
    } as i32)
}
pub fn n_jdk_internal_misc_Unsafe__compareAndExchangeReference(this: Ref, o: Ref, off: i64, e: Ref, n: Ref) -> Res<Ref> {
    Ok(unsafe {
        crate::heap::satb_overwrite(addr(o, off) as *mut *mut Obj);
        match std::sync::atomic::AtomicPtr::from_ptr(addr(o, off) as *mut *mut Obj).compare_exchange(e.0, n.0, std::sync::atomic::Ordering::SeqCst, std::sync::atomic::Ordering::SeqCst) { Ok(v) => Ref(v), Err(v) => Ref(v) }
    })
}
pub fn n_jdk_internal_misc_Unsafe__fullFence(this: Ref) -> Res<()> {
    std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
    Ok(())
}
pub fn n_jdk_internal_misc_Unsafe__copyMemory0(this: Ref, so: Ref, soff: i64, d: Ref, doff: i64, n: i64) -> Res<()> {
    unsafe { crate::heap::satb_log_range(addr(d, doff) as *const u8, n as usize) };
    unsafe { std::ptr::copy(addr(so, soff), addr(d, doff), n as usize) };
    Ok(())
}
pub fn n_jdk_internal_misc_Unsafe__ensureClassInitialized0(this: Ref, c: Ref) -> Res<()> {
    n_java_lang_Class__ensureInit(c)
}
pub fn n_java_lang_invoke_VarHandle__fence0() -> Res<()> {
    std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
    Ok(())
}
pub fn n_java_lang_invoke_MethodHandles__fieldOffset0(c: Ref, name: Ref, is_static: i32) -> Res<i64> {
    let n = rt::str_to_rust(name);
    let k = rt::class_of_mirror(rt::nn_ref(c)?);
    match k.find_field(&n, is_static != 0) {
        Some((owner, f)) => match &f.loc {
            rt::FLoc::Off(o) => Ok(*o as i64),
            rt::FLoc::Static(p) => {
                if let Some(i) = owner.init { i().res()?; }
                Ok(p.0 as i64)
            }
        },
        None => Ok(-1),
    }
}
pub fn n_java_lang_invoke_MethodHandles__fieldIsFinal0(c: Ref, name: Ref, is_static: i32) -> Res<i32> {
    let n = rt::str_to_rust(name);
    let k = rt::class_of_mirror(rt::nn_ref(c)?);
    Ok(match k.find_field(&n, is_static != 0) { Some((_, f)) => (f.access & 0x10 != 0) as i32, None => 0 })
}

// ------------------------------------------------------------------ Class: init, enums
pub fn n_java_lang_Class__ensureInit(this: Ref) -> Res<()> {
    let c = rt::class_of_mirror(rt::nn_ref(this)?);
    if let Some(i) = c.init { i().res()?; }
    Ok(())
}
pub fn n_java_lang_Class__enumValues0(this: Ref) -> Res<Ref> {
    let c = rt::class_of_mirror(rt::nn_ref(this)?);
    match c.enum_values {
        Some(f) => f().res(),
        None => Ok(Ref::NULL),
    }
}

// ------------------------------------------------------------------ misc small
pub fn n_java_lang_Float__intBitsToFloat(v: i32) -> Res<f32> {
    Ok(f32::from_bits(v as u32))
}
pub fn n_java_lang_Double__longBitsToDouble(v: i64) -> Res<f64> {
    Ok(f64::from_bits(v as u64))
}
pub fn n_java_util_concurrent_atomic_AtomicInteger__cas(this: Ref, expected: i32, new: i32) -> Res<i32> {
    let p = rt::nn(this)?;
    let a = unsafe { &*((p as *mut u8).add(16) as *const std::sync::atomic::AtomicI32) };
    Ok(a.compare_exchange(expected, new, std::sync::atomic::Ordering::SeqCst, std::sync::atomic::Ordering::SeqCst).is_ok() as i32)
}
pub fn n_java_lang_String__intern(this: Ref) -> Res<Ref> {
    rt::nn(this)?;
    Ok(rt::intern_string(this))
}

// ------------------------------------------------------------------ monitors and threads (sync.rs)
use crate::sync;
pub fn n_java_lang_Object__notify(this: Ref) -> Res<()> { sync::notify(this, false) }
pub fn n_java_lang_Object__notifyAll(this: Ref) -> Res<()> { sync::notify(this, true) }
pub fn n_java_lang_Object__wait0(this: Ref, ms: i64, ns: i32) -> Res<()> { sync::wait(this, ms, ns) }
pub fn n_java_lang_Thread__currentThread0() -> Res<Ref> { Ok(sync::current_thread()) }
pub fn n_java_lang_Thread__yield() -> Res<()> { std::thread::yield_now(); Ok(()) }
pub fn n_java_lang_Thread__sleep0(nanos: i64) -> Res<()> { sync::sleep(nanos) }
pub fn n_java_lang_Thread__start0(this: Ref, stack: i64) -> Res<()> { sync::start(this, stack) }
pub fn n_java_lang_Thread__interrupt0(this: Ref) -> Res<()> { sync::interrupt(this) }
pub fn n_java_lang_Thread__clearInterrupt0(this: Ref) -> Res<i32> { Ok(sync::is_interrupted(this, true)? as i32) }
pub fn n_java_lang_Thread__isInterrupted0(this: Ref) -> Res<i32> { Ok(sync::is_interrupted(this, false)? as i32) }
pub fn n_java_lang_Thread__holdsLock(o: Ref) -> Res<i32> { Ok(sync::holds_lock(o)? as i32) }
pub fn n_jdk_internal_misc_Unsafe__park(this: Ref, absolute: i32, time: i64) -> Res<()> { sync::park(absolute != 0, time); Ok(()) }
pub fn n_jdk_internal_misc_Unsafe__unpark(this: Ref, t: Ref) -> Res<()> { sync::unpark(t); Ok(()) }

// ------------------------------------------------------------------ System properties, Runtime
pub fn n_java_lang_System__cwd0() -> Res<Ref> {
    Ok(rt::new_str(std::env::current_dir().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| "/".into())))
}
pub fn n_java_lang_System__arch0() -> Res<Ref> {
    Ok(rt::new_str(match std::env::consts::ARCH { "x86_64" => "amd64", "x86" => "i386", a => a }.to_string()))
}
pub fn n_java_lang_System__exeDir0() -> Res<Ref> {
    let d = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_string_lossy().into_owned())).unwrap_or_else(|| "/".into());
    Ok(rt::new_str(d))
}
pub fn n_java_lang_System__userProps0() -> Res<Ref> {
    rt::string_array(&rt::user_props())
}
pub fn n_java_lang_Runtime__halt0(status: i32) -> Res<()> {
    std::process::exit(status)
}
pub fn n_java_lang_Runtime__freeMemory(this: Ref) -> Res<i64> {
    let h = crate::heap::heap_bytes().max(crate::heap::live_bytes()) as i64;
    let l = crate::heap::live_bytes() as i64;
    Ok((h - l).max(1 << 20))
}
pub fn n_java_lang_Runtime__totalMemory(this: Ref) -> Res<i64> {
    Ok((crate::heap::heap_bytes().max(32 << 20)) as i64)
}
pub fn n_java_lang_Runtime__gc(this: Ref) -> Res<()> {
    crate::heap::collect_explicit();
    Ok(())
}
pub fn n_java_security_SecureRandom__fill0(b: Ref) -> Res<()> {
    use std::io::Read;
    let a = rt::nn(b)? as *mut Arr;
    let n = unsafe { (*a).len } as usize;
    let buf = unsafe { std::slice::from_raw_parts_mut(arr_data(b), n) };
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(buf);
    }
    Ok(())
}
pub fn n_java_io_ObjectStreamClass__initNative() -> Res<()> { Ok(()) }

// ------------------------------------------------------------------ Reflection basics, Class queries
pub fn n_jdk_internal_reflect_Reflection__getCallerClass() -> Res<Ref> {
    match rt::caller() {
        Some(c) => rt::class_mirror(c),
        None => Ok(Ref::NULL),
    }
}
pub fn n_jdk_internal_reflect_Reflection__getClassAccessFlags(c: Ref) -> Res<i32> {
    Ok(rt::class_of_mirror(rt::nn_ref(c)?).access as i32)
}
pub fn n_jdk_internal_reflect_Reflection__areNestMates(a: Ref, b: Ref) -> Res<i32> { Ok(1) }
pub fn n_java_lang_Class__forName0(name: Ref) -> Res<Ref> {
    match rt::find_class(&rt::str_to_rust(rt::nn_ref(name)?)) {
        Some(c) => rt::class_mirror(c),
        None => Ok(Ref::NULL),
    }
}
pub fn n_java_lang_Class__isInstance0(this: Ref, o: Ref) -> Res<i32> {
    Ok(rt::instance_of(o, rt::class_of_mirror(rt::nn_ref(this)?)) as i32)
}
pub fn n_java_lang_Class__super0(this: Ref) -> Res<Ref> {
    let c = rt::class_of_mirror(rt::nn_ref(this)?);
    if c.kind == 1 { return Ok(Ref::NULL); }
    match c.super_ { Some(s) => rt::class_mirror(s), None => Ok(Ref::NULL) }
}
pub fn n_java_lang_Class__interfaces0(this: Ref) -> Res<Ref> {
    let c = rt::class_of_mirror(rt::nn_ref(this)?);
    let arr = rt::new_array(b'A', c.ifaces.len() as i32)?;
    for (i, k) in c.ifaces.iter().enumerate() {
        rt::astore::<Ref>(arr, i as i32, rt::class_mirror(k)?)?;
    }
    Ok(arr)
}
pub fn n_java_lang_Class__modifiers0(this: Ref) -> Res<i32> {
    Ok(rt::class_of_mirror(rt::nn_ref(this)?).access as i32)
}
