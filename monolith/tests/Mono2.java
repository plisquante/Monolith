public class Mono2 {
    static int counter;
    static final int[] TABLE = new int[8];
    static long total = 100L;
    static { for (int i = 0; i < TABLE.length; i++) TABLE[i] = i * i; counter = 5; }

    static abstract class Shape {
        int id; static int made;
        Shape(int id) { this.id = id; made++; }
        abstract double area();
        int describe() { return id * 1000 + (int) area(); }
    }
    static class Rect extends Shape {
        double w, h;
        Rect(int id, double w, double h) { super(id); this.w = w; this.h = h; }
        double area() { return w * h; }
    }
    static class Square extends Rect {
        Square(int id, double s) { super(id, s, s); }
        @Override int describe() { return super.describe() + 1; }
    }
    static class Circle extends Shape {
        double r;
        Circle(int id, double r) { super(id); this.r = r; }
        double area() { return 3.0 * r * r; }
    }
    static class Node { int v; Node next; Node(int v, Node n) { this.v = v; this.next = n; } }
    static class Box { boolean z; byte b; char c; short s; long l; float f; double d; Object o; }

    public static void main(String[] args) {
        System.out.println(counter);
        System.out.println(TABLE[7]);
        Shape[] shapes = { new Rect(1, 2.0, 3.0), new Square(2, 4.0), new Circle(3, 2.0) };
        int sum = 0;
        for (Shape s : shapes) sum += s.describe();
        System.out.println(sum);
        System.out.println(Shape.made);
        Object o = shapes[1];
        System.out.println(o instanceof Rect);
        System.out.println(o instanceof Circle);
        Rect rr = (Rect) o;
        System.out.println(rr.id);
        Node head = null;
        for (int i = 0; i < 5; i++) head = new Node(i, head);
        int ns = 0;
        for (Node n = head; n != null; n = n.next) ns = ns * 10 + n.v;
        System.out.println(ns);
        Box b = new Box();
        b.z = true; b.b = (byte) 200; b.c = (char) 66000; b.s = (short) 40000; b.l = 1L << 50; b.f = 1.5f; b.d = 2.25;
        System.out.println(b.z); System.out.println(b.b); System.out.println((int) b.c); System.out.println(b.s);
        System.out.println(b.l); System.out.println((int) (b.f * 4)); System.out.println((int) (b.d * 4)); System.out.println(b.o == null);
        int[][] grid = new int[3][4];
        grid[2][3] = 7; grid[1][1] = grid[2][3] * 2;
        System.out.println(grid[1][1] + grid.length + grid[0].length);
        boolean[] flags = new boolean[3]; flags[1] = true;
        System.out.println(flags[1]); System.out.println(flags[0]);
        byte[] bytes = new byte[2]; bytes[0] = (byte) 255; System.out.println(bytes[0]);
        char[] cs = new char[2]; cs[0] = 'a'; cs[1] = (char) (cs[0] + 2); System.out.println(cs[1]);
        long[] ls = new long[2]; ls[0] = Long.MAX_VALUE; ls[1] = ls[0] + 1; System.out.println(ls[1]);
        double[] ds = {1.5, 2.5}; System.out.println((int) (ds[0] + ds[1]));
        String[] names = new String[2]; System.out.println(names[0] == null); names[1] = "mono"; System.out.println(names[1]);
        System.out.println(total);
    }
}
