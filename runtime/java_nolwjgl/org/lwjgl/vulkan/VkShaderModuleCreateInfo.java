package org.lwjgl.vulkan;

public class VkShaderModuleCreateInfo extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkShaderModuleCreateInfo(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkShaderModuleCreateInfo(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkShaderModuleCreateInfo calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkShaderModuleCreateInfo(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkShaderModuleCreateInfo pCode(java.nio.ByteBuffer p0) { return this; }
    public org.lwjgl.vulkan.VkShaderModuleCreateInfo sType$Default() { return this; }
}
