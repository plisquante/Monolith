import java.util.*;
import java.util.stream.*;

public class StackWalk {
    static final StackWalker W = StackWalker.getInstance(StackWalker.Option.RETAIN_CLASS_REFERENCE);

    static class Logger {
        static String create() { return W.getCallerClass().getSimpleName(); }
    }

    static List<String> trace() {
        return StackWalker.getInstance().walk(s -> s.limit(3).map(f -> f.getClassName() + "." + f.getMethodName()).collect(Collectors.toList()));
    }

    static String nested() { return trace().toString(); }

    public static void main(String[] args) throws Exception {
        System.out.println(Logger.create());
        System.out.println(nested());
        Runnable r = () -> System.out.println("lambda caller " + W.getCallerClass().getSimpleName());
        r.run();
        System.out.println(StackWalk.class.getDeclaredMethod("nested").invoke(null));
        StackWalker.getInstance(StackWalker.Option.RETAIN_CLASS_REFERENCE).forEach(f -> System.out.println(f.getDeclaringClass().getSimpleName() + " " + f.getMethodName() + " " + f.getDescriptor()));
        try { StackWalker.getInstance().getCallerClass(); } catch (UnsupportedOperationException e) { System.out.println("UOE"); }
    }
}
