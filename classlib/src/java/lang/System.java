package java.lang;

import java.io.InputStream;
import java.io.OutputStream;
import java.io.PrintStream;
import java.io.IOException;
import java.util.Properties;

/** monolith overlay of java.lang.System. */
public final class System {
    private System() {}

    // Not final in this overlay so setOut/setErr/setIn can reassign them without natives.
    public static InputStream in = new FdInputStream(0);
    private static final String NATIVE_ENCODING = nativeEncoding();
    public static PrintStream out = newPrintStream(1);
    public static PrintStream err = newPrintStream(2);

    private static PrintStream newPrintStream(int fd) {
        try {
            return new PrintStream(new FdOutputStream(fd), true, NATIVE_ENCODING.equals("UTF-8") ? "UTF-8" : "US-ASCII");
        } catch (java.io.UnsupportedEncodingException e) {
            return new PrintStream(new FdOutputStream(fd), true);
        }
    }

    /** Same rule as the JDK on Linux: the C locale's codeset (UTF-8, else ASCII "ANSI_X3.4-1968"). */
    private static String nativeEncoding() {
        String v = getenv0("LC_ALL");
        if (v == null || v.isEmpty()) v = getenv0("LC_CTYPE");
        if (v == null || v.isEmpty()) v = getenv0("LANG");
        if (v != null) {
            String u = v.toUpperCase();
            if (u.contains("UTF-8") || u.contains("UTF8")) return "UTF-8";
        }
        return "ANSI_X3.4-1968";
    }
    private static Properties props;
    /** no module system in a native image */
    static ModuleLayer bootLayer;

    static {
        jdk.internal.access.SharedSecrets.setJavaLangAccess(new MonoJavaLangAccess());
    }

    static final class FdOutputStream extends OutputStream {
        private final int fd;
        FdOutputStream(int fd) { this.fd = fd; }
        public void write(int b) throws IOException { write0(fd, new byte[]{(byte) b}, 0, 1); }
        public void write(byte[] b, int off, int len) throws IOException {
            java.util.Objects.checkFromIndexSize(off, len, b.length);
            write0(fd, b, off, len);
        }
    }

    static final class FdInputStream extends InputStream {
        private final int fd;
        FdInputStream(int fd) { this.fd = fd; }
        public int read() throws IOException {
            byte[] b = new byte[1];
            int n = read0(fd, b, 0, 1);
            return n <= 0 ? -1 : b[0] & 0xff;
        }
        public int read(byte[] b, int off, int len) throws IOException {
            java.util.Objects.checkFromIndexSize(off, len, b.length);
            if (len == 0) return 0;
            int n = read0(fd, b, off, len);
            return n <= 0 ? -1 : n;
        }
    }

    static native void write0(int fd, byte[] b, int off, int len);
    static native int read0(int fd, byte[] b, int off, int len);

    public static void setOut(PrintStream o) { out = o; }
    public static void setErr(PrintStream e) { err = e; }
    public static void setIn(InputStream i) { in = i; }

    public static native long currentTimeMillis();
    public static native long nanoTime();
    public static native void arraycopy(Object src, int srcPos, Object dest, int destPos, int length);
    public static native int identityHashCode(Object x);
    private static native String getenv0(String name);
    private static native String[] env0();
    private static native String cwd0();
    private static native String exeDir0();
    private static native String arch0();

    public static String lineSeparator() {
        return "\n";
    }

    public static synchronized Properties getProperties() {
        if (props == null) {
            Properties p = new Properties();
            p.setProperty("java.version", "25");
            p.setProperty("java.specification.version", "25");
            p.setProperty("java.vendor", "monolith");
            p.setProperty("java.vm.name", "monolith native image");
            p.setProperty("java.class.version", "69.0");
            p.setProperty("os.name", "Linux");
            p.setProperty("os.arch", arch0());
            p.setProperty("os.version", "");
            p.setProperty("file.separator", "/");
            p.setProperty("path.separator", ":");
            p.setProperty("line.separator", "\n");
            p.setProperty("file.encoding", "UTF-8");
            p.setProperty("native.encoding", NATIVE_ENCODING);
            p.setProperty("stdout.encoding", NATIVE_ENCODING);
            p.setProperty("stderr.encoding", NATIVE_ENCODING);
            p.setProperty("sun.jnu.encoding", NATIVE_ENCODING);
            p.setProperty("user.dir", cwd0());
            String home = getenv0("HOME");
            p.setProperty("user.home", home == null ? "/" : home);
            String user = getenv0("USER");
            p.setProperty("user.name", user == null ? "" : user);
            // Android has no /tmp: the launcher points TMPDIR at the app cache directory
            String tmp = getenv0("TMPDIR");
            p.setProperty("java.io.tmpdir", tmp == null || tmp.isEmpty() ? "/tmp" : tmp);
            // java.home holds conf/ (java.security, net.properties, ...) and lib/tzdb.dat. On a desktop it is the
            // executable's directory; on Android the executable is /system/bin/app_process, so the launcher extracts
            // that tree into the app's files and names it in MONO_JAVA_HOME.
            String javaHome = getenv0("MONO_JAVA_HOME");
            p.setProperty("java.home", javaHome == null || javaHome.isEmpty() ? exeDir0() : javaHome);
            p.setProperty("java.library.path", "");
            p.setProperty("sun.boot.library.path", "");
            p.setProperty("java.class.path", "");
            p.setProperty("user.language", "en");
            p.setProperty("user.country", "US");
            p.setProperty("java.vm.specification.version", "25");
            p.setProperty("java.runtime.version", "25");
            p.setProperty("java.vm.version", "25");
            p.setProperty("jna.nosys", "false");
            // -Dname=value launcher arguments override the defaults
            for (String kv : userProps0()) {
                int eq = kv.indexOf('=');
                if (eq > 0) p.setProperty(kv.substring(0, eq), kv.substring(eq + 1));
                else if (!kv.isEmpty()) p.setProperty(kv, "");
            }
            props = p;
        }
        return props;
    }

    private static native String[] userProps0();

    public static void setProperties(Properties p) {
        synchronized (System.class) {
            props = p;
        }
    }

    public static String getProperty(String key) {
        checkKey(key);
        return getProperties().getProperty(key);
    }

    public static String getProperty(String key, String def) {
        checkKey(key);
        return getProperties().getProperty(key, def);
    }

    public static String setProperty(String key, String value) {
        checkKey(key);
        return (String) getProperties().setProperty(key, value);
    }

    public static String clearProperty(String key) {
        checkKey(key);
        return (String) getProperties().remove(key);
    }

    private static void checkKey(String key) {
        if (key == null) throw new NullPointerException("key can't be null");
        if (key.isEmpty()) throw new IllegalArgumentException("key can't be empty");
    }

    public static String getenv(String name) {
        return getenv0(name);
    }

    public static java.util.Map<String, String> getenv() {
        java.util.HashMap<String, String> m = new java.util.HashMap<>();
        String[] kv = env0();
        for (int i = 0; i + 1 < kv.length; i += 2) m.put(kv[i], kv[i + 1]);
        return java.util.Collections.unmodifiableMap(m);
    }

    @Deprecated(since = "17", forRemoval = true)
    public static SecurityManager getSecurityManager() {
        return null;
    }

    @Deprecated(since = "17", forRemoval = true)
    public static void setSecurityManager(SecurityManager sm) {
        throw new UnsupportedOperationException("The Security Manager is deprecated and will be removed in a future release");
    }

    public static java.io.Console console() {
        return null;
    }

    public static void exit(int status) {
        Runtime.getRuntime().exit(status);
    }

    public static void gc() {
        // Minecraft asks for a full collection every tick; one stops the game for about half a second. Only on request
        // (-Dmono.explicit.gc): the runtime collects by itself when the heap needs it.
        if (System.getProperty("mono.explicit.gc") != null) Runtime.getRuntime().gc();
    }

    public static void runFinalization() {}

    public static void load(String filename) {
        Runtime.getRuntime().load(filename);
    }

    public static void loadLibrary(String libname) {
        Runtime.getRuntime().loadLibrary(libname);
    }

    public static String mapLibraryName(String libname) {
        return "lib" + libname + ".so";
    }

    public static System.Logger getLogger(String name) {
        java.util.Objects.requireNonNull(name);
        return new ConsoleLogger(name);
    }

    public interface Logger {
        public enum Level {
            ALL(Integer.MIN_VALUE), TRACE(400), DEBUG(500), INFO(800), WARNING(900), ERROR(1000), OFF(Integer.MAX_VALUE);

            private final int severity;

            private Level(int severity) { this.severity = severity; }

            public final String getName() { return name(); }

            public final int getSeverity() { return severity; }
        }

        String getName();

        boolean isLoggable(Level level);

        default void log(Level level, String msg) {
            log(level, (java.util.ResourceBundle) null, msg, (Object[]) null);
        }

        default void log(Level level, java.util.function.Supplier<String> msgSupplier) {
            java.util.Objects.requireNonNull(msgSupplier);
            if (isLoggable(java.util.Objects.requireNonNull(level))) {
                log(level, (java.util.ResourceBundle) null, msgSupplier.get(), (Object[]) null);
            }
        }

        default void log(Level level, Object obj) {
            java.util.Objects.requireNonNull(obj);
            if (isLoggable(java.util.Objects.requireNonNull(level))) {
                this.log(level, (java.util.ResourceBundle) null, obj.toString(), (Object[]) null);
            }
        }

        default void log(Level level, String msg, Throwable thrown) {
            this.log(level, null, msg, thrown);
        }

        default void log(Level level, java.util.function.Supplier<String> msgSupplier, Throwable thrown) {
            java.util.Objects.requireNonNull(msgSupplier);
            if (isLoggable(java.util.Objects.requireNonNull(level))) {
                this.log(level, null, msgSupplier.get(), thrown);
            }
        }

        default void log(Level level, String format, Object... params) {
            this.log(level, null, format, params);
        }

        void log(Level level, java.util.ResourceBundle bundle, String msg, Throwable thrown);

        void log(Level level, java.util.ResourceBundle bundle, String format, Object... params);
    }

    /** The default System.Logger: WARNING and above (the java.util.logging default is INFO) to System.err. */
    private static final class ConsoleLogger implements Logger {
        private final String name;

        ConsoleLogger(String name) { this.name = name; }

        public String getName() { return name; }

        public boolean isLoggable(Level level) { return level.getSeverity() >= Level.INFO.getSeverity() && level != Level.OFF; }

        public void log(Level level, java.util.ResourceBundle bundle, String msg, Throwable thrown) {
            if (!isLoggable(level)) return;
            StringBuilder sb = new StringBuilder();
            sb.append(level.getName()).append(": ").append(localize(bundle, msg));
            System.err.println(sb);
            if (thrown != null) thrown.printStackTrace();
        }

        public void log(Level level, java.util.ResourceBundle bundle, String format, Object... params) {
            if (!isLoggable(level)) return;
            String m = localize(bundle, format);
            if (params != null && params.length > 0) m = java.text.MessageFormat.format(m, params);
            System.err.println(level.getName() + ": " + m);
        }

        private static String localize(java.util.ResourceBundle b, String key) {
            if (b != null && key != null && b.containsKey(key)) return b.getString(key);
            return key;
        }
    }

    public static System.Logger getLogger(String name, java.util.ResourceBundle bundle) {
        return getLogger(name);
    }

    public static java.nio.channels.Channel inheritedChannel() throws java.io.IOException {
        return null;
    }

    /** Called by the runtime when main() returns: waits for non-daemon threads, runs shutdown hooks. */
    static void monoShutdown() {
        Thread.monoAwaitNonDaemon();
        Runtime.getRuntime().runHooks();
        out.flush();
        err.flush();
    }
}
