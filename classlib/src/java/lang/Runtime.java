package java.lang;

import java.util.ArrayList;

/** monolith overlay of java.lang.Runtime. */
public class Runtime {
    private static final Runtime currentRuntime = new Runtime();
    private final ArrayList<Thread> hooks = new ArrayList<>();
    private boolean shuttingDown;

    public static Runtime getRuntime() {
        return currentRuntime;
    }

    private Runtime() {}

    public void exit(int status) {
        runHooks();
        System.out.flush();
        System.err.flush();
        halt0(status);
    }

    public void halt(int status) {
        halt0(status);
    }

    private static native void halt0(int status);

    public void addShutdownHook(Thread hook) {
        synchronized (hooks) {
            if (shuttingDown) throw new IllegalStateException("Shutdown in progress");
            if (hooks.contains(hook)) throw new IllegalArgumentException("Hook previously registered");
            hooks.add(hook);
        }
    }

    public boolean removeShutdownHook(Thread hook) {
        synchronized (hooks) {
            return hooks.remove(hook);
        }
    }

    void runHooks() {
        ArrayList<Thread> list;
        synchronized (hooks) {
            if (shuttingDown) return;
            shuttingDown = true;
            list = new ArrayList<>(hooks);
        }
        for (Thread t : list) t.start();
        // A hook that never finishes would keep the process alive forever; on Android that leaves a frozen black
        // window after Minecraft's crash handler called System.exit. Hooks get hookTimeoutMs in total; the ones
        // still running are reported by name and the process halts anyway.
        final long hookTimeoutMs = 10_000L; // a local, not a static field: keeps this class's facades unchanged
        long deadline = System.currentTimeMillis() + hookTimeoutMs;
        for (Thread t : list) {
            while (t.isAlive()) {
                long left = deadline - System.currentTimeMillis();
                if (left <= 0) break;
                try {
                    t.join(left);
                } catch (InterruptedException ignored) {
                }
            }
        }
        for (Thread t : list) {
            if (t.isAlive()) {
                System.err.println("monolith: shutdown hook '" + t.getName() + "' did not finish within "
                        + hookTimeoutMs + " ms; halting without it");
            }
        }
    }

    /**
     * Native libraries are dlopen()ed at run time (JNI): libraries in the natives directory are
     * loaded when the first JNI call needs them; load/loadLibrary load additional ones by path.
     */
    public void load(String filename) {
        load0(null, filename);
    }

    public void loadLibrary(String libname) {
        loadLibrary0(null, libname);
    }

    void load0(Class<?> fromClass, String filename) {
        if (!new java.io.File(filename).isAbsolute())
            throw new UnsatisfiedLinkError("Expecting an absolute path of the library: " + filename);
        if (!load1(filename)) throw new UnsatisfiedLinkError("Can't load library: " + filename);
    }

    void loadLibrary0(Class<?> fromClass, String libname) {
        if (libname.indexOf('/') >= 0) throw new UnsatisfiedLinkError("Directory separator should not appear in library name: " + libname);
        // libraries of the JDK itself (java, net, nio, zip, management, ...) are part of the runtime
        switch (libname) {
            case "java": case "net": case "nio": case "zip": case "jimage": case "management": case "management_ext":
            case "prefs": case "extnet": case "instrument": case "j2gss": case "j2pcsc": case "j2pkcs11": case "jaas":
            case "jsig": case "syslookup": case "verify": case "attach": case "rmi": case "sctp":
                return;
            default:
        }
        String file = System.mapLibraryName(libname);
        String path = System.getProperty("java.library.path", "");
        for (String dir : path.split(":")) {
            if (dir.isEmpty()) continue;
            java.io.File f = new java.io.File(dir, file);
            if (f.isFile()) {
                if (!load1(f.getPath())) throw new UnsatisfiedLinkError("Can't load library: " + f);
                return;
            }
        }
        if (isPreloaded(file)) return; // shipped in <exe>/natives and loaded for JNI lookups
        throw new UnsatisfiedLinkError("no " + libname + " in java.library.path: " + path);
    }

    private native boolean load1(String path);

    private native boolean isPreloaded(String file);

    public Process exec(String command) throws java.io.IOException {
        return exec(command.split(" "));
    }

    public Process exec(String[] cmdarray) throws java.io.IOException {
        return new ProcessBuilder(cmdarray).start();
    }

    public Process exec(String[] cmdarray, String[] envp) throws java.io.IOException {
        return exec(cmdarray);
    }

    public native int availableProcessors();
    public native long freeMemory();
    public native long totalMemory();
    public native long maxMemory();
    public native void gc();

    public void runFinalization() {}

    private static volatile Version version;

    public static Version version() {
        Version v = version;
        if (v == null) version = v = Version.parse(System.getProperty("java.runtime.version", System.getProperty("java.version", "25")));
        return v;
    }

    public static final class Version implements Comparable<Version> {
        private final java.util.List<Integer> version;
        private final java.util.Optional<String> pre;
        private final java.util.Optional<Integer> build;
        private final java.util.Optional<String> optional;

        private Version(java.util.List<Integer> version, java.util.Optional<String> pre, java.util.Optional<Integer> build, java.util.Optional<String> optional) {
            this.version = version; this.pre = pre; this.build = build; this.optional = optional;
        }

        /** Grammar: $VNUM(-$PRE)?(\+($BUILD)?)?(\-$OPT)? (JEP 223, as java.lang.Runtime.Version.parse). */
        public static Version parse(String s) {
            if (s == null) throw new NullPointerException();
            java.util.regex.Matcher m = java.util.regex.Pattern.compile(
                    "^([1-9][0-9]*(?:(?:\\.0)*\\.[1-9][0-9]*)*)(?:-([a-zA-Z0-9]+))?(?:(\\+)(0|[1-9][0-9]*)?)?(?:-([-a-zA-Z0-9.]+))?$").matcher(s);
            if (!m.matches()) throw new IllegalArgumentException("Invalid version string: '" + s + "'");
            java.util.List<Integer> nums = new java.util.ArrayList<>();
            for (String part : m.group(1).split("\\.")) nums.add(Integer.parseInt(part));
            java.util.Optional<String> pre = java.util.Optional.ofNullable(m.group(2));
            java.util.Optional<Integer> build = m.group(4) == null ? java.util.Optional.empty() : java.util.Optional.of(Integer.parseInt(m.group(4)));
            java.util.Optional<String> opt = java.util.Optional.ofNullable(m.group(5));
            return new Version(java.util.Collections.unmodifiableList(nums), pre, build, opt);
        }

        public java.util.List<Integer> version() { return version; }
        private int at(int i) { return i < version.size() ? version.get(i) : 0; }
        public int feature() { return version.get(0); }
        public int interim() { return at(1); }
        public int update() { return at(2); }
        public int patch() { return at(3); }
        @Deprecated(since = "10") public int major() { return feature(); }
        @Deprecated(since = "10") public int minor() { return interim(); }
        @Deprecated(since = "10") public int security() { return update(); }
        public java.util.Optional<String> pre() { return pre; }
        public java.util.Optional<Integer> build() { return build; }
        public java.util.Optional<String> optional() { return optional; }

        public int compareTo(Version obj) { return compare(obj, false); }
        public int compareToIgnoreOptional(Version obj) { return compare(obj, true); }

        private int compare(Version o, boolean ignoreOpt) {
            // same ordering and magnitudes as the JDK: version numbers by difference, then size,
            // pre-release, build, optional
            int min = Math.min(version.size(), o.version.size());
            for (int i = 0; i < min; i++) {
                int val = version.get(i), oVal = o.version.get(i);
                if (val != oVal) return val - oVal;
            }
            int c = version.size() - o.version.size();
            if (c != 0) return c;
            c = comparePre(o);
            if (c != 0) return c;
            c = compareBuild(o);
            if (c != 0) return c;
            return ignoreOpt ? 0 : compareOptional(o);
        }

        private int comparePre(Version o) {
            if (pre.isEmpty()) return o.pre.isPresent() ? 1 : 0;
            if (o.pre.isEmpty()) return -1;
            String val = pre.get(), oVal = o.pre.get();
            boolean vn = val.matches("\\d+");
            boolean on = oVal.matches("\\d+");
            if (vn) return on ? new java.math.BigInteger(val).compareTo(new java.math.BigInteger(oVal)) : -1;
            return on ? 1 : val.compareTo(oVal);
        }

        private int compareBuild(Version o) {
            if (o.build.isPresent()) return build.isPresent() ? build.get().compareTo(o.build.get()) : -1;
            return build.isPresent() ? 1 : 0;
        }

        private int compareOptional(Version o) {
            if (optional.isEmpty()) return o.optional.isPresent() ? -1 : 0;
            if (o.optional.isEmpty()) return 1;
            return optional.get().compareTo(o.optional.get());
        }

        public boolean equals(Object o) { return o instanceof Version v && compare(v, false) == 0; }
        public boolean equalsIgnoreOptional(Object o) { return o instanceof Version v && compare(v, true) == 0; }
        public int hashCode() { return java.util.Objects.hash(version, pre, build, optional); }

        public String toString() {
            StringBuilder sb = new StringBuilder();
            for (int i = 0; i < version.size(); i++) { if (i > 0) sb.append('.'); sb.append(version.get(i)); }
            pre.ifPresent(p -> sb.append('-').append(p));
            if (build.isPresent()) sb.append('+').append(build.get());
            if (optional.isPresent()) { if (build.isEmpty() && pre.isEmpty()) sb.append("+"); sb.append('-').append(optional.get()); }
            return sb.toString();
        }
    }
}
