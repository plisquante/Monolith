package java.lang.ref;

/** monolith overlay. */
public non-sealed class WeakReference<T> extends Reference<T> {
    public WeakReference(T referent) { super(referent); }
    public WeakReference(T referent, ReferenceQueue<? super T> q) { super(referent, q); }
}
