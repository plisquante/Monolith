package org.lwjgl.vulkan;

public class VkPipelineVertexInputStateCreateInfo extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkPipelineVertexInputStateCreateInfo(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkPipelineVertexInputStateCreateInfo(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkPipelineVertexInputStateCreateInfo calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkPipelineVertexInputStateCreateInfo(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkPipelineVertexInputStateCreateInfo pNext(org.lwjgl.vulkan.VkPipelineVertexInputDivisorStateCreateInfoEXT p0) { return this; }
    public org.lwjgl.vulkan.VkPipelineVertexInputStateCreateInfo pVertexAttributeDescriptions(org.lwjgl.vulkan.VkVertexInputAttributeDescription.Buffer p0) { return this; }
    public org.lwjgl.vulkan.VkPipelineVertexInputStateCreateInfo pVertexBindingDescriptions(org.lwjgl.vulkan.VkVertexInputBindingDescription.Buffer p0) { return this; }
    public org.lwjgl.vulkan.VkPipelineVertexInputStateCreateInfo sType$Default() { return this; }
}
