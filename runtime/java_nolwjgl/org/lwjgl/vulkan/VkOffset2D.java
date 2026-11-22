package org.lwjgl.vulkan;

public class VkOffset2D extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkOffset2D(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkOffset2D(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public org.lwjgl.vulkan.VkOffset2D set(int p0, int p1) { return this; }
}
