import java.util.*;

public class GcStress {
    static final class Node { final int v; Node next; byte[] payload; Node(int v, Node n) { this.v = v; next = n; payload = new byte[v % 64]; } }

    static long churn(int rounds, int seed) {
        long check = 0;
        Node keep = null;
        for (int r = 0; r < rounds; r++) {
            Node head = null;
            for (int i = 0; i < 1000; i++) head = new Node(i ^ seed, head);
            int len = 0; for (Node n = head; n != null; n = n.next) { len++; check += n.v + n.payload.length; }
            if (r % 100 == 0) { head.next.next = keep; keep = head; }
            check += len;
            String s = "round" + r + ":" + seed;
            check += s.hashCode();
        }
        int kept = 0; for (Node n = keep; n != null; n = n.next) kept++;
        return check * 31 + kept;
    }

    public static void main(String[] args) throws Exception {
        long t = churn(20000, 1);
        System.out.println("single " + t);
        List<int[]> big = new ArrayList<>();
        for (int i = 0; i < 400; i++) { big.add(new int[100000]); if (big.size() > 8) big.remove(0); big.get(big.size() - 1)[i % 100000] = i; }
        long s = 0; for (int[] a : big) for (int x : a) s += x;
        System.out.println("big arrays " + s);
        long[] results = new long[4];
        Thread[] ts = new Thread[4];
        for (int i = 0; i < 4; i++) { final int id = i; ts[i] = new Thread(() -> results[id] = churn(6000, id + 10)); ts[i].start(); }
        for (Thread th : ts) th.join();
        System.out.println("threads " + Arrays.toString(results));
        Map<Integer, String> m = new HashMap<>();
        for (int i = 0; i < 300000; i++) { m.put(i, "v" + i); if (i % 3 == 0) m.remove(i / 2); }
        long hs = 0; for (Map.Entry<Integer, String> e : m.entrySet()) hs += e.getKey() * 7L + e.getValue().length();
        System.out.println("map " + m.size() + " " + hs);
        java.lang.ref.WeakReference<Object> weak = new java.lang.ref.WeakReference<>(new Object());
        Object strong = new Object();
        java.lang.ref.WeakReference<Object> weak2 = new java.lang.ref.WeakReference<>(strong);
        for (int i = 0; i < 20; i++) { byte[] junk = new byte[10_000_000]; junk[i] = 1; }
        System.gc();
        System.out.println("weak cleared=" + (weak.get() == null) + " strong kept=" + (weak2.get() == strong));
    }
}
