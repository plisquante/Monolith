public class Intrinsics {
    static String d(double x) { return Long.toHexString(Double.doubleToRawLongBits(x)); }
    static String f(float x) { return Integer.toHexString(Float.floatToRawIntBits(x)); }

    public static void main(String[] args) {
        double[] ds = {0.0, -0.0, 1.5, -1.5, 2.0, -2.0, 0.49999999999999994, -0.5, 1e300, -1e300, 4.5e15, -4.5e15,
                Double.NaN, Double.POSITIVE_INFINITY, Double.NEGATIVE_INFINITY, Double.MIN_VALUE, -Double.MIN_VALUE,
                Double.MAX_VALUE, 123456.789, -123456.789, 9.0, 2.0000000000000004};
        for (double a : ds) {
            System.out.println("u " + d(a) + " floor " + d(Math.floor(a)) + " ceil " + d(Math.ceil(a))
                    + " sqrt " + d(Math.sqrt(a)) + " abs " + d(Math.abs(a)) + " sfloor " + d(StrictMath.floor(a)));
            for (double b : ds) {
                System.out.println("b " + d(a) + " " + d(b) + " min " + d(Math.min(a, b)) + " max " + d(Math.max(a, b)));
            }
        }
        float[] fs = {0f, -0f, 1.5f, -1.5f, Float.NaN, Float.POSITIVE_INFINITY, Float.NEGATIVE_INFINITY, Float.MIN_VALUE, Float.MAX_VALUE};
        for (float a : fs) {
            System.out.println("f " + f(a) + " abs " + f(Math.abs(a)));
            for (float b : fs) System.out.println("fb " + f(a) + " " + f(b) + " " + f(Math.min(a, b)) + " " + f(Math.max(a, b)));
        }
        int[] is = {0, 1, -1, 2, -2, 7, -7, 100, -100, Integer.MAX_VALUE, Integer.MIN_VALUE, 65536, -65536};
        for (int a : is) {
            System.out.println("i " + a + " abs " + Math.abs(a) + " bc " + Integer.bitCount(a) + " lz " + Integer.numberOfLeadingZeros(a)
                    + " tz " + Integer.numberOfTrailingZeros(a) + " ul " + Integer.toUnsignedLong(a)
                    + " raw " + Float.intBitsToFloat(a) + " " + f(Float.intBitsToFloat(a)));
            for (int b : is) {
                String r;
                try { r = Math.floorDiv(a, b) + " " + Math.floorMod(a, b); } catch (ArithmeticException e) { r = e.getMessage(); }
                System.out.println("ib " + a + " " + b + " min " + Math.min(a, b) + " max " + Math.max(a, b) + " fd/fm " + r);
            }
        }
        long[] ls = {0, 1, -1, 7, -7, 1L << 40, -(1L << 40), Long.MAX_VALUE, Long.MIN_VALUE, 123456789012345L};
        for (long a : ls) {
            System.out.println("l " + a + " abs " + Math.abs(a) + " bc " + Long.bitCount(a) + " lz " + Long.numberOfLeadingZeros(a)
                    + " tz " + Long.numberOfTrailingZeros(a) + " d " + d(Double.longBitsToDouble(a)));
            for (long b : ls) {
                String r;
                try { r = Math.floorDiv(a, b) + " " + Math.floorMod(a, b); } catch (ArithmeticException e) { r = e.getMessage(); }
                System.out.println("lb " + a + " " + b + " min " + Math.min(a, b) + " max " + Math.max(a, b) + " fd/fm " + r);
            }
        }
        for (double a : ds) System.out.println("exp " + d(a) + " " + Math.getExponent(a) + " " + Math.getExponent((float) a));
        for (int a : is) {
            System.out.println("ci " + a + " sig " + Integer.signum(a) + " h " + Integer.hashCode(a)
                    + " rl " + Integer.rotateLeft(a, 5) + " rr " + Integer.rotateRight(a, 37) + " rl0 " + Integer.rotateLeft(a, -3));
            for (int b : is) System.out.println("cmp " + a + " " + b + " " + Integer.compare(a, b));
        }
        for (long a : ls) {
            System.out.println("cl " + a + " sig " + Long.signum(a) + " h " + Long.hashCode(a)
                    + " rl " + Long.rotateLeft(a, 13) + " rr " + Long.rotateRight(a, 70));
            for (long b : ls) System.out.println("cmpl " + a + " " + b + " " + Long.compare(a, b));
        }
        long h = 0;
        for (int i = 0; i < 2_000_000; i++) {
            double x = (i - 1_000_000) * 0.37;
            h = h * 31 + (long) Math.floor(x) + (long) Math.ceil(x) + Math.floorMod(i, 7) + Math.abs(i - 50000);
        }
        System.out.println("h " + h);
    }
}
