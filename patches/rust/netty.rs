// Netty yerel taşıyıcıları (Linux'ta epoll) bu derlemenin parçası değildir.
//
// EventLoopGroupHolder.remote(useNativeTransport), netty'nin yerel kütüphanesi yüklenirse KQUEUE ya da EPOLL, aksi hâlde NIO seçer.
// Yerel taşıyıcılar netty'nin JNI kütüphanesine ulaşır (yaklaşık 150 yerel metot, monolith/rt/natives_jni.rs içinde öykünülür). Burada remote()
// her zaman NIO döndürür; bu, yerel kütüphane yokken JVM'in döndürdüğüyle aynıdır (`KQueue.isAvailable()` ve
// `Epoll.isAvailable()` false), yani oyun her platformda java.nio taşıyıcısını kullanır. EPOLL ve KQUEUE tutucuları
// hâlâ vardır (sınıf başlatıcı onları oluşturur) ama hiçbir şey onları dağıtmaz ve ioHandlerFactory() metotları da
// değiştirilir; böylece epoll/kqueue işleyici sınıfları ve yerel metotları programdan çıkar ("replace": true, docs/mimari/04 §6).

pub fn remote_nio(_use_native_transport: i32) -> rt::Res<rt::Ref> {
    Ok(unsafe { *s_nio() })
}

pub fn no_native_transport(_this: rt::Ref) -> rt::Res<rt::Ref> {
    rt::throw("java/lang/UnsupportedOperationException", "netty native transports are not part of this build".to_string())
}
