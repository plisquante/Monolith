package org.lwjgl.util.freetype;

import org.lwjgl.system.MemoryStack;
import org.lwjgl.system.MemoryUtil;
import org.lwjgl.system.Struct;

import java.nio.ByteBuffer;

public class FT_Vector extends Struct<FT_Vector> {
    public static final int SIZEOF = 16;

    public FT_Vector(long address, ByteBuffer container) {
        super(address, container);
    }

    @Override
    public int sizeof() { return SIZEOF; }

    public static FT_Vector malloc(MemoryStack stack) {
        ByteBuffer b = stack.malloc(8, SIZEOF);
        return new FT_Vector(MemoryUtil.memAddress(b), b);
    }

    public long x() { return MemoryUtil.memGetAddress(address); }

    public FT_Vector set(long x, long y) {
        MemoryUtil.memPutAddress(address, x);
        MemoryUtil.memPutAddress(address + 8, y);
        return this;
    }
}
