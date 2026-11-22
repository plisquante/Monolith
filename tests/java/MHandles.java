import java.lang.invoke.*;
import java.util.*;

public class MHandles {
    private int base = 10;
    static String sfield = "static";
    public MHandles() {}
    public MHandles(int b) { base = b; }
    public int add(int x, long y) { return (int) (base + x + y); }
    public static String cat(String a, Object b) { return a + "+" + b; }
    private String secret() { return "secret" + base; }
    public static int sum(int... xs) { int s = 0; for (int x : xs) s += x; return s; }
    static boolean isEven(int x) { return x % 2 == 0; }
    static String even(int x) { return "even " + x; }
    static String odd(int x) { return "odd " + x; }
    static String fail(String s) { throw new IllegalStateException(s); }
    static String recover(IllegalStateException e, String s) { return "caught " + e.getMessage(); }
    interface Shape { default String name() { return "shape"; } double area(); }
    static class Sq implements Shape { public double area() { return 4.0; } public String name() { return "square"; } }

    public static void main(String[] args) throws Throwable {
        MethodHandles.Lookup L = MethodHandles.lookup();
        MethodHandle add = L.findVirtual(MHandles.class, "add", MethodType.methodType(int.class, int.class, long.class));
        MHandles o = new MHandles(5);
        int r = (int) add.invokeExact(o, 3, 4L);
        System.out.println(r + " " + add.type());
        System.out.println((int) add.invoke(o, (short) 3, 4));          // asType widening
        Object boxed = add.invoke(o, 1, 1L);
        System.out.println(boxed + " " + add.invokeWithArguments(o, 2, 2L));
        MethodHandle cat = L.findStatic(MHandles.class, "cat", MethodType.methodType(String.class, String.class, Object.class));
        System.out.println((String) cat.invokeExact("a", (Object) 42));
        MethodHandle bound = cat.bindTo("bound");
        System.out.println((String) bound.invoke("x") + " " + bound.type());
        MethodHandle ins = MethodHandles.insertArguments(cat, 1, 7);
        System.out.println((String) ins.invokeExact("ins"));
        MethodHandle ctor = L.findConstructor(MHandles.class, MethodType.methodType(void.class, int.class));
        MHandles o2 = (MHandles) ctor.invoke(99);
        System.out.println(o2.base + " " + ctor.type());
        MethodHandle getter = L.findGetter(MHandles.class, "base", int.class);
        MethodHandle setter = L.findSetter(MHandles.class, "base", int.class);
        setter.invoke(o2, 123);
        System.out.println((int) getter.invokeExact(o2) + " " + L.findStaticGetter(MHandles.class, "sfield", String.class).invoke());
        MethodHandle secret = L.findSpecial(MHandles.class, "secret", MethodType.methodType(String.class), MHandles.class);
        System.out.println((String) secret.invoke(o2));
        MethodHandle sum = L.findStatic(MHandles.class, "sum", MethodType.methodType(int.class, int[].class));
        System.out.println((int) sum.invoke(new int[]{1, 2, 3}) + " " + sum.asVarargsCollector(int[].class).invoke(1, 2, 3, 4));
        MethodHandle area = L.findVirtual(Shape.class, "area", MethodType.methodType(double.class));
        MethodHandle nm = L.findVirtual(Shape.class, "name", MethodType.methodType(String.class));
        System.out.println((double) area.invoke(new Sq()) + " " + nm.invoke(new Sq()) + " " + nm.invoke((Shape) () -> 1.0));
        MethodHandle guard = MethodHandles.guardWithTest(
                L.findStatic(MHandles.class, "isEven", MethodType.methodType(boolean.class, int.class)),
                L.findStatic(MHandles.class, "even", MethodType.methodType(String.class, int.class)),
                L.findStatic(MHandles.class, "odd", MethodType.methodType(String.class, int.class)));
        System.out.println(guard.invoke(4) + " / " + guard.invoke(7));
        MethodHandle len = L.findVirtual(String.class, "length", MethodType.methodType(int.class));
        MethodHandle filtered = MethodHandles.filterReturnValue(L.findStatic(MHandles.class, "even", MethodType.methodType(String.class, int.class)), len);
        System.out.println((int) filtered.invoke(12));
        MethodHandle fa = MethodHandles.filterArguments(cat, 0, L.findVirtual(Object.class, "toString", MethodType.methodType(String.class)).asType(MethodType.methodType(String.class, Integer.class)));
        System.out.println(fa.invoke(5, "z") + " " + fa.type());
        MethodHandle caught = MethodHandles.catchException(
                L.findStatic(MHandles.class, "fail", MethodType.methodType(String.class, String.class)), IllegalStateException.class,
                L.findStatic(MHandles.class, "recover", MethodType.methodType(String.class, IllegalStateException.class, String.class)));
        System.out.println((String) caught.invokeExact("oops"));
        try { MethodHandle f = L.findStatic(MHandles.class, "fail", MethodType.methodType(String.class, String.class)); String s = (String) f.invokeExact("direct"); }
        catch (IllegalStateException e) { System.out.println("ISE " + e.getMessage()); }
        MethodHandle perm = MethodHandles.permuteArguments(cat, MethodType.methodType(String.class, Object.class, String.class), 1, 0);
        System.out.println(perm.invoke(1, "p"));
        MethodHandle drop = MethodHandles.dropArguments(cat, 0, int.class);
        System.out.println(drop.invoke(0, "d", "e") + " " + MethodHandles.constant(String.class, "K").invoke() + " " + MethodHandles.identity(int.class).invoke(3));
        MethodHandle fold = MethodHandles.foldArguments(cat, L.findVirtual(Object.class, "toString", MethodType.methodType(String.class)));
        System.out.println(fold.invoke((Object) "f") + " " + fold.type());
        MethodHandle ec = MethodHandles.explicitCastArguments(MethodHandles.identity(int.class), MethodType.methodType(byte.class, long.class));
        System.out.println((byte) ec.invokeExact(300L));
        MethodHandle ag = MethodHandles.arrayElementGetter(String[].class);
        System.out.println(ag.invoke(new String[]{"q", "r"}, 1) + " " + MethodHandles.arrayLength(int[].class).invoke(new int[5]));
        MethodHandle un = L.unreflect(MHandles.class.getMethod("add", int.class, long.class));
        System.out.println(un.invoke(new MHandles(), 1, 2L));
        try { L.findVirtual(MHandles.class, "nope", MethodType.methodType(void.class)); } catch (NoSuchMethodException e) { System.out.println("NSME"); }
        MethodHandle inv = MethodHandles.invoker(MethodType.methodType(String.class, String.class, Object.class));
        System.out.println((String) inv.invokeExact(cat, "via", (Object) "invoker"));
    }
}
