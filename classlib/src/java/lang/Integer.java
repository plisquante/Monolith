package java.lang;

/** j2zig overlay of java.lang.Integer. */
public final class Integer extends Number implements Comparable<Integer> {
    public static final int MIN_VALUE = 0x80000000;
    public static final int MAX_VALUE = 0x7fffffff;
    @SuppressWarnings("unchecked")
    public static final Class<Integer> TYPE = (Class<Integer>) Class.getPrimitiveClass("int");
    public static final int SIZE = 32;
    public static final int BYTES = 4;
    static final char[] digits = {
        '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h',
        'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z'
    };
    private static final long serialVersionUID = 1360826667806852920L;
    private final int value;

    private static final class IntegerCache {
        static final Integer[] cache = new Integer[256];
        static {
            for (int i = 0; i < 256; i++) cache[i] = new Integer(i - 128);
        }
    }

    @Deprecated(since = "9", forRemoval = true)
    public Integer(int value) {
        this.value = value;
    }

    @Deprecated(since = "9", forRemoval = true)
    public Integer(String s) throws NumberFormatException {
        this.value = parseInt(s, 10);
    }

    public static String toString(int i, int radix) {
        if (radix < Character.MIN_RADIX || radix > Character.MAX_RADIX) radix = 10;
        if (radix == 10) return toString(i);
        char[] buf = new char[33];
        boolean negative = (i < 0);
        int charPos = 32;
        if (!negative) i = -i;
        while (i <= -radix) {
            buf[charPos--] = digits[-(i % radix)];
            i = i / radix;
        }
        buf[charPos] = digits[-i];
        if (negative) buf[--charPos] = '-';
        return new String(buf, charPos, (33 - charPos));
    }

    public static String toUnsignedString(int i, int radix) {
        return Long.toUnsignedString(toUnsignedLong(i), radix);
    }

    public static String toHexString(int i) {
        return toUnsignedString0(i, 4);
    }

    public static String toOctalString(int i) {
        return toUnsignedString0(i, 3);
    }

    public static String toBinaryString(int i) {
        return toUnsignedString0(i, 1);
    }

    private static String toUnsignedString0(int val, int shift) {
        int mag = SIZE - numberOfLeadingZeros(val);
        int chars = Math.max(((mag + (shift - 1)) / shift), 1);
        char[] buf = new char[chars];
        int charPos = chars;
        int radix = 1 << shift;
        int mask = radix - 1;
        do {
            buf[--charPos] = digits[val & mask];
            val >>>= shift;
        } while (charPos > 0);
        return new String(buf, true);
    }

    public static String toString(int i) {
        int size = stringSize(i);
        char[] buf = new char[size];
        getChars(i, size, buf);
        return new String(buf, true);
    }

    public static String toUnsignedString(int i) {
        return Long.toString(toUnsignedLong(i));
    }

    static int getChars(int i, int index, char[] buf) {
        boolean negative = i < 0;
        if (!negative) i = -i;
        while (i <= -10) {
            int q = i / 10;
            buf[--index] = (char) ('0' + (q * 10 - i));
            i = q;
        }
        buf[--index] = (char) ('0' - i);
        if (negative) buf[--index] = '-';
        return index;
    }

    static int stringSize(int x) {
        int d = 1;
        if (x >= 0) {
            d = 0;
            x = -x;
        }
        int p = -10;
        for (int i = 1; i < 10; i++) {
            if (x > p) return i + d;
            p = 10 * p;
        }
        return 10 + d;
    }

    public static int parseInt(String s, int radix) throws NumberFormatException {
        if (s == null) throw new NumberFormatException("Cannot parse null string: null");
        return parseInt(s, 0, s.length(), radix, s);
    }

    public static int parseInt(CharSequence s, int beginIndex, int endIndex, int radix) throws NumberFormatException {
        java.util.Objects.requireNonNull(s);
        java.util.Objects.checkFromToIndex(beginIndex, endIndex, s.length());
        return parseInt(s, beginIndex, endIndex, radix, null);
    }

    private static int parseInt(CharSequence s, int i, int len, int radix, String whole) throws NumberFormatException {
        if (radix < Character.MIN_RADIX)
            throw new NumberFormatException("radix " + radix + " less than Character.MIN_RADIX");
        if (radix > Character.MAX_RADIX)
            throw new NumberFormatException("radix " + radix + " greater than Character.MAX_RADIX");
        boolean negative = false;
        int limit = -Integer.MAX_VALUE;
        int start = i;
        if (i < len) {
            char firstChar = s.charAt(i);
            if (firstChar < '0') {
                if (firstChar == '-') {
                    negative = true;
                    limit = Integer.MIN_VALUE;
                } else if (firstChar != '+') {
                    throw forInput(s, start, len, radix, whole);
                }
                i++;
                if (i == len) throw forInput(s, start, len, radix, whole);
            }
            int multmin = limit / radix;
            int result = 0;
            while (i < len) {
                int digit = Character.digit(s.charAt(i++), radix);
                if (digit < 0 || result < multmin) throw forInput(s, start, len, radix, whole);
                result *= radix;
                if (result < limit + digit) throw forInput(s, start, len, radix, whole);
                result -= digit;
            }
            return negative ? result : -result;
        }
        if (whole != null) throw new NumberFormatException("For input string: \"" + whole + "\"" + (radix == 10 ? "" : " under radix " + radix));
        throw new NumberFormatException("");
    }

    static NumberFormatException forInput(CharSequence s, int start, int end, int radix, String whole) {
        if (whole != null)
            return new NumberFormatException("For input string: \"" + whole + "\"" + (radix == 10 ? "" : " under radix " + radix));
        return new NumberFormatException("Error at index " + (end - start) + " in: \"" + s.subSequence(start, end) + "\"");
    }

    public static int parseInt(String s) throws NumberFormatException {
        return parseInt(s, 10);
    }

    public static int parseUnsignedInt(String s, int radix) throws NumberFormatException {
        if (s == null) throw new NumberFormatException("Cannot parse null string: null");
        int len = s.length();
        if (len > 0) {
            char firstChar = s.charAt(0);
            if (firstChar == '-') {
                throw new NumberFormatException("Illegal leading minus sign on unsigned string " + s + ".");
            }
            if (len <= 5 || (radix == 10 && len <= 9)) return parseInt(s, radix);
            long ell = Long.parseLong(s, radix);
            if ((ell & 0xffff_ffff_0000_0000L) == 0) return (int) ell;
            throw new NumberFormatException("String value " + s + " exceeds range of unsigned int.");
        }
        throw new NumberFormatException("For input string: \"" + s + "\"" + (radix == 10 ? "" : " under radix " + radix));
    }

    public static int parseUnsignedInt(CharSequence s, int beginIndex, int endIndex, int radix) throws NumberFormatException {
        java.util.Objects.requireNonNull(s);
        java.util.Objects.checkFromToIndex(beginIndex, endIndex, s.length());
        return parseUnsignedInt(s.subSequence(beginIndex, endIndex).toString(), radix);
    }

    public Integer resolveConstantDesc(java.lang.invoke.MethodHandles.Lookup lookup) {
        return this;
    }

    public static int parseUnsignedInt(String s) throws NumberFormatException {
        return parseUnsignedInt(s, 10);
    }

    public static Integer valueOf(String s, int radix) throws NumberFormatException {
        return Integer.valueOf(parseInt(s, radix));
    }

    public static Integer valueOf(String s) throws NumberFormatException {
        return Integer.valueOf(parseInt(s, 10));
    }

    public static Integer valueOf(int i) {
        if (i >= -128 && i <= 127) return IntegerCache.cache[i + 128];
        return new Integer(i);
    }

    public byte byteValue() { return (byte) value; }
    public short shortValue() { return (short) value; }
    public int intValue() { return value; }
    public long longValue() { return (long) value; }
    public float floatValue() { return (float) value; }
    public double doubleValue() { return (double) value; }

    public String toString() {
        return toString(value);
    }

    public int hashCode() {
        return value;
    }

    public static int hashCode(int value) {
        return value;
    }

    public boolean equals(Object obj) {
        if (obj instanceof Integer i) return value == i.value;
        return false;
    }

    public static Integer getInteger(String nm) {
        return getInteger(nm, null);
    }

    public static Integer getInteger(String nm, int val) {
        Integer result = getInteger(nm, null);
        return (result == null) ? Integer.valueOf(val) : result;
    }

    public static Integer getInteger(String nm, Integer val) {
        String v = null;
        try {
            v = System.getProperty(nm);
        } catch (IllegalArgumentException | NullPointerException e) {
        }
        if (v != null) {
            try {
                return Integer.decode(v);
            } catch (NumberFormatException e) {
            }
        }
        return val;
    }

    public static Integer decode(String nm) throws NumberFormatException {
        int radix = 10;
        int index = 0;
        boolean negative = false;
        int result;
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
            result = parseInt(nm, index, nm.length(), radix);
            result = negative ? -result : result;
        } catch (NumberFormatException e) {
            String constant = negative ? ("-" + nm.substring(index)) : nm.substring(index);
            result = parseInt(constant, radix);
        }
        return result;
    }

    public int compareTo(Integer anotherInteger) {
        return compare(this.value, anotherInteger.value);
    }

    public static int compare(int x, int y) {
        return (x < y) ? -1 : ((x == y) ? 0 : 1);
    }

    public static int compareUnsigned(int x, int y) {
        return compare(x + MIN_VALUE, y + MIN_VALUE);
    }

    public static long toUnsignedLong(int x) {
        return ((long) x) & 0xffffffffL;
    }

    public static int divideUnsigned(int dividend, int divisor) {
        return (int) (toUnsignedLong(dividend) / toUnsignedLong(divisor));
    }

    public static int remainderUnsigned(int dividend, int divisor) {
        return (int) (toUnsignedLong(dividend) % toUnsignedLong(divisor));
    }

    public static int highestOneBit(int i) {
        return i & (MIN_VALUE >>> numberOfLeadingZeros(i));
    }

    public static int lowestOneBit(int i) {
        return i & -i;
    }

    public static int numberOfLeadingZeros(int i) {
        if (i <= 0) return i == 0 ? 32 : 0;
        int n = 31;
        if (i >= 1 << 16) { n -= 16; i >>>= 16; }
        if (i >= 1 << 8) { n -= 8; i >>>= 8; }
        if (i >= 1 << 4) { n -= 4; i >>>= 4; }
        if (i >= 1 << 2) { n -= 2; i >>>= 2; }
        return n - (i >>> 1);
    }

    public static int numberOfTrailingZeros(int i) {
        i = ~i & (i - 1);
        if (i <= 0) return i & 32;
        int n = 1;
        if (i > 1 << 16) { n += 16; i >>>= 16; }
        if (i > 1 << 8) { n += 8; i >>>= 8; }
        if (i > 1 << 4) { n += 4; i >>>= 4; }
        if (i > 1 << 2) { n += 2; i >>>= 2; }
        return n + (i >>> 1);
    }

    public static int bitCount(int i) {
        i = i - ((i >>> 1) & 0x55555555);
        i = (i & 0x33333333) + ((i >>> 2) & 0x33333333);
        i = (i + (i >>> 4)) & 0x0f0f0f0f;
        i = i + (i >>> 8);
        i = i + (i >>> 16);
        return i & 0x3f;
    }

    public static int rotateLeft(int i, int distance) {
        return (i << distance) | (i >>> -distance);
    }

    public static int rotateRight(int i, int distance) {
        return (i >>> distance) | (i << -distance);
    }

    public static int reverse(int i) {
        i = (i & 0x55555555) << 1 | (i >>> 1) & 0x55555555;
        i = (i & 0x33333333) << 2 | (i >>> 2) & 0x33333333;
        i = (i & 0x0f0f0f0f) << 4 | (i >>> 4) & 0x0f0f0f0f;
        return reverseBytes(i);
    }

    public static int compress(int i, int mask) {
        int r = 0;
        for (int bb = 1; mask != 0; bb += bb) {
            if ((i & mask & -mask) != 0) r |= bb;
            mask &= mask - 1;
        }
        return r;
    }

    public static int expand(int i, int mask) {
        int r = 0;
        for (int bb = 1; mask != 0; bb += bb) {
            if ((i & bb) != 0) r |= mask & -mask;
            mask &= mask - 1;
        }
        return r;
    }

    public static int signum(int i) {
        return (i >> 31) | (-i >>> 31);
    }

    public static int reverseBytes(int i) {
        return (i << 24) | ((i & 0xff00) << 8) | ((i >>> 8) & 0xff00) | (i >>> 24);
    }

    public static int sum(int a, int b) {
        return a + b;
    }

    public static int max(int a, int b) {
        return Math.max(a, b);
    }

    public static int min(int a, int b) {
        return Math.min(a, b);
    }

    public java.util.Optional<Integer> describeConstable() {
        return java.util.Optional.of(this);
    }
}
