package org.lwjgl.vulkan;

public class VkBufferViewCreateInfo extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkBufferViewCreateInfo(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkBufferViewCreateInfo(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public org.lwjgl.vulkan.VkBufferViewCreateInfo buffer(long p0) { return this; }
    public static org.lwjgl.vulkan.VkBufferViewCreateInfo calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkBufferViewCreateInfo(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkBufferViewCreateInfo format(int p0) { return this; }
    public org.lwjgl.vulkan.VkBufferViewCreateInfo offset(long p0) { return this; }
    public org.lwjgl.vulkan.VkBufferViewCreateInfo range(long p0) { return this; }
    public org.lwjgl.vulkan.VkBufferViewCreateInfo sType$Default() { return this; }
}
