import java.util.*;
import java.util.function.*;

public class Mono5 {
    enum Color { RED, GREEN, BLUE; Color next() { return values()[(ordinal() + 1) % values().length]; } }
    record Point(int x, int y) { int dist() { return Math.abs(x) + Math.abs(y); } }
    sealed interface Shape permits Circle, Square {}
    record Circle(double r) implements Shape {}
    record Square(double s) implements Shape {}
    static double area(Shape s) { return switch (s) { case Circle c -> 3.0 * c.r() * c.r(); case Square q -> q.s() * q.s(); }; }
    static int sum(int... xs) { int t = 0; for (int x : xs) t += x; return t; }
    static <T extends Comparable<T>> T max(List<T> l) { T m = l.get(0); for (T t : l) if (t.compareTo(m) > 0) m = t; return m; }

    public static void main(String[] args) {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < 5; i++) sb.append(i).append(',');
        sb.append("end").append(1.5).append('c').append(true).append(10000000000L);
        System.out.println(sb.toString());
        System.out.println(sb.length());
        String s = "Hello, Monolith";
        System.out.println(s.toUpperCase() + s.substring(7) + s.indexOf("Mono") + s.charAt(4) + s.length());
        System.out.println(s.equals("Hello, Monolith") + " " + "a".compareTo("b") + " " + s.hashCode());
        List<Integer> list = new ArrayList<>();
        for (int i = 0; i < 10; i++) list.add(i * i);
        System.out.println(list);
        Map<String, Integer> map = new HashMap<>();
        map.put("one", 1); map.put("two", 2); map.put("three", 3);
        System.out.println(map.get("two") + map.size() + " " + map.containsKey("four"));
        int total = 0;
        for (Map.Entry<String, Integer> e : new TreeMap<>(map).entrySet()) total += e.getValue() * e.getKey().length();
        System.out.println(total);
        Function<Integer, Integer> f = x -> x * 3;
        BiFunction<Integer, Integer, Integer> g = (a, b) -> a + b;
        Supplier<String> sup = () -> "sup";
        System.out.println(f.apply(7) + g.apply(1, 2) + sup.get());
        System.out.println(list.stream().filter(x -> x % 2 == 0).mapToInt(Integer::intValue).sum());
        System.out.println(Color.RED.next() + " " + Color.BLUE.next() + " " + Color.valueOf("GREEN").ordinal());
        Point p = new Point(3, -4);
        System.out.println(p + " " + p.dist() + " " + p.equals(new Point(3, -4)));
        System.out.println(area(new Circle(2)) + area(new Square(3)));
        System.out.println(sum() + sum(1) + sum(1, 2, 3));
        System.out.println(max(Arrays.asList(3, 9, 4)) + max(Arrays.asList("b", "z", "a")));
        String sw = "beta";
        switch (sw) { case "alpha": System.out.println(1); break; case "beta": System.out.println(2); break; default: System.out.println(3); }
        int[] arr = {5, 3, 9, 1};
        Arrays.sort(arr);
        System.out.println(Arrays.toString(arr));
        System.out.println(String.format("%d-%s-%.2f-%x", 42, "x", 3.14159, 255));
        System.out.println(Integer.parseInt("1234") + Long.parseLong("99") + Double.parseDouble("2.5"));
        System.out.println(Integer.toBinaryString(10) + Integer.toHexString(-1) + Long.MAX_VALUE + Double.MAX_VALUE + Float.MIN_VALUE);
        Optional<String> o = Optional.of("opt");
        System.out.println(o.map(String::toUpperCase).orElse("none"));
        Object[] objs = {1, "two", 3.0, 'c', null};
        for (Object x : objs) System.out.println(x instanceof Integer ? "I" : x instanceof String ? "S" : x == null ? "N" : "O");
        try { Object x = list.get(99); } catch (IndexOutOfBoundsException e) { System.out.println(e.getMessage()); }
        try { Integer.parseInt("zz"); } catch (NumberFormatException e) { System.out.println(e.getMessage()); }
        long h = 0; for (int i = 0; i < 1000000; i++) h = h * 31 + i; System.out.println(h);
        System.out.println(Math.sqrt(2) + " " + Math.pow(2, 10) + " " + Math.floor(-1.5) + " " + Math.round(2.5) + " " + (int) Math.signum(-3.0));
    }
}
