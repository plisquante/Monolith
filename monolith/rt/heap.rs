//! Heap cephesi. Çalışma zamanı yalnızca bu modül üzerinden tahsis eder, yazma bariyerlerini çalıştırır ve toplama yapar.
//! Şu an `gc` içindeki izleyici (tracing) toplayıcıya yönlendirir; toplayıcı, yerel metotlara, üretilen koda ya da sınıf modeline
//! dokunmadan bu modülün arkasında değiştirilebilir.
pub use crate::gc::*;
