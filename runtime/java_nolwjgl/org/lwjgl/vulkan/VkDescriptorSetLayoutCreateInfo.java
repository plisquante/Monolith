package org.lwjgl.vulkan;

public class VkDescriptorSetLayoutCreateInfo extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkDescriptorSetLayoutCreateInfo(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkDescriptorSetLayoutCreateInfo(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkDescriptorSetLayoutCreateInfo calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkDescriptorSetLayoutCreateInfo(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkDescriptorSetLayoutCreateInfo flags(int p0) { return this; }
    public org.lwjgl.vulkan.VkDescriptorSetLayoutCreateInfo pBindings(org.lwjgl.vulkan.VkDescriptorSetLayoutBinding.Buffer p0) { return this; }
    public org.lwjgl.vulkan.VkDescriptorSetLayoutCreateInfo sType$Default() { return this; }
}
