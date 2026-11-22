package java.lang;

/** j2zig overlay of InheritableThreadLocal. */
public class InheritableThreadLocal<T> extends ThreadLocal<T> {
    public InheritableThreadLocal() {}

    protected T childValue(T parentValue) {
        return parentValue;
    }

    @Override
    ThreadLocalMap getMap(Thread t) {
        return t.inheritableThreadLocals;
    }

    @Override
    void createMap(Thread t) {
        t.inheritableThreadLocals = new ThreadLocalMap();
    }
}
