import java.nio.ByteBuffer;

public class Jni {
    static { System.loadLibrary("Jni"); }
    static native int add(int a, int b);
    static native int registerTriple();
    static native int triple(int x);
    static native long mul(long a, double b);
    static native String greet(String who);
    static native int sum(int[] values);
    static native void fill(byte[] dst, byte v);
    static native void fail(String msg);
    static native boolean isString(Object o);
    static native long addressOf(ByteBuffer direct);
    static native int countGlobal(Object o);
    native int instanceAdd(int x);
    static native String callbacks(Jni self);
    static native ByteBuffer wrap();
    int twice(int x) { return x * 2; }
    static String sjoin(long a, double b, float c, boolean d, Object e, byte f, char g, short h) { return a + "|" + b + "|" + c + "|" + d + "|" + e + "|" + f + "|" + g + "|" + h; }
    static void dump(Throwable t) { t.printStackTrace(); }
    static void boom() { throw new IllegalArgumentException("java boom"); }
    static String sfield = "static-field";
    static class Pair { final int a; final String b; Pair(int a, String b) { this.a = a; this.b = b; } public String toString() { return "Pair(" + a + "," + b + ")"; } }
    static void warm() { new Jni().twice(1); sjoin(1, 2, 3, true, "x", (byte) 1, 'c', (short) 2); dump(new Error("warm")); try { boom(); } catch (RuntimeException e) { } new Pair(1, "x").toString(); }
    int base = 40;

    public static void main(String[] args) {
        System.out.println(add(2, 3) + " " + mul(6, 7.5) + " " + greet("Zig çağrısı"));
        System.out.println(registerTriple() + " " + triple(14));
        System.out.println(sum(new int[]{1, 2, 3, 4, 5}));
        byte[] b = new byte[5];
        fill(b, (byte) 7);
        System.out.println(java.util.Arrays.toString(b));
        try { fail("from C"); System.out.println("no exception"); }
        catch (IllegalStateException e) { System.out.println("caught " + e.getMessage()); }
        System.out.println(isString("x") + " " + isString(42));
        ByteBuffer bb = ByteBuffer.allocateDirect(16);
        System.out.println(addressOf(bb) != 0);
        System.out.println(countGlobal(new Object()));
        System.out.println(new Jni().instanceAdd(2));
        warm();
        System.out.println(callbacks(new Jni()));
        ByteBuffer w = wrap();
        System.out.println(w.capacity() + " " + w.isDirect() + " " + w.get(3));
    }
}
