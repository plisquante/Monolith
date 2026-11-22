package org.lwjgl.vulkan;

public class VkImageSubresourceLayers extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkImageSubresourceLayers(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkImageSubresourceLayers(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public org.lwjgl.vulkan.VkImageSubresourceLayers aspectMask(int p0) { return this; }
    public org.lwjgl.vulkan.VkImageSubresourceLayers baseArrayLayer(int p0) { return this; }
    public static org.lwjgl.vulkan.VkImageSubresourceLayers calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkImageSubresourceLayers(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkImageSubresourceLayers layerCount(int p0) { return this; }
    public org.lwjgl.vulkan.VkImageSubresourceLayers mipLevel(int p0) { return this; }
}
