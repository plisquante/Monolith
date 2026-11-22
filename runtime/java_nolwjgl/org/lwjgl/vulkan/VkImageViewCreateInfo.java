package org.lwjgl.vulkan;

public class VkImageViewCreateInfo extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkImageViewCreateInfo(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkImageViewCreateInfo(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkImageViewCreateInfo calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkImageViewCreateInfo(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkImageViewCreateInfo format(int p0) { return this; }
    public org.lwjgl.vulkan.VkImageViewCreateInfo image(long p0) { return this; }
    public org.lwjgl.vulkan.VkImageViewCreateInfo sType$Default() { return this; }
    public org.lwjgl.vulkan.VkImageSubresourceRange subresourceRange() { return null; }
    public org.lwjgl.vulkan.VkImageViewCreateInfo viewType(int p0) { return this; }
}
