package org.lwjgl.util.vma;

import org.lwjgl.system.MemoryStack;
import org.lwjgl.system.MemoryUtil;
import org.lwjgl.system.Struct;

import java.nio.ByteBuffer;

public class VmaAllocationCreateInfo extends Struct<VmaAllocationCreateInfo> {
    public static final int SIZEOF = 48;

    public VmaAllocationCreateInfo(long address, ByteBuffer container) {
        super(address, container);
    }

    @Override
    public int sizeof() { return SIZEOF; }

    public static VmaAllocationCreateInfo calloc(MemoryStack stack) {
        ByteBuffer b = stack.calloc(8, SIZEOF);
        return new VmaAllocationCreateInfo(MemoryUtil.memAddress(b), b);
    }

    public int flags() { return MemoryUtil.memGetInt(address); }
    public VmaAllocationCreateInfo flags(int v) { MemoryUtil.memPutInt(address, v); return this; }
    public VmaAllocationCreateInfo usage(int v) { MemoryUtil.memPutInt(address + 4, v); return this; }
    public int requiredFlags() { return MemoryUtil.memGetInt(address + 8); }
    public VmaAllocationCreateInfo requiredFlags(int v) { MemoryUtil.memPutInt(address + 8, v); return this; }
    public int preferredFlags() { return MemoryUtil.memGetInt(address + 12); }
    public VmaAllocationCreateInfo preferredFlags(int v) { MemoryUtil.memPutInt(address + 12, v); return this; }
}
