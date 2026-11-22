package org.lwjgl.system;

import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicLong;

public abstract class Callback implements Pointer, AutoCloseable {
    private static final AtomicLong NEXT_ID = new AtomicLong(0x1000L);
    private static final ConcurrentHashMap<Long, Callback> REGISTRY = new ConcurrentHashMap<Long, Callback>();

    private final long address;

    protected Callback() {
        this.address = NEXT_ID.getAndIncrement();
        REGISTRY.put(this.address, this);
    }

    protected Callback(long address) {
        this.address = address;
        if (address != 0L) REGISTRY.put(address, this);
    }

    @Override
    public long address() {
        return address;
    }

    @SuppressWarnings("unchecked")
    public static <T extends Callback> T get(long address) {
        return (T) REGISTRY.get(address);
    }

    public void free() {
        REGISTRY.remove(address);
    }

    @Override
    public void close() {
        free();
    }

    public static void free(long functionPointer) {
        REGISTRY.remove(functionPointer);
    }
}
