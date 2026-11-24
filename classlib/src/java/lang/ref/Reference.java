package java.lang.ref;

/**
 * monolith overlay of Reference. The referent field is traced weakly by the collector:
 * monolith/rt/gc.rs clears it when the referent is otherwise unreachable (see weak reference
 * processing in gc.rs), then enqueues the reference if it has a queue.
 */
public abstract sealed class Reference<T> permits PhantomReference, SoftReference, WeakReference, FinalReference {
    private T referent;
    volatile ReferenceQueue<? super T> queue;
    Reference<?> monoNext;

    Reference(T referent) {
        this(referent, null);
    }

    Reference(T referent, ReferenceQueue<? super T> queue) {
        this.referent = referent;
        this.queue = queue;
    }

    public T get() {
        return referent;
    }

    public final boolean refersTo(T obj) {
        return referent == obj;
    }

    public void clear() {
        referent = null;
    }

    public boolean isEnqueued() {
        return false;
    }

    public boolean enqueue() {
        referent = null;
        ReferenceQueue<? super T> q = queue;
        return q != null && q.enqueue0(this);
    }

    public static void reachabilityFence(Object ref) {}

    protected Object clone() throws CloneNotSupportedException {
        throw new CloneNotSupportedException();
    }
}
