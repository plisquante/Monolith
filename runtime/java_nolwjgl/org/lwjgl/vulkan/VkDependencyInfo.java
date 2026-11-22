package org.lwjgl.vulkan;

public class VkDependencyInfo extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkDependencyInfo(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkDependencyInfo(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkDependencyInfo calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkDependencyInfo(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkDependencyInfo pImageMemoryBarriers(org.lwjgl.vulkan.VkImageMemoryBarrier2.Buffer p0) { return this; }
    public org.lwjgl.vulkan.VkDependencyInfo pMemoryBarriers(org.lwjgl.vulkan.VkMemoryBarrier2.Buffer p0) { return this; }
    public org.lwjgl.vulkan.VkDependencyInfo sType$Default() { return this; }
}
