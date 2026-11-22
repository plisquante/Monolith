package java.util.concurrent.atomic;

/** j2zig overlay. */
public class AtomicBoolean implements java.io.Serializable {
    private volatile int value;

    public AtomicBoolean(boolean initialValue) { value = initialValue ? 1 : 0; }
    public AtomicBoolean() {}

    private native boolean cas(int expected, int newValue);

    public final boolean get() { return value != 0; }
    public final boolean compareAndSet(boolean expectedValue, boolean newValue) { return cas(expectedValue ? 1 : 0, newValue ? 1 : 0); }
    public boolean weakCompareAndSet(boolean expectedValue, boolean newValue) { return compareAndSet(expectedValue, newValue); }
    public boolean weakCompareAndSetPlain(boolean expectedValue, boolean newValue) { return compareAndSet(expectedValue, newValue); }
    public boolean weakCompareAndSetVolatile(boolean expectedValue, boolean newValue) { return compareAndSet(expectedValue, newValue); }
    public final void set(boolean newValue) { value = newValue ? 1 : 0; }
    public final void lazySet(boolean newValue) { set(newValue); }
    public final boolean getAndSet(boolean newValue) { while (true) { int v = value; if (cas(v, newValue ? 1 : 0)) return v != 0; } }
    public String toString() { return Boolean.toString(get()); }
    public final boolean getPlain() { return value != 0; }
    public final void setPlain(boolean newValue) { set(newValue); }
    public final boolean getOpaque() { return get(); }
    public final void setOpaque(boolean newValue) { set(newValue); }
    public final boolean getAcquire() { return get(); }
    public final void setRelease(boolean newValue) { set(newValue); }
    public final boolean compareAndExchange(boolean e, boolean n) { while (true) { boolean v = get(); if (v != e) return v; if (compareAndSet(v, n)) return v; } }

    public final boolean compareAndExchangeAcquire(boolean expectedValue, boolean newValue) { return compareAndExchange(expectedValue, newValue); }
    public final boolean compareAndExchangeRelease(boolean expectedValue, boolean newValue) { return compareAndExchange(expectedValue, newValue); }
    public boolean weakCompareAndSetAcquire(boolean expectedValue, boolean newValue) { return compareAndSet(expectedValue, newValue); }
    public boolean weakCompareAndSetRelease(boolean expectedValue, boolean newValue) { return compareAndSet(expectedValue, newValue); }
}
