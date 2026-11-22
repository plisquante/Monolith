public class IfaceInit {
    interface Desc {
        Object D = mk();
        static Object mk() { System.out.println("Desc.<clinit>"); return "descriptor"; }
        default Object get() { return D; }
        void run();
    }
    interface Plain { Object P = mk(); static Object mk() { System.out.println("Plain.<clinit>"); return "p"; } void x(); }

    public static void main(String[] a) {
        Plain pl = () -> { };
        System.out.println("plain made");
        Desc d = () -> { };
        System.out.println("before get");
        System.out.println(d.get());
        Desc d2 = new Desc() { public void run() { } };
        System.out.println(d2.get());
    }
}
