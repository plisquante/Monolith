package org.lwjgl.vulkan;

public class VkPipelineVertexInputDivisorStateCreateInfoEXT extends org.lwjgl.vulkan.VkPipelineVertexInputDivisorStateCreateInfo {
    public VkPipelineVertexInputDivisorStateCreateInfoEXT(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkPipelineVertexInputDivisorStateCreateInfoEXT(java.nio.ByteBuffer container) { super(container); }
    public static org.lwjgl.vulkan.VkPipelineVertexInputDivisorStateCreateInfoEXT calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkPipelineVertexInputDivisorStateCreateInfoEXT(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkPipelineVertexInputDivisorStateCreateInfoEXT pVertexBindingDivisors(org.lwjgl.vulkan.VkVertexInputBindingDivisorDescription.Buffer p0) { return this; }
    public org.lwjgl.vulkan.VkPipelineVertexInputDivisorStateCreateInfoEXT sType$Default() { return this; }
    public int vertexBindingDivisorCount() { return 0; }
}
