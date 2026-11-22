package java.lang;

import java.util.HashMap;
import java.util.Map;

/**
 * j2zig overlay of java.lang.Thread: platform threads backed by native OS threads
 * (rt.startThread). {@code eetop} holds the runtime ThreadState address.
 */
public class Thread implements Runnable {
    public static final int MIN_PRIORITY = 1;
    public static final int NORM_PRIORITY = 5;
    public static final int MAX_PRIORITY = 10;

    private volatile long eetop;
    private volatile String name;
    private int priority = NORM_PRIORITY;
    private boolean daemon;
    private final Runnable target;
    private final long tid;
    private final long stackSize;
    private volatile int threadStatus; // 0 new, 1 runnable, 2 terminated
    private volatile UncaughtExceptionHandler uncaughtExceptionHandler;
    Map<Object, Object> j2zLocals;
    ThreadLocal.ThreadLocalMap threadLocals;
    ThreadLocal.ThreadLocalMap inheritableThreadLocals;
    private ClassLoader contextClassLoader;
    private ThreadGroup group;
    // accessed by java.util.concurrent.ThreadLocalRandom through Unsafe field offsets
    volatile Object parkBlocker;               // java.util.concurrent.locks.LockSupport
    Object inheritedAccessControlContext;      // ForkJoinWorkerThread
    long threadLocalRandomSeed;
    int threadLocalRandomProbe;
    int threadLocalRandomSecondarySeed;

    private static final Object lock = new Object();
    private static long nextTid = 1;
    private static int threadInitNumber;
    private static int nonDaemonCount;
    private static volatile UncaughtExceptionHandler defaultUncaughtExceptionHandler;

    private static synchronized int nextThreadNum() {
        return threadInitNumber++;
    }

    private static long nextId() {
        synchronized (lock) {
            return nextTid++;
        }
    }

    public Thread() {
        this(null, null, "Thread-" + nextThreadNum(), 0);
    }

    public Thread(Runnable task) {
        this(null, task, "Thread-" + nextThreadNum(), 0);
    }

    public Thread(ThreadGroup group, Runnable task) {
        this(group, task, "Thread-" + nextThreadNum(), 0);
    }

    public Thread(String name) {
        this(null, null, name, 0);
    }

    public Thread(ThreadGroup group, String name) {
        this(group, null, name, 0);
    }

    public Thread(Runnable task, String name) {
        this(null, task, name, 0);
    }

    public Thread(ThreadGroup group, Runnable task, String name) {
        this(group, task, name, 0);
    }

    public Thread(ThreadGroup group, Runnable task, String name, long stackSize) {
        if (name == null) throw new NullPointerException("'name' is null");
        this.name = name;
        this.target = task;
        this.stackSize = stackSize;
        this.tid = nextId();
        Thread parent = currentThread0();
        this.group = group != null ? group : (parent != null && parent.group != null ? parent.group : ThreadGroup.MAIN);
        if (parent != null) {
            this.daemon = parent.daemon;
            this.priority = parent.priority;
            this.contextClassLoader = parent.contextClassLoader;
            if (parent.inheritableThreadLocals != null)
                this.inheritableThreadLocals = ThreadLocal.createInheritedMap(parent.inheritableThreadLocals);
        }
    }

    public Thread(ThreadGroup group, Runnable task, String name, long stackSize, boolean inheritInheritableThreadLocals) {
        this(group, task, name, stackSize);
    }

    private static native Thread currentThread0();

    public static Thread currentThread() {
        return currentThread0();
    }

    public static native void yield();

    public static void sleep(long millis) throws InterruptedException {
        if (millis < 0) throw new IllegalArgumentException("timeout value is negative");
        sleep0(millis * 1000000L);
    }

    public static void sleep(long millis, int nanos) throws InterruptedException {
        if (millis < 0) throw new IllegalArgumentException("timeout value is negative");
        if (nanos < 0 || nanos > 999999) throw new IllegalArgumentException("nanosecond timeout value out of range");
        sleep0(millis * 1000000L + nanos);
    }

    public static void sleep(java.time.Duration duration) throws InterruptedException {
        long nanos = duration.toNanos();
        if (nanos < 0) return;
        sleep0(nanos);
    }

    private static native void sleep0(long nanos) throws InterruptedException;

    public static void onSpinWait() {}

    public void start() {
        synchronized (this) {
            if (threadStatus != 0) throw new IllegalThreadStateException();
            threadStatus = 1;
        }
        if (!daemon) {
            synchronized (lock) {
                nonDaemonCount++;
            }
        }
        start0(stackSize);
    }

    private native void start0(long stackSize);

    @Override
    public void run() {
        Runnable task = target;
        if (task != null) task.run();
    }

    /** Runtime entry point of a new native thread. */
    static void j2zRun(Thread t) {
        try {
            t.run();
        } catch (Throwable e) {
            t.dispatchUncaughtException(e);
        } finally {
            t.exit();
        }
    }

    private void exit() {
        synchronized (this) {
            threadStatus = 2;
            notifyAll();
        }
        if (!daemon) {
            synchronized (lock) {
                nonDaemonCount--;
                lock.notifyAll();
            }
        }
    }

    static Thread j2zStartMain() {
        Thread t = new Thread(null, null, "main", 0);
        t.threadStatus = 1;
        return t;
    }

    /** Runtime hook: Thread object for a native thread that calls into Java (JNI attach). */
    static Thread j2zForeign() {
        Thread t = new Thread(null, null, "Thread-" + nextThreadNum(), 0);
        t.daemon = true;
        t.threadStatus = 1;
        return t;
    }

    static void j2zUncaught(Thread t, Throwable e) {
        if (t == null) {
            System.err.print("Exception in thread \"main\" ");
            e.printStackTrace(System.err);
            return;
        }
        t.dispatchUncaughtException(e);
    }

    static void j2zAwaitNonDaemon() {
        synchronized (lock) {
            while (nonDaemonCount > 0) {
                try {
                    lock.wait();
                } catch (InterruptedException ignored) {
                }
            }
        }
    }

    void dispatchUncaughtException(Throwable e) {
        UncaughtExceptionHandler h = getUncaughtExceptionHandler();
        try {
            if (h == null) h = ThreadGroup.MAIN;
            h.uncaughtException(this, e);
        } catch (Throwable ignore) {
        }
    }

    // interruptible I/O (java.nio.channels.spi.AbstractInterruptibleChannel), as in the JDK
    private volatile sun.nio.ch.Interruptible nioBlocker;
    private final Object blockerLock = new Object();

    static void blockedOn(sun.nio.ch.Interruptible b) {
        Thread me = currentThread();
        synchronized (me.blockerLock) {
            me.nioBlocker = b;
        }
    }

    public void interrupt() {
        if (this != currentThread()) {
            synchronized (blockerLock) {
                sun.nio.ch.Interruptible b = nioBlocker;
                if (b != null) {
                    interrupt0();
                    b.interrupt(this);
                    b.postInterrupt();
                    return;
                }
            }
        }
        interrupt0();
        sun.nio.ch.Interruptible b = nioBlocker;
        if (b != null) b.postInterrupt();
    }

    private native void interrupt0();

    public static boolean interrupted() {
        return currentThread().clearInterrupt0();
    }

    private native boolean clearInterrupt0();

    public boolean isInterrupted() {
        return isInterrupted0();
    }

    private native boolean isInterrupted0();

    public final boolean isAlive() {
        return threadStatus == 1;
    }

    public final void setPriority(int newPriority) {
        if (newPriority > MAX_PRIORITY || newPriority < MIN_PRIORITY) throw new IllegalArgumentException();
        priority = newPriority;
    }

    public final int getPriority() {
        return priority;
    }

    public final synchronized void setName(String name) {
        if (name == null) throw new NullPointerException("name cannot be null");
        this.name = name;
    }

    public final String getName() {
        return name;
    }

    public final ThreadGroup getThreadGroup() {
        return threadStatus == 2 ? null : group;
    }

    public static int activeCount() {
        return 1;
    }

    public final void join(long millis) throws InterruptedException {
        if (millis < 0) throw new IllegalArgumentException("timeout value is negative");
        synchronized (this) {
            if (millis > 0) {
                long deadline = System.nanoTime() + millis * 1000000L;
                while (isAlive()) {
                    long left = (deadline - System.nanoTime()) / 1000000L;
                    if (left <= 0) break;
                    wait(left);
                }
            } else {
                while (isAlive()) wait(0);
            }
        }
    }

    public final void join(long millis, int nanos) throws InterruptedException {
        join(millis + (nanos > 0 ? 1 : 0));
    }

    public final void join() throws InterruptedException {
        join(0);
    }

    public final boolean join(java.time.Duration duration) throws InterruptedException {
        join(Math.max(1, duration.toMillis()));
        return !isAlive();
    }

    public static void dumpStack() {
        new Exception("Stack trace").printStackTrace();
    }

    public final void setDaemon(boolean on) {
        if (isAlive()) throw new IllegalThreadStateException();
        daemon = on;
    }

    public final boolean isDaemon() {
        return daemon;
    }

    public final boolean isVirtual() {
        return false;
    }

    public final void checkAccess() {}

    public String toString() {
        return "Thread[#" + threadId() + "," + getName() + "," + getPriority() + "," + "main" + "]";
    }

    public ClassLoader getContextClassLoader() {
        return contextClassLoader;
    }

    public void setContextClassLoader(ClassLoader cl) {
        contextClassLoader = cl;
    }

    public static native boolean holdsLock(Object obj);

    public StackTraceElement[] getStackTrace() {
        if (this == currentThread()) {
            StackTraceElement[] st = new Exception().getStackTrace();
            return st;
        }
        return new StackTraceElement[0];
    }

    @Deprecated(since = "19")
    public long getId() {
        return threadId();
    }

    public final long threadId() {
        return tid;
    }

    public enum State { NEW, RUNNABLE, BLOCKED, WAITING, TIMED_WAITING, TERMINATED }

    public State getState() {
        switch (threadStatus) {
            case 0: return State.NEW;
            case 1: return State.RUNNABLE;
            default: return State.TERMINATED;
        }
    }

    @FunctionalInterface
    public interface UncaughtExceptionHandler {
        void uncaughtException(Thread t, Throwable e);
    }

    public static void setDefaultUncaughtExceptionHandler(UncaughtExceptionHandler ueh) {
        defaultUncaughtExceptionHandler = ueh;
    }

    public static UncaughtExceptionHandler getDefaultUncaughtExceptionHandler() {
        return defaultUncaughtExceptionHandler;
    }

    public UncaughtExceptionHandler getUncaughtExceptionHandler() {
        UncaughtExceptionHandler h = uncaughtExceptionHandler;
        return h != null ? h : group;
    }

    public void setUncaughtExceptionHandler(UncaughtExceptionHandler ueh) {
        uncaughtExceptionHandler = ueh;
    }

    /**
     * Virtual threads are mapped onto platform (OS) threads in this runtime: the semantics
     * visible to programs are the same, only the scalability characteristics differ.
     */
    public static Builder.OfVirtual ofVirtual() {
        return new PlatformBuilder();
    }

    public static Thread startVirtualThread(Runnable task) {
        Thread t = new Thread(task);
        t.setDaemon(true);
        t.start();
        return t;
    }

    public static java.util.Map<Thread, StackTraceElement[]> getAllStackTraces() {
        java.util.Map<Thread, StackTraceElement[]> m = new java.util.HashMap<>();
        m.put(currentThread(), currentThread().getStackTrace());
        return m;
    }

    public static int enumerate(Thread[] tarray) {
        if (tarray.length == 0) return 0;
        tarray[0] = currentThread();
        return 1;
    }

    public static Builder.OfPlatform ofPlatform() {
        return new PlatformBuilder();
    }

    public interface Builder {
        Builder name(String name);
        Builder name(String prefix, long start);
        Builder inheritInheritableThreadLocals(boolean inherit);
        Builder uncaughtExceptionHandler(UncaughtExceptionHandler ueh);
        Thread unstarted(Runnable task);
        Thread start(Runnable task);
        java.util.concurrent.ThreadFactory factory();

        interface OfPlatform extends Builder {
            @Override OfPlatform name(String name);
            @Override OfPlatform name(String prefix, long start);
            @Override OfPlatform inheritInheritableThreadLocals(boolean inherit);
            @Override OfPlatform uncaughtExceptionHandler(UncaughtExceptionHandler ueh);
            OfPlatform group(ThreadGroup group);
            OfPlatform daemon(boolean on);
            default OfPlatform daemon() { return daemon(true); }
            OfPlatform priority(int priority);
            OfPlatform stackSize(long stackSize);
        }

        interface OfVirtual extends Builder {
            @Override OfVirtual name(String name);
            @Override OfVirtual name(String prefix, long start);
            @Override OfVirtual inheritInheritableThreadLocals(boolean inherit);
            @Override OfVirtual uncaughtExceptionHandler(UncaughtExceptionHandler ueh);
        }
    }

    private static final class PlatformBuilder implements Builder.OfPlatform, Builder.OfVirtual {
        private String name;
        private long counter = -1;
        private boolean daemon, daemonSet;
        private int priority;
        private long stackSize;
        private ThreadGroup group;
        private UncaughtExceptionHandler ueh;
        private boolean virtual;

        PlatformBuilder() {}

        public PlatformBuilder name(String name) { this.name = java.util.Objects.requireNonNull(name); counter = -1; return this; }
        public PlatformBuilder name(String prefix, long start) {
            java.util.Objects.requireNonNull(prefix);
            if (start < 0) throw new IllegalArgumentException("'start' is negative");
            this.name = prefix; counter = start; return this;
        }
        public PlatformBuilder inheritInheritableThreadLocals(boolean inherit) { return this; }
        public PlatformBuilder group(ThreadGroup g) { group = java.util.Objects.requireNonNull(g); return this; }
        public PlatformBuilder daemon(boolean on) { daemon = on; daemonSet = true; return this; }
        public PlatformBuilder priority(int p) {
            if (p < MIN_PRIORITY || p > MAX_PRIORITY) throw new IllegalArgumentException();
            priority = p; return this;
        }
        public PlatformBuilder stackSize(long s) {
            if (s < 0) throw new IllegalArgumentException();
            stackSize = s; return this;
        }
        public PlatformBuilder uncaughtExceptionHandler(UncaughtExceptionHandler u) { ueh = java.util.Objects.requireNonNull(u); return this; }

        public Thread unstarted(Runnable task) {
            java.util.Objects.requireNonNull(task);
            String n = name == null ? "Thread-" + nextThreadNum() : counter >= 0 ? name + (counter++) : name;
            Thread t = new Thread(group, task, n, stackSize);
            if (daemonSet) t.setDaemon(daemon);
            if (priority != 0) t.setPriority(priority);
            if (ueh != null) t.setUncaughtExceptionHandler(ueh);
            return t;
        }

        public Thread start(Runnable task) {
            Thread t = unstarted(task);
            t.start();
            return t;
        }

        public java.util.concurrent.ThreadFactory factory() {
            return this::unstarted;
        }
    }

    // ---- ScopedValue support (jdk.internal.misc.ScopedValue*): per-thread bindings and cache
    private Object scopedValueBindings;
    private Object[] scopedValueCache;

    static Object scopedValueBindings() { return currentThread().scopedValueBindings; }
    static void setScopedValueBindings(Object bindings) { currentThread().scopedValueBindings = bindings; }
    static Object[] scopedValueCache() { return currentThread().scopedValueCache; }
    static void setScopedValueCache(Object[] cache) { currentThread().scopedValueCache = cache; }
    static Object findScopedValueBindings() { return currentThread().scopedValueBindings; }
    static void ensureMaterializedForStackWalk(Object o) {}
}
