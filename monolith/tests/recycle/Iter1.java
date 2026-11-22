import java.util.*;
// for-each iterator recycling: every shape of loop must behave exactly like HotSpot, whether or not the iterator
// of the loop is given back at its end.
public class Iter1 {
    static Iterator<Integer> held;                       // an iterator that outlives its loop
    static int sum(List<Integer> l) { int s = 0; for (int x : l) s += x; return s; }
    static int nested(List<Integer> a, List<Integer> b) { int s = 0; for (int x : a) for (int y : b) s += x * y; return s; }
    static int early(List<Integer> l, int stop) { int s = 0; for (int x : l) { if (x == stop) break; s += x; } return s; }
    static int ret(List<Integer> l, int want) { for (int x : l) if (x == want) return x * 10; return -1; }
    static int skip(List<Integer> l) { int s = 0; for (int x : l) { if ((x & 1) == 0) continue; s += x; } return s; }
    static int thrown(List<Integer> l) { int s = 0; try { for (int x : l) { if (x == 3) throw new IllegalStateException("x3"); s += x; } } catch (IllegalStateException e) { s += 1000; } return s; }
    static int cme(List<Integer> l) { int s = 0; try { for (int x : l) { s += x; if (x == 2) l.add(99); } } catch (ConcurrentModificationException e) { s += 5000; } return s; }
    static int removing(List<Integer> l) { int s = 0; Iterator<Integer> it = l.iterator(); while (it.hasNext()) { int x = it.next(); if (x % 2 == 0) it.remove(); else s += x; } return s + l.size() * 100; }
    static boolean after(List<Integer> l) { Iterator<Integer> it = l.iterator(); while (it.hasNext()) it.next(); return it.hasNext(); }   // used after the loop
    static int alias(List<Integer> l) { Iterator<Integer> it = l.iterator(); Iterator<Integer> a = it; int s = 0; while (a.hasNext()) s += a.next(); return s; }
    static int keep(List<Integer> l) { Iterator<Integer> it = l.iterator(); held = it; int s = 0; while (it.hasNext()) s += it.next(); return s; }
    static int reuseSlot(List<Integer> l) { int s = 0; for (int x : l) s += x; int t = 7; for (int i = 0; i < 3; i++) t += i; String u = "k"; return s + t + u.length(); }
    static int rec(List<Integer> l, int d) { if (d == 0) return 0; int s = 0; for (int x : l) s += x + rec(l, d - 1); return s; }
    static int twice(List<Integer> l, boolean b) { int s = 0; if (b) { for (int x : l) s += x; } else { for (int x : l) s -= x; } return s; }
    static int sets(Set<Integer> a, Map<String, Integer> m) { int s = 0; for (int x : a) s += x; for (String k : m.keySet()) s += k.length(); for (int v : m.values()) s += v; for (Map.Entry<String, Integer> e : m.entrySet()) s += e.getKey().length() * e.getValue(); return s; }
    static int empty() { int s = 0; for (int x : new ArrayList<Integer>()) s++; for (int x : List.<Integer>of()) s++; for (int x : Collections.<Integer>emptyList()) s++; return s; }
    static int imm(List<Integer> l) { int s = 0; for (int x : l) s += x; return s; }

    public static void main(String[] args) {
        List<Integer> a = new ArrayList<>(List.of(1, 2, 3, 4, 5, 6, 7, 8));
        List<Integer> b = new ArrayList<>(List.of(10, 20, 30));
        long h = 0;
        for (int round = 0; round < 3000; round++) {
            h += sum(a) + nested(a, b) + early(a, 5) + ret(a, 6) + skip(a) + thrown(a) + twice(a, (round & 1) == 0) + reuseSlot(a) + empty();
            h += imm(List.of(1, 2)) + imm(List.of(1, 2, 3, 4)) + imm(List.copyOf(a));
            h += rec(b, 3);
            h += alias(a) + keep(b) + (after(a) ? 1 : 0);
        }
        System.out.println("loops " + h);
        System.out.println("cme " + cme(new ArrayList<>(List.of(1, 2, 3))));
        System.out.println("removing " + removing(new ArrayList<>(List.of(1, 2, 3, 4, 5, 6))));
        System.out.println("held " + held.hasNext());
        Map<String, Integer> m = new LinkedHashMap<>(); m.put("a", 1); m.put("bb", 2); m.put("ccc", 3);
        Set<Integer> set = new LinkedHashSet<>(List.of(5, 6, 7));
        long g = 0;
        for (int round = 0; round < 3000; round++) g += sets(set, m) + sets(new HashSet<>(set), new HashMap<>(m));
        System.out.println("maps " + g);
        EnumSet<Thread.State> es = EnumSet.of(Thread.State.NEW, Thread.State.BLOCKED);
        int c = 0; for (int r = 0; r < 2000; r++) for (Thread.State st : es) c += st.ordinal();
        System.out.println("enums " + c);
    }
}
