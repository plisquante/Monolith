package org.lwjgl.util.freetype;

import org.lwjgl.system.MemoryUtil;
import org.lwjgl.system.Struct;

import java.nio.ByteBuffer;

public class FT_Face extends Struct<FT_Face> {
    public static final int SIZEOF = 256;
    private FT_GlyphSlot glyphSlot;

    public FT_Face(long address, ByteBuffer container) {
        super(address, container);
    }

    @Override
    public int sizeof() { return SIZEOF; }

    public static FT_Face create(long address) {
        return address == 0L ? null : new FT_Face(address, null);
    }

    public FT_GlyphSlot glyph() {
        if (glyphSlot == null) {
            ByteBuffer b = MemoryUtil.memCalloc(FT_GlyphSlot.SIZEOF);
            glyphSlot = new FT_GlyphSlot(MemoryUtil.memAddress(b), b);
        }
        return glyphSlot;
    }
}
