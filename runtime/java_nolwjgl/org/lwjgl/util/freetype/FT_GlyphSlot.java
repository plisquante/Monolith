package org.lwjgl.util.freetype;

import org.lwjgl.system.MemoryUtil;
import org.lwjgl.system.Struct;

import java.nio.ByteBuffer;

public class FT_GlyphSlot extends Struct<FT_GlyphSlot> {
    public static final int SIZEOF = 256;
    private final FT_Vector advanceVec = new FT_Vector(address + 128, null);
    private final FT_Bitmap bmp = new FT_Bitmap(address + 144, null);

    public FT_GlyphSlot(long address, ByteBuffer container) {
        super(address, container);
    }

    @Override
    public int sizeof() { return SIZEOF; }

    public FT_Vector advance() { return advanceVec; }
    public FT_Bitmap bitmap() { return bmp; }
    public int bitmap_left() { return MemoryUtil.memGetInt(address + 192); }
    public int bitmap_top() { return MemoryUtil.memGetInt(address + 196); }
}
