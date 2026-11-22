package org.lwjgl.vulkan;

public class VkPresentInfoKHR extends org.lwjgl.system.Struct implements org.lwjgl.system.NativeResource {
    public VkPresentInfoKHR(long address, java.nio.ByteBuffer container) { super(address, container); }
    public VkPresentInfoKHR(java.nio.ByteBuffer container) { super(org.lwjgl.system.MemoryUtil.memAddress(container), container); }
    @Override public int sizeof() { return 64; }
    public static org.lwjgl.vulkan.VkPresentInfoKHR calloc(org.lwjgl.system.MemoryStack p0) { java.nio.ByteBuffer b = p0.calloc(8, 64); return new VkPresentInfoKHR(org.lwjgl.system.MemoryUtil.memAddress(b), b); }
    public org.lwjgl.vulkan.VkPresentInfoKHR pImageIndices(java.nio.IntBuffer p0) { return this; }
    public org.lwjgl.vulkan.VkPresentInfoKHR pSwapchains(java.nio.LongBuffer p0) { return this; }
    public org.lwjgl.vulkan.VkPresentInfoKHR pWaitSemaphores(java.nio.LongBuffer p0) { return this; }
    public org.lwjgl.vulkan.VkPresentInfoKHR sType$Default() { return this; }
    public org.lwjgl.vulkan.VkPresentInfoKHR swapchainCount(int p0) { return this; }
}
