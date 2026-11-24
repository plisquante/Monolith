// NarratorLinux$FliteLibrary.loadNative / CmuUsKal16.loadNative: libflite ve kal16 sesini yükler.
//
// Orijinaller JNA'yı çağırır (`NativeLibrary.getInstance` + `Native.register`); bu, sınıfın `native` metotlarını JNA'nın kendi
// kayıt defteri üzerinden bağlar: açılışta yaklaşık 120 JNA öykünme çağrısı (docs/JNI_ENVANTER_TR.md). Burada
// `native` metotları zaten adla Rust'a bağlı (monolith/rt/natives_jni.rs, libflite.so.1'in dlopen'ı), bu yüzden yüklemenin
// yalnızca kütüphanenin orada olup olmadığını söylemesi gerekir. JVM ile aynı sonuç: varsa -> dön, yoksa -> aynı
// InitializeException mesajı (JVM ayrıca JNA'nın UnsatisfiedLinkError'ını neden olarak ekler; yalnızca günlükte görünür),
// bu da Narrator.getNarrator()'ın boş anlatıcıya geri düşmesini sağlar.

pub fn load_flite() -> rt::Res<()> {
    if mono_rt::natives_jni::flite_available() {
        return Ok(());
    }
    rt::throw("com/mojang/text2speech/Narrator$InitializeException", "Failed to load library flite".to_string())
}

pub fn load_kal16() -> rt::Res<()> {
    if mono_rt::natives_jni::flite_kal16_available() {
        return Ok(());
    }
    rt::throw("com/mojang/text2speech/Narrator$InitializeException", "Failed to load library flite_cmu_us_kal16".to_string())
}

// Narrator.getNarrator: platform anlatıcısını seçer. Orijinal NarratorLinux, NarratorWindows (JNA üzerinden SAPI
// COM) ya da NarratorMac (JNA üzerinden AppKit) kurar. Yalnızca Linux olanı tutulur: yukarıdaki load_flite sayesinde JNA'sız çalışır.
// Windows, orijinalin anlatıcısı başlatılamadığında yaptığı gibi tam olarak boş anlatıcıyı alır; konuşma arka uçları
// Rust'ta yazılana kadar (docs/mimari/04 §7). Orijinalle aynı istisna işleme: bir
// FatalException yayılır, başka her şey günlüğe yazılır ("Error while loading the narrator") ve EMPTY verir.
pub fn get_narrator() -> rt::Res<rt::Ref> {
    if !cfg!(target_os = "linux") {
        return Ok(unsafe { *s_empty() });
    }
    match n_linux() {
        Ok(n) => Ok(n),
        Err(t) => {
            if is_Fatal(t.obj) {
                return Err(t);
            }
            let logger: rt::Ref = unsafe { *s_logger() };
            v_log_error(logger, str_load_error(), t.obj)?;
            Ok(unsafe { *s_empty() })
        }
    }
}
