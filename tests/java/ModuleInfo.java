public class ModuleInfo {
    public static void main(String[] a) throws Exception {
        Module jb = Runtime.class.getModule();
        System.out.println(jb.getName() + " " + jb.isNamed() + " " + (jb.getLayer() != null));
        Module app = ModuleInfo.class.getModule();
        System.out.println(app.isNamed() + " " + (app.getLayer() == null));
        System.out.println(String.class.getModule() == jb);
        java.lang.reflect.Field f = String.class.getDeclaredField("value");
        try { f.setAccessible(true); System.out.println("accessible"); }
        catch (RuntimeException e) { System.out.println(e.getClass().getName()); }
        System.out.println(jb.isOpen("java.lang", app) + " " + jb.isExported("java.io", app) + " " + jb.isExported("jdk.internal.misc", app));
        System.out.println(java.io.File.class.getConstructor(String.class).newInstance("x").getName());
        java.lang.reflect.Field u = sun.misc.Unsafe.class.getDeclaredField("theUnsafe");
        u.setAccessible(true);
        System.out.println(u.get(null) != null);
    }
}
