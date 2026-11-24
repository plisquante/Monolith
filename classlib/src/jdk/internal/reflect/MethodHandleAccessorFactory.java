package jdk.internal.reflect;

import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import jdk.internal.misc.Unsafe;

/**
 * monolith overlay: reflective access without method handles.
 * Fields are read/written through Unsafe offsets; methods and constructors are invoked by
 * runtime-generated invokers keyed by the method's erased signature (monolith/rt/natives_reflect.rs
 * MONO reflection natives).
 */
final class MethodHandleAccessorFactory {
    private MethodHandleAccessorFactory() {}

    private static final Unsafe U = Unsafe.getUnsafe();

    static MethodAccessorImpl newMethodAccessor(Method method, boolean callerSensitive) {
        return new MethodAccessorImpl() {
            public Object invoke(Object obj, Object[] args) throws InvocationTargetException {
                boolean isStatic = Modifier.isStatic(method.getModifiers());
                if (!isStatic) {
                    if (obj == null) throw new NullPointerException("Cannot invoke \"Object." + method.getName() + "()\" because \"obj\" is null");
                    if (!method.getDeclaringClass().isInstance(obj)) throw new IllegalArgumentException("object is not an instance of declaring class");
                } else {
                    U.ensureClassInitialized(method.getDeclaringClass());
                }
                checkArgs(method.getParameterTypes(), args);
                try {
                    return invoke0(method, obj, args);
                } catch (Throwable t) {
                    throw new InvocationTargetException(t);
                }
            }
        };
    }

    static ConstructorAccessorImpl newConstructorAccessor(Constructor<?> ctor) {
        return new ConstructorAccessorImpl() {
            public Object newInstance(Object[] args) throws InstantiationException, InvocationTargetException {
                if (Modifier.isAbstract(ctor.getDeclaringClass().getModifiers()))
                    throw new InstantiationException(ctor.getDeclaringClass().getName());
                U.ensureClassInitialized(ctor.getDeclaringClass());
                checkArgs(ctor.getParameterTypes(), args);
                try {
                    return newInstance0(ctor, args);
                } catch (Throwable t) {
                    throw new InvocationTargetException(t);
                }
            }
        };
    }

    static ConstructorAccessorImpl newSerializableConstructorAccessor(Class<?> decl, Constructor<?> ctor) {
        throw new UnsupportedOperationException("monolith: serialization constructors");
    }

    static FieldAccessorImpl newFieldAccessor(Field field, boolean isReadOnly) {
        return new MonoFieldAccessor(field, isReadOnly);
    }

    /** Argument checks of Method.invoke (count, unboxing + widening conversion, reference types). */
    static void checkArgs(Class<?>[] ptypes, Object[] args) {
        int n = ptypes.length;
        int have = args == null ? 0 : args.length;
        if (have != n) throw new IllegalArgumentException("wrong number of arguments: " + have + " expected: " + n);
        for (int i = 0; i < n; i++) {
            Class<?> p = ptypes[i];
            Object a = args[i];
            if (p.isPrimitive()) {
                if (a == null || !widens(a.getClass(), p)) throw new IllegalArgumentException("argument type mismatch");
            } else if (a != null && !p.isInstance(a)) {
                throw new IllegalArgumentException("argument type mismatch");
            }
        }
    }

    private static int rank(Class<?> box) {
        if (box == Byte.class) return 1;
        if (box == Short.class) return 2;
        if (box == Character.class) return 3;
        if (box == Integer.class) return 4;
        if (box == Long.class) return 5;
        if (box == Float.class) return 6;
        if (box == Double.class) return 7;
        return -1;
    }

    private static boolean widens(Class<?> box, Class<?> prim) {
        if (prim == boolean.class) return box == Boolean.class;
        int r = rank(box);
        if (r < 0) return false;
        if (prim == byte.class) return r == 1;
        if (prim == short.class) return r == 1 || r == 2;
        if (prim == char.class) return r == 3;
        if (prim == int.class) return r <= 4;
        if (prim == long.class) return r <= 5;
        if (prim == float.class) return r <= 6;
        return true; // double
    }

    /** Dynamic dispatch for non-private instance methods, unboxing/boxing like Method.invoke. */
    private static native Object invoke0(Method m, Object obj, Object[] args) throws Throwable;

    private static native Object newInstance0(Constructor<?> c, Object[] args) throws Throwable;
}
