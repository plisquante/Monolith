// Monitors of objects that die: the runtime keeps one monitor per locked object and drops it when the collector finds the
// object dead. Locking many short-lived objects with collections in between must keep every lock correct (mutual exclusion,
// wait/notify, reentrancy), including an object that reuses the address of a dead one.
import java.util.ArrayList;
import java.util.List;

public class MonitorChurn {
    static int shared;
    static final Object HOT = new Object();

    public static void main(String[] args) throws Exception {
        long sum = 0;
        for (int round = 0; round < 6; round++) {
            for (int i = 0; i < 40000; i++) {
                Object o = new Object();
                synchronized (o) {
                    synchronized (o) {
                        sum += i & 7; // reentrant
                    }
                }
            }
            System.gc();
        }
        System.out.println("locked many dead objects: " + sum);

        // a hot lock under contention while the collector runs: no lost update
        final int threads = 4, per = 20000;
        List<Thread> ts = new ArrayList<>();
        for (int t = 0; t < threads; t++) {
            Thread th = new Thread(() -> {
                int mine = 0; // touched under a different, short-lived lock: must not interfere with HOT
                for (int i = 0; i < per; i++) {
                    synchronized (HOT) {
                        shared++;
                    }
                    if ((i & 4095) == 0) {
                        Object junk = new Object();
                        synchronized (junk) { mine++; }
                    }
                }
            });
            ts.add(th);
            th.start();
        }
        for (int i = 0; i < 5; i++) {
            System.gc();
            Thread.sleep(2);
        }
        for (Thread th : ts) th.join();
        System.out.println("shared counter: " + shared + " (expected " + threads * per + ")");

        // wait/notify across collections
        final Object box = new Object();
        final int[] state = new int[1];
        Thread waiter = new Thread(() -> {
            synchronized (box) {
                while (state[0] == 0) {
                    try {
                        box.wait();
                    } catch (InterruptedException e) {
                        return;
                    }
                }
                state[0] = 2;
            }
        });
        waiter.start();
        Thread.sleep(20);
        System.gc();
        synchronized (box) {
            state[0] = 1;
            box.notifyAll();
        }
        waiter.join(10000);
        System.out.println("waiter finished: " + !waiter.isAlive() + ", state " + state[0]);
    }
}
