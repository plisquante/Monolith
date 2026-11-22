import java.util.*;
import java.util.function.*;

public class Lambdas {
    interface TriFunc<A, B, C, R> { R apply(A a, B b, C c); }
    int base = 10;
    int addBase(int x) { return x + base; }
    static int twice(int x) { return 2 * x; }
    static <T, R> List<R> map(List<T> in, Function<? super T, ? extends R> f) { List<R> out = new ArrayList<>(); for (T t : in) out.add(f.apply(t)); return out; }

    void instanceStuff() {
        Function<Integer, Integer> f = this::addBase;
        IntUnaryOperator g = x -> x * base;
        Supplier<String> s = () -> "base=" + base;
        System.out.println(f.apply(5) + " " + g.applyAsInt(3) + " " + s.get());
    }

    public static void main(String[] args) {
        int captured = 7;
        String prefix = "p:";
        Function<Integer, Integer> add = x -> x + captured;
        BiFunction<Integer, Integer, Integer> mul = (x, y) -> x * y;
        Function<Integer, Integer> chain = add.andThen(Lambdas::twice).compose(x -> x - 1);
        UnaryOperator<String> up = String::toUpperCase;
        Function<String, Integer> len = String::length;
        Supplier<List<String>> mk = ArrayList::new;
        Function<Integer, int[]> mkArr = int[]::new;
        BiPredicate<String, String> eq = String::equals;
        TriFunc<Integer, Long, Double, String> tri = (a, b, c) -> prefix + a + "," + b + "," + c;
        ToIntFunction<String> parse = Integer::parseInt;
        IntBinaryOperator max = Math::max;
        LongSupplier ls = () -> 1L << 40;
        DoubleUnaryOperator half = d -> d / 2;
        Predicate<Integer> even = x -> x % 2 == 0;
        Consumer<String> printer = System.out::println;
        System.out.println(add.apply(1) + " " + mul.apply(6, 7) + " " + chain.apply(10) + " " + up.apply("abc") + " " + len.apply("four") + " " + mk.get().size()
                + " " + mkArr.apply(4).length + " " + eq.test("a", "a") + " " + tri.apply(1, 2L, 3.0) + " " + parse.applyAsInt("99") + " " + max.applyAsInt(3, 8)
                + " " + ls.getAsLong() + " " + half.applyAsDouble(5) + " " + even.negate().test(4) + " " + even.and(x -> x > 2).test(4));
        printer.accept("consumer works");
        new Lambdas().instanceStuff();
        List<String> words = new ArrayList<>(List.of("banana", "Apple", "cherry", "date", "fig", "elderberry"));
        words.sort(Comparator.comparing(String::length).thenComparing(Comparator.reverseOrder()));
        System.out.println(words);
        words.sort(String.CASE_INSENSITIVE_ORDER);
        System.out.println(words + " " + map(words, w -> w.charAt(0)));
        words.removeIf(w -> w.length() < 5);
        words.replaceAll(w -> w + "!");
        System.out.println(words);
        Map<String, Integer> counts = new TreeMap<>();
        for (String w : "the quick brown fox jumps over the lazy dog the end".split(" ")) counts.merge(w, 1, Integer::sum);
        counts.computeIfAbsent("zebra", k -> k.length());
        counts.computeIfPresent("fox", (k, v) -> v + 100);
        StringBuilder sb = new StringBuilder();
        counts.forEach((k, v) -> sb.append(k).append('=').append(v).append(' '));
        System.out.println(sb);
        Optional<String> first = words.stream().filter(w -> w.startsWith("c")).findFirst();
        System.out.println(first.orElse("none") + " " + first.map(String::length).orElse(-1) + " " + Optional.empty().isPresent());
        Runnable[] rs = new Runnable[3];
        for (int i = 0; i < 3; i++) { final int j = i; rs[i] = () -> System.out.print("r" + j + " "); }
        for (Runnable r : rs) r.run();
        System.out.println();
        Function<Integer, Function<Integer, Integer>> curried = x -> y -> x * 10 + y;
        System.out.println(curried.apply(4).apply(2));
        IntFunction<String> boxer = Integer::toString;
        Function<Object, String> ts = Object::toString;
        Comparator<Integer> natural = Comparator.naturalOrder();
        System.out.println(boxer.apply(5) + " " + ts.apply(List.of(1)) + " " + natural.compare(1, 2) + " " + Collections.max(List.of(3, 9, 4), natural.reversed()));
        Supplier<Supplier<String>> nested = () -> () -> "deep";
        System.out.println(nested.get().get());
    }
}
