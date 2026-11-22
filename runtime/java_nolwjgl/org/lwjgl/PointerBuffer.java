package org.lwjgl;

import org.lwjgl.system.CustomBuffer;
import org.lwjgl.system.MemoryUtil;

import java.nio.ByteBuffer;

public class PointerBuffer extends CustomBuffer<PointerBuffer> {
    protected PointerBuffer(long address, ByteBuffer container, int mark, int position, int limit, int capacity) {
        super(address, container, mark, position, limit, capacity);
    }

    public static PointerBuffer allocateDirect(int capacity) {
        long addr = MemoryUtil.nmemCalloc(Math.max(1, capacity), 8L);
        return new PointerBuffer(addr, null, -1, 0, capacity, capacity);
    }

    public static PointerBuffer create(long address, int capacity) {
        return new PointerBuffer(address, null, -1, 0, capacity, capacity);
    }

    public static PointerBuffer create(ByteBuffer source) {
        long addr = MemoryUtil.memAddress(source);
        int cap = source.remaining() >> 3;
        return new PointerBuffer(addr, source, -1, 0, cap, cap);
    }

    @Override
    public int sizeof() {
        return POINTER_SIZE;
    }

    public long get() {
        long v = get(position);
        position++;
        return v;
    }

    public long get(int index) {
        return MemoryUtil.memGetAddress(address + (long) index * POINTER_SIZE);
    }

    public PointerBuffer put(long value) {
        put(position++, value);
        return this;
    }

    public PointerBuffer put(int index, long value) {
        MemoryUtil.memPutAddress(address + (long) index * POINTER_SIZE, value);
        return this;
    }

    public PointerBuffer put(ByteBuffer value) {
        return put(value == null ? 0L : MemoryUtil.memAddress(value));
    }

    public PointerBuffer put(int index, ByteBuffer value) {
        return put(index, value == null ? 0L : MemoryUtil.memAddress(value));
    }

    @Override
    public PointerBuffer slice(int offset, int capacity) {
        return new PointerBuffer(address + (long) offset * POINTER_SIZE, container, -1, 0, capacity, capacity);
    }
}
