package java.lang;

/** monolith overlay of ThreadGroup: a named tree used for daemon/priority defaults and uncaught exceptions. */
public class ThreadGroup implements Thread.UncaughtExceptionHandler {
    static final ThreadGroup SYSTEM = new ThreadGroup();
    static final ThreadGroup MAIN = new ThreadGroup(SYSTEM, "main");

    private final ThreadGroup parent;
    private final String name;
    private volatile int maxPriority = Thread.MAX_PRIORITY;
    private volatile boolean daemon;

    private ThreadGroup() {
        this.parent = null;
        this.name = "system";
    }

    public ThreadGroup(String name) {
        this(Thread.currentThread() == null ? MAIN : Thread.currentThread().getThreadGroup(), name);
    }

    public ThreadGroup(ThreadGroup parent, String name) {
        if (parent == null) throw new NullPointerException();
        this.parent = parent;
        this.name = name;
        this.maxPriority = parent.maxPriority;
        this.daemon = parent.daemon;
    }

    public final String getName() { return name; }
    public final ThreadGroup getParent() { return parent; }
    public final int getMaxPriority() { return maxPriority; }
    @Deprecated(since = "16", forRemoval = true)
    public final boolean isDaemon() { return daemon; }
    @Deprecated(since = "16", forRemoval = true)
    public boolean isDestroyed() { return false; }
    @Deprecated(since = "16", forRemoval = true)
    public final void setDaemon(boolean daemon) { this.daemon = daemon; }
    public final void setMaxPriority(int pri) {
        if (pri >= Thread.MIN_PRIORITY && pri <= Thread.MAX_PRIORITY)
            maxPriority = parent != null ? Math.min(pri, parent.maxPriority) : pri;
    }
    public final boolean parentOf(ThreadGroup g) {
        for (; g != null; g = g.parent) if (g == this) return true;
        return false;
    }
    @Deprecated(since = "17", forRemoval = true)
    public final void checkAccess() {}
    public int activeCount() { return 0; }
    public int enumerate(Thread[] list) { return 0; }
    public int enumerate(Thread[] list, boolean recurse) { return 0; }
    public int activeGroupCount() { return 0; }
    public int enumerate(ThreadGroup[] list) { return 0; }
    public int enumerate(ThreadGroup[] list, boolean recurse) { return 0; }
    public final void interrupt() {}
    public void list() { System.out.println(this); }

    public void uncaughtException(Thread t, Throwable e) {
        if (parent != null) {
            parent.uncaughtException(t, e);
        } else {
            Thread.UncaughtExceptionHandler ueh = Thread.getDefaultUncaughtExceptionHandler();
            if (ueh != null) {
                ueh.uncaughtException(t, e);
            } else {
                System.err.print("Exception in thread \"" + t.getName() + "\" ");
                e.printStackTrace(System.err);
            }
        }
    }

    public String toString() {
        return getClass().getName() + "[name=" + getName() + ",maxpri=" + getMaxPriority() + "]";
    }
}
