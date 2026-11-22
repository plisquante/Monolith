import java.util.*;

interface Shape { double area(); default String describe() { return name() + " with area " + area(); } String name(); static Shape unit() { return new Square(1); } }
abstract class Base implements Shape { static int created; final int id; Base() { id = ++created; init(); } void init() { } public String name() { return getClass().getSimpleName() + "#" + id; } }
class Square extends Base { double side; Square(double s) { side = s; } public double area() { return side * side; } }
class Circle extends Base { double r; String tag = "circle"; Circle(double r) { this.r = r; } public double area() { return 3 * r * r; } void init() { tag = "init"; } public String describe() { return "Circle:" + super.describe() + ":" + tag; } }
class Animal { String sound() { return "..."; } String who() { return "animal says " + sound(); } static String st() { return "Animal.st"; } String field = "A"; }
class Dog extends Animal { String sound() { return "woof"; } String who() { return "dog: " + super.who(); } static String st() { return "Dog.st"; } String field = "D"; }
class Puppy extends Dog { String sound() { return "yip"; } }
interface Visitor<R> { R visitNum(int n); R visitAdd(Expr a, Expr b); }
interface Expr { <R> R accept(Visitor<R> v); }
record Num(int n) implements Expr { public <R> R accept(Visitor<R> v) { return v.visitNum(n); } }
record Add(Expr a, Expr b) implements Expr { public <R> R accept(Visitor<R> v) { return v.visitAdd(a, b); } }
record Point(int x, int y, String label, double w, long l, boolean f, char ch, float fl) {
    Point { if (x < 0) throw new IllegalArgumentException("negative"); }
    Point(int x) { this(x, 0, "origin", 0.5, 7L, true, 'c', 1.5f); }
    int sum() { return x + y; }
}
enum Planet {
    MERCURY(3.303e+23, 2.4397e6), EARTH(5.976e+24, 6.37814e6) { String extra() { return "home"; } };
    final double mass, radius;
    Planet(double m, double r) { mass = m; radius = r; }
    double gravity() { return 6.67300E-11 * mass / (radius * radius); }
    String extra() { return "none"; }
}
enum Op { ADD { int apply(int a, int b) { return a + b; } }, MUL { int apply(int a, int b) { return a * b; } }; abstract int apply(int a, int b); }
sealed interface Vehicle permits Car, Bike {}
record Car(int seats) implements Vehicle {}
record Bike(boolean electric) implements Vehicle {}
class Outer { private int secret = 7; class Inner { int peek() { return secret * 2; } } static class Nested { int v = 3; } int viaLocal() { class Local { int g() { return secret + 1; } } return new Local().g(); } }
class Gen<T extends Comparable<T>> { T best; void offer(T t) { if (best == null || t.compareTo(best) > 0) best = t; } }
class Counter implements Comparable<Counter> { int n; Counter(int n) { this.n = n; } public int compareTo(Counter o) { return Integer.compare(n, o.n); } public String toString() { return "C" + n; } }
class StaticInit { static final List<String> log = new ArrayList<>(); static int a = step("a"); static { step("block"); } static int b = step("b"); static int step(String s) { log.add(s); return log.size(); } }

public class Oop {
    static String classify(Vehicle v) {
        return switch (v) {
            case Car c when c.seats() > 4 -> "big car " + c.seats();
            case Car c -> "car " + c.seats();
            case Bike b -> b.electric() ? "e-bike" : "bike";
        };
    }
    static String obj(Object o) {
        return switch (o) {
            case null -> "null!";
            case Integer i when i > 10 -> "large int";
            case Integer i -> "int " + i;
            case String s -> "string " + s.length();
            case int[] arr -> "int array " + arr.length;
            default -> "other " + o.getClass().getName();
        };
    }
    static int days(String m) { return switch (m) { case "feb" -> 28; case "apr", "jun", "sep", "nov" -> 30; default -> { int x = 31; yield x; } }; }

    public static void main(String[] args) {
        List<Shape> shapes = List.of(new Square(2), new Circle(1), Shape.unit());
        for (Shape s : shapes) System.out.println(s.describe());
        Animal a = new Puppy();
        System.out.println(a.who() + " " + a.field + " " + ((Dog) a).field + " " + Animal.st() + " " + Dog.st() + " " + (a instanceof Dog) + " " + (a instanceof Puppy));
        Expr e = new Add(new Num(3), new Add(new Num(4), new Num(5)));
        int val = e.accept(new Visitor<Integer>() { public Integer visitNum(int n) { return n; } public Integer visitAdd(Expr x, Expr y) { return x.accept(this) + y.accept(this); } });
        String shown = e.accept(new Visitor<String>() { public String visitNum(int n) { return "" + n; } public String visitAdd(Expr x, Expr y) { return "(" + x.accept(this) + "+" + y.accept(this) + ")"; } });
        System.out.println(shown + " = " + val + " " + e);
        Point p = new Point(1, 2, "p", 1.25, 3L, false, 'z', 2.5f), q = new Point(1, 2, "p", 1.25, 3L, false, 'z', 2.5f);
        System.out.println(p + " " + p.equals(q) + " " + (p.hashCode() == q.hashCode()) + " " + p.hashCode() + " " + new Point(5) + " " + p.sum() + " " + p.equals(new Point(1)));
        try { new Point(-1); } catch (IllegalArgumentException ex) { System.out.println("rejected " + ex.getMessage()); }
        for (Planet pl : Planet.values()) System.out.printf("%s %d %.3f %s%n", pl, pl.ordinal(), pl.gravity(), pl.extra());
        System.out.println(Planet.valueOf("EARTH").name() + " " + Op.MUL.apply(6, 7) + " " + Op.valueOf("ADD").apply(1, 2) + " " + Op.ADD.getClass() + " " + Op.ADD.getDeclaringClass() + " " + Planet.EARTH.compareTo(Planet.MERCURY));
        try { Planet.valueOf("PLUTO"); } catch (IllegalArgumentException ex) { System.out.println(ex.getMessage()); }
        EnumMap<Op, String> em = new EnumMap<>(Op.class); em.put(Op.MUL, "times"); em.put(Op.ADD, "plus");
        System.out.println(em + " " + EnumSet.allOf(Planet.class));
        for (Op op : Op.values()) { switch (op) { case ADD -> System.out.print("add "); case MUL -> System.out.print("mul "); } }
        System.out.println();
        System.out.println(classify(new Car(7)) + ", " + classify(new Car(2)) + ", " + classify(new Bike(true)));
        for (Object o : new Object[]{null, 5, 50, "hey", new int[3], 2.0}) System.out.print(obj(o) + "; ");
        System.out.println();
        System.out.println(days("feb") + " " + days("jun") + " " + days("jan"));
        Outer out = new Outer();
        System.out.println(out.new Inner().peek() + " " + new Outer.Nested().v + " " + out.viaLocal());
        Gen<Counter> g = new Gen<>(); for (int i : new int[]{3, 9, 2}) g.offer(new Counter(i));
        System.out.println(g.best + " " + StaticInit.b + " " + StaticInit.log);
        Object[] objs = {new Square(1), "s", 1, 'c', 2L, 1.0f, new int[0], new String[0][0], Op.ADD, new Num(1)};
        for (Object o : objs) System.out.print(o.getClass().getName() + "/" + o.getClass().getSimpleName() + " ");
        System.out.println();
        System.out.println(int[].class.getName() + " " + String.class.getName() + " " + int.class + " " + Integer.TYPE + " " + Shape.class.isInterface() + " " + Square.class.getSuperclass().getName() + " " + Comparable.class.isAssignableFrom(String.class));
    }
}
