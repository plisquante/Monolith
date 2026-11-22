package org.lwjgl.vulkan;

public class VkPipelineRenderingCreateInfoKHR extends org.lwjgl.vulkan.VkPipelineRenderingCreateInfo {
    public VkPipelineRenderingCreateInfoKHR(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkPipelineRenderingCreateInfoKHR(java.nio.ByteBuffer container) { super(container); }
    public static org.lwjgl.vulkan.VkPipelineRenderingCreateInfoKHR calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkPipelineRenderingCreateInfoKHR(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkPipelineRenderingCreateInfoKHR depthAttachmentFormat(int p0) { return this; }
    public org.lwjgl.vulkan.VkPipelineRenderingCreateInfoKHR pColorAttachmentFormats(java.nio.IntBuffer p0) { return this; }
    public org.lwjgl.vulkan.VkPipelineRenderingCreateInfoKHR sType$Default() { return this; }
}
