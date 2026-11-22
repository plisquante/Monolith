package java.lang.ref;

/** monolith overlay (cleared like a weak reference, only under the same conditions). */
public non-sealed class SoftReference<T> extends Reference<T> {
    public SoftReference(T referent) { super(referent); }
    public SoftReference(T referent, ReferenceQueue<? super T> q) { super(referent, q); }
}
