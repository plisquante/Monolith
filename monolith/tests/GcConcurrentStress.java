// Stress for the concurrent collector (MONO_GC_CONCURRENT=1): lists are reversed in place (every pass overwrites every
// reference) while garbage is allocated, on several threads. A node that the collector wrongly frees would break the
// count or the sum (or the heap). HotSpot prints the same total.
public class GcConcurrentStress {
    static final class N {
        N next;
        final int v;
        final byte[] pad;
        N(int v) { this.v = v; this.pad = new byte[48]; }
    }

    static long work(int seed) {
        long total = 0;
        for (int round = 0; round < 2500; round++) {
            N head = null;
            for (int i = 0; i < 1500; i++) {
                N n = new N(i + seed);
                n.next = head;
                head = n;
            }
            for (int pass = 0; pass < 6; pass++) {
                N prev = null;
                N cur = head;
                while (cur != null) {
                    N nx = cur.next;
                    cur.next = prev;
                    prev = cur;
                    cur = nx;
                }
                head = prev;
                for (int g = 0; g < 400; g++) {
                    new N(-g);
                }
            }
            // the same nodes in an array that is shifted with System.arraycopy (reference copies, no barrier in the JVM
            // loop; the runtime must log the overwritten elements), then checked
            N[] arr = new N[1500];
            int n = 0;
            for (N x = head; x != null; x = x.next) {
                arr[n++] = x;
            }
            for (int pass = 0; pass < 6; pass++) {
                System.arraycopy(arr, 0, arr, 1, 1499);
                arr[0] = new N(pass);
                for (int g = 0; g < 200; g++) {
                    new N(-g);
                }
            }
            long sum = 0;
            int count = 0;
            for (int i = 0; i < arr.length; i++) {
                sum += arr[i].v;
                count++;
            }
            long sum2 = 0;
            int count2 = 0;
            for (N x = head; x != null; x = x.next) {
                sum2 += x.v;
                count2++;
            }
            total += sum + count + sum2 + count2;
        }
        return total;
    }

    public static void main(String[] args) throws Exception {
        long[] res = new long[4];
        Thread[] ts = new Thread[4];
        for (int t = 0; t < 4; t++) {
            final int id = t;
            ts[t] = new Thread(() -> res[id] = work(id * 100000));
            ts[t].start();
        }
        long total = 0;
        for (int t = 0; t < 4; t++) {
            ts[t].join();
            total += res[t];
        }
        System.out.println("total " + total);
    }
}
