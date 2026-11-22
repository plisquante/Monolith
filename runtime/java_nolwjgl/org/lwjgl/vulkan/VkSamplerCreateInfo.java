package org.lwjgl.vulkan;

public class VkSamplerCreateInfo extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkSamplerCreateInfo(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkSamplerCreateInfo(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public org.lwjgl.vulkan.VkSamplerCreateInfo addressModeU(int p0) { return this; }
    public org.lwjgl.vulkan.VkSamplerCreateInfo addressModeV(int p0) { return this; }
    public org.lwjgl.vulkan.VkSamplerCreateInfo anisotropyEnable(boolean p0) { return this; }
    public static org.lwjgl.vulkan.VkSamplerCreateInfo calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkSamplerCreateInfo(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkSamplerCreateInfo magFilter(int p0) { return this; }
    public org.lwjgl.vulkan.VkSamplerCreateInfo maxAnisotropy(float p0) { return this; }
    public org.lwjgl.vulkan.VkSamplerCreateInfo maxLod(float p0) { return this; }
    public org.lwjgl.vulkan.VkSamplerCreateInfo minFilter(int p0) { return this; }
    public org.lwjgl.vulkan.VkSamplerCreateInfo mipLodBias(float p0) { return this; }
    public org.lwjgl.vulkan.VkSamplerCreateInfo mipmapMode(int p0) { return this; }
    public org.lwjgl.vulkan.VkSamplerCreateInfo sType$Default() { return this; }
}
