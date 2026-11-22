package org.lwjgl.sdl;

import org.lwjgl.system.MemoryStack;
import org.lwjgl.system.MemoryUtil;
import org.lwjgl.system.Struct;

import java.nio.ByteBuffer;

public class SDL_MessageBoxButtonData extends Struct<SDL_MessageBoxButtonData> {
    public static final int SIZEOF = 24;

    public SDL_MessageBoxButtonData(long address, ByteBuffer container) {
        super(address, container);
    }

    @Override public int sizeof() { return SIZEOF; }

    public static Buffer calloc(int capacity, MemoryStack stack) {
        ByteBuffer b = stack.calloc(8, capacity * SIZEOF);
        return new Buffer(b);
    }

    public SDL_MessageBoxButtonData buttonID(int value) { return this; }
    public SDL_MessageBoxButtonData text(ByteBuffer value) { return this; }

    public static class Buffer extends StructBuffer<SDL_MessageBoxButtonData, Buffer> {
        public Buffer(ByteBuffer container) {
            super(MemoryUtil.memAddress(container), container, -1, 0, container.remaining() / SIZEOF, container.remaining() / SIZEOF);
        }

        public Buffer(long address, int capacity) {
            super(address, null, -1, 0, capacity, capacity);
        }

        @Override public int sizeof() { return SIZEOF; }

        @Override
        public SDL_MessageBoxButtonData get(int index) {
            return new SDL_MessageBoxButtonData(address + (long) index * SIZEOF, container);
        }

        @Override
        public Buffer slice(int offset, int capacity) {
            return new Buffer(address + (long) offset * SIZEOF, capacity);
        }
    }
}
