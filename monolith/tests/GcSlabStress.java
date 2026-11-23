// Stress for the slab allocator: many short-lived threads each leave a partly filled slab behind when they end,
// while collections run often (a small threshold). Regression for "fresh slab has slot" (a full slab was handed out
// as if it had room). The output is a checksum that HotSpot computes the same way.
import java.util.ArrayList;
import java.util.List;

public class GcSlabStress {
    static final class Box {
        final Object a;
        final long b;
        Box(Object a, long b) { this.a = a; this.b = b; }
    }

    public static void main(String[] args) throws Exception {
        long sum = 0;
        for (int round = 0; round < 60; round++) {
            List<Thread> ts = new ArrayList<>();
            final long[] res = new long[64];
            for (int t = 0; t < 64; t++) {
                final int id = t, r = round;
                Thread th = new Thread(() -> {
                    long s = 0;
                    Object keep = null;
                    // a few hundred small objects of one size class: the thread's active slab stays partly full
                    for (int i = 0; i < 300 + (id * 7 + r) % 97; i++) {
                        Box b = new Box(keep, i + id);
                        if (i % 5 == 0) keep = b;
                        s += b.b;
                    }
                    res[id] = s;
                });
                ts.add(th);
                th.start();
                if (t % 8 == 7) {
                    for (Thread x : ts) x.join();
                    ts.clear();
                }
            }
            for (Thread x : ts) x.join();
            for (long v : res) sum += v;
        }
        System.out.println("checksum " + sum);
    }
}
