// Netty native transports (epoll on Linux) are not part of this build.
//
// EventLoopGroupHolder.remote(useNativeTransport) picks KQUEUE or EPOLL when netty's native library loads, NIO otherwise.
// The native transports reach netty's JNI library (about 150 natives, emulated in monolith/rt/natives_jni.rs). Here remote()
// always returns NIO, exactly what the JVM returns when the native library is missing (`KQueue.isAvailable()` and
// `Epoll.isAvailable()` false), so the game uses the java.nio transport on every platform. The holders EPOLL and KQUEUE
// still exist (the class initializer creates them) but nothing hands them out, and their ioHandlerFactory() is replaced
// too, so the epoll/kqueue handler classes and their natives leave the program ("replace": true, docs/mimari/04 §6).

pub fn remote_nio(_use_native_transport: i32) -> rt::Res<rt::Ref> {
    Ok(unsafe { *s_nio() })
}

pub fn no_native_transport(_this: rt::Ref) -> rt::Res<rt::Ref> {
    rt::throw("java/lang/UnsupportedOperationException", "netty native transports are not part of this build".to_string())
}
