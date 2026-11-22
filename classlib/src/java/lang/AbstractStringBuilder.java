package java.lang;

import java.util.Arrays;

/** j2zig overlay: growable UTF-16 buffer shared by StringBuilder and StringBuffer. */
abstract sealed class AbstractStringBuilder implements Appendable, CharSequence permits StringBuilder, StringBuffer {
    char[] value;
    int count;

    AbstractStringBuilder() {
        value = new char[16];
    }

    AbstractStringBuilder(int capacity) {
        if (capacity < 0) throw new NegativeArraySizeException(Integer.toString(capacity));
        value = new char[capacity];
    }

    AbstractStringBuilder(String str) {
        int length = str.length();
        int capacity = (length < Integer.MAX_VALUE - 16) ? length + 16 : Integer.MAX_VALUE;
        value = new char[capacity];
        append(str);
    }

    AbstractStringBuilder(CharSequence seq) {
        int length = seq.length();
        if (length < 0) throw new NegativeArraySizeException("Negative length: " + length);
        int capacity = (length < Integer.MAX_VALUE - 16) ? length + 16 : Integer.MAX_VALUE;
        value = new char[capacity];
        append(seq);
    }

    public int length() {
        return count;
    }

    public int capacity() {
        return value.length;
    }

    public void ensureCapacity(int minimumCapacity) {
        if (minimumCapacity > 0) ensureCapacityInternal(minimumCapacity);
    }

    private void ensureCapacityInternal(int minimumCapacity) {
        int oldCapacity = value.length;
        if (minimumCapacity - oldCapacity > 0) {
            int newCapacity = Math.max(minimumCapacity, (oldCapacity << 1) + 2);
            if (newCapacity < 0) {
                if (minimumCapacity < 0) throw new OutOfMemoryError("Required length exceeds implementation limit");
                newCapacity = Integer.MAX_VALUE - 8;
            }
            value = Arrays.copyOf(value, newCapacity);
        }
    }

    public void trimToSize() {
        if (count < value.length) value = Arrays.copyOf(value, count);
    }

    public void setLength(int newLength) {
        if (newLength < 0) throw new StringIndexOutOfBoundsException(newLength);
        ensureCapacityInternal(newLength);
        if (count < newLength) Arrays.fill(value, count, newLength, '\0');
        count = newLength;
    }

    public char charAt(int index) {
        String.checkIndex(index, count);
        return value[index];
    }

    public int codePointAt(int index) {
        String.checkIndex(index, count);
        return Character.codePointAt(value, index, count);
    }

    public int codePointBefore(int index) {
        int i = index - 1;
        String.checkIndex(i, count);
        return Character.codePointBefore(value, index, 0);
    }

    public int codePointCount(int beginIndex, int endIndex) {
        String.checkBoundsBeginEnd(beginIndex, endIndex, count);
        return Character.codePointCount(value, beginIndex, endIndex - beginIndex);
    }

    public int offsetByCodePoints(int index, int codePointOffset) {
        if (index < 0 || index > count) throw new IndexOutOfBoundsException();
        return Character.offsetByCodePoints(value, 0, count, index, codePointOffset);
    }

    public void getChars(int srcBegin, int srcEnd, char[] dst, int dstBegin) {
        String.checkBoundsBeginEnd(srcBegin, srcEnd, count);
        System.arraycopy(value, srcBegin, dst, dstBegin, srcEnd - srcBegin);
    }

    public void setCharAt(int index, char ch) {
        String.checkIndex(index, count);
        value[index] = ch;
    }

    public AbstractStringBuilder append(Object obj) {
        return append(String.valueOf(obj));
    }

    public AbstractStringBuilder append(String str) {
        if (str == null) str = "null";
        int len = str.length();
        ensureCapacityInternal(count + len);
        str.getChars(0, len, value, count);
        count += len;
        return this;
    }

    public AbstractStringBuilder append(StringBuffer sb) {
        return append((CharSequence) sb);
    }

    AbstractStringBuilder append(AbstractStringBuilder asb) {
        if (asb == null) return append("null");
        int len = asb.length();
        ensureCapacityInternal(count + len);
        System.arraycopy(asb.value, 0, value, count, len);
        count += len;
        return this;
    }

    public AbstractStringBuilder append(CharSequence s) {
        if (s == null) return append("null");
        if (s instanceof String str) return append(str);
        if (s instanceof AbstractStringBuilder asb) return append(asb);
        return this.append(s, 0, s.length());
    }

    public AbstractStringBuilder append(CharSequence s, int start, int end) {
        if (s == null) s = "null";
        String.checkBoundsBeginEnd(start, end, s.length());
        int len = end - start;
        ensureCapacityInternal(count + len);
        for (int i = start, j = count; i < end; i++, j++) value[j] = s.charAt(i);
        count += len;
        return this;
    }

    public AbstractStringBuilder append(char[] str) {
        int len = str.length;
        ensureCapacityInternal(count + len);
        System.arraycopy(str, 0, value, count, len);
        count += len;
        return this;
    }

    public AbstractStringBuilder append(char[] str, int offset, int len) {
        String.checkBoundsOffCount(offset, len, str.length);
        ensureCapacityInternal(count + len);
        System.arraycopy(str, offset, value, count, len);
        count += len;
        return this;
    }

    public AbstractStringBuilder append(boolean b) {
        return append(b ? "true" : "false");
    }

    public AbstractStringBuilder append(char c) {
        ensureCapacityInternal(count + 1);
        value[count++] = c;
        return this;
    }

    public AbstractStringBuilder append(int i) {
        return append(Integer.toString(i));
    }

    public AbstractStringBuilder append(long l) {
        return append(Long.toString(l));
    }

    public AbstractStringBuilder append(float f) {
        return append(Float.toString(f));
    }

    public AbstractStringBuilder append(double d) {
        return append(Double.toString(d));
    }

    public AbstractStringBuilder delete(int start, int end) {
        int count = this.count;
        if (end > count) end = count;
        String.checkBoundsBeginEnd(start, end, count);
        int len = end - start;
        if (len > 0) {
            System.arraycopy(value, start + len, value, start, count - end);
            this.count = count - len;
        }
        return this;
    }

    public AbstractStringBuilder appendCodePoint(int codePoint) {
        if (Character.isBmpCodePoint(codePoint)) return append((char) codePoint);
        if (!Character.isValidCodePoint(codePoint))
            throw new IllegalArgumentException("Not a valid Unicode code point: 0x" + Integer.toHexString(codePoint).toUpperCase());
        append(Character.highSurrogate(codePoint));
        return append(Character.lowSurrogate(codePoint));
    }

    public AbstractStringBuilder deleteCharAt(int index) {
        String.checkIndex(index, count);
        System.arraycopy(value, index + 1, value, index, count - index - 1);
        count--;
        return this;
    }

    public AbstractStringBuilder replace(int start, int end, String str) {
        int count = this.count;
        if (end > count) end = count;
        String.checkBoundsBeginEnd(start, end, count);
        int len = str.length();
        int newCount = count + len - (end - start);
        ensureCapacityInternal(newCount);
        System.arraycopy(value, end, value, start + len, count - end);
        str.getChars(0, len, value, start);
        this.count = newCount;
        return this;
    }

    public String substring(int start) {
        return substring(start, count);
    }

    public CharSequence subSequence(int start, int end) {
        return substring(start, end);
    }

    public String substring(int start, int end) {
        String.checkBoundsBeginEnd(start, end, count);
        return new String(value, start, end - start);
    }

    public AbstractStringBuilder insert(int index, char[] str, int offset, int len) {
        String.checkBoundsOffCount(index, 0, count);
        String.checkBoundsOffCount(offset, len, str.length);
        ensureCapacityInternal(count + len);
        System.arraycopy(value, index, value, index + len, count - index);
        System.arraycopy(str, offset, value, index, len);
        count += len;
        return this;
    }

    public AbstractStringBuilder insert(int offset, Object obj) {
        return insert(offset, String.valueOf(obj));
    }

    public AbstractStringBuilder insert(int offset, String str) {
        String.checkBoundsOffCount(offset, 0, count);
        if (str == null) str = "null";
        int len = str.length();
        ensureCapacityInternal(count + len);
        System.arraycopy(value, offset, value, offset + len, count - offset);
        str.getChars(0, len, value, offset);
        count += len;
        return this;
    }

    public AbstractStringBuilder insert(int offset, char[] str) {
        return insert(offset, str, 0, str.length);
    }

    public AbstractStringBuilder insert(int dstOffset, CharSequence s) {
        return insert(dstOffset, String.valueOf(s));
    }

    public AbstractStringBuilder insert(int dstOffset, CharSequence s, int start, int end) {
        if (s == null) s = "null";
        return insert(dstOffset, s.subSequence(start, end).toString());
    }

    public AbstractStringBuilder insert(int offset, boolean b) {
        return insert(offset, String.valueOf(b));
    }

    public AbstractStringBuilder insert(int offset, char c) {
        String.checkBoundsOffCount(offset, 0, count);
        ensureCapacityInternal(count + 1);
        System.arraycopy(value, offset, value, offset + 1, count - offset);
        value[offset] = c;
        count += 1;
        return this;
    }

    public AbstractStringBuilder insert(int offset, int i) {
        return insert(offset, String.valueOf(i));
    }

    public AbstractStringBuilder insert(int offset, long l) {
        return insert(offset, String.valueOf(l));
    }

    public AbstractStringBuilder insert(int offset, float f) {
        return insert(offset, String.valueOf(f));
    }

    public AbstractStringBuilder insert(int offset, double d) {
        return insert(offset, String.valueOf(d));
    }

    public int indexOf(String str) {
        return indexOf(str, 0);
    }

    public int indexOf(String str, int fromIndex) {
        return String.indexOf(value, count, str.value(), fromIndex);
    }

    public int lastIndexOf(String str) {
        return lastIndexOf(str, count);
    }

    public int lastIndexOf(String str, int fromIndex) {
        return String.lastIndexOf(value, count, str.value(), fromIndex);
    }

    public AbstractStringBuilder reverse() {
        int n = count - 1;
        boolean hasSurrogates = false;
        for (int j = (n - 1) >> 1; j >= 0; j--) {
            int k = n - j;
            char cj = value[j];
            char ck = value[k];
            value[j] = ck;
            value[k] = cj;
            if (Character.isSurrogate(cj) || Character.isSurrogate(ck)) hasSurrogates = true;
        }
        if (hasSurrogates) {
            for (int i = 0; i < n; i++) {
                char c2 = value[i];
                if (Character.isLowSurrogate(c2)) {
                    char c1 = value[i + 1];
                    if (Character.isHighSurrogate(c1)) {
                        value[i++] = c1;
                        value[i] = c2;
                    }
                }
            }
        }
        return this;
    }

    public AbstractStringBuilder repeat(int codePoint, int count) {
        if (count < 0) throw new IllegalArgumentException("count is negative: " + count);
        for (int i = 0; i < count; i++) appendCodePoint(codePoint);
        return this;
    }

    public AbstractStringBuilder repeat(CharSequence cs, int count) {
        if (count < 0) throw new IllegalArgumentException("count is negative: " + count);
        for (int i = 0; i < count; i++) append(cs);
        return this;
    }

    public abstract String toString();

    int compareTo(AbstractStringBuilder another) {
        if (this == another) return 0;
        int lim = Math.min(count, another.count);
        for (int k = 0; k < lim; k++) {
            if (value[k] != another.value[k]) return value[k] - another.value[k];
        }
        return count - another.count;
    }
}
