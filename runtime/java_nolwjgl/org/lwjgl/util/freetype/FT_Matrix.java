package org.lwjgl.util.freetype;

import org.lwjgl.system.MemoryStack;
import org.lwjgl.system.MemoryUtil;
import org.lwjgl.system.Struct;

import java.nio.ByteBuffer;

public class FT_Matrix extends Struct<FT_Matrix> {
    public static final int SIZEOF = 32;

    public FT_Matrix(long address, ByteBuffer container) {
        super(address, container);
    }

    public FT_Matrix(ByteBuffer container) {
        this(MemoryUtil.memAddress(container), container);
    }

    @Override
    public int sizeof() { return SIZEOF; }

    public static FT_Matrix calloc(MemoryStack stack) {
        ByteBuffer b = stack.calloc(8, SIZEOF);
        return new FT_Matrix(MemoryUtil.memAddress(b), b);
    }

    public long xx() { return MemoryUtil.memGetAddress(address); }
    public long xy() { return MemoryUtil.memGetAddress(address + 8); }
    public long yx() { return MemoryUtil.memGetAddress(address + 16); }
    public long yy() { return MemoryUtil.memGetAddress(address + 24); }
    public FT_Matrix xx(long v) { MemoryUtil.memPutAddress(address, v); return this; }
    public FT_Matrix xy(long v) { MemoryUtil.memPutAddress(address + 8, v); return this; }
    public FT_Matrix yx(long v) { MemoryUtil.memPutAddress(address + 16, v); return this; }
    public FT_Matrix yy(long v) { MemoryUtil.memPutAddress(address + 24, v); return this; }
}
