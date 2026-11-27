import java.util.concurrent.*;
import java.util.concurrent.atomic.*;

// volatile alan okuma/yazma (getfield/putfield/getstatic/putstatic), CompletableFuture zinciri, ForkJoinPool ve kilitsiz kuyruklar.
// Çıktı belirlenimci: JVM ile birebir karşılaştırılır. arm64'te volatile çitleri olmadan görev kaybı/kilitlenme görülürdü.
public class VolatileQueue {
    static volatile int stage;
    static volatile Object box;
    volatile long counter;
    volatile boolean done;

    public static void main(String[] args) throws Exception {
        // 1) volatile bayrakla yayımlama: üretici veriyi yazar, sonra bayrağı açar; tüketici bayrağı görünce veriyi okur
        final int[] data = new int[1000];
        Thread p = new Thread(() -> {
            for (int i = 0; i < data.length; i++) data[i] = i * 3;
            box = "hazir";
            stage = 1;
        });
        p.start();
        while (stage != 1) Thread.onSpinWait();
        long s = 0;
        for (int v : data) s += v;
        System.out.println("yayim " + box + " " + s);
        p.join();

        // 2) volatile örnek alanı: iki iş parçacığı sırayla artırır (synchronized ile), sonuç bayrağı volatile
        VolatileQueue q = new VolatileQueue();
        Thread[] ts = new Thread[4];
        for (int t = 0; t < ts.length; t++) {
            ts[t] = new Thread(() -> {
                for (int i = 0; i < 10000; i++) synchronized (q) { q.counter++; }
            });
            ts[t].start();
        }
        for (Thread t : ts) t.join();
        q.done = true;
        System.out.println("sayac " + q.counter + " " + q.done);

        // 3) CompletableFuture zinciri ForkJoinPool üzerinde
        CompletableFuture<Integer> f = CompletableFuture.supplyAsync(() -> 21)
            .thenApplyAsync(x -> x * 2)
            .thenCombine(CompletableFuture.supplyAsync(() -> 100), Integer::sum);
        System.out.println("future " + f.join());

        // 4) çok sayıda iç içe join (IOWorker kalıbı: görev içinden başka görevlerin sonucunu bekleme)
        ForkJoinPool pool = new ForkJoinPool(4);
        long total = pool.submit(() -> {
            CompletableFuture<?>[] fs = new CompletableFuture<?>[64];
            AtomicLong acc = new AtomicLong();
            for (int i = 0; i < fs.length; i++) {
                final int k = i;
                fs[i] = CompletableFuture.runAsync(() -> acc.addAndGet(k), pool);
            }
            CompletableFuture.allOf(fs).join();
            return acc.get();
        }).get();
        pool.shutdown();
        System.out.println("icice " + total);

        // 5) kilitsiz kuyruk: üretici/tüketici
        ConcurrentLinkedQueue<Integer> cq = new ConcurrentLinkedQueue<>();
        AtomicInteger got = new AtomicInteger();
        Thread cons = new Thread(() -> {
            int n = 0;
            while (n < 50000) { Integer v = cq.poll(); if (v != null) { got.addAndGet(v & 1); n++; } }
        });
        cons.start();
        for (int i = 0; i < 50000; i++) cq.add(i);
        cons.join();
        System.out.println("kuyruk " + got.get());
    }
}
