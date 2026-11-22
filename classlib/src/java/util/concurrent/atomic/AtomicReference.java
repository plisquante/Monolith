package java.util.concurrent.atomic;

import java.util.function.BinaryOperator;
import java.util.function.UnaryOperator;

/** j2zig overlay. */
public class AtomicReference<V> implements java.io.Serializable {
    private volatile V value;

    public AtomicReference(V initialValue) { value = initialValue; }
    public AtomicReference() {}

    private native boolean cas(Object expected, Object newValue);

    public final V get() { return value; }
    public final void set(V newValue) { value = newValue; }
    public final void lazySet(V newValue) { value = newValue; }
    public final boolean compareAndSet(V expectedValue, V newValue) { return cas(expectedValue, newValue); }
    public final boolean weakCompareAndSet(V expectedValue, V newValue) { return cas(expectedValue, newValue); }
    public final boolean weakCompareAndSetPlain(V expectedValue, V newValue) { return cas(expectedValue, newValue); }
    public final boolean weakCompareAndSetVolatile(V expectedValue, V newValue) { return cas(expectedValue, newValue); }
    public final V getAndSet(V newValue) { while (true) { V v = value; if (cas(v, newValue)) return v; } }
    public final V getAndUpdate(UnaryOperator<V> f) { while (true) { V v = value; V n = f.apply(v); if (cas(v, n)) return v; } }
    public final V updateAndGet(UnaryOperator<V> f) { while (true) { V v = value; V n = f.apply(v); if (cas(v, n)) return n; } }
    public final V getAndAccumulate(V x, BinaryOperator<V> f) { while (true) { V v = value; V n = f.apply(v, x); if (cas(v, n)) return v; } }
    public final V accumulateAndGet(V x, BinaryOperator<V> f) { while (true) { V v = value; V n = f.apply(v, x); if (cas(v, n)) return n; } }
    public String toString() { return String.valueOf(get()); }
    public final V getPlain() { return value; }
    public final void setPlain(V newValue) { value = newValue; }
    public final V getOpaque() { return value; }
    public final void setOpaque(V newValue) { value = newValue; }
    public final V getAcquire() { return value; }
    public final void setRelease(V newValue) { value = newValue; }
    public final V compareAndExchange(V e, V n) { while (true) { V v = value; if (v != e) return v; if (cas(v, n)) return v; } }

    public final V compareAndExchangeAcquire(V expectedValue, V newValue) { return compareAndExchange(expectedValue, newValue); }
    public final V compareAndExchangeRelease(V expectedValue, V newValue) { return compareAndExchange(expectedValue, newValue); }
    public final boolean weakCompareAndSetAcquire(V expectedValue, V newValue) { return compareAndSet(expectedValue, newValue); }
    public final boolean weakCompareAndSetRelease(V expectedValue, V newValue) { return compareAndSet(expectedValue, newValue); }
}
