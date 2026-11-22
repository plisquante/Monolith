package org.lwjgl.vulkan;

public class VkClearColorValue extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkClearColorValue(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkClearColorValue(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkClearColorValue calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkClearColorValue(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkClearColorValue float32(int p0, float p1) { return this; }
}
