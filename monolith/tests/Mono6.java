import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.*;

public class Mono6 {
    static int shared, shared2;
    static final Object LOCK = new Object();
    static synchronized void inc() { shared++; }
    public static void main(String[] args) throws Exception {
        Thread[] ts = new Thread[4];
        for (int i = 0; i < ts.length; i++) {
            ts[i] = new Thread(() -> { for (int k = 0; k < 20000; k++) { inc(); synchronized (LOCK) { shared2++; } } });
            ts[i].start();
        }
        for (Thread t : ts) t.join();
        System.out.println(shared + " " + shared2);
        final Object mon = new Object();
        final boolean[] ready = {false};
        Thread waiter = new Thread(() -> { synchronized (mon) { while (!ready[0]) { try { mon.wait(); } catch (InterruptedException e) {} } System.out.println("woke"); } });
        waiter.start();
        Thread.sleep(50);
        synchronized (mon) { ready[0] = true; mon.notifyAll(); }
        waiter.join();
        Thread sl = new Thread(() -> { try { Thread.sleep(10000); } catch (InterruptedException e) { System.out.println("interrupted"); } });
        sl.start(); Thread.sleep(30); sl.interrupt(); sl.join();
        CountDownLatch cl = new CountDownLatch(2);
        new Thread(cl::countDown).start(); new Thread(cl::countDown).start();
        cl.await();
        System.out.println("latch " + cl.getCount());
    }
}
