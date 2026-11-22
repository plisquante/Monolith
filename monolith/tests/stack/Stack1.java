import java.util.*;
// Stack allocation of non-escaping objects (escape analysis + inlining): every shape must behave exactly like HotSpot.
// The traps are the loops: an object that is still referenced by the next iteration must not share one slot.
public class Stack1 {
    static final class Vec {
        final double x, y, z;
        Vec(double x, double y, double z) { this.x = x; this.y = y; this.z = z; }
        Vec add(Vec o) { return new Vec(x + o.x, y + o.y, z + o.z); }
        Vec sub(Vec o) { return new Vec(x - o.x, y - o.y, z - o.z); }
        Vec scale(double f) { return new Vec(x * f, y * f, z * f); }
        double len2() { return x * x + y * y + z * z; }
    }
    static final class Holder { final Object payload; final int tag; Holder(Object p, int t) { payload = p; tag = t; } }
    static Vec kept; static Vec[] arr = new Vec[8]; static List<Vec> list = new ArrayList<>();
    Vec field;

    static double temps(int n) { double s = 0; Vec a = new Vec(1, 2, 3), b = new Vec(0.5, 0.25, 2); for (int i = 0; i < n; i++) { Vec v = a.sub(b).scale(i).add(b); s += v.len2(); } return s; }
    static double carried(int n) { double s = 0; Vec prev = null; for (int i = 0; i < n; i++) { Vec cur = new Vec(i, i + 1, i + 2); if (prev != null) s += prev.x * 10 + cur.y; prev = cur; } return s + prev.z; }
    static double usedAfter(int n) { Vec t = null; double s = 0; for (int i = 0; i < n; i++) { t = new Vec(i, 0, 0); s += t.x; } return s + t.x * 1000; }
    static double deadAfter(int n) { double s = 0; for (int i = 0; i < n; i++) { Vec t = new Vec(i, 1, 0); s += t.x + t.y; } return s; }
    static double toArray(int n) { for (int i = 0; i < n; i++) arr[i & 7] = new Vec(i, 0, 0); double s = 0; for (Vec v : arr) s += v == null ? 0 : v.x; return s; }
    static double toStatic(int n) { for (int i = 0; i < n; i++) kept = new Vec(i, 2, 0); return kept.x; }
    static double toList(int n) { list.clear(); for (int i = 0; i < n; i++) list.add(new Vec(i, 3, 0)); double s = 0; for (Vec v : list) s += v.x; return s; }
    void toField(int n) { for (int i = 0; i < n; i++) field = new Vec(i, 4, 0); }
    static Vec returned(int i) { return new Vec(i, 5, 0); }
    static void store(Stack1 s, Vec v) { s.field = v; }
    static double viaCallee(int n) { Stack1 s = new Stack1(); for (int i = 0; i < n; i++) store(s, new Vec(i, 6, 0)); return s.field.x; }
    static double nested(int n) { double s = 0; Vec outer = new Vec(1, 1, 1); for (int i = 0; i < n; i++) { Vec o = outer.scale(i); for (int j = 0; j < 3; j++) { Vec t = o.add(new Vec(j, 0, 0)); s += t.len2(); } outer = o.scale(0.5); } return s + outer.x; }
    static double caught(int n) { double s = 0; for (int i = 0; i < n; i++) { Vec v = new Vec(i, 0, 0); try { if (i % 7 == 3) throw new IllegalStateException("x"); s += v.x; } catch (IllegalStateException e) { s += v.x * 100; } } return s; }
    static double recur(int d) { Vec v = new Vec(d, 1, 0); if (d == 0) return v.x; double r = recur(d - 1); return r + v.x * 2; }
    static boolean ident(int n) { boolean same = true; Vec k = new Vec(1, 1, 1); for (int i = 0; i < n; i++) { Vec t = new Vec(1, 1, 1); same &= (t != k) && (t.x == k.x); k = (i & 1) == 0 ? t : k; } return same; }
    static int heapRefs(int n) {
        // temporaries on the stack that hold the only reference to a heap object, while garbage collections run
        int s = 0;
        for (int i = 0; i < n; i++) {
            Holder h = new Holder(new byte[512 + (i & 63)], i);
            byte[] junk = new byte[2048];            // pressure
            junk[5] = (byte) i;
            ((byte[]) h.payload)[7] = (byte) (i * 3);
            s += ((byte[]) h.payload)[7] + h.tag + junk[5];
        }
        return s;
    }

    public static void main(String[] args) {
        System.out.println("temps " + temps(20000));
        System.out.println("carried " + carried(5000));
        System.out.println("usedAfter " + usedAfter(5000));
        System.out.println("deadAfter " + deadAfter(5000));
        System.out.println("toArray " + toArray(10000));
        System.out.println("toStatic " + toStatic(10000));
        System.out.println("toList " + toList(3000));
        Stack1 s = new Stack1(); s.toField(5000); System.out.println("toField " + s.field.x);
        System.out.println("viaCallee " + viaCallee(5000));
        System.out.println("nested " + nested(3000));
        System.out.println("caught " + caught(10000));
        System.out.println("recur " + recur(200));
        System.out.println("ident " + ident(1000));
        System.out.println("returned " + returned(41).x);
        System.out.println("heapRefs " + heapRefs(60000));
    }
}
