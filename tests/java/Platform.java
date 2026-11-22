import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.*;
import java.lang.invoke.*;

public class Platform {
    public static void main(String[] args) throws Throwable {
        Runtime.Version v = Runtime.Version.parse("25.0.5+8-LTS");
        System.out.println(v + " " + v.feature() + " " + v.interim() + " " + v.update() + " " + v.patch() + " " + v.build() + " " + v.optional() + " " + v.pre());
        System.out.println(Runtime.Version.parse("21").compareTo(Runtime.Version.parse("21.0.1")) + " " + Runtime.Version.parse("17-ea+5").pre() + " " + Runtime.Version.parse("17+-opt") + " " + Runtime.Version.parse("17+1-opt").optional());
        System.out.println(Runtime.version().feature() >= 21);
        System.Logger lg = System.getLogger("x.y");
        System.out.println(lg.getName() + " " + lg.isLoggable(System.Logger.Level.DEBUG) + " " + lg.isLoggable(System.Logger.Level.ERROR));
        Thread t = Thread.ofVirtual().name("virt-", 3).unstarted(() -> System.out.println("in " + Thread.currentThread().getName()));
        t.start(); t.join();
        ThreadFactory f = Thread.ofPlatform().name("plat").daemon().factory();
        Thread p = f.newThread(() -> {});
        System.out.println(p.getName() + " " + p.isDaemon());
        AtomicLong al = new AtomicLong(5);
        System.out.println(al.weakCompareAndSetVolatile(5, 6) + " " + al.compareAndExchangeAcquire(6, 7) + " " + al.get());
        AtomicReference<String> ar = new AtomicReference<>("a");
        System.out.println(ar.weakCompareAndSetRelease("a", "b") + " " + ar.get());
        StackTraceElement e = new StackTraceElement("ldr", "mod", "1.0", "C", "m", "F.java", 3);
        System.out.println(e.getClassLoaderName() + " " + e.getModuleName() + " " + e.getModuleVersion() + " " + e.getClassName());
        System.out.println(Platform.class.getClassLoader() == ClassLoader.getSystemClassLoader());
        System.out.println(Platform.class.getPackage().getName().isEmpty() + " " + String.class.getPackage().getName());
        MethodHandles.Lookup L = MethodHandles.lookup();
        MethodHandle sum = L.findStatic(Platform.class, "add", MethodType.methodType(int.class, int.class, int.class));
        MethodHandle fin = MethodHandles.tryFinally(L.findStatic(Platform.class, "work", MethodType.methodType(int.class, int.class)),
                L.findStatic(Platform.class, "cleanup", MethodType.methodType(int.class, Throwable.class, int.class, int.class)));
        System.out.println((int) fin.invoke(4));
        MethodHandle ts = MethodHandles.tableSwitch(L.findStatic(Platform.class, "other", MethodType.methodType(String.class, int.class)),
                L.findStatic(Platform.class, "zero", MethodType.methodType(String.class, int.class)),
                L.findStatic(Platform.class, "one", MethodType.methodType(String.class, int.class)));
        System.out.println(ts.invoke(0) + " " + ts.invoke(1) + " " + ts.invoke(9));
        MethodHandle counted = MethodHandles.countedLoop(MethodHandles.constant(int.class, 5), MethodHandles.zero(int.class),
                L.findStatic(Platform.class, "accum", MethodType.methodType(int.class, int.class, int.class)));
        System.out.println((int) counted.invoke());
        System.out.println(MethodHandles.reflectAs(java.lang.reflect.Method.class, sum).getName());
    }
    static int add(int a, int b) { return a + b; }
    static int work(int x) { return x * 10; }
    static int cleanup(Throwable t, int r, int x) { return r + x + (t == null ? 0 : 1000); }
    static String zero(int i) { return "zero"; }
    static String one(int i) { return "one"; }
    static String other(int i) { return "other" + i; }
    static int accum(int v, int i) { return v + i; }
}
