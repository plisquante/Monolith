package org.lwjgl.vulkan;

public class VkBufferCreateInfo extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkBufferCreateInfo(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkBufferCreateInfo(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkBufferCreateInfo calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkBufferCreateInfo(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkBufferCreateInfo pQueueFamilyIndices(java.nio.IntBuffer p0) { return this; }
    public org.lwjgl.vulkan.VkBufferCreateInfo sType$Default() { return this; }
    public org.lwjgl.vulkan.VkBufferCreateInfo sharingMode(int p0) { return this; }
    public org.lwjgl.vulkan.VkBufferCreateInfo size(long p0) { return this; }
    public org.lwjgl.vulkan.VkBufferCreateInfo usage(int p0) { return this; }
}
