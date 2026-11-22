public class CallBench {
    static int fib(int n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }
    interface Shape { int area(int k); }
    static final class Sq implements Shape { public int area(int k) { return k * k; } }
    static final class Rc implements Shape { public int area(int k) { return k * 2; } }
    int f; int getF() { return f; }

    public static void main(String[] args) {
        long t0 = System.nanoTime();
        int r = fib(40);
        long t1 = System.nanoTime();
        Shape[] ss = {new Sq(), new Rc()};
        long sum = 0;
        for (int i = 0; i < 400_000_000; i++) sum += ss[i & 1].area(i);
        long t2 = System.nanoTime();
        CallBench b = new CallBench();
        long acc = 0;
        for (int i = 0; i < 800_000_000; i++) { b.f = i; acc += b.getF(); }
        long t3 = System.nanoTime();
        System.out.println(r + " " + sum + " " + acc);
        System.err.println("fib(40) ms " + (t1 - t0) / 1_000_000 + "  iface 4e8 ms " + (t2 - t1) / 1_000_000 + "  getter 8e8 ms " + (t3 - t2) / 1_000_000);
    }
}
