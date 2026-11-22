package org.lwjgl.vulkan;

public class VkMemoryHeap extends org.lwjgl.system.Struct {
    public VkMemoryHeap(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkMemoryHeap(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public int flags() { return 0; }
    public long size() { return 0L; }
}
