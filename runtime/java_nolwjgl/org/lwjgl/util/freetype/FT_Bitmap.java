package org.lwjgl.util.freetype;

import org.lwjgl.system.MemoryUtil;
import org.lwjgl.system.Struct;

import java.nio.ByteBuffer;

public class FT_Bitmap extends Struct<FT_Bitmap> {
    public static final int SIZEOF = 40;

    public FT_Bitmap(long address, ByteBuffer container) {
        super(address, container);
    }

    @Override
    public int sizeof() { return SIZEOF; }

    public int rows() { return MemoryUtil.memGetInt(address); }
    public int width() { return MemoryUtil.memGetInt(address + 4); }
    public byte pixel_mode() { return MemoryUtil.memGetByte(address + 16); }
    public ByteBuffer buffer(int capacity) {
        long ptr = MemoryUtil.memGetAddress(address + 8);
        return ptr == 0L ? MemoryUtil.memAlloc(Math.max(capacity, 0)) : MemoryUtil.memByteBuffer(ptr, capacity);
    }
}
