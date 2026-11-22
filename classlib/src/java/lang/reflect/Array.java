package java.lang.reflect;

/**
 * j2zig overlay of java.lang.reflect.Array: element access in Java (typed, with the JLS widening
 * conversions the JDK applies); allocation stays native.
 */
public final class Array {
    private Array() {}

    public static Object newInstance(Class<?> componentType, int length) throws NegativeArraySizeException {
        return newArray(componentType, length);
    }

    public static Object newInstance(Class<?> componentType, int... dimensions) throws IllegalArgumentException, NegativeArraySizeException {
        return multiNewArray(componentType, dimensions);
    }

    public static native int getLength(Object array) throws IllegalArgumentException;

    private static native Object newArray(Class<?> componentType, int length) throws NegativeArraySizeException;

    private static native Object multiNewArray(Class<?> componentType, int[] dimensions) throws IllegalArgumentException, NegativeArraySizeException;

    private static IllegalArgumentException notArray(Object a) {
        if (a == null) throw new NullPointerException();
        return new IllegalArgumentException(a.getClass().isArray() ? "argument type mismatch" : "Argument is not an array");
    }

    public static Object get(Object array, int index) throws IllegalArgumentException, ArrayIndexOutOfBoundsException {
        if (array instanceof Object[] a) return a[index];
        if (array instanceof boolean[] a) return a[index];
        if (array instanceof byte[] a) return a[index];
        if (array instanceof char[] a) return a[index];
        if (array instanceof short[] a) return a[index];
        if (array instanceof int[] a) return a[index];
        if (array instanceof long[] a) return a[index];
        if (array instanceof float[] a) return a[index];
        if (array instanceof double[] a) return a[index];
        throw notArray(array);
    }

    public static boolean getBoolean(Object array, int index) throws IllegalArgumentException, ArrayIndexOutOfBoundsException {
        if (array instanceof boolean[] x) return x[index];
        throw notArray(array);
    }

    public static byte getByte(Object array, int index) throws IllegalArgumentException, ArrayIndexOutOfBoundsException {
        if (array instanceof byte[] x) return x[index];
        throw notArray(array);
    }

    public static char getChar(Object array, int index) throws IllegalArgumentException, ArrayIndexOutOfBoundsException {
        if (array instanceof char[] x) return x[index];
        throw notArray(array);
    }

    public static short getShort(Object array, int index) throws IllegalArgumentException, ArrayIndexOutOfBoundsException {
        if (array instanceof byte[] x) return x[index];
        if (array instanceof short[] x) return x[index];
        throw notArray(array);
    }

    public static int getInt(Object array, int index) throws IllegalArgumentException, ArrayIndexOutOfBoundsException {
        if (array instanceof byte[] x) return x[index];
        if (array instanceof char[] x) return x[index];
        if (array instanceof short[] x) return x[index];
        if (array instanceof int[] x) return x[index];
        throw notArray(array);
    }

    public static long getLong(Object array, int index) throws IllegalArgumentException, ArrayIndexOutOfBoundsException {
        if (array instanceof byte[] x) return x[index];
        if (array instanceof char[] x) return x[index];
        if (array instanceof short[] x) return x[index];
        if (array instanceof int[] x) return x[index];
        if (array instanceof long[] x) return x[index];
        throw notArray(array);
    }

    public static float getFloat(Object array, int index) throws IllegalArgumentException, ArrayIndexOutOfBoundsException {
        if (array instanceof byte[] x) return x[index];
        if (array instanceof char[] x) return x[index];
        if (array instanceof short[] x) return x[index];
        if (array instanceof int[] x) return x[index];
        if (array instanceof long[] x) return x[index];
        if (array instanceof float[] x) return x[index];
        throw notArray(array);
    }

    public static double getDouble(Object array, int index) throws IllegalArgumentException, ArrayIndexOutOfBoundsException {
        if (array instanceof byte[] x) return x[index];
        if (array instanceof char[] x) return x[index];
        if (array instanceof short[] x) return x[index];
        if (array instanceof int[] x) return x[index];
        if (array instanceof long[] x) return x[index];
        if (array instanceof float[] x) return x[index];
        if (array instanceof double[] x) return x[index];
        throw notArray(array);
    }

    public static void set(Object array, int index, Object value) throws IllegalArgumentException, ArrayIndexOutOfBoundsException {
        if (array instanceof Object[] a) {
            if (value != null && !a.getClass().getComponentType().isInstance(value))
                throw new IllegalArgumentException("array element type mismatch");
            a[index] = value;
            return;
        }
        if (array == null) throw new NullPointerException();
        if (!array.getClass().isArray()) throw new IllegalArgumentException("Argument is not an array");
        if (value instanceof Boolean v) setBoolean(array, index, v);
        else if (value instanceof Byte v) setByte(array, index, v);
        else if (value instanceof Character v) setChar(array, index, v);
        else if (value instanceof Short v) setShort(array, index, v);
        else if (value instanceof Integer v) setInt(array, index, v);
        else if (value instanceof Long v) setLong(array, index, v);
        else if (value instanceof Float v) setFloat(array, index, v);
        else if (value instanceof Double v) setDouble(array, index, v);
        else throw new IllegalArgumentException("argument type mismatch");
    }

    public static void setBoolean(Object array, int index, boolean v) throws IllegalArgumentException, ArrayIndexOutOfBoundsException {
        if (array instanceof boolean[] x) { x[index] = v; return; }
        throw notArray(array);
    }

    public static void setByte(Object array, int index, byte v) throws IllegalArgumentException, ArrayIndexOutOfBoundsException {
        if (array instanceof byte[] x) { x[index] = v; return; }
        if (array instanceof short[] x) { x[index] = v; return; }
        if (array instanceof int[] x) { x[index] = v; return; }
        if (array instanceof long[] x) { x[index] = v; return; }
        if (array instanceof float[] x) { x[index] = v; return; }
        if (array instanceof double[] x) { x[index] = v; return; }
        throw notArray(array);
    }

    public static void setChar(Object array, int index, char v) throws IllegalArgumentException, ArrayIndexOutOfBoundsException {
        if (array instanceof char[] x) { x[index] = v; return; }
        if (array instanceof int[] x) { x[index] = v; return; }
        if (array instanceof long[] x) { x[index] = v; return; }
        if (array instanceof float[] x) { x[index] = v; return; }
        if (array instanceof double[] x) { x[index] = v; return; }
        throw notArray(array);
    }

    public static void setShort(Object array, int index, short v) throws IllegalArgumentException, ArrayIndexOutOfBoundsException {
        if (array instanceof short[] x) { x[index] = v; return; }
        if (array instanceof int[] x) { x[index] = v; return; }
        if (array instanceof long[] x) { x[index] = v; return; }
        if (array instanceof float[] x) { x[index] = v; return; }
        if (array instanceof double[] x) { x[index] = v; return; }
        throw notArray(array);
    }

    public static void setInt(Object array, int index, int v) throws IllegalArgumentException, ArrayIndexOutOfBoundsException {
        if (array instanceof int[] x) { x[index] = v; return; }
        if (array instanceof long[] x) { x[index] = v; return; }
        if (array instanceof float[] x) { x[index] = v; return; }
        if (array instanceof double[] x) { x[index] = v; return; }
        throw notArray(array);
    }

    public static void setLong(Object array, int index, long v) throws IllegalArgumentException, ArrayIndexOutOfBoundsException {
        if (array instanceof long[] x) { x[index] = v; return; }
        if (array instanceof float[] x) { x[index] = v; return; }
        if (array instanceof double[] x) { x[index] = v; return; }
        throw notArray(array);
    }

    public static void setFloat(Object array, int index, float v) throws IllegalArgumentException, ArrayIndexOutOfBoundsException {
        if (array instanceof float[] x) { x[index] = v; return; }
        if (array instanceof double[] x) { x[index] = v; return; }
        throw notArray(array);
    }

    public static void setDouble(Object array, int index, double v) throws IllegalArgumentException, ArrayIndexOutOfBoundsException {
        if (array instanceof double[] x) { x[index] = v; return; }
        throw notArray(array);
    }

}
