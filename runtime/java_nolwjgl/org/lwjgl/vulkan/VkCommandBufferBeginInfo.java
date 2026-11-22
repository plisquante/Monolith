package org.lwjgl.vulkan;

public class VkCommandBufferBeginInfo extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkCommandBufferBeginInfo(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkCommandBufferBeginInfo(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkCommandBufferBeginInfo calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkCommandBufferBeginInfo(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkCommandBufferBeginInfo flags(int p0) { return this; }
    public org.lwjgl.vulkan.VkCommandBufferBeginInfo sType$Default() { return this; }
}
