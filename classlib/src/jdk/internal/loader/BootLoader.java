package jdk.internal.loader;

import java.io.IOException;
import java.io.InputStream;
import java.net.URL;
import java.util.Collections;
import java.util.Enumeration;
import java.util.concurrent.ConcurrentHashMap;
import java.util.stream.Stream;

/**
 * j2zig overlay of the boot class loader facade. Native libraries are linked into the
 * executable at build time, so loadLibrary has nothing to do; classes come from the
 * built-in class table; there is no module system.
 */
public class BootLoader {
    private BootLoader() {}

    private static final ConcurrentHashMap<?, ?> CLASS_LOADER_VALUE_MAP = new ConcurrentHashMap<>();

    public static Module getUnnamedModule() {
        throw new UnsupportedOperationException("j2zig: modules are not available");
    }

    public static jdk.internal.module.ServicesCatalog getServicesCatalog() {
        throw new UnsupportedOperationException("j2zig: module services catalog is not available");
    }

    public static ConcurrentHashMap<?, ?> getClassLoaderValueMap() {
        return CLASS_LOADER_VALUE_MAP;
    }

    public static boolean hasClassPath() {
        return false;
    }

    public static Class<?> loadClassOrNull(String name) {
        try {
            return Class.forName(name, false, null);
        } catch (ClassNotFoundException e) {
            return null;
        }
    }

    public static Class<?> loadClass(Module module, String name) {
        return loadClassOrNull(name);
    }

    public static void loadLibrary(String name) {
        // statically linked
    }

    public static URL findResource(String mn, String name) throws IOException {
        return null;
    }

    public static InputStream findResourceAsStream(String mn, String name) throws IOException {
        return null;
    }

    public static URL findResource(String name) {
        return null;
    }

    public static Enumeration<URL> findResources(String name) throws IOException {
        return Collections.emptyEnumeration();
    }

    public static Package definePackage(Class<?> c) {
        return null;
    }

    public static Package getDefinedPackage(String pn) {
        return null;
    }

    public static Stream<Package> packages() {
        return Stream.empty();
    }
}
