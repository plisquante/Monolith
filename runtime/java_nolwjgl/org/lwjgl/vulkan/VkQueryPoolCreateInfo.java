package org.lwjgl.vulkan;

public class VkQueryPoolCreateInfo extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkQueryPoolCreateInfo(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkQueryPoolCreateInfo(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkQueryPoolCreateInfo calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkQueryPoolCreateInfo(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkQueryPoolCreateInfo queryCount(int p0) { return this; }
    public org.lwjgl.vulkan.VkQueryPoolCreateInfo queryType(int p0) { return this; }
    public org.lwjgl.vulkan.VkQueryPoolCreateInfo sType$Default() { return this; }
}
