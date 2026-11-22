import java.lang.annotation.*;
import java.lang.reflect.*;
import java.util.*;

public class Annot {
    enum Color { RED, GREEN }

    @Retention(RetentionPolicy.RUNTIME) @Inherited @Target({ElementType.TYPE, ElementType.METHOD, ElementType.FIELD, ElementType.PARAMETER})
    @interface Info {
        String value() default "none";
        int level() default 3;
        long big() default 1L << 40;
        double ratio() default 0.5;
        float f() default -0.0f;
        char ch() default 'q';
        boolean on() default true;
        Color color() default Color.GREEN;
        Class<?> type() default Object.class;
        String[] tags() default {};
        Tag nested() default @Tag("inner");
    }

    @Retention(RetentionPolicy.RUNTIME) @interface Tag { String value(); }

    @Retention(RetentionPolicy.CLASS) @interface Invisible {}

    interface Greeter { String greet(String who); int count(); default String twice(String w) { return greet(w) + greet(w); } }

    @Info(value = "base", level = 7, tags = {"a", "b"}, color = Color.RED, type = String.class)
    static class Base<T extends Comparable<T>> implements Comparable<Base<T>> {
        @Info("field") @Invisible public List<Map<String, Integer>> data;
        public int compareTo(Base<T> o) { return 0; }
        @Info(level = 1) public <E extends Number> List<E> make(@Tag("p0") Map<String, E> in, int x) throws java.io.IOException { return null; }
    }

    static class Derived extends Base<String> { }

    class Inner { }

    public static void main(String[] args) throws Exception {
        Info i = Base.class.getAnnotation(Info.class);
        System.out.println(i.value() + " " + i.level() + " " + i.big() + " " + i.ratio() + " " + i.f() + " " + i.ch() + " " + i.on()
                + " " + i.color() + " " + i.type().getName() + " " + Arrays.toString(i.tags()) + " " + i.nested().value());
        System.out.println(i.annotationType().getSimpleName() + " " + i.equals(Base.class.getAnnotation(Info.class)) + " " + (i.hashCode() == Base.class.getAnnotation(Info.class).hashCode()));
        System.out.println("inherited: " + (Derived.class.getAnnotation(Info.class) != null) + " declared: " + (Derived.class.getDeclaredAnnotation(Info.class) != null));
        Field f = Base.class.getField("data");
        System.out.println(f.getAnnotation(Info.class).value() + " " + f.getAnnotations().length + " " + f.getGenericType());
        Method m = Base.class.getMethod("make", Map.class, int.class);
        System.out.println(m.getAnnotation(Info.class).level() + " " + m.getGenericReturnType() + " " + Arrays.toString(m.getGenericParameterTypes()));
        System.out.println(Arrays.toString(m.getTypeParameters()) + " " + Arrays.toString(m.getExceptionTypes()));
        System.out.println(((Tag) m.getParameterAnnotations()[0][0]).value() + " " + m.getParameterAnnotations()[1].length);
        System.out.println(Info.class.getMethod("level").getDefaultValue());
        System.out.println(Derived.class.getGenericSuperclass() + " " + Arrays.toString(Base.class.getGenericInterfaces()) + " " + Arrays.toString(Base.class.getTypeParameters()));
        System.out.println(Inner.class.getSimpleName() + " " + Inner.class.getCanonicalName() + " " + Inner.class.isMemberClass() + " " + Inner.class.getDeclaringClass().getSimpleName() + " " + Modifier.toString(Base.class.getModifiers()));
        Runnable anon = new Runnable() { public void run() {} };
        class Local { }
        System.out.println("[" + anon.getClass().getSimpleName() + "] " + anon.getClass().isAnonymousClass() + " " + Local.class.getSimpleName() + " " + Local.class.isLocalClass() + " " + Local.class.getEnclosingMethod().getName() + " " + anon.getClass().getCanonicalName());
        Greeter g = (Greeter) Proxy.newProxyInstance(Annot.class.getClassLoader(), new Class<?>[]{Greeter.class}, (proxy, method, a) -> {
            switch (method.getName()) {
                case "greet": return "hello " + a[0];
                case "count": return 42;
                case "twice": return InvocationHandler.invokeDefault(proxy, method, a);
                case "toString": return "GreeterProxy";
                default: throw new UnsupportedOperationException(method.getName());
            }
        });
        System.out.println(g.greet("x") + " " + g.count() + " " + g.twice("y") + " " + g + " " + Proxy.isProxyClass(g.getClass()));
        try { g.hashCode(); } catch (UnsupportedOperationException e) { System.out.println("UOE " + e.getMessage()); }
    }
}
