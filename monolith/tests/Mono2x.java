public class Mono2x {
    static class P { int[] a = new int[2]; }
    public static void main(String[] args) {
        P p = new P();
        System.out.println("before");
        System.out.println(p.a[1]);
        System.out.println(p.a[5]);
        System.out.println("never");
    }
}
