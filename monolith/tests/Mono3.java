public class Mono3 {
    static class AppException extends RuntimeException {
        int code;
        AppException(String m, int code) { super(m); this.code = code; }
    }
    static class Checked extends Exception {
        Checked(String m, Throwable cause) { super(m, cause); }
    }
    static int depth;
    static int thrower(int n) {
        if (n == 0) throw new AppException("bottom", 42);
        return thrower(n - 1) + 1;
    }
    static int fin(int x) {
        try {
            if (x > 0) throw new IllegalStateException("x>0");
            return 1;
        } catch (IllegalStateException e) {
            return 2;
        } finally {
            depth++;
        }
    }
    static void chain() throws Checked {
        try {
            int[] a = new int[1];
            a[3] = 1;
        } catch (ArrayIndexOutOfBoundsException e) {
            throw new Checked("wrapped", e);
        }
    }
    public static void main(String[] args) {
        try { thrower(5); } catch (AppException e) { System.out.println(e.getMessage()); System.out.println(e.code); }
        try { System.out.println(10 / (args.length)); } catch (ArithmeticException e) { System.out.println(e.getMessage()); }
        try { AppException o = null; o.code++; } catch (NullPointerException e) { System.out.println("npe"); }
        try { int[] a = new int[2]; a[2] = 1; } catch (RuntimeException e) { System.out.println(e.getMessage()); }
        try { Object o = new int[1]; Mono3 s = (Mono3) o; } catch (ClassCastException e) { System.out.println("cce"); }
        try { int[] a = new int[-1]; } catch (NegativeArraySizeException e) { System.out.println(e.getMessage()); }
        System.out.println(fin(0)); System.out.println(fin(1)); System.out.println(depth);
        try { chain(); } catch (Checked e) { System.out.println(e.getMessage()); System.out.println(e.getCause().getMessage()); }
        int s = 0;
        for (int i = 0; i < 5; i++) {
            try { if (i % 2 == 0) throw new Error("e0"); s += i; } catch (Error e) { s += 100; }
        }
        System.out.println(s);
        try { try { throw new AppException("inner", 1); } finally { System.out.println("fin1"); } }
        catch (Exception e) { System.out.println("outer"); }
        throw new AppException("fatal", 7);
    }
}
