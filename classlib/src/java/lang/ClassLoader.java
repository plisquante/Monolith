package java.lang;

import java.io.IOException;
import java.io.InputStream;
import java.net.URL;
import java.util.Collections;
import java.util.Enumeration;

/**
 * monolith overlay of ClassLoader. A native image is a closed world: every class is linked in at
 * build time, so loaders only resolve names against the built-in class table. Resources are
 * served from the directory named by the {@code mono.resources} system property / MONO_RESOURCES
 * environment variable (the application JAR's contents, extracted at build time).
 */
public abstract class ClassLoader {
    private final ClassLoader parent;
    private final String name;

    private static final class SystemLoader extends ClassLoader {
        SystemLoader(String name) {
            super(name, null);
        }
    }

    private static final ClassLoader SYSTEM = new SystemLoader("app");
    private static final ClassLoader PLATFORM = new SystemLoader("platform");

    protected ClassLoader(String name, ClassLoader parent) {
        this.name = name;
        this.parent = parent;
    }

    protected ClassLoader(ClassLoader parent) {
        this(null, parent);
    }

    protected ClassLoader() {
        this(null, SYSTEM);
    }

    public String getName() {
        return name;
    }

    public Class<?> loadClass(String name) throws ClassNotFoundException {
        return loadClass(name, false);
    }

    protected Class<?> loadClass(String name, boolean resolve) throws ClassNotFoundException {
        return Class.forName(name, false, this);
    }

    protected Class<?> findClass(String name) throws ClassNotFoundException {
        throw new ClassNotFoundException(name);
    }

    protected final Class<?> findLoadedClass(String name) {
        try {
            return Class.forName(name, false, this);
        } catch (ClassNotFoundException e) {
            return null;
        }
    }

    protected final Class<?> findSystemClass(String name) throws ClassNotFoundException {
        return Class.forName(name, false, this);
    }

    protected final Class<?> defineClass(String name, byte[] b, int off, int len) throws ClassFormatError {
        throw new UnsupportedOperationException("monolith: classes cannot be defined at run time (" + name + ")");
    }

    protected final Class<?> defineClass(String name, byte[] b, int off, int len, java.security.ProtectionDomain pd) {
        throw new UnsupportedOperationException("monolith: classes cannot be defined at run time (" + name + ")");
    }


    public final ClassLoader getParent() {
        return parent;
    }

    public static ClassLoader getSystemClassLoader() {
        return SYSTEM;
    }

    public static ClassLoader getPlatformClassLoader() {
        return PLATFORM;
    }

    static String resourceRoot() {
        String r = System.getProperty("mono.resources");
        if (r == null) r = System.getenv("MONO_RESOURCES");
        return r;
    }

    static java.io.File resourceFile(String name) {
        String root = resourceRoot();
        if (root == null || name == null) return null;
        while (name.startsWith("/")) name = name.substring(1);
        java.io.File f = new java.io.File(root, name);
        return f.isFile() ? f : null;
    }

    public URL getResource(String name) {
        java.io.File f = resourceFile(name);
        if (f == null) return null;
        try {
            return f.toURI().toURL();
        } catch (java.net.MalformedURLException e) {
            return null;
        }
    }

    public Enumeration<URL> getResources(String name) throws IOException {
        URL u = getResource(name);
        return u == null ? Collections.emptyEnumeration() : Collections.enumeration(Collections.singletonList(u));
    }

    public InputStream getResourceAsStream(String name) {
        java.io.File f = resourceFile(name);
        if (f == null) return null;
        try {
            return new java.io.FileInputStream(f);
        } catch (IOException e) {
            return null;
        }
    }

    public static URL getSystemResource(String name) {
        return SYSTEM.getResource(name);
    }

    public static Enumeration<URL> getSystemResources(String name) throws IOException {
        return SYSTEM.getResources(name);
    }

    public static InputStream getSystemResourceAsStream(String name) {
        return SYSTEM.getResourceAsStream(name);
    }

    public final Package getDefinedPackage(String name) {
        return packages.get(name);
    }

    public final Package[] getDefinedPackages() {
        return packages.values().toArray(new Package[0]);
    }

    protected Package getPackage(String name) {
        return null;
    }

    public void setDefaultAssertionStatus(boolean enabled) {}

    public void setPackageAssertionStatus(String packageName, boolean enabled) {}

    public void setClassAssertionStatus(String className, boolean enabled) {}

    public void clearAssertionStatus() {}

    protected static boolean registerAsParallelCapable() {
        return true;
    }

    public final boolean isRegisteredAsParallelCapable() {
        return true;
    }

    protected Object getClassLoadingLock(String className) {
        return this;
    }

    private Module unnamedModule;

    public final Module getUnnamedModule() {
        synchronized (this) {
            if (unnamedModule == null) unnamedModule = new Module(this);
            return unnamedModule;
        }
    }

    // ---- class definition: impossible in a closed-world image, reported precisely
    private static UnsupportedOperationException noDefine() {
        return new UnsupportedOperationException("monolith: classes cannot be defined at run time in a native image");
    }

    protected final Class<?> defineClass(byte[] b, int off, int len) throws ClassFormatError { throw noDefine(); }
    protected final Class<?> defineClass(String name, java.nio.ByteBuffer b, java.security.ProtectionDomain pd) throws ClassFormatError { throw noDefine(); }
    protected final void resolveClass(Class<?> c) {}

    // ---- packages
    private final java.util.concurrent.ConcurrentHashMap<String, Package> packages = new java.util.concurrent.ConcurrentHashMap<>();

    protected Package definePackage(String name, String specTitle, String specVersion, String specVendor,
                                    String implTitle, String implVersion, String implVendor, URL sealBase) {
        Package p = new Package(name, specTitle, specVersion, specVendor, implTitle, implVersion, implVendor, sealBase, this);
        Package old = packages.putIfAbsent(name, p);
        if (old != null) throw new IllegalArgumentException(name);
        return p;
    }

    Package definePackage(Class<?> c) {
        return packages.computeIfAbsent(c.getPackageName(), n -> new Package(n, c.getModule()));
    }

    Package definePackage(String name, Module m) {
        return packages.computeIfAbsent(name, n -> new Package(n, m));
    }

    java.util.stream.Stream<Package> packages() {
        return packages.values().stream();
    }

    // ---- lookups used by the JDK
    Class<?> loadClass(Module module, String name) {
        try {
            return loadClass(name);
        } catch (ClassNotFoundException e) {
            return null;
        }
    }

    protected Class<?> findClass(String moduleName, String name) {
        return null;
    }

    protected URL findResource(String moduleName, String name) throws IOException {
        return getResource(name);
    }

    protected String findLibrary(String libname) {
        return null;
    }

    boolean desiredAssertionStatus(String className) {
        return false;
    }

    static Class<?> findBootstrapClassOrNull(String name) {
        try {
            return Class.forName(name, false, null);
        } catch (ClassNotFoundException e) {
            return null;
        }
    }
}
