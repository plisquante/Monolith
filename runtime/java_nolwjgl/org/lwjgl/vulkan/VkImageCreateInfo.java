package org.lwjgl.vulkan;

public class VkImageCreateInfo extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkImageCreateInfo(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkImageCreateInfo(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public org.lwjgl.vulkan.VkImageCreateInfo arrayLayers(int p0) { return this; }
    public static org.lwjgl.vulkan.VkImageCreateInfo calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkImageCreateInfo(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkExtent3D extent() { return null; }
    public org.lwjgl.vulkan.VkImageCreateInfo flags(int p0) { return this; }
    public org.lwjgl.vulkan.VkImageCreateInfo format(int p0) { return this; }
    public org.lwjgl.vulkan.VkImageCreateInfo imageType(int p0) { return this; }
    public org.lwjgl.vulkan.VkImageCreateInfo initialLayout(int p0) { return this; }
    public org.lwjgl.vulkan.VkImageCreateInfo mipLevels(int p0) { return this; }
    public org.lwjgl.vulkan.VkImageCreateInfo sType$Default() { return this; }
    public org.lwjgl.vulkan.VkImageCreateInfo samples(int p0) { return this; }
    public org.lwjgl.vulkan.VkImageCreateInfo sharingMode(int p0) { return this; }
    public org.lwjgl.vulkan.VkImageCreateInfo tiling(int p0) { return this; }
    public org.lwjgl.vulkan.VkImageCreateInfo usage(int p0) { return this; }
}
