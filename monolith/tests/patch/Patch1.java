// Differential test of the Rust patch system: every patched method must behave exactly like the Java original.
public class Patch1 {
    static final class PatchTarget {
        static int mix(int a, int b) { return a * 31 ^ (b >>> 3); }
        static double scale(double d, float f) { return d * f + 0.5; }
        static long sum(long n) { long s = 0; for (long i = 0; i < n; i++) s += i * i; return s; }
        static double nan() { return 0.0 / 0.0; }
    }

    /** One bit per index; out-of-range access throws (the patch hands those cases to the original). */
    static final class Bits {
        final int size; final long[] data;
        Bits(int size) { this.size = size; this.data = new long[(size + 63) >> 6]; }
        int get(int i) {
            if (i < 0 || i >= size) throw new IndexOutOfBoundsException("index " + i + " of " + size);
            return (int) ((data[i >> 6] >>> (i & 63)) & 1);
        }
        void set(int i, int v) {
            if (i < 0 || i >= size) throw new IndexOutOfBoundsException("index " + i + " of " + size);
            if ((v & ~1) != 0) throw new IllegalArgumentException("value " + v);
            long m = 1L << (i & 63);
            if (v == 0) data[i >> 6] &= ~m; else data[i >> 6] |= m;
        }
    }

    static class Base { int a = 3; }
    static final class Derived extends Base {
        int b = 4;
        int total() { return a + b; }
    }

    static final class Reg {
        static int count;
        static int helper(int x) { return x * 7; }     // reached only through the patch bindings
        static int bump(int x) { count++; return x; }
    }

    static final class Acc {
        int total;
        synchronized int add(int a) { total += a; return total; }
    }

    interface Shape { int area(); }
    static final class Sq implements Shape { final int s; Sq(int s) { this.s = s; } public int area() { return s * s; } }
    static final class Rect implements Shape { final int w, h; Rect(int w, int h) { this.w = w; this.h = h; } public int area() { return w * h; } }
    static final class Shapes {
        /** The patch handles exactly Sq and Rect, anything else goes to the original. */
        static int areaOf(Shape s) { return s.area(); }
    }

    public static void main(String[] a) {
        long h = 0;
        for (int i = 0; i < 2000; i++) h = h * 1000003 + PatchTarget.mix(i, i * 7 - 1000);
        System.out.println("mix " + h);
        System.out.println("scale " + PatchTarget.scale(1.5, 2.25f) + " " + PatchTarget.scale(-0.0, 1f) + " " + PatchTarget.scale(Double.NaN, 1f));
        System.out.println("sum " + PatchTarget.sum(100000));
        System.out.println("nan " + Double.doubleToRawLongBits(PatchTarget.nan()));

        Bits b = new Bits(200);
        for (int i = 0; i < 200; i += 3) b.set(i, 1);
        b.set(3, 0);
        int ones = 0;
        for (int i = 0; i < 200; i++) ones += b.get(i);
        System.out.println("bits " + ones + " " + b.get(0) + " " + b.get(3) + " " + b.get(198));
        try { b.get(200); } catch (IndexOutOfBoundsException e) { System.out.println("caught " + e.getMessage()); }
        try { b.get(-1); } catch (IndexOutOfBoundsException e) { System.out.println("caught " + e.getMessage()); }
        try { b.set(5, 2); } catch (IllegalArgumentException e) { System.out.println("caught " + e.getMessage()); }
        try { ((Bits) null).get(0); } catch (NullPointerException e) { System.out.println("npe"); }

        System.out.println("derived " + new Derived().total());
        int r = 0;
        for (int i = 0; i < 10; i++) r += Reg.bump(i);
        System.out.println("reg " + r + " " + Reg.count);
        Acc acc = new Acc();
        for (int i = 1; i <= 10; i++) acc.add(i);
        System.out.println("acc " + acc.add(0));
        System.out.println("shapes " + Shapes.areaOf(new Sq(5)) + " " + Shapes.areaOf(new Rect(3, 4)) + " " + Shapes.areaOf(() -> 42));
    }
}
