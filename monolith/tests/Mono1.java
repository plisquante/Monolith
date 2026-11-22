public class Mono1 {
    static int fib(int n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }
    static long mix(long x) {
        long h = 1469598103934665603L;
        for (int i = 0; i < 100; i++) { h ^= (x + i); h *= 1099511628211L; h = (h << 7) | (h >>> 57); }
        return h;
    }
    static int sw(int v) {
        switch (v) { case 1: return 10; case 2: return 20; case 5: return 50; default: return -1; }
    }
    static int conv(double d, float f, long l) {
        int r = (int) d + (int) f + (int) l;
        r += (byte) 300 + (char) -1 + (short) 70000;
        return r;
    }
    static int shifts(int a, long b) { return (a << 33) ^ (a >> 3) ^ (a >>> 5) ^ (int) (b << 65) ^ (int) (b >> 2) ^ (int) (b >>> 60); }
    static int cmp(double a, double b, float c, float d, long e, long f) {
        int r = 0;
        if (a < b) r |= 1; if (a > b) r |= 2; if (c <= d) r |= 4; if (c >= d) r |= 8; if (e == f) r |= 16; if (e != f) r |= 32;
        return r;
    }
    static long loops(int n) { long s = 0; int i = 0; while (i < n) { s += (long) i * i; i += 3; } return s; }
    public static void main(String[] args) {
        System.out.println("monolith slice 1");
        System.out.println(fib(20));
        System.out.println(mix(12345L));
        System.out.println(sw(1) + sw(2) + sw(5) + sw(9));
        System.out.println(conv(3.99e10, -2.5f, 1L << 40));
        System.out.println(conv(Double.NaN, Float.NEGATIVE_INFINITY, -1L));
        System.out.println(shifts(-12345, 0x123456789abcdefL));
        System.out.println(cmp(1.0, 2.0, 3f, 3f, 5L, 6L));
        System.out.println(cmp(Double.NaN, 1.0, Float.NaN, 1f, 7L, 7L));
        System.out.println(loops(1000000));
        System.out.println(Integer.MIN_VALUE / -1 == Integer.MIN_VALUE);
        System.out.println('x');
    }
}
