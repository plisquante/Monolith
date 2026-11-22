package java.lang.invoke;

import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;

/**
 * j2zig overlay of MethodHandles. VarHandles resolve to field offsets / static field addresses;
 * method handles are built on the reflection layer (see the MethodHandle overlay and J2zMH).
 * Lookups resolve members reflectively; the transpiler keeps constant lookup targets reachable.
 * Access checks are not enforced (a closed-world image has no untrusted code to defend against).
 */
public class MethodHandles {
    private MethodHandles() {}

    private static final Lookup PUBLIC = new Lookup(Object.class);

    /** Caller-sensitive, like the JDK: the lookup class is the class that called lookup(). */
    public static Lookup lookup() {
        Class<?> caller = jdk.internal.reflect.Reflection.getCallerClass();
        return new Lookup(caller != null ? caller : Object.class);
    }

    public static Lookup publicLookup() {
        return PUBLIC;
    }

    public static Lookup privateLookupIn(Class<?> targetClass, Lookup caller) {
        return new Lookup(targetClass);
    }

    public static VarHandle arrayElementVarHandle(Class<?> arrayClass) {
        if (!arrayClass.isArray()) throw new IllegalArgumentException("not an array class: " + arrayClass.getName());
        return new VarHandle(VarHandle.ARRAY, 0, arrayClass.getComponentType(), arrayClass, false);
    }

    /** Views a byte[] as elements of viewArrayClass's component type (plain get/set, any alignment). */
    public static VarHandle byteArrayViewVarHandle(Class<?> viewArrayClass, java.nio.ByteOrder byteOrder) {
        if (!viewArrayClass.isArray()) throw new IllegalArgumentException("not an array class: " + viewArrayClass.getName());
        return new VarHandle(VarHandle.BYTE_VIEW, 0, viewArrayClass.getComponentType(), byte[].class, false,
                byteOrder == java.nio.ByteOrder.BIG_ENDIAN);
    }

    // ------------------------------------------------------------------ combinators
    private static Object[] convertAll(Object[] values, MethodType t, int pos) {
        Object[] r = new Object[values.length];
        for (int i = 0; i < values.length; i++) r[i] = MethodHandle.convert(values[i], t.parameterType(pos + i), false);
        return r;
    }

    public static MethodHandle insertArguments(MethodHandle target, int pos, Object... values) {
        MethodType t = target.type();
        if (pos < 0 || pos + values.length > t.parameterCount()) throw new IllegalArgumentException("too many values to insert");
        return new J2zMH.Bound(target, pos, convertAll(values, t, pos), t.dropParameterTypes(pos, pos + values.length));
    }

    public static MethodHandle dropArguments(MethodHandle target, int pos, java.util.List<Class<?>> valueTypes) {
        return new J2zMH.Drop(target, pos, valueTypes.size(), target.type().insertParameterTypes(pos, valueTypes));
    }

    public static MethodHandle dropArguments(MethodHandle target, int pos, Class<?>... valueTypes) {
        return dropArguments(target, pos, java.util.Arrays.asList(valueTypes));
    }

    public static MethodHandle dropReturn(MethodHandle target) {
        return target.asType(target.type().changeReturnType(void.class));
    }

    public static MethodHandle constant(Class<?> type, Object value) {
        if (type == void.class) throw new IllegalArgumentException("void type");
        return new J2zMH.Constant(MethodHandle.convert(value, type, false), MethodType.methodType(type));
    }

    public static MethodHandle identity(Class<?> type) {
        if (type == void.class) throw new IllegalArgumentException("void type");
        return new J2zMH.Identity(MethodType.methodType(type, type));
    }

    public static MethodHandle zero(Class<?> type) {
        return new J2zMH.Constant(MethodHandle.zero(type), MethodType.methodType(type));
    }

    public static MethodHandle empty(MethodType type) {
        return dropArguments(zero(type.returnType()), 0, type.parameterList());
    }

    public static MethodHandle explicitCastArguments(MethodHandle target, MethodType newType) {
        if (newType.parameterCount() != target.type().parameterCount())
            throw new WrongMethodTypeException("cannot explicitly cast " + target + " to " + newType);
        return new J2zMH.AsType(target, newType, true);
    }

    public static MethodHandle filterArguments(MethodHandle target, int pos, MethodHandle... filters) {
        MethodType t = target.type();
        for (int i = 0; i < filters.length; i++)
            if (filters[i] != null) t = t.changeParameterType(pos + i, filters[i].type().parameterType(0));
        return new J2zMH.FilterArgs(target, pos, filters.clone(), t);
    }

    public static MethodHandle filterReturnValue(MethodHandle target, MethodHandle filter) {
        return new J2zMH.FilterReturn(target, filter, target.type().changeReturnType(filter.type().returnType()));
    }

    public static MethodHandle foldArguments(MethodHandle target, MethodHandle combiner) {
        return foldArguments(target, 0, combiner);
    }

    public static MethodHandle foldArguments(MethodHandle target, int pos, MethodHandle combiner) {
        MethodType t = target.type();
        if (combiner.type().returnType() != void.class) t = t.dropParameterTypes(pos, pos + 1);
        return new J2zMH.Fold(target, pos, combiner, false, t);
    }

    public static MethodHandle collectArguments(MethodHandle target, int pos, MethodHandle filter) {
        MethodType t = target.type();
        if (filter.type().returnType() != void.class) t = t.dropParameterTypes(pos, pos + 1);
        t = t.insertParameterTypes(pos, filter.type().parameterList());
        return new J2zMH.Fold(target, pos, filter, true, t);
    }

    public static MethodHandle guardWithTest(MethodHandle test, MethodHandle target, MethodHandle fallback) {
        return new J2zMH.Guard(test, target, fallback);
    }

    public static MethodHandle permuteArguments(MethodHandle target, MethodType newType, int... reorder) {
        if (reorder.length != target.type().parameterCount()) throw new IllegalArgumentException("bad reorder array");
        return new J2zMH.Permute(target, newType, reorder.clone());
    }

    public static MethodHandle catchException(MethodHandle target, Class<? extends Throwable> exType, MethodHandle handler) {
        return new J2zMH.Catch(target, exType, handler);
    }

    public static MethodHandle throwException(Class<?> returnType, Class<? extends Throwable> exType) {
        return new J2zMH.Thrower(MethodType.methodType(returnType, exType));
    }

    public static MethodHandle collectReturnValue(MethodHandle target, MethodHandle filter) {
        // filter(result, args...) is the new result: target's result is collected as filter's first argument
        MethodType ft = filter.type();
        return new J2zMH.Fold(target, 0, filter, true, target.type()) {
            @Override Object invokeImpl(Object[] a) throws Throwable {
                Object r = target.invokeImpl(a);
                if (target.type().returnType() == void.class) return filter.invokeImpl(a);
                Object[] b = new Object[a.length + 1];
                b[0] = r;
                System.arraycopy(a, 0, b, 1, a.length);
                return filter.invokeImpl(b);
            }
        };
    }

    public static MethodHandle tryFinally(MethodHandle target, MethodHandle cleanup) {
        return new J2zMH.TryFinally(target, cleanup);
    }

    public static MethodHandle tableSwitch(MethodHandle fallback, MethodHandle... targets) {
        return new J2zMH.TableSwitch(fallback, targets.clone());
    }

    public static MethodHandle dropArguments(MethodHandle target, int pos, Class<?> valueType) {
        return dropArguments(target, pos, java.util.List.of(valueType));
    }

    public static MethodHandle dropArgumentsToMatch(MethodHandle target, int skip, java.util.List<Class<?>> newTypes, int pos) {
        MethodType t = target.type();
        int keep = t.parameterCount() - skip;
        MethodHandle h = target;
        if (pos > 0) h = dropArguments(h, skip, newTypes.subList(0, pos));
        int tail = newTypes.size() - pos - keep;
        if (tail > 0) h = dropArguments(h, skip + pos + keep, newTypes.subList(pos + keep, newTypes.size()));
        return h;
    }

    public static MethodHandle whileLoop(MethodHandle init, MethodHandle pred, MethodHandle body) {
        return new J2zMH.While(init, pred, body, false);
    }

    public static MethodHandle doWhileLoop(MethodHandle init, MethodHandle body, MethodHandle pred) {
        return new J2zMH.While(init, pred, body, true);
    }

    public static MethodHandle countedLoop(MethodHandle iterations, MethodHandle init, MethodHandle body) {
        return new J2zMH.Counted(null, iterations, init, body);
    }

    public static MethodHandle countedLoop(MethodHandle start, MethodHandle end, MethodHandle init, MethodHandle body) {
        return new J2zMH.Counted(start, end, init, body);
    }

    public static MethodHandle iteratedLoop(MethodHandle iterator, MethodHandle init, MethodHandle body) {
        return new J2zMH.Iterated(iterator, init, body);
    }

    public static <T extends java.lang.reflect.Member> T reflectAs(Class<T> expected, MethodHandle target) {
        if (target instanceof J2zMH.DirectMethod d) return expected.cast(d.m);
        if (target instanceof J2zMH.DirectConstructor d) return expected.cast(d.c);
        if (target instanceof J2zMH.FieldAccess d) return expected.cast(d.f);
        throw new IllegalArgumentException("not a direct method handle");
    }

    public static MethodHandle invoker(MethodType type) {
        return new J2zMH.Invoker(type);
    }

    public static MethodHandle exactInvoker(MethodType type) {
        return new J2zMH.Invoker(type);
    }

    public static MethodHandle spreadInvoker(MethodType type, int leadingArgCount) {
        return invoker(type).asSpreader(Object[].class, type.parameterCount() - leadingArgCount);
    }

    public static MethodHandle arrayElementGetter(Class<?> arrayClass) {
        return new J2zMH.ArrayOp(J2zMH.ArrayOp.GET, arrayClass, MethodType.methodType(arrayClass.getComponentType(), arrayClass, int.class));
    }

    public static MethodHandle arrayElementSetter(Class<?> arrayClass) {
        return new J2zMH.ArrayOp(J2zMH.ArrayOp.SET, arrayClass, MethodType.methodType(void.class, arrayClass, int.class, arrayClass.getComponentType()));
    }

    public static MethodHandle arrayLength(Class<?> arrayClass) {
        return new J2zMH.ArrayOp(J2zMH.ArrayOp.LENGTH, arrayClass, MethodType.methodType(int.class, arrayClass));
    }

    public static MethodHandle arrayConstructor(Class<?> arrayClass) {
        return new J2zMH.ArrayOp(J2zMH.ArrayOp.NEW, arrayClass, MethodType.methodType(arrayClass, int.class));
    }

    public static VarHandle filterValue(VarHandle target, MethodHandle filterToTarget, MethodHandle filterFromTarget) {
        return target;
    }

    public static VarHandle insertCoordinates(VarHandle target, int pos, Object... values) {
        return target;
    }

    public static VarHandle collectCoordinates(VarHandle target, int pos, MethodHandle filter) {
        return target;
    }

    private static native long fieldOffset0(Class<?> c, String name, boolean isStatic);
    private static native boolean fieldIsFinal0(Class<?> c, String name, boolean isStatic);

    public static final class Lookup {
        public static final int PUBLIC = 1, PRIVATE = 2, PROTECTED = 4, PACKAGE = 8, MODULE = 16, UNCONDITIONAL = 32, ORIGINAL = 64;
        static final Lookup IMPL_LOOKUP = new Lookup(Object.class);
        private final Class<?> lookupClass;

        Lookup(Class<?> c) {
            lookupClass = c;
        }

        void checkAccess(byte refKind, Class<?> refc, MemberName m) throws IllegalAccessException {}

        public Class<?> lookupClass() {
            return lookupClass;
        }

        public int lookupModes() {
            return PUBLIC | PRIVATE | PROTECTED | PACKAGE | MODULE | ORIGINAL;
        }

        public Lookup in(Class<?> requestedLookupClass) {
            return new Lookup(requestedLookupClass);
        }

        public Lookup dropLookupMode(int modeToDrop) {
            return this;
        }

        public VarHandle findVarHandle(Class<?> recv, String name, Class<?> type) throws NoSuchFieldException, IllegalAccessException {
            long off = fieldOffset0(recv, name, false);
            if (off < 0) throw new NoSuchFieldException(name);
            return new VarHandle(VarHandle.FIELD, off, type, null, fieldIsFinal0(recv, name, false));
        }

        public VarHandle findStaticVarHandle(Class<?> decl, String name, Class<?> type) throws NoSuchFieldException, IllegalAccessException {
            long addr = fieldOffset0(decl, name, true);
            if (addr < 0) throw new NoSuchFieldException(name);
            return new VarHandle(VarHandle.STATIC, addr, type, null, fieldIsFinal0(decl, name, true));
        }

        public Class<?> findClass(String targetName) throws ClassNotFoundException, IllegalAccessException {
            return Class.forName(targetName);
        }

        public Class<?> ensureInitialized(Class<?> targetClass) throws IllegalAccessException {
            try {
                Class.forName(targetClass.getName());
            } catch (ClassNotFoundException e) {
                throw new IllegalAccessException(e.getMessage());
            }
            return targetClass;
        }

        private static boolean same(Method m, String name, MethodType type) {
            return m.getName().equals(name) && m.getReturnType() == type.returnType()
                    && java.util.Arrays.equals(m.getParameterTypes(), type.parameterArray());
        }

        private static Method findMethod(Class<?> refc, String name, MethodType type) throws NoSuchMethodException {
            java.util.ArrayDeque<Class<?>> todo = new java.util.ArrayDeque<>();
            java.util.HashSet<Class<?>> seen = new java.util.HashSet<>();
            for (Class<?> c = refc; c != null; c = c.getSuperclass()) {
                for (Method m : c.getDeclaredMethods()) if (same(m, name, type)) return m;
                for (Class<?> i : c.getInterfaces()) todo.add(i);
            }
            while (!todo.isEmpty()) {
                Class<?> i = todo.poll();
                if (!seen.add(i)) continue;
                for (Method m : i.getDeclaredMethods()) if (same(m, name, type)) return m;
                for (Class<?> j : i.getInterfaces()) todo.add(j);
            }
            if (refc.isInterface()) for (Method m : Object.class.getDeclaredMethods()) if (same(m, name, type)) return m;
            throw new NoSuchMethodException("no such method: " + refc.getName() + "." + name + type);
        }

        private static Field findField(Class<?> refc, String name, Class<?> type, boolean wantStatic) throws NoSuchFieldException, IllegalAccessException {
            for (Class<?> c = refc; c != null; c = c.getSuperclass()) {
                for (Field f : c.getDeclaredFields()) {
                    if (!f.getName().equals(name)) continue;
                    if (f.getType() != type) throw new NoSuchFieldException("no such field: " + refc.getName() + "." + name + "/" + type.getName());
                    if (Modifier.isStatic(f.getModifiers()) != wantStatic)
                        throw new IllegalAccessException("expected " + (wantStatic ? "a static" : "a non-static") + " field: " + f);
                    return f;
                }
            }
            throw new NoSuchFieldException("no such field: " + refc.getName() + "." + name + "/" + type.getName());
        }

        public MethodHandle findVirtual(Class<?> refc, String name, MethodType type) throws NoSuchMethodException, IllegalAccessException {
            Method m = findMethod(refc, name, type);
            if (Modifier.isStatic(m.getModifiers())) throw new IllegalAccessException("no such method: expected instance method " + m);
            return new J2zMH.DirectMethod(m, type.insertParameterTypes(0, refc), true);
        }

        public MethodHandle findStatic(Class<?> refc, String name, MethodType type) throws NoSuchMethodException, IllegalAccessException {
            Method m = findMethod(refc, name, type);
            if (!Modifier.isStatic(m.getModifiers())) throw new IllegalAccessException("no such method: expected static method " + m);
            return new J2zMH.DirectMethod(m, type, false);
        }

        public MethodHandle findSpecial(Class<?> refc, String name, MethodType type, Class<?> specialCaller) throws NoSuchMethodException, IllegalAccessException {
            Method m = findMethod(refc, name, type);
            if (Modifier.isStatic(m.getModifiers())) throw new IllegalAccessException("no such method: expected instance method " + m);
            return new J2zMH.DirectMethod(m, type.insertParameterTypes(0, specialCaller), false);
        }

        public MethodHandle findConstructor(Class<?> refc, MethodType type) throws NoSuchMethodException, IllegalAccessException {
            if (type.returnType() != void.class) throw new NoSuchMethodException("constructor type must return void: " + type);
            for (Constructor<?> c : refc.getDeclaredConstructors())
                if (java.util.Arrays.equals(c.getParameterTypes(), type.parameterArray()))
                    return unreflectConstructor(c);
            throw new NoSuchMethodException("no such constructor: " + refc.getName() + ".<init>" + type);
        }

        public MethodHandle findGetter(Class<?> refc, String name, Class<?> type) throws NoSuchFieldException, IllegalAccessException {
            return unreflectGetter(findField(refc, name, type, false));
        }

        public MethodHandle findSetter(Class<?> refc, String name, Class<?> type) throws NoSuchFieldException, IllegalAccessException {
            return unreflectSetter(findField(refc, name, type, false));
        }

        public MethodHandle findStaticGetter(Class<?> refc, String name, Class<?> type) throws NoSuchFieldException, IllegalAccessException {
            return unreflectGetter(findField(refc, name, type, true));
        }

        public MethodHandle findStaticSetter(Class<?> refc, String name, Class<?> type) throws NoSuchFieldException, IllegalAccessException {
            return unreflectSetter(findField(refc, name, type, true));
        }

        public MethodHandle bind(Object receiver, String name, MethodType type) throws NoSuchMethodException, IllegalAccessException {
            return findVirtual(receiver.getClass(), name, type).bindTo(receiver);
        }

        public MethodHandle unreflect(Method m) throws IllegalAccessException {
            MethodType t = MethodType.methodType(m.getReturnType(), m.getParameterTypes());
            MethodHandle mh = Modifier.isStatic(m.getModifiers()) ? new J2zMH.DirectMethod(m, t, false)
                    : new J2zMH.DirectMethod(m, t.insertParameterTypes(0, m.getDeclaringClass()), true);
            return m.isVarArgs() ? mh.asVarargsCollector(m.getParameterTypes()[m.getParameterCount() - 1]) : mh;
        }

        public MethodHandle unreflectSpecial(Method m, Class<?> specialCaller) throws IllegalAccessException {
            MethodType t = MethodType.methodType(m.getReturnType(), m.getParameterTypes());
            return new J2zMH.DirectMethod(m, t.insertParameterTypes(0, specialCaller), false);
        }

        public MethodHandle unreflectConstructor(Constructor<?> c) throws IllegalAccessException {
            MethodHandle mh = new J2zMH.DirectConstructor(c, MethodType.methodType(c.getDeclaringClass(), c.getParameterTypes()));
            return c.isVarArgs() ? mh.asVarargsCollector(c.getParameterTypes()[c.getParameterCount() - 1]) : mh;
        }

        public MethodHandle unreflectGetter(Field f) throws IllegalAccessException {
            MethodType t = Modifier.isStatic(f.getModifiers()) ? MethodType.methodType(f.getType())
                    : MethodType.methodType(f.getType(), f.getDeclaringClass());
            return new J2zMH.FieldAccess(f, false, t);
        }

        public MethodHandle unreflectSetter(Field f) throws IllegalAccessException {
            MethodType t = Modifier.isStatic(f.getModifiers()) ? MethodType.methodType(void.class, f.getType())
                    : MethodType.methodType(void.class, f.getDeclaringClass(), f.getType());
            return new J2zMH.FieldAccess(f, true, t);
        }

        public VarHandle unreflectVarHandle(Field f) throws IllegalAccessException {
            try {
                return Modifier.isStatic(f.getModifiers()) ? findStaticVarHandle(f.getDeclaringClass(), f.getName(), f.getType())
                        : findVarHandle(f.getDeclaringClass(), f.getName(), f.getType());
            } catch (NoSuchFieldException e) {
                throw new IllegalAccessException(e.getMessage());
            }
        }

        public <T> Class<T> accessClass(Class<T> targetClass) {
            return targetClass;
        }

        public boolean hasFullPrivilegeAccess() {
            return true;
        }

        public String toString() {
            return lookupClass.getName();
        }
    }
}
