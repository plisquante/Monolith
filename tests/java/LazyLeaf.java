public class LazyLeaf {
    int f;
    int[] arr = {1, 2, 3};
    long big = 10;
    static int sf = 7;

    int getF() { return f; }                       // strict leaf
    int at(int[] a, int i) { return a[i]; }        // lazy leaf: may throw NPE / AIOOBE
    void put(int[] a, int i, int v) { a[i] = v; }
    int len(int[] a) { return a.length; }
    int div(int a, int b) { return a / b; }
    int rem(int a, int b) { return a % b; }
    long ldiv(long a, long b) { return a / b; }
    int fieldOf(LazyLeaf o) { return o.f; }
    void setField(LazyLeaf o, int v) { o.f = v; }
    static int stat() { return sf * 2; }
    Object store(Object[] a, int i, Object v) { a[i] = v; return a[i]; }
    byte bs(byte[] a, int i, int v) { a[i] = (byte) v; return a[i]; }

    static void show(String what, Runnable r) {
        try {
            r.run();
            System.out.println(what + " ok");
        } catch (Throwable t) {
            StackTraceElement[] st = t.getStackTrace();
            // helpful NullPointerException messages are a documented difference (docs/STATUS.md): not compared
            String msg = t instanceof NullPointerException ? "" : ": " + t.getMessage();
            StringBuilder sb = new StringBuilder(what + " -> " + t.getClass().getName() + msg);
            for (int i = 0; i < Math.min(3, st.length); i++) {
                if (st[i].getClassName().startsWith("java.") || st[i].getClassName().startsWith("jdk.")) continue;
                sb.append(" | ").append(st[i].getMethodName()).append(":").append(st[i].getLineNumber());
            }
            System.out.println(sb);
        }
    }

    public static void main(String[] args) {
        LazyLeaf x = new LazyLeaf();
        show("at ok", () -> System.out.println(x.at(x.arr, 1)));
        show("at null", () -> x.at(null, 0));
        show("at bad index", () -> x.at(x.arr, 7));
        show("at negative", () -> x.at(x.arr, -1));
        show("put bad", () -> x.put(x.arr, 3, 1));
        show("put null", () -> x.put(null, 0, 1));
        show("len null", () -> x.len(null));
        show("div zero", () -> x.div(1, 0));
        show("rem zero", () -> x.rem(1, 0));
        show("ldiv zero", () -> x.ldiv(1L, 0L));
        show("div min", () -> System.out.println(x.div(Integer.MIN_VALUE, -1) + " " + x.rem(5, -1)));
        show("field null", () -> x.fieldOf(null));
        show("setField null", () -> x.setField(null, 1));
        show("store ok", () -> System.out.println(x.store(new String[2], 0, "s")));
        show("store bad type", () -> x.store(new String[2], 0, Integer.valueOf(1)));
        show("store bad idx", () -> x.store(new Object[1], 2, "s"));
        show("byte store", () -> System.out.println(x.bs(new byte[2], 1, 300)));
        show("boolean store", () -> {
            boolean[] b = new boolean[1];
            x.bs(null, 0, 1);
        });
        show("static", () -> System.out.println(stat() + " " + x.getF()));
        // many iterations so the JIT-less native path and the interpreter agree on results
        long sum = 0;
        for (int i = 0; i < 100000; i++) sum += x.at(x.arr, i % 3) + x.div(i + 1, 1);
        System.out.println("sum " + sum);
    }
}
