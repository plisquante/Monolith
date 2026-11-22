package org.lwjgl.vulkan;

public class VkPipelineInputAssemblyStateCreateInfo extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkPipelineInputAssemblyStateCreateInfo(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkPipelineInputAssemblyStateCreateInfo(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkPipelineInputAssemblyStateCreateInfo calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkPipelineInputAssemblyStateCreateInfo(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkPipelineInputAssemblyStateCreateInfo sType$Default() { return this; }
    public org.lwjgl.vulkan.VkPipelineInputAssemblyStateCreateInfo topology(int p0) { return this; }
}
