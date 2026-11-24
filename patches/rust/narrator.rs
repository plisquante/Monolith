// NarratorLinux$FliteLibrary.loadNative / CmuUsKal16.loadNative: load libflite and the kal16 voice.
//
// The originals call JNA (`NativeLibrary.getInstance` + `Native.register`), which binds the `native` methods of the
// class through JNA's own registry: about 120 JNA emulation calls at start-up (docs/JNI_ENVANTER_TR.md). Here the
// `native` methods are already bound by name to Rust (monolith/rt/natives_jni.rs, dlopen of libflite.so.1), so loading
// only has to tell whether the library is there. Same outcome as the JVM: present -> return, missing -> the same
// InitializeException message (the JVM also attaches JNA's UnsatisfiedLinkError as the cause; it only shows in the log),
// which makes Narrator.getNarrator() fall back to the empty narrator.

pub fn load_flite(_orig: impl Fn() -> rt::Res<()>) -> rt::Res<()> {
    if mono_rt::natives_jni::flite_available() {
        return Ok(());
    }
    rt::throw("com/mojang/text2speech/Narrator$InitializeException", "Failed to load library flite".to_string())
}

pub fn load_kal16(_orig: impl Fn() -> rt::Res<()>) -> rt::Res<()> {
    if mono_rt::natives_jni::flite_kal16_available() {
        return Ok(());
    }
    rt::throw("com/mojang/text2speech/Narrator$InitializeException", "Failed to load library flite_cmu_us_kal16".to_string())
}
