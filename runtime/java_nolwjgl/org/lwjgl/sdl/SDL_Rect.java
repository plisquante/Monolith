package org.lwjgl.sdl;

import org.lwjgl.system.MemoryUtil;
import org.lwjgl.system.Struct;

import java.nio.ByteBuffer;

public class SDL_Rect extends Struct<SDL_Rect> {
    public static final int SIZEOF = 16;

    public SDL_Rect(ByteBuffer container) {
        this(MemoryUtil.memAddress(container), container);
    }

    public SDL_Rect(long address, ByteBuffer container) {
        super(address, container);
    }

    @Override
    public int sizeof() { return SIZEOF; }

    public static SDL_Rect malloc(org.lwjgl.system.MemoryStack stack) {
        return new SDL_Rect(stack.malloc(4, SIZEOF));
    }

    public static Buffer malloc(int capacity, org.lwjgl.system.MemoryStack stack) {
        return new Buffer(stack.malloc(4, capacity * SIZEOF));
    }

    public int x() { return MemoryUtil.memGetInt(address); }
    public int y() { return MemoryUtil.memGetInt(address + 4); }
    public int w() { return MemoryUtil.memGetInt(address + 8); }
    public int h() { return MemoryUtil.memGetInt(address + 12); }

    public SDL_Rect x(int v) { MemoryUtil.memPutInt(address, v); return this; }
    public SDL_Rect y(int v) { MemoryUtil.memPutInt(address + 4, v); return this; }
    public SDL_Rect w(int v) { MemoryUtil.memPutInt(address + 8, v); return this; }
    public SDL_Rect h(int v) { MemoryUtil.memPutInt(address + 12, v); return this; }

    public static class Buffer extends StructBuffer<SDL_Rect, Buffer> {
        public Buffer(ByteBuffer container) {
            super(MemoryUtil.memAddress(container), container, -1, 0, container.remaining() / SIZEOF, container.remaining() / SIZEOF);
        }

        public Buffer(long address, int capacity) {
            super(address, null, -1, 0, capacity, capacity);
        }

        @Override public int sizeof() { return SIZEOF; }

        @Override
        public SDL_Rect get(int index) {
            return new SDL_Rect(address + (long) index * SIZEOF, container);
        }

        public Buffer x(int v) { MemoryUtil.memPutInt(address(), v); return this; }
        public Buffer y(int v) { MemoryUtil.memPutInt(address() + 4, v); return this; }
        public Buffer w(int v) { MemoryUtil.memPutInt(address() + 8, v); return this; }
        public Buffer h(int v) { MemoryUtil.memPutInt(address() + 12, v); return this; }

        @Override
        public Buffer slice(int offset, int capacity) {
            return new Buffer(address + (long) offset * SIZEOF, capacity);
        }
    }
}
