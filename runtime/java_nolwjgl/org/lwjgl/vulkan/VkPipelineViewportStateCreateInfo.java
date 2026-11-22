package org.lwjgl.vulkan;

public class VkPipelineViewportStateCreateInfo extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkPipelineViewportStateCreateInfo(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkPipelineViewportStateCreateInfo(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkPipelineViewportStateCreateInfo calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkPipelineViewportStateCreateInfo(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkPipelineViewportStateCreateInfo sType$Default() { return this; }
    public org.lwjgl.vulkan.VkPipelineViewportStateCreateInfo scissorCount(int p0) { return this; }
    public org.lwjgl.vulkan.VkPipelineViewportStateCreateInfo viewportCount(int p0) { return this; }
}
