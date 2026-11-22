package org.lwjgl.vulkan;

public class VkMemoryType extends org.lwjgl.system.Struct {
    public VkMemoryType(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkMemoryType(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public int heapIndex() { return 0; }
    public int propertyFlags() { return 0; }
}
