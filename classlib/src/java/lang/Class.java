package java.lang;

/**
 * monolith overlay of java.lang.Class. Each instance mirrors a runtime rt.Class
 * (address kept in {@code monoClass}). Reflection over members is not available
 * in the closed-world native image (reported by the transpiler).
 */
public final class Class<T> implements java.io.Serializable, java.lang.reflect.Type, java.lang.reflect.GenericDeclaration {
    private long monoClass;
    private transient String name;
    /** used by java.lang.ClassValue */
    transient ClassValue.ClassValueMap classValueMap;

    private Class() {}

    private native String getName0();
    private native int modifiers0();
    private native Class<?> super0();
    private native Class<?> component0();
    private native Class<?>[] interfaces0();
    private native boolean isInstance0(Object o);
    private native boolean isAssignableFrom0(Class<?> c);
    private native int kind0();  // 0 instance, 1 interface, 2 array, 3 primitive
    private native Object[] enumValues0();
    private static native Class<?> forName0(String name);
    static native Class<?> getPrimitiveClass(String name);

    public String toString() {
        String kind = isInterface() ? "interface " : isPrimitive() ? "" : "class ";
        return kind.concat(getName());
    }

    public String toGenericString() {
        return toString();
    }

    public static Class<?> forName(String className) throws ClassNotFoundException {
        Class<?> c = forName0(className);
        if (c == null) throw new ClassNotFoundException(className);
        c.ensureInit();
        return c;
    }

    public static Class<?> forName(String name, boolean initialize, ClassLoader loader) throws ClassNotFoundException {
        Class<?> c = forName0(name);
        if (c == null) throw new ClassNotFoundException(name);
        if (initialize) c.ensureInit();
        return c;
    }

    private native void ensureInit();

    public String getName() {
        String n = name;
        if (n == null) name = n = getName0();
        return n;
    }

    public boolean isInstance(Object obj) {
        return isInstance0(obj);
    }

    public boolean isAssignableFrom(Class<?> cls) {
        return isAssignableFrom0(cls);
    }

    public boolean isInterface() {
        return kind0() == 1;
    }

    public boolean isArray() {
        return kind0() == 2;
    }

    public boolean isPrimitive() {
        return kind0() == 3;
    }

    public boolean isAnnotation() {
        return (getModifiers() & 0x2000) != 0;
    }

    public boolean isSynthetic() {
        return (getModifiers() & 0x1000) != 0;
    }

    public boolean isEnum() {
        return (getModifiers() & 0x4000) != 0 && getSuperclass() == Enum.class;
    }

    public boolean isRecord() {
        return getSuperclass() == Record.class && (getModifiers() & 0x0010) != 0;
    }

    public boolean isSealed() {
        return false;
    }

    private native boolean isBoot0();

    public ClassLoader getClassLoader() {
        // JDK classes belong to the boot loader (null); application classes to the system loader
        Class<?> c = this;
        while (c.isArray()) c = c.getComponentType();
        if (c.isPrimitive() || c.isBoot0()) return null;
        return ClassLoader.getSystemClassLoader();
    }

    /** JDK classes live in a named, open "java.base" module of the (empty) boot layer, so that
     *  {@code Runtime.class.getModule().getLayer()} is non-null as on HotSpot and reflection into the JDK
     *  is not denied; application classes in the system class loader's unnamed module (class-path launch). */
    private static final class BootModule {
        static final Module M = new Module("java.base", false, java.util.Set.of());
    }

    /** sun.misc.Unsafe and friends: jdk.unsupported opens its packages (LWJGL, Netty, Guava use theUnsafe). */
    private static final class UnsupportedModule {
        static final Module M = new Module("jdk.unsupported", true, java.util.Set.of("sun.misc", "sun.reflect"));
    }

    public Module getModule() {
        ClassLoader l = getClassLoader();
        if (l != null) return l.getUnnamedModule();
        String n = getName();
        return n.startsWith("sun.misc.") || n.startsWith("sun.reflect.") ? UnsupportedModule.M : BootModule.M;
    }

    @SuppressWarnings("unchecked")
    public Class<? super T> getSuperclass() {
        return (Class<? super T>) super0();
    }

    public Class<?>[] getInterfaces() {
        return interfaces0();
    }

    public Class<?> getComponentType() {
        return component0();
    }

    public Class<?> componentType() {
        return component0();
    }

    public Class<?> arrayType() {
        return java.lang.reflect.Array.newInstance(this, 0).getClass();
    }

    public int getModifiers() {
        // member classes report the access flags of their InnerClasses entry (as HotSpot does)
        int f = isArray() || isPrimitive() ? modifiers0() : innerFlags0();
        if (f < 0) f = modifiers0();
        return f & 0x761F & ~0x0020;
    }

    public String getSimpleName() {
        if (isArray()) return getComponentType().getSimpleName().concat("[]");
        String simpleName = getSimpleBinaryName();
        if (simpleName == null) { // top level class or anonymous
            if (isLocalOrAnonymousClass()) return "";
            String n = getName();
            return n.substring(n.lastIndexOf('.') + 1);
        }
        return simpleName;
    }

    public String getTypeName() {
        if (isArray()) {
            Class<?> cl = this;
            int dimensions = 0;
            while (cl.isArray()) {
                dimensions++;
                cl = cl.getComponentType();
            }
            return cl.getName() + "[]".repeat(dimensions);
        }
        return getName();
    }

    public String getCanonicalName() {
        if (isArray()) {
            String c = getComponentType().getCanonicalName();
            return c == null ? null : c.concat("[]");
        }
        if (isHidden() || isLocalOrAnonymousClass()) return null;
        Class<?> enclosingClass = getEnclosingClass();
        if (enclosingClass == null) return getName();
        String enclosingName = enclosingClass.getCanonicalName();
        return enclosingName == null ? null : enclosingName + "." + getSimpleName();
    }

    public boolean isHidden() {
        return false;
    }

    public String getPackageName() {
        Class<?> c = this;
        while (c.isArray()) c = c.getComponentType();
        if (c.isPrimitive()) return "java.lang";
        String cn = c.getName();
        int dot = cn.lastIndexOf('.');
        return (dot != -1) ? cn.substring(0, dot) : "";
    }

    public String descriptorString() {
        if (isPrimitive()) {
            switch (getName()) {
                case "int": return "I";
                case "long": return "J";
                case "boolean": return "Z";
                case "byte": return "B";
                case "char": return "C";
                case "short": return "S";
                case "float": return "F";
                case "double": return "D";
                default: return "V";
            }
        }
        if (isArray()) return "[" + getComponentType().descriptorString();
        return "L" + getName().replace('.', '/') + ";";
    }

    @SuppressWarnings("unchecked")
    public T[] getEnumConstants() {
        Object[] v = enumValues0();
        return v == null ? null : (T[]) v;
    }

    @SuppressWarnings("unchecked")
    T[] getEnumConstantsShared() {
        return (T[]) enumValues0();
    }

    private transient volatile java.util.Map<String, T> enumConstantDirectory;

    java.util.Map<String, T> enumConstantDirectory() {
        java.util.Map<String, T> directory = enumConstantDirectory;
        if (directory == null) {
            T[] universe = getEnumConstantsShared();
            if (universe == null) throw new IllegalArgumentException(getName() + " is not an enum class");
            directory = new java.util.HashMap<>((int) (universe.length / 0.75f) + 1);
            for (T constant : universe) directory.put(((Enum<?>) constant).name(), constant);
            enumConstantDirectory = directory;
        }
        return directory;
    }

    @SuppressWarnings("unchecked")
    public T cast(Object obj) {
        if (obj != null && !isInstance(obj))
            throw new ClassCastException("Cannot cast " + obj.getClass().getName() + " to " + getName());
        return (T) obj;
    }

    @SuppressWarnings("unchecked")
    public <U> Class<? extends U> asSubclass(Class<U> clazz) {
        if (clazz.isAssignableFrom(this)) return (Class<? extends U>) this;
        throw new ClassCastException(this.toString());
    }

    public boolean desiredAssertionStatus() {
        return false;
    }

    // ---- nesting (InnerClasses / EnclosingMethod / NestHost attributes, kept by the transpiler)
    private native int innerFlags0();          // -1 if this class has no InnerClasses entry
    private native String outerName0();        // declaring class (binary name) or null
    private native String innerName0();        // simple binary name, null for anonymous classes
    private native String[] enclosingMethod0(); // {class, name or null, descriptor or null} or null
    private native String[] declaredClasses0();
    private native String nestHost0();

    private static Class<?> named(String n) {
        return n == null ? null : forName0(n.replace('/', '.'));
    }

    private String getSimpleBinaryName() {
        return isArray() || isPrimitive() || innerFlags0() < 0 ? null : innerName0();
    }

    private boolean isLocalOrAnonymousClass() {
        return !isArray() && !isPrimitive() && enclosingMethod0() != null;
    }

    private boolean isTopLevelClass() {
        return !isLocalOrAnonymousClass() && getDeclaringClass0() == null;
    }

    private Class<?> getDeclaringClass0() {
        if (isArray() || isPrimitive() || innerFlags0() < 0) return null;
        return named(outerName0());
    }

    public Class<?> getNestHost() {
        if (isArray() || isPrimitive()) return this;
        Class<?> h = named(nestHost0());
        return h == null ? this : h;
    }

    public boolean isNestmateOf(Class<?> c) {
        return this == c || getNestHost() == c.getNestHost();
    }

    public Class<?> getEnclosingClass() {
        String[] em = isArray() || isPrimitive() ? null : enclosingMethod0();
        if (em != null) return named(em[0]);
        return getDeclaringClass0();
    }

    public Class<?> getDeclaringClass() {
        return isLocalOrAnonymousClass() ? null : getDeclaringClass0();
    }

    public boolean isAnonymousClass() {
        return isLocalOrAnonymousClass() && getSimpleBinaryName() == null;
    }

    public boolean isMemberClass() {
        return !isLocalOrAnonymousClass() && getDeclaringClass0() != null;
    }

    public boolean isLocalClass() {
        return isLocalOrAnonymousClass() && getSimpleBinaryName() != null;
    }

    private java.lang.reflect.Executable enclosingExecutable(boolean ctor) {
        String[] em = isArray() || isPrimitive() ? null : enclosingMethod0();
        if (em == null || em[1] == null || ctor != em[1].equals("<init>")) return null;
        Class<?> ec = named(em[0]);
        if (ec == null) return null;
        java.lang.reflect.Executable[] cands = ctor ? ec.getDeclaredConstructors() : ec.getDeclaredMethods();
        for (java.lang.reflect.Executable e : cands) {
            if (!ctor && !e.getName().equals(em[1])) continue;
            StringBuilder d = new StringBuilder("(");
            for (Class<?> p : e.getParameterTypes()) d.append(p.descriptorString());
            d.append(')').append(ctor ? "V" : ((java.lang.reflect.Method) e).getReturnType().descriptorString());
            if (d.toString().equals(em[2])) return e;
        }
        return null;
    }

    public java.lang.reflect.Method getEnclosingMethod() {
        return (java.lang.reflect.Method) enclosingExecutable(false);
    }

    public java.lang.reflect.Constructor<?> getEnclosingConstructor() {
        return (java.lang.reflect.Constructor<?>) enclosingExecutable(true);
    }

    public Class<?>[] getDeclaredClasses() {
        if (isArray() || isPrimitive()) return NO_CLASSES;
        java.util.ArrayList<Class<?>> l = new java.util.ArrayList<>();
        for (String n : declaredClasses0()) {
            Class<?> c = named(n);
            if (c != null) l.add(c);
        }
        return l.toArray(NO_CLASSES);
    }

    public Class<?>[] getClasses() {
        java.util.ArrayList<Class<?>> l = new java.util.ArrayList<>();
        for (Class<?> c = this; c != null; c = c.getSuperclass())
            for (Class<?> m : c.getDeclaredClasses())
                if (java.lang.reflect.Modifier.isPublic(m.getModifiers())) l.add(m);
        return l.toArray(NO_CLASSES);
    }

    // ---- generics (Signature attribute, parsed by the JDK's sun.reflect.generics)
    private native String genericSignature0();
    private transient volatile sun.reflect.generics.repository.ClassRepository genericInfo;

    private sun.reflect.generics.factory.GenericsFactory getFactory() {
        return sun.reflect.generics.factory.CoreReflectionFactory.make(this, sun.reflect.generics.scope.ClassScope.make(this));
    }

    private sun.reflect.generics.repository.ClassRepository getGenericInfo() {
        sun.reflect.generics.repository.ClassRepository gi = genericInfo;
        if (gi == null) {
            String s = isArray() || isPrimitive() ? null : genericSignature0();
            gi = s == null ? sun.reflect.generics.repository.ClassRepository.NONE
                    : sun.reflect.generics.repository.ClassRepository.make(s, getFactory());
            genericInfo = gi;
        }
        return gi != sun.reflect.generics.repository.ClassRepository.NONE ? gi : null;
    }

    public java.lang.reflect.Type getGenericSuperclass() {
        sun.reflect.generics.repository.ClassRepository info = getGenericInfo();
        if (info == null) return getSuperclass();
        if (isInterface()) return null;
        return info.getSuperclass();
    }

    public java.lang.reflect.Type[] getGenericInterfaces() {
        sun.reflect.generics.repository.ClassRepository info = getGenericInfo();
        return info == null ? getInterfaces() : info.getSuperInterfaces();
    }

    private native java.lang.reflect.Field newField0(String name);
    private native Object[] fields0();
    private native Object[] methods0(boolean constructors);
    private static final Class<?>[] NO_CLASSES = new Class<?>[0];

    public java.lang.reflect.Field[] getDeclaredFields() {
        Object[] a = fields0();
        java.lang.reflect.Field[] r = new java.lang.reflect.Field[a.length];
        System.arraycopy(a, 0, r, 0, a.length);
        return r;
    }

    public java.lang.reflect.Field[] getFields() {
        java.util.ArrayList<java.lang.reflect.Field> l = new java.util.ArrayList<>();
        for (Class<?> c = this; c != null; c = c.getSuperclass())
            for (java.lang.reflect.Field f : c.getDeclaredFields())
                if (java.lang.reflect.Modifier.isPublic(f.getModifiers())) l.add(f);
        return l.toArray(new java.lang.reflect.Field[0]);
    }

    public java.lang.reflect.Method[] getDeclaredMethods() {
        Object[] a = methods0(false);
        java.lang.reflect.Method[] r = new java.lang.reflect.Method[a.length];
        System.arraycopy(a, 0, r, 0, a.length);
        return r;
    }

    public java.lang.reflect.Method[] getMethods() {
        java.util.LinkedHashMap<String, java.lang.reflect.Method> m = new java.util.LinkedHashMap<>();
        java.util.ArrayDeque<Class<?>> todo = new java.util.ArrayDeque<>();
        todo.add(this);
        while (!todo.isEmpty()) {
            Class<?> c = todo.poll();
            for (java.lang.reflect.Method x : c.getDeclaredMethods()) {
                if (!java.lang.reflect.Modifier.isPublic(x.getModifiers())) continue;
                String key = x.getName() + java.util.Arrays.toString(x.getParameterTypes());
                m.putIfAbsent(key, x);
            }
            if (c.getSuperclass() != null) todo.add(c.getSuperclass());
            for (Class<?> i : c.getInterfaces()) todo.add(i);
        }
        return m.values().toArray(new java.lang.reflect.Method[0]);
    }

    private static boolean sameParams(Class<?>[] a, Class<?>[] b) {
        return java.util.Arrays.equals(a, b == null ? NO_CLASSES : b);
    }

    private static String sig(String name, Class<?>[] p) {
        StringBuilder sb = new StringBuilder(name).append('(');
        if (p != null) for (int i = 0; i < p.length; i++) {
            if (i > 0) sb.append(", ");
            sb.append(p[i] == null ? "null" : p[i].getName());
        }
        return sb.append(')').toString();
    }

    public java.lang.reflect.Method getDeclaredMethod(String name, Class<?>... parameterTypes) throws NoSuchMethodException {
        java.lang.reflect.Method best = null;
        for (java.lang.reflect.Method m : getDeclaredMethods())
            if (m.getName().equals(name) && sameParams(m.getParameterTypes(), parameterTypes)
                    && (best == null || best.getReturnType().isAssignableFrom(m.getReturnType()))) best = m;
        if (best == null) throw new NoSuchMethodException(getName() + "." + sig(name, parameterTypes));
        return best;
    }

    public java.lang.reflect.Method getMethod(String name, Class<?>... parameterTypes) throws NoSuchMethodException {
        for (java.lang.reflect.Method m : getMethods())
            if (m.getName().equals(name) && sameParams(m.getParameterTypes(), parameterTypes)) return m;
        throw new NoSuchMethodException(getName() + "." + sig(name, parameterTypes));
    }

    @SuppressWarnings("unchecked")
    public java.lang.reflect.Constructor<T>[] getDeclaredConstructors() {
        Object[] a = methods0(true);
        java.lang.reflect.Constructor<T>[] r = new java.lang.reflect.Constructor[a.length];
        System.arraycopy(a, 0, r, 0, a.length);
        return r;
    }

    public java.lang.reflect.Constructor<?>[] getConstructors() {
        java.util.ArrayList<java.lang.reflect.Constructor<?>> l = new java.util.ArrayList<>();
        for (java.lang.reflect.Constructor<?> c : getDeclaredConstructors())
            if (java.lang.reflect.Modifier.isPublic(c.getModifiers())) l.add(c);
        return l.toArray(new java.lang.reflect.Constructor<?>[0]);
    }

    public java.lang.reflect.Constructor<T> getDeclaredConstructor(Class<?>... parameterTypes) throws NoSuchMethodException {
        for (java.lang.reflect.Constructor<T> c : getDeclaredConstructors())
            if (sameParams(c.getParameterTypes(), parameterTypes)) return c;
        throw new NoSuchMethodException(getName() + "." + sig("<init>", parameterTypes));
    }

    @SuppressWarnings("unchecked")
    public java.lang.reflect.Constructor<T> getConstructor(Class<?>... parameterTypes) throws NoSuchMethodException {
        java.lang.reflect.Constructor<T> c = getDeclaredConstructor(parameterTypes);
        if (!java.lang.reflect.Modifier.isPublic(c.getModifiers()))
            throw new NoSuchMethodException(getName() + "." + sig("<init>", parameterTypes));
        return c;
    }

    @Deprecated(since = "9")
    public T newInstance() throws InstantiationException, IllegalAccessException {
        try {
            return getDeclaredConstructor().newInstance();
        } catch (NoSuchMethodException e) {
            throw new InstantiationException(getName());
        } catch (java.lang.reflect.InvocationTargetException e) {
            Throwable c = e.getCause();
            if (c instanceof RuntimeException r) throw r;
            if (c instanceof Error er) throw er;
            throw new InstantiationException(c.toString());
        }
    }

    /**
     * Field objects carry declaring class, name, type and modifiers (enough for Unsafe /
     * VarHandle offset lookups); reading or writing through Field.get/set is not supported.
     */
    public java.lang.reflect.Field getDeclaredField(String name) throws NoSuchFieldException {
        java.util.Objects.requireNonNull(name);
        java.lang.reflect.Field f = newField0(name);
        if (f == null) throw new NoSuchFieldException(name);
        return f;
    }

    public java.lang.reflect.Field getField(String name) throws NoSuchFieldException {
        for (Class<?> c = this; c != null; c = c.getSuperclass()) {
            java.lang.reflect.Field f = c.newField0(name);
            if (f != null && java.lang.reflect.Modifier.isPublic(f.getModifiers())) return f;
        }
        throw new NoSuchFieldException(name);
    }

    @SuppressWarnings("unchecked")
    public java.lang.reflect.TypeVariable<Class<T>>[] getTypeParameters() {
        sun.reflect.generics.repository.ClassRepository info = getGenericInfo();
        if (info != null) return (java.lang.reflect.TypeVariable<Class<T>>[]) info.getTypeParameters();
        return (java.lang.reflect.TypeVariable<Class<T>>[]) new java.lang.reflect.TypeVariable<?>[0];
    }

    // ---- annotations (RuntimeVisibleAnnotations, decoded by sun.reflect.annotation.AnnotationParser)
    native byte[] getRawAnnotations();
    native jdk.internal.reflect.ConstantPool getConstantPool();

    private static final class AnnotationData {
        final java.util.Map<Class<? extends java.lang.annotation.Annotation>, java.lang.annotation.Annotation> annotations;
        final java.util.Map<Class<? extends java.lang.annotation.Annotation>, java.lang.annotation.Annotation> declaredAnnotations;

        AnnotationData(java.util.Map<Class<? extends java.lang.annotation.Annotation>, java.lang.annotation.Annotation> annotations,
                       java.util.Map<Class<? extends java.lang.annotation.Annotation>, java.lang.annotation.Annotation> declaredAnnotations) {
            this.annotations = annotations;
            this.declaredAnnotations = declaredAnnotations;
        }
    }

    private transient volatile AnnotationData annotationData;
    private transient volatile sun.reflect.annotation.AnnotationType annotationType;

    private AnnotationData annotationData() {
        AnnotationData d = annotationData;
        if (d != null) return d;
        java.util.Map<Class<? extends java.lang.annotation.Annotation>, java.lang.annotation.Annotation> declared =
                isArray() || isPrimitive() ? java.util.Collections.emptyMap()
                        : sun.reflect.annotation.AnnotationParser.parseAnnotations(getRawAnnotations(), getConstantPool(), this);
        Class<?> sup = getSuperclass();
        java.util.Map<Class<? extends java.lang.annotation.Annotation>, java.lang.annotation.Annotation> all;
        if (sup == null) {
            all = declared;
        } else {
            all = new java.util.LinkedHashMap<>();
            for (java.util.Map.Entry<Class<? extends java.lang.annotation.Annotation>, java.lang.annotation.Annotation> e
                    : sup.annotationData().annotations.entrySet()) {
                if (sun.reflect.annotation.AnnotationType.getInstance(e.getKey()).isInherited())
                    all.put(e.getKey(), e.getValue());
            }
            all.putAll(declared);
        }
        d = new AnnotationData(all, declared);
        annotationData = d;
        return d;
    }

    boolean casAnnotationType(sun.reflect.annotation.AnnotationType oldType, sun.reflect.annotation.AnnotationType newType) {
        synchronized (this) {
            if (annotationType != oldType) return false;
            annotationType = newType;
            return true;
        }
    }

    sun.reflect.annotation.AnnotationType getAnnotationType() {
        return annotationType;
    }

    java.util.Map<Class<? extends java.lang.annotation.Annotation>, java.lang.annotation.Annotation> getDeclaredAnnotationMap() {
        return annotationData().declaredAnnotations;
    }

    @SuppressWarnings("unchecked")
    public <A extends java.lang.annotation.Annotation> A getAnnotation(Class<A> annotationClass) {
        java.util.Objects.requireNonNull(annotationClass);
        return (A) annotationData().annotations.get(annotationClass);
    }

    public boolean isAnnotationPresent(Class<? extends java.lang.annotation.Annotation> annotationClass) {
        return getAnnotation(annotationClass) != null;
    }

    public <A extends java.lang.annotation.Annotation> A[] getAnnotationsByType(Class<A> annotationClass) {
        java.util.Objects.requireNonNull(annotationClass);
        return sun.reflect.annotation.AnnotationSupport.getAssociatedAnnotations(annotationData().declaredAnnotations, this, annotationClass);
    }

    public java.lang.annotation.Annotation[] getAnnotations() {
        return sun.reflect.annotation.AnnotationParser.toArray(annotationData().annotations);
    }

    @SuppressWarnings("unchecked")
    public <A extends java.lang.annotation.Annotation> A getDeclaredAnnotation(Class<A> annotationClass) {
        java.util.Objects.requireNonNull(annotationClass);
        return (A) annotationData().declaredAnnotations.get(annotationClass);
    }

    public <A extends java.lang.annotation.Annotation> A[] getDeclaredAnnotationsByType(Class<A> annotationClass) {
        java.util.Objects.requireNonNull(annotationClass);
        return sun.reflect.annotation.AnnotationSupport.getDirectlyAndIndirectlyPresent(annotationData().declaredAnnotations, annotationClass);
    }

    public java.lang.annotation.Annotation[] getDeclaredAnnotations() {
        return sun.reflect.annotation.AnnotationParser.toArray(annotationData().declaredAnnotations);
    }

    // ---- resources: served from the resources directory (MONO_RESOURCES), like the class path root
    private String resolveName(String name) {
        if (!name.startsWith("/")) {
            String base = getPackageName();
            if (!base.isEmpty()) name = base.replace('.', '/') + "/" + name;
        } else {
            name = name.substring(1);
        }
        return name;
    }

    public java.io.InputStream getResourceAsStream(String name) {
        return ClassLoader.getSystemClassLoader().getResourceAsStream(resolveName(name));
    }

    public java.net.URL getResource(String name) {
        return ClassLoader.getSystemClassLoader().getResource(resolveName(name));
    }

    // ---- misc API completed for libraries that probe it
    private static final java.security.ProtectionDomain MONO_PD = new java.security.ProtectionDomain(null, null);
    private static final java.util.concurrent.ConcurrentHashMap<String, Package> PACKAGES = new java.util.concurrent.ConcurrentHashMap<>();

    public java.security.ProtectionDomain getProtectionDomain() {
        return MONO_PD;
    }

    public Package getPackage() {
        if (isPrimitive() || isArray()) return null;
        String n = getPackageName();
        return PACKAGES.computeIfAbsent(n, k -> new Package(k, getModule()));
    }

    ClassLoader getClassLoader0() {
        return getClassLoader();
    }

    public Object[] getSigners() {
        return null;
    }

    void setSigners(Object[] signers) {}

    static Class<?> forPrimitiveName(String name) {
        switch (name) {
            case "int": case "long": case "boolean": case "byte": case "char": case "short":
            case "float": case "double": case "void":
                return getPrimitiveClass(name);
            default:
                return null;
        }
    }

    public static Class<?> forName(Module module, String name) {
        java.util.Objects.requireNonNull(module);
        java.util.Objects.requireNonNull(name);
        Class<?> c = forName0(name);
        if (c != null) c.ensureInit();
        return c;
    }

    public Class<?>[] getNestMembers() {
        java.util.ArrayList<Class<?>> l = new java.util.ArrayList<>();
        Class<?> host = getNestHost();
        l.add(host);
        collectNest(host, l);
        return l.toArray(NO_CLASSES);
    }

    private static void collectNest(Class<?> c, java.util.List<Class<?>> out) {
        for (Class<?> m : c.getDeclaredClasses()) {
            if (!out.contains(m)) {
                out.add(m);
                collectNest(m, out);
            }
        }
    }

    public java.util.Set<java.lang.reflect.AccessFlag> accessFlags() {
        java.lang.reflect.AccessFlag.Location loc = isMemberClass() || isLocalOrAnonymousClass()
                ? java.lang.reflect.AccessFlag.Location.INNER_CLASS : java.lang.reflect.AccessFlag.Location.CLASS;
        return java.lang.reflect.AccessFlag.maskToAccessFlags(isArray() ? modifiers0() : getModifiers() | (modifiers0() & 0x1000), loc);
    }

    public java.util.Optional<java.lang.constant.ClassDesc> describeConstable() {
        return java.util.Optional.of(java.lang.constant.ClassDesc.ofDescriptor(descriptorString()));
    }

    public java.lang.reflect.RecordComponent[] getRecordComponents() {
        return isRecord() ? new java.lang.reflect.RecordComponent[0] : null;
    }
}
