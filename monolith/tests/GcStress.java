import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicLong;

public class GcStress {
    static final class Node {
        Node next;
        byte[] pad;
        int id;
        Node(int id, Node next, int bytes) {
            this.id = id;
            this.next = next;
            this.pad = new byte[bytes];
        }
    }

    static long sumChain(int n) {
        Node head = null;
        for (int i = 0; i < n; i++) {
            head = new Node(i, head, 1024);
        }
        long s = 0;
        for (Node cur = head; cur != null; cur = cur.next) {
            s += cur.id;
        }
        return s;
    }

    static int nonEscapingTemp(int x) {
        int[] a = new int[4];
        a[0] = x;
        a[1] = x + 1;
        a[2] = a[0] + a[1];
        a[3] = a[2] * 2;
        return a[3];
    }

    public static void main(String[] args) throws Exception {
        long arenaSum = 0;
        for (int i = 0; i < 200000; i++) {
            arenaSum += nonEscapingTemp(i & 255);
        }
        long total = 0;
        // Allocate ~150 MiB of short-lived heap chains across multiple rounds
        for (int round = 0; round < 30; round++) {
            total += sumChain(4500);
        }
        // Multithreaded GC stress
        ExecutorService pool = Executors.newFixedThreadPool(4);
        AtomicLong mtSum = new AtomicLong();
        Future<?>[] futs = new Future<?>[4];
        for (int t = 0; t < 4; t++) {
            final int tid = t;
            futs[t] = pool.submit(() -> {
                long acc = 0;
                for (int r = 0; r < 10; r++) {
                    acc += sumChain(1200 + tid);
                    if (r % 3 == 0) {
                        Object[] tmp = new String[200];
                        for (int k = 0; k < tmp.length; k++) tmp[k] = "s" + k;
                        acc += tmp.length;
                    }
                }
                mtSum.addAndGet(acc);
            });
        }
        for (Future<?> f : futs) f.get();
        pool.shutdown();
        System.gc();
        System.out.println(arenaSum + " " + total + " " + mtSum.get());
    }
}
