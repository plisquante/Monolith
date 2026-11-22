package org.lwjgl.vulkan;

public class VkCommandPoolCreateInfo extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkCommandPoolCreateInfo(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkCommandPoolCreateInfo(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkCommandPoolCreateInfo calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkCommandPoolCreateInfo(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkCommandPoolCreateInfo flags(int p0) { return this; }
    public org.lwjgl.vulkan.VkCommandPoolCreateInfo queueFamilyIndex(int p0) { return this; }
    public org.lwjgl.vulkan.VkCommandPoolCreateInfo sType$Default() { return this; }
}
