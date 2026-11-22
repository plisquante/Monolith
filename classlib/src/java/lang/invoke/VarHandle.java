package java.lang.invoke;

/**
 * j2zig overlay of VarHandle. A VarHandle is a (kind, base, offset, type) descriptor; the
 * signature-polymorphic access methods (get, set, compareAndSet, getAndAdd, ...) are not
 * declared here: the transpiler lowers every invokevirtual of them to a direct atomic
 * memory operation in the runtime (runtime/varhandle.zig), using the call site descriptor.
 */
public class VarHandle {
    static final int FIELD = 0, STATIC = 1, ARRAY = 2, BYTE_VIEW = 3;
    final VarForm vform;
    final int j2zKind;
    final long j2zOffset;      // field offset, or absolute address of a static field
    final Class<?> j2zVarType;
    final Class<?> j2zArrayClass;
    final boolean j2zFinal;
    final boolean j2zBigEndian;

    VarHandle(int kind, long offset, Class<?> varType, Class<?> arrayClass, boolean isFinal) {
        this(kind, offset, varType, arrayClass, isFinal, false);
    }

    VarHandle(int kind, long offset, Class<?> varType, Class<?> arrayClass, boolean isFinal, boolean bigEndian) {
        this.vform = null;
        this.j2zBigEndian = bigEndian;
        this.j2zKind = kind;
        this.j2zOffset = offset;
        this.j2zVarType = varType;
        this.j2zArrayClass = arrayClass;
        this.j2zFinal = isFinal;
    }

    VarHandle(VarForm form) {
        this(form, false);
    }

    VarHandle(VarForm form, boolean exact) {
        this.vform = form;
        this.j2zKind = FIELD;
        this.j2zOffset = 0;
        this.j2zVarType = Object.class;
        this.j2zArrayClass = null;
        this.j2zFinal = exact;
        this.j2zBigEndian = false;
    }

    MethodType accessModeTypeUncached(AccessType at) {
        return MethodType.methodType(at.returnType);
    }

    public final MethodType accessModeType(AccessMode accessMode) {
        return accessModeTypeUncached(accessMode.at);
    }

    final MethodType accessModeType(int accessModeOrdinal) {
        return accessModeType(AccessMode.valueFromOrdinal(accessModeOrdinal));
    }

    VarHandle asDirect() {
        return this;
    }

    public java.util.List<Class<?>> coordinateTypes() {
        return switch (j2zKind) {
            case STATIC -> java.util.Collections.emptyList();
            case ARRAY -> java.util.List.of(j2zArrayClass, int.class);
            case BYTE_VIEW -> java.util.List.of(byte[].class, int.class);
            default -> java.util.List.of(Object.class);
        };
    }

    public Class<?> varType() {
        return j2zVarType;
    }

    public VarHandle withInvokeExactBehavior() {
        return this;
    }

    public VarHandle withInvokeBehavior() {
        return this;
    }

    public boolean hasInvokeExactBehavior() {
        return false;
    }

    public static void fullFence() {
        fence0();
    }

    public static void acquireFence() {
        fence0();
    }

    public static void releaseFence() {
        fence0();
    }

    public static void loadLoadFence() {
        fence0();
    }

    public static void storeStoreFence() {
        fence0();
    }

    private static native void fence0();

    public String toString() {
        return "VarHandle[varType=" + j2zVarType.getName() + "]";
    }

    enum AccessType {
        GET(Object.class),
        SET(void.class),
        COMPARE_AND_SET(boolean.class),
        COMPARE_AND_EXCHANGE(Object.class),
        GET_AND_UPDATE(Object.class);

        static final int COUNT = values().length;
        final Class<?> returnType;
        final boolean isMonomorphicInReturnType;

        AccessType(Class<?> returnType) {
            this.returnType = returnType;
            this.isMonomorphicInReturnType = (returnType != Object.class);
        }

        MethodType accessModeType(Class<?> receiver, Class<?> value, Class<?>... intermediate) {
            return MethodType.methodType(returnType);
        }
    }

    public enum AccessMode {
        GET("get", AccessType.GET),
        SET("set", AccessType.SET),
        GET_VOLATILE("getVolatile", AccessType.GET),
        SET_VOLATILE("setVolatile", AccessType.SET),
        GET_ACQUIRE("getAcquire", AccessType.GET),
        SET_RELEASE("setRelease", AccessType.SET),
        GET_OPAQUE("getOpaque", AccessType.GET),
        SET_OPAQUE("setOpaque", AccessType.SET),
        COMPARE_AND_SET("compareAndSet", AccessType.COMPARE_AND_SET),
        COMPARE_AND_EXCHANGE("compareAndExchange", AccessType.COMPARE_AND_EXCHANGE),
        COMPARE_AND_EXCHANGE_ACQUIRE("compareAndExchangeAcquire", AccessType.COMPARE_AND_EXCHANGE),
        COMPARE_AND_EXCHANGE_RELEASE("compareAndExchangeRelease", AccessType.COMPARE_AND_EXCHANGE),
        WEAK_COMPARE_AND_SET_PLAIN("weakCompareAndSetPlain", AccessType.COMPARE_AND_SET),
        WEAK_COMPARE_AND_SET("weakCompareAndSet", AccessType.COMPARE_AND_SET),
        WEAK_COMPARE_AND_SET_ACQUIRE("weakCompareAndSetAcquire", AccessType.COMPARE_AND_SET),
        WEAK_COMPARE_AND_SET_RELEASE("weakCompareAndSetRelease", AccessType.COMPARE_AND_SET),
        GET_AND_SET("getAndSet", AccessType.GET_AND_UPDATE),
        GET_AND_SET_ACQUIRE("getAndSetAcquire", AccessType.GET_AND_UPDATE),
        GET_AND_SET_RELEASE("getAndSetRelease", AccessType.GET_AND_UPDATE),
        GET_AND_ADD("getAndAdd", AccessType.GET_AND_UPDATE),
        GET_AND_ADD_ACQUIRE("getAndAddAcquire", AccessType.GET_AND_UPDATE),
        GET_AND_ADD_RELEASE("getAndAddRelease", AccessType.GET_AND_UPDATE),
        GET_AND_BITWISE_OR("getAndBitwiseOr", AccessType.GET_AND_UPDATE),
        GET_AND_BITWISE_OR_RELEASE("getAndBitwiseOrRelease", AccessType.GET_AND_UPDATE),
        GET_AND_BITWISE_OR_ACQUIRE("getAndBitwiseOrAcquire", AccessType.GET_AND_UPDATE),
        GET_AND_BITWISE_AND("getAndBitwiseAnd", AccessType.GET_AND_UPDATE),
        GET_AND_BITWISE_AND_RELEASE("getAndBitwiseAndRelease", AccessType.GET_AND_UPDATE),
        GET_AND_BITWISE_AND_ACQUIRE("getAndBitwiseAndAcquire", AccessType.GET_AND_UPDATE),
        GET_AND_BITWISE_XOR("getAndBitwiseXor", AccessType.GET_AND_UPDATE),
        GET_AND_BITWISE_XOR_RELEASE("getAndBitwiseXorRelease", AccessType.GET_AND_UPDATE),
        GET_AND_BITWISE_XOR_ACQUIRE("getAndBitwiseXorAcquire", AccessType.GET_AND_UPDATE);

        static final int COUNT = values().length;
        final String methodName;
        final AccessType at;

        AccessMode(String methodName, AccessType at) {
            this.methodName = methodName;
            this.at = at;
        }

        public String methodName() {
            return methodName;
        }

        public static AccessMode valueFromMethodName(String methodName) {
            for (AccessMode m : values()) {
                if (m.methodName.equals(methodName)) return m;
            }
            throw new IllegalArgumentException("No AccessMode value for method name " + methodName);
        }

        static AccessMode valueFromOrdinal(int ordinal) {
            return values()[ordinal];
        }
    }
}
