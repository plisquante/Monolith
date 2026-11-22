package java.util;

/** j2zig overlay: the stream boxing tripwire (a debugging aid behind a system property) is disabled. */
final class Tripwire {
    static final boolean ENABLED = false;

    private Tripwire() {}

    static void trip(Class<?> trippingClass, String msg) {}
}
