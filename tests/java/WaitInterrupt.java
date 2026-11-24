// Interrupt versus Object.wait, and thread state that must not outlive a thread.
// 1. A thread interrupted right after start() must see InterruptedException from wait(), however early or late the interrupt
//    arrives; an interrupt that slips between the check at the start of wait() and the moment the thread is registered as a
//    waiter must not be lost (the thread would then wait forever).
// 2. A thread that ends interrupted must not leave its interrupted flag to a later thread: the runtime keys per-thread state
//    by object address, and the address of a dead Thread can be reused.
// 3. holdsLock and notify on an object that was never locked: false, and IllegalMonitorStateException.
public class WaitInterrupt {
    static int lostOrHung = 0;

    static void waitAfterInterrupt(int rounds) throws Exception {
        int interrupted = 0;
        for (int i = 0; i < rounds; i++) {
            final Object lock = new Object();
            final boolean[] got = new boolean[1];
            Thread t = new Thread(() -> {
                synchronized (lock) {
                    try {
                        lock.wait();
                    } catch (InterruptedException e) {
                        got[0] = true;
                    }
                }
            });
            t.start();
            t.interrupt();
            t.join(10000);
            if (t.isAlive()) {
                lostOrHung++;
                System.out.println("HANG in round " + i);
                System.exit(1);
            }
            if (got[0]) interrupted++;
        }
        System.out.println("wait interrupted " + interrupted + " of " + rounds);
    }

    static void staleFlag(int rounds) throws Exception {
        int spurious = 0;
        for (int i = 0; i < rounds; i++) {
            Thread dying = new Thread(() -> Thread.currentThread().interrupt());
            dying.start();
            dying.join();
            dying = null;
            if (i % 50 == 0) System.gc();
            final boolean[] bad = new boolean[1];
            Thread fresh = new Thread(() -> {
                try {
                    Thread.sleep(1);
                } catch (InterruptedException e) {
                    bad[0] = true;
                }
            });
            fresh.start();
            fresh.join();
            if (bad[0]) spurious++;
        }
        System.out.println("spurious interrupts " + spurious);
    }

    static void unlockedObject() {
        Object o = new Object();
        System.out.println("holdsLock(never locked) = " + Thread.holdsLock(o));
        try {
            o.notify();
            System.out.println("notify without owning: no exception");
        } catch (IllegalMonitorStateException e) {
            System.out.println("notify without owning: IllegalMonitorStateException");
        }
        synchronized (o) {
            System.out.println("holdsLock(inside synchronized) = " + Thread.holdsLock(o));
        }
        System.out.println("holdsLock(after) = " + Thread.holdsLock(o));
    }

    public static void main(String[] args) throws Exception {
        waitAfterInterrupt(3000);
        staleFlag(400);
        unlockedObject();
    }
}
