package org.lwjgl.vulkan;

public class VkImageSubresourceRange extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkImageSubresourceRange(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkImageSubresourceRange(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public org.lwjgl.vulkan.VkImageSubresourceRange aspectMask(int p0) { return this; }
    public org.lwjgl.vulkan.VkImageSubresourceRange baseArrayLayer(int p0) { return this; }
    public org.lwjgl.vulkan.VkImageSubresourceRange baseMipLevel(int p0) { return this; }
    public static org.lwjgl.vulkan.VkImageSubresourceRange calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkImageSubresourceRange(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkImageSubresourceRange layerCount(int p0) { return this; }
    public org.lwjgl.vulkan.VkImageSubresourceRange levelCount(int p0) { return this; }
}
