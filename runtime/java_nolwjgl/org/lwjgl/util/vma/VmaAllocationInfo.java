package org.lwjgl.util.vma;

import org.lwjgl.system.MemoryStack;
import org.lwjgl.system.MemoryUtil;
import org.lwjgl.system.Struct;

import java.nio.ByteBuffer;

public class VmaAllocationInfo extends Struct<VmaAllocationInfo> {
    public static final int SIZEOF = 48;

    public VmaAllocationInfo(long address, ByteBuffer container) {
        super(address, container);
    }

    @Override
    public int sizeof() { return SIZEOF; }

    public static VmaAllocationInfo calloc(MemoryStack stack) {
        ByteBuffer b = stack.calloc(8, SIZEOF);
        return new VmaAllocationInfo(MemoryUtil.memAddress(b), b);
    }

    public int memoryType() { return MemoryUtil.memGetInt(address); }
}
