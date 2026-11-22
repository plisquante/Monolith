package java.lang;

import java.nio.charset.Charset;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.Iterator;
import java.util.List;
import java.util.Locale;
import java.util.StringJoiner;

/**
 * monolith overlay of java.lang.String: immutable UTF-16 code unit sequence stored in a char[].
 * The runtime (rt.zig / generated hooks) creates literals directly through the {@code value} field.
 */
public final class String implements java.io.Serializable, Comparable<String>, CharSequence {
    private final char[] value;
    private int hash;
    private boolean hashIsZero;
    // Satisfies reflective field lookups such as LWJGL MemoryBackendUnsafeLegacy9 ("coder"); packs in hashIsZero padding (0B extra).
    private final byte coder = UTF16;

    private static final long serialVersionUID = -6849794470754667710L;
    static final boolean COMPACT_STRINGS = false;
    static final byte LATIN1 = 0;
    static final byte UTF16 = 1;

    public static final Comparator<String> CASE_INSENSITIVE_ORDER = new CaseInsensitiveComparator();

    private static final class CaseInsensitiveComparator implements Comparator<String>, java.io.Serializable {
        public int compare(String s1, String s2) {
            return s1.compareToIgnoreCase(s2);
        }
    }

    public String() {
        value = new char[0];
    }

    public String(String original) {
        value = original.value;
        hash = original.hash;
    }

    public String(char[] value) {
        this.value = value.clone();
    }

    public String(char[] value, int offset, int count) {
        checkBoundsOffCount(offset, count, value.length);
        char[] v = new char[count];
        System.arraycopy(value, offset, v, 0, count);
        this.value = v;
    }

    public String(int[] codePoints, int offset, int count) {
        checkBoundsOffCount(offset, count, codePoints.length);
        int n = 0;
        for (int i = offset; i < offset + count; i++) {
            int cp = codePoints[i];
            if (Character.isBmpCodePoint(cp)) n++;
            else if (Character.isValidCodePoint(cp)) n += 2;
            else throw new IllegalArgumentException(Integer.toString(cp));
        }
        char[] v = new char[n];
        int j = 0;
        for (int i = offset; i < offset + count; i++) {
            int cp = codePoints[i];
            if (Character.isBmpCodePoint(cp)) v[j++] = (char) cp;
            else {
                v[j++] = Character.highSurrogate(cp);
                v[j++] = Character.lowSurrogate(cp);
            }
        }
        this.value = v;
    }

    @Deprecated(since = "1.1")
    public String(byte[] ascii, int hibyte, int offset, int count) {
        checkBoundsOffCount(offset, count, ascii.length);
        char[] v = new char[count];
        for (int i = 0; i < count; i++) v[i] = (char) (((hibyte & 0xff) << 8) | (ascii[i + offset] & 0xff));
        this.value = v;
    }

    @Deprecated(since = "1.1")
    public String(byte[] ascii, int hibyte) {
        this(ascii, hibyte, 0, ascii.length);
    }

    public String(byte[] bytes, int offset, int length, String charsetName) throws java.io.UnsupportedEncodingException {
        checkBoundsOffCount(offset, length, bytes.length);
        this.value = decode(lookupCharset(charsetName), bytes, offset, length);
    }

    public String(byte[] bytes, int offset, int length, Charset charset) {
        checkBoundsOffCount(offset, length, bytes.length);
        this.value = decode(charsetCode(charset), bytes, offset, length);
    }

    public String(byte[] bytes, String charsetName) throws java.io.UnsupportedEncodingException {
        this(bytes, 0, bytes.length, charsetName);
    }

    public String(byte[] bytes, Charset charset) {
        this(bytes, 0, bytes.length, charset);
    }

    public String(byte[] bytes, int offset, int length) {
        checkBoundsOffCount(offset, length, bytes.length);
        this.value = decode(CS_UTF8, bytes, offset, length);
    }

    public String(byte[] bytes) {
        this(bytes, 0, bytes.length);
    }

    public String(StringBuffer buffer) {
        this.value = buffer.toString().value;
    }

    public String(StringBuilder builder) {
        this.value = builder.toString().value;
    }

    /** Package-private sharing constructor (no copy). */
    String(char[] value, boolean share) {
        this.value = value;
    }

    // ---------------------------------------------------------------- charsets
    static final int CS_UTF8 = 0, CS_LATIN1 = 1, CS_ASCII = 2, CS_UTF16BE = 3, CS_UTF16LE = 4, CS_UTF16 = 5;

    private static int lookupCharset(String name) throws java.io.UnsupportedEncodingException {
        if (name == null) throw new NullPointerException("charsetName");
        int c = charsetCodeByName(name);
        if (c < 0) throw new java.io.UnsupportedEncodingException(name);
        return c;
    }

    static int charsetCodeByName(String name) {
        char[] u = name.value.clone();
        for (int i = 0; i < u.length; i++) {
            char c = u[i];
            if (c >= 'a' && c <= 'z') u[i] = (char) (c - 32);
            else if (c == '_') u[i] = '-';
        }
        String n = new String(u, true);
        switch (n) {
            case "UTF-8": case "UTF8": return CS_UTF8;
            case "ISO-8859-1": case "ISO8859-1": case "LATIN1": case "ISO-LATIN-1": case "8859-1": return CS_LATIN1;
            case "US-ASCII": case "ASCII": return CS_ASCII;
            case "UTF-16BE": return CS_UTF16BE;
            case "UTF-16LE": return CS_UTF16LE;
            case "UTF-16": return CS_UTF16;
            default: return -1;
        }
    }

    private static int charsetCode(Charset cs) {
        if (cs == null) throw new NullPointerException("charset");
        int c = charsetCodeByName(cs.name());
        if (c < 0) throw new UnsupportedOperationException("monolith: charset " + cs.name());
        return c;
    }

    static char[] decode(int cs, byte[] b, int off, int len) {
        switch (cs) {
            case CS_LATIN1: {
                char[] v = new char[len];
                for (int i = 0; i < len; i++) v[i] = (char) (b[off + i] & 0xff);
                return v;
            }
            case CS_ASCII: {
                char[] v = new char[len];
                for (int i = 0; i < len; i++) {
                    int x = b[off + i];
                    v[i] = x >= 0 ? (char) x : '�';
                }
                return v;
            }
            case CS_UTF16BE: case CS_UTF16LE: case CS_UTF16: {
                boolean le = cs == CS_UTF16LE;
                int p = off, end = off + len;
                if (cs == CS_UTF16 && len >= 2) {
                    int b0 = b[p] & 0xff, b1 = b[p + 1] & 0xff;
                    if (b0 == 0xFE && b1 == 0xFF) p += 2;
                    else if (b0 == 0xFF && b1 == 0xFE) { le = true; p += 2; }
                }
                int n = (end - p) / 2;
                boolean odd = ((end - p) & 1) != 0;
                char[] v = new char[n + (odd ? 1 : 0)];
                for (int i = 0; i < n; i++, p += 2) {
                    int hi = b[p] & 0xff, lo = b[p + 1] & 0xff;
                    v[i] = le ? (char) ((lo << 8) | hi) : (char) ((hi << 8) | lo);
                }
                if (odd) v[n] = '�';
                return v;
            }
            default:
                return decodeUtf8(b, off, len);
        }
    }

    private static char[] decodeUtf8(byte[] b, int off, int len) {
        char[] dst = new char[len];
        int dp = 0;
        int sp = off, sl = off + len;
        while (sp < sl) {
            int b1 = b[sp];
            if (b1 >= 0) {
                dst[dp++] = (char) b1;
                sp++;
            } else if ((b1 >> 5) == -2 && (b1 & 0x1e) != 0) {
                if (sp + 1 < sl && (b[sp + 1] & 0xc0) == 0x80) {
                    dst[dp++] = (char) (((b1 << 6) ^ b[sp + 1]) ^ (((byte) 0xC0 << 6) ^ ((byte) 0x80)));
                    sp += 2;
                } else {
                    dst[dp++] = '�';
                    sp++;
                }
            } else if ((b1 >> 4) == -2) {
                if (sp + 2 < sl && (b[sp + 1] & 0xc0) == 0x80 && (b[sp + 2] & 0xc0) == 0x80
                        && !(b1 == (byte) 0xe0 && (b[sp + 1] & 0xe0) == 0x80)) {
                    char c = (char) ((b1 << 12) ^ (b[sp + 1] << 6) ^ (b[sp + 2] ^ (((byte) 0xE0 << 12) ^ ((byte) 0x80 << 6) ^ ((byte) 0x80))));
                    if (Character.isSurrogate(c)) {
                        dst[dp++] = '�';
                    } else {
                        dst[dp++] = c;
                    }
                    sp += 3;
                } else {
                    // malformed: consume the valid prefix as one replacement
                    int n = 1;
                    if (sp + 1 < sl && (b[sp + 1] & 0xc0) == 0x80 && !(b1 == (byte) 0xe0 && (b[sp + 1] & 0xe0) == 0x80)
                            && !(b1 == (byte) 0xed && (b[sp + 1] & 0xe0) == 0xa0)) n = 2;
                    dst[dp++] = '�';
                    sp += n;
                }
            } else if ((b1 >> 3) == -2) {
                int b2 = sp + 1 < sl ? b[sp + 1] : 0, b3 = sp + 2 < sl ? b[sp + 2] : 0, b4 = sp + 3 < sl ? b[sp + 3] : 0;
                int uc = ((b1 << 18) ^ (b2 << 12) ^ (b3 << 6) ^ (b4 ^ (((byte) 0xF0 << 18) ^ ((byte) 0x80 << 12) ^ ((byte) 0x80 << 6) ^ ((byte) 0x80))));
                if (sp + 3 < sl && (b2 & 0xc0) == 0x80 && (b3 & 0xc0) == 0x80 && (b4 & 0xc0) == 0x80
                        && Character.isSupplementaryCodePoint(uc)) {
                    if (dp + 2 > dst.length) dst = java.util.Arrays.copyOf(dst, dst.length + 4);
                    dst[dp++] = Character.highSurrogate(uc);
                    dst[dp++] = Character.lowSurrogate(uc);
                    sp += 4;
                } else {
                    int n = 1;
                    if (sp + 1 < sl && (b2 & 0xc0) == 0x80 && !((b1 & 0xff) == 0xf0 && (b2 & 0xff) < 0x90)
                            && !((b1 & 0xff) == 0xf4 && (b2 & 0xff) >= 0x90) && (b1 & 0xff) <= 0xf4) {
                        n = 2;
                        if (sp + 2 < sl && (b3 & 0xc0) == 0x80) n = 3;
                    }
                    dst[dp++] = '�';
                    sp += n;
                }
            } else {
                dst[dp++] = '�';
                sp++;
            }
        }
        return dp == dst.length ? dst : java.util.Arrays.copyOf(dst, dp);
    }

    static byte[] encode(int cs, char[] v, int off, int len) {
        switch (cs) {
            case CS_LATIN1: case CS_ASCII: {
                int max = cs == CS_LATIN1 ? 0xff : 0x7f;
                int n = 0;
                byte[] out = new byte[len];
                for (int i = off; i < off + len; i++) {
                    char c = v[i];
                    if (c <= max) out[n++] = (byte) c;
                    else {
                        if (Character.isHighSurrogate(c) && i + 1 < off + len && Character.isLowSurrogate(v[i + 1])) i++;
                        out[n++] = '?';
                    }
                }
                return n == len ? out : java.util.Arrays.copyOf(out, n);
            }
            case CS_UTF16BE: case CS_UTF16LE: case CS_UTF16: {
                boolean bom = cs == CS_UTF16;
                byte[] out = new byte[len * 2 + (bom ? 2 : 0)];
                int p = 0;
                if (bom) { out[p++] = (byte) 0xFE; out[p++] = (byte) 0xFF; }
                for (int i = off; i < off + len; i++) {
                    char c = v[i];
                    if (cs == CS_UTF16LE) { out[p++] = (byte) c; out[p++] = (byte) (c >> 8); }
                    else { out[p++] = (byte) (c >> 8); out[p++] = (byte) c; }
                }
                return out;
            }
            default: {
                int n = 0;
                for (int i = off; i < off + len; i++) {
                    char c = v[i];
                    if (c < 0x80) n++;
                    else if (c < 0x800) n += 2;
                    else if (Character.isHighSurrogate(c) && i + 1 < off + len && Character.isLowSurrogate(v[i + 1])) { n += 4; i++; }
                    else if (Character.isSurrogate(c)) n++;
                    else n += 3;
                }
                byte[] out = new byte[n];
                int p = 0;
                for (int i = off; i < off + len; i++) {
                    char c = v[i];
                    if (c < 0x80) out[p++] = (byte) c;
                    else if (c < 0x800) {
                        out[p++] = (byte) (0xc0 | (c >> 6));
                        out[p++] = (byte) (0x80 | (c & 0x3f));
                    } else if (Character.isHighSurrogate(c) && i + 1 < off + len && Character.isLowSurrogate(v[i + 1])) {
                        int uc = Character.toCodePoint(c, v[++i]);
                        out[p++] = (byte) (0xf0 | (uc >> 18));
                        out[p++] = (byte) (0x80 | ((uc >> 12) & 0x3f));
                        out[p++] = (byte) (0x80 | ((uc >> 6) & 0x3f));
                        out[p++] = (byte) (0x80 | (uc & 0x3f));
                    } else if (Character.isSurrogate(c)) {
                        out[p++] = '?';
                    } else {
                        out[p++] = (byte) (0xe0 | (c >> 12));
                        out[p++] = (byte) (0x80 | ((c >> 6) & 0x3f));
                        out[p++] = (byte) (0x80 | (c & 0x3f));
                    }
                }
                return out;
            }
        }
    }

    // ---------------------------------------------------------------- basics
    // Same checks and messages as the JDK (Preconditions with SIOOBE_FORMATTER).
    static void checkBoundsOffCount(int offset, int count, int length) {
        if ((length | offset | count) < 0 || count > length - offset)
            throw new StringIndexOutOfBoundsException("Range [" + offset + ", " + offset + " + " + count
                    + ") out of bounds for length " + length);
    }

    static void checkBoundsBeginEnd(int begin, int end, int length) {
        if (begin < 0 || begin > end || end > length)
            throw new StringIndexOutOfBoundsException("Range [" + begin + ", " + end + ") out of bounds for length " + length);
    }

    static void checkIndex(int index, int length) {
        if (index < 0 || index >= length)
            throw new StringIndexOutOfBoundsException("Index " + index + " out of bounds for length " + length);
    }

    char[] value() {
        return value;
    }

    /** monolith internal (used by java.io.PrintStream): charset code, or -1 if unsupported. */
    public static int monoCharsetCode(String charsetName) {
        return charsetCodeByName(charsetName);
    }

    /** monolith internal: encode with a charset code from {@link #monoCharsetCode}. */
    public static byte[] monoEncode(int cs, String s) {
        return encode(cs, s.value, 0, s.value.length);
    }

    public int length() {
        return value.length;
    }

    public boolean isEmpty() {
        return value.length == 0;
    }

    public char charAt(int index) {
        checkIndex(index, value.length);
        return value[index];
    }

    public int codePointAt(int index) {
        checkIndex(index, value.length);
        return Character.codePointAt(value, index, value.length);
    }

    public int codePointBefore(int index) {
        int i = index - 1;
        checkIndex(i, value.length);
        return Character.codePointBefore(value, index, 0);
    }

    public int codePointCount(int beginIndex, int endIndex) {
        checkBoundsBeginEnd(beginIndex, endIndex, value.length);
        return Character.codePointCount(value, beginIndex, endIndex - beginIndex);
    }

    public int offsetByCodePoints(int index, int codePointOffset) {
        return Character.offsetByCodePoints(this, index, codePointOffset);
    }

    public void getChars(int srcBegin, int srcEnd, char[] dst, int dstBegin) {
        checkBoundsBeginEnd(srcBegin, srcEnd, value.length);
        System.arraycopy(value, srcBegin, dst, dstBegin, srcEnd - srcBegin);
    }

    @Deprecated(since = "1.1")
    public void getBytes(int srcBegin, int srcEnd, byte[] dst, int dstBegin) {
        checkBoundsBeginEnd(srcBegin, srcEnd, value.length);
        for (int i = srcBegin; i < srcEnd; i++) dst[dstBegin++] = (byte) value[i];
    }

    public byte[] getBytes(String charsetName) throws java.io.UnsupportedEncodingException {
        return encode(lookupCharset(charsetName), value, 0, value.length);
    }

    public byte[] getBytes(Charset charset) {
        return encode(charsetCode(charset), value, 0, value.length);
    }

    public byte[] getBytes() {
        return encode(CS_UTF8, value, 0, value.length);
    }

    public boolean equals(Object anObject) {
        if (this == anObject) return true;
        if (anObject instanceof String s) {
            char[] a = value, b = s.value;
            int n = a.length;
            if (n != b.length) return false;
            for (int i = 0; i < n; i++) if (a[i] != b[i]) return false;
            return true;
        }
        return false;
    }

    public boolean contentEquals(StringBuffer sb) {
        return contentEquals((CharSequence) sb);
    }

    public boolean contentEquals(CharSequence cs) {
        int n = value.length;
        if (n != cs.length()) return false;
        for (int i = 0; i < n; i++) if (value[i] != cs.charAt(i)) return false;
        return true;
    }

    public boolean equalsIgnoreCase(String anotherString) {
        return (this == anotherString) ? true
                : (anotherString != null) && (anotherString.value.length == value.length)
                && regionMatches(true, 0, anotherString, 0, value.length);
    }

    public int compareTo(String anotherString) {
        char[] a = value, b = anotherString.value;
        int lim = Math.min(a.length, b.length);
        for (int k = 0; k < lim; k++) {
            if (a[k] != b[k]) return a[k] - b[k];
        }
        return a.length - b.length;
    }

    public int compareToIgnoreCase(String str) {
        char[] a = value, b = str.value;
        int n1 = a.length, n2 = b.length;
        int lim = Math.min(n1, n2);
        for (int k = 0; k < lim; k++) {
            char c1 = a[k], c2 = b[k];
            if (c1 != c2) {
                c1 = Character.toUpperCase(c1);
                c2 = Character.toUpperCase(c2);
                if (c1 != c2) {
                    c1 = Character.toLowerCase(c1);
                    c2 = Character.toLowerCase(c2);
                    if (c1 != c2) return c1 - c2;
                }
            }
        }
        return n1 - n2;
    }

    public boolean regionMatches(int toffset, String other, int ooffset, int len) {
        if ((ooffset < 0) || (toffset < 0) || (toffset > (long) value.length - len)
                || (ooffset > (long) other.value.length - len)) return false;
        for (int i = 0; i < len; i++) if (value[toffset + i] != other.value[ooffset + i]) return false;
        return true;
    }

    public boolean regionMatches(boolean ignoreCase, int toffset, String other, int ooffset, int len) {
        if (!ignoreCase) return regionMatches(toffset, other, ooffset, len);
        if ((ooffset < 0) || (toffset < 0) || (toffset > (long) value.length - len)
                || (ooffset > (long) other.value.length - len)) return false;
        for (int i = 0; i < len; i++) {
            char c1 = value[toffset + i], c2 = other.value[ooffset + i];
            if (c1 == c2) continue;
            char u1 = Character.toUpperCase(c1), u2 = Character.toUpperCase(c2);
            if (u1 == u2) continue;
            if (Character.toLowerCase(u1) == Character.toLowerCase(u2)) continue;
            return false;
        }
        return true;
    }

    public boolean startsWith(String prefix, int toffset) {
        if (toffset < 0 || toffset > value.length - prefix.value.length) return false;
        char[] p = prefix.value;
        for (int i = 0; i < p.length; i++) if (value[toffset + i] != p[i]) return false;
        return true;
    }

    public boolean startsWith(String prefix) {
        return startsWith(prefix, 0);
    }

    public boolean endsWith(String suffix) {
        return startsWith(suffix, value.length - suffix.value.length);
    }

    public int hashCode() {
        int h = hash;
        if (h == 0 && !hashIsZero) {
            for (char c : value) h = 31 * h + c;
            if (h == 0) hashIsZero = true;
            else hash = h;
        }
        return h;
    }

    public int indexOf(int ch) {
        return indexOf(ch, 0);
    }

    public int indexOf(int ch, int fromIndex) {
        char[] v = value;
        int max = v.length;
        if (fromIndex < 0) fromIndex = 0;
        else if (fromIndex >= max) return -1;
        if (ch < Character.MIN_SUPPLEMENTARY_CODE_POINT) {
            for (int i = fromIndex; i < max; i++) if (v[i] == ch) return i;
            return -1;
        }
        if (Character.isValidCodePoint(ch)) {
            char hi = Character.highSurrogate(ch), lo = Character.lowSurrogate(ch);
            for (int i = fromIndex; i < max - 1; i++) if (v[i] == hi && v[i + 1] == lo) return i;
        }
        return -1;
    }

    public int indexOf(int ch, int beginIndex, int endIndex) {
        checkBoundsBeginEnd(beginIndex, endIndex, value.length);
        int r = substring(0, endIndex).indexOf(ch, beginIndex);
        return r;
    }

    public int lastIndexOf(int ch) {
        return lastIndexOf(ch, value.length - 1);
    }

    public int lastIndexOf(int ch, int fromIndex) {
        char[] v = value;
        if (ch < Character.MIN_SUPPLEMENTARY_CODE_POINT) {
            int i = Math.min(fromIndex, v.length - 1);
            for (; i >= 0; i--) if (v[i] == ch) return i;
            return -1;
        }
        if (Character.isValidCodePoint(ch)) {
            char hi = Character.highSurrogate(ch), lo = Character.lowSurrogate(ch);
            int i = Math.min(fromIndex, v.length - 2);
            for (; i >= 0; i--) if (v[i] == hi && v[i + 1] == lo) return i;
        }
        return -1;
    }

    public int indexOf(String str) {
        return indexOf(str, 0);
    }

    public int indexOf(String str, int fromIndex) {
        return indexOf(value, value.length, str.value, fromIndex);
    }

    static int indexOf(char[] src, int srcCount, char[] tgt, int fromIndex) {
        int tgtCount = tgt.length;
        if (fromIndex >= srcCount) return (tgtCount == 0 ? srcCount : -1);
        if (fromIndex < 0) fromIndex = 0;
        if (tgtCount == 0) return fromIndex;
        char first = tgt[0];
        int max = srcCount - tgtCount;
        for (int i = fromIndex; i <= max; i++) {
            if (src[i] != first) {
                while (++i <= max && src[i] != first);
            }
            if (i <= max) {
                int j = i + 1;
                int end = j + tgtCount - 1;
                for (int k = 1; j < end && src[j] == tgt[k]; j++, k++);
                if (j == end) return i;
            }
        }
        return -1;
    }

    public int indexOf(String str, int beginIndex, int endIndex) {
        checkBoundsBeginEnd(beginIndex, endIndex, value.length);
        int r = indexOf(value, endIndex, str.value, beginIndex);
        return (r >= 0 && r + str.value.length <= endIndex) ? r : -1;
    }

    public java.util.stream.IntStream codePoints() {
        return CharSequence.super.codePoints();
    }

    public String[] splitWithDelimiters(String regex, int limit) {
        return java.util.regex.Pattern.compile(regex).splitWithDelimiters(this, limit);
    }

    public String resolveConstantDesc(java.lang.invoke.MethodHandles.Lookup lookup) {
        return this;
    }

    /** package-private JDK helper (used by Throwable/JavaLangAccess callers) */
    static String join(String prefix, String suffix, String delimiter, String[] elements, int size) {
        StringBuilder sb = new StringBuilder(prefix);
        for (int i = 0; i < size; i++) {
            if (i > 0) sb.append(delimiter);
            sb.append(elements[i]);
        }
        return sb.append(suffix).toString();
    }

    public int lastIndexOf(String str) {
        return lastIndexOf(str, value.length);
    }

    public int lastIndexOf(String str, int fromIndex) {
        return lastIndexOf(value, value.length, str.value, fromIndex);
    }

    static int lastIndexOf(char[] src, int srcCount, char[] tgt, int fromIndex) {
        int tgtCount = tgt.length;
        int rightIndex = srcCount - tgtCount;
        if (fromIndex > rightIndex) fromIndex = rightIndex;
        if (fromIndex < 0) return -1;
        if (tgtCount == 0) return fromIndex;
        int strLastIndex = tgtCount - 1;
        char strLastChar = tgt[strLastIndex];
        int min = tgtCount - 1;
        int i = min + fromIndex;
        startSearchForLastChar:
        while (true) {
            while (i >= min && src[i] != strLastChar) i--;
            if (i < min) return -1;
            int j = i - 1;
            int start = j - (tgtCount - 1);
            int k = strLastIndex - 1;
            while (j > start) {
                if (src[j--] != tgt[k--]) {
                    i--;
                    continue startSearchForLastChar;
                }
            }
            return start + 1;
        }
    }

    public String substring(int beginIndex) {
        return substring(beginIndex, value.length);
    }

    public String substring(int beginIndex, int endIndex) {
        int length = value.length;
        checkBoundsBeginEnd(beginIndex, endIndex, length);
        if (beginIndex == 0 && endIndex == length) return this;
        return new String(value, beginIndex, endIndex - beginIndex);
    }

    public CharSequence subSequence(int beginIndex, int endIndex) {
        return this.substring(beginIndex, endIndex);
    }

    public String concat(String str) {
        if (str.isEmpty()) return this;
        char[] v = new char[value.length + str.value.length];
        System.arraycopy(value, 0, v, 0, value.length);
        System.arraycopy(str.value, 0, v, value.length, str.value.length);
        return new String(v, true);
    }

    public String replace(char oldChar, char newChar) {
        if (oldChar != newChar) {
            char[] v = value;
            for (int i = 0; i < v.length; i++) {
                if (v[i] == oldChar) {
                    char[] n = v.clone();
                    for (int j = i; j < n.length; j++) if (n[j] == oldChar) n[j] = newChar;
                    return new String(n, true);
                }
            }
        }
        return this;
    }

    public boolean matches(String regex) {
        return java.util.regex.Pattern.matches(regex, this);
    }

    public boolean contains(CharSequence s) {
        return indexOf(s.toString()) >= 0;
    }

    public String replaceFirst(String regex, String replacement) {
        return java.util.regex.Pattern.compile(regex).matcher(this).replaceFirst(replacement);
    }

    public String replaceAll(String regex, String replacement) {
        return java.util.regex.Pattern.compile(regex).matcher(this).replaceAll(replacement);
    }

    public String replace(CharSequence target, CharSequence replacement) {
        String trgtStr = target.toString();
        String replStr = replacement.toString();
        int j = indexOf(trgtStr);
        if (j < 0) return this;
        int tl = trgtStr.length();
        StringBuilder sb = new StringBuilder();
        int i = 0;
        if (tl == 0) {
            for (; i < value.length; i++) {
                sb.append(replStr).append(value[i]);
            }
            sb.append(replStr);
            return sb.toString();
        }
        do {
            sb.append(value, i, j - i).append(replStr);
            i = j + tl;
        } while (j < value.length && (j = indexOf(trgtStr, i)) >= 0);
        sb.append(value, i, value.length - i);
        return sb.toString();
    }

    public String[] split(String regex, int limit) {
        char ch;
        if (((regex.length() == 1 && ".$|()[{^?*+\\".indexOf(ch = regex.charAt(0)) == -1)
                || (regex.length() == 2 && regex.charAt(0) == '\\' && ((ch = regex.charAt(1)) - '0' | '9' - ch) < 0
                && ((ch - 'a') | ('z' - ch)) < 0 && ((ch - 'A') | ('Z' - ch)) < 0))
                && (ch < Character.MIN_HIGH_SURROGATE || ch > Character.MAX_LOW_SURROGATE)) {
            int off = 0;
            int next;
            boolean limited = limit > 0;
            ArrayList<String> list = new ArrayList<>();
            while ((next = indexOf(ch, off)) != -1) {
                if (!limited || list.size() < limit - 1) {
                    list.add(substring(off, next));
                    off = next + 1;
                } else {
                    list.add(substring(off, value.length));
                    off = value.length;
                    break;
                }
            }
            if (off == 0) return new String[]{this};
            if (!limited || list.size() < limit) list.add(substring(off, value.length));
            int resultSize = list.size();
            if (limit == 0) {
                while (resultSize > 0 && list.get(resultSize - 1).isEmpty()) resultSize--;
            }
            String[] result = new String[resultSize];
            return list.subList(0, resultSize).toArray(result);
        }
        return java.util.regex.Pattern.compile(regex).split(this, limit);
    }

    public String[] split(String regex) {
        return split(regex, 0);
    }

    public static String join(CharSequence delimiter, CharSequence... elements) {
        StringJoiner j = new StringJoiner(delimiter);
        for (CharSequence cs : elements) j.add(cs);
        return j.toString();
    }

    public static String join(CharSequence delimiter, Iterable<? extends CharSequence> elements) {
        StringJoiner j = new StringJoiner(delimiter);
        for (CharSequence cs : elements) j.add(cs);
        return j.toString();
    }

    public String toLowerCase(Locale locale) {
        return toLowerCase();
    }

    public String toLowerCase() {
        char[] v = value;
        int i = 0;
        for (; i < v.length; i++) {
            char c = v[i];
            if (Character.isSurrogate(c) || Character.toLowerCase(c) != c || c == 'İ') break;
        }
        if (i == v.length) return this;
        StringBuilder sb = new StringBuilder(v.length);
        sb.append(v, 0, i);
        for (; i < v.length; ) {
            int cp = Character.codePointAt(v, i);
            int n = Character.charCount(cp);
            if (cp == 'İ') sb.append("i̇");
            else if (cp == 'Σ') sb.append(isFinalSigma(v, i) ? 'ς' : 'σ');
            else sb.appendCodePoint(Character.toLowerCase(cp));
            i += n;
        }
        return sb.toString();
    }

    private static boolean isFinalSigma(char[] v, int i) {
        boolean before = false;
        for (int j = i - 1; j >= 0; j--) {
            char c = v[j];
            if (Character.isLetter(c)) { before = true; break; }
            if (!Character.isSpaceChar(c) && Character.getType(c) != Character.NON_SPACING_MARK) break;
            if (Character.isSpaceChar(c)) return false;
        }
        if (!before) return false;
        for (int j = i + 1; j < v.length; j++) {
            char c = v[j];
            if (Character.isLetter(c)) return false;
            if (Character.getType(c) != Character.NON_SPACING_MARK) break;
        }
        return true;
    }

    public String toUpperCase(Locale locale) {
        return toUpperCase();
    }

    public String toUpperCase() {
        char[] v = value;
        int i = 0;
        for (; i < v.length; i++) {
            char c = v[i];
            if (Character.isSurrogate(c) || Character.toUpperCase(c) != c || c == 'ß') break;
        }
        if (i == v.length) return this;
        StringBuilder sb = new StringBuilder(v.length);
        sb.append(v, 0, i);
        for (; i < v.length; ) {
            int cp = Character.codePointAt(v, i);
            int n = Character.charCount(cp);
            if (cp == 'ß') sb.append("SS");
            else sb.appendCodePoint(Character.toUpperCase(cp));
            i += n;
        }
        return sb.toString();
    }

    public String trim() {
        int len = value.length;
        int st = 0;
        char[] val = value;
        while ((st < len) && (val[st] <= ' ')) st++;
        while ((st < len) && (val[len - 1] <= ' ')) len--;
        return ((st > 0) || (len < value.length)) ? substring(st, len) : this;
    }

    public String strip() {
        int l = leading(), r = trailing();
        return (l == 0 && r == value.length) ? this : (l >= r ? "" : substring(l, r));
    }

    public String stripLeading() {
        int l = leading();
        return l == 0 ? this : substring(l);
    }

    public String stripTrailing() {
        int r = trailing();
        return r == value.length ? this : substring(0, r);
    }

    private int leading() {
        int i = 0;
        while (i < value.length) {
            int cp = Character.codePointAt(value, i);
            if (!Character.isWhitespace(cp)) break;
            i += Character.charCount(cp);
        }
        return i;
    }

    private int trailing() {
        int i = value.length;
        while (i > 0) {
            int cp = Character.codePointBefore(value, i);
            if (!Character.isWhitespace(cp)) break;
            i -= Character.charCount(cp);
        }
        return i;
    }

    public boolean isBlank() {
        return leading() == value.length;
    }

    public String repeat(int count) {
        if (count < 0) throw new IllegalArgumentException("count is negative: " + count);
        if (count == 1) return this;
        int len = value.length;
        if (len == 0 || count == 0) return "";
        if (Integer.MAX_VALUE / count < len)
            throw new OutOfMemoryError("Required length exceeds implementation limit");
        char[] v = new char[len * count];
        for (int i = 0; i < count; i++) System.arraycopy(value, 0, v, i * len, len);
        return new String(v, true);
    }

    public String indent(int n) {
        if (isEmpty()) return "";
        StringBuilder sb = new StringBuilder();
        for (String line : lines0()) {
            if (n > 0) sb.append(" ".repeat(n)).append(line);
            else if (n < 0) {
                int k = 0;
                while (k < -n && k < line.length() && Character.isWhitespace(line.charAt(k))) k++;
                sb.append(line, k, line.length());
            } else sb.append(line);
            sb.append('\n');
        }
        return sb.toString();
    }

    private List<String> lines0() {
        ArrayList<String> out = new ArrayList<>();
        int start = 0, i = 0, n = value.length;
        while (i < n) {
            char c = value[i];
            if (c == '\n' || c == '\r') {
                out.add(substring(start, i));
                if (c == '\r' && i + 1 < n && value[i + 1] == '\n') i++;
                start = i + 1;
            }
            i++;
        }
        if (start < n) out.add(substring(start, n));
        return out;
    }

    public java.util.stream.Stream<String> lines() {
        return lines0().stream();
    }

    public String toString() {
        return this;
    }

    public char[] toCharArray() {
        return value.clone();
    }

    public static String format(String format, Object... args) {
        return monoFormat(format, args);
    }

    public static String format(Locale l, String format, Object... args) {
        return monoFormat(format, args);
    }

    public String formatted(Object... args) {
        return monoFormat(this, args);
    }

    private static String monoFormat(String fmt, Object... args) {
        if (fmt == null) throw new NullPointerException();
        StringBuilder out = new StringBuilder(fmt.length() + 32);
        int seqIdx = 0;
        int lastIdx = 0;
        int n = fmt.length();
        int i = 0;
        while (i < n) {
            char ch = fmt.charAt(i++);
            if (ch != '%') {
                out.append(ch);
                continue;
            }
            if (i >= n) throw new IllegalArgumentException("Format specifier '%'");
            int argIdx = -1;
            boolean reuseLast = false;
            // check for [arg_index$] or '<'
            int save = i;
            if (fmt.charAt(i) == '<') {
                reuseLast = true;
                i++;
            } else {
                int num = 0;
                int digits = 0;
                while (i < n && fmt.charAt(i) >= '0' && fmt.charAt(i) <= '9') {
                    num = num * 10 + (fmt.charAt(i) - '0');
                    digits++;
                    i++;
                }
                if (digits > 0 && i < n && fmt.charAt(i) == '$') {
                    argIdx = num - 1;
                    i++;
                } else {
                    i = save;
                }
            }
            boolean left = false, plus = false, space = false, zero = false, group = false, paren = false, alt = false;
            while (i < n) {
                char f = fmt.charAt(i);
                if (f == '-') left = true;
                else if (f == '+') plus = true;
                else if (f == ' ') space = true;
                else if (f == '0') zero = true;
                else if (f == ',') group = true;
                else if (f == '(') paren = true;
                else if (f == '#') alt = true;
                else if (f == '<') reuseLast = true;
                else break;
                i++;
            }
            int width = -1;
            if (i < n && fmt.charAt(i) >= '0' && fmt.charAt(i) <= '9') {
                width = 0;
                while (i < n && fmt.charAt(i) >= '0' && fmt.charAt(i) <= '9') {
                    width = width * 10 + (fmt.charAt(i++) - '0');
                }
            }
            int prec = -1;
            if (i < n && fmt.charAt(i) == '.') {
                i++;
                prec = 0;
                while (i < n && fmt.charAt(i) >= '0' && fmt.charAt(i) <= '9') {
                    prec = prec * 10 + (fmt.charAt(i++) - '0');
                }
            }
            if (i >= n) throw new IllegalArgumentException("Format specifier '%'");
            char conv = fmt.charAt(i++);
            if (conv == '%') {
                out.append('%');
                continue;
            }
            if (conv == 'n') {
                out.append('\n');
                continue;
            }
            int useIdx;
            if (argIdx >= 0) {
                useIdx = argIdx;
                lastIdx = useIdx;
            } else if (reuseLast) {
                useIdx = lastIdx;
            } else {
                useIdx = seqIdx++;
                lastIdx = useIdx;
            }
            Object arg = (args != null && useIdx < args.length) ? args[useIdx] : null;
            String rendered = monoRenderSpec(conv, arg, left, plus, space, zero, group, paren, alt, width, prec);
            out.append(rendered);
        }
        return out.toString();
    }

    private static String monoRenderSpec(char conv, Object arg, boolean left, boolean plus, boolean space,
                                        boolean zero, boolean group, boolean paren, boolean alt, int width, int prec) {
        switch (conv) {
            case 's':
            case 'S': {
                String s = (arg == null) ? "null" : arg.toString();
                if (prec >= 0 && prec < s.length()) s = s.substring(0, prec);
                if (conv == 'S') s = s.toUpperCase();
                return monoPad(s, width, left, ' ');
            }
            case 'b':
            case 'B': {
                String s = (arg == null) ? "false" : (arg instanceof Boolean b ? b.toString() : "true");
                if (prec >= 0 && prec < s.length()) s = s.substring(0, prec);
                if (conv == 'B') s = s.toUpperCase();
                return monoPad(s, width, left, ' ');
            }
            case 'c':
            case 'C': {
                char c;
                if (arg instanceof Character ch) c = ch.charValue();
                else if (arg instanceof Number num) c = (char) num.intValue();
                else c = String.valueOf(arg).charAt(0);
                String s = String.valueOf(c);
                if (conv == 'C') s = s.toUpperCase();
                return monoPad(s, width, left, ' ');
            }
            case 'h':
            case 'H': {
                String s = (arg == null) ? "null" : Integer.toHexString(arg.hashCode());
                if (prec >= 0 && prec < s.length()) s = s.substring(0, prec);
                if (conv == 'H') s = s.toUpperCase();
                return monoPad(s, width, left, ' ');
            }
            case 'd': {
                long v = (arg instanceof Number num) ? num.longValue() : 0L;
                boolean neg = v < 0;
                String digits = (v == Long.MIN_VALUE) ? "9223372036854775808" : Long.toString(neg ? -v : v);
                if (group) digits = monoGroup(digits);
                String prefix = "";
                String suffix = "";
                if (neg) {
                    if (paren) { prefix = "("; suffix = ")"; }
                    else prefix = "-";
                } else if (plus) {
                    prefix = "+";
                } else if (space) {
                    prefix = " ";
                }
                return monoPadNumeric(prefix, digits, suffix, width, left, zero);
            }
            case 'x':
            case 'X':
            case 'o': {
                long u;
                if (arg instanceof Byte b) u = b.byteValue() & 0xffL;
                else if (arg instanceof Short s) u = s.shortValue() & 0xffffL;
                else if (arg instanceof Integer in) u = in.intValue() & 0xffffffffL;
                else if (arg instanceof Long l) u = l.longValue();
                else u = 0L;
                String digits = (conv == 'o') ? Long.toOctalString(u) : Long.toHexString(u);
                if (conv == 'X') digits = digits.toUpperCase();
                String prefix = alt ? (conv == 'o' ? "0" : (conv == 'X' ? "0X" : "0x")) : "";
                return monoPadNumeric(prefix, digits, "", width, left, zero);
            }
            case 'f':
            case 'e':
            case 'E':
            case 'g':
            case 'G': {
                double d = (arg instanceof Number num) ? num.doubleValue() : 0.0;
                if (Double.isNaN(d)) return monoPad("NaN", width, left, ' ');
                boolean neg = (Double.doubleToRawLongBits(d) & 0x8000000000000000L) != 0;
                double abs = Math.abs(d);
                if (Double.isInfinite(abs)) {
                    String p = neg ? (paren ? "(" : "-") : (plus ? "+" : (space ? " " : ""));
                    String s = (neg && paren) ? ")" : "";
                    return monoPad(p + (conv == 'E' || conv == 'G' ? "INFINITY" : "Infinity") + s, width, left, ' ');
                }
                int p = (prec < 0) ? 6 : prec;
                String body = monoFormatFloat(conv, abs, p, group);
                String prefix = "";
                String suffix = "";
                if (neg) {
                    if (paren) { prefix = "("; suffix = ")"; }
                    else prefix = "-";
                } else if (plus) {
                    prefix = "+";
                } else if (space) {
                    prefix = " ";
                }
                return monoPadNumeric(prefix, body, suffix, width, left, zero);
            }
            default:
                return String.valueOf(arg);
        }
    }

    private static String monoFormatFloat(char conv, double abs, int prec, boolean group) {
        if (abs == 0.0) {
            if (conv == 'f') {
                return prec == 0 ? "0" : "0." + monoZeros(prec);
            }
            if (conv == 'e' || conv == 'E') {
                return (prec == 0 ? "0" : "0." + monoZeros(prec)) + (conv == 'E' ? "E+00" : "e+00");
            }
            int sig = (prec == 0) ? 1 : prec;
            return sig <= 1 ? "0" : "0." + monoZeros(sig - 1);
        }
        String ds = Double.toString(abs);
        int ePos = ds.indexOf('E');
        if (ePos < 0) ePos = ds.indexOf('e');
        int exp10 = (ePos >= 0) ? Integer.parseInt(ds.substring(ePos + 1)) : 0;
        String mant = (ePos >= 0) ? ds.substring(0, ePos) : ds;
        int dot = mant.indexOf('.');
        String raw = (dot >= 0) ? (mant.substring(0, dot) + mant.substring(dot + 1)) : mant;
        int intLen = (dot >= 0) ? dot : mant.length();
        int firstNonZero = 0;
        while (firstNonZero < raw.length() && raw.charAt(firstNonZero) == '0') firstNonZero++;
        String sigDigits = raw.substring(firstNonZero);
        int exp = intLen - firstNonZero + exp10; // value = 0.<sigDigits> * 10^exp

        if (conv == 'f') {
            return monoFormatFixed(sigDigits, exp, prec, group);
        }
        if (conv == 'e' || conv == 'E') {
            return monoFormatSci(sigDigits, exp, prec, conv == 'E');
        }
        int sig = (prec == 0) ? 1 : prec;
        int[] carry = new int[1];
        String rounded = monoRoundSig(sigDigits, sig, carry);
        int rExp = exp + carry[0];
        int sciExp = rExp - 1;
        if (sciExp >= -4 && sciExp < sig) {
            int frac = Math.max(0, sig - 1 - sciExp);
            return monoFormatFixed(rounded, rExp, frac, group);
        }
        return monoFormatSci(rounded, rExp, Math.max(0, sig - 1), conv == 'G');
    }

    private static String monoRoundSig(String sigDigits, int keep, int[] carryOut) {
        if (keep <= 0) {
            if (!sigDigits.isEmpty() && sigDigits.charAt(0) >= '5') {
                carryOut[0] = 1;
                return "1";
            }
            return "";
        }
        char[] buf = new char[keep];
        int slen = sigDigits.length();
        for (int i = 0; i < keep; i++) {
            buf[i] = (i < slen) ? sigDigits.charAt(i) : '0';
        }
        if (keep < slen && sigDigits.charAt(keep) >= '5') {
            int k = keep - 1;
            while (k >= 0 && buf[k] == '9') {
                buf[k] = '0';
                k--;
            }
            if (k >= 0) {
                buf[k]++;
            } else {
                carryOut[0] = 1;
                buf[0] = '1';
            }
        }
        return new String(buf, true);
    }

    private static String monoFormatFixed(String sigDigits, int exp, int prec, boolean group) {
        int keep = exp + prec;
        if (keep < 0) {
            return prec == 0 ? "0" : "0." + monoZeros(prec);
        }
        int[] carry = new int[1];
        String r = monoRoundSig(sigDigits, keep, carry);
        if (carry[0] != 0) {
            // r is "1" at the new leading position when nothing was kept (keep == 0), else all zeros plus a lead 1
            exp += 1;
            if (keep != 0) r = r + "0";
        }
        String intPart;
        String fracPart;
        if (exp <= 0) {
            intPart = "0";
            StringBuilder fb = new StringBuilder(prec);
            for (int i = 0; i < -exp && fb.length() < prec; i++) fb.append('0');
            for (int i = 0; i < r.length() && fb.length() < prec; i++) fb.append(r.charAt(i));
            while (fb.length() < prec) fb.append('0');
            fracPart = fb.toString();
        } else {
            StringBuilder ib = new StringBuilder(exp);
            for (int i = 0; i < exp; i++) ib.append(i < r.length() ? r.charAt(i) : '0');
            intPart = ib.toString();
            StringBuilder fb = new StringBuilder(prec);
            for (int i = 0; i < prec; i++) {
                int idx = exp + i;
                fb.append(idx < r.length() ? r.charAt(idx) : '0');
            }
            fracPart = fb.toString();
        }
        if (group) intPart = monoGroup(intPart);
        return prec == 0 ? intPart : (intPart + "." + fracPart);
    }

    private static String monoFormatSci(String sigDigits, int exp, int prec, boolean upper) {
        int[] carry = new int[1];
        String r = monoRoundSig(sigDigits, prec + 1, carry);
        int sciExp = exp + carry[0] - 1;
        StringBuilder sb = new StringBuilder(prec + 8);
        sb.append(r.isEmpty() ? '0' : r.charAt(0));
        if (prec > 0) {
            sb.append('.');
            for (int i = 1; i <= prec; i++) sb.append(i < r.length() ? r.charAt(i) : '0');
        }
        sb.append(upper ? 'E' : 'e');
        sb.append(sciExp >= 0 ? '+' : '-');
        int aexp = Math.abs(sciExp);
        if (aexp < 10) sb.append('0');
        sb.append(aexp);
        return sb.toString();
    }

    private static String monoZeros(int count) {
        char[] z = new char[count];
        for (int i = 0; i < count; i++) z[i] = '0';
        return new String(z, true);
    }

    private static String monoGroup(String digits) {
        int len = digits.length();
        if (len <= 3) return digits;
        StringBuilder sb = new StringBuilder(len + len / 3);
        int rem = len % 3;
        if (rem > 0) sb.append(digits, 0, rem);
        for (int i = rem; i < len; i += 3) {
            if (sb.length() > 0) sb.append(',');
            sb.append(digits, i, i + 3);
        }
        return sb.toString();
    }

    private static String monoPad(String s, int width, boolean left, char pad) {
        if (width <= s.length()) return s;
        StringBuilder sb = new StringBuilder(width);
        int diff = width - s.length();
        if (left) {
            sb.append(s);
            for (int i = 0; i < diff; i++) sb.append(' ');
        } else {
            for (int i = 0; i < diff; i++) sb.append(pad);
            sb.append(s);
        }
        return sb.toString();
    }

    private static String monoPadNumeric(String prefix, String body, String suffix, int width, boolean left, boolean zero) {
        int len = prefix.length() + body.length() + suffix.length();
        if (width <= len) return prefix + body + suffix;
        int diff = width - len;
        StringBuilder sb = new StringBuilder(width);
        if (left) {
            sb.append(prefix).append(body).append(suffix);
            for (int i = 0; i < diff; i++) sb.append(' ');
        } else if (zero) {
            sb.append(prefix);
            for (int i = 0; i < diff; i++) sb.append('0');
            sb.append(body).append(suffix);
        } else {
            for (int i = 0; i < diff; i++) sb.append(' ');
            sb.append(prefix).append(body).append(suffix);
        }
        return sb.toString();
    }

    public static String valueOf(Object obj) {
        return (obj == null) ? "null" : obj.toString();
    }

    public static String valueOf(char[] data) {
        return new String(data);
    }

    public static String valueOf(char[] data, int offset, int count) {
        return new String(data, offset, count);
    }

    public static String copyValueOf(char[] data, int offset, int count) {
        return new String(data, offset, count);
    }

    public static String copyValueOf(char[] data) {
        return new String(data);
    }

    public static String valueOf(boolean b) {
        return b ? "true" : "false";
    }

    public static String valueOf(char c) {
        return new String(new char[]{c}, true);
    }

    static String valueOfCodePoint(int codePoint) {
        if (Character.isBmpCodePoint(codePoint)) return valueOf((char) codePoint);
        if (Character.isValidCodePoint(codePoint))
            return new String(new char[]{Character.highSurrogate(codePoint), Character.lowSurrogate(codePoint)}, true);
        throw new IllegalArgumentException("Not a valid Unicode code point: 0x" + Integer.toHexString(codePoint).toUpperCase());
    }

    public static String valueOf(int i) {
        return Integer.toString(i);
    }

    public static String valueOf(long l) {
        return Long.toString(l);
    }

    public static String valueOf(float f) {
        return Float.toString(f);
    }

    public static String valueOf(double d) {
        return Double.toString(d);
    }

    public native String intern();

    public java.util.stream.IntStream chars() {
        return CharSequence.super.chars();
    }

    public <R> R transform(java.util.function.Function<? super String, ? extends R> f) {
        return f.apply(this);
    }

    public String translateEscapes() {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < value.length; i++) {
            char c = value[i];
            if (c != '\\') { sb.append(c); continue; }
            if (++i >= value.length) throw new IllegalArgumentException("Invalid escape sequence");
            c = value[i];
            switch (c) {
                case 'b': sb.append('\b'); break;
                case 'f': sb.append('\f'); break;
                case 'n': sb.append('\n'); break;
                case 'r': sb.append('\r'); break;
                case 's': sb.append(' '); break;
                case 't': sb.append('\t'); break;
                case '\'': case '\"': case '\\': sb.append(c); break;
                case '\n': break;
                case '\r': if (i + 1 < value.length && value[i + 1] == '\n') i++; break;
                default:
                    if (c >= '0' && c <= '7') {
                        int code = c - '0';
                        int limit = Integer.min(i + (c <= '3' ? 2 : 1), value.length - 1);
                        while (i < limit && value[i + 1] >= '0' && value[i + 1] <= '7') code = code * 8 + (value[++i] - '0');
                        sb.append((char) code);
                    } else throw new IllegalArgumentException("Invalid escape sequence: \\" + c);
            }
        }
        return sb.toString();
    }

    public String stripIndent() {
        return this;
    }

    public java.util.Optional<String> describeConstable() {
        return java.util.Optional.of(this);
    }
}
