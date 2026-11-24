//! El yazımı yamalar için API (`patches/rust/*.rs`).
//!
//! Bir yama, çevrilmiş tek bir metodu Rust koduyla değiştirir. Yama, yamaladığı sınıfın yongasında derlenir;
//! manifestin istediği bağlamaları (alan ofsetleri, statikler, sınıf referansları, çağrılabilir metotlar) zaten içeren bir
//! modül içinde. Bu modül programa bağlı olmayan parçaları tutar: ham alan erişimi, dizi görünümleri ve `MONO_PATCH_VERIFY` öz denetimi.
//!
use crate::rt::{self, Obj, Ref};
use std::sync::OnceLock;

/// `MONO_PATCH_VERIFY` ayarlıysa: ilkel değer döndüren her yamalı metot hem orijinali hem yamayı çalıştırır,
/// ham bitleri karşılaştırır ve ilk farkta durdurur.
pub fn verify() -> bool {
    static V: OnceLock<bool> = OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MONO_PATCH_VERIFY").is_some_and(|v| !v.is_empty() && v != "0"))
}

pub fn mismatch(what: &str) -> ! {
    rt::abort(&format!("patch mismatch in {what}: the patch and the translated original disagree"))
}

/// Ham bitler üzerinde eşitlik (NaN ve -0.0 Java sonucu gibi tam olarak karşılaştırılır).
pub trait Same {
    fn same(&self, other: &Self) -> bool;
}
macro_rules! same_int { ($($t:ty),*) => { $(impl Same for $t { #[inline] fn same(&self, o: &Self) -> bool { *self == *o } })* } }
same_int!(i32, i64, i8, i16, u16);
impl Same for f32 { #[inline] fn same(&self, o: &Self) -> bool { self.to_bits() == o.to_bits() } }
impl Same for f64 { #[inline] fn same(&self, o: &Self) -> bool { self.to_bits() == o.to_bits() } }

/// `o` nesnesinin `off` bayt ofsetindeki bir alanını okur (ofsetler manifest bağlamalarından gelir: `F_<ad>`).
/// Boolean, byte, char ve short kendi genişliklerinde saklanır; referanslar `Ref` olarak.
///
/// # Güvenlik
/// `o`, `off` ofsetinde `T` türünde bir alanı olan boş olmayan bir nesne olmalıdır.
#[inline(always)]
pub unsafe fn get<T: Copy>(o: Ref, off: usize) -> T {
    *((o.0 as *const u8).add(off) as *const T)
}

/// # Güvenlik
/// [`get`] ile aynı sözleşme. Referans yazmaları için yazma bariyeri gerekmez: toplayıcı taşımaz ve yığınları ile
/// heap'i muhafazakâr biçimde tarar.
#[inline(always)]
pub unsafe fn set<T>(o: Ref, off: usize, v: T) {
    let p = (o.0 as *mut u8).add(off);
    // Bir yama referans yazabilir: üzerine yazılan baytlar SATB işaretleyicisi için günlüğe alınır (döngü yoksa ucuzdur)
    crate::heap::satb_log_range(p as *const u8, std::mem::size_of::<T>());
    *(p as *mut T) = v;
}

/// Bir nesnenin sınıf işaretçisi; `class_<ad>()` bağlamasına karşı kimlik denetimi için.
///
/// # Güvenlik
/// `o` boş olmamalıdır.
#[inline(always)]
pub unsafe fn class_ptr(o: Ref) -> *const rt::Class {
    (*(o.0 as *const Obj)).class as *const rt::Class
}

/// Bir dizi nesnesinin uzunluğu.
///
/// # Güvenlik
/// `a` boş olmayan bir dizi olmalıdır.
#[inline(always)]
pub unsafe fn array_len(a: Ref) -> i32 {
    (*(a.0 as *const rt::Arr)).len
}

/// Bir dizi nesnesinin elemanları, dilim olarak.
///
/// # Güvenlik
/// `a`, eleman türü `T` olan boş olmayan bir dizi olmalıdır (`long[]` için `i64`, `int[]` için `i32`, nesne dizileri için `Ref`
/// ...).
#[inline(always)]
pub unsafe fn array<'a, T>(a: Ref) -> &'a mut [T] {
    std::slice::from_raw_parts_mut((a.0 as *mut u8).add(std::mem::size_of::<rt::Arr>()) as *mut T, array_len(a) as usize)
}
