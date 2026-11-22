package org.lwjgl.vulkan;

public class VkExtent3D extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkExtent3D(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkExtent3D(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public org.lwjgl.vulkan.VkExtent3D set(int p0, int p1, int p2) { return this; }
}
