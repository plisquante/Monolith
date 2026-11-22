package org.lwjgl.system;

public final class Configuration<T> {
    public static final Configuration<String> LIBRARY_PATH = new Configuration<String>();
    public static final Configuration<String> SHARED_LIBRARY_EXTRACT_PATH = new Configuration<String>();
    public static final Configuration<Boolean> DEBUG = new Configuration<Boolean>();
    public static final Configuration<Boolean> DEBUG_LOADER = new Configuration<Boolean>();
    public static final Configuration<Object> DEBUG_STREAM = new Configuration<Object>();
    public static final Configuration<Integer> STACK_SIZE = new Configuration<Integer>();
    public static final Configuration<Boolean> VULKAN_EXPLICIT_INIT = new Configuration<Boolean>();

    private T value;

    public void set(T value) {
        this.value = value;
    }

    public T get() {
        return value;
    }

    public T get(T fallback) {
        return value != null ? value : fallback;
    }
}
