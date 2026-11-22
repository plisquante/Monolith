package java.lang.ref;

/** j2zig overlay. */
public non-sealed class PhantomReference<T> extends Reference<T> {
    public PhantomReference(T referent, ReferenceQueue<? super T> q) { super(referent, q); }
    public T get() { return null; }
}
