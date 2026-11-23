//! Reflection, ConstantPool, Unsafe field-offset, MethodHandleAccessorFactory, and MethodHandleNatives.
use crate::rt::{self, Arr, Class, CpEnt, FLoc, Ref, Res};
use crate::natives::n_java_lang_String__intern;
use std::sync::atomic::Ordering;

fn arr_bytes(a: Ref) -> *mut u8 {
    unsafe { (a.0 as *mut u8).add(24) }
}

fn opt_str(s: &str) -> Ref {
    if s.is_empty() { Ref::NULL } else { rt::new_str(s.to_string()) }
}

fn opt_bytes(b: &[u8]) -> Res<Ref> {
    if b.is_empty() {
        return Ok(Ref::NULL);
    }
    let a = rt::new_array(b'B', b.len() as i32)?;
    unsafe { std::ptr::copy_nonoverlapping(b.as_ptr(), arr_bytes(a), b.len()) };
    Ok(a)
}

fn string_array(items: &[&str]) -> Res<Ref> {
    let arr = rt::new_obj_array(rt::hooks().string_class, items.len() as i32)?;
    for (i, s) in items.iter().enumerate() {
        let v = opt_str(s);
        rt::aastore_ref(arr, i as i32, v)?;
    }
    Ok(arr)
}

fn set_ref(o: Ref, name: &str, v: Ref) {
    let c = unsafe { (*o.0).class };
    if let Some((_, f)) = c.find_field(name, false) {
        if let FLoc::Off(off) = f.loc {
            unsafe { crate::rt::satb_store((o.0 as *mut u8).add(off as usize) as *mut Ref, v); }
        }
    }
}

fn get_ref(o: Ref, name: &str) -> Ref {
    let c = unsafe { (*o.0).class };
    if let Some((_, f)) = c.find_field(name, false) {
        if let FLoc::Off(off) = f.loc {
            return unsafe { *((o.0 as *const u8).add(off as usize) as *const Ref) };
        }
    }
    Ref::NULL
}

fn set_i32(o: Ref, name: &str, v: i32) {
    let c = unsafe { (*o.0).class };
    if let Some((_, f)) = c.find_field(name, false) {
        if let FLoc::Off(off) = f.loc {
            unsafe { *((o.0 as *mut u8).add(off as usize) as *mut i32) = v; }
        }
    }
}

fn get_i32(o: Ref, name: &str) -> i32 {
    let c = unsafe { (*o.0).class };
    if let Some((_, f)) = c.find_field(name, false) {
        if let FLoc::Off(off) = f.loc {
            return unsafe { *((o.0 as *const u8).add(off as usize) as *const i32) };
        }
    }
    0
}

pub fn class_for_desc(desc: &str) -> &'static Class {
    rt::class_for_desc(desc).unwrap_or(rt::hooks().object_class)
}

fn mirror_for_desc(desc: &str) -> Res<Ref> {
    rt::class_mirror(class_for_desc(desc))
}

fn parse_param_descs(desc: &str) -> Vec<&str> {
    let bytes = desc.as_bytes();
    let mut out = Vec::new();
    let mut i = 1;
    while i < bytes.len() && bytes[i] != b')' {
        let start = i;
        while i < bytes.len() && bytes[i] == b'[' {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b'L' {
            while i < bytes.len() && bytes[i] != b';' {
                i += 1;
            }
            i += 1;
        } else {
            i += 1;
        }
        out.push(&desc[start..i]);
    }
    out
}

// ------------------------------------------------------------------ java.lang.Class reflection
pub fn n_java_lang_Class__newField0(this: Ref, name: Ref) -> Res<Ref> {
    let c = rt::class_of_mirror(rt::nn_ref(this)?);
    let n = rt::str_to_rust(rt::nn_ref(name)?);
    let mut found = None;
    for f in c.fields {
        if f.name == n {
            found = Some(f);
            break;
        }
    }
    let f = match found {
        Some(f) => f,
        None => return Ok(Ref::NULL),
    };
    let field_cls = match rt::find_class("java/lang/reflect/Field") {
        Some(fc) => fc,
        None => return rt::throw("java/lang/InternalError", "java.lang.reflect.Field not loaded".to_string()),
    };
    if let Some(init) = field_cls.init { init().res()?; }
    let o = rt::alloc(field_cls);
    let ty = mirror_for_desc(f.desc)?;
    let iname = n_java_lang_String__intern(name)?;
    set_ref(o, "clazz", this);
    set_ref(o, "name", iname);
    set_ref(o, "type", ty);
    set_i32(o, "modifiers", f.access as i32);
    set_ref(o, "signature", opt_str(f.sig));
    set_ref(o, "annotations", opt_bytes(f.ann)?);
    Ok(o)
}

pub fn n_java_lang_Class__fields0(this: Ref) -> Res<Ref> {
    let c = rt::class_of_mirror(rt::nn_ref(this)?);
    let arr = rt::new_obj_array(rt::hooks().object_class, c.fields.len() as i32)?;
    for (i, f) in c.fields.iter().enumerate() {
        let s = rt::new_str(f.name.to_string());
        let fo = n_java_lang_Class__newField0(this, s)?;
        rt::aastore_ref(arr, i as i32, fo)?;
    }
    Ok(arr)
}

pub fn n_java_lang_Class__methods0(this: Ref, ctors: i32) -> Res<Ref> {
    let c = rt::class_of_mirror(rt::nn_ref(this)?);
    let want_ctor = ctors != 0;
    let count = c.methods.iter().filter(|m| (m.name == "<init>") == want_ctor).count();
    let arr = rt::new_obj_array(rt::hooks().object_class, count as i32)?;
    let target_cls_name = if want_ctor { "java/lang/reflect/Constructor" } else { "java/lang/reflect/Method" };
    let mc = match rt::find_class(target_cls_name) {
        Some(x) => x,
        None => return rt::throw("java/lang/InternalError", format!("{} not loaded", target_cls_name)),
    };
    if let Some(init) = mc.init { init().res()?; }
    let mut k = 0i32;
    for (slot, m) in c.methods.iter().enumerate() {
        let is_ctor = m.name == "<init>";
        if is_ctor != want_ctor {
            continue;
        }
        let ps = parse_param_descs(m.desc);
        let ptypes = rt::new_obj_array(rt::hooks().class_class, ps.len() as i32)?;
        for (i, pd) in ps.iter().enumerate() {
            rt::aastore_ref(ptypes, i as i32, mirror_for_desc(pd)?)?;
        }
        let ex_valid: Vec<&'static Class> = m.ex.iter().filter_map(|en| rt::find_class(en)).collect();
        let etypes = rt::new_obj_array(rt::hooks().class_class, ex_valid.len() as i32)?;
        for (i, ec) in ex_valid.iter().enumerate() {
            rt::aastore_ref(etypes, i as i32, rt::class_mirror(ec)?)?;
        }
        let o = rt::alloc(mc);
        set_ref(o, "clazz", this);
        set_i32(o, "slot", slot as i32);
        set_ref(o, "parameterTypes", ptypes);
        set_ref(o, "exceptionTypes", etypes);
        set_i32(o, "modifiers", m.access as i32);
        set_ref(o, "signature", opt_str(m.sig));
        set_ref(o, "annotations", opt_bytes(m.ann)?);
        set_ref(o, "parameterAnnotations", opt_bytes(m.pann)?);
        if !is_ctor {
            let iname = n_java_lang_String__intern(rt::new_str(m.name.to_string()))?;
            set_ref(o, "name", iname);
            let rd = match m.desc.find(')') {
                Some(idx) => &m.desc[idx + 1..],
                None => "V",
            };
            set_ref(o, "returnType", mirror_for_desc(rd)?);
            set_ref(o, "annotationDefault", opt_bytes(m.dflt)?);
        }
        rt::aastore_ref(arr, k, o)?;
        k += 1;
    }
    Ok(arr)
}

pub fn n_java_lang_Class__innerFlags0(this: Ref) -> Res<i32> {
    let c = rt::class_of_mirror(rt::nn_ref(this)?);
    Ok(match c.refl {
        Some(r) if r.is_inner => r.inner_flags as i32,
        _ => -1,
    })
}
pub fn n_java_lang_Class__outerName0(this: Ref) -> Res<Ref> {
    let c = rt::class_of_mirror(rt::nn_ref(this)?);
    Ok(match c.refl {
        Some(r) => opt_str(r.outer),
        None => Ref::NULL,
    })
}
pub fn n_java_lang_Class__innerName0(this: Ref) -> Res<Ref> {
    let c = rt::class_of_mirror(rt::nn_ref(this)?);
    Ok(match c.refl {
        Some(r) => opt_str(r.inner_name),
        None => Ref::NULL,
    })
}
pub fn n_java_lang_Class__enclosingMethod0(this: Ref) -> Res<Ref> {
    let c = rt::class_of_mirror(rt::nn_ref(this)?);
    match c.refl {
        Some(r) if !r.enc_class.is_empty() => string_array(&[r.enc_class, r.enc_name, r.enc_desc]),
        _ => Ok(Ref::NULL),
    }
}
pub fn n_java_lang_Class__declaredClasses0(this: Ref) -> Res<Ref> {
    let c = rt::class_of_mirror(rt::nn_ref(this)?);
    let items = match c.refl {
        Some(r) => r.declared,
        None => &[],
    };
    string_array(items)
}
pub fn n_java_lang_Class__nestHost0(this: Ref) -> Res<Ref> {
    let c = rt::class_of_mirror(rt::nn_ref(this)?);
    Ok(match c.refl {
        Some(r) => opt_str(r.nest_host),
        None => Ref::NULL,
    })
}
pub fn n_java_lang_Class__genericSignature0(this: Ref) -> Res<Ref> {
    let c = rt::class_of_mirror(rt::nn_ref(this)?);
    Ok(match c.refl {
        Some(r) => opt_str(r.sig),
        None => Ref::NULL,
    })
}
pub fn n_java_lang_Class__getRawAnnotations(this: Ref) -> Res<Ref> {
    let c = rt::class_of_mirror(rt::nn_ref(this)?);
    match c.refl {
        Some(r) => opt_bytes(r.ann),
        None => Ok(Ref::NULL),
    }
}
pub fn n_java_lang_Class__getConstantPool(this: Ref) -> Res<Ref> {
    let cpc = match rt::find_class("jdk/internal/reflect/ConstantPool") {
        Some(c) => c,
        None => return rt::throw("java/lang/InternalError", "ConstantPool not loaded".to_string()),
    };
    if let Some(init) = cpc.init { init().res()?; }
    let o = rt::alloc(cpc);
    set_ref(o, "constantPoolOop", this);
    Ok(o)
}

// ------------------------------------------------------------------ jdk.internal.reflect.ConstantPool
fn cp_entry(oop: Ref, idx: i32) -> Res<&'static CpEnt> {
    let c = rt::class_of_mirror(rt::nn_ref(oop)?);
    let r = c.refl.ok_or_else(|| ())
        .map_err(|_| crate::rt::Thrown { obj: (rt::hooks().throw_new)("java/lang/IllegalArgumentException", rt::new_str("Constant pool index out of bounds".to_string())) })?;
    if idx <= 0 || (idx as usize) >= r.cpool.len() {
        return rt::throw("java/lang/IllegalArgumentException", "Constant pool index out of bounds".to_string());
    }
    Ok(&r.cpool[idx as usize])
}
fn cp_wrong<T>() -> Res<T> {
    rt::throw("java/lang/IllegalArgumentException", "Wrong type at constant pool index".to_string())
}

pub fn n_jdk_internal_reflect_ConstantPool__getSize0(this: Ref, oop: Ref) -> Res<i32> {
    let c = rt::class_of_mirror(rt::nn_ref(oop)?);
    Ok(c.refl.map(|r| r.cpool.len() as i32).unwrap_or(0))
}
pub fn n_jdk_internal_reflect_ConstantPool__getUTF8At0(this: Ref, oop: Ref, idx: i32) -> Res<Ref> {
    match cp_entry(oop, idx)? {
        CpEnt::Str(s) => Ok(rt::new_str(s.to_string())),
        _ => cp_wrong(),
    }
}
pub fn n_jdk_internal_reflect_ConstantPool__getIntAt0(this: Ref, oop: Ref, idx: i32) -> Res<i32> {
    match cp_entry(oop, idx)? {
        CpEnt::Int(v) => Ok(*v),
        _ => cp_wrong(),
    }
}
pub fn n_jdk_internal_reflect_ConstantPool__getLongAt0(this: Ref, oop: Ref, idx: i32) -> Res<i64> {
    match cp_entry(oop, idx)? {
        CpEnt::Long(v) => Ok(*v),
        _ => cp_wrong(),
    }
}
pub fn n_jdk_internal_reflect_ConstantPool__getFloatAt0(this: Ref, oop: Ref, idx: i32) -> Res<f32> {
    match cp_entry(oop, idx)? {
        CpEnt::Float(v) => Ok(*v),
        _ => cp_wrong(),
    }
}
pub fn n_jdk_internal_reflect_ConstantPool__getDoubleAt0(this: Ref, oop: Ref, idx: i32) -> Res<f64> {
    match cp_entry(oop, idx)? {
        CpEnt::Double(v) => Ok(*v),
        _ => cp_wrong(),
    }
}
pub fn n_jdk_internal_reflect_ConstantPool__getTagAt0(this: Ref, oop: Ref, idx: i32) -> Res<i32> {
    Ok(match cp_entry(oop, idx)? {
        CpEnt::None => 0,
        CpEnt::Str(_) => 1,
        CpEnt::Int(_) => 3,
        CpEnt::Float(_) => 4,
        CpEnt::Long(_) => 5,
        CpEnt::Double(_) => 6,
    })
}

// ------------------------------------------------------------------ Unsafe Field-based offset queries
fn field_target(f: Ref) -> Res<(&'static Class, String, bool)> {
    rt::nn(f)?;
    let clazz = get_ref(f, "clazz");
    let c = rt::class_of_mirror(rt::nn_ref(clazz)?);
    let name = rt::str_to_rust(rt::nn_ref(get_ref(f, "name"))?);
    let mods = get_i32(f, "modifiers");
    Ok((c, name, (mods & 0x8) != 0))
}

pub fn n_jdk_internal_misc_Unsafe__objectFieldOffset0(this: Ref, f: Ref) -> Res<i64> {
    let (c, name, _) = field_target(f)?;
    if let Some((_, fi)) = c.find_field(&name, false) {
        if let FLoc::Off(off) = fi.loc {
            return Ok(off as i64);
        }
    }
    rt::throw("java/lang/InternalError", format!("Unsafe.objectFieldOffset: no such field {}", name))
}

pub fn n_jdk_internal_misc_Unsafe__staticFieldOffset0(this: Ref, f: Ref) -> Res<i64> {
    let (c, name, _) = field_target(f)?;
    if let Some((_, fi)) = c.find_field(&name, true) {
        if let FLoc::Static(p) = &fi.loc {
            return Ok(p.0 as i64);
        }
    }
    rt::throw("java/lang/InternalError", format!("Unsafe.staticFieldOffset: no such field {}", name))
}

pub fn n_jdk_internal_misc_Unsafe__staticFieldBase0(this: Ref, f: Ref) -> Res<Ref> {
    Ok(Ref::NULL)
}

// ------------------------------------------------------------------ Reflective invocation
fn find_target(decl: &'static Class, slot: usize, recv: Ref, dispatch: bool) -> Res<&'static rt::MInfo> {
    let m = &decl.methods[slot];
    if dispatch && (m.access & 0x000A) == 0 {
        if !recv.is_null() {
            let rc = unsafe { (*recv.0).class };
            let mut k: Option<&'static Class> = Some(rc);
            while let Some(kk) = k {
                for km in kk.methods {
                    if !km.fp.0.is_null() && (km.access & 0x000A) == 0 && km.name == m.name && km.desc == m.desc {
                        return Ok(km);
                    }
                }
                k = kk.super_;
            }
            let mut best_iface: Option<&'static Class> = None;
            let mut best_m: Option<&'static rt::MInfo> = None;
            for ic in rc.ifaces {
                for km in ic.methods {
                    if !km.fp.0.is_null() && (km.access & 0x040A) == 0 && km.name == m.name && km.desc == m.desc {
                        if best_iface.map_or(true, |bi| rt::is_subclass(ic, bi)) {
                            best_iface = Some(ic);
                            best_m = Some(km);
                        }
                    }
                }
            }
            if let Some(bm) = best_m {
                return Ok(bm);
            }
        }
    }
    if m.fp.0.is_null() {
        return rt::throw(
            "java/lang/UnsupportedOperationException",
            format!("reflective target was not compiled (not reachable at build time): {}.{}{}", decl.name, m.name, m.desc),
        );
    }
    Ok(m)
}

pub fn n_jdk_internal_reflect_MethodHandleAccessorFactory__invoke0(m: Ref, recv: Ref, args: Ref) -> Res<Ref> {
    rt::nn(m)?;
    let decl = rt::class_of_mirror(rt::nn_ref(get_ref(m, "clazz"))?);
    let slot = get_i32(m, "slot") as usize;
    let t = find_target(decl, slot, recv, true)?;
    (rt::hooks().reflect_invoke)(t.shape, t.fp.0, recv, args)
}

pub fn n_jdk_internal_reflect_MethodHandleAccessorFactory__newInstance0(c: Ref, args: Ref) -> Res<Ref> {
    rt::nn(c)?;
    let decl = rt::class_of_mirror(rt::nn_ref(get_ref(c, "clazz"))?);
    let slot = get_i32(c, "slot") as usize;
    let t = find_target(decl, slot, Ref::NULL, false)?;
    let o = rt::alloc(decl);
    (rt::hooks().reflect_invoke)(t.shape, t.fp.0, o, args)?;
    Ok(o)
}

pub fn n_java_lang_reflect_Proxy__invokeDefault0(m: Ref, proxy: Ref, args: Ref) -> Res<Ref> {
    rt::nn(m)?;
    let decl = rt::class_of_mirror(rt::nn_ref(get_ref(m, "clazz"))?);
    let slot = get_i32(m, "slot") as usize;
    let t = find_target(decl, slot, Ref::NULL, false)?;
    (rt::hooks().reflect_invoke)(t.shape, t.fp.0, proxy, args)
}

pub fn n_java_lang_invoke_MonoMH__invokeMethod0(m: Ref, recv: Ref, args: Ref, dispatch: i32) -> Res<Ref> {
    rt::nn(m)?;
    let decl = rt::class_of_mirror(rt::nn_ref(get_ref(m, "clazz"))?);
    let slot = get_i32(m, "slot") as usize;
    let t = find_target(decl, slot, recv, dispatch != 0)?;
    (rt::hooks().reflect_invoke)(t.shape, t.fp.0, recv, args)
}

pub fn n_java_lang_invoke_MonoMH__newInstance0(c: Ref, args: Ref) -> Res<Ref> {
    n_jdk_internal_reflect_MethodHandleAccessorFactory__newInstance0(c, args)
}

// ------------------------------------------------------------------ JDK 25 MethodHandleNatives & NativeImageBuffer
pub fn n_java_lang_invoke_MethodHandleNatives__registerNatives() -> Res<()> { Ok(()) }
pub fn n_java_lang_invoke_MethodHandleNatives__getNamedCon(which: i32, name: Ref) -> Res<i32> { Ok(0) }
pub fn n_java_lang_invoke_MethodHandleNatives__init(this_mh: Ref, r: Ref) -> Res<()> { Ok(()) }
pub fn n_java_lang_invoke_MethodHandleNatives__expand(this_mh: Ref) -> Res<()> { Ok(()) }
pub fn n_jdk_internal_jimage_NativeImageBuffer__getNativeMap(name: Ref) -> Res<Ref> { Ok(Ref::NULL) }
