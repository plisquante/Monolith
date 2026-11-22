package org.lwjgl.vulkan;

public class VkPipelineRasterizationStateCreateInfo extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkPipelineRasterizationStateCreateInfo(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkPipelineRasterizationStateCreateInfo(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkPipelineRasterizationStateCreateInfo calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkPipelineRasterizationStateCreateInfo(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkPipelineRasterizationStateCreateInfo cullMode(int p0) { return this; }
    public org.lwjgl.vulkan.VkPipelineRasterizationStateCreateInfo depthBiasConstantFactor(float p0) { return this; }
    public org.lwjgl.vulkan.VkPipelineRasterizationStateCreateInfo depthBiasEnable(boolean p0) { return this; }
    public org.lwjgl.vulkan.VkPipelineRasterizationStateCreateInfo depthBiasSlopeFactor(float p0) { return this; }
    public org.lwjgl.vulkan.VkPipelineRasterizationStateCreateInfo frontFace(int p0) { return this; }
    public org.lwjgl.vulkan.VkPipelineRasterizationStateCreateInfo lineWidth(float p0) { return this; }
    public org.lwjgl.vulkan.VkPipelineRasterizationStateCreateInfo polygonMode(int p0) { return this; }
    public org.lwjgl.vulkan.VkPipelineRasterizationStateCreateInfo sType$Default() { return this; }
}
