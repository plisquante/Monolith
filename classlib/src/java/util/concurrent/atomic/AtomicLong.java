package java.util.concurrent.atomic;

import java.util.function.LongBinaryOperator;
import java.util.function.LongUnaryOperator;

/** monolith overlay: lock-free updates are implemented with the runtime's native CAS on the value field. */
public class AtomicLong extends Number implements java.io.Serializable {
    private volatile long value;

    public AtomicLong(long initialValue) { value = initialValue; }
    public AtomicLong() {}

    private native boolean cas(long expected, long newValue);

    public final long get() { return value; }
    public final void set(long newValue) { value = newValue; }
    public final void lazySet(long newValue) { value = newValue; }
    public final long getAndSet(long newValue) { while (true) { long v = value; if (cas(v, newValue)) return v; } }
    public final boolean compareAndSet(long expectedValue, long newValue) { return cas(expectedValue, newValue); }
    public final boolean weakCompareAndSet(long expectedValue, long newValue) { return cas(expectedValue, newValue); }
    public final boolean weakCompareAndSetPlain(long expectedValue, long newValue) { return cas(expectedValue, newValue); }
    public final long getAndIncrement() { return getAndAdd(1); }
    public final long getAndDecrement() { return getAndAdd(-1); }
    public final long getAndAdd(long delta) { while (true) { long v = value; if (cas(v, v + delta)) return v; } }
    public final long incrementAndGet() { return getAndAdd(1) + 1; }
    public final long decrementAndGet() { return getAndAdd(-1) - 1; }
    public final long addAndGet(long delta) { return getAndAdd(delta) + delta; }
    public final long getAndUpdate(LongUnaryOperator f) { while (true) { long v = value; long n = f.applyAsLong(v); if (cas(v, n)) return v; } }
    public final long updateAndGet(LongUnaryOperator f) { while (true) { long v = value; long n = f.applyAsLong(v); if (cas(v, n)) return n; } }
    public final long getAndAccumulate(long x, LongBinaryOperator f) { while (true) { long v = value; long n = f.applyAsLong(v, x); if (cas(v, n)) return v; } }
    public final long accumulateAndGet(long x, LongBinaryOperator f) { while (true) { long v = value; long n = f.applyAsLong(v, x); if (cas(v, n)) return n; } }
    public String toString() { return Long.toString(get()); }
    public int intValue() { return (int) get(); }
    public long longValue() { return (long) get(); }
    public float floatValue() { return (float) get(); }
    public double doubleValue() { return (double) get(); }
    public final long getPlain() { return value; }
    public final void setPlain(long newValue) { value = newValue; }
    public final long getOpaque() { return value; }
    public final void setOpaque(long newValue) { value = newValue; }
    public final long getAcquire() { return value; }
    public final void setRelease(long newValue) { value = newValue; }
    public final long compareAndExchange(long expectedValue, long newValue) { while (true) { long v = value; if (v != expectedValue) return v; if (cas(v, newValue)) return v; } }

    public final long compareAndExchangeAcquire(long expectedValue, long newValue) { return compareAndExchange(expectedValue, newValue); }
    public final long compareAndExchangeRelease(long expectedValue, long newValue) { return compareAndExchange(expectedValue, newValue); }
    public final boolean weakCompareAndSetVolatile(long expectedValue, long newValue) { return compareAndSet(expectedValue, newValue); }
    public final boolean weakCompareAndSetAcquire(long expectedValue, long newValue) { return compareAndSet(expectedValue, newValue); }
    public final boolean weakCompareAndSetRelease(long expectedValue, long newValue) { return compareAndSet(expectedValue, newValue); }
}
