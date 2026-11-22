public class PatchTarget {
    static int mix(int a, int b) { return (a * 31 + b) ^ (a >>> 3); }
    static double scale(double x, float f) { return x * f + 0.5; }
    int field = 5;
    long sum(long k) { return k + field; }

    public static void main(String[] args) {
        int h = 0;
        double d = 0;
        for (int i = 0; i < 1000; i++) {
            h += mix(i, h);
            d += scale(i, 1.5f);
        }
        PatchTarget p = new PatchTarget();
        System.out.println(h + " " + d + " " + p.sum(10));
    }
}
