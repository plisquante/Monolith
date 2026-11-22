package org.lwjgl.system;

public final class Library {
    private Library() {}

    public static void initialize() {}

    public static SharedLibrary loadNative(Class<?> context, String module, String name) {
        return new SharedLibrary.Default(name, name);
    }

    public static SharedLibrary loadNative(Class<?> context, String module, String name, boolean bundled) {
        return new SharedLibrary.Default(name, name);
    }
}
