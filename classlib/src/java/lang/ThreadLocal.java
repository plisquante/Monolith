package java.lang;

import java.util.IdentityHashMap;
import java.util.Objects;
import java.util.function.Supplier;

/** monolith overlay of ThreadLocal backed by a per-thread identity map. */
public class ThreadLocal<T> {
    private static final Object NULL = new Object();

    public ThreadLocal() {}

    protected T initialValue() {
        return null;
    }

    public static <S> ThreadLocal<S> withInitial(Supplier<? extends S> supplier) {
        Objects.requireNonNull(supplier);
        return new ThreadLocal<S>() {
            @Override
            protected S initialValue() {
                return supplier.get();
            }
        };
    }

    ThreadLocalMap getMap(Thread t) {
        return t.threadLocals;
    }

    void createMap(Thread t) {
        t.threadLocals = new ThreadLocalMap();
    }

    public T get() {
        return get(Thread.currentThread());
    }

    @SuppressWarnings("unchecked")
    private T get(Thread t) {
        ThreadLocalMap map = getMap(t);
        if (map == null) {
            createMap(t);
            map = getMap(t);
        }
        Object v = map.map.get(this);
        if (v == null) {
            T init = initialValue();
            map.map.put(this, init == null ? NULL : init);
            return init;
        }
        return v == NULL ? null : (T) v;
    }

    public void set(T value) {
        set(Thread.currentThread(), value);
    }

    private void set(Thread t, T value) {
        ThreadLocalMap map = getMap(t);
        if (map == null) {
            createMap(t);
            map = getMap(t);
        }
        map.map.put(this, value == null ? NULL : value);
    }

    public void remove() {
        remove(Thread.currentThread());
    }

    private void remove(Thread t) {
        ThreadLocalMap map = getMap(t);
        if (map != null) map.map.remove(this);
    }

    // jdk.internal.misc.CarrierThreadLocal support (all threads are platform threads, so the
    // carrier thread is the current thread); non-virtual so CarrierThreadLocal.get() can use it
    T getCarrierThreadLocal() {
        return get(Thread.currentThread());
    }

    void setCarrierThreadLocal(T value) {
        set(Thread.currentThread(), value);
    }

    void removeCarrierThreadLocal() {
        remove(Thread.currentThread());
    }

    boolean isCarrierThreadLocalPresent() {
        ThreadLocalMap map = getMap(Thread.currentThread());
        return map != null && map.map.containsKey(this);
    }

    T childValue(T parentValue) {
        throw new UnsupportedOperationException();
    }

    static ThreadLocalMap createInheritedMap(ThreadLocalMap parentMap) {
        ThreadLocalMap m = new ThreadLocalMap();
        for (var e : parentMap.map.entrySet()) {
            @SuppressWarnings("unchecked")
            ThreadLocal<Object> k = (ThreadLocal<Object>) e.getKey();
            Object v = e.getValue();
            m.map.put(k, v == NULL ? NULL : k.childValue(v));
        }
        return m;
    }

    static final class ThreadLocalMap {
        final IdentityHashMap<ThreadLocal<?>, Object> map = new IdentityHashMap<>();
    }
}
