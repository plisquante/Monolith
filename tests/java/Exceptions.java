import java.util.*;

public class Exceptions {
    static class MyEx extends Exception { final int code; MyEx(String m, int code) { super(m); this.code = code; } }
    static class Res implements AutoCloseable { final String n; Res(String n) { this.n = n; System.out.println("open " + n); } public void close() throws Exception { System.out.println("close " + n); if (n.equals("bad")) throw new IllegalStateException("close failed " + n); } }
    static class Boom { static int v; static { if (true) throw new RuntimeException("static boom"); } }

    static int depth(int n) throws MyEx { if (n == 0) throw new MyEx("bottom", 42); return depth(n - 1) + 1; }
    @SuppressWarnings("finally")
    static int finallyOverride() { try { return 1; } finally { return 2; } }
    static String order() {
        StringBuilder sb = new StringBuilder();
        try { try { sb.append('a'); throw new IllegalArgumentException("x"); } finally { sb.append('b'); } }
        catch (IllegalArgumentException e) { sb.append('c').append(e.getMessage()); }
        finally { sb.append('d'); }
        return sb.toString();
    }
    static int recurse(int n) { return n == 0 ? 0 : 1 + recurse(n - 1); }
    static int infinite(int n) { return infinite(n + 1) + 1; }

    public static void main(String[] args) throws Exception {
        try { depth(50); } catch (MyEx e) { System.out.println(e.getMessage() + " " + e.code + " " + (e instanceof Exception)); }
        System.out.println(finallyOverride() + " " + order());
        int[] arr = new int[3];
        Object o = "str";
        String nul = null;
        List<Runnable> bad = List.of(
            () -> System.out.println(arr[3]),
            () -> System.out.println(arr[-1]),
            () -> System.out.println(((Integer) o)),
            () -> System.out.println(nul.length()),
            () -> System.out.println(new int[-5].length),
            () -> { Object[] x = new String[1]; x[0] = 1; },
            () -> System.out.println(10 / (arr[0])),
            () -> { throw new UnsupportedOperationException("nope"); },
            () -> List.of(1).add(2),
            () -> new ArrayList<Integer>().get(0),
            () -> new ArrayList<>(List.of(1, 2)).iterator().remove(),
            () -> Objects.requireNonNull(null, "must not be null"),
            () -> "abc".substring(5),
            () -> { throw new Error("custom error"); },
            () -> new HashMap<String, Integer>().keySet().iterator().next()
        );
        for (Runnable r : bad) {
            try { r.run(); System.out.println("no exception?"); }
            catch (ClassCastException e) { System.out.println("CCE " + (e.getMessage() != null)); }
            catch (NullPointerException e) { System.out.println("NPE"); }
            catch (RuntimeException e) { System.out.println(e.getClass().getName() + ": " + e.getMessage()); }
            catch (Error e) { System.out.println("Error " + e.getMessage()); }
        }
        try (Res r1 = new Res("one"); Res r2 = new Res("bad")) { System.out.println("body"); throw new RuntimeException("in body"); }
        catch (RuntimeException e) { System.out.println("caught " + e.getMessage() + " suppressed=" + Arrays.toString(e.getSuppressed())); }
        for (int i = 0; i < 2; i++) {
            try { System.out.println(Boom.v); } catch (Throwable t) { System.out.println(t.getClass().getName() + " cause=" + (t.getCause() == null ? null : t.getCause().getMessage())); }
        }
        try { throw new MyEx("multi", 1); } catch (IllegalStateException | MyEx e) { System.out.println("multi " + e.getMessage()); }
        Exception chained = new RuntimeException("outer", new IllegalArgumentException("inner"));
        System.out.println(chained + " / " + chained.getCause() + " / " + new Exception(new Error("wrapped")).getMessage());
        System.out.println(recurse(20000)); // deep, but far from HotSpot's -Xss8m limit (100000 overflowed it intermittently)
        try { infinite(0); } catch (StackOverflowError e) { System.out.println("SOE caught"); }
        int count = 0;
        for (int i = 0; i < 100000; i++) { try { if (i % 3 == 0) throw new IllegalStateException(); count++; } catch (IllegalStateException e) { count += 2; } finally { count++; } }
        System.out.println(count);
        StackTraceElement[] st = new Throwable().getStackTrace();
        System.out.println(st[0].getClassName() + "." + st[0].getMethodName());
        Thread t = new Thread(() -> { throw new IllegalStateException("from thread"); });
        t.setUncaughtExceptionHandler((th, ex) -> System.out.println("handler got " + ex.getMessage() + " in " + th.getName()));
        t.setName("worker"); t.start(); t.join();
        throw new IllegalStateException("uncaught at end");
    }
}
