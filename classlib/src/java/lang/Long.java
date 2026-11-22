package java.lang;

/** monolith overlay of java.lang.Long. */
public final class Long extends Number implements Comparable<Long> {
    public static final long MIN_VALUE = 0x8000000000000000L;
    public static final long MAX_VALUE = 0x7fffffffffffffffL;
    @SuppressWarnings("unchecked")
    public static final Class<Long> TYPE = (Class<Long>) Class.getPrimitiveClass("long");
    public static final int SIZE = 64;
    public static final int BYTES = 8;
    private static final long serialVersionUID = 4290774380558885855L;
    private final long value;

    private static final class LongCache {
        static final Long[] cache = new Long[256];
        static {
            for (int i = 0; i < 256; i++) cache[i] = new Long(i - 128);
        }
    }

    @Deprecated(since = "9", forRemoval = true)
    public Long(long value) {
        this.value = value;
    }

    @Deprecated(since = "9", forRemoval = true)
    public Long(String s) throws NumberFormatException {
        this.value = parseLong(s, 10);
    }

    public static String toString(long i, int radix) {
        if (radix < Character.MIN_RADIX || radix > Character.MAX_RADIX) radix = 10;
        if (radix == 10) return toString(i);
        char[] buf = new char[65];
        int charPos = 64;
        boolean negative = (i < 0);
        if (!negative) i = -i;
        while (i <= -radix) {
            buf[charPos--] = Integer.digits[(int) (-(i % radix))];
            i = i / radix;
        }
        buf[charPos] = Integer.digits[(int) (-i)];
        if (negative) buf[--charPos] = '-';
        return new String(buf, charPos, (65 - charPos));
    }

    public static String toUnsignedString(long i, int radix) {
        if (i >= 0) return toString(i, radix);
        switch (radix) {
            case 2: return toBinaryString(i);
            case 4: return toUnsignedString0(i, 2);
            case 8: return toOctalString(i);
            case 16: return toHexString(i);
            case 32: return toUnsignedString0(i, 5);
            default: {
                if (radix < Character.MIN_RADIX || radix > Character.MAX_RADIX) radix = 10;
                long quotient = divideUnsigned(i, radix);
                long rem = i - quotient * radix;
                return toString(quotient, radix) + Character.forDigit((int) rem, radix);
            }
        }
    }

    public static String toHexString(long i) {
        return toUnsignedString0(i, 4);
    }

    public static String toOctalString(long i) {
        return toUnsignedString0(i, 3);
    }

    public static String toBinaryString(long i) {
        return toUnsignedString0(i, 1);
    }

    static String toUnsignedString0(long val, int shift) {
        int mag = SIZE - numberOfLeadingZeros(val);
        int chars = Math.max(((mag + (shift - 1)) / shift), 1);
        char[] buf = new char[chars];
        int charPos = chars;
        int radix = 1 << shift;
        int mask = radix - 1;
        do {
            buf[--charPos] = Integer.digits[((int) val) & mask];
            val >>>= shift;
        } while (charPos > 0);
        return new String(buf, true);
    }

    public static String toString(long i) {
        int size = stringSize(i);
        char[] buf = new char[size];
        getChars(i, size, buf);
        return new String(buf, true);
    }

    public static String toUnsignedString(long i) {
        return toUnsignedString(i, 10);
    }

    static int getChars(long i, int index, char[] buf) {
        boolean negative = i < 0;
        if (!negative) i = -i;
        while (i <= -10) {
            long q = i / 10;
            buf[--index] = (char) ('0' + (int) (q * 10 - i));
            i = q;
        }
        buf[--index] = (char) ('0' - (int) i);
        if (negative) buf[--index] = '-';
        return index;
    }

    static int stringSize(long x) {
        int d = 1;
        if (x >= 0) {
            d = 0;
            x = -x;
        }
        long p = -10;
        for (int i = 1; i < 19; i++) {
            if (x > p) return i + d;
            p = 10 * p;
        }
        return 19 + d;
    }

    public static long parseLong(String s, int radix) throws NumberFormatException {
        if (s == null) throw new NumberFormatException("Cannot parse null string: null");
        return parseLong(s, 0, s.length(), radix, s);
    }

    public static long parseLong(CharSequence s, int beginIndex, int endIndex, int radix) throws NumberFormatException {
        java.util.Objects.requireNonNull(s);
        java.util.Objects.checkFromToIndex(beginIndex, endIndex, s.length());
        return parseLong(s, beginIndex, endIndex, radix, null);
    }

    private static long parseLong(CharSequence s, int i, int len, int radix, String whole) throws NumberFormatException {
        if (radix < Character.MIN_RADIX)
            throw new NumberFormatException("radix " + radix + " less than Character.MIN_RADIX");
        if (radix > Character.MAX_RADIX)
            throw new NumberFormatException("radix " + radix + " greater than Character.MAX_RADIX");
        boolean negative = false;
        long limit = -Long.MAX_VALUE;
        int start = i;
        if (i < len) {
            char firstChar = s.charAt(i);
            if (firstChar < '0') {
                if (firstChar == '-') {
                    negative = true;
                    limit = Long.MIN_VALUE;
                } else if (firstChar != '+') {
                    throw Integer.forInput(s, start, len, radix, whole);
                }
                i++;
                if (i == len) throw Integer.forInput(s, start, len, radix, whole);
            }
            long multmin = limit / radix;
            long result = 0;
            while (i < len) {
                int digit = Character.digit(s.charAt(i++), radix);
                if (digit < 0 || result < multmin) throw Integer.forInput(s, start, len, radix, whole);
                result *= radix;
                if (result < limit + digit) throw Integer.forInput(s, start, len, radix, whole);
                result -= digit;
            }
            return negative ? result : -result;
        }
        if (whole != null) throw new NumberFormatException("For input string: \"" + whole + "\"" + (radix == 10 ? "" : " under radix " + radix));
        throw new NumberFormatException("");
    }

    public static long parseLong(String s) throws NumberFormatException {
        return parseLong(s, 10);
    }

    public static long parseUnsignedLong(String s, int radix) throws NumberFormatException {
        if (s == null) throw new NumberFormatException("Cannot parse null string: null");
        int len = s.length();
        if (len > 0) {
            char firstChar = s.charAt(0);
            if (firstChar == '-') throw new NumberFormatException("Illegal leading minus sign on unsigned string " + s + ".");
            if (len <= 12 || (radix == 10 && len <= 18)) return parseLong(s, radix);
            long first = parseLong(s, 0, len - 1, radix);
            int second = Character.digit(s.charAt(len - 1), radix);
            if (second < 0) throw new NumberFormatException("Bad digit at end of " + s);
            long result = first * radix + second;
            int guard = radix * (int) (first >>> 57);
            if (guard >= 128 || (result >= 0 && guard >= 128 - Character.MAX_RADIX))
                throw new NumberFormatException("String value " + s + " exceeds range of unsigned long.");
            return result;
        }
        throw new NumberFormatException("For input string: \"" + s + "\"");
    }

    public static long parseUnsignedLong(CharSequence s, int beginIndex, int endIndex, int radix) throws NumberFormatException {
        java.util.Objects.requireNonNull(s);
        java.util.Objects.checkFromToIndex(beginIndex, endIndex, s.length());
        return parseUnsignedLong(s.subSequence(beginIndex, endIndex).toString(), radix);
    }

    public static long compress(long i, long mask) {
        long r = 0;
        for (long bb = 1; mask != 0; bb += bb) {
            if ((i & mask & -mask) != 0) r |= bb;
            mask &= mask - 1;
        }
        return r;
    }

    public static long expand(long i, long mask) {
        long r = 0;
        for (long bb = 1; mask != 0; bb += bb) {
            if ((i & bb) != 0) r |= mask & -mask;
            mask &= mask - 1;
        }
        return r;
    }

    public Long resolveConstantDesc(java.lang.invoke.MethodHandles.Lookup lookup) {
        return this;
    }

    public static long parseUnsignedLong(String s) throws NumberFormatException {
        return parseUnsignedLong(s, 10);
    }

    public static Long valueOf(String s, int radix) throws NumberFormatException {
        return Long.valueOf(parseLong(s, radix));
    }

    public static Long valueOf(String s) throws NumberFormatException {
        return Long.valueOf(parseLong(s, 10));
    }

    public static Long valueOf(long l) {
        if (l >= -128 && l <= 127) return LongCache.cache[(int) l + 128];
        return new Long(l);
    }

    public static Long decode(String nm) throws NumberFormatException {
        int radix = 10;
        int index = 0;
        boolean negative = false;
        long result;
        if (nm.isEmpty()) throw new NumberFormatException("Zero length string");
        char firstChar = nm.charAt(0);
        if (firstChar == '-') {
            negative = true;
            index++;
        } else if (firstChar == '+') index++;
        if (nm.startsWith("0x", index) || nm.startsWith("0X", index)) {
            index += 2;
            radix = 16;
        } else if (nm.startsWith("#", index)) {
            index++;
            radix = 16;
        } else if (nm.startsWith("0", index) && nm.length() > 1 + index) {
            index++;
            radix = 8;
        }
        if (nm.startsWith("-", index) || nm.startsWith("+", index))
            throw new NumberFormatException("Sign character in wrong position");
        try {
            result = parseLong(nm, index, nm.length(), radix);
            result = negative ? -result : result;
        } catch (NumberFormatException e) {
            String constant = negative ? ("-" + nm.substring(index)) : nm.substring(index);
            result = parseLong(constant, radix);
        }
        return result;
    }

    public byte byteValue() { return (byte) value; }
    public short shortValue() { return (short) value; }
    public int intValue() { return (int) value; }
    public long longValue() { return value; }
    public float floatValue() { return (float) value; }
    public double doubleValue() { return (double) value; }

    public String toString() {
        return toString(value);
    }

    public int hashCode() {
        return Long.hashCode(value);
    }

    public static int hashCode(long value) {
        return (int) (value ^ (value >>> 32));
    }

    public boolean equals(Object obj) {
        if (obj instanceof Long l) return value == l.value;
        return false;
    }

    public static Long getLong(String nm) {
        return getLong(nm, null);
    }

    public static Long getLong(String nm, long val) {
        Long result = Long.getLong(nm, null);
        return (result == null) ? Long.valueOf(val) : result;
    }

    public static Long getLong(String nm, Long val) {
        String v = null;
        try {
            v = System.getProperty(nm);
        } catch (IllegalArgumentException | NullPointerException e) {
        }
        if (v != null) {
            try {
                return Long.decode(v);
            } catch (NumberFormatException e) {
            }
        }
        return val;
    }

    public int compareTo(Long anotherLong) {
        return compare(this.value, anotherLong.value);
    }

    public static int compare(long x, long y) {
        return (x < y) ? -1 : ((x == y) ? 0 : 1);
    }

    public static int compareUnsigned(long x, long y) {
        return compare(x + MIN_VALUE, y + MIN_VALUE);
    }

    public static long divideUnsigned(long dividend, long divisor) {
        if (divisor < 0) return (compareUnsigned(dividend, divisor) < 0) ? 0L : 1L;
        if (dividend > 0) return dividend / divisor;
        long q = ((dividend >>> 1) / divisor) << 1;
        long r = dividend - q * divisor;
        return q + (compareUnsigned(r, divisor) >= 0 ? 1 : 0);
    }

    public static long remainderUnsigned(long dividend, long divisor) {
        return dividend - divideUnsigned(dividend, divisor) * divisor;
    }

    public static long highestOneBit(long i) {
        return i & (MIN_VALUE >>> numberOfLeadingZeros(i));
    }

    public static long lowestOneBit(long i) {
        return i & -i;
    }

    public static int numberOfLeadingZeros(long i) {
        int x = (int) (i >>> 32);
        return x == 0 ? 32 + Integer.numberOfLeadingZeros((int) i) : Integer.numberOfLeadingZeros(x);
    }

    public static int numberOfTrailingZeros(long i) {
        int x = (int) i;
        return x == 0 ? 32 + Integer.numberOfTrailingZeros((int) (i >>> 32)) : Integer.numberOfTrailingZeros(x);
    }

    public static int bitCount(long i) {
        i = i - ((i >>> 1) & 0x5555555555555555L);
        i = (i & 0x3333333333333333L) + ((i >>> 2) & 0x3333333333333333L);
        i = (i + (i >>> 4)) & 0x0f0f0f0f0f0f0f0fL;
        i = i + (i >>> 8);
        i = i + (i >>> 16);
        i = i + (i >>> 32);
        return (int) i & 0x7f;
    }

    public static long rotateLeft(long i, int distance) {
        return (i << distance) | (i >>> -distance);
    }

    public static long rotateRight(long i, int distance) {
        return (i >>> distance) | (i << -distance);
    }

    public static long reverse(long i) {
        i = (i & 0x5555555555555555L) << 1 | (i >>> 1) & 0x5555555555555555L;
        i = (i & 0x3333333333333333L) << 2 | (i >>> 2) & 0x3333333333333333L;
        i = (i & 0x0f0f0f0f0f0f0f0fL) << 4 | (i >>> 4) & 0x0f0f0f0f0f0f0f0fL;
        return reverseBytes(i);
    }

    public static int signum(long i) {
        return (int) ((i >> 63) | (-i >>> 63));
    }

    public static long reverseBytes(long i) {
        i = (i & 0x00ff00ff00ff00ffL) << 8 | (i >>> 8) & 0x00ff00ff00ff00ffL;
        return (i << 48) | ((i & 0xffff0000L) << 16) | ((i >>> 16) & 0xffff0000L) | (i >>> 48);
    }

    public static long sum(long a, long b) {
        return a + b;
    }

    public static long max(long a, long b) {
        return Math.max(a, b);
    }

    public static long min(long a, long b) {
        return Math.min(a, b);
    }

    public java.util.Optional<Long> describeConstable() {
        return java.util.Optional.of(this);
    }
}
