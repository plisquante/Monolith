package java.lang.ref;

/** monolith overlay. */
public class ReferenceQueue<T> {
    private Reference<? extends T> head;
    private final Object lock = new Object();

    public ReferenceQueue() {}

    boolean enqueue0(Reference<? extends T> r) {
        synchronized (lock) {
            if (r.queue == null) return false;
            r.queue = null;
            r.monoNext = head;
            head = r;
            lock.notifyAll();
            return true;
        }
    }

    @SuppressWarnings("unchecked")
    public Reference<? extends T> poll() {
        synchronized (lock) {
            Reference<? extends T> r = head;
            if (r != null) {
                head = (Reference<? extends T>) r.monoNext;
                r.monoNext = null;
            }
            return r;
        }
    }

    public Reference<? extends T> remove(long timeout) throws InterruptedException {
        if (timeout < 0) throw new IllegalArgumentException("Negative timeout value");
        synchronized (lock) {
            Reference<? extends T> r = poll();
            if (r != null) return r;
            long start = System.nanoTime();
            for (;;) {
                lock.wait(timeout);
                r = poll();
                if (r != null) return r;
                if (timeout != 0 && (System.nanoTime() - start) / 1000000 >= timeout) return null;
            }
        }
    }

    public Reference<? extends T> remove() throws InterruptedException {
        return remove(0);
    }
}
