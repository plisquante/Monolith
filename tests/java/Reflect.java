import java.lang.reflect.*;

public class Reflect {
    public int base = 5;
    public static long counter = 7;
    private String name;

    public Reflect() { this.name = "default"; }
    public Reflect(String n, int b) { this.name = n; this.base = b; }

    public int add(int a, long b) { return (int) (base + a + b); }
    public static double scale(double x, float f) { return x * f; }
    public String greet(String who) { return "hi " + who + " from " + name; }
    public void bump() { counter++; }
    public boolean flag(char c, short s, byte b) { return c == 'x' && s == 3 && b == 4; }
    public String fail() { throw new IllegalStateException("boom"); }
    public String toString() { return "Reflect(" + name + "," + base + ")"; }

    public static String create(String s) { return "created " + s; }
    static Object viaParam(Class<?> c) throws Exception { return c.getMethod("create", String.class).invoke(null, "dyn"); }

    static class Sub extends Reflect {
        public Sub() { super("sub", 100); }
        @Override public int add(int a, long b) { return -1; }
    }

    public static void main(String[] args) throws Exception {
        // keep everything reachable for the closed-world build
        Reflect warm = new Reflect("w", 1);
        warm.add(1, 2); scale(1, 2); warm.greet("x"); warm.bump(); warm.flag('a', (short) 1, (byte) 1);
        new Sub().add(0, 0);
        try { warm.fail(); } catch (IllegalStateException e) { }
        counter = 7;

        System.out.println(viaParam(Reflect.class));
        Class<?> c = Reflect.class;
        Reflect r = (Reflect) c.getConstructor(String.class, int.class).newInstance("bob", 10);
        System.out.println(r);
        Reflect d = (Reflect) c.getConstructor().newInstance();
        System.out.println(d);
        Method add = c.getMethod("add", int.class, long.class);
        System.out.println(add.invoke(r, 3, 4L));
        System.out.println(add.invoke(r, (short) 3, 4)); // widening
        System.out.println(add.invoke(new Sub(), 3, 4L)); // virtual dispatch
        System.out.println(c.getMethod("scale", double.class, float.class).invoke(null, 2.5, 2f));
        System.out.println(c.getMethod("greet", String.class).invoke(r, "ann"));
        c.getMethod("bump").invoke(r);
        System.out.println(counter);
        System.out.println(c.getMethod("flag", char.class, short.class, byte.class).invoke(r, 'x', (short) 3, (byte) 4));
        try {
            c.getMethod("fail").invoke(r);
        } catch (InvocationTargetException e) {
            System.out.println("ITE: " + e.getCause());
        }
        try {
            add.invoke(r, "nope", 1L);
        } catch (IllegalArgumentException e) {
            System.out.println("IAE");
        }
        Field f = c.getField("base");
        System.out.println(f.getInt(r) + " " + f.get(d));
        f.setInt(r, 42);
        System.out.println(r.base + " " + f.getLong(r));
        Field cf = c.getField("counter");
        cf.set(null, 99L);
        System.out.println(counter + " " + cf.get(null));
        Field nf = c.getDeclaredField("name");
        nf.setAccessible(true);
        nf.set(r, "zed");
        System.out.println(r + " " + nf.get(r));
    }
}
