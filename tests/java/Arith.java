public class Arith {
    static int seed = 12345;
    static int next() { seed = seed * 1103515245 + 12345; return seed; }

    public static void main(String[] args) {
        int[] ints = {0, 1, -1, 7, -7, 13, Integer.MAX_VALUE, Integer.MIN_VALUE, 1 << 30, -(1 << 30)};
        long[] longs = {0L, 1L, -1L, 7L, -13L, Long.MAX_VALUE, Long.MIN_VALUE, 1L << 40, 123456789012345L};
        StringBuilder sb = new StringBuilder();
        for (int a : ints) for (int b : ints) {
            sb.append(a + b).append(' ').append(a - b).append(' ').append(a * b).append(' ');
            if (b != 0) sb.append(a / b).append(' ').append(a % b).append(' ');
            sb.append(a << b).append(' ').append(a >> b).append(' ').append(a >>> b).append(' ');
            sb.append(a & b).append(a | b).append(a ^ b).append(' ').append(Integer.compare(a, b)).append('\n');
        }
        for (long a : longs) for (long b : longs) {
            sb.append(a + b).append(' ').append(a - b).append(' ').append(a * b).append(' ');
            if (b != 0) sb.append(a / b).append(' ').append(a % b).append(' ');
            sb.append(a << (int) b).append(' ').append(a >> (int) b).append(' ').append(a >>> (int) b).append(' ');
            sb.append(Long.compare(a, b)).append('\n');
        }
        System.out.print(sb);
        try { System.out.println(1 / (ints[0])); } catch (ArithmeticException e) { System.out.println("AE " + e.getMessage()); }
        try { System.out.println(1L % (longs[0])); } catch (ArithmeticException e) { System.out.println("AE " + e.getMessage()); }
        double[] ds = {0.0, -0.0, 1.0, -1.5, 0.1, 1e-300, 1e300, Double.NaN, Double.POSITIVE_INFINITY,
                Double.NEGATIVE_INFINITY, Double.MIN_VALUE, Double.MAX_VALUE, 3.141592653589793, 2.5, -2.5, 1e7, 1e-3, 123456.789,
                9.999999999999999e22, 4.9e-324, 1.0E23, 2.0E-3, 100.0, 0.5, 1.0 / 3.0};
        for (double d : ds) {
            System.out.println(d + " " + (float) d + " " + (int) d + " " + (long) d + " " + (char) d + "|" + (short) d + " " + (byte) d
                    + " " + Math.floor(d) + " " + Math.ceil(d) + " " + Math.rint(d) + " " + Math.round(d) + " " + Math.abs(d)
                    + " " + Math.sqrt(d) + " " + StrictMath.sin(d) + " " + StrictMath.cos(d) + " " + StrictMath.exp(d)
                    + " " + StrictMath.log(d) + " " + StrictMath.pow(d, 1.5) + " " + StrictMath.atan2(d, 2.0) + " " + Math.signum(d)
                    + " " + Double.doubleToLongBits(d) + " " + Double.compare(d, 0.0) + " " + (d == d) + " " + (d < 1.0)
                    + " " + Math.max(d, -0.0) + " " + Math.min(d, 0.0) + " " + (d % 0.7) + " " + StrictMath.tanh(d) + " " + StrictMath.cbrt(d));
        }
        for (int i = 0; i < 2000; i++) {
            long bits = ((long) next() << 32) ^ (next() & 0xffffffffL);
            double d = Double.longBitsToDouble(bits);
            float f = Float.intBitsToFloat(next());
            System.out.println(d + " " + f + " " + Double.parseDouble(Double.toString(d)) + " " + Float.parseFloat(Float.toString(f))
                    + " " + (float) d + " " + (double) f + " " + (long) d + " " + (int) f);
        }
        float[] fs = {0f, -0f, 1f, 0.1f, 1e-45f, 3.4028235e38f, Float.NaN, 1.17549435E-38f, 16777216f, 0.3f, 100f, 1e7f, 1.0e-3f};
        for (float f : fs) System.out.println(f + " " + (f * 3) + " " + (f / 7) + " " + (int) f + " " + Float.floatToIntBits(f) + " " + Math.round(f));
        System.out.println(Integer.toString(255, 16) + " " + Integer.toHexString(-1) + " " + Integer.toBinaryString(42) + " " + Integer.toOctalString(-8)
                + " " + Long.toHexString(Long.MIN_VALUE) + " " + Integer.parseInt("-2147483648") + " " + Long.parseLong("9223372036854775807")
                + " " + Integer.bitCount(0xF0F0) + " " + Integer.reverse(1) + " " + Long.numberOfTrailingZeros(1L << 50) + " " + Integer.highestOneBit(1000)
                + " " + Integer.parseInt("zz", 36) + " " + Integer.toUnsignedString(-1) + " " + Long.toUnsignedString(-1L) + " " + Integer.divideUnsigned(-1, 3)
                + " " + Long.remainderUnsigned(-1L, 7) + " " + Character.getNumericValue('7') + " " + Character.isLetter('x') + " " + Character.toUpperCase('q'));
        try { Integer.parseInt("12a"); } catch (NumberFormatException e) { System.out.println(e.getMessage()); }
        try { Long.parseLong(""); } catch (NumberFormatException e) { System.out.println(e.getMessage()); }
        char c = 'A'; c += 2; c++;
        short s = (short) 40000; byte b = (byte) 200;
        System.out.println(c + " " + (int) c + " " + s + " " + b + " " + (s + b) + " " + (char) (c + 1) + " " + (b >>> 2) + " " + (byte) (b << 1));
        System.out.println(Math.floorDiv(-7, 2) + " " + Math.floorMod(-7, 2) + " " + Math.abs(Integer.MIN_VALUE) + " " + Math.hypot(3, 4) + " " + StrictMath.log10(1000)
                + " " + Math.toDegrees(Math.PI) + " " + Math.addExact(1, 2) + " " + Math.multiplyHigh(Long.MAX_VALUE, 3));
        try { Math.addExact(Integer.MAX_VALUE, 1); } catch (ArithmeticException e) { System.out.println(e.getMessage()); }
        try { Math.toIntExact(1L << 33); } catch (ArithmeticException e) { System.out.println(e.getMessage()); }
    }
}
