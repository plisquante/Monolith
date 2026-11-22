package org.lwjgl.vulkan;

public class VkPipelineMultisampleStateCreateInfo extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkPipelineMultisampleStateCreateInfo(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkPipelineMultisampleStateCreateInfo(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkPipelineMultisampleStateCreateInfo calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkPipelineMultisampleStateCreateInfo(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkPipelineMultisampleStateCreateInfo rasterizationSamples(int p0) { return this; }
    public org.lwjgl.vulkan.VkPipelineMultisampleStateCreateInfo sType$Default() { return this; }
    public org.lwjgl.vulkan.VkPipelineMultisampleStateCreateInfo sampleShadingEnable(boolean p0) { return this; }
}
