package java.lang.ref;

/** j2zig overlay: finalization is not supported (Object.finalize is never invoked). */
non-sealed class FinalReference<T> extends Reference<T> {
    FinalReference(T referent, ReferenceQueue<? super T> q) { super(referent, q); }
}
