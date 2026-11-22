package org.lwjgl.system;

import java.nio.ByteBuffer;

public abstract class CustomBuffer<SELF extends CustomBuffer<SELF>> implements Pointer {
    protected long address;
    protected ByteBuffer container;
    protected int mark = -1;
    protected int position = 0;
    protected int limit;
    protected int capacity;

    protected CustomBuffer(long address, ByteBuffer container, int mark, int position, int limit, int capacity) {
        this.address = address;
        this.container = container;
        this.mark = mark;
        this.position = position;
        this.limit = limit;
        this.capacity = capacity;
    }

    @SuppressWarnings("unchecked")
    protected SELF self() {
        return (SELF) this;
    }

    public abstract int sizeof();

    @Override
    public long address() {
        return address + (long) position * sizeof();
    }

    public long address0() {
        return address;
    }

    public int capacity() {
        return capacity;
    }

    public int position() {
        return position;
    }

    public SELF position(int newPosition) {
        this.position = newPosition;
        if (mark > newPosition) mark = -1;
        return self();
    }

    public int limit() {
        return limit;
    }

    public SELF limit(int newLimit) {
        this.limit = newLimit;
        if (position > newLimit) position = newLimit;
        if (mark > newLimit) mark = -1;
        return self();
    }

    public SELF clear() {
        position = 0;
        limit = capacity;
        mark = -1;
        return self();
    }

    public SELF flip() {
        limit = position;
        position = 0;
        mark = -1;
        return self();
    }

    public SELF rewind() {
        position = 0;
        mark = -1;
        return self();
    }

    public int remaining() {
        return Math.max(0, limit - position);
    }

    public boolean hasRemaining() {
        return position < limit;
    }

    public abstract SELF slice(int offset, int capacity);

    public void free() {
        if (address != 0L && container == null) {
            MemoryUtil.nmemFree(address);
            address = 0L;
        }
    }
}
