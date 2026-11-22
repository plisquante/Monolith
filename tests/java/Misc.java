import java.util.*;

public class Misc {
    static int sw(int x) { switch (x) { case 1: return 10; case 2: return 20; case 3: case 4: return 34; case 100: return 1000; case -5: return -50; case 1 << 20: return 7; default: return -1; } }
    static int dense(int x) { switch (x) { case 0: return 0; case 1: return 1; case 2: return 4; case 3: return 9; case 4: return 16; case 5: return 25; default: return x; } }
    static long varargs(int... xs) { long s = 0; for (int x : xs) s += x; return s * 10 + xs.length; }
    static String over(Object o) { return "obj"; } static String over(String s) { return "str"; } static String over(Integer i) { return "int"; } static String over(long l) { return "long"; }
    static <T extends Comparable<T>> T maxOf(List<T> l) { T best = l.get(0); for (T t : l) if (t.compareTo(best) > 0) best = t; return best; }

    public static void main(String[] args) {
        StringBuilder sb = new StringBuilder();
        for (int i = -6; i < 8; i++) sb.append(sw(i)).append(',').append(dense(i)).append(' ');
        sb.append(sw(1 << 20)).append(' ').append(sw(100));
        System.out.println(sb);
        outer:
        for (int i = 0; i < 10; i++) {
            for (int j = 0; j < 10; j++) {
                if (j == 3) continue outer;
                if (i == 4) break outer;
                System.out.print(i * 10 + j + " ");
            }
        }
        System.out.println();
        int[][] tri = new int[6][];
        for (int i = 0; i < tri.length; i++) { tri[i] = new int[i + 1]; tri[i][0] = tri[i][i] = 1; for (int j = 1; j < i; j++) tri[i][j] = tri[i - 1][j - 1] + tri[i - 1][j]; }
        System.out.println(Arrays.deepToString(tri));
        int[][][] cube = new int[3][4][5]; cube[2][3][4] = 9;
        char[][] chars = new char[2][3]; boolean[] bools = new boolean[2]; double[] dbl = new double[1]; String[] strs = new String[2];
        System.out.println(cube[2][3][4] + " " + cube[0].length + " " + cube[0][0].length + " " + (int) chars[1][2] + " " + bools[1] + " " + dbl[0] + " " + strs[1]);
        int[] src = {1, 2, 3, 4, 5, 6, 7, 8};
        System.arraycopy(src, 0, src, 2, 5);
        int[] cl = src.clone(); cl[0] = 99;
        System.out.println(Arrays.toString(src) + " " + Arrays.toString(cl));
        System.out.println(varargs() + " " + varargs(1, 2, 3) + " " + over("s") + over(1) + over(1L) + over(1.0) + over((Object) "x"));
        System.out.println(maxOf(List.of(3, 7, 2)) + " " + maxOf(List.of("pear", "apple", "zoo")));
        Integer i1 = 1000, i2 = 1000, i3 = 100, i4 = 100;
        Long big = 5L; Double dd = 2.5; Character ch = 'x'; Boolean bo = true;
        System.out.println((i1 == i2) + " " + i1.equals(i2) + " " + (i3 == i4) + " " + (big + dd) + " " + (char) (ch + 1) + " " + !bo);
        long acc = 0;
        for (int i = 0; i < 3_000_000; i++) { acc += (i ^ (i >>> 3)) * 31L; acc ^= acc << 7; }
        System.out.println(acc);
        int x = 5; x += x++ + ++x; int y = x-- - --x;
        System.out.println(x + " " + y + " " + (x > y ? x : y) + " " + (5 & 3 | 2 ^ 7) + " " + (~5) + " " + (-17 >> 2) + " " + (-17 >>> 28));
        Object[] objs = new Integer[]{1, 2};
        System.out.println(objs.getClass().getSimpleName() + " " + objs.length + " " + (objs instanceof Integer[]) + " " + (objs instanceof Object[]) + " " + new int[0].getClass().getSuperclass());
        StringBuilder fizz = new StringBuilder();
        for (int i = 1; i <= 30; i++) fizz.append(i % 15 == 0 ? "FizzBuzz" : i % 3 == 0 ? "Fizz" : i % 5 == 0 ? "Buzz" : String.valueOf(i)).append(' ');
        System.out.println(fizz);
        System.out.println(String.format("%5d|%-5s|%05.2f|%x|%X|%o|%e|%.3e|%10.4f|%b|%c|%%|%n", 42, "ab", 3.14159, 255, 255, 8, 12345.678, 0.000123, Math.PI, true, 'z').trim());
        System.out.println(String.format("%,d %+d %(d %s %S %08X %.0f %.1f %.2f %g", 1234567, 5, -3, null, "up", -1, 2.5, 0.05, 1.005, 0.0001234));
        System.out.println(String.format("[%3$s %1$s %2$s] %-8.3s| %h", "a", "b", "c", "abcdef", "hi"));
        System.exit(3);
    }
}
