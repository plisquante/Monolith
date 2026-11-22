package org.lwjgl.vulkan;

public class VkSemaphoreTypeCreateInfo extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkSemaphoreTypeCreateInfo(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkSemaphoreTypeCreateInfo(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkSemaphoreTypeCreateInfo calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkSemaphoreTypeCreateInfo(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkSemaphoreTypeCreateInfo initialValue(long p0) { return this; }
    public org.lwjgl.vulkan.VkSemaphoreTypeCreateInfo sType$Default() { return this; }
    public org.lwjgl.vulkan.VkSemaphoreTypeCreateInfo semaphoreType(int p0) { return this; }
}
