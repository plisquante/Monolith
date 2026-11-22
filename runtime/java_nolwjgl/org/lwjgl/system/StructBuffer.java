package org.lwjgl.system;

import java.nio.ByteBuffer;

public class StructBuffer<T extends Struct<T>, SELF extends StructBuffer<T, SELF>> extends CustomBuffer<SELF> implements NativeResource {
    protected StructBuffer(long address, ByteBuffer container, int mark, int position, int limit, int capacity) {
        super(address, container, mark, position, limit, capacity);
    }

    @Override
    public int sizeof() {
        return 8;
    }

    public T get() {
        return get(position++);
    }

    public T get(int index) {
        return null;
    }

    @Override
    @SuppressWarnings("unchecked")
    public SELF slice(int offset, int capacity) {
        return (SELF) new StructBuffer<T, SELF>(address + (long) offset * sizeof(), container, -1, 0, capacity, capacity);
    }
}
