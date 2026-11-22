package org.lwjgl.vulkan;

public class VkSemaphoreCreateInfo extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkSemaphoreCreateInfo(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkSemaphoreCreateInfo(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkSemaphoreCreateInfo calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkSemaphoreCreateInfo(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkSemaphoreCreateInfo pNext(org.lwjgl.vulkan.VkSemaphoreTypeCreateInfo p0) { return this; }
    public org.lwjgl.vulkan.VkSemaphoreCreateInfo sType$Default() { return this; }
}
