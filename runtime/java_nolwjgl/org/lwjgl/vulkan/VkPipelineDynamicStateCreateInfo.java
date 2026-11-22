package org.lwjgl.vulkan;

public class VkPipelineDynamicStateCreateInfo extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkPipelineDynamicStateCreateInfo(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkPipelineDynamicStateCreateInfo(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkPipelineDynamicStateCreateInfo calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkPipelineDynamicStateCreateInfo(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkPipelineDynamicStateCreateInfo pDynamicStates(java.nio.IntBuffer p0) { return this; }
    public org.lwjgl.vulkan.VkPipelineDynamicStateCreateInfo sType$Default() { return this; }
}
