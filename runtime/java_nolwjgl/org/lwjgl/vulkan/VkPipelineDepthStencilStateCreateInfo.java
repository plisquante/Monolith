package org.lwjgl.vulkan;

public class VkPipelineDepthStencilStateCreateInfo extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkPipelineDepthStencilStateCreateInfo(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkPipelineDepthStencilStateCreateInfo(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkPipelineDepthStencilStateCreateInfo calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkPipelineDepthStencilStateCreateInfo(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkPipelineDepthStencilStateCreateInfo depthCompareOp(int p0) { return this; }
    public org.lwjgl.vulkan.VkPipelineDepthStencilStateCreateInfo depthTestEnable(boolean p0) { return this; }
    public org.lwjgl.vulkan.VkPipelineDepthStencilStateCreateInfo depthWriteEnable(boolean p0) { return this; }
    public org.lwjgl.vulkan.VkPipelineDepthStencilStateCreateInfo sType$Default() { return this; }
}
