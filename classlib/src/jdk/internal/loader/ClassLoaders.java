package jdk.internal.loader;

/**
 * monolith overlay of jdk.internal.loader.ClassLoaders: delegates directly to the native image's
 * platform and system ClassLoaders without instantiating BuiltinClassLoader / URLClassPath /
 * JarVerifier.
 */
public class ClassLoaders {
    private ClassLoaders() {}

    static BuiltinClassLoader bootLoader() {
        throw new UnsupportedOperationException("monolith: ClassLoaders.bootLoader");
    }

    public static ClassLoader platformClassLoader() {
        return ClassLoader.getPlatformClassLoader();
    }

    public static ClassLoader appClassLoader() {
        return ClassLoader.getSystemClassLoader();
    }
}
