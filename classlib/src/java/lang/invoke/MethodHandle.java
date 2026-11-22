package java.lang.invoke;

import java.lang.constant.Constable;
import java.lang.constant.MethodHandleDesc;
import java.util.Arrays;
import java.util.List;
import java.util.Objects;
import java.util.Optional;

/**
 * j2zig overlay of java.lang.invoke.MethodHandle.
 *
 * <p>The native image has no LambdaForms or bytecode spinning. A method handle is a tree of
 * small Java objects evaluated by {@link #invokeImpl}: direct handles call the compiled method
 * through the reflection natives (classes.zig reflectInvoke), combinators transform argument
 * arrays. The signature-polymorphic call sites {@code invokeExact}/{@code invoke} are rewritten by
 * the transpiler (j2zig/program.py: mh_invoker) into static stubs that box the arguments, call
 * {@link #j2zInvoke} and unbox the result with {@code j2zTo*}. {@code invokeExact} is evaluated
 * with {@code invoke} (asType) semantics: correct programs behave identically, programs relying on
 * WrongMethodTypeException for an inexact call site do not get it.
 */
public abstract class MethodHandle implements Constable {
    @java.lang.annotation.Target({java.lang.annotation.ElementType.METHOD})
    @java.lang.annotation.Retention(java.lang.annotation.RetentionPolicy.RUNTIME)
    @interface PolymorphicSignature { }

    private final MethodType type;

    MethodHandle(MethodType type) {
        this.type = Objects.requireNonNull(type);
    }

    public MethodType type() {
        return type;
    }

    /** Arguments are already converted to the parameter types of {@link #type()} (boxed). */
    abstract Object invokeImpl(Object[] args) throws Throwable;

    @PolymorphicSignature
    public final native Object invokeExact(Object... args) throws Throwable;

    @PolymorphicSignature
    public final native Object invoke(Object... args) throws Throwable;

    @PolymorphicSignature
    public final native Object invokeBasic(Object... args) throws Throwable;

    /** Entry point of the transpiler's invoke/invokeExact stubs. */
    final Object j2zInvoke(Object[] args) throws Throwable {
        return invokeImpl(adapt(args));
    }

    private Object[] adapt(Object[] args) {
        int n = type.parameterCount();
        if (isVarargsCollector()) args = collectVarargs(args);
        if (args.length != n)
            throw new WrongMethodTypeException("cannot convert " + this + " to arity " + args.length);
        Object[] a = new Object[n];
        for (int i = 0; i < n; i++) a[i] = convert(args[i], type.parameterType(i), false);
        return a;
    }

    private Object[] collectVarargs(Object[] args) {
        int n = type.parameterCount();
        Class<?> arr = type.parameterType(n - 1);
        if (args.length == n && (args[n - 1] == null || arr.isInstance(args[n - 1]))) return args;
        if (args.length < n - 1) return args;
        int k = args.length - (n - 1);
        Object packed = java.lang.reflect.Array.newInstance(arr.getComponentType(), k);
        for (int i = 0; i < k; i++)
            java.lang.reflect.Array.set(packed, i, convert(args[n - 1 + i], arr.getComponentType(), false));
        Object[] r = Arrays.copyOf(args, n);
        r[n - 1] = packed;
        return r;
    }

    public Object invokeWithArguments(Object... arguments) throws Throwable {
        return j2zInvoke(arguments == null ? new Object[0] : arguments.clone());
    }

    public Object invokeWithArguments(List<?> arguments) throws Throwable {
        return j2zInvoke(arguments.toArray());
    }

    // ---- return-value conversions used by the call-site stubs
    static boolean j2zToBoolean(Object o) { return (Boolean) convert(o, boolean.class, false); }
    static byte j2zToByte(Object o) { return (Byte) convert(o, byte.class, false); }
    static char j2zToChar(Object o) { return (Character) convert(o, char.class, false); }
    static short j2zToShort(Object o) { return (Short) convert(o, short.class, false); }
    static int j2zToInt(Object o) { return (Integer) convert(o, int.class, false); }
    static long j2zToLong(Object o) { return (Long) convert(o, long.class, false); }
    static float j2zToFloat(Object o) { return (Float) convert(o, float.class, false); }
    static double j2zToDouble(Object o) { return (Double) convert(o, double.class, false); }

    private static int rank(Object v) {
        if (v instanceof Byte) return 1;
        if (v instanceof Short) return 2;
        if (v instanceof Character) return 3;
        if (v instanceof Integer) return 4;
        if (v instanceof Long) return 5;
        if (v instanceof Float) return 6;
        if (v instanceof Double) return 7;
        if (v instanceof Boolean) return 0;
        return -1;
    }

    private static int targetRank(Class<?> t) {
        if (t == byte.class) return 1;
        if (t == short.class) return 2;
        if (t == char.class) return 3;
        if (t == int.class) return 4;
        if (t == long.class) return 5;
        if (t == float.class) return 6;
        if (t == double.class) return 7;
        return 0; // boolean
    }

    private static boolean widens(int from, int to) {
        if (from == to) return true;
        if (from == 0 || to == 0) return false;
        if (to == 3 || from == 3 && to == 2) return false; // nothing widens to char; char !-> short
        if (from == 1 && to == 3) return false;
        return from < to;
    }

    /**
     * MethodHandle.asType conversion of a (boxed) value to type t; with {@code explicit},
     * explicitCastArguments rules (primitive narrowing, boolean as the low bit, null to zero).
     */
    static Object convert(Object v, Class<?> t, boolean explicit) {
        if (t == void.class) return null;
        if (!t.isPrimitive()) {
            if (v == null || t.isInstance(v)) return v;
            throw new ClassCastException("Cannot cast " + v.getClass().getName() + " to " + t.getName());
        }
        if (v == null) {
            if (explicit) return zero(t);
            throw new NullPointerException("Cannot unbox null value to " + t.getName());
        }
        int from = rank(v);
        if (from < 0) {
            throw new ClassCastException("Cannot cast " + v.getClass().getName() + " to " + t.getName());
        }
        int to = targetRank(t);
        if (!explicit && !widens(from, to)) {
            throw new WrongMethodTypeException("cannot convert " + v.getClass().getName() + " to " + t.getName());
        }
        if (to == 0) {
            if (from == 0) return v;
            return (longOf(v) & 1) != 0;
        }
        if (from == 0) {
            int b = ((Boolean) v) ? 1 : 0;
            v = b;
        }
        switch (to) {
            case 1: return (byte) longOf(v);
            case 2: return (short) longOf(v);
            case 3: return (char) longOf(v);
            case 4: return (v instanceof Float || v instanceof Double) ? (int) ((Number) v).doubleValue() : (int) longOf(v);
            case 5: return (v instanceof Float || v instanceof Double) ? (long) ((Number) v).doubleValue() : longOf(v);
            case 6: return (v instanceof Double) ? (float) (double) (Double) v : (v instanceof Float) ? v : (float) longOf(v);
            default: return (v instanceof Float || v instanceof Double) ? ((Number) v).doubleValue() : (double) longOf(v);
        }
    }

    private static long longOf(Object v) {
        if (v instanceof Character c) return c;
        if (v instanceof Float || v instanceof Double) return (long) ((Number) v).doubleValue();
        return ((Number) v).longValue();
    }

    static Object zero(Class<?> t) {
        if (!t.isPrimitive() || t == void.class) return null;
        return convert(0, t, true);
    }

    static final class Wrapper {
        static Class<?> unwrap(Class<?> box) {
            if (box == Integer.class) return int.class;
            if (box == Long.class) return long.class;
            if (box == Short.class) return short.class;
            if (box == Byte.class) return byte.class;
            if (box == Character.class) return char.class;
            if (box == Float.class) return float.class;
            if (box == Double.class) return double.class;
            if (box == Boolean.class) return boolean.class;
            return null;
        }
    }

    // ---- adapters
    public MethodHandle asType(MethodType newType) {
        if (newType.equals(type)) return this;
        if (newType.parameterCount() != type.parameterCount())
            throw new WrongMethodTypeException("cannot convert " + this + " to " + newType);
        return new J2zMH.AsType(this, newType, false);
    }

    public MethodHandle bindTo(Object x) {
        Class<?> p0 = type.parameterCount() > 0 ? type.parameterType(0) : null;
        if (p0 == null || p0.isPrimitive())
            throw new IllegalArgumentException("no leading reference parameter: " + x);
        return MethodHandles.insertArguments(this, 0, p0.cast(x));
    }

    public MethodHandle asSpreader(Class<?> arrayType, int arrayLength) {
        return asSpreader(type.parameterCount() - arrayLength, arrayType, arrayLength);
    }

    public MethodHandle asSpreader(int spreadArgPos, Class<?> arrayType, int arrayLength) {
        if (!arrayType.isArray() || arrayLength < 0 || spreadArgPos < 0 || spreadArgPos + arrayLength > type.parameterCount())
            throw new IllegalArgumentException("bad spread array type / length");
        return new J2zMH.Spreader(this, spreadArgPos, arrayType, arrayLength);
    }

    public MethodHandle asCollector(Class<?> arrayType, int arrayLength) {
        return asCollector(type.parameterCount() - 1, arrayType, arrayLength);
    }

    public MethodHandle asCollector(int collectArgPos, Class<?> arrayType, int arrayLength) {
        if (!arrayType.isArray() || arrayLength < 0)
            throw new IllegalArgumentException("bad collect array type / length");
        return new J2zMH.Collector(this, collectArgPos, arrayType, arrayLength);
    }

    public MethodHandle asVarargsCollector(Class<?> arrayType) {
        if (!arrayType.isArray() || type.parameterCount() == 0
                || !type.parameterType(type.parameterCount() - 1).isAssignableFrom(arrayType))
            throw new IllegalArgumentException("not an array type for the last parameter: " + arrayType);
        return new J2zMH.Varargs(this.asFixedArity(), arrayType);
    }

    public boolean isVarargsCollector() {
        return false;
    }

    public MethodHandle withVarargs(boolean makeVarargs) {
        if (makeVarargs == isVarargsCollector()) return this;
        if (makeVarargs) return asVarargsCollector(type.parameterType(type.parameterCount() - 1));
        return asFixedArity();
    }

    public MethodHandle asFixedArity() {
        return this;
    }

    @Override
    public Optional<MethodHandleDesc> describeConstable() {
        return Optional.empty();
    }

    @Override
    public String toString() {
        return "MethodHandle" + type;
    }
}
