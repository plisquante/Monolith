package org.lwjgl.vulkan;

public class VkExtent2D extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkExtent2D(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkExtent2D(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkExtent2D calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkExtent2D(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public int height() { return 0; }
    public org.lwjgl.vulkan.VkExtent2D set(int p0, int p1) { return this; }
    public int width() { return 0; }
}
