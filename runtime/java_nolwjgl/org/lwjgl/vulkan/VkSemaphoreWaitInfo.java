package org.lwjgl.vulkan;

public class VkSemaphoreWaitInfo extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkSemaphoreWaitInfo(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkSemaphoreWaitInfo(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkSemaphoreWaitInfo calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkSemaphoreWaitInfo(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkSemaphoreWaitInfo pSemaphores(java.nio.LongBuffer p0) { return this; }
    public org.lwjgl.vulkan.VkSemaphoreWaitInfo pValues(java.nio.LongBuffer p0) { return this; }
    public org.lwjgl.vulkan.VkSemaphoreWaitInfo sType$Default() { return this; }
    public org.lwjgl.vulkan.VkSemaphoreWaitInfo semaphoreCount(int p0) { return this; }
}
