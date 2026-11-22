package jdk.internal.util;

import java.util.List;
import java.util.function.BiFunction;
import java.util.function.Function;

/**
 * j2zig overlay of jdk.internal.util.Preconditions. Same checks and exception messages as the
 * JDK, but messages are built by concatenation instead of String.format (which would pull the
 * whole java.util.Formatter / Locale machinery into every program).
 */
public class Preconditions {
    public static final BiFunction<String, List<Number>, StringIndexOutOfBoundsException> SIOOBE_FORMATTER =
            (s, l) -> new StringIndexOutOfBoundsException(message(s, l));
    public static final BiFunction<String, List<Number>, ArrayIndexOutOfBoundsException> AIOOBE_FORMATTER =
            (s, l) -> new ArrayIndexOutOfBoundsException(message(s, l));
    public static final BiFunction<String, List<Number>, IndexOutOfBoundsException> IOOBE_FORMATTER =
            (s, l) -> new IndexOutOfBoundsException(message(s, l));

    private Preconditions() {}

    public static <X extends RuntimeException> BiFunction<String, List<Number>, X> outOfBoundsExceptionFormatter(
            Function<String, X> f) {
        return (s, l) -> f.apply(message(s, l));
    }

    private static String message(String checkKind, List<? extends Number> args) {
        if (checkKind == null && args == null) return "Range check failed";
        if (checkKind == null) return "Range check failed: " + args;
        if (args == null) return "Range check failed: " + checkKind;
        switch (checkKind) {
            case "checkIndex":
                return "Index " + args.get(0) + " out of bounds for length " + args.get(1);
            case "checkFromToIndex":
                return "Range [" + args.get(0) + ", " + args.get(1) + ") out of bounds for length " + args.get(2);
            case "checkFromIndexSize":
                return "Range [" + args.get(0) + ", " + args.get(0) + " + " + args.get(1)
                        + ") out of bounds for length " + args.get(2);
            default:
                return "Range check failed: " + checkKind + " " + args;
        }
    }

    private static RuntimeException outOfBounds(BiFunction<String, List<Number>, ? extends RuntimeException> oobef,
                                                String checkKind, Number... args) {
        List<Number> largs = List.of(args);
        RuntimeException e = oobef == null ? null : oobef.apply(checkKind, largs);
        return e == null ? new IndexOutOfBoundsException(message(checkKind, largs)) : e;
    }

    public static <X extends RuntimeException> int checkIndex(int index, int length,
            BiFunction<String, List<Number>, X> oobef) {
        if (index < 0 || index >= length) throw outOfBounds(oobef, "checkIndex", index, length);
        return index;
    }

    public static <X extends RuntimeException> long checkIndex(long index, long length,
            BiFunction<String, List<Number>, X> oobef) {
        if (index < 0 || index >= length) throw outOfBounds(oobef, "checkIndex", index, length);
        return index;
    }

    public static <X extends RuntimeException> int checkFromToIndex(int fromIndex, int toIndex, int length,
            BiFunction<String, List<Number>, X> oobef) {
        if (fromIndex < 0 || fromIndex > toIndex || toIndex > length)
            throw outOfBounds(oobef, "checkFromToIndex", fromIndex, toIndex, length);
        return fromIndex;
    }

    public static <X extends RuntimeException> long checkFromToIndex(long fromIndex, long toIndex, long length,
            BiFunction<String, List<Number>, X> oobef) {
        if (fromIndex < 0 || fromIndex > toIndex || toIndex > length)
            throw outOfBounds(oobef, "checkFromToIndex", fromIndex, toIndex, length);
        return fromIndex;
    }

    public static <X extends RuntimeException> int checkFromIndexSize(int fromIndex, int size, int length,
            BiFunction<String, List<Number>, X> oobef) {
        if ((length | fromIndex | size) < 0 || size > length - fromIndex)
            throw outOfBounds(oobef, "checkFromIndexSize", fromIndex, size, length);
        return fromIndex;
    }

    public static <X extends RuntimeException> long checkFromIndexSize(long fromIndex, long size, long length,
            BiFunction<String, List<Number>, X> oobef) {
        if ((length | fromIndex | size) < 0 || size > length - fromIndex)
            throw outOfBounds(oobef, "checkFromIndexSize", fromIndex, size, length);
        return fromIndex;
    }
}
