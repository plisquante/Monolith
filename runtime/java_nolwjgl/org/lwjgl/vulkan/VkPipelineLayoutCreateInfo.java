package org.lwjgl.vulkan;

public class VkPipelineLayoutCreateInfo extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkPipelineLayoutCreateInfo(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkPipelineLayoutCreateInfo(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkPipelineLayoutCreateInfo calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkPipelineLayoutCreateInfo(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkPipelineLayoutCreateInfo pPushConstantRanges(org.lwjgl.vulkan.VkPushConstantRange.Buffer p0) { return this; }
    public org.lwjgl.vulkan.VkPipelineLayoutCreateInfo pSetLayouts(java.nio.LongBuffer p0) { return this; }
    public org.lwjgl.vulkan.VkPipelineLayoutCreateInfo sType$Default() { return this; }
}
