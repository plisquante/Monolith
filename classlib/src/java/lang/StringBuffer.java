package java.lang;

/** j2zig overlay. */
public final class StringBuffer extends AbstractStringBuilder implements java.io.Serializable, Comparable<StringBuffer>, CharSequence {
    public StringBuffer() { super(); }
    public StringBuffer(int capacity) { super(capacity); }
    public StringBuffer(String str) { super(str); }
    public StringBuffer(CharSequence seq) { super(seq); }
    @Override public synchronized StringBuffer append(Object obj) { super.append(obj); return this; }
    @Override public synchronized StringBuffer append(String str) { super.append(str); return this; }
    @Override public synchronized StringBuffer append(StringBuffer sb) { super.append(sb); return this; }
    @Override public synchronized StringBuffer append(CharSequence s) { super.append(s); return this; }
    @Override public synchronized StringBuffer append(CharSequence s, int start, int end) { super.append(s, start, end); return this; }
    @Override public synchronized StringBuffer append(char[] str) { super.append(str); return this; }
    @Override public synchronized StringBuffer append(char[] str, int offset, int len) { super.append(str, offset, len); return this; }
    @Override public synchronized StringBuffer append(boolean b) { super.append(b); return this; }
    @Override public synchronized StringBuffer append(char c) { super.append(c); return this; }
    @Override public synchronized StringBuffer append(int i) { super.append(i); return this; }
    @Override public synchronized StringBuffer append(long l) { super.append(l); return this; }
    @Override public synchronized StringBuffer append(float f) { super.append(f); return this; }
    @Override public synchronized StringBuffer append(double d) { super.append(d); return this; }
    @Override public synchronized StringBuffer delete(int start, int end) { super.delete(start, end); return this; }
    @Override public synchronized StringBuffer appendCodePoint(int codePoint) { super.appendCodePoint(codePoint); return this; }
    @Override public synchronized StringBuffer deleteCharAt(int index) { super.deleteCharAt(index); return this; }
    @Override public synchronized StringBuffer replace(int start, int end, String str) { super.replace(start, end, str); return this; }
    @Override public synchronized StringBuffer insert(int index, char[] str, int offset, int len) { super.insert(index, str, offset, len); return this; }
    @Override public synchronized StringBuffer insert(int offset, Object obj) { super.insert(offset, obj); return this; }
    @Override public synchronized StringBuffer insert(int offset, String str) { super.insert(offset, str); return this; }
    @Override public synchronized StringBuffer insert(int offset, char[] str) { super.insert(offset, str); return this; }
    @Override public synchronized StringBuffer insert(int dstOffset, CharSequence s) { super.insert(dstOffset, s); return this; }
    @Override public synchronized StringBuffer insert(int dstOffset, CharSequence s, int start, int end) { super.insert(dstOffset, s, start, end); return this; }
    @Override public synchronized StringBuffer insert(int offset, boolean b) { super.insert(offset, b); return this; }
    @Override public synchronized StringBuffer insert(int offset, char c) { super.insert(offset, c); return this; }
    @Override public synchronized StringBuffer insert(int offset, int i) { super.insert(offset, i); return this; }
    @Override public synchronized StringBuffer insert(int offset, long l) { super.insert(offset, l); return this; }
    @Override public synchronized StringBuffer insert(int offset, float f) { super.insert(offset, f); return this; }
    @Override public synchronized StringBuffer insert(int offset, double d) { super.insert(offset, d); return this; }
    @Override public synchronized StringBuffer reverse() { super.reverse(); return this; }
    @Override public synchronized StringBuffer repeat(int codePoint, int count) { super.repeat(codePoint, count); return this; }
    @Override public synchronized StringBuffer repeat(CharSequence cs, int count) { super.repeat(cs, count); return this; }
    @Override public synchronized String toString() { return new String(value, 0, count); }
    @Override public synchronized int compareTo(StringBuffer another) { return super.compareTo(another); }
}
