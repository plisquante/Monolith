import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.*;
import java.util.concurrent.locks.*;

public class Threads {
    static int counter = 0;
    static final Object lock = new Object();
    static volatile boolean stop = false;

    static synchronized void inc() { counter++; }

    public static void main(String[] args) throws Exception {
        Thread[] ts = new Thread[8];
        for (int i = 0; i < ts.length; i++) { ts[i] = new Thread(() -> { for (int k = 0; k < 20000; k++) inc(); }); ts[i].start(); }
        for (Thread t : ts) t.join();
        System.out.println("synchronized counter " + counter);
        AtomicInteger ai = new AtomicInteger(); AtomicLong al = new AtomicLong();
        Thread[] ts2 = new Thread[6];
        for (int i = 0; i < ts2.length; i++) { ts2[i] = new Thread(() -> { for (int k = 0; k < 20000; k++) { ai.incrementAndGet(); al.addAndGet(2); } }); ts2[i].start(); }
        for (Thread t : ts2) t.join();
        System.out.println("atomics " + ai.get() + " " + al.get());
        // producer / consumer with wait/notify
        LinkedList<Integer> q = new LinkedList<>();
        long[] consumed = new long[1];
        Thread prod = new Thread(() -> {
            for (int i = 1; i <= 5000; i++) synchronized (lock) { while (q.size() >= 10) { try { lock.wait(); } catch (InterruptedException e) { return; } } q.add(i); lock.notifyAll(); }
        });
        Thread cons = new Thread(() -> {
            for (int i = 1; i <= 5000; i++) synchronized (lock) { while (q.isEmpty()) { try { lock.wait(); } catch (InterruptedException e) { return; } } consumed[0] += q.removeFirst(); lock.notifyAll(); }
        });
        prod.start(); cons.start(); prod.join(); cons.join();
        System.out.println("consumed " + consumed[0]);
        ReentrantLock rl = new ReentrantLock();
        Condition cond = rl.newCondition();
        int[] shared = {0};
        Thread[] ts3 = new Thread[4];
        for (int i = 0; i < ts3.length; i++) { ts3[i] = new Thread(() -> { for (int k = 0; k < 10000; k++) { rl.lock(); try { shared[0]++; cond.signalAll(); } finally { rl.unlock(); } } }); ts3[i].start(); }
        for (Thread t : ts3) t.join();
        System.out.println("reentrant lock " + shared[0]);
        ConcurrentHashMap<Integer, Integer> chm = new ConcurrentHashMap<>();
        Thread[] ts4 = new Thread[4];
        for (int i = 0; i < ts4.length; i++) { final int id = i; ts4[i] = new Thread(() -> { for (int k = 0; k < 5000; k++) chm.merge(k % 100, 1, Integer::sum); }); ts4[i].start(); }
        for (Thread t : ts4) t.join();
        int total = 0; for (int v : chm.values()) total += v;
        System.out.println("chm " + chm.size() + " " + total);
        ExecutorService pool = Executors.newFixedThreadPool(4);
        List<Future<Long>> fs = new ArrayList<>();
        for (int i = 0; i < 16; i++) { final long n = i; fs.add(pool.submit(() -> { long s = 0; for (long k = 0; k <= n * 1000; k++) s += k; return s; })); }
        long tot = 0; for (Future<Long> f : fs) tot += f.get();
        pool.shutdown();
        System.out.println("executor " + tot + " " + pool.awaitTermination(10, TimeUnit.SECONDS));
        CountDownLatch latch = new CountDownLatch(3);
        for (int i = 0; i < 3; i++) new Thread(latch::countDown).start();
        latch.await();
        System.out.println("latch done");
        CompletableFuture<Integer> cf = CompletableFuture.supplyAsync(() -> 21).thenApply(x -> x * 2);
        System.out.println("completable " + cf.get());
        Thread sleeper = new Thread(() -> { try { Thread.sleep(100000); System.out.println("not interrupted?"); } catch (InterruptedException e) { System.out.println("interrupted ok"); } });
        sleeper.start(); Thread.sleep(50); sleeper.interrupt(); sleeper.join();
        Thread spinner = new Thread(() -> { long n = 0; while (!stop) n++; System.out.println("spinner stopped"); });
        spinner.start(); Thread.sleep(20); stop = true; spinner.join();
        ThreadLocal<Integer> tl = ThreadLocal.withInitial(() -> 5);
        tl.set(9);
        int[] seen = new int[1];
        Thread tlt = new Thread(() -> seen[0] = tl.get()); tlt.start(); tlt.join();
        System.out.println("threadlocal " + tl.get() + " " + seen[0] + " " + Thread.currentThread().getName());
        Thread daemon = new Thread(() -> { while (true) { try { Thread.sleep(1000); } catch (InterruptedException e) { } } });
        daemon.setDaemon(true); daemon.start();
        Thread late = new Thread(() -> { try { Thread.sleep(100); } catch (InterruptedException e) { } System.out.println("non-daemon finished after main"); });
        late.start();
        System.out.println("main done");
    }
}
