package org.lwjgl.vulkan;

public class VkCommandBufferAllocateInfo extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkCommandBufferAllocateInfo(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkCommandBufferAllocateInfo(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkCommandBufferAllocateInfo calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkCommandBufferAllocateInfo(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkCommandBufferAllocateInfo commandBufferCount(int p0) { return this; }
    public org.lwjgl.vulkan.VkCommandBufferAllocateInfo commandPool(long p0) { return this; }
    public org.lwjgl.vulkan.VkCommandBufferAllocateInfo level(int p0) { return this; }
    public org.lwjgl.vulkan.VkCommandBufferAllocateInfo sType$Default() { return this; }
}
