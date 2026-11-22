import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.*;

/** Needs reflection natives (Class.newField0, Field access): not run by monolith/run_tests.py yet. */
public class Mono7 {
    public static void main(String[] args) throws Exception {
        AtomicInteger ai = new AtomicInteger();
        ExecutorService ex = Executors.newFixedThreadPool(3);
        List<Future<Integer>> fs = new ArrayList<>();
        for (int i = 0; i < 10; i++) { final int n = i; fs.add(ex.submit(() -> { ai.addAndGet(n); return n * n; })); }
        int sum = 0; for (Future<Integer> f : fs) sum += f.get();
        ex.shutdown();
        System.out.println(sum + " " + ai.get() + " " + ex.awaitTermination(5, TimeUnit.SECONDS));
        ConcurrentHashMap<Integer, Integer> chm = new ConcurrentHashMap<>();
        Thread[] ws = new Thread[3];
        for (int i = 0; i < 3; i++) { ws[i] = new Thread(() -> { for (int k = 0; k < 1000; k++) chm.merge(k % 10, 1, Integer::sum); }); ws[i].start(); }
        for (Thread t : ws) t.join();
        System.out.println(chm.get(3) + " " + chm.size());
        System.out.println(CompletableFuture.supplyAsync(() -> 21).thenApply(x -> x * 2).get());
    }
}
