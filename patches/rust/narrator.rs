// NarratorLinux$FliteLibrary.loadNative / CmuUsKal16.loadNative: load libflite and the kal16 voice.
//
// The originals call JNA (`NativeLibrary.getInstance` + `Native.register`), which binds the `native` methods of the
// class through JNA's own registry: about 120 JNA emulation calls at start-up (docs/JNI_ENVANTER_TR.md). Here the
// `native` methods are already bound by name to Rust (monolith/rt/natives_jni.rs, dlopen of libflite.so.1), so loading
// only has to tell whether the library is there. Same outcome as the JVM: present -> return, missing -> the same
// InitializeException message (the JVM also attaches JNA's UnsatisfiedLinkError as the cause; it only shows in the log),
// which makes Narrator.getNarrator() fall back to the empty narrator.

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

// Narrator.getNarrator: picks the platform narrator. The original builds NarratorLinux, NarratorWindows (SAPI through JNA
// COM) or NarratorMac (AppKit through JNA). Only the Linux one is kept: it already runs without JNA (load_flite above).
// Windows gets the empty narrator, exactly as the original does when its narrator fails to initialize, until
// their speech back ends are written in Rust (docs/mimari/04 §7). Same exception handling as the original: a
// FatalException propagates, anything else is logged ("Error while loading the narrator") and gives EMPTY.
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
