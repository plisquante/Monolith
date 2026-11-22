package jdk.internal.misc;

/** j2zig overlay: there is no class data sharing archive in a native image. */
public class CDS {
    private CDS() {}
    public static boolean isDumpingClassList() { return false; }
    public static boolean isDumpingArchive() { return false; }
    public static boolean isSharingEnabled() { return false; }
    public static boolean isDumpingStaticArchive() { return false; }
    public static boolean isDumpingHeap() { return false; }
    public static boolean isLoggingLambdaFormInvokers() { return false; }
    public static boolean isUsingArchive() { return false; }
    public static void initializeFromArchive(Class<?> c) {}
    public static long getRandomSeedForDumping() { return 0L; }
    public static void defineArchivedModules(ClassLoader platformLoader, ClassLoader systemLoader) {}
    public static void logLambdaFormInvoker(String line) {}
    public static void traceLambdaFormInvoker(String prefix, String holder, String name, String type) {}
    public static void traceSpeciesType(String prefix, String cn) {}
    public static boolean isSingleThreadVM() { return false; }
    public static void keepAlive(Object o) {}
    public static void logLambdaFormInvoker(String prefix, String holder, String name, String type) {}
    public static void logSpeciesType(String prefix, String cn) {}
    public static boolean needsClassInitBarrier(Class<?> c) { return false; }
    static final String DIRECT_HOLDER_CLASS_NAME = "java.lang.invoke.DirectMethodHandle$Holder";
    static final String DELEGATING_HOLDER_CLASS_NAME = "java.lang.invoke.DelegatingMethodHandle$Holder";
    static final String BASIC_FORMS_HOLDER_CLASS_NAME = "java.lang.invoke.LambdaForm$Holder";
    static final String INVOKERS_HOLDER_CLASS_NAME = "java.lang.invoke.Invokers$Holder";
}
