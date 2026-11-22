import java.lang.reflect.InvocationHandler;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.util.concurrent.locks.LockSupport;

public class OptSuite {
    interface Greeter {
        String greet(String who);
        default String polite(String who) {
            return "Hello, " + who + "!";
        }
    }

    interface Calc {
        int eval(int a, int b);
    }

    static final class AddCalc implements Calc {
        final int bias;
        AddCalc(int bias) { this.bias = bias; }
        public int eval(int a, int b) { return a + b + bias; }
    }

    static final class MulCalc implements Calc {
        final int scale;
        MulCalc(int scale) { this.scale = scale; }
        public int eval(int a, int b) { return (a - b) * scale; }
    }

    static int staticCombine(int x, int y) {
        int t = x * 3 + y;
        return t ^ (t >>> 2);
    }

    static int leafSwitch(int k) {
        switch (k) {
            case 0: return 11;
            case 1: return 22;
            case 2: return 33;
            case 3: return 44;
            default: return k + 100;
        }
    }

    static int lazyLeafLookup(int[] arr, int idx) {
        switch (idx) {
            case 0: return arr[0] + 1;
            case 1: return arr[1] + 2;
            default: return arr[idx] + Math.abs(idx);
        }
    }

    public static void main(String[] args) throws Exception {
        testIntrinsics();
        testInliningAndSwitches();
        testArraysAndMonitor();
        testParkUnpark();
        testProxies();
        System.out.println("OptSuite OK");
    }

    static void testIntrinsics() {
        double[] ds = { 0.0, -0.0, 1.5, -2.25, Double.POSITIVE_INFINITY, Double.NEGATIVE_INFINITY, Double.NaN };
        for (double d : ds) {
            System.out.println(Double.doubleToLongBits(Math.copySign(d, -1.0)) + " "
                    + Double.doubleToLongBits(Math.copySign(d, 1.0)) + " "
                    + Double.doubleToLongBits(Math.signum(d)) + " "
                    + Double.isNaN(d) + " " + Double.isInfinite(d) + " " + Double.isFinite(d) + " "
                    + Double.doubleToLongBits(d));
        }
        System.out.println(Double.doubleToLongBits(Math.fma(1.5, 2.0, -0.5)) + " "
                + Float.floatToIntBits(Math.fma(1.5f, -2.0f, 0.25f)) + " "
                + Float.floatToIntBits(Math.copySign(-0.0f, 3.0f)) + " "
                + Float.floatToIntBits(Math.signum(-4.5f)) + " "
                + Float.isNaN(Float.NaN) + " " + Float.isInfinite(Float.NEGATIVE_INFINITY) + " "
                + Float.isFinite(1.0f));

        int[] ints = { 0, 1, -1, Integer.MIN_VALUE, Integer.MAX_VALUE, 0x12345678 };
        for (int x : ints) {
            System.out.println(Integer.reverse(x) + " " + Integer.reverseBytes(x) + " "
                    + Integer.compareUnsigned(x, -10) + " "
                    + Integer.divideUnsigned(x, 7) + " " + Integer.remainderUnsigned(x, 7));
        }
        try {
            System.out.println(Integer.divideUnsigned(10, 0));
        } catch (ArithmeticException e) {
            System.out.println("divu0:" + (e.getStackTrace()[0].getLineNumber() > 0));
        }
        long[] longs = { 0L, 1L, -1L, Long.MIN_VALUE, 0x0123456789ABCDEFL };
        for (long l : longs) {
            System.out.println(Long.reverse(l) + " " + Long.reverseBytes(l) + " "
                    + Long.compareUnsigned(l, -10L) + " "
                    + Long.divideUnsigned(l, 13L) + " " + Long.remainderUnsigned(l, 13L));
        }
        System.out.println((int) Short.reverseBytes((short) 0x1234) + " " + (int) Character.reverseBytes('\u1234'));
    }

    static void testInliningAndSwitches() {
        Calc[] cs = { new AddCalc(5), new MulCalc(3) };
        int sum = 0;
        for (int i = 0; i < 10; i++) {
            sum += cs[i & 1].eval(i, 2) + leafSwitch(i & 3) + staticCombine(i, 7);
        }
        int[] arr = { 10, 20, 30 };
        int lazyOk = lazyLeafLookup(arr, 1) + lazyLeafLookup(arr, 2);
        int lazyErrLine = -1;
        try {
            lazyLeafLookup(arr, 9);
        } catch (ArrayIndexOutOfBoundsException e) {
            lazyErrLine = e.getStackTrace()[0].getLineNumber();
        }
        System.out.println(sum + " " + lazyOk + " " + lazyErrLine);
    }

    static void testArraysAndMonitor() throws Exception {
        byte[] bs = new byte[4];
        boolean[] bools = new boolean[4];
        Object[] objs = new String[2];
        for (int i = 0; i < 4; i++) {
            bs[i] = (byte) (i * 10 - 5);
            bools[i] = (i & 1) != 0;
        }
        objs[0] = "a";
        objs[1] = null;
        boolean ase = false;
        try {
            objs[0] = Integer.valueOf(1);
        } catch (ArrayStoreException e) {
            ase = true;
        }
        System.out.println(bs[3] + " " + bools[1] + " " + bools[2] + " " + objs[0] + " " + objs[1] + " " + ase);

        final Object lock = new Object();
        final boolean[] ready = { false };
        synchronized (lock) {
            lock.notify();
            lock.notifyAll();
        }
        Thread t = new Thread(() -> {
            synchronized (lock) {
                while (!ready[0]) {
                    try { lock.wait(); } catch (InterruptedException ignored) {}
                }
                ready[0] = false;
                lock.notifyAll();
            }
        });
        t.start();
        Thread.sleep(20);
        synchronized (lock) {
            ready[0] = true;
            lock.notify();
            while (ready[0]) {
                lock.wait();
            }
        }
        t.join();
        System.out.println("monitor OK");
    }

    static void testParkUnpark() throws Exception {
        LockSupport.unpark(Thread.currentThread());
        LockSupport.park();
        LockSupport.parkNanos(1_000_000L);
        final boolean[] woke = { false };
        final Thread[] target = new Thread[1];
        Thread waiter = new Thread(() -> {
            target[0] = Thread.currentThread();
            LockSupport.park();
            woke[0] = true;
        });
        waiter.start();
        while (target[0] == null) Thread.yield();
        Thread.sleep(20);
        LockSupport.unpark(target[0]);
        waiter.join();
        System.out.println("park OK " + woke[0]);
    }

    static void testProxies() throws Exception {
        Class<?> pc = Proxy.getProxyClass(OptSuite.class.getClassLoader(), Greeter.class);
        InvocationHandler h = (proxy, method, args) -> {
            if (method.isDefault()) {
                return InvocationHandler.invokeDefault(proxy, method, args);
            }
            return "HI:" + args[0];
        };
        Greeter g = (Greeter) Proxy.newProxyInstance(OptSuite.class.getClassLoader(), new Class<?>[]{ Greeter.class }, h);
        System.out.println(Proxy.isProxyClass(pc) + " " + (Proxy.getInvocationHandler(g) == h) + " "
                + g.greet("zig") + " " + g.polite("jvm"));
    }
}
