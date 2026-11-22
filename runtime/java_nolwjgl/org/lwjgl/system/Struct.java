package org.lwjgl.system;

import java.nio.ByteBuffer;

public abstract class Struct<SELF extends Struct<SELF>> implements Pointer, NativeResource {
    protected long address;
    protected ByteBuffer container;

    protected Struct(long address, ByteBuffer container) {
        this.address = address;
        this.container = container;
    }

    @Override
    public long address() {
        return address;
    }

    public abstract int sizeof();

    public void close() {
        free();
    }

    public void free() {
        if (address != 0L && container == null) {
            MemoryUtil.nmemFree(address);
            address = 0L;
        }
    }

    public static class StructBuffer<T extends Struct<T>, SELF extends StructBuffer<T, SELF>> extends org.lwjgl.system.StructBuffer<T, SELF> {
        protected StructBuffer(long address, ByteBuffer container, int mark, int position, int limit, int capacity) {
            super(address, container, mark, position, limit, capacity);
        }
    }
}
