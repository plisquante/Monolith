package java.lang;

/** j2zig overlay: root of the class hierarchy. Identity hash, monitors and cloning are runtime natives. */
public class Object {
    public Object() {}

    public final native Class<?> getClass();

    public native int hashCode();

    public boolean equals(Object obj) {
        return this == obj;
    }

    protected native Object clone() throws CloneNotSupportedException;

    public String toString() {
        return getClass().getName() + "@" + Integer.toHexString(hashCode());
    }

    public final native void notify();

    public final native void notifyAll();

    public final void wait() throws InterruptedException {
        wait0(0L, 0);
    }

    public final void wait(long timeoutMillis) throws InterruptedException {
        if (timeoutMillis < 0) throw new IllegalArgumentException("timeout value is negative");
        wait0(timeoutMillis, 0);
    }

    public final void wait(long timeoutMillis, int nanos) throws InterruptedException {
        if (timeoutMillis < 0) throw new IllegalArgumentException("timeout value is negative");
        if (nanos < 0 || nanos > 999999) throw new IllegalArgumentException("nanosecond timeout value out of range");
        wait0(timeoutMillis, nanos);
    }

    private native void wait0(long millis, int nanos) throws InterruptedException;

    @Deprecated(since = "9", forRemoval = true)
    protected void finalize() throws Throwable {}
}
