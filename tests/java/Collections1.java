import java.util.*;
import java.util.stream.*;

public class Collections1 {
    public static void main(String[] args) {
        List<Integer> list = new ArrayList<>();
        for (int i = 0; i < 50; i++) list.add((i * 37) % 101);
        Collections.sort(list);
        System.out.println(list + " " + Collections.binarySearch(list, 37) + " " + list.indexOf(74) + " " + list.contains(1000));
        LinkedList<String> ll = new LinkedList<>(List.of("b", "c"));
        ll.addFirst("a"); ll.addLast("d"); ll.removeFirst(); ll.add(1, "x");
        System.out.println(ll + " " + ll.peekLast() + " " + ll.size());
        Map<String, Integer> hm = new HashMap<>();
        for (int i = 0; i < 1000; i++) hm.put("k" + i, i);
        hm.remove("k5"); hm.put("k7", -7);
        long sum = 0; for (int v : hm.values()) sum += v;
        System.out.println(hm.size() + " " + sum + " " + hm.get("k999") + " " + hm.getOrDefault("nope", -1) + " " + hm.containsKey("k5") + " " + new TreeMap<>(hm).firstKey());
        LinkedHashMap<Integer, String> lru = new LinkedHashMap<>(16, 0.75f, true) { protected boolean removeEldestEntry(Map.Entry<Integer, String> e) { return size() > 3; } };
        for (int i = 0; i < 6; i++) { lru.put(i, "v" + i); lru.get(i - 1); }
        System.out.println(lru);
        TreeMap<String, Integer> tm = new TreeMap<>(Comparator.reverseOrder());
        tm.put("delta", 4); tm.put("alpha", 1); tm.put("charlie", 3); tm.put("bravo", 2);
        System.out.println(tm + " " + tm.headMap("bravo") + " " + tm.ceilingKey("c") + " " + tm.descendingMap() + " " + tm.pollFirstEntry());
        Set<Integer> hs = new HashSet<>(); for (int i = 0; i < 100; i++) hs.add(i % 17);
        TreeSet<Integer> ts = new TreeSet<>(hs);
        System.out.println(hs.size() + " " + ts + " " + ts.subSet(3, 8) + " " + ts.floor(100) + " " + ts.higher(5));
        ArrayDeque<Integer> dq = new ArrayDeque<>();
        for (int i = 0; i < 10; i++) { if (i % 2 == 0) dq.push(i); else dq.offerLast(i); }
        System.out.println(dq + " " + dq.pop() + " " + dq.pollLast());
        PriorityQueue<int[]> pq = new PriorityQueue<>((x, y) -> x[0] != y[0] ? Integer.compare(x[0], y[0]) : Integer.compare(y[1], x[1]));
        for (int i = 0; i < 20; i++) pq.add(new int[]{(i * 7) % 5, i});
        StringBuilder sb = new StringBuilder(); while (!pq.isEmpty()) { int[] e = pq.poll(); sb.append(e[0]).append(':').append(e[1]).append(' '); }
        System.out.println(sb);
        int[] arr = {5, 3, 9, 1, 7, 2, 8};
        int[] copy = Arrays.copyOf(arr, 10); Arrays.sort(arr);
        Integer[] boxed = {5, 3, 9, 1}; Arrays.sort(boxed, Collections.reverseOrder());
        long[][] grid = new long[3][4]; grid[1][2] = 5;
        System.out.println(Arrays.toString(arr) + " " + Arrays.toString(copy) + " " + Arrays.toString(boxed) + " " + Arrays.deepToString(grid) + " " + Arrays.equals(arr, arr.clone()) + " " + Arrays.binarySearch(arr, 7));
        Iterator<Integer> it = list.iterator(); while (it.hasNext()) if (it.next() % 2 == 0) it.remove();
        System.out.println(list.size() + " " + list.subList(0, 5) + " " + Collections.unmodifiableList(list).get(0) + " " + Collections.nCopies(3, "z") + " " + Collections.frequency(list, 1));
        List<String> names = Arrays.asList("delta", "alpha", "charlie", "bravo", "echo", "alpha");
        System.out.println(names.stream().distinct().sorted().collect(Collectors.toList()));
        System.out.println(new TreeMap<>(names.stream().collect(Collectors.groupingBy(String::length, Collectors.counting()))));
        System.out.println(IntStream.rangeClosed(1, 10).map(x -> x * x).filter(x -> x % 2 == 1).sum() + " " + IntStream.range(0, 5).boxed().collect(Collectors.toList())
                + " " + names.stream().map(String::toUpperCase).collect(Collectors.joining(",", "[", "]")) + " " + names.stream().mapToInt(String::length).max().getAsInt()
                + " " + Stream.iterate(1, x -> x * 2).limit(10).reduce(0, Integer::sum) + " " + names.stream().anyMatch(s -> s.startsWith("e")) + " " + names.stream().collect(Collectors.toMap(s -> s, String::length, (x, y) -> x, TreeMap::new)));
        System.out.println(Stream.of("a", "bb", "ccc").flatMap(s -> s.chars().mapToObj(c -> (char) c)).map(String::valueOf).collect(Collectors.joining()) + " " + IntStream.of(3, 1, 2).sorted().boxed().toList() + " " + Arrays.stream(new int[]{1, 2, 3}).average().getAsDouble()
                + " " + names.stream().collect(Collectors.partitioningBy(s -> s.length() > 4)));
        Map<Character, List<String>> byFirst = new TreeMap<>(); for (String n : names) byFirst.computeIfAbsent(n.charAt(0), k -> new ArrayList<>()).add(n);
        System.out.println(byFirst + " " + List.copyOf(names).equals(names) + " " + Set.of(1).contains(1) + " " + Map.of("a", 1).get("a") + " " + Map.entry("k", "v"));
        BitSet bs = new BitSet(); bs.set(3); bs.set(10, 13);
        System.out.println(bs + " " + bs.cardinality() + " " + Collections.emptyList() + " " + new StringJoiner("|").add("x").add("y"));
        Random rnd = new Random(42);
        System.out.println(rnd.nextInt() + " " + rnd.nextInt(100) + " " + rnd.nextLong() + " " + rnd.nextDouble() + " " + rnd.nextBoolean() + " " + rnd.nextGaussian());
        List<Integer> shuf = new ArrayList<>(List.of(1, 2, 3, 4, 5, 6, 7, 8)); Collections.shuffle(shuf, new Random(7));
        System.out.println(shuf);
    }
}
