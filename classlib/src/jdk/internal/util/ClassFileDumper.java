package jdk.internal.util;

/**
 * j2zig overlay of ClassFileDumper: a closed-world native image never generates or dumps bytecode
 * class files at run time, so ClassFileDumper is disabled and does not pull java.nio.file into
 * every MethodType / lambda user.
 */
public final class ClassFileDumper {
    private static final ClassFileDumper DISABLED = new ClassFileDumper();

    private ClassFileDumper() {}

    public static ClassFileDumper getInstance(String key, String path) {
        return DISABLED;
    }

    public String key() {
        return "";
    }

    public boolean isEnabled() {
        return false;
    }

    public void dumpClass(String name, Class<?> c, byte[] bytes) {}

    public void dumpFailedClass(String name, byte[] bytes) {}
}
