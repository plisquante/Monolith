package jdk.internal.misc;

import java.util.Map;
import java.util.Properties;
import jdk.internal.access.SharedSecrets;

/** monolith overlay of jdk.internal.misc.VM: a native image is always fully booted. */
public class VM {
    public static final int SYSTEM_LOADER_INITIALIZATION_COMPLETED = 3;
    public static final int SYSTEM_BOOTED = 4;
    public static final int SYSTEM_SHUTDOWN = 5;
    private static final Object lock = new Object();

    public static void initLevel(int value) {}
    public static int initLevel() { return SYSTEM_BOOTED; }
    public static void awaitInitLevel(int value) {}
    public static boolean isModuleSystemInited() { return true; }
    public static boolean isBooted() { return true; }
    public static void shutdown() {}
    public static boolean isShutdown() { return false; }
    public static long maxDirectMemory() { return Runtime.getRuntime().maxMemory(); }
    public static boolean isDirectMemoryPageAligned() { return false; }
    public static boolean isSystemDomainLoader(ClassLoader loader) { return loader == null; }
    public static String getSavedProperty(String key) { return System.getProperty(key); }
    public static Map<String, String> getSavedProperties() { return Map.of(); }
    public static void saveProperties(Map<String, String> props) {}
    public static boolean isSetUID() { return false; }
    public static long getNanoTimeAdjustment(long offsetInSeconds) {
        long ms = System.currentTimeMillis();
        long secs = ms / 1000 - offsetInSeconds;
        long nanos = (ms % 1000) * 1000000L;
        return secs * 1000000000L + nanos;
    }
    public static Thread.State toThreadState(int threadStatus) { return Thread.State.RUNNABLE; }
    public static String[] getRuntimeArguments() { return new String[0]; }
    public static boolean isSupportedClassFileVersion(int major, int minor) { return true; }
    public static boolean isSupportedModuleDescriptorVersion(int major, int minor) { return true; }
    public static ClassLoader latestUserDefinedLoader() { return null; }
    public static long getuid() { return 0; }
    public static long geteuid() { return 0; }
    public static long getgid() { return 0; }
    public static long getegid() { return 0; }
    public static boolean isJavaLangInvokeInited() { return false; }
    public static void setJavaLangInvokeInited() {}

    // ---- finalizer statistics (no finalizers run in the native image)
    public static void addFinalRefCount(int n) {}
    public static int getFinalRefCount() { return 0; }
    public static int getPeakFinalRefCount() { return 0; }

    public static void initializeOSEnvironment() {}

    private static volatile java.io.PrintStream initialErr;

    /** System.err as it was before any System.setErr (the image never reassigns it before main). */
    public static java.io.PrintStream initialErr() {
        java.io.PrintStream e = initialErr;
        if (e == null) initialErr = e = System.err;
        return e;
    }

    public static java.util.List<BufferPool> getBufferPools() {
        java.util.List<BufferPool> l = new java.util.ArrayList<>();
        l.add(SharedSecrets.getJavaNioAccess().getDirectBufferPool());
        return l;
    }

    public interface BufferPool {
        String getName();
        long getCount();
        long getTotalCapacity();
        long getMemoryUsed();
    }
}
