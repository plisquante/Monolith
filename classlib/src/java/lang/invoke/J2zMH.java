package java.lang.invoke;

import java.lang.reflect.Array;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.util.Arrays;

/** j2zig: method handle implementations (see the MethodHandle overlay). */
final class J2zMH {
    private J2zMH() {}

    private static final jdk.internal.misc.Unsafe U = jdk.internal.misc.Unsafe.getUnsafe();

    /** Calls the compiled method of m (virtual dispatch on the receiver when dispatch). */
    static native Object invokeMethod0(Method m, Object recv, Object[] args, boolean dispatch) throws Throwable;

    static native Object newInstance0(Constructor<?> c, Object[] args) throws Throwable;

    static Object[] rest(Object[] a, int from) {
        return Arrays.copyOfRange(a, from, a.length);
    }

    // ------------------------------------------------------------------ direct handles
    static final class DirectMethod extends MethodHandle {
        final Method m;
        final boolean isStatic;
        final boolean dispatch;
        private volatile boolean initialized;

        DirectMethod(Method m, MethodType type, boolean dispatch) {
            super(type);
            this.m = m;
            this.isStatic = Modifier.isStatic(m.getModifiers());
            this.dispatch = dispatch && !isStatic && !Modifier.isPrivate(m.getModifiers());
        }

        Object invokeImpl(Object[] a) throws Throwable {
            if (isStatic) {
                if (!initialized) {
                    U.ensureClassInitialized(m.getDeclaringClass());
                    initialized = true;
                }
                return invokeMethod0(m, null, a, false);
            }
            Object recv = a[0];
            if (recv == null) throw new NullPointerException("receiver of " + m.getName() + " is null");
            return invokeMethod0(m, recv, rest(a, 1), dispatch);
        }

        public String toString() {
            return "MethodHandle" + type();
        }
    }

    static final class DirectConstructor extends MethodHandle {
        final Constructor<?> c;
        private volatile boolean initialized;

        DirectConstructor(Constructor<?> c, MethodType type) {
            super(type);
            this.c = c;
        }

        Object invokeImpl(Object[] a) throws Throwable {
            if (!initialized) {
                U.ensureClassInitialized(c.getDeclaringClass());
                initialized = true;
            }
            return newInstance0(c, a);
        }
    }

    static final class FieldAccess extends MethodHandle {
        final Field f;
        final boolean setter;
        final boolean isStatic;

        FieldAccess(Field f, boolean setter, MethodType type) {
            super(type);
            f.setAccessible(true);
            this.f = f;
            this.setter = setter;
            this.isStatic = Modifier.isStatic(f.getModifiers());
        }

        Object invokeImpl(Object[] a) throws Throwable {
            Object recv = isStatic ? null : a[0];
            if (!isStatic && recv == null) throw new NullPointerException();
            if (setter) {
                f.set(recv, a[isStatic ? 0 : 1]);
                return null;
            }
            return f.get(recv);
        }
    }

    // ------------------------------------------------------------------ adapters
    static final class AsType extends MethodHandle {
        final MethodHandle target;
        final boolean explicit;

        AsType(MethodHandle target, MethodType newType, boolean explicit) {
            super(newType);
            this.target = target;
            this.explicit = explicit;
        }

        Object invokeImpl(Object[] a) throws Throwable {
            MethodType tt = target.type();
            Object[] b = new Object[a.length];
            for (int i = 0; i < a.length; i++) b[i] = MethodHandle.convert(a[i], tt.parameterType(i), explicit);
            Object r = target.invokeImpl(b);
            Class<?> rt = type().returnType();
            if (rt == void.class) return null;
            if (tt.returnType() == void.class) return MethodHandle.zero(rt);
            return MethodHandle.convert(r, rt, explicit);
        }
    }

    static final class Bound extends MethodHandle {
        final MethodHandle target;
        final int pos;
        final Object[] values;

        Bound(MethodHandle target, int pos, Object[] values, MethodType type) {
            super(type);
            this.target = target;
            this.pos = pos;
            this.values = values;
        }

        Object invokeImpl(Object[] a) throws Throwable {
            Object[] b = new Object[a.length + values.length];
            System.arraycopy(a, 0, b, 0, pos);
            System.arraycopy(values, 0, b, pos, values.length);
            System.arraycopy(a, pos, b, pos + values.length, a.length - pos);
            return target.invokeImpl(b);
        }
    }

    static final class Drop extends MethodHandle {
        final MethodHandle target;
        final int pos;
        final int count;

        Drop(MethodHandle target, int pos, int count, MethodType type) {
            super(type);
            this.target = target;
            this.pos = pos;
            this.count = count;
        }

        Object invokeImpl(Object[] a) throws Throwable {
            Object[] b = new Object[a.length - count];
            System.arraycopy(a, 0, b, 0, pos);
            System.arraycopy(a, pos + count, b, pos, a.length - pos - count);
            return target.invokeImpl(b);
        }
    }

    static final class Constant extends MethodHandle {
        final Object value;

        Constant(Object value, MethodType type) {
            super(type);
            this.value = value;
        }

        Object invokeImpl(Object[] a) {
            return value;
        }
    }

    static final class Identity extends MethodHandle {
        Identity(MethodType type) {
            super(type);
        }

        Object invokeImpl(Object[] a) {
            return a[0];
        }
    }

    static final class FilterArgs extends MethodHandle {
        final MethodHandle target;
        final int pos;
        final MethodHandle[] filters;

        FilterArgs(MethodHandle target, int pos, MethodHandle[] filters, MethodType type) {
            super(type);
            this.target = target;
            this.pos = pos;
            this.filters = filters;
        }

        Object invokeImpl(Object[] a) throws Throwable {
            Object[] b = a.clone();
            for (int i = 0; i < filters.length; i++) {
                MethodHandle f = filters[i];
                if (f != null) b[pos + i] = f.invokeImpl(new Object[]{a[pos + i]});
            }
            return target.invokeImpl(b);
        }
    }

    static final class FilterReturn extends MethodHandle {
        final MethodHandle target;
        final MethodHandle filter;

        FilterReturn(MethodHandle target, MethodHandle filter, MethodType type) {
            super(type);
            this.target = target;
            this.filter = filter;
        }

        Object invokeImpl(Object[] a) throws Throwable {
            Object r = target.invokeImpl(a);
            return filter.invokeImpl(target.type().returnType() == void.class ? new Object[0] : new Object[]{r});
        }
    }

    /** foldArguments / collectArguments: combiner(args[pos..pos+n)) result inserted at pos. */
    static class Fold extends MethodHandle {
        final MethodHandle target;
        final MethodHandle combiner;
        final int pos;
        final boolean consume; // collectArguments consumes the combiner's arguments

        Fold(MethodHandle target, int pos, MethodHandle combiner, boolean consume, MethodType type) {
            super(type);
            this.target = target;
            this.pos = pos;
            this.combiner = combiner;
            this.consume = consume;
        }

        Object invokeImpl(Object[] a) throws Throwable {
            int n = combiner.type().parameterCount();
            Object r = combiner.invokeImpl(Arrays.copyOfRange(a, pos, pos + n));
            boolean hasResult = combiner.type().returnType() != void.class;
            int keepTail = consume ? pos + n : pos;
            Object[] b = new Object[pos + (hasResult ? 1 : 0) + (a.length - keepTail)];
            System.arraycopy(a, 0, b, 0, pos);
            if (hasResult) b[pos] = r;
            System.arraycopy(a, keepTail, b, pos + (hasResult ? 1 : 0), a.length - keepTail);
            return target.invokeImpl(b);
        }
    }

    static final class Guard extends MethodHandle {
        final MethodHandle test, target, fallback;

        Guard(MethodHandle test, MethodHandle target, MethodHandle fallback) {
            super(target.type());
            this.test = test;
            this.target = target;
            this.fallback = fallback;
        }

        Object invokeImpl(Object[] a) throws Throwable {
            int n = test.type().parameterCount();
            boolean ok = (Boolean) test.invokeImpl(Arrays.copyOf(a, n));
            return ok ? target.invokeImpl(a) : fallback.invokeImpl(a);
        }
    }

    static final class Permute extends MethodHandle {
        final MethodHandle target;
        final int[] reorder;

        Permute(MethodHandle target, MethodType newType, int[] reorder) {
            super(newType);
            this.target = target;
            this.reorder = reorder;
        }

        Object invokeImpl(Object[] a) throws Throwable {
            Object[] b = new Object[reorder.length];
            for (int i = 0; i < reorder.length; i++) b[i] = a[reorder[i]];
            return target.invokeImpl(b);
        }
    }

    static final class Catch extends MethodHandle {
        final MethodHandle target, handler;
        final Class<? extends Throwable> exType;

        Catch(MethodHandle target, Class<? extends Throwable> exType, MethodHandle handler) {
            super(target.type());
            this.target = target;
            this.exType = exType;
            this.handler = handler;
        }

        Object invokeImpl(Object[] a) throws Throwable {
            try {
                return target.invokeImpl(a);
            } catch (Throwable t) {
                if (!exType.isInstance(t)) throw t;
                int n = handler.type().parameterCount();
                Object[] b = new Object[n];
                b[0] = t;
                System.arraycopy(a, 0, b, 1, n - 1);
                return handler.invokeImpl(b);
            }
        }
    }

    static final class Thrower extends MethodHandle {
        Thrower(MethodType type) {
            super(type);
        }

        Object invokeImpl(Object[] a) throws Throwable {
            throw (Throwable) a[0];
        }
    }

    static final class Spreader extends MethodHandle {
        final MethodHandle target;
        final int pos, len;

        Spreader(MethodHandle target, int pos, Class<?> arrayType, int len) {
            super(spreadType(target.type(), pos, arrayType, len));
            this.target = target;
            this.pos = pos;
            this.len = len;
        }

        static MethodType spreadType(MethodType t, int pos, Class<?> arrayType, int len) {
            return t.dropParameterTypes(pos, pos + len).insertParameterTypes(pos, arrayType);
        }

        Object invokeImpl(Object[] a) throws Throwable {
            Object arr = a[pos];
            int have = arr == null ? 0 : Array.getLength(arr);
            if (have != len) throw new IllegalArgumentException("array is not of length " + len);
            Object[] b = new Object[a.length - 1 + len];
            System.arraycopy(a, 0, b, 0, pos);
            MethodType tt = target.type();
            for (int i = 0; i < len; i++) b[pos + i] = MethodHandle.convert(Array.get(arr, i), tt.parameterType(pos + i), false);
            System.arraycopy(a, pos + 1, b, pos + len, a.length - pos - 1);
            return target.invokeImpl(b);
        }
    }

    static final class Collector extends MethodHandle {
        final MethodHandle target;
        final int pos, len;
        final Class<?> arrayType;

        Collector(MethodHandle target, int pos, Class<?> arrayType, int len) {
            super(collectType(target.type(), pos, arrayType, len));
            this.target = target;
            this.pos = pos;
            this.len = len;
            this.arrayType = arrayType;
        }

        static MethodType collectType(MethodType t, int pos, Class<?> arrayType, int len) {
            Class<?>[] elems = new Class<?>[len];
            Arrays.fill(elems, arrayType.getComponentType());
            return t.dropParameterTypes(pos, pos + 1).insertParameterTypes(pos, elems);
        }

        Object invokeImpl(Object[] a) throws Throwable {
            Object arr = Array.newInstance(arrayType.getComponentType(), len);
            for (int i = 0; i < len; i++) Array.set(arr, i, a[pos + i]);
            Object[] b = new Object[a.length - len + 1];
            System.arraycopy(a, 0, b, 0, pos);
            b[pos] = arr;
            System.arraycopy(a, pos + len, b, pos + 1, a.length - pos - len);
            return target.invokeImpl(b);
        }
    }

    static final class Varargs extends MethodHandle {
        final MethodHandle target;

        Varargs(MethodHandle target, Class<?> arrayType) {
            super(target.type().changeParameterType(target.type().parameterCount() - 1, arrayType));
            this.target = target;
        }

        public boolean isVarargsCollector() {
            return true;
        }

        public MethodHandle asFixedArity() {
            return target;
        }

        Object invokeImpl(Object[] a) throws Throwable {
            return target.invokeImpl(a);
        }
    }

    /** MethodHandles.invoker / exactInvoker: leading MethodHandle argument. */
    static final class Invoker extends MethodHandle {
        Invoker(MethodType type) {
            super(type.insertParameterTypes(0, MethodHandle.class));
        }

        Object invokeImpl(Object[] a) throws Throwable {
            return ((MethodHandle) a[0]).j2zInvoke(rest(a, 1));
        }
    }

    static final class ArrayOp extends MethodHandle {
        static final int GET = 0, SET = 1, LENGTH = 2, NEW = 3;
        final int op;
        final Class<?> arrayClass;

        ArrayOp(int op, Class<?> arrayClass, MethodType type) {
            super(type);
            this.op = op;
            this.arrayClass = arrayClass;
        }

        Object invokeImpl(Object[] a) {
            switch (op) {
                case GET: return Array.get(java.util.Objects.requireNonNull(a[0]), (Integer) a[1]);
                case SET: Array.set(java.util.Objects.requireNonNull(a[0]), (Integer) a[1], a[2]); return null;
                case LENGTH: return Array.getLength(java.util.Objects.requireNonNull(a[0]));
                default: return Array.newInstance(arrayClass.getComponentType(), (Integer) a[0]);
            }
        }
    }

    static final class TryFinally extends MethodHandle {
        final MethodHandle target, cleanup;

        TryFinally(MethodHandle target, MethodHandle cleanup) {
            super(target.type());
            this.target = target;
            this.cleanup = cleanup;
        }

        Object invokeImpl(Object[] a) throws Throwable {
            Throwable t = null;
            Object r = null;
            boolean isVoid = target.type().returnType() == void.class;
            try {
                r = target.invokeImpl(a);
            } catch (Throwable x) {
                t = x;
            }
            int n = cleanup.type().parameterCount();
            Object[] b = new Object[n];
            int k = 0;
            b[k++] = t;
            if (!isVoid) b[k++] = t == null ? r : MethodHandle.zero(target.type().returnType());
            System.arraycopy(a, 0, b, k, n - k);
            Object c = cleanup.invokeImpl(b);
            return c;
        }
    }

    static final class TableSwitch extends MethodHandle {
        final MethodHandle fallback;
        final MethodHandle[] targets;

        TableSwitch(MethodHandle fallback, MethodHandle[] targets) {
            super(fallback.type());
            this.fallback = fallback;
            this.targets = targets;
        }

        Object invokeImpl(Object[] a) throws Throwable {
            int i = (Integer) a[0];
            return (i >= 0 && i < targets.length ? targets[i] : fallback).invokeImpl(a);
        }
    }

    static final class While extends MethodHandle {
        final MethodHandle init, pred, body;
        final boolean doWhile;

        While(MethodHandle init, MethodHandle pred, MethodHandle body, boolean doWhile) {
            super(body.type().returnType() == void.class ? body.type() : body.type().dropParameterTypes(0, 1));
            this.init = init;
            this.pred = pred;
            this.body = body;
            this.doWhile = doWhile;
        }

        private Object[] withV(Object v, Object[] a, MethodHandle h) {
            boolean hasV = body.type().returnType() != void.class;
            int n = h.type().parameterCount();
            Object[] b = new Object[n];
            int k = 0;
            if (hasV) b[k++] = v;
            System.arraycopy(a, 0, b, k, n - k);
            return b;
        }

        Object invokeImpl(Object[] a) throws Throwable {
            boolean hasV = body.type().returnType() != void.class;
            Object v = hasV ? (init != null ? init.invokeImpl(Arrays.copyOf(a, init.type().parameterCount())) : MethodHandle.zero(body.type().returnType())) : null;
            if (!hasV && init != null) init.invokeImpl(Arrays.copyOf(a, init.type().parameterCount()));
            if (doWhile) {
                do {
                    Object r = body.invokeImpl(withV(v, a, body));
                    if (hasV) v = r;
                } while ((Boolean) pred.invokeImpl(withV(v, a, pred)));
            } else {
                while ((Boolean) pred.invokeImpl(withV(v, a, pred))) {
                    Object r = body.invokeImpl(withV(v, a, body));
                    if (hasV) v = r;
                }
            }
            return v;
        }
    }

    static final class Counted extends MethodHandle {
        final MethodHandle start, end, init, body;

        Counted(MethodHandle start, MethodHandle end, MethodHandle init, MethodHandle body) {
            super(counterType(end, body));
            this.start = start;
            this.end = end;
            this.init = init;
            this.body = body;
        }

        static MethodType counterType(MethodHandle end, MethodHandle body) {
            MethodType bt = body.type();
            boolean hasV = bt.returnType() != void.class;
            return bt.dropParameterTypes(0, hasV ? 2 : 1).changeReturnType(bt.returnType());
        }

        Object invokeImpl(Object[] a) throws Throwable {
            boolean hasV = body.type().returnType() != void.class;
            int s = start == null ? 0 : (Integer) start.invokeImpl(Arrays.copyOf(a, start.type().parameterCount()));
            int e = (Integer) end.invokeImpl(Arrays.copyOf(a, end.type().parameterCount()));
            Object v = hasV ? (init != null ? init.invokeImpl(Arrays.copyOf(a, init.type().parameterCount())) : MethodHandle.zero(body.type().returnType())) : null;
            for (int i = s; i < e; i++) {
                Object[] b = new Object[body.type().parameterCount()];
                int k = 0;
                if (hasV) b[k++] = v;
                b[k++] = i;
                System.arraycopy(a, 0, b, k, b.length - k);
                Object r = body.invokeImpl(b);
                if (hasV) v = r;
            }
            return v;
        }
    }

    static final class Iterated extends MethodHandle {
        final MethodHandle iterator, init, body;

        Iterated(MethodHandle iterator, MethodHandle init, MethodHandle body) {
            super(body.type().returnType() == void.class ? iterType(iterator, body, false) : iterType(iterator, body, true));
            this.iterator = iterator;
            this.init = init;
            this.body = body;
        }

        static MethodType iterType(MethodHandle iterator, MethodHandle body, boolean hasV) {
            MethodType bt = body.type();
            return bt.dropParameterTypes(0, hasV ? 2 : 1);
        }

        Object invokeImpl(Object[] a) throws Throwable {
            boolean hasV = body.type().returnType() != void.class;
            Object src = iterator == null ? a[0] : iterator.invokeImpl(Arrays.copyOf(a, iterator.type().parameterCount()));
            java.util.Iterator<?> it = iterator == null ? ((Iterable<?>) src).iterator() : (java.util.Iterator<?>) src;
            Object v = hasV ? (init != null ? init.invokeImpl(Arrays.copyOf(a, init.type().parameterCount())) : MethodHandle.zero(body.type().returnType())) : null;
            while (it.hasNext()) {
                Object[] b = new Object[body.type().parameterCount()];
                int k = 0;
                if (hasV) b[k++] = v;
                b[k++] = it.next();
                System.arraycopy(a, 0, b, k, b.length - k);
                Object r = body.invokeImpl(b);
                if (hasV) v = r;
            }
            return v;
        }
    }
}
