public class Mono4 {
    interface Named { int id(); default int twice() { return id() * 2; } }
    interface Scored extends Named { int score(int bonus); static int zero() { return 0; } }
    interface Marker {}
    static abstract class Base implements Scored {
        int base;
        Base(int b) { base = b; }
        public int id() { return base; }
    }
    static class A extends Base implements Marker {
        A() { super(10); }
        public int score(int bonus) { return base + bonus; }
    }
    static class B extends Base {
        B() { super(20); }
        public int score(int bonus) { return base * bonus; }
        public int twice() { return -1; }
    }
    static class C implements Named {
        public int id() { return 7; }
    }
    static int total(Named[] ns) { int t = 0; for (Named n : ns) t += n.twice() + n.id(); return t; }
    public static void main(String[] args) {
        Scored a = new A(), b = new B();
        System.out.println(a.score(5));
        System.out.println(b.score(5));
        System.out.println(a.twice());
        System.out.println(b.twice());
        Named[] ns = { a, b, new C() };
        System.out.println(total(ns));
        System.out.println(ns[2] instanceof Scored);
        System.out.println(ns[0] instanceof Marker);
        System.out.println(ns[1] instanceof Named);
        System.out.println(Scored.zero());
        Object o = ns[2];
        Named n = (Named) o;
        System.out.println(n.id());
        try { Scored s = (Scored) o; } catch (ClassCastException e) { System.out.println("cce"); }
        A aa = (A) a;
        System.out.println(aa.twice());
    }
}
