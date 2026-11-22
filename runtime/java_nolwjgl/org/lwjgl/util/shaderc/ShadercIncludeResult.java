package org.lwjgl.util.shaderc;

import org.lwjgl.system.MemoryUtil;
import org.lwjgl.system.Struct;

import java.nio.ByteBuffer;

public class ShadercIncludeResult extends Struct<ShadercIncludeResult> {
    public static final int SIZEOF = 40;
    public static final int SOURCE_NAME = 0;
    public static final int CONTENT = 16;

    public ShadercIncludeResult(long address, ByteBuffer container) {
        super(address, container);
    }

    @Override
    public int sizeof() { return SIZEOF; }

    public static ShadercIncludeResult calloc() {
        long addr = MemoryUtil.nmemCalloc(1L, SIZEOF);
        return new ShadercIncludeResult(addr, null);
    }

    public ShadercIncludeResult source_name(ByteBuffer value) {
        MemoryUtil.memPutAddress(address + SOURCE_NAME, value == null ? 0L : MemoryUtil.memAddress(value));
        MemoryUtil.memPutAddress(address + 8, value == null ? 0L : value.remaining());
        return this;
    }

    public ShadercIncludeResult content(ByteBuffer value) {
        MemoryUtil.memPutAddress(address + CONTENT, value == null ? 0L : MemoryUtil.memAddress(value));
        MemoryUtil.memPutAddress(address + 24, value == null ? 0L : value.remaining());
        return this;
    }
}
