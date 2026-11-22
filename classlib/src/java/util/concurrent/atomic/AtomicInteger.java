package java.util.concurrent.atomic;

import java.util.function.IntBinaryOperator;
import java.util.function.IntUnaryOperator;

/** j2zig overlay: lock-free updates are implemented with the runtime's native CAS on the value field. */
public class AtomicInteger extends Number implements java.io.Serializable {
    private volatile int value;

    public AtomicInteger(int initialValue) { value = initialValue; }
    public AtomicInteger() {}

    private native boolean cas(int expected, int newValue);

    public final int get() { return value; }
    public final void set(int newValue) { value = newValue; }
    public final void lazySet(int newValue) { value = newValue; }
    public final int getAndSet(int newValue) { while (true) { int v = value; if (cas(v, newValue)) return v; } }
    public final boolean compareAndSet(int expectedValue, int newValue) { return cas(expectedValue, newValue); }
    public final boolean weakCompareAndSet(int expectedValue, int newValue) { return cas(expectedValue, newValue); }
    public final boolean weakCompareAndSetPlain(int expectedValue, int newValue) { return cas(expectedValue, newValue); }
    public final int getAndIncrement() { return getAndAdd(1); }
    public final int getAndDecrement() { return getAndAdd(-1); }
    public final int getAndAdd(int delta) { while (true) { int v = value; if (cas(v, v + delta)) return v; } }
    public final int incrementAndGet() { return getAndAdd(1) + 1; }
    public final int decrementAndGet() { return getAndAdd(-1) - 1; }
    public final int addAndGet(int delta) { return getAndAdd(delta) + delta; }
    public final int getAndUpdate(IntUnaryOperator f) { while (true) { int v = value; int n = f.applyAsInt(v); if (cas(v, n)) return v; } }
    public final int updateAndGet(IntUnaryOperator f) { while (true) { int v = value; int n = f.applyAsInt(v); if (cas(v, n)) return n; } }
    public final int getAndAccumulate(int x, IntBinaryOperator f) { while (true) { int v = value; int n = f.applyAsInt(v, x); if (cas(v, n)) return v; } }
    public final int accumulateAndGet(int x, IntBinaryOperator f) { while (true) { int v = value; int n = f.applyAsInt(v, x); if (cas(v, n)) return n; } }
    public String toString() { return Integer.toString(get()); }
    public int intValue() { return (int) get(); }
    public long longValue() { return (long) get(); }
    public float floatValue() { return (float) get(); }
    public double doubleValue() { return (double) get(); }
    public final int getPlain() { return value; }
    public final void setPlain(int newValue) { value = newValue; }
    public final int getOpaque() { return value; }
    public final void setOpaque(int newValue) { value = newValue; }
    public final int getAcquire() { return value; }
    public final void setRelease(int newValue) { value = newValue; }
    public final int compareAndExchange(int expectedValue, int newValue) { while (true) { int v = value; if (v != expectedValue) return v; if (cas(v, newValue)) return v; } }

    public final int compareAndExchangeAcquire(int expectedValue, int newValue) { return compareAndExchange(expectedValue, newValue); }
    public final int compareAndExchangeRelease(int expectedValue, int newValue) { return compareAndExchange(expectedValue, newValue); }
    public final boolean weakCompareAndSetVolatile(int expectedValue, int newValue) { return compareAndSet(expectedValue, newValue); }
    public final boolean weakCompareAndSetAcquire(int expectedValue, int newValue) { return compareAndSet(expectedValue, newValue); }
    public final boolean weakCompareAndSetRelease(int expectedValue, int newValue) { return compareAndSet(expectedValue, newValue); }
}
